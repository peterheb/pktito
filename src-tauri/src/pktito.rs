//! pktito's C API (vendor/pktito/core/pktito.h), and safe wrappers for the
//! parts the app uses: a model, a transcriber and its transcription of a
//! whole recording, captions.

use std::ffi::{c_char, c_int, c_long, c_void, CStr, CString};
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
pub struct Span {
    pub start: i64,
    pub end: i64,
}

/// A chunk as pk_asr_transcribe hands it over (pk_chunk_out)
#[repr(C)]
struct ChunkOut {
    i: usize,
    n: usize,
    at: Span,
    toks: *const Tok,
    count: c_long,
    retried: c_int,
    done: i64,
    total: i64,
}

/// A chunk of a recording, transcribed: its samples in the recording, its
/// tokens (frames from its start, a retried stretch's among them), whether
/// it is the last, and the samples of the chunks done so far, of all of
/// them.
pub struct Done<'a> {
    pub start: i64,
    pub end: i64,
    pub toks: &'a [Tok],
    pub last: bool,
    pub done: i64,
    pub total: i64,
}

#[repr(C)]
struct CapWord {
    text: *const c_char,
    start: i64,
    end: i64,
}

/// A cue of a subtitle track (pk_cue): words [first, first + n) from start
/// to end in milliseconds, the second line from word line2 on (0: one line).
#[repr(C)]
#[derive(Clone, Copy, Default, serde::Serialize)]
pub struct Cue {
    pub first: i32,
    pub n: i32,
    pub line2: i32,
    pub chars: i32,
    pub flags: i32,
    pub start: i64,
    pub end: i64,
}

extern "C" {
    fn pk_model_load(path: *const c_char, err: *mut c_char, errlen: usize) -> *mut RawModel;
    fn pk_model_free(m: *mut RawModel);
    fn pk_model_name(m: *const RawModel) -> *const c_char;
    fn pk_model_rate(m: *const RawModel) -> c_int;
    fn pk_model_frame_seconds(m: *const RawModel) -> f64;
    fn pk_asr_new(m: *const RawModel, threads: c_int) -> *mut RawAsr;
    fn pk_asr_free(a: *mut RawAsr);
    // o NULL: pktito's defaults, chunks of 30 s at most and the retry
    fn pk_asr_transcribe(
        a: *mut RawAsr,
        x: *const f32,
        n: usize,
        o: *const c_void,
        f: unsafe extern "C" fn(*mut c_void, *const ChunkOut) -> c_int,
        arg: *mut c_void,
    ) -> c_int;
    fn pk_asr_threads(a: *const RawAsr) -> c_int;
    fn pk_text(m: *const RawModel, t: *const Tok, n: usize, out: *mut c_char, cap: usize) -> usize;
    fn pk_words(m: *const RawModel, t: *const Tok, n: usize, out: *mut Word) -> usize;
    fn pk_backend() -> *const c_char;
    fn pk_compiler() -> *const c_char;
    fn pk_captions(
        w: *const CapWord,
        n: usize,
        english: c_int,
        media_end: i64,
        out: *mut Cue,
    ) -> c_long;
}

/// The cues of a subtitle track for words and their times in milliseconds,
/// as the CLI's --srt has them (core/captions.c): English's rules for
/// english 1, punctuation and pauses alone for 0, and for -1 English's if
/// the words are English by their commonest (core/language.c), as the
/// multilingual model's may be.
pub fn captions(
    words: &[(String, i64, i64)],
    english: i32,
    media_end: i64,
) -> Result<Vec<Cue>, String> {
    // a NUL inside a word would end it early: none comes from pk_text
    let texts: Vec<CString> = words
        .iter()
        .map(|w| CString::new(w.0.replace('\0', "")).unwrap())
        .collect();
    let raw: Vec<CapWord> = words
        .iter()
        .zip(&texts)
        .map(|(w, t)| CapWord {
            text: t.as_ptr(),
            start: w.1,
            end: w.2,
        })
        .collect();
    let mut out = vec![Cue::default(); words.len().max(1)];
    let n = unsafe {
        pk_captions(
            raw.as_ptr(),
            raw.len(),
            english as c_int,
            media_end,
            out.as_mut_ptr(),
        )
    };
    if n < 0 {
        return Err("Not enough memory".into());
    }
    out.truncate(n as usize);
    Ok(out)
}

/// The name of the backend the CPU picked, e.g. "neon-dp (ARMv8.2 NEON: SDOT int8, fp32)".
pub fn backend() -> String {
    unsafe { CStr::from_ptr(pk_backend()) }
        .to_string_lossy()
        .into_owned()
}

/// The compiler that built the engine, e.g. "gcc 11.4.0": a transcript is the
/// same from any, its speed not.
pub fn compiler() -> String {
    unsafe { CStr::from_ptr(pk_compiler()) }
        .to_string_lossy()
        .into_owned()
}

/// A loaded .pkf: read-only, shareable across threads.
pub struct Model {
    raw: *mut RawModel,
}

unsafe impl Send for Model {}
unsafe impl Sync for Model {}

impl Model {
    pub fn load(path: &Path) -> Result<Model, String> {
        let c =
            CString::new(path.to_string_lossy().as_bytes()).map_err(|_| "bad path".to_string())?;
        let mut err = [0 as c_char; 512];
        let raw = unsafe { pk_model_load(c.as_ptr(), err.as_mut_ptr(), err.len()) };
        if raw.is_null() {
            return Err(unsafe { CStr::from_ptr(err.as_ptr()) }
                .to_string_lossy()
                .into_owned());
        }
        Ok(Model { raw })
    }

    pub fn name(&self) -> String {
        unsafe { CStr::from_ptr(pk_model_name(self.raw)) }
            .to_string_lossy()
            .into_owned()
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
            let len = unsafe {
                pk_text(
                    self.raw,
                    toks.as_ptr(),
                    toks.len(),
                    buf.as_mut_ptr() as *mut c_char,
                    cap,
                )
            };
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

    /// 16 kHz mono samples of any length transcribed as pktito transcribes
    /// a recording (pk_asr_transcribe): Silero's chunks, each one's search
    /// beside the next one's encoder, and the stretches the model wrote no
    /// word for heard again. each(chunk) has the chunks in order as they
    /// are done, and returns true to stop there. Whether every chunk was
    /// done.
    pub fn transcribe<F: FnMut(&Done) -> bool>(
        &mut self,
        x: &[f32],
        each: F,
    ) -> Result<bool, String> {
        struct Call<F> {
            each: F,
            panic: Option<Box<dyn std::any::Any + Send>>,
        }
        unsafe extern "C" fn chunk<F: FnMut(&Done) -> bool>(
            arg: *mut c_void,
            c: *const ChunkOut,
        ) -> c_int {
            let (call, c) = unsafe { (&mut *(arg as *mut Call<F>), &*c) };
            let toks = if c.count > 0 {
                unsafe { std::slice::from_raw_parts(c.toks, c.count as usize) }
            } else {
                &[]
            };
            let done = Done {
                start: c.at.start,
                end: c.at.end,
                toks,
                last: c.i + 1 == c.n,
                done: c.done,
                total: c.total,
            };
            // a panic can't unwind through C: it stops the run, and goes on
            // from transcribe()
            match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| (call.each)(&done))) {
                Ok(stop) => stop as c_int,
                Err(p) => {
                    call.panic = Some(p);
                    1
                }
            }
        }
        let mut call = Call { each, panic: None };
        let rc = unsafe {
            pk_asr_transcribe(
                self.raw,
                x.as_ptr(),
                x.len(),
                ptr::null(),
                chunk::<F>,
                &mut call as *mut Call<F> as *mut c_void,
            )
        };
        if let Some(p) = call.panic {
            std::panic::resume_unwind(p);
        }
        match rc {
            0 => Ok(true),
            1 => Ok(false),
            _ => Err("out of memory while transcribing".into()),
        }
    }

    /// The threads its passes run on: one a core, or on a chip of two kinds
    /// the fast kind alone once a recording has timed it faster.
    pub fn threads(&self) -> i32 {
        unsafe { pk_asr_threads(self.raw) }
    }
}

impl Drop for Asr {
    fn drop(&mut self) {
        unsafe { pk_asr_free(self.raw) };
    }
}
