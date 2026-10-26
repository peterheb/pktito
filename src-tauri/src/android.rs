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

use crate::{audio, decode, download, format, model, pktito, recent, transcribe};
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
}

/// A transcript as Kotlin gets it, in JSON, and gives back to format() and
/// captions(): with the name of the model that made it, whose language its
/// captions' rules follow (format::cues), as the desktop's window names a
/// file's own model to its formats; the model loaded since may be another.
#[derive(Serialize, Deserialize)]
struct Transcript {
    duration: f64,
    /// false where stop() ended it before the end
    complete: bool,
    model: String,
    segments: Vec<transcribe::Segment>,
}

/// stop()'s word to the transcription running, read before each chunk.
static STOP: AtomicBool = AtomicBool::new(false);

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
                Ok(Loaded { asr, model })
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
                "{}, {}, {} threads",
                l.model.name(),
                pktito::backend(),
                l.asr.threads()
            );
            Ok(env.new_string(about)?.into())
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// Engine.transcribe(handle, fd, name, wav, progress): the recording open
/// at fd (name: what to call it in an error), read and transcribed, as a
/// Transcript's JSON. Once it is read, the samples pktito hears go to wav as
/// a 16-bit WAV, for the screen to play in step with the transcript's
/// times whatever the original's format, as the desktop's window plays
/// them (audio.rs; "" for none), and progress.audio(seconds, a video's
/// frame rate or 0, the loudness in audio::PEAKS slices) draws the strip; progress.update(fraction, the
/// segments just done, in JSON) after each chunk. All on this thread.
#[unsafe(no_mangle)]
pub extern "system" fn Java_net_pktito_app_Engine_transcribe<'a>(
    mut unowned: EnvUnowned<'a>,
    _class: JClass<'a>,
    handle: jlong,
    fd: jint,
    name: JString<'a>,
    wav: JString<'a>,
    progress: JObject<'a>,
) -> JObject<'a> {
    unowned
        .with_env(|env| -> jni::errors::Result<JObject<'a>> {
            let l = unsafe { &mut *(handle as *mut Loaded) };
            let name = name.try_to_string(env)?;
            let wav = wav.try_to_string(env)?;
            STOP.store(false, Ordering::Relaxed);
            let fps = decode::frame_rate_fd(fd).unwrap_or(0.0);
            let x = match decode::decode_fd(fd, &name) {
                Ok(x) => x,
                Err(e) => return throw(env, &e.message()).map(|_| JObject::null()),
            };
            let duration = x.len() as f64 / decode::RATE as f64;
            // a WAV that cannot be written leaves the transcript without
            // playback, as on the desktop
            let wav = Path::new(&wav);
            if let (Some(dir), Some(id)) = (wav.parent(), wav.file_stem().and_then(|s| s.to_str()))
            {
                let _ = audio::save(dir, id, &x, decode::RATE as u32);
            }
            let peaks = audio::peaks(&x, audio::PEAKS);
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
            let mut failed = None;
            let run = transcribe::run(
                &l.model,
                &mut l.asr,
                &x,
                |done, segs| {
                    let segs = serde_json::to_string(segs).unwrap_or_default();
                    // a frame of its own, so a long recording's calls leave
                    // no local references behind
                    let call = env.with_local_frame(2, |env| -> jni::errors::Result<()> {
                        let segs = env.new_string(segs)?;
                        env.call_method(
                            &progress,
                            jni_str!("update"),
                            jni_sig!("(DLjava/lang/String;)V"),
                            &[JValue::Double(done), JValue::from(&segs)],
                        )?;
                        Ok(())
                    });
                    if let Err(e) = call {
                        failed.get_or_insert(e);
                        STOP.store(true, Ordering::Relaxed);
                    }
                },
                || STOP.load(Ordering::Relaxed),
            );
            if let Some(e) = failed {
                return Err(e);
            }
            let (segments, complete) = match run {
                Ok(r) => r,
                Err(why) => return throw(env, &why).map(|_| JObject::null()),
            };
            let t = Transcript {
                duration,
                complete,
                model: l.model.name(),
                segments,
            };
            let json = serde_json::to_string(&t).unwrap_or_default();
            Ok(env.new_string(json)?.into())
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// Engine.stop(): the transcription running ends before its next chunk.
#[unsafe(no_mangle)]
pub extern "system" fn Java_net_pktito_app_Engine_stop<'a>(
    _unowned: EnvUnowned<'a>,
    _class: JClass<'a>,
) {
    STOP.store(true, Ordering::Relaxed);
}

/// Engine.format(transcript, ext, name, times): a Transcript's JSON as a
/// file of the kind ext names (txt, md, srt, vtt, json), the desktop's;
/// times puts each sentence's time before it in txt and md.
#[unsafe(no_mangle)]
pub extern "system" fn Java_net_pktito_app_Engine_format<'a>(
    mut unowned: EnvUnowned<'a>,
    _class: JClass<'a>,
    transcript: JString<'a>,
    ext: JString<'a>,
    name: JString<'a>,
    times: jboolean,
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
            let text = format::text(
                &format::Transcript {
                    name: &name,
                    duration: t.duration,
                    model: Some(&t.model),
                    segments: &t.segments,
                },
                &ext,
                times,
            );
            match text {
                Ok(text) => Ok(env.new_string(text)?.into()),
                Err(why) => throw(env, &why).map(|_| JObject::null()),
            }
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// Engine.captions(transcript): a Transcript's subtitle track, the cues its
/// .srt and .vtt have (format::cues), as JSON: each cue's start and end in
/// milliseconds, its text, a line break where its second line begins, and
/// its words, n of the transcript's from the first. The Captions view shows
/// them.
#[unsafe(no_mangle)]
pub extern "system" fn Java_net_pktito_app_Engine_captions<'a>(
    mut unowned: EnvUnowned<'a>,
    _class: JClass<'a>,
    transcript: JString<'a>,
) -> JObject<'a> {
    unowned
        .with_env(|env| -> jni::errors::Result<JObject<'a>> {
            let json = transcript.try_to_string(env)?;
            let t: Transcript = match serde_json::from_str(&json) {
                Ok(t) => t,
                Err(e) => return throw(env, &e.to_string()).map(|_| JObject::null()),
            };
            let cues = format::cues(&format::Transcript {
                name: "",
                duration: t.duration,
                model: Some(&t.model),
                segments: &t.segments,
            });
            match cues {
                Ok(c) => Ok(env
                    .new_string(serde_json::to_string(&c).unwrap_or_default())?
                    .into()),
                Err(why) => throw(env, &why).map(|_| JObject::null()),
            }
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// The recent list's ids, newest first, which the audio cache keeps.
fn kept(data: &Path) -> Vec<String> {
    recent::list(data).into_iter().map(|i| i.id).collect()
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
/// and a video's frame rate. data is the app's no_backup folder, which
/// Android leaves out of backups and transfers to a new phone, as it does
/// the models there.
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

/// Engine.keep(data, audio, item, transcript, peaks): a finished transcript
/// at the top of the recent list, in place of an earlier one of the same
/// file, as the desktop's window keeps each (main.rs, job): item is
/// recent.rs's Item, as JSON, transcript what transcribe() gave, and peaks
/// its loudness. The audio of the transcripts let go goes from audio, the
/// cache's folder, and the rest is held to what the cache keeps
/// (audio::cap). The list after, as recent() gives it.
#[unsafe(no_mangle)]
pub extern "system" fn Java_net_pktito_app_Engine_keep<'a>(
    mut unowned: EnvUnowned<'a>,
    _class: JClass<'a>,
    data: JString<'a>,
    audio: JString<'a>,
    item: JString<'a>,
    transcript: JString<'a>,
    peaks: JByteArray<'a>,
) -> JObject<'a> {
    unowned
        .with_env(|env| -> jni::errors::Result<JObject<'a>> {
            let data = PathBuf::from(data.try_to_string(env)?);
            let dir = PathBuf::from(audio.try_to_string(env)?);
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
            };
            match recent::add(&data, &t) {
                Ok(gone) => {
                    for g in &gone {
                        let _ = fs::remove_file(audio::wav_in(&dir, g));
                    }
                    audio::cap(&dir, &kept(&data));
                }
                Err(why) => return throw(env, &why).map(|_| JObject::null()),
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

/// Engine.forget(data, audio, id): one transcript off the recent list, with
/// its audio in the cache; the file it was made from is not touched. The
/// list after.
#[unsafe(no_mangle)]
pub extern "system" fn Java_net_pktito_app_Engine_forget<'a>(
    mut unowned: EnvUnowned<'a>,
    _class: JClass<'a>,
    data: JString<'a>,
    audio: JString<'a>,
    id: JString<'a>,
) -> JObject<'a> {
    unowned
        .with_env(|env| -> jni::errors::Result<JObject<'a>> {
            let data = PathBuf::from(data.try_to_string(env)?);
            let dir = PathBuf::from(audio.try_to_string(env)?);
            let id = id.try_to_string(env)?;
            if let Err(why) = recent::remove(&data, &id) {
                return throw(env, &why).map(|_| JObject::null());
            }
            let _ = fs::remove_file(audio::wav_in(&dir, &id));
            listed(env, &data)
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// Engine.clearRecent(data, audio): the recent list emptied, its audio with
/// it, once the person says so: the screen asks first, as the desktop's
/// window does.
#[unsafe(no_mangle)]
pub extern "system" fn Java_net_pktito_app_Engine_clearRecent<'a>(
    mut unowned: EnvUnowned<'a>,
    _class: JClass<'a>,
    data: JString<'a>,
    audio: JString<'a>,
) {
    unowned
        .with_env(|env| -> jni::errors::Result<()> {
            let data = PathBuf::from(data.try_to_string(env)?);
            let dir = PathBuf::from(audio.try_to_string(env)?);
            for id in kept(&data) {
                let _ = fs::remove_file(audio::wav_in(&dir, &id));
            }
            recent::clear(&data);
            Ok(())
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// Engine.prune(data, audio): once a process, before anything is
/// transcribed, the audio in the cache that no transcript on the list needs
/// let go, and the rest held to what the cache keeps, as the desktop's
/// window does before it opens (audio::prune).
#[unsafe(no_mangle)]
pub extern "system" fn Java_net_pktito_app_Engine_prune<'a>(
    mut unowned: EnvUnowned<'a>,
    _class: JClass<'a>,
    data: JString<'a>,
    audio: JString<'a>,
) {
    unowned
        .with_env(|env| -> jni::errors::Result<()> {
            let data = PathBuf::from(data.try_to_string(env)?);
            let dir = PathBuf::from(audio.try_to_string(env)?);
            audio::prune(&dir, &kept(&data));
            Ok(())
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// Engine.reread(fd, name, wav, duration): a recent transcript's audio read
/// again from its original, open at fd, once the cache has let it go (the
/// desktop's reload_audio), and written to wav as transcribe() writes it,
/// if it is the recording transcribed: true. false, and nothing written,
/// where its length is not the transcript's, within 0.1 s, two of an
/// encoder's frames; a file edited since is longer or shorter by more.
#[unsafe(no_mangle)]
pub extern "system" fn Java_net_pktito_app_Engine_reread<'a>(
    mut unowned: EnvUnowned<'a>,
    _class: JClass<'a>,
    fd: jint,
    name: JString<'a>,
    wav: JString<'a>,
    duration: jdouble,
) -> jboolean {
    unowned
        .with_env(|env| -> jni::errors::Result<jboolean> {
            let name = name.try_to_string(env)?;
            let wav = PathBuf::from(wav.try_to_string(env)?);
            let x = match decode::decode_fd(fd, &name) {
                Ok(x) => x,
                Err(e) => return throw(env, &e.message()).map(|_| false),
            };
            if (x.len() as f64 / decode::RATE as f64 - duration).abs() > 0.1 {
                return Ok(false);
            }
            let (Some(dir), Some(id)) = (wav.parent(), wav.file_stem().and_then(|s| s.to_str()))
            else {
                return throw(env, &format!("{}: not a WAV's path", wav.display())).map(|_| false);
            };
            match audio::save(dir, id, &x, decode::RATE as u32) {
                Ok(_) => Ok(true),
                Err(e) => throw(env, &e.to_string()).map(|_| false),
            }
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

/// What catalog() says of each model, as main.rs's Offer: its entry,
/// whether it is downloaded, the bytes a download that stopped has kept,
/// and its file's path, which the app loads it from.
#[derive(Serialize)]
struct Offer {
    #[serde(flatten)]
    entry: &'static model::Entry,
    have: bool,
    partial: u64,
    file: String,
}

/// Engine.catalog(dir): the models models.pktito.net serves, as JSON, each
/// as it stands in dir, the app's models folder. No model is loaded for it.
#[unsafe(no_mangle)]
pub extern "system" fn Java_net_pktito_app_Engine_catalog<'a>(
    mut unowned: EnvUnowned<'a>,
    _class: JClass<'a>,
    dir: JString<'a>,
) -> JObject<'a> {
    unowned
        .with_env(|env| -> jni::errors::Result<JObject<'a>> {
            let dir = PathBuf::from(dir.try_to_string(env)?);
            let offers: Vec<Offer> = model::CATALOG
                .iter()
                .map(|e| Offer {
                    entry: e,
                    have: model::have(&dir, e),
                    partial: model::partial(&dir, e),
                    file: model::file_in(&dir, e).to_string_lossy().into_owned(),
                })
                .collect();
            Ok(env
                .new_string(serde_json::to_string(&offers).unwrap_or_default())?
                .into())
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// Engine.download(dir, name, progress): the catalog model name downloaded
/// into dir, or its download gone on with from what it kept, on this
/// thread, as the window's runs on one of its own (download.rs: checked
/// against its size and SHA-256, tried again by itself while the network
/// is away). progress.state(json) hears each change of download::State, at
/// most ten a second while bytes come, and last how it ended. The checked
/// file's path, or "" where it stopped short: paused, cancelled or failed.
#[unsafe(no_mangle)]
pub extern "system" fn Java_net_pktito_app_Engine_download<'a>(
    mut unowned: EnvUnowned<'a>,
    _class: JClass<'a>,
    dir: JString<'a>,
    name: JString<'a>,
    progress: JObject<'a>,
) -> JObject<'a> {
    unowned
        .with_env(|env| -> jni::errors::Result<JObject<'a>> {
            let dir = PathBuf::from(dir.try_to_string(env)?);
            let name = name.try_to_string(env)?;
            let Some(e) = model::entry(&name) else {
                return throw(env, &format!("{name}: no such model")).map(|_| JObject::null());
            };
            let job = Arc::new(download::Job::new(e, &dir));
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
            let got = job.run(e, &dir, &tell);
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

/// Engine.cancelDownload(dir, name): the download of name stops and what it
/// had is deleted, by its own thread, which says so, if it is running
/// (true); else here, from dir, the app's models folder (false).
#[unsafe(no_mangle)]
pub extern "system" fn Java_net_pktito_app_Engine_cancelDownload<'a>(
    mut unowned: EnvUnowned<'a>,
    _class: JClass<'a>,
    dir: JString<'a>,
    name: JString<'a>,
) -> jboolean {
    unowned
        .with_env(|env| -> jni::errors::Result<jboolean> {
            let dir = PathBuf::from(dir.try_to_string(env)?);
            let name = name.try_to_string(env)?;
            if let Some(j) = DOWNLOAD.lock().unwrap().as_ref() {
                j.ctl.cancel.store(true, Ordering::Relaxed);
                return Ok(true);
            }
            if let Some(e) = model::entry(&name) {
                let _ = fs::remove_file(model::part_in(&dir, e));
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
