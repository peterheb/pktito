//! pktito's C API (vendor/pktito/core/pktito.h), and safe wrappers for the
//! parts the app uses: a model, a transcriber and its transcription of a
//! whole recording, sentences, captions.

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
    at_frame: i32,
}

/// A chunk of a recording, transcribed: its samples in the recording, its
/// tokens (frames from its start, a retried stretch's among them), whether
/// it is the last, the samples of the chunks done so far, of all of them,
/// and its start in encoder frames, rounded (pk_chunk_out.at_frame), which
/// counts its tokens' frames from the recording's start for pk_sentences.
pub struct Done<'a> {
    pub start: i64,
    pub end: i64,
    pub toks: &'a [Tok],
    pub last: bool,
    pub done: i64,
    pub total: i64,
    pub at_frame: i32,
}

/// A sentence (pk_sentence): tokens [first, first + n) of those given;
/// flags PK_SENTENCE_LONG where it was broken at 30 s, not at its end.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Sentence {
    pub first: i32,
    pub n: i32,
    pub flags: i32,
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
    // o NULL: pktito's defaults (pk_transcribe_defaults): chunks of 30.72 s at
    // most, the far side of a cut in mid-sentence heard, and the retry
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
        lang: *const c_char,
        media_end: i64,
        out: *mut Cue,
    ) -> c_long;
    fn pk_language(w: *const CapWord, n: usize) -> *const c_char;
    fn pk_sentences(
        m: *const RawModel,
        t: *const Tok,
        n: usize,
        lang: *const c_char,
        done: c_int,
        out: *mut Sentence,
    ) -> c_long;
}

// the ane fork's (vendor/pktito-ane), which the Apple builds take: the
// encoder on the Neural Engine, and the slim model file whose only encoder
// it is
#[cfg(any(target_os = "macos", target_os = "ios"))]
extern "C" {
    fn pk_model_has_encoder(m: *const RawModel) -> c_int;
    fn pk_asr_use_ane(
        a: *mut RawAsr,
        path: *const c_char,
        err: *mut c_char,
        errlen: usize,
    ) -> c_int;
    fn pk_asr_ane_warm(a: *mut RawAsr, err: *mut c_char, errlen: usize) -> c_int;
}

// Android's, from the fork's qnn branch (vendor/pktito-qnn): the encoder
// on a Snapdragon's NPU (core/qnn.h), in a build with QNN_SDK set
// (build.rs), without which it says so; the chip it is, for the graph
// compiled for it; and the fork's slim model file, whose only encoder that
// graph is
#[cfg(target_os = "android")]
extern "C" {
    fn pk_model_has_encoder(m: *const RawModel) -> c_int;
    fn pk_asr_use_qnn(
        a: *mut RawAsr,
        on: c_int,
        cache: *const c_char,
        err: *mut c_char,
        errlen: usize,
    ) -> c_int;
    fn pk_qnn_chip(chip: *mut c_char, cap: usize, err: *mut c_char, errlen: usize) -> c_int;
}

/// The phone's NPU as QNN describes it, as the graph compiled for it names
/// it (model::Graph's chip: "57,75,8,1,0"); why not where QNN finds none,
/// or the build has no QNN. QNN loads the NPU's own library to say, 137 to
/// 168 ms on the Googlebook (2026-11-03): asked once, off the screen's
/// thread.
#[cfg(target_os = "android")]
pub fn qnn_chip() -> Result<String, String> {
    let mut chip = [0 as c_char; 64];
    let mut err = [0 as c_char; 1024];
    if unsafe { pk_qnn_chip(chip.as_mut_ptr(), chip.len(), err.as_mut_ptr(), err.len()) } != 0 {
        return Err(unsafe { CStr::from_ptr(err.as_ptr()) }
            .to_string_lossy()
            .into_owned());
    }
    Ok(unsafe { CStr::from_ptr(chip.as_ptr()) }
        .to_string_lossy()
        .into_owned())
}

/// The language of words, as pk_language names it ("en", "de"): the one
/// their commonest words are commonest in, of the multilingual model's 25
/// (core/language.c); None for too few words, or none ahead by enough.
pub fn language(words: &[&str]) -> Option<String> {
    let texts: Vec<CString> = words
        .iter()
        .map(|w| CString::new(w.replace('\0', "")).unwrap())
        .collect();
    let raw: Vec<CapWord> = texts
        .iter()
        .map(|t| CapWord {
            text: t.as_ptr(),
            start: 0,
            end: 0,
        })
        .collect();
    let l = unsafe { pk_language(raw.as_ptr(), raw.len()) };
    if l.is_null() {
        return None;
    }
    Some(unsafe { CStr::from_ptr(l) }.to_string_lossy().into_owned())
}

/// The language whose subtitle rules words take: "en" for english 1 (a
/// model of English alone), none for 0, and for -1 the words' own as
/// pk_language finds it (the multilingual model's).
pub fn captions_lang(words: &[(String, i64, i64)], english: i32) -> Option<String> {
    match english {
        1 => Some("en".into()),
        0 => None,
        _ => language(&words.iter().map(|w| w.0.trim()).collect::<Vec<_>>()),
    }
}

/// The cues of a subtitle track for words and their times in milliseconds,
/// as the CLI's --srt has them (core/captions.c): English's rules where
/// lang is "en", punctuation and pauses alone for any other or None.
pub fn captions(
    words: &[(String, i64, i64)],
    lang: Option<&str>,
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
    let lang = lang.map(|l| CString::new(l).unwrap());
    let mut out = vec![Cue::default(); words.len().max(1)];
    let n = unsafe {
        pk_captions(
            raw.as_ptr(),
            raw.len(),
            lang.as_ref().map_or(ptr::null(), |l| l.as_ptr()),
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

    /// The sentences of toks, their frames counted from the recording's
    /// start (pk_sentences): lang, pk_language's code for their words or
    /// None; done, whether they run to the recording's end - if not, the
    /// words after the last sentence's end are none yet, and come again with
    /// the next chunk's.
    pub fn sentences(
        &self,
        toks: &[Tok],
        lang: Option<&str>,
        done: bool,
    ) -> Result<Vec<Sentence>, String> {
        let lang = lang.map(|l| CString::new(l).unwrap());
        let mut out = vec![Sentence::default(); toks.len()];
        let n = unsafe {
            pk_sentences(
                self.raw,
                toks.as_ptr(),
                toks.len(),
                lang.as_ref().map_or(ptr::null(), |l| l.as_ptr()),
                done as c_int,
                out.as_mut_ptr(),
            )
        };
        if n < 0 {
            return Err("not enough memory to find the sentences".into());
        }
        out.truncate(n as usize);
        Ok(out)
    }

    pub fn raw(&self) -> *const RawModel {
        self.raw
    }

    /// Whether the file holds the encoder: not a slim one (model.rs,
    /// Coreml), which transcribes only with a Core ML encoder (Asr::use_ane)
    /// or, on Android, an NPU's graph (Asr::use_qnn).
    #[cfg(any(target_os = "macos", target_os = "ios", target_os = "android"))]
    pub fn has_encoder(&self) -> bool {
        unsafe { pk_model_has_encoder(self.raw) != 0 }
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
                at_frame: c.at_frame,
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

    /// The encoder on the Neural Engine: the compiled Core ML model at path,
    /// made from the model's own checkpoint, loaded and run once on silence,
    /// so that a device that can't run it says so here, with Core ML's
    /// reason; None for the model's own encoder again.
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    pub fn use_ane(&mut self, path: Option<&Path>) -> Result<(), String> {
        let c = match path {
            Some(p) => Some(
                CString::new(p.to_string_lossy().as_bytes()).map_err(|_| "bad path".to_string())?,
            ),
            None => None,
        };
        let mut err = [0 as c_char; 1024];
        let rc = unsafe {
            let p = c.as_ref().map_or(std::ptr::null(), |c| c.as_ptr());
            if pk_asr_use_ane(self.raw, p, err.as_mut_ptr(), err.len()) != 0 {
                -1
            } else if c.is_some() {
                pk_asr_ane_warm(self.raw, err.as_mut_ptr(), err.len())
            } else {
                0
            }
        };
        if rc != 0 {
            return Err(unsafe { CStr::from_ptr(err.as_ptr()) }
                .to_string_lossy()
                .into_owned());
        }
        Ok(())
    }

    /// The encoder on the phone's NPU (a Snapdragon's Hexagon, through
    /// Qualcomm's QNN: core/qnn.h): the graph at path, compiled on a PC for
    /// this chip from the model's checkpoint, read in 0.7 s on a Galaxy
    /// S24 Ultra. Where QNN refuses it, why, and the model's own encoder on
    /// the CPU, which a slim file has not. The app ships no compiler, so a
    /// whole file is never compiled here.
    #[cfg(target_os = "android")]
    pub fn use_qnn(&mut self, cache: &Path) -> Result<(), String> {
        let c =
            CString::new(cache.to_string_lossy().as_bytes()).map_err(|_| "bad path".to_string())?;
        let mut err = [0 as c_char; 1024];
        if unsafe { pk_asr_use_qnn(self.raw, 1, c.as_ptr(), err.as_mut_ptr(), err.len()) } != 0 {
            return Err(unsafe { CStr::from_ptr(err.as_ptr()) }
                .to_string_lossy()
                .into_owned());
        }
        Ok(())
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
