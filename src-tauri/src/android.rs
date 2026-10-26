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

use crate::{audio, decode, download, format, model, pktito, recent, transcribe, view};
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

/// Engine.transcribe(handle, fd, name, progress): the recording open at fd
/// (name: what to call it in an error), read and transcribed, as a
/// Transcript's JSON. Once it is read, progress.audio(seconds, a video's
/// frame rate or 0, the loudness in audio::PEAKS slices) draws the strip;
/// progress.update(fraction, the segments just done, in JSON) after each
/// chunk. All on this thread. The screen plays the recording itself (the
/// app's Player.kt, view.rs), so nothing of its samples is kept.
#[unsafe(no_mangle)]
pub extern "system" fn Java_net_pktito_app_Engine_transcribe<'a>(
    mut unowned: EnvUnowned<'a>,
    _class: JClass<'a>,
    handle: jlong,
    fd: jint,
    name: JString<'a>,
    progress: JObject<'a>,
) -> JObject<'a> {
    unowned
        .with_env(|env| -> jni::errors::Result<JObject<'a>> {
            let l = unsafe { &mut *(handle as *mut Loaded) };
            let name = name.try_to_string(env)?;
            STOP.store(false, Ordering::Relaxed);
            let fps = decode::frame_rate_fd(fd).unwrap_or(0.0);
            let x = match decode::decode_fd(fd, &name) {
                Ok(x) => x,
                Err(e) => return throw(env, &e.message()).map(|_| JObject::null()),
            };
            let duration = x.len() as f64 / decode::RATE as f64;
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
