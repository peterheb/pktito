//! pktito's C API (vendor/pktito/core/pktito.h), and safe wrappers for the
//! parts the app uses: a model, a transcriber, the VAD's chunking.

use std::ffi::{c_char, c_int, c_long, CStr, CString};
use std::path::Path;
use std::ptr;

#[repr(C)]
pub struct RawModel {
    _private: [u8; 0],
}
#[repr(C)]
pub struct RawAsr {
    _private: [u8; 0],
}
#[repr(C)]
pub struct RawVad {
    _private: [u8; 0],
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Tok {
    pub id: i32,
    pub frame: i32,
    pub dur: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Word {
    pub tok: i32,
    pub ntok: i32,
    pub start: i32,
    pub end: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct SegParams {
    threshold: f64,
    neg_threshold: f64,
    min_speech_ms: f64,
    max_speech_s: f64,
    min_silence_ms: f64,
    speech_pad_ms: f64,
    min_silence_at_max_speech_ms: f64,
    use_max_poss_sil_at_max_speech: c_int,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Span {
    pub start: i64,
    pub end: i64,
}

const VAD_SAMPLES: usize = 512;

extern "C" {
    fn pk_model_load(path: *const c_char, err: *mut c_char, errlen: usize) -> *mut RawModel;
    fn pk_model_free(m: *mut RawModel);
    fn pk_model_name(m: *const RawModel) -> *const c_char;
    fn pk_model_rate(m: *const RawModel) -> c_int;
    fn pk_model_frame_seconds(m: *const RawModel) -> f64;
    fn pk_asr_new(m: *const RawModel, threads: c_int) -> *mut RawAsr;
    fn pk_asr_free(a: *mut RawAsr);
    fn pk_asr_run(a: *mut RawAsr, x: *const f32, n: usize, toks: *mut *const Tok) -> c_long;
    fn pk_text(m: *const RawModel, t: *const Tok, n: usize, out: *mut c_char, cap: usize) -> usize;
    fn pk_words(m: *const RawModel, t: *const Tok, n: usize, out: *mut Word) -> usize;
    fn pk_backend() -> *const c_char;
    fn pk_vad_new(m: *const RawModel) -> *mut RawVad;
    fn pk_vad_free(v: *mut RawVad);
    fn pk_vad_probs(v: *mut RawVad, x: *const f32, n: usize, probs: *mut f32);
    fn pk_seg_defaults(p: *mut SegParams);
    fn pk_segments(
        probs: *const f32, n: usize, rate: c_int, n_samples: i64, p: *const SegParams, out: *mut Span,
        cap: usize,
    ) -> usize;
    fn pk_chunks(speech: *const Span, n_speech: usize, total: i64, max_len: i64, out: *mut Span) -> usize;
}

/// The name of the backend the CPU picked, e.g. "neon-dp (ARMv8.2 NEON: SDOT int8, fp32)".
pub fn backend() -> String {
    unsafe { CStr::from_ptr(pk_backend()) }.to_string_lossy().into_owned()
}

/// A loaded .pkf: read-only, shareable across threads.
pub struct Model {
    raw: *mut RawModel,
}

unsafe impl Send for Model {}
unsafe impl Sync for Model {}

impl Model {
    pub fn load(path: &Path) -> Result<Model, String> {
        let c = CString::new(path.to_string_lossy().as_bytes()).map_err(|_| "bad path".to_string())?;
        let mut err = [0 as c_char; 512];
        let raw = unsafe { pk_model_load(c.as_ptr(), err.as_mut_ptr(), err.len()) };
        if raw.is_null() {
            return Err(unsafe { CStr::from_ptr(err.as_ptr()) }.to_string_lossy().into_owned());
        }
        Ok(Model { raw })
    }

    pub fn name(&self) -> String {
        unsafe { CStr::from_ptr(pk_model_name(self.raw)) }.to_string_lossy().into_owned()
    }

    pub fn rate(&self) -> usize {
        unsafe { pk_model_rate(self.raw) as usize }
    }

    pub fn frame_seconds(&self) -> f64 {
        unsafe { pk_model_frame_seconds(self.raw) }
    }

    /// The text of tokens, as NeMo writes it.
    pub fn text(&self, toks: &[Tok]) -> String {
        if toks.is_empty() {
            return String::new();
        }
        let mut cap = 64 + toks.len() * 16;
        loop {
            let mut buf = vec![0u8; cap];
            let len = unsafe { pk_text(self.raw, toks.as_ptr(), toks.len(), buf.as_mut_ptr() as *mut c_char, cap) };
            if len < cap {
                buf.truncate(len);
                return String::from_utf8_lossy(&buf).into_owned();
            }
            cap = len + 1;
        }
    }

    /// NeMo's words of tokens, each with its frames.
    pub fn words(&self, toks: &[Tok]) -> Vec<Word> {
        let mut out = vec![Word::default(); toks.len()];
        let n = unsafe { pk_words(self.raw, toks.as_ptr(), toks.len(), out.as_mut_ptr()) };
        out.truncate(n);
        out
    }

    /// Where a recording of n samples is cut, as `pktito` cuts it: whole if it
    /// fits in max_chunk seconds, else at the middle of its silences (Silero's
    /// VAD), every chunk max_chunk seconds at most.
    pub fn chunks(&self, x: &[f32], max_chunk: f64) -> Result<Vec<Span>, String> {
        let rate = self.rate();
        let max_len = (max_chunk * rate as f64) as i64;
        if x.len() as i64 <= max_len {
            return Ok(vec![Span { start: 0, end: x.len() as i64 }]);
        }
        let frames = x.len().div_ceil(VAD_SAMPLES);
        let mut pad = vec![0f32; frames * VAD_SAMPLES];
        pad[..x.len()].copy_from_slice(x);
        let mut probs = vec![0f32; frames];
        unsafe {
            let v = pk_vad_new(self.raw);
            if v.is_null() {
                return Err("out of memory".into());
            }
            pk_vad_probs(v, pad.as_ptr(), frames, probs.as_mut_ptr());
            pk_vad_free(v);
        }
        drop(pad);
        let mut sp = SegParams::default();
        unsafe { pk_seg_defaults(&mut sp) };
        // segments no longer than a chunk less the padding Silero puts round them
        sp.max_speech_s = max_chunk - 2.0 * sp.speech_pad_ms / 1000.0 - 0.1;
        let cap = 2 * frames + 1;
        let mut seg = vec![Span::default(); cap];
        let ns = unsafe { pk_segments(probs.as_ptr(), frames, rate as c_int, x.len() as i64, &sp, seg.as_mut_ptr(), cap) };
        let mut out = vec![Span::default(); ns + 1];
        let nc = unsafe { pk_chunks(seg.as_ptr(), ns, x.len() as i64, max_len, out.as_mut_ptr()) };
        out.truncate(nc);
        Ok(out)
    }

    pub fn raw(&self) -> *const RawModel {
        self.raw
    }
}

impl Drop for Model {
    fn drop(&mut self) {
        unsafe { pk_model_free(self.raw) };
    }
}

/// A transcriber: its own working memory and thread pool, one caller at a time.
pub struct Asr {
    raw: *mut RawAsr,
}

unsafe impl Send for Asr {}

impl Asr {
    /// threads 0: one a physical core.
    pub fn new(model: &Model, threads: i32) -> Result<Asr, String> {
        let raw = unsafe { pk_asr_new(model.raw(), threads) };
        if raw.is_null() {
            return Err("not enough memory to start transcribing".into());
        }
        Ok(Asr { raw })
    }

    /// The tokens of 16 kHz mono samples heard as one utterance.
    pub fn run(&mut self, x: &[f32]) -> Result<Vec<Tok>, String> {
        let mut toks: *const Tok = ptr::null();
        let n = unsafe { pk_asr_run(self.raw, x.as_ptr(), x.len(), &mut toks) };
        if n < 0 {
            return Err("out of memory while transcribing".into());
        }
        if n == 0 || toks.is_null() {
            return Ok(Vec::new());
        }
        Ok(unsafe { std::slice::from_raw_parts(toks, n as usize) }.to_vec())
    }
}

impl Drop for Asr {
    fn drop(&mut self) {
        unsafe { pk_asr_free(self.raw) };
    }
}
