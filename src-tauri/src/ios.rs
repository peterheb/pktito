//! The iOS app's way in (ios/, in Swift): what android.rs is to the Kotlin
//! app, as C functions over JSON strings, which ios/Core/pktito_app.h
//! declares. Swift owns the screen, the files (each a path it has access
//! to), the downloads (URLSession, which goes on while the app is in the
//! background) and the unpacking of a model's Core ML encoder; this owns
//! what the desktop's Rust owns, the same code, so a recording gives the
//! same transcript here as on every other platform.
//!
//! What can fail comes back as a JSON object with "error", its reason in
//! English, in place of the result. Every string returned is the caller's,
//! to free with pkapp_free_string. A panic is caught and comes back as an
//! error; it never unwinds into Swift.

use crate::{audio, decode, format, model, pktito, recent, transcribe};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::ffi::{c_char, c_void, CStr, CString};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

/// What pkapp_load's handle stands for: a model and its transcriber, one
/// transcription at a time.
struct Loaded {
    // declared first, so dropped first: it reads the model
    asr: pktito::Asr,
    model: pktito::Model,
}

/// A transcript as Swift gets it, and gives back to pkapp_format and
/// pkapp_captions: android.rs's, with the name of the model that made it.
#[derive(Serialize, Deserialize)]
struct Transcript {
    duration: f64,
    /// false where pkapp_stop ended it before the end
    complete: bool,
    model: String,
    segments: Vec<transcribe::Segment>,
}

/// pkapp_stop's word to the transcription running, read before each chunk.
static STOP: AtomicBool = AtomicBool::new(false);

fn arg(p: *const c_char) -> String {
    if p.is_null() {
        return String::new();
    }
    unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned()
}

fn give(s: String) -> *mut c_char {
    // a NUL inside would end the string early: JSON escapes every one
    CString::new(s.replace('\0', ""))
        .unwrap_or_default()
        .into_raw()
}

/// f's JSON, or {"error": why}; a panic is an error too.
fn answer(f: impl FnOnce() -> Result<serde_json::Value, String>) -> *mut c_char {
    let v = match catch_unwind(AssertUnwindSafe(f)) {
        Ok(Ok(v)) => v,
        Ok(Err(why)) => json!({ "error": why }),
        Err(_) => json!({ "error": "pktito stopped on an internal error" }),
    };
    give(v.to_string())
}

#[unsafe(no_mangle)]
pub extern "C" fn pkapp_free_string(s: *mut c_char) {
    if !s.is_null() {
        drop(unsafe { CString::from_raw(s) });
    }
}

/// The engine's code for this processor ("neon-dp (ARMv8.2 NEON: SDOT
/// int8, fp32)"), for About's version line.
#[unsafe(no_mangle)]
pub extern "C" fn pkapp_backend() -> *mut c_char {
    give(pktito::backend())
}

/// The models models.pktito.net serves, as JSON: model.rs's entries, each
/// with the addresses of its .pkf and of its Core ML edition's two files.
#[unsafe(no_mangle)]
pub extern "C" fn pkapp_catalog() -> *mut c_char {
    answer(|| {
        let offers: Vec<serde_json::Value> = model::CATALOG
            .iter()
            .map(|e| {
                let mut v = serde_json::to_value(e).unwrap_or_default();
                v["url"] = json!(model::download_url(e));
                v["coreml"]["slim"]["url"] = json!(model::part_url(&e.coreml.slim));
                v["coreml"]["encoder"]["url"] = json!(model::part_url(&e.coreml.encoder));
                v
            })
            .collect();
        Ok(json!(offers))
    })
}

/// The model at path and a transcriber for it, on one thread a core (the
/// engine's choice among them), as the window's: {"handle", "name",
/// "encoder": whether the file holds one (not a slim one), "threads"}.
#[unsafe(no_mangle)]
pub extern "C" fn pkapp_load(path: *const c_char) -> *mut c_char {
    let path = arg(path);
    answer(|| {
        let model = pktito::Model::load(Path::new(&path))?;
        let asr = pktito::Asr::new(&model, transcribe::threads_wanted())?;
        let about = json!({
            "name": model.name(),
            "encoder": model.has_encoder(),
            "threads": asr.threads(),
        });
        let handle = Box::into_raw(Box::new(Loaded { asr, model })) as usize as u64;
        let mut v = about;
        v["handle"] = json!(handle);
        Ok(v)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn pkapp_free(handle: u64) {
    if handle != 0 {
        drop(unsafe { Box::from_raw(handle as usize as *mut Loaded) });
    }
}

/// The encoder on the Neural Engine for handle's model: the compiled Core
/// ML model at path (a directory, NAME.mlmodelc), made from the model's own
/// checkpoint, loaded and run once on silence, so that a device that can't
/// run it says so here, with Core ML's reason, before any recording; NULL
/// for the model's own encoder again. {} or {"error"}.
#[unsafe(no_mangle)]
pub extern "C" fn pkapp_use_ane(handle: u64, path: *const c_char) -> *mut c_char {
    let path = (!path.is_null()).then(|| PathBuf::from(arg(path)));
    answer(|| {
        let l = unsafe { &mut *(handle as usize as *mut Loaded) };
        l.asr.use_ane(path.as_deref())?;
        Ok(json!({}))
    })
}

/// The recording at path (name: what to call it in an error), read and
/// transcribed, as a Transcript's JSON. Once it is read, audio(ctx, seconds,
/// a video's frame rate or 0, the loudness in audio::PEAKS slices, their
/// count) draws the strip; update(ctx, fraction, the segments just done, in
/// JSON) after each chunk. All on this thread.
#[unsafe(no_mangle)]
pub extern "C" fn pkapp_transcribe(
    handle: u64,
    path: *const c_char,
    name: *const c_char,
    ctx: *mut c_void,
    audio_fn: extern "C" fn(*mut c_void, f64, f64, *const u8, usize),
    update_fn: extern "C" fn(*mut c_void, f64, *const c_char),
) -> *mut c_char {
    let path = PathBuf::from(arg(path));
    let name = arg(name);
    answer(|| {
        let l = unsafe { &mut *(handle as usize as *mut Loaded) };
        STOP.store(false, Ordering::Relaxed);
        let fps = decode::frame_rate(&path).unwrap_or(0.0);
        // a file that can't be read: its words' key and what said so, for
        // the app's catalog to word (Words.swift), the line for a log
        let x = match decode::decode(&path) {
            Ok(x) => x,
            Err(e) => {
                let p = e.problem();
                let line = if name.is_empty() {
                    p.line()
                } else {
                    format!("{name}: {}", p.line())
                };
                return Ok(json!({ "error": line, "key": p.key, "detail": p.detail }));
            }
        };
        let duration = x.len() as f64 / decode::RATE as f64;
        let peaks = audio::peaks(&x, audio::PEAKS);
        audio_fn(ctx, duration, fps, peaks.as_ptr(), peaks.len());
        let (segments, complete) = transcribe::run(
            &l.model,
            &mut l.asr,
            &x,
            |done, segs| {
                let segs = CString::new(
                    serde_json::to_string(segs)
                        .unwrap_or_default()
                        .replace('\0', ""),
                )
                .unwrap_or_default();
                update_fn(ctx, done, segs.as_ptr());
            },
            || STOP.load(Ordering::Relaxed),
        )?;
        let t = Transcript {
            duration,
            complete,
            model: l.model.name(),
            segments,
        };
        serde_json::to_value(&t).map_err(|e| e.to_string())
    })
}

/// The transcription running ends before its next chunk.
#[unsafe(no_mangle)]
pub extern "C" fn pkapp_stop() {
    STOP.store(true, Ordering::Relaxed);
}

fn parse(transcript: *const c_char) -> Result<Transcript, String> {
    serde_json::from_str(&arg(transcript)).map_err(|e| e.to_string())
}

/// A Transcript's JSON as a file of the kind ext names (txt, md, srt, vtt,
/// json), the desktop's; times puts each sentence's time before it in txt
/// and md. {"text"}.
#[unsafe(no_mangle)]
pub extern "C" fn pkapp_format(
    transcript: *const c_char,
    ext: *const c_char,
    name: *const c_char,
    times: bool,
) -> *mut c_char {
    let (ext, name) = (arg(ext), arg(name));
    answer(|| {
        let t = parse(transcript)?;
        let text = format::text(
            &format::Transcript {
                name: &name,
                duration: t.duration,
                model: Some(&t.model),
                segments: &t.segments,
            },
            &ext,
            times,
        )?;
        Ok(json!({ "text": text }))
    })
}

/// A Transcript's subtitle track, the cues its .srt and .vtt have
/// (format::cues), for the Captions view: {"cues"}.
#[unsafe(no_mangle)]
pub extern "C" fn pkapp_captions(transcript: *const c_char) -> *mut c_char {
    answer(|| {
        let t = parse(transcript)?;
        let cues = format::cues(&format::Transcript {
            name: "",
            duration: t.duration,
            model: Some(&t.model),
            segments: &t.segments,
        })?;
        Ok(json!({ "cues": cues }))
    })
}

fn listed(data: &Path) -> serde_json::Value {
    json!({ "items": recent::list(data) })
}

/// The recent list (recent.rs), newest first: {"items"}. data is the app's
/// folder for it (Recents.data), which iOS backs up, as it does the
/// settings.
#[unsafe(no_mangle)]
pub extern "C" fn pkapp_recent(data: *const c_char) -> *mut c_char {
    let data = PathBuf::from(arg(data));
    answer(|| Ok(listed(&data)))
}

/// A finished transcript at the top of the recent list, in place of an
/// earlier one of the same file: item is recent.rs's Item, as JSON,
/// transcript what pkapp_transcribe gave, peaks its loudness. The list
/// after.
#[unsafe(no_mangle)]
pub extern "C" fn pkapp_keep(
    data: *const c_char,
    item: *const c_char,
    transcript: *const c_char,
    peaks: *const u8,
    n: usize,
) -> *mut c_char {
    let (data, item) = (PathBuf::from(arg(data)), arg(item));
    let peaks = if peaks.is_null() {
        Vec::new()
    } else {
        unsafe { std::slice::from_raw_parts(peaks, n) }.to_vec()
    };
    answer(|| {
        let item: recent::Item = serde_json::from_str(&item).map_err(|e| e.to_string())?;
        let t = parse(transcript)?;
        recent::add(
            &data,
            &recent::Transcript {
                item,
                peaks,
                segments: t.segments,
            },
        )?;
        Ok(listed(&data))
    })
}

/// A transcript of the recent list as recent.rs keeps it: its Item's
/// fields, its loudness (peaks) and its segments.
#[unsafe(no_mangle)]
pub extern "C" fn pkapp_open_recent(data: *const c_char, id: *const c_char) -> *mut c_char {
    let (data, id) = (PathBuf::from(arg(data)), arg(id));
    answer(|| {
        let t = recent::load(&data, &id).map_err(|p| p.line())?;
        serde_json::to_value(&t).map_err(|e| e.to_string())
    })
}

/// One transcript off the recent list; the file it was made from is not
/// touched. The list after.
#[unsafe(no_mangle)]
pub extern "C" fn pkapp_forget(data: *const c_char, id: *const c_char) -> *mut c_char {
    let (data, id) = (PathBuf::from(arg(data)), arg(id));
    answer(|| {
        recent::remove(&data, &id)?;
        Ok(listed(&data))
    })
}

/// The recent list emptied, once the person says so.
#[unsafe(no_mangle)]
pub extern "C" fn pkapp_clear_recent(data: *const c_char) {
    let data = PathBuf::from(arg(data));
    let _ = catch_unwind(AssertUnwindSafe(|| recent::clear(&data)));
}

/// A transcript's file found again, at path, with its size and its last
/// change in seconds since 1970, in place of the ones it was kept with
/// (recent::relink). The list after.
#[unsafe(no_mangle)]
pub extern "C" fn pkapp_relink(
    data: *const c_char,
    id: *const c_char,
    path: *const c_char,
    size: u64,
    modified: u64,
) -> *mut c_char {
    let (data, id, path) = (PathBuf::from(arg(data)), arg(id), PathBuf::from(arg(path)));
    answer(|| {
        recent::relink(&data, &id, &path, size, modified)?;
        Ok(listed(&data))
    })
}

/// What a model file that pkapp_load refused for why means to its user, as
/// the catalog's key ("model_file.not_model"), for the app's words, as the
/// window words it (model::trouble): {"key"}.
#[unsafe(no_mangle)]
pub extern "C" fn pkapp_trouble(why: *const c_char) -> *mut c_char {
    let why = arg(why);
    answer(|| Ok(json!({ "key": model::trouble(&why) })))
}
