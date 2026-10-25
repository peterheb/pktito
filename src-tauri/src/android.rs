//! The Android app's way in (android/, in Kotlin): the model, a recording's
//! samples, transcribing and the formats, behind the native methods of
//! net.pktito.app.Engine. Kotlin owns the screen, the files and the service
//! that keeps a transcription running; this owns what the desktop's Rust
//! owns, the same code, so a recording gives the same transcript here as on
//! every other platform.
//!
//! Each method takes the JNI environment Android hands it and upgrades it
//! (jni's with_env) before anything else; what the app gets wrong comes
//! back as an IOException with the reason, and a panic as a
//! RuntimeException, never across into Java.

use crate::{decode, format, pktito, transcribe};
use jni::errors::ThrowRuntimeExAndDefault;
use jni::objects::{JClass, JObject, JString, JValue};
use jni::strings::JNIString;
use jni::sys::{jint, jlong};
use jni::{jni_sig, jni_str, Env, EnvUnowned};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

/// What Engine.load's Long stands for: a model and its transcriber, one
/// transcription at a time (Kotlin's service queues them).
struct Loaded {
    // declared first, so dropped first: it reads the model
    asr: pktito::Asr,
    model: pktito::Model,
}

/// A transcript as Kotlin gets it, in JSON, and gives back to format().
#[derive(Serialize, Deserialize)]
struct Transcript {
    duration: f64,
    /// false where stop() ended it before the end
    complete: bool,
    segments: Vec<transcribe::Segment>,
}

/// stop()'s word to the transcription running, read before each chunk.
static STOP: AtomicBool = AtomicBool::new(false);

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
/// Transcript's JSON. progress.update(fraction, the segments just done, in
/// JSON) after each chunk, on this thread.
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
            let x = match decode::decode_fd(fd, &name) {
                Ok(x) => x,
                Err(e) => return throw(env, &e.message()).map(|_| JObject::null()),
            };
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
                duration: x.len() as f64 / decode::RATE as f64,
                complete,
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

/// Engine.format(handle, transcript, ext, name): a Transcript's JSON as a
/// file of the kind ext names (txt, md, srt, vtt, json), the desktop's.
#[unsafe(no_mangle)]
pub extern "system" fn Java_net_pktito_app_Engine_format<'a>(
    mut unowned: EnvUnowned<'a>,
    _class: JClass<'a>,
    handle: jlong,
    transcript: JString<'a>,
    ext: JString<'a>,
    name: JString<'a>,
) -> JObject<'a> {
    unowned
        .with_env(|env| -> jni::errors::Result<JObject<'a>> {
            let l = unsafe { &*(handle as *const Loaded) };
            let json = transcript.try_to_string(env)?;
            let ext = ext.try_to_string(env)?;
            let name = name.try_to_string(env)?;
            let t: Transcript = match serde_json::from_str(&json) {
                Ok(t) => t,
                Err(e) => return throw(env, &e.to_string()).map(|_| JObject::null()),
            };
            let model = l.model.name();
            let text = format::text(
                &format::Transcript {
                    name: &name,
                    duration: t.duration,
                    model: Some(&model),
                    segments: &t.segments,
                },
                &ext,
                false,
            );
            match text {
                Ok(text) => Ok(env.new_string(text)?.into()),
                Err(why) => throw(env, &why).map(|_| JObject::null()),
            }
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}
