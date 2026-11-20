//! The Android app's way in (android/, in Kotlin): the models and their
//! download, the model, a recording's samples, transcribing, the formats
//! and the recent list, behind the native methods of
//! net.pktito.app.Engine. Kotlin owns the screen, the files and the service
//! that keeps a transcription running; this owns what the desktop's Rust
//! owns, the same code, so a recording gives the same transcript here as on
//! every other platform.
//!
//! Each method takes the JNI environment Android hands it and upgrades it
//! (jni's with_env) before anything else; what the app gets wrong comes
//! back as an IOException with the reason, and a panic as a
//! RuntimeException, never across into Java.

use crate::{
    audio, audiocache, decode, download, editions, format, model, pktito, recent, transcribe, view,
};
use jni::errors::ThrowRuntimeExAndDefault;
use jni::objects::{JByteArray, JClass, JObject, JString, JValue};
use jni::strings::JNIString;
use jni::sys::{jboolean, jdouble, jint, jlong};
use jni::{jni_sig, jni_str, Env, EnvUnowned};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// What Engine.load's Long stands for: a model and its transcriber, one
/// transcription at a time (Kotlin's service queues them).
struct Loaded {
    // declared first, so dropped first: it reads the model
    asr: pktito::Asr,
    model: pktito::Model,
    /// its encoder on the NPU (Engine.npu)
    npu: bool,
}

/// A transcript as Kotlin gets it, in JSON, and gives back to format():
/// with the name of the model that made it, whose language its captions'
/// rules follow (format::cues), as the desktop's window names a file's own
/// model to its formats; the model loaded since may be another.
#[derive(Serialize, Deserialize)]
struct Transcript {
    duration: f64,
    /// false where stop() ended it before the end
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
    /// switch shows them (speakers), as the window's formats have them.
    fn formatted<'t>(&'t self, name: &'t str, speakers: bool) -> format::Transcript<'t> {
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

/// What Engine.loadDiarizer's Long stands for: the diarizer's model and a
/// diarizer for it, for transcribe() and findSpeakers() (Kotlin's Speakers
/// holds it while the switch is on).
struct DiarLoaded {
    // declared first, so dropped first: it reads the model
    diar: pktito::Diar,
    model: pktito::Model,
}

/// stop()'s word to the transcription running, read before it reads the
/// recording, at each percent Android's decoder reads, and before each
/// chunk; begin() clears it.
static STOP: AtomicBool = AtomicBool::new(false);
/// Speakers turned off while the run goes on (Engine.quiet): from the next
/// chunk the diarizer is fed no more (transcribe::Speakers::off_when), as
/// the desktop's switch does (main.rs, quiet).
static QUIET: AtomicBool = AtomicBool::new(false);

/// The download download() is running, for pauseDownload, cancelDownload
/// and retryDownload to tell from the screen's thread, as main.rs's
/// AppState.download is the window's; None while none runs.
static DOWNLOAD: Mutex<Option<Arc<download::Job>>> = Mutex::new(None);

fn throw(env: &mut Env, why: &str) -> jni::errors::Result<()> {
    env.throw_new(jni_str!("java/io/IOException"), JNIString::from(why))
}

/// Engine.load(path): the model at path and a transcriber for it, on one
/// thread a core (the engine's choice among them), as the window's.
#[unsafe(no_mangle)]
pub extern "system" fn Java_net_pktito_app_Engine_load<'a>(
    mut unowned: EnvUnowned<'a>,
    _class: JClass<'a>,
    path: JString<'a>,
) -> jlong {
    unowned
        .with_env(|env| -> jni::errors::Result<jlong> {
            let path = path.try_to_string(env)?;
            let loaded = pktito::Model::load(Path::new(&path)).and_then(|model| {
                let asr = pktito::Asr::new(&model, transcribe::threads_wanted())?;
                Ok(Loaded {
                    asr,
                    model,
                    npu: false,
                })
            });
            match loaded {
                Ok(l) => Ok(Box::into_raw(Box::new(l)) as jlong),
                Err(why) => throw(env, &why).map(|_| 0),
            }
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_net_pktito_app_Engine_free<'a>(
    _unowned: EnvUnowned<'a>,
    _class: JClass<'a>,
    handle: jlong,
) {
    if handle != 0 {
        drop(unsafe { Box::from_raw(handle as *mut Loaded) });
    }
}

/// Engine.about(handle): the model, the backend and the threads, for the
/// screen to say what it is running on.
#[unsafe(no_mangle)]
pub extern "system" fn Java_net_pktito_app_Engine_about<'a>(
    mut unowned: EnvUnowned<'a>,
    _class: JClass<'a>,
    handle: jlong,
) -> JObject<'a> {
    unowned
        .with_env(|env| -> jni::errors::Result<JObject<'a>> {
            let l = unsafe { &*(handle as *const Loaded) };
            let about = format!(
                "{}, {}{}, {} threads",
                l.model.name(),
                pktito::backend(),
                if l.npu {
                    ", the encoder on the NPU"
                } else {
                    ""
                },
                l.asr.threads()
            );
            Ok(env.new_string(about)?.into())
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// Where the NPU's half of QNN, its skel library, is read from: libs, the
/// app's folders for QNN's libraries (Editions.libs: the skels it copies
/// out of its assets, then its native library folder), then the system's
/// folders.
fn npu_libs(libs: &str) {
    std::env::set_var(
        "ADSP_LIBRARY_PATH",
        format!("{libs};/vendor/dsp/cdsp;/vendor/lib/rfsa/adsp;/system/lib/rfsa/adsp;/dsp"),
    );
}

/// Engine.npuChip(libs): the phone's NPU as QNN describes it (pktito::
/// qnn_chip), as editions::Runtime::Qnn's chip names the one a graph is for; an
/// IOException with why where QNN finds none: a phone of another make, a
/// Snapdragon whose NPU's library the app does not bring in libs, or a
/// build without QNN.
#[unsafe(no_mangle)]
pub extern "system" fn Java_net_pktito_app_Engine_npuChip<'a>(
    mut unowned: EnvUnowned<'a>,
    _class: JClass<'a>,
    libs: JString<'a>,
) -> JObject<'a> {
    unowned
        .with_env(|env| -> jni::errors::Result<JObject<'a>> {
            npu_libs(&libs.try_to_string(env)?);
            match pktito::qnn_chip() {
                Ok(chip) => Ok(env.new_string(chip)?.into()),
                Err(why) => throw(env, &why).map(|_| JObject::null()),
            }
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// Engine.npu(handle, graph, libs): handle's encoder on the phone's NPU
/// (pktito::Asr::use_qnn), the graph at graph, compiled for this chip:
/// the seconds QNN took to read it, or an IOException with why where it
/// refused it, the model's own encoder then the one, where it has one.
/// Under PKTITO_REFUSE_NPU=1 (editions::refusing_for_test) every graph is
/// refused, for a refusal's path on any phone.
#[unsafe(no_mangle)]
pub extern "system" fn Java_net_pktito_app_Engine_npu<'a>(
    mut unowned: EnvUnowned<'a>,
    _class: JClass<'a>,
    handle: jlong,
    graph: JString<'a>,
    libs: JString<'a>,
) -> jdouble {
    unowned
        .with_env(|env| -> jni::errors::Result<jdouble> {
            let l = unsafe { &mut *(handle as *mut Loaded) };
            let graph = graph.try_to_string(env)?;
            if editions::refusing_for_test() {
                return throw(env, "PKTITO_REFUSE_NPU=1").map(|_| 0.0);
            }
            npu_libs(&libs.try_to_string(env)?);
            let t = std::time::Instant::now();
            match l.asr.use_qnn(Path::new(&graph)) {
                Ok(()) => {
                    l.npu = true;
                    Ok(t.elapsed().as_secs_f64())
                }
                Err(why) => throw(env, &why).map(|_| 0.0),
            }
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// Engine.transcribe(handle, diar, fd, name, kept, progress): the recording
/// open at fd (name: what to call it in an error), read and transcribed, as a
/// Transcript's JSON; with diar (loadDiarizer's, or 0 for none) each
/// segment one speaker's and saying whose, the diarizer fed in the same
/// pass (transcribe::Speakers), and the speakers' stretches with them. While
/// Android's decoder reads it, progress.reading(fraction), a whole percent at
/// a time (decode::decode_fd). Once it is read, progress.audio(seconds, a video's
/// frame rate or 0, the loudness in audio::PEAKS slices) draws the strip;
/// progress.update(fraction, the segments just done, in JSON) after each
/// chunk. All on this thread. The screen plays the recording itself (the
/// app's Player.kt, view.rs); its samples, read to the end, are kept for
/// kept's transcript (Engine.Kept, audiocache), and read from there in place
/// of the decoder the next time, as findSpeakers reads them. A
/// recording that can't be read throws its problem as JSON, the catalog's
/// key and what said so: the app words the key from its own catalog and
/// keeps the particulars to its log (Engine.problem), as iOS's does
/// (ios.rs). Stopped (Engine.stop) before or while the recording is read,
/// it is a transcript of nothing, not complete, with no duration.
#[unsafe(no_mangle)]
pub extern "system" fn Java_net_pktito_app_Engine_transcribe<'a>(
    mut unowned: EnvUnowned<'a>,
    _class: JClass<'a>,
    handle: jlong,
    diar: jlong,
    fd: jint,
    name: JString<'a>,
    kept: JObject<'a>,
    progress: JObject<'a>,
) -> JObject<'a> {
    unowned
        .with_env(|env| -> jni::errors::Result<JObject<'a>> {
            let l = unsafe { &mut *(handle as *mut Loaded) };
            let kept = kept_as(env, &kept)?;
            let mut d = (diar != 0).then(|| unsafe { &mut *(diar as *mut DiarLoaded) });
            // the diarizer's model's name, which the JSON names
            let heard_by = d.as_ref().map(|d| d.model.name()).unwrap_or_default();
            let mut speakers = d.as_mut().map(|d| {
                transcribe::Speakers::new(&d.model, &mut d.diar)
                    .off_when(|| QUIET.load(Ordering::Relaxed))
            });
            let name = name.try_to_string(env)?;
            // stopped while the model loaded, or while the recording is read:
            // nothing of it transcribed
            let nothing = || {
                let t = Transcript {
                    duration: 0.0,
                    complete: false,
                    model: l.model.name(),
                    segments: Vec::new(),
                    names: Vec::new(),
                    stretches: Vec::new(),
                    diarizer: String::new(),
                    hidden: false,
                };
                serde_json::to_string(&t).unwrap_or_default()
            };
            if STOP.load(Ordering::Relaxed) {
                return Ok(env.new_string(nothing())?.into());
            }
            let fps = decode::frame_rate_fd(fd).unwrap_or(0.0);
            // read before (audiocache): its samples as they were, in one
            // push, in place of the decoder
            let hit = kept
                .as_ref()
                .and_then(|(dir, id, src)| audiocache::load(dir, id, src));
            let from_cache = hit.is_some();
            // the recording transcribed as Android's decoder reads it
            // (LIB-8): the decoder on a thread of its own, filling x, the
            // transcription reading it as it fills; Stop, while the
            // decoder goes, cancels x, which ends both
            let x = pktito::Growing::new();
            if let Some(y) = hit {
                x.set_estimate(y.len());
                x.push(&y);
                x.end();
            }
            let (run, decoded, failed) = std::thread::scope(|sc| {
                let decoder = (!from_cache).then(|| {
                    sc.spawn(|| {
                        decode::decode_fd_into(
                            fd,
                            &name,
                            &mut |_| {
                                if STOP.load(Ordering::Relaxed) {
                                    x.cancel();
                                }
                                x.cancelled()
                            },
                            &x,
                        )
                    })
                });
                let mut failed = None;
                // the waveform as the decoder passes it, over the length the
                // container gives until the end, said with each chunk
                let mut rising: Option<audio::Rising> = None;
                let audio = |env: &mut Env, duration: f64, peaks: &[u8]| {
                    env.with_local_frame(2, |env| -> jni::errors::Result<()> {
                        let peaks = env.byte_array_from_slice(peaks)?;
                        env.call_method(
                            &progress,
                            jni_str!("audio"),
                            jni_sig!("(DD[B)V"),
                            &[
                                JValue::Double(duration),
                                JValue::Double(fps),
                                JValue::from(&peaks),
                            ],
                        )?;
                        Ok(())
                    })
                };
                let run = transcribe::run_growing(
                    &l.model,
                    &mut l.asr,
                    &x,
                    speakers.as_mut(),
                    |done, segs| {
                        if failed.is_some() {
                            return;
                        }
                        let estimate = x.estimate();
                        let r = rising
                            .get_or_insert_with(|| audio::Rising::new(estimate, audio::PEAKS));
                        if let Ok(new) = x.slice(r.seen(), x.len()) {
                            r.add(&new);
                        }
                        let duration = estimate as f64 / decode::RATE as f64;
                        let peaks = r.peaks();
                        let segs = serde_json::to_string(segs).unwrap_or_default();
                        // a frame of its own, so a long recording's calls leave
                        // no local references behind
                        let call = audio(env, duration, &peaks).and_then(|_| {
                            env.with_local_frame(2, |env| -> jni::errors::Result<()> {
                                let segs = env.new_string(segs)?;
                                env.call_method(
                                    &progress,
                                    jni_str!("update"),
                                    jni_sig!("(DLjava/lang/String;)V"),
                                    &[JValue::Double(done), JValue::from(&segs)],
                                )?;
                                Ok(())
                            })
                        });
                        if let Err(e) = call {
                            failed.get_or_insert(e);
                            STOP.store(true, Ordering::Relaxed);
                            x.cancel();
                        }
                    },
                    || STOP.load(Ordering::Relaxed),
                );
                // the decoder ends x whatever happens, so this returns
                let decoded = match decoder {
                    Some(d) => d.join().unwrap_or(Err(decode::DecodeError::Stopped)),
                    None => Ok(()),
                };
                (run, decoded, failed)
            });
            if let Some(e) = failed {
                return Err(e);
            }
            // the decoder's trouble before the transcription's: a recording
            // that could not be read is said as one
            // Stop while it read: what was transcribed by then, a stopped
            // transcript, as Stop between chunks gives
            match decoded {
                Ok(()) | Err(decode::DecodeError::Stopped) => {}
                Err(e) => {
                    let p = e.problem();
                    let why = serde_json::json!({ "key": p.key, "detail": p.detail });
                    return throw(env, &why.to_string()).map(|_| JObject::null());
                }
            }
            // stopped before the decoder's end: the length the container
            // gives, and the waveform as far as it got
            let length = x.estimate();
            let whole = x.ended();
            let x = x.into_vec();
            let duration = length as f64 / decode::RATE as f64;
            if x.is_empty() {
                return Ok(env.new_string(nothing())?.into());
            }
            // read to its end, kept for finding its speakers or transcribing
            // it again; a cache that can't be written is only slower later
            if whole && !from_cache {
                if let Some((dir, id, src)) = &kept {
                    let _ = audiocache::save(dir, id, src, &x);
                }
            }
            // the waveform of the whole recording, its length now known
            let peaks = match whole {
                true => audio::peaks(&x, audio::PEAKS),
                false => {
                    let mut r = audio::Rising::new(length, audio::PEAKS);
                    r.add(&x);
                    r.peaks()
                }
            };
            env.with_local_frame(2, |env| -> jni::errors::Result<()> {
                let peaks = env.byte_array_from_slice(&peaks)?;
                env.call_method(
                    &progress,
                    jni_str!("audio"),
                    jni_sig!("(DD[B)V"),
                    &[
                        JValue::Double(duration),
                        JValue::Double(fps),
                        JValue::from(&peaks),
                    ],
                )?;
                Ok(())
            })?;
            let (segments, complete) = match run {
                Ok(r) => r,
                // a slim file's encoder is the NPU's alone: the NPU failed a
                // chunk, which a whole file's encoder would have run on the
                // CPU (core/encoder.c); said with its key, as a recording's
                // problem is, for the app to fall back to the whole file
                Err(why) if !l.model.has_encoder() => {
                    let why = serde_json::json!({ "key": "npu.failed", "detail": why });
                    return throw(env, &why.to_string()).map(|_| JObject::null());
                }
                Err(why) => return throw(env, &why).map(|_| JObject::null()),
            };
            // the diarizer, where it found speakers: not where the switch
            // went off before the end
            let diarizer = match segments.iter().any(|s| s.speaker.is_some()) {
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
            let json = serde_json::to_string(&t).unwrap_or_default();
            Ok(env.new_string(json)?.into())
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// Engine.begin(): a new transcription is on its way, which no stop() or
/// quiet() before this is meant for. Called as the service takes the
/// recording, not as transcribe() starts: the model may load between the
/// two, and a Stop tapped then was lost (0.9.8).
#[unsafe(no_mangle)]
pub extern "system" fn Java_net_pktito_app_Engine_begin<'a>(
    _unowned: EnvUnowned<'a>,
    _class: JClass<'a>,
) {
    STOP.store(false, Ordering::Relaxed);
    QUIET.store(false, Ordering::Relaxed);
}

/// Engine.stop(): the transcription running ends where it is: before it
/// reads the recording, within a percent of Android's decoder's reading
/// of it, or before its next chunk.
#[unsafe(no_mangle)]
pub extern "system" fn Java_net_pktito_app_Engine_stop<'a>(
    _unowned: EnvUnowned<'a>,
    _class: JClass<'a>,
) {
    STOP.store(true, Ordering::Relaxed);
}

/// Engine.quiet(): the transcription running goes on without its speakers
/// from its next chunk (QUIET), its transcript as one made without them.
#[unsafe(no_mangle)]
pub extern "system" fn Java_net_pktito_app_Engine_quiet<'a>(
    _unowned: EnvUnowned<'a>,
    _class: JClass<'a>,
) {
    QUIET.store(true, Ordering::Relaxed);
}

/// Engine.format(transcript, ext, name, times, speakers): a Transcript's
/// JSON as a file of the kind ext names (txt, md, srt, vtt, json), the
/// desktop's; times puts each sentence's time before it in txt and md, and
/// speakers each turn's name, where the switch shows them.
#[unsafe(no_mangle)]
pub extern "system" fn Java_net_pktito_app_Engine_format<'a>(
    mut unowned: EnvUnowned<'a>,
    _class: JClass<'a>,
    transcript: JString<'a>,
    ext: JString<'a>,
    name: JString<'a>,
    times: jboolean,
    speakers: jboolean,
) -> JObject<'a> {
    unowned
        .with_env(|env| -> jni::errors::Result<JObject<'a>> {
            let json = transcript.try_to_string(env)?;
            let ext = ext.try_to_string(env)?;
            let name = name.try_to_string(env)?;
            let t: Transcript = match serde_json::from_str(&json) {
                Ok(t) => t,
                Err(e) => return throw(env, &e.to_string()).map(|_| JObject::null()),
            };
            let text = format::text(&t.formatted(&name, speakers), &ext, times);
            match text {
                Ok(text) => Ok(env.new_string(text)?.into()),
                Err(why) => throw(env, &why).map(|_| JObject::null()),
            }
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// Engine.diarizer(dir): the diarizer the app offers (model::DIARIZER), as
/// JSON: its name, licence, file and bytes, what a download that stopped
/// kept of it, and whether it is in dir, the app's models folder; "" where
/// this build offers none.
#[unsafe(no_mangle)]
pub extern "system" fn Java_net_pktito_app_Engine_diarizer<'a>(
    mut unowned: EnvUnowned<'a>,
    _class: JClass<'a>,
    dir: JString<'a>,
) -> JObject<'a> {
    unowned
        .with_env(|env| -> jni::errors::Result<JObject<'a>> {
            let dir = PathBuf::from(dir.try_to_string(env)?);
            let json = model::DIARIZER.map_or_else(String::new, |d| {
                let w = model::Wanted::diarizer(&dir, &d);
                serde_json::json!({
                    "name": d.name,
                    "license": d.license,
                    "file": d.model.file,
                    "bytes": d.model.bytes,
                    "kept": w.partial(),
                    "have": w.here(),
                })
                .to_string()
            });
            Ok(env.new_string(json)?.into())
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// Engine.deleteDiarizer(dir, part): the speaker model's files in dir, the
/// models folder, deleted for Manage models (model::delete_diarizer): the
/// model and what a download of it kept, or with part what the download
/// kept alone. Nothing may have it loaded (Speakers.delete). Throws why it
/// couldn't.
#[unsafe(no_mangle)]
pub extern "system" fn Java_net_pktito_app_Engine_deleteDiarizer<'a>(
    mut unowned: EnvUnowned<'a>,
    _class: JClass<'a>,
    dir: JString<'a>,
    part: jboolean,
) {
    unowned
        .with_env(|env| -> jni::errors::Result<()> {
            let dir = PathBuf::from(dir.try_to_string(env)?);
            let Some(d) = model::DIARIZER else {
                return Ok(());
            };
            match model::delete_diarizer(&dir, &d, part) {
                Ok(()) => Ok(()),
                Err(why) => throw(env, &why),
            }
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// Engine.loadDiarizer(path): the diarizer's model at path and a diarizer
/// for it, on the transcriber's threads: a handle for transcribe() and
/// findSpeakers(), let go with freeDiarizer().
#[unsafe(no_mangle)]
pub extern "system" fn Java_net_pktito_app_Engine_loadDiarizer<'a>(
    mut unowned: EnvUnowned<'a>,
    _class: JClass<'a>,
    path: JString<'a>,
) -> jlong {
    unowned
        .with_env(|env| -> jni::errors::Result<jlong> {
            let path = path.try_to_string(env)?;
            let loaded = pktito::Model::load(Path::new(&path)).and_then(|model| {
                let diar = pktito::Diar::new(&model, transcribe::threads_wanted())?;
                Ok(DiarLoaded { diar, model })
            });
            match loaded {
                Ok(d) => Ok(Box::into_raw(Box::new(d)) as jlong),
                Err(why) => throw(env, &why).map(|_| 0),
            }
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_net_pktito_app_Engine_freeDiarizer<'a>(
    _unowned: EnvUnowned<'a>,
    _class: JClass<'a>,
    handle: jlong,
) {
    if handle != 0 {
        drop(unsafe { Box::from_raw(handle as *mut DiarLoaded) });
    }
}

/// Engine.Kept's fields: the cache's folder, the transcript's id, and the
/// recording as the recent list knows it (its URI, size and last change), for
/// audiocache; None for a null one, or one whose size is not known, which
/// could not tell a changed recording from the one it was.
fn kept_as<'a>(
    env: &mut Env<'a>,
    k: &JObject<'a>,
) -> jni::errors::Result<Option<(PathBuf, String, audiocache::Source)>> {
    if k.is_null() {
        return Ok(None);
    }
    let text = |env: &mut Env<'a>, field| -> jni::errors::Result<String> {
        let o = env
            .get_field(k, field, jni_sig!("Ljava/lang/String;"))?
            .l()?;
        JString::cast_local(env, o)?.try_to_string(env)
    };
    let dir = text(env, jni_str!("dir"))?;
    let id = text(env, jni_str!("id"))?;
    let what = text(env, jni_str!("what"))?;
    let size = env.get_field(k, jni_str!("size"), jni_sig!("J"))?.j()?;
    let modified = env.get_field(k, jni_str!("modified"), jni_sig!("J"))?.j()?;
    if dir.is_empty() || id.is_empty() || size <= 0 {
        return Ok(None);
    }
    let src = audiocache::Source {
        what,
        size: size as u64,
        modified,
    };
    Ok(Some((PathBuf::from(dir), id, src)))
}

/// Engine.pruneKept(dir, keep): the cache's samples (audiocache) of every
/// transcript but keep's, the recent list's ids newest first, let go, and the
/// oldest past its cap.
#[unsafe(no_mangle)]
pub extern "system" fn Java_net_pktito_app_Engine_pruneKept<'a>(
    mut unowned: EnvUnowned<'a>,
    _class: JClass<'a>,
    dir: JString<'a>,
    keep: JString<'a>,
) {
    unowned
        .with_env(|env| -> jni::errors::Result<()> {
            let dir = PathBuf::from(dir.try_to_string(env)?);
            let keep: Vec<String> =
                serde_json::from_str(&keep.try_to_string(env)?).unwrap_or_default();
            audiocache::prune(&dir, &keep);
            Ok(())
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// Engine.findSpeakers(diar, fd, name, transcript, kept, progress): who said what
/// in a finished transcript (a Transcript's JSON), from its recording open
/// at fd, without transcribing it again (transcribe::find): the transcript
/// with each segment one speaker's, its speakers' stretches, and no names
/// yet; progress.done(fraction of the recording heard, the bar so far) a
/// whole percent at a time, on this thread, the bar's JSON the stretches
/// ended since the last ("closed") and those going ("open"), as
/// transcribe::SoFar has them. A recording that can't be read throws its
/// problem as transcribe() throws it.
#[unsafe(no_mangle)]
pub extern "system" fn Java_net_pktito_app_Engine_findSpeakers<'a>(
    mut unowned: EnvUnowned<'a>,
    _class: JClass<'a>,
    diar: jlong,
    fd: jint,
    name: JString<'a>,
    transcript: JString<'a>,
    kept: JObject<'a>,
    progress: JObject<'a>,
) -> JObject<'a> {
    unowned
        .with_env(|env| -> jni::errors::Result<JObject<'a>> {
            let d = unsafe { &mut *(diar as *mut DiarLoaded) };
            let kept = kept_as(env, &kept)?;
            let heard_by = d.model.name();
            let name = name.try_to_string(env)?;
            let json = transcript.try_to_string(env)?;
            let mut t: Transcript = match serde_json::from_str(&json) {
                Ok(t) => t,
                Err(e) => return throw(env, &e.to_string()).map(|_| JObject::null()),
            };
            // how far Android's decoder has got, before the diarizer hears any
            // of it (progress.decoding)
            let mut told = None;
            // read before (audiocache): no decoding at all
            let hit = kept
                .as_ref()
                .and_then(|(dir, id, src)| audiocache::load(dir, id, src));
            let from_cache = hit.is_some();
            let x = match hit {
                Some(y) => Ok(y),
                None => decode::decode_fd(fd, &name, &mut |fraction| {
                    if told.is_none() {
                        if let Err(e) = env.call_method(
                            &progress,
                            jni_str!("decoding"),
                            jni_sig!("(D)V"),
                            &[JValue::Double(fraction)],
                        ) {
                            told = Some(e);
                        }
                    }
                    false
                }),
            };
            if let Some(e) = told {
                return Err(e);
            }
            let x = match x {
                Ok(x) => x,
                Err(e) => {
                    let p = e.problem();
                    let why = serde_json::json!({ "key": p.key, "detail": p.detail });
                    return throw(env, &why.to_string()).map(|_| JObject::null());
                }
            };
            if !from_cache {
                if let Some((dir, id, src)) = &kept {
                    let _ = audiocache::save(dir, id, src, &x);
                }
            }
            let segments = std::mem::take(&mut t.segments);
            let (mut shown, mut sent, mut failed) = (0, 0, None);
            let tell = |done: f64, bar: &transcribe::SoFar| {
                let pct = (done * 100.0) as u32;
                if pct <= shown || failed.is_some() {
                    return;
                }
                shown = pct;
                // the bar's stretches ended since the last, and those going,
                // as the window's find has them (main.rs, Finding)
                let json = serde_json::json!({ "closed": &bar.closed[sent..], "open": &bar.open })
                    .to_string();
                sent = bar.closed.len();
                // a frame of its own, so a long recording's calls leave no
                // local references behind
                let call = env.with_local_frame(2, |env| -> jni::errors::Result<()> {
                    let bar = env.new_string(json)?;
                    env.call_method(
                        &progress,
                        jni_str!("done"),
                        jni_sig!("(DLjava/lang/String;)V"),
                        &[JValue::Double(done), JValue::from(&bar)],
                    )?;
                    Ok(())
                });
                if let Err(e) = call {
                    failed = Some(e);
                }
            };
            let found = transcribe::find(&d.model, &mut d.diar, &x, segments, tell);
            if let Some(e) = failed {
                return Err(e);
            }
            match found {
                Ok((segments, stretches)) => {
                    t.diarizer = match segments.iter().any(|s| s.speaker.is_some()) {
                        true => heard_by,
                        false => String::new(),
                    };
                    t.segments = segments;
                    t.stretches = stretches;
                    t.names = Vec::new();
                    let json = serde_json::to_string(&t).unwrap_or_default();
                    Ok(env.new_string(json)?.into())
                }
                Err(why) => throw(env, &why).map(|_| JObject::null()),
            }
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// Engine.keepSpeakers(data, id, transcript): a kept transcript's speakers
/// as transcript (a Transcript's JSON) has them now, found or changed on
/// the screen: its segments, names and stretches in place of the ones it
/// was kept with, as the window's find_speakers and set_speakers keep them.
#[unsafe(no_mangle)]
pub extern "system" fn Java_net_pktito_app_Engine_keepSpeakers<'a>(
    mut unowned: EnvUnowned<'a>,
    _class: JClass<'a>,
    data: JString<'a>,
    id: JString<'a>,
    transcript: JString<'a>,
) {
    unowned
        .with_env(|env| -> jni::errors::Result<()> {
            let data = PathBuf::from(data.try_to_string(env)?);
            let id = id.try_to_string(env)?;
            let json = transcript.try_to_string(env)?;
            let t: Transcript = match serde_json::from_str(&json) {
                Ok(t) => t,
                Err(e) => return throw(env, &e.to_string()),
            };
            let kept = recent::load(&data, &id).and_then(|mut k| {
                k.segments = t.segments;
                k.names = t.names;
                k.stretches = t.stretches;
                k.diarizer = t.diarizer;
                k.hidden = t.hidden;
                recent::replace(&data, &k)
            });
            match kept {
                Ok(()) => Ok(()),
                Err(p) => throw(env, &p.line()),
            }
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// The recent list as recent() gives it.
fn listed<'a>(env: &mut Env<'a>, data: &Path) -> jni::errors::Result<JObject<'a>> {
    let items = recent::list(data);
    Ok(env
        .new_string(serde_json::to_string(&items).unwrap_or_default())?
        .into())
}

/// Engine.recent(data): the recent list (recent.rs), newest first, as JSON:
/// each transcript's id, the content URI it was read from (its path), its
/// name, length, when it finished, what it took, the model that made it
/// and a video's frame rate. data is the app's files folder, which
/// Android's backup and its move to a new phone take (Recents.data); the
/// models and the recordings' copies stay in no_backup, which they leave
/// out.
#[unsafe(no_mangle)]
pub extern "system" fn Java_net_pktito_app_Engine_recent<'a>(
    mut unowned: EnvUnowned<'a>,
    _class: JClass<'a>,
    data: JString<'a>,
) -> JObject<'a> {
    unowned
        .with_env(|env| -> jni::errors::Result<JObject<'a>> {
            let data = PathBuf::from(data.try_to_string(env)?);
            listed(env, &data)
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// Engine.keep(data, item, transcript, peaks): a finished transcript at the
/// top of the recent list, in place of an earlier one of the same file, as
/// the desktop's window keeps each (main.rs, job): item is recent.rs's
/// Item, as JSON, transcript what transcribe() gave, and peaks its
/// loudness. The list after, as recent() gives it.
#[unsafe(no_mangle)]
pub extern "system" fn Java_net_pktito_app_Engine_keep<'a>(
    mut unowned: EnvUnowned<'a>,
    _class: JClass<'a>,
    data: JString<'a>,
    item: JString<'a>,
    transcript: JString<'a>,
    peaks: JByteArray<'a>,
) -> JObject<'a> {
    unowned
        .with_env(|env| -> jni::errors::Result<JObject<'a>> {
            let data = PathBuf::from(data.try_to_string(env)?);
            let item = item.try_to_string(env)?;
            let json = transcript.try_to_string(env)?;
            let peaks = env.convert_byte_array(&peaks)?;
            let parsed = serde_json::from_str::<recent::Item>(&item)
                .and_then(|item| Ok((item, serde_json::from_str::<Transcript>(&json)?)));
            let (item, t) = match parsed {
                Ok(p) => p,
                Err(e) => return throw(env, &e.to_string()).map(|_| JObject::null()),
            };
            let t = recent::Transcript {
                item,
                peaks,
                segments: t.segments,
                names: t.names,
                stretches: t.stretches,
                diarizer: t.diarizer,
                hidden: t.hidden,
            };
            if let Err(why) = recent::add(&data, &t) {
                return throw(env, &why).map(|_| JObject::null());
            }
            listed(env, &data)
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// Engine.openRecent(data, id): a transcript of the recent list as recent.rs
/// keeps it, in JSON: its Item's fields, its loudness (peaks) and its
/// segments.
#[unsafe(no_mangle)]
pub extern "system" fn Java_net_pktito_app_Engine_openRecent<'a>(
    mut unowned: EnvUnowned<'a>,
    _class: JClass<'a>,
    data: JString<'a>,
    id: JString<'a>,
) -> JObject<'a> {
    unowned
        .with_env(|env| -> jni::errors::Result<JObject<'a>> {
            let data = PathBuf::from(data.try_to_string(env)?);
            let id = id.try_to_string(env)?;
            match recent::load(&data, &id) {
                Ok(t) => Ok(env
                    .new_string(serde_json::to_string(&t).unwrap_or_default())?
                    .into()),
                Err(p) => throw(env, &p.line()).map(|_| JObject::null()),
            }
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// Engine.forget(data, id): one transcript off the recent list; the file it
/// was made from is not touched. The list after.
#[unsafe(no_mangle)]
pub extern "system" fn Java_net_pktito_app_Engine_forget<'a>(
    mut unowned: EnvUnowned<'a>,
    _class: JClass<'a>,
    data: JString<'a>,
    id: JString<'a>,
) -> JObject<'a> {
    unowned
        .with_env(|env| -> jni::errors::Result<JObject<'a>> {
            let data = PathBuf::from(data.try_to_string(env)?);
            let id = id.try_to_string(env)?;
            if let Err(why) = recent::remove(&data, &id) {
                return throw(env, &why).map(|_| JObject::null());
            }
            listed(env, &data)
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// Engine.clearRecent(data): the recent list emptied, once the person says
/// so: the screen asks first, as the desktop's window does.
#[unsafe(no_mangle)]
pub extern "system" fn Java_net_pktito_app_Engine_clearRecent<'a>(
    mut unowned: EnvUnowned<'a>,
    _class: JClass<'a>,
    data: JString<'a>,
) {
    unowned
        .with_env(|env| -> jni::errors::Result<()> {
            let data = PathBuf::from(data.try_to_string(env)?);
            recent::clear(&data);
            Ok(())
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// Engine.relink(data, id, path, size, modified): a transcript's file
/// found again, at path, a content URI, its size and its last change in
/// seconds since 1970 (0 where its provider does not say), in place of
/// the ones it was kept with (recent::relink).
#[unsafe(no_mangle)]
pub extern "system" fn Java_net_pktito_app_Engine_relink<'a>(
    mut unowned: EnvUnowned<'a>,
    _class: JClass<'a>,
    data: JString<'a>,
    id: JString<'a>,
    path: JString<'a>,
    size: jlong,
    modified: jlong,
) -> JObject<'a> {
    unowned
        .with_env(|env| -> jni::errors::Result<JObject<'a>> {
            let data = PathBuf::from(data.try_to_string(env)?);
            let id = id.try_to_string(env)?;
            let path = PathBuf::from(path.try_to_string(env)?);
            if let Err(why) = recent::relink(
                &data,
                &id,
                &path,
                size.max(0) as u64,
                modified.max(0) as u64,
            ) {
                return throw(env, &why).map(|_| JObject::null());
            }
            listed(env, &data)
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// Engine.backend(): the engine's code for this processor, picked by its
/// CPU, by its name and what it uses ("neon-dp (ARMv8.2 NEON: SDOT int8,
/// fp32)"), for About's version line, as the desktop's status has it. No
/// model is loaded for it.
#[unsafe(no_mangle)]
pub extern "system" fn Java_net_pktito_app_Engine_backend<'a>(
    mut unowned: EnvUnowned<'a>,
    _class: JClass<'a>,
) -> JObject<'a> {
    unowned
        .with_env(|env| -> jni::errors::Result<JObject<'a>> {
            Ok(env.new_string(pktito::backend())?.into())
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// The phone as Kotlin describes it (Editions.device), in JSON: an
/// IOException's why for JSON that isn't one.
fn device(json: &str) -> Result<editions::Device, String> {
    serde_json::from_str(json).map_err(|err| format!("device: {err}"))
}

/// The catalog model name; why not, for one it doesn't list.
fn entry(name: &str) -> Result<&'static model::Entry, String> {
    model::entry(name).ok_or_else(|| format!("{name}: no such model"))
}

/// name's edition id, of the catalog's; why not, for one it doesn't have.
fn edition(e: &'static model::Entry, id: &str) -> Result<&'static editions::Edition, String> {
    e.editions
        .iter()
        .find(|ed| ed.id == id)
        .ok_or_else(|| format!("{}: no edition {id}", e.name))
}

/// Engine.catalog(dir, device): the models models.pktito.net serves, as
/// JSON, each as the core's view of it in dir, the app's models folder, for
/// device (editions::view; docs/EDITIONS.md): the edition chosen, what runs,
/// why the NPU refused the one that fits best, the offer. No model is
/// loaded for it.
#[unsafe(no_mangle)]
pub extern "system" fn Java_net_pktito_app_Engine_catalog<'a>(
    mut unowned: EnvUnowned<'a>,
    _class: JClass<'a>,
    dir: JString<'a>,
    device: JString<'a>,
) -> JObject<'a> {
    unowned
        .with_env(|env| -> jni::errors::Result<JObject<'a>> {
            let dir = PathBuf::from(dir.try_to_string(env)?);
            let d = match self::device(&device.try_to_string(env)?) {
                Ok(d) => d,
                Err(why) => return throw(env, &why).map(|_| JObject::null()),
            };
            let views: Vec<editions::View> = model::CATALOG
                .iter()
                .map(|e| editions::view(&dir, e, &d))
                .collect();
            Ok(env
                .new_string(serde_json::to_string(&views).unwrap_or_default())?
                .into())
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// Engine.adopt(dir): the files in dir from before 0.9.6 given their
/// records, swaps cut short finished (editions::adopt). Once a launch, off
/// the main thread: a whole .pkf takes about 2 s.
#[unsafe(no_mangle)]
pub extern "system" fn Java_net_pktito_app_Engine_adopt<'a>(
    mut unowned: EnvUnowned<'a>,
    _class: JClass<'a>,
    dir: JString<'a>,
) {
    unowned
        .with_env(|env| -> jni::errors::Result<()> {
            editions::adopt(&PathBuf::from(dir.try_to_string(env)?));
            Ok(())
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// Engine.pending(dir, name): whether name has a download that checked and
/// waits to be put in place (editions::pending).
#[unsafe(no_mangle)]
pub extern "system" fn Java_net_pktito_app_Engine_pending<'a>(
    mut unowned: EnvUnowned<'a>,
    _class: JClass<'a>,
    dir: JString<'a>,
    name: JString<'a>,
) -> jboolean {
    unowned
        .with_env(|env| -> jni::errors::Result<jboolean> {
            let dir = PathBuf::from(dir.try_to_string(env)?);
            Ok(model::entry(&name.try_to_string(env)?).is_some_and(|e| editions::pending(&dir, e)))
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// Engine.settle(dir, name): name's downloads that checked put in place
/// over the files they replace (editions::settle), or an IOException with
/// why. Only while no engine holds name's files: the transcriber lets go of
/// it first (Transcriber.model).
#[unsafe(no_mangle)]
pub extern "system" fn Java_net_pktito_app_Engine_settle<'a>(
    mut unowned: EnvUnowned<'a>,
    _class: JClass<'a>,
    dir: JString<'a>,
    name: JString<'a>,
) {
    unowned
        .with_env(|env| -> jni::errors::Result<()> {
            let dir = PathBuf::from(dir.try_to_string(env)?);
            let settled = entry(&name.try_to_string(env)?).and_then(|e| editions::settle(&dir, e));
            match settled {
                Ok(()) => Ok(()),
                Err(why) => throw(env, &why),
            }
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// Engine.refuse(dir, name, edition, device, why): the NPU refused name's
/// edition for why, here (editions::refuse): its graph goes either way, and
/// the refusal is recorded for device where the graph was the catalog's.
/// Whether it was, as the refusals now say: an Older graph refused says
/// nothing of the catalog's, which Manage models then offers.
#[unsafe(no_mangle)]
pub extern "system" fn Java_net_pktito_app_Engine_refuse<'a>(
    mut unowned: EnvUnowned<'a>,
    _class: JClass<'a>,
    dir: JString<'a>,
    name: JString<'a>,
    id: JString<'a>,
    device: JString<'a>,
    why: JString<'a>,
) -> jboolean {
    unowned
        .with_env(|env| -> jni::errors::Result<jboolean> {
            let dir = PathBuf::from(dir.try_to_string(env)?);
            let name = name.try_to_string(env)?;
            let id = id.try_to_string(env)?;
            let why = why.try_to_string(env)?;
            let device = device.try_to_string(env)?;
            let got = entry(&name).and_then(|e| Ok((e, edition(e, &id)?, self::device(&device)?)));
            let (e, ed, d) = match got {
                Ok(x) => x,
                Err(why) => return throw(env, &why).map(|_| false),
            };
            editions::refuse(&dir, e, ed, &d, &why);
            Ok(editions::Refusals::read(&dir).holds(e, ed, &d).is_some())
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// Engine.ran(dir, name, edition, model): a transcription finished on name's
/// edition with the model file at model; the whole .pkf deleted once an
/// NPU's edition has run from its own slim file (editions::ran). Whether it
/// was.
#[unsafe(no_mangle)]
pub extern "system" fn Java_net_pktito_app_Engine_ran<'a>(
    mut unowned: EnvUnowned<'a>,
    _class: JClass<'a>,
    dir: JString<'a>,
    name: JString<'a>,
    id: JString<'a>,
    model: JString<'a>,
) -> jboolean {
    unowned
        .with_env(|env| -> jni::errors::Result<jboolean> {
            let dir = PathBuf::from(dir.try_to_string(env)?);
            let name = name.try_to_string(env)?;
            let id = id.try_to_string(env)?;
            let model = PathBuf::from(model.try_to_string(env)?);
            Ok(entry(&name)
                .and_then(|e| Ok((e, edition(e, &id)?)))
                .is_ok_and(|(e, ed)| editions::ran(&dir, e, ed, &model)))
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// Engine.delete(dir, name): every file of name's editions in dir, their
/// records, parts and replacements waiting, and its refusals
/// (editions::remove_part, forget). The engine lets go of it first
/// (Models.delete).
#[unsafe(no_mangle)]
pub extern "system" fn Java_net_pktito_app_Engine_delete<'a>(
    mut unowned: EnvUnowned<'a>,
    _class: JClass<'a>,
    dir: JString<'a>,
    name: JString<'a>,
) {
    unowned
        .with_env(|env| -> jni::errors::Result<()> {
            let dir = PathBuf::from(dir.try_to_string(env)?);
            if let Some(e) = model::entry(&name.try_to_string(env)?) {
                for p in editions::parts(e) {
                    editions::remove_part(&dir, p);
                }
                editions::forget(&dir, e);
            }
            Ok(())
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// The files of the catalog model name that files, a JSON array of their
/// names, names (model::Wanted::named), in dir; why not, for one it does
/// not have.
fn wanted(dir: &Path, name: &str, files: &str) -> Result<Vec<model::Wanted>, String> {
    // the diarizer, whose one file is all there is of it
    if let Some(d) = model::DIARIZER.filter(|d| d.name == name) {
        return Ok(vec![model::Wanted::diarizer(dir, &d)]);
    }
    let e = model::entry(name).ok_or_else(|| format!("{name}: no such model"))?;
    let names: Vec<String> = serde_json::from_str(files).map_err(|err| err.to_string())?;
    let w = names
        .iter()
        .map(|f| model::Wanted::named(dir, e, f).ok_or_else(|| format!("{name} has no file {f}")))
        .collect::<Result<Vec<_>, _>>()?;
    if w.is_empty() {
        return Err(format!("{name}: no files to download"));
    }
    Ok(w)
}

/// Engine.download(dir, name, files, progress): files of the catalog model
/// name (a JSON array of their names: its edition for this phone, the
/// whole .pkf or the slim one and the NPU's graph, or a graph alone for a
/// whole .pkf here), downloaded into dir in turn, or their download gone
/// on with from what it kept, on this thread, as the window's runs on one
/// of its own (download.rs: each checked against its size and SHA-256,
/// tried again by itself while the network is away). progress.state(json)
/// hears each change of download::State, the files' bytes counted as one,
/// at most ten a second while bytes come, and last how it ended. The last
/// file's path, or "" where it stopped short: paused, cancelled or failed.
#[unsafe(no_mangle)]
pub extern "system" fn Java_net_pktito_app_Engine_download<'a>(
    mut unowned: EnvUnowned<'a>,
    _class: JClass<'a>,
    dir: JString<'a>,
    name: JString<'a>,
    files: JString<'a>,
    progress: JObject<'a>,
) -> JObject<'a> {
    unowned
        .with_env(|env| -> jni::errors::Result<JObject<'a>> {
            let dir = PathBuf::from(dir.try_to_string(env)?);
            let name = name.try_to_string(env)?;
            let files = match wanted(&dir, &name, &files.try_to_string(env)?) {
                Ok(w) => w,
                Err(why) => return throw(env, &why).map(|_| JObject::null()),
            };
            let job = Arc::new(download::Job::files(&files));
            *DOWNLOAD.lock().unwrap() = Some(job.clone());
            let failed = RefCell::new(None);
            let tell = |s: &download::State| {
                // after Kotlin threw, nothing more is said: its exception
                // waits to be thrown on, and JNI takes no call over it
                if failed.borrow().is_some() {
                    return;
                }
                let json = serde_json::to_string(s).unwrap_or_default();
                let call = env.with_local_frame(2, |env| -> jni::errors::Result<()> {
                    let json = env.new_string(json)?;
                    env.call_method(
                        &progress,
                        jni_str!("state"),
                        jni_sig!("(Ljava/lang/String;)V"),
                        &[JValue::from(&json)],
                    )?;
                    Ok(())
                });
                if let Err(err) = call {
                    // the download stops where it is, keeping what it has
                    failed.replace(Some(err));
                    job.ctl.pause.store(true, Ordering::Relaxed);
                }
            };
            let got = job.run_files(&files, &tell);
            tell(&job.state());
            {
                let mut slot = DOWNLOAD.lock().unwrap();
                if slot.as_ref().is_some_and(|j| Arc::ptr_eq(j, &job)) {
                    *slot = None;
                }
            }
            if let Some(err) = failed.into_inner() {
                return Err(err);
            }
            let path = got
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default();
            Ok(env.new_string(path)?.into())
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// Engine.pauseDownload(): the download running stops before its next read,
/// keeping what it has; download() goes on from there.
#[unsafe(no_mangle)]
pub extern "system" fn Java_net_pktito_app_Engine_pauseDownload<'a>(
    _unowned: EnvUnowned<'a>,
    _class: JClass<'a>,
) {
    if let Some(j) = DOWNLOAD.lock().unwrap().as_ref() {
        j.ctl.pause.store(true, Ordering::Relaxed);
    }
}

/// Engine.retryDownload(): the download running, waiting to try again after
/// the network failed it, tries now.
#[unsafe(no_mangle)]
pub extern "system" fn Java_net_pktito_app_Engine_retryDownload<'a>(
    _unowned: EnvUnowned<'a>,
    _class: JClass<'a>,
) {
    if let Some(j) = DOWNLOAD.lock().unwrap().as_ref() {
        j.resume_now();
    }
}

/// Engine.cancelDownload(dir, name, files): the download of files of name
/// (as download() takes them) stops and what it had is deleted, by its own
/// thread, which says so, if it is running (true); else here, from dir,
/// the app's models folder, as its thread would have (false): each file's
/// part, and each file, done or one an update left (download::Job::run_files).
#[unsafe(no_mangle)]
pub extern "system" fn Java_net_pktito_app_Engine_cancelDownload<'a>(
    mut unowned: EnvUnowned<'a>,
    _class: JClass<'a>,
    dir: JString<'a>,
    name: JString<'a>,
    files: JString<'a>,
) -> jboolean {
    unowned
        .with_env(|env| -> jni::errors::Result<jboolean> {
            let dir = PathBuf::from(dir.try_to_string(env)?);
            let name = name.try_to_string(env)?;
            let files = files.try_to_string(env)?;
            if let Some(j) = DOWNLOAD.lock().unwrap().as_ref() {
                j.ctl.cancel.store(true, Ordering::Relaxed);
                return Ok(true);
            }
            for w in wanted(&dir, &name, &files).unwrap_or_default() {
                let _ = fs::remove_file(&w.part);
                let _ = fs::remove_file(&w.dst);
            }
            Ok(false)
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// Engine.trouble(why): what a model file that load() refused for why
/// means to its user, as the catalog's key ("model_file.not_model"), for
/// the app's words, as the window words it (model::trouble).
#[unsafe(no_mangle)]
pub extern "system" fn Java_net_pktito_app_Engine_trouble<'a>(
    mut unowned: EnvUnowned<'a>,
    _class: JClass<'a>,
    why: JString<'a>,
) -> JObject<'a> {
    unowned
        .with_env(|env| -> jni::errors::Result<JObject<'a>> {
            let why = why.try_to_string(env)?;
            Ok(env.new_string(model::trouble(&why))?.into())
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// A File of its own on fd's file, which a view index keeps while fd
/// closes as Kotlin's.
fn own(fd: jint) -> Option<fs::File> {
    use std::os::fd::BorrowedFd;
    if fd < 0 {
        return None;
    }
    let f = unsafe { BorrowedFd::borrow_raw(fd) }
        .try_clone_to_owned()
        .ok()?;
    Some(fs::File::from(f))
}

/// Engine.viewIndex(fd): the frames or pages of the recording open at fd,
/// for views of it from a time on, which Android's player starts afresh
/// at (view.rs): a handle for view(), 0 where it is a recording of another
/// kind, which the player is handed as it is. Read once a recording, on a
/// thread of the screen's choosing: an MP3's frames, 192,000 in 74
/// minutes, are each read. viewIndexFree lets it go.
#[unsafe(no_mangle)]
pub extern "system" fn Java_net_pktito_app_Engine_viewIndex<'a>(
    mut unowned: EnvUnowned<'a>,
    _class: JClass<'a>,
    fd: jint,
) -> jlong {
    unowned
        .with_env(|_env| -> jni::errors::Result<jlong> {
            Ok(own(fd)
                .and_then(view::Index::read)
                .map_or(0, |i| Box::into_raw(Box::new(i)) as jlong))
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_net_pktito_app_Engine_viewIndexFree<'a>(
    mut unowned: EnvUnowned<'a>,
    _class: JClass<'a>,
    index: jlong,
) {
    unowned
        .with_env(|_env| -> jni::errors::Result<()> {
            if index != 0 {
                drop(unsafe { Box::from_raw(index as *mut view::Index) });
            }
            Ok(())
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// Engine.view(index, t): the view of the recording from t seconds in, as
/// a handle for viewStart, viewSize and viewRead, a player's data source;
/// 0 where it cannot be cut. viewFree lets it go.
#[unsafe(no_mangle)]
pub extern "system" fn Java_net_pktito_app_Engine_view<'a>(
    mut unowned: EnvUnowned<'a>,
    _class: JClass<'a>,
    index: jlong,
    t: jdouble,
) -> jlong {
    unowned
        .with_env(|_env| -> jni::errors::Result<jlong> {
            let i = unsafe { &*(index as *const view::Index) };
            Ok(i.view(t).map_or(0, |v| Box::into_raw(Box::new(v)) as jlong))
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// Engine.viewStart(view): where the view's first sample is in the
/// recording, in seconds, which the player's time in it is counted from.
#[unsafe(no_mangle)]
pub extern "system" fn Java_net_pktito_app_Engine_viewStart<'a>(
    mut unowned: EnvUnowned<'a>,
    _class: JClass<'a>,
    view: jlong,
) -> jdouble {
    unowned
        .with_env(|_env| -> jni::errors::Result<jdouble> {
            Ok(unsafe { &*(view as *const view::View) }.start)
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// Engine.viewSize(view): its bytes.
#[unsafe(no_mangle)]
pub extern "system" fn Java_net_pktito_app_Engine_viewSize<'a>(
    mut unowned: EnvUnowned<'a>,
    _class: JClass<'a>,
    view: jlong,
) -> jlong {
    unowned
        .with_env(|_env| -> jni::errors::Result<jlong> {
            Ok(unsafe { &*(view as *const view::View) }.size() as jlong)
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// Engine.viewRead(view, pos, buf, off, n): up to n of the view's bytes
/// from pos, into buf from off, as MediaDataSource.readAt asks them, on
/// the player's thread: how many, -1 at the view's end, or on a read the
/// file refused.
#[unsafe(no_mangle)]
pub extern "system" fn Java_net_pktito_app_Engine_viewRead<'a>(
    mut unowned: EnvUnowned<'a>,
    _class: JClass<'a>,
    view: jlong,
    pos: jlong,
    buf: JByteArray<'a>,
    off: jint,
    n: jint,
) -> jint {
    unowned
        .with_env(|env| -> jni::errors::Result<jint> {
            let v = unsafe { &*(view as *const view::View) };
            if pos < 0 || off < 0 || n <= 0 {
                return Ok(if n == 0 { 0 } else { -1 });
            }
            let mut b = vec![0u8; n.min(1 << 20) as usize];
            let got = match v.read_at(pos as u64, &mut b) {
                Ok(0) | Err(_) => return Ok(-1),
                Ok(got) => got,
            };
            let bytes: Vec<i8> = b[..got].iter().map(|&x| x as i8).collect();
            buf.set_region(env, off, &bytes)?;
            Ok(got as jint)
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_net_pktito_app_Engine_viewFree<'a>(
    mut unowned: EnvUnowned<'a>,
    _class: JClass<'a>,
    view: jlong,
) {
    unowned
        .with_env(|_env| -> jni::errors::Result<()> {
            if view != 0 {
                drop(unsafe { Box::from_raw(view as *mut view::View) });
            }
            Ok(())
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}
