//! The iOS app's way in (ios/, in Swift): what android.rs is to the Kotlin
//! app, as C functions over JSON strings, which ios/Core/pktito_app.h
//! declares. Swift owns the screen, the files (each a path it has access
//! to) and the transfers of the downloads (URLSession, which goes on while
//! the app is in the background); this owns what the desktop's Rust owns,
//! the same code: a recording gives the same transcript here as on every
//! other platform, and the models folder is kept by the same rule
//! (editions.rs), each download checked and put in place, a Core ML
//! encoder unpacked, an edition the Neural Engine refused recorded.
//!
//! What can fail comes back as a JSON object with "error", its reason in
//! English, in place of the result. Every string returned is the caller's,
//! to free with pkapp_free_string. A panic is caught and comes back as an
//! error; it never unwinds into Swift.

use crate::{audio, decode, editions, format, model, pktito, recent, transcribe};
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

/// A transcript as Swift gets it, and gives back to pkapp_format:
/// android.rs's, with the name of the model that made it.
#[derive(Serialize, Deserialize)]
struct Transcript {
    duration: f64,
    /// false where pkapp_stop ended it before the end
    complete: bool,
    model: String,
    segments: Vec<transcribe::Segment>,
    /// With speakers: the names given in place of Speaker N, by number from
    /// 1 ("" keeps Speaker N), and each speaker's stretches for the bar.
    /// Left out without them, so a transcript without speakers is the JSON
    /// it was.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    names: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    stretches: Vec<transcribe::Stretch>,
    /// And the diarizer that found them, its model's name, for the JSON.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    diarizer: String,
    /// Its speakers hidden by its own switch (recent.rs, Transcript).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    hidden: bool,
}

impl Transcript {
    /// What format.rs takes of it, named name: with its speakers where the
    /// switch shows them (speakers), as android.rs's has it, and with none
    /// where the diarizer heard no one.
    fn formatted<'t>(&'t self, name: &'t str, speakers: bool) -> format::Transcript<'t> {
        let speakers =
            speakers || format::no_one_spoke(&self.diarizer, self.hidden, &self.segments);
        format::Transcript {
            name,
            duration: self.duration,
            model: Some(&self.model),
            segments: &self.segments,
            names: speakers.then_some(&self.names[..]),
            diarizer: Some(self.diarizer.as_str()).filter(|d| !d.is_empty()),
            name_lone: false,
        }
    }
}

/// What pkapp_load_diarizer's handle stands for: the diarizer's model and a
/// diarizer for it, for pkapp_transcribe and pkapp_find_speakers; Swift's
/// Speakers loads it for each and lets it go after, one at a time.
struct DiarLoaded {
    // declared first, so dropped first: it reads the model
    diar: pktito::Diar,
    model: pktito::Model,
}

/// pkapp_stop's word to the transcription running, read before each chunk.
static STOP: AtomicBool = AtomicBool::new(false);
/// pkapp_quiet's: the transcription running goes on without its speakers
/// from its next chunk (transcribe::Speakers::off_when).
static QUIET: AtomicBool = AtomicBool::new(false);

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

/// # Safety
///
/// s is null or a string one of these functions gave, freed once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkapp_free_string(s: *mut c_char) {
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

/// The iPhone or iPad as Swift describes it (Editions.device): editions.rs's
/// Device, in JSON.
fn device(json: &str) -> Result<editions::Device, String> {
    serde_json::from_str(json).map_err(|err| format!("device: {err}"))
}

/// The catalog model name; why not, for one it doesn't list.
fn entry(name: &str) -> Result<&'static model::Entry, String> {
    model::entry(name).ok_or_else(|| format!("{name}: no such model"))
}

/// e's edition id; why not, for one it doesn't have.
fn edition(e: &'static model::Entry, id: &str) -> Result<&'static editions::Edition, String> {
    e.editions
        .iter()
        .find(|ed| ed.id == id)
        .ok_or_else(|| format!("{}: no edition {id}", e.name))
}

/// The models models.pktito.net serves, each as the core's view of it in
/// dir, the app's models folder, for device (editions::view;
/// docs/EDITIONS.md): the edition chosen, what runs, why the Neural Engine
/// refused the one that fits best, and the offer, whose files come with
/// their sizes, SHA-256s and addresses for URLSession (offer.parts). No
/// model is loaded for it.
#[unsafe(no_mangle)]
pub extern "C" fn pkapp_catalog(dir: *const c_char, device: *const c_char) -> *mut c_char {
    let (dir, device) = (PathBuf::from(arg(dir)), arg(device));
    answer(|| {
        let d = self::device(&device)?;
        let views: Vec<serde_json::Value> = model::CATALOG
            .iter()
            .map(|e| {
                let view = editions::view(&dir, e, &d);
                let mut v = serde_json::to_value(&view).unwrap_or_default();
                if let Some(o) = &view.offer {
                    let parts: Vec<serde_json::Value> = editions::parts(e)
                        .into_iter()
                        .filter(|p| o.files.contains(&p.file))
                        .map(|p| {
                            let mut x = json!(p);
                            x["url"] = json!(model::part_url(p));
                            x
                        })
                        .collect();
                    v["offer"]["parts"] = json!(parts);
                }
                v
            })
            .collect();
        Ok(json!(views))
    })
}

/// The models' credits for About's Models (build.rs's CREDITS, from
/// models.json): [{name, task, by, license, license_url, source, changes?,
/// notice?}].
#[unsafe(no_mangle)]
pub extern "C" fn pkapp_credits() -> *mut c_char {
    give(model::CREDITS.to_string())
}

/// The files in dir from before 0.9.6 given their records, and swaps cut
/// short finished (editions::adopt). Once a launch, off the main thread: a
/// whole .pkf takes about 2 s.
#[unsafe(no_mangle)]
pub extern "C" fn pkapp_adopt(dir: *const c_char) {
    let dir = PathBuf::from(arg(dir));
    let _ = catch_unwind(|| editions::adopt(&dir));
}

/// name's file that URLSession brought, at got: checked against the
/// catalog's SHA-256 and put where it lands with its record (model::land,
/// editions::landing): in place, or beside the file it replaces until that
/// is let go (pkapp_settle). One that doesn't check is deleted. {"path"},
/// or {"error", "key", "detail"} with the catalog's key for the app's words.
#[unsafe(no_mangle)]
pub extern "C" fn pkapp_land(
    dir: *const c_char,
    name: *const c_char,
    file: *const c_char,
    got: *const c_char,
) -> *mut c_char {
    let (dir, name, file, got) = (PathBuf::from(arg(dir)), arg(name), arg(file), arg(got));
    answer(|| {
        let e = entry(&name)?;
        let w = model::Wanted::named(&dir, e, &file)
            .ok_or_else(|| format!("{name} has no file {file}"))?;
        Ok(match model::land(&w, Path::new(&got)) {
            Ok(path) => json!({ "path": path }),
            Err(p) => json!({ "error": p.line(), "key": p.key, "detail": p.detail }),
        })
    })
}

/// Whether name has a download that checked and waits to be put in place
/// (editions::pending).
#[unsafe(no_mangle)]
pub extern "C" fn pkapp_pending(dir: *const c_char, name: *const c_char) -> bool {
    let (dir, name) = (PathBuf::from(arg(dir)), arg(name));
    catch_unwind(|| model::entry(&name).is_some_and(|e| editions::pending(&dir, e)))
        .unwrap_or(false)
}

/// name's downloads that checked put in place over the files they replace,
/// a Core ML encoder unpacked over the one before (editions::settle). Only
/// while no engine holds name's files: the transcriber lets go of it first.
/// {} or {"error"}.
#[unsafe(no_mangle)]
pub extern "C" fn pkapp_settle(dir: *const c_char, name: *const c_char) -> *mut c_char {
    let (dir, name) = (PathBuf::from(arg(dir)), arg(name));
    answer(|| {
        editions::settle(&dir, entry(&name)?)?;
        Ok(json!({}))
    })
}

/// pkapp_settle for what replaces nothing: a Core ML encoder unpacked where
/// none is in place, right after its download, with any engine running
/// (editions::settle_new). {} or {"error"}.
#[unsafe(no_mangle)]
pub extern "C" fn pkapp_settle_new(dir: *const c_char, name: *const c_char) -> *mut c_char {
    let (dir, name) = (PathBuf::from(arg(dir)), arg(name));
    answer(|| {
        editions::settle_new(&dir, entry(&name)?)?;
        Ok(json!({}))
    })
}

/// The Neural Engine refused name's edition for why, here
/// (editions::refuse): its encoder goes either way, and the refusal is
/// recorded for device where the encoder was the catalog's. {"refused":
/// whether it was, as the refusals now say}: an Older encoder refused says
/// nothing of the catalog's, which Manage models then offers.
#[unsafe(no_mangle)]
pub extern "C" fn pkapp_refuse(
    dir: *const c_char,
    name: *const c_char,
    id: *const c_char,
    device: *const c_char,
    why: *const c_char,
) -> *mut c_char {
    let (dir, name, id) = (PathBuf::from(arg(dir)), arg(name), arg(id));
    let (device, why) = (arg(device), arg(why));
    answer(|| {
        let e = entry(&name)?;
        let ed = edition(e, &id)?;
        let d = self::device(&device)?;
        editions::refuse(&dir, e, ed, &d, &why);
        let refused = editions::Refusals::read(&dir).holds(e, ed, &d).is_some();
        Ok(json!({ "refused": refused }))
    })
}

/// A transcription finished on name's edition with the model file at
/// model; the whole .pkf deleted once the Neural Engine's edition has run
/// from its own slim file (editions::ran). Whether it was.
#[unsafe(no_mangle)]
pub extern "C" fn pkapp_ran(
    dir: *const c_char,
    name: *const c_char,
    id: *const c_char,
    model: *const c_char,
) -> bool {
    let (dir, name, id, model) = (PathBuf::from(arg(dir)), arg(name), arg(id), arg(model));
    catch_unwind(|| {
        entry(&name)
            .and_then(|e| Ok((e, edition(e, &id)?)))
            .is_ok_and(|(e, ed)| editions::ran(&dir, e, ed, Path::new(&model)))
    })
    .unwrap_or(false)
}

/// Every file of name's editions in dir, their records, parts and
/// replacements waiting, and its refusals (editions::remove_part, forget).
/// The engine lets go of it first.
#[unsafe(no_mangle)]
pub extern "C" fn pkapp_delete(dir: *const c_char, name: *const c_char) {
    let (dir, name) = (PathBuf::from(arg(dir)), arg(name));
    let _ = catch_unwind(|| {
        if let Some(e) = model::entry(&name) {
            for p in editions::parts(e) {
                editions::remove_part(&dir, p);
            }
            editions::forget(&dir, e);
        }
    });
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
/// transcribed, as a Transcript's JSON; with diar, a pkapp_load_diarizer
/// handle (0 for none), who said what too, fed ahead in the same pass
/// (transcribe::Speakers), each segment one speaker's, with the speakers'
/// stretches. Once it is read, audio(ctx, seconds, a video's frame rate or
/// 0, the loudness in audio::PEAKS slices, their count) draws the strip;
/// update(ctx, fraction, the segments just done, in JSON) after each chunk.
/// All on this thread.
#[unsafe(no_mangle)]
pub extern "C" fn pkapp_transcribe(
    handle: u64,
    diar: u64,
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
        let mut d = (diar != 0).then(|| unsafe { &mut *(diar as usize as *mut DiarLoaded) });
        // the diarizer's model's name, which the JSON names
        let heard_by = d.as_ref().map(|d| d.model.name()).unwrap_or_default();
        let mut speakers = d.as_mut().map(|d| {
            transcribe::Speakers::new(&d.model, &mut d.diar)
                .off_when(|| QUIET.load(Ordering::Relaxed))
        });
        STOP.store(false, Ordering::Relaxed);
        QUIET.store(false, Ordering::Relaxed);
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
            speakers.as_mut(),
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
        // the diarizer, where it heard the whole recording: not where the
        // switch went off before the end
        let diarizer = match speakers.as_ref().is_some_and(|s| s.heard) {
            true => heard_by,
            false => String::new(),
        };
        let t = Transcript {
            duration,
            complete,
            model: l.model.name(),
            segments,
            names: Vec::new(),
            stretches: speakers.map(|s| s.stretches).unwrap_or_default(),
            diarizer,
            hidden: false,
        };
        serde_json::to_value(&t).map_err(|e| e.to_string())
    })
}

/// The transcription running ends before its next chunk.
#[unsafe(no_mangle)]
pub extern "C" fn pkapp_stop() {
    STOP.store(true, Ordering::Relaxed);
}

/// The transcription running goes on without its speakers from its next
/// chunk, its transcript as one made without them.
#[unsafe(no_mangle)]
pub extern "C" fn pkapp_quiet() {
    QUIET.store(true, Ordering::Relaxed);
}

fn parse(transcript: *const c_char) -> Result<Transcript, String> {
    serde_json::from_str(&arg(transcript)).map_err(|e| e.to_string())
}

/// A Transcript's JSON as a file of the kind ext names (txt, md, srt, vtt,
/// json), the desktop's; times puts each sentence's time before it in txt
/// and md, and speakers each turn's name, where the switch shows them.
/// {"text"}.
#[unsafe(no_mangle)]
pub extern "C" fn pkapp_format(
    transcript: *const c_char,
    ext: *const c_char,
    name: *const c_char,
    times: bool,
    speakers: bool,
) -> *mut c_char {
    let (ext, name) = (arg(ext), arg(name));
    answer(|| {
        let t = parse(transcript)?;
        let text = format::text(&t.formatted(&name, speakers), &ext, times)?;
        Ok(json!({ "text": text }))
    })
}

/// The diarizer the app offers (model::DIARIZER), for the speakers'
/// switch, Manage models and URLSession: {"offer": {name, license, file,
/// bytes, url, kept: what a download that stopped kept of it, have: whether
/// it is in dir, the models folder}}, or {"offer": null} where this build
/// offers none.
#[unsafe(no_mangle)]
pub extern "C" fn pkapp_diarizer(dir: *const c_char) -> *mut c_char {
    let dir = PathBuf::from(arg(dir));
    answer(|| {
        let offer = model::DIARIZER.map(|d| {
            let w = model::Wanted::diarizer(&dir, &d);
            json!({
                "name": d.name,
                "license": d.license,
                "file": d.model.file,
                "bytes": d.model.bytes,
                "url": w.url,
                "kept": w.partial(),
                "have": w.here(),
            })
        });
        Ok(json!({ "offer": offer }))
    })
}

/// The speaker model's file that URLSession brought, at got: checked against
/// the catalog's SHA-256 and put in place in dir (model::land), as
/// pkapp_land puts a model's. {"path"}, or {"error", "key", "detail"}.
#[unsafe(no_mangle)]
pub extern "C" fn pkapp_land_diarizer(dir: *const c_char, got: *const c_char) -> *mut c_char {
    let (dir, got) = (PathBuf::from(arg(dir)), arg(got));
    answer(|| {
        let d = model::DIARIZER.ok_or("this build offers no speaker model")?;
        Ok(
            match model::land(&model::Wanted::diarizer(&dir, &d), Path::new(&got)) {
                Ok(path) => json!({ "path": path }),
                Err(p) => json!({ "error": p.line(), "key": p.key, "detail": p.detail }),
            },
        )
    })
}

/// The speaker model's files in dir deleted for Manage models
/// (model::delete_diarizer): the model and what a download of it kept, or
/// with part what the download kept alone. Nothing may have it loaded.
/// {} or {"error"}.
#[unsafe(no_mangle)]
pub extern "C" fn pkapp_delete_diarizer(dir: *const c_char, part: bool) -> *mut c_char {
    let dir = PathBuf::from(arg(dir));
    answer(|| {
        if let Some(d) = model::DIARIZER {
            model::delete_diarizer(&dir, &d, part)?;
        }
        Ok(json!({}))
    })
}

/// The diarizer's model at path and a diarizer for it, on one thread a
/// core, as the transcriber's: {"handle"}, for pkapp_transcribe and
/// pkapp_find_speakers, let go with pkapp_free_diarizer.
#[unsafe(no_mangle)]
pub extern "C" fn pkapp_load_diarizer(path: *const c_char) -> *mut c_char {
    let path = arg(path);
    answer(|| {
        let model = pktito::Model::load(Path::new(&path))?;
        let diar = pktito::Diar::new(&model, transcribe::threads_wanted())?;
        let handle = Box::into_raw(Box::new(DiarLoaded { diar, model })) as usize as u64;
        Ok(json!({ "handle": handle }))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn pkapp_free_diarizer(handle: u64) {
    if handle != 0 {
        drop(unsafe { Box::from_raw(handle as usize as *mut DiarLoaded) });
    }
}

/// Who said what in a finished transcript (a Transcript's JSON), from its
/// recording at path, without transcribing it again (transcribe::find): the
/// transcript with each segment one speaker's, its speakers' stretches and
/// no names yet. done(ctx, fraction of the recording heard, the bar so far)
/// a whole percent at a time, on this thread: the bar's JSON is the
/// stretches ended since the last ("closed") and those going ("open"), as
/// transcribe::SoFar has them. A recording that can't be read answers as
/// pkapp_transcribe's does.
#[unsafe(no_mangle)]
pub extern "C" fn pkapp_find_speakers(
    diar: u64,
    path: *const c_char,
    name: *const c_char,
    transcript: *const c_char,
    ctx: *mut c_void,
    done_fn: extern "C" fn(*mut c_void, f64, *const c_char),
) -> *mut c_char {
    let path = PathBuf::from(arg(path));
    let name = arg(name);
    answer(|| {
        let d = unsafe { &mut *(diar as usize as *mut DiarLoaded) };
        let heard_by = d.model.name();
        let mut t = parse(transcript)?;
        // the recording as AVFoundation reads it, the diarizer hearing it
        // as it fills (transcribe::find_growing), as Android's: the meter
        // moves from the start, where the audio was 3 s read first, and a
        // 10:40 video's speakers are found in 9.4 to 9.8 s for 10.4 to
        // 10.8 (an iPhone 16e, two runs each, 2026-11-20; the same
        // speakers and stretches)
        let x = pktito::Growing::new();
        let segments = std::mem::take(&mut t.segments);
        let (mut shown, mut sent) = (0, 0);
        let tell = |done: f64, bar: &transcribe::SoFar| {
            let pct = (done * 100.0) as u32;
            if pct <= shown {
                return;
            }
            shown = pct;
            // the bar's stretches ended since the last, and those going, as
            // the window's find has them (main.rs, Finding)
            let json = json!({ "closed": &bar.closed[sent..], "open": &bar.open }).to_string();
            sent = bar.closed.len();
            let json = CString::new(json.replace('\0', "")).unwrap_or_default();
            done_fn(ctx, done, json.as_ptr());
        };
        let (found, decoded) = std::thread::scope(|sc| {
            let decoder = sc.spawn(|| decode::decode_into(&path, &x));
            let found = transcribe::find_growing(&d.model, &mut d.diar, &x, segments, tell);
            // the diarizer failed: the decoder goes no further
            if found.is_err() {
                x.cancel();
            }
            // the decoder ends x whatever happens, so this returns
            let decoded = decoder.join().unwrap_or(Err(decode::DecodeError::Stopped));
            (found, decoded)
        });
        // the decoder's trouble before the diarizer's: a recording that
        // could not be read answers as pkapp_transcribe's does
        match decoded {
            Ok(()) | Err(decode::DecodeError::Stopped) => {}
            Err(e) => {
                let p = e.problem();
                let line = if name.is_empty() {
                    p.line()
                } else {
                    format!("{name}: {}", p.line())
                };
                return Ok(json!({ "error": line, "key": p.key, "detail": p.detail }));
            }
        }
        let (segments, stretches) = found?;
        // named though it found no one: the JSON's "speakers": []
        t.diarizer = heard_by;
        t.segments = segments;
        t.stretches = stretches;
        t.names = Vec::new();
        serde_json::to_value(&t).map_err(|e| e.to_string())
    })
}

/// A kept transcript's speakers as transcript (a Transcript's JSON) has
/// them now, found or changed on the screen: its segments, names, stretches
/// and diarizer in place of the ones it was kept with, as android.rs's
/// keepSpeakers keeps them. {} or {"error"}.
#[unsafe(no_mangle)]
pub extern "C" fn pkapp_keep_speakers(
    data: *const c_char,
    id: *const c_char,
    transcript: *const c_char,
) -> *mut c_char {
    let (data, id) = (PathBuf::from(arg(data)), arg(id));
    answer(|| {
        let t = parse(transcript)?;
        let mut k = recent::load(&data, &id).map_err(|p| p.line())?;
        k.segments = t.segments;
        k.names = t.names;
        k.stretches = t.stretches;
        k.diarizer = t.diarizer;
        k.hidden = t.hidden;
        recent::replace(&data, &k).map_err(|p| p.line())?;
        Ok(json!({}))
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
///
/// # Safety
///
/// peaks is null or n bytes the caller holds for the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkapp_keep(
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
                names: t.names,
                stretches: t.stretches,
                diarizer: t.diarizer,
                hidden: t.hidden,
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
