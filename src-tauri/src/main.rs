//! pktito's desktop app: drop audio or video files, read their transcripts.
//! The audio comes out through the OS's media libraries (or ffmpeg, see
//! decode.rs) and is transcribed on this computer by pktito; each finished
//! transcript is kept on the recent list, and its audio in the cache to
//! play it back against.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use pktito_app::strings::{t, Problem};
use pktito_app::{
    audio, awake, decode, download, format, model, pktito, recent, strings, transcribe,
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_dialog::{DialogExt, FileDialogBuilder};

/// The model and a transcriber over it, loaded once and kept.
struct Engine {
    model: pktito::Model,
    asr: pktito::Asr,
    path: PathBuf,
}

#[derive(Default)]
struct AppState {
    engine: Mutex<Option<Engine>>,
    /// The model download under way, paused or failed: one at a time.
    download: Mutex<Option<Arc<download::Job>>>,
    /// The transcription asked to stop, by id; 0 for none.
    stop: AtomicU32,
    /// Files the system asked the app to open (a .pkf double-clicked) before
    /// the page was there to take them (hand_over), and whether it is now;
    /// the lock is held for both, so that none falls between.
    opened: Mutex<Vec<String>>,
    page_ready: AtomicBool,
    /// On Linux, the server the player's audio comes from, from the first
    /// file played (serve.rs).
    #[cfg(target_os = "linux")]
    serve: Mutex<Option<pktito_app::serve::Server>>,
}

const VIDEO: &[&str] = &["mp4", "m4v", "mov", "webm", "mkv", "wmv", "avi", "3gp"];

/// A video's frame rate, for times in frames; 0 for audio, and for a video
/// whose rate neither the OS nor ffmpeg can say.
fn fps_of(path: &Path) -> f64 {
    let video = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| VIDEO.contains(&e.to_ascii_lowercase().as_str()));
    if video {
        decode::frame_rate(path).unwrap_or(0.0)
    } else {
        0.0
    }
}

fn data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path().app_data_dir().map_err(|e| e.to_string())
}

/// The Microsoft Store edition: the same exes as the setup program's,
/// wrapped in an MSIX package, so they ask Windows whether they run from
/// one. In a package, AppData is redirected into the package's own storage,
/// which Explorer outside it does not show and uninstalling deletes: the
/// page hides the model's Show in File Explorer and says so in Privacy.
/// Asked with no buffer, the call answers ERROR_INSUFFICIENT_BUFFER in a
/// package and APPMODEL_ERROR_NO_PACKAGE outside one (appmodel.h).
#[cfg(target_os = "windows")]
fn store_edition() -> bool {
    #[link(name = "kernel32")]
    extern "system" {
        fn GetCurrentPackageFullName(len: *mut u32, name: *mut u16) -> i32;
    }
    const APPMODEL_ERROR_NO_PACKAGE: i32 = 15700;
    let mut len = 0u32;
    unsafe {
        GetCurrentPackageFullName(&mut len, std::ptr::null_mut()) != APPMODEL_ERROR_NO_PACKAGE
    }
}

#[cfg(not(target_os = "windows"))]
fn store_edition() -> bool {
    false
}

/// Where the recent list's audio is kept: the webview may read this folder
/// and nothing else (tauri.conf.json, assetProtocol).
fn audio_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_cache_dir()
        .map(|d| d.join("audio"))
        .map_err(|e| e.to_string())
}

#[derive(Serialize)]
struct Offer {
    #[serde(flatten)]
    entry: &'static model::Entry,
    /// Downloaded and on this computer.
    have: bool,
    /// Bytes kept from a download that stopped.
    partial: u64,
}

#[derive(Serialize)]
struct Status {
    /// The catalog model in use, by name.
    model: Option<String>,
    /// A model file of the user's own, used instead when set.
    model_file: Option<PathBuf>,
    /// There is a model on this computer to transcribe with.
    ready: bool,
    catalog: Vec<Offer>,
    /// A download under way, paused or failed.
    download: Option<download::State>,
    prefs: model::Prefs,
    /// The app's, from Cargo.toml, for About.
    version: &'static str,
    backend: String,
    platform: &'static str,
    /// Running from the Microsoft Store's package (store_edition).
    store: bool,
    media: &'static [&'static str],
    video: &'static [&'static str],
}

/// A file dialog owned by the window on Windows, so that it is modal and in
/// front of it: one without an owner opened behind the window in the
/// Windows test (2026-10-21), where it could sit unnoticed. macOS runs its
/// panels modal for the app already.
fn file_dialog(app: &AppHandle) -> FileDialogBuilder<tauri::Wry> {
    let d = app.dialog().file();
    #[cfg(target_os = "windows")]
    if let Some(w) = app.get_webview_window("main") {
        return d.set_parent(&w);
    }
    d
}

/// Load the model at path into the engine, unless it is the one there, on
/// PKTITO_THREADS's threads (transcribe::threads_wanted).
fn ensure_engine<'a>(slot: &'a mut Option<Engine>, path: &Path) -> Result<&'a mut Engine, String> {
    if slot.as_ref().map(|e| e.path != path).unwrap_or(true) {
        *slot = None;
        let model = pktito::Model::load(path)?;
        let asr = pktito::Asr::new(&model, transcribe::threads_wanted())?;
        *slot = Some(Engine {
            model,
            asr,
            path: path.to_path_buf(),
        });
    }
    Ok(slot.as_mut().unwrap())
}

#[tauri::command]
fn status(app: AppHandle, state: State<'_, AppState>) -> Result<Status, Problem> {
    let data = data_dir(&app)?;
    let mut s = model::load_settings(&data);
    if s.model_file.as_ref().map(|p| !p.is_file()).unwrap_or(false) {
        // moved or deleted since: the catalog model is in use again, if there is one
        s.model_file = None;
        model::save_settings(&data, &s)?;
    }
    let dir = data.join("models");
    let download = state
        .download
        .lock()
        .unwrap()
        .as_ref()
        .map(|j| j.state())
        .filter(|d| d.step != "done" && d.step != "cancelled");
    Ok(Status {
        ready: model::usable(&data, &s).is_some(),
        model: s.model,
        model_file: s.model_file,
        catalog: model::CATALOG
            .iter()
            .map(|e| Offer {
                entry: e,
                have: model::have(&dir, e),
                partial: model::partial(&dir, e),
            })
            .collect(),
        download,
        prefs: s.prefs,
        version: env!("CARGO_PKG_VERSION"),
        backend: pktito::backend(),
        platform: std::env::consts::OS,
        store: store_edition(),
        media: decode::MEDIA,
        video: VIDEO,
    })
}

#[tauri::command]
fn set_prefs(app: AppHandle, prefs: model::Prefs) -> Result<(), Problem> {
    let data = data_dir(&app)?;
    let mut s = model::load_settings(&data);
    s.prefs = prefs;
    Ok(model::save_settings(&data, &s)?)
}

/// Use a catalog model that is on this computer.
#[tauri::command]
fn choose_model(app: AppHandle, name: String) -> Result<(), Problem> {
    let e = model::entry(&name).ok_or("no such model")?;
    let data = data_dir(&app)?;
    if !model::have(&data.join("models"), e) {
        return Err(Problem::new("choose.not_downloaded"));
    }
    let mut s = model::load_settings(&data);
    s.model = Some(name);
    s.model_file = None;
    Ok(model::save_settings(&data, &s)?)
}

/// Download a catalog model, or go on with its download; once it checks it
/// is the model in use. "download" events say how it goes.
#[tauri::command]
fn download_model(app: AppHandle, state: State<'_, AppState>, name: String) -> Result<(), Problem> {
    let e = model::entry(&name).ok_or("no such model")?;
    let dir = data_dir(&app)?.join("models");
    let mut slot = state.download.lock().unwrap();
    if let Some(j) = slot.as_ref() {
        if j.alive() && !j.ctl.cancel.load(Ordering::Relaxed) {
            if j.name() == name {
                j.resume_now();
                return Ok(());
            }
            return Err(Problem::new("download.busy").with("name", j.name()));
        }
        // one cancelled and on its way out: let it delete what it had first
        let t = Instant::now();
        while j.alive() && t.elapsed() < Duration::from_secs(3) {
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    let job = Arc::new(download::Job::new(e, &dir));
    *slot = Some(job.clone());
    let (tell, done) = (app.clone(), app.clone());
    job.start(
        e,
        dir,
        move |s| {
            let _ = tell.emit("download", s);
        },
        move |_| {
            if let Ok(data) = data_dir(&done) {
                let mut s = model::load_settings(&data);
                s.model = Some(e.name.into());
                s.model_file = None;
                let _ = model::save_settings(&data, &s);
            }
        },
    );
    Ok(())
}

#[tauri::command]
fn pause_download(state: State<'_, AppState>) {
    if let Some(j) = state.download.lock().unwrap().as_ref() {
        j.ctl.pause.store(true, Ordering::Relaxed);
    }
}

/// Stop the download and delete what it had.
#[tauri::command]
fn cancel_download(app: AppHandle, state: State<'_, AppState>) -> Result<(), Problem> {
    let slot = state.download.lock().unwrap();
    let Some(j) = slot.as_ref() else {
        return Ok(());
    };
    if j.alive() {
        // its thread deletes what it had, and says so
        j.ctl.cancel.store(true, Ordering::Relaxed);
        return Ok(());
    }
    let e = model::entry(&j.name()).ok_or("no such model")?;
    let _ = fs::remove_file(model::part_in(&data_dir(&app)?.join("models"), e));
    j.set_step("cancelled");
    let _ = app.emit("download", j.state());
    Ok(())
}

/// A .pkf already on this computer, checked by loading it.
#[tauri::command]
async fn use_model_file(app: AppHandle, state: State<'_, AppState>) -> Result<bool, Problem> {
    let Some(file) = file_dialog(&app)
        .add_filter(t("dialog.model_files"), &["pkf"])
        .blocking_pick_file()
    else {
        return Ok(false);
    };
    use_file(&app, &state, &file.into_path().map_err(|e| e.to_string())?)?;
    Ok(true)
}

/// A .pkf opened from outside the window, double-clicked or given on the
/// command line, as Open a model file uses one: its name, to say so.
#[tauri::command]
async fn open_model(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> Result<String, Problem> {
    let path = PathBuf::from(path);
    use_file(&app, &state, &path)?;
    Ok(path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default())
}

/// path the model in use once the engine has loaded it (model::choose_file):
/// one pktito cannot load leaves the one in use as it was.
fn use_file(app: &AppHandle, state: &AppState, path: &Path) -> Result<(), Problem> {
    {
        let mut slot = state.engine.lock().unwrap();
        ensure_engine(&mut slot, path)
            .map_err(|why| Problem::new(model::trouble(&why)).detail(why))?;
    }
    Ok(model::choose_file(&data_dir(app)?, path)?)
}

/// Of these paths, the files that are there: the window queues no others.
/// A drop or the command line can name one that is not; Windows' Open
/// dialog refuses a typed name of nothing itself (the Windows test,
/// 2026-10-21), though rfd 0.16 sets only FOS_ALLOWMULTISELECT.
#[tauri::command]
fn existing(paths: Vec<String>) -> Vec<String> {
    paths
        .into_iter()
        .filter(|p| Path::new(p).is_file())
        .collect()
}

/// Files named on the command line (`pktito-app a.mp4 b.mp3`, or a .pkf
/// Explorer opens with it), to queue or use at start, and any the system
/// handed over before the page was there (hand_over). From here on they go
/// to the page as they come.
#[tauri::command]
fn initial_files(state: State<'_, AppState>) -> Vec<String> {
    let mut files = files_in(std::env::args().skip(1), Path::new(""));
    let mut opened = state.opened.lock().unwrap();
    files.append(&mut opened);
    state.page_ready.store(true, Ordering::Release);
    files
}

/// The files among a program's arguments, whole paths, relative to cwd.
/// Made whole as written (std::path::absolute): fs::canonicalize's begin
/// \\?\ on Windows, which the window would show (the Windows test,
/// 2026-10-23).
fn files_in(args: impl Iterator<Item = String>, cwd: &Path) -> Vec<String> {
    args.filter(|a| !a.starts_with('-'))
        .map(|a| cwd.join(a))
        .filter(|p| p.is_file())
        .map(|p| {
            std::path::absolute(&p)
                .unwrap_or(p)
                .to_string_lossy()
                .into_owned()
        })
        .collect()
}

/// Files the system hands the app while it runs: macOS's open events, and
/// on Windows and Linux a second start's arguments (the single-instance
/// plugin's), its window brought forward. To the page if it is there, else
/// kept for it (initial_files).
fn hand_over(app: &AppHandle, files: Vec<String>) {
    let state = app.state::<AppState>();
    let mut opened = state.opened.lock().unwrap();
    if !state.page_ready.load(Ordering::Acquire) {
        opened.extend(files);
        return;
    }
    if !files.is_empty() {
        let _ = app.emit("open", files);
    }
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

#[tauri::command]
async fn pick_files(app: AppHandle) -> Result<Vec<String>, Problem> {
    let files = file_dialog(&app)
        .add_filter(t("dialog.media_files"), decode::MEDIA)
        .blocking_pick_files();
    Ok(files
        .unwrap_or_default()
        .into_iter()
        .filter_map(|f| f.into_path().ok())
        .map(|p| p.to_string_lossy().into_owned())
        .collect())
}

/// How a transcription goes. Each event carries what is new: the outline
/// and the audio once read, the segments as each chunk finishes.
#[derive(Clone, Serialize, Default)]
struct JobEvent {
    id: u32,
    /// "reading", "loading", "transcribing", "done", "stopped" or "error"
    step: &'static str,
    /// Of the audio, how much is transcribed: 0 to 1.
    done: f64,
    /// Seconds left, as far as can be told; -1 until it can be.
    eta: f64,
    duration: f64,
    segments: Vec<transcribe::Segment>,
    peaks: Vec<u8>,
    /// The audio as pktito heard it, to play.
    audio: Option<String>,
    /// A video's frame rate, 0 for audio: for times in frames.
    fps: f64,
    took: f64,
    /// Of that, the seconds transcribing it; while it goes, those so far.
    transcribed: f64,
    model: String,
    /// Its place on the recent list.
    recent: Option<String>,
    /// An error: what happened.
    problem: Option<Problem>,
}

#[tauri::command]
async fn transcribe(app: AppHandle, id: u32, path: String) -> Result<(), Problem> {
    tauri::async_runtime::spawn_blocking(move || job(&app, id, Path::new(&path)))
        .await
        .map_err(|e| Problem::from(e.to_string()))
}

fn job(app: &AppHandle, id: u32, path: &Path) {
    // the computer awake from the file's reading to its transcript's end,
    // let go on this thread when the job returns (awake.rs)
    let _awake = awake::awake("pktito is transcribing");
    let state = app.state::<AppState>();
    let send = |ev: JobEvent| {
        let _ = app.emit("job", JobEvent { id, ..ev });
    };
    let fail = |p: Problem| {
        send(JobEvent {
            step: "error",
            problem: Some(p),
            ..Default::default()
        })
    };
    // the engine's own words for why it stopped: memory has words of its own
    let trouble = |key: &'static str, why: String| {
        Problem::new(if why.contains("memory") {
            "problem.memory"
        } else {
            key
        })
        .detail(why)
    };
    // a Stop meant for this file's last run, which failed before it looked
    let _ = state
        .stop
        .compare_exchange(id, 0, Ordering::Relaxed, Ordering::Relaxed);
    let t0 = Instant::now();
    send(JobEvent {
        step: "reading",
        eta: -1.0,
        ..Default::default()
    });
    let x = match decode::decode(path) {
        Ok(x) => x,
        Err(e) => return fail(e.problem()),
    };
    let duration = x.len() as f64 / decode::RATE as f64;
    let fps = fps_of(path);
    let key = recent::new_id();
    let audio = audio_dir(app)
        .ok()
        .and_then(|d| audio::save(&d, &key, &x, decode::RATE as u32).ok())
        .map(|p| p.to_string_lossy().into_owned());
    let peaks = audio::peaks(&x, audio::PEAKS);
    let data = match data_dir(app) {
        Ok(d) => d,
        Err(why) => return fail(Problem::from(why)),
    };
    let Some(model_path) = model::usable(&data, &model::load_settings(&data)) else {
        return fail(Problem::new("transcribe.no_model"));
    };
    let mut slot = state.engine.lock().unwrap();
    let loading = slot.as_ref().map(|e| e.path != model_path).unwrap_or(true);
    send(JobEvent {
        step: if loading { "loading" } else { "transcribing" },
        eta: -1.0,
        duration,
        peaks: peaks.clone(),
        audio,
        fps,
        ..Default::default()
    });
    let Engine { model, asr, .. } = match ensure_engine(&mut slot, &model_path) {
        Ok(e) => e,
        Err(why) => return fail(trouble("transcribe.cant_load_model", why.to_string())),
    };
    if loading {
        send(JobEvent {
            step: "transcribing",
            eta: -1.0,
            duration,
            ..Default::default()
        });
    }
    let t1 = Instant::now();
    let r = transcribe::run(
        model,
        asr,
        &x,
        |done, segs| {
            // the pace so far, the model's loading left out
            let spent = t1.elapsed().as_secs_f64();
            let eta = if done < 1.0 {
                spent / done * (1.0 - done)
            } else {
                0.0
            };
            send(JobEvent {
                step: "transcribing",
                done,
                eta,
                duration,
                transcribed: spent,
                segments: segs.to_vec(),
                ..Default::default()
            });
        },
        || state.stop.load(Ordering::Relaxed) == id,
    );
    let transcribed = t1.elapsed().as_secs_f64();
    let name = model.name();
    drop(slot);
    let _ = state
        .stop
        .compare_exchange(id, 0, Ordering::Relaxed, Ordering::Relaxed);
    match r {
        Ok((segments, true)) => {
            let took = t0.elapsed().as_secs_f64();
            let (size, modified) = recent::stamp(path);
            let item = recent::Item {
                id: key.clone(),
                path: path.to_path_buf(),
                name: path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                duration,
                finished: recent::now(),
                took,
                transcribed,
                model: name.clone(),
                fps,
                size,
                modified,
            };
            let kept = recent::add(
                &data,
                &recent::Transcript {
                    item,
                    peaks,
                    segments,
                },
            );
            if let (Ok(gone), Ok(dir)) = (&kept, audio_dir(app)) {
                for g in gone {
                    let _ = fs::remove_file(audio::wav_in(&dir, g));
                }
                audio::cap(
                    &dir,
                    &recent::list(&data)
                        .into_iter()
                        .map(|i| i.id)
                        .collect::<Vec<_>>(),
                );
            }
            send(JobEvent {
                step: "done",
                done: 1.0,
                duration,
                took,
                transcribed,
                model: name,
                recent: kept.ok().map(|_| key),
                ..Default::default()
            });
        }
        Ok((_, false)) => send(JobEvent {
            step: "stopped",
            duration,
            ..Default::default()
        }),
        Err(why) => fail(trouble("transcribe.failed", why)),
    }
}

/// Stop a transcription after the chunk it is on.
#[tauri::command]
fn stop(state: State<'_, AppState>, id: u32) {
    state.stop.store(id, Ordering::Relaxed);
}

#[tauri::command]
fn recent_list(app: AppHandle) -> Result<Vec<recent::Item>, Problem> {
    Ok(recent::list(&data_dir(&app)?))
}

#[derive(Serialize)]
struct Opened {
    #[serde(flatten)]
    transcript: recent::Transcript,
    /// Its audio, if the cache still has it.
    audio: Option<String>,
}

#[tauri::command]
fn open_recent(app: AppHandle, id: String) -> Result<Opened, Problem> {
    let transcript = recent::load(&data_dir(&app)?, &id)?;
    let wav = audio::wav_in(&audio_dir(&app)?, &id);
    Ok(Opened {
        transcript,
        audio: wav.is_file().then(|| wav.to_string_lossy().into_owned()),
    })
}

/// Where the window's player gets a cached transcript's audio on Linux: the
/// app's own server on 127.0.0.1, since WebKitGTK's player can't read media
/// by the asset protocol (serve.rs). Only a WAV in the audio cache.
#[cfg(target_os = "linux")]
#[tauri::command]
fn audio_url(app: AppHandle, state: State<'_, AppState>, path: String) -> Result<String, Problem> {
    let p = PathBuf::from(&path);
    let dir = audio_dir(&app)?;
    if p.parent() != Some(dir.as_path()) || p.extension().map_or(true, |e| e != "wav") {
        return Err(Problem::new("play.cant"));
    }
    let mut serve = state.serve.lock().unwrap();
    if serve.is_none() {
        let s = pktito_app::serve::Server::start();
        *serve = Some(s.map_err(|e| Problem::new("play.cant").detail(e))?);
    }
    Ok(serve.as_ref().unwrap().url(&p))
}

/// A recent transcript's audio, read again from its original once the cache
/// has let it go.
#[tauri::command]
async fn reload_audio(app: AppHandle, id: String) -> Result<String, Problem> {
    tauri::async_runtime::spawn_blocking(move || {
        let t = recent::load(&data_dir(&app)?, &id)?;
        if !t.item.path.is_file() {
            return Err(Problem::new("play.moved"));
        }
        let x = decode::decode(&t.item.path).map_err(|e| e.problem())?;
        // 0.1 s: two of the encoder's frames; a file edited since is longer or shorter by more
        if (x.len() as f64 / decode::RATE as f64 - t.item.duration).abs() > 0.1 {
            return Err(Problem::new("play.changed"));
        }
        let p = audio::save(&audio_dir(&app)?, &id, &x, decode::RATE as u32)
            .map_err(|e| e.to_string())?;
        Ok(p.to_string_lossy().into_owned())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Empty the recent list, once the user says so: the window asks (app.js,
/// askClear), since the system's dialog can't make Cancel its default.
#[tauri::command]
fn clear_recent(app: AppHandle) -> Result<(), Problem> {
    let data = data_dir(&app)?;
    let dir = audio_dir(&app)?;
    for i in recent::list(&data) {
        let _ = fs::remove_file(audio::wav_in(&dir, &i.id));
    }
    recent::clear(&data);
    Ok(())
}

use format::{free_path, stem_of};

#[derive(Serialize)]
struct Saved {
    path: String,
    /// Beside the original, as asked; false when the dialog chose.
    beside: bool,
}

/// A transcript saved as the original's name with ext: beside the original
/// when next_to (unless its folder cannot be written), else where the
/// dialog says. None if the dialog was cancelled.
#[tauri::command]
async fn save_text(
    app: AppHandle,
    original: String,
    ext: String,
    text: String,
    next_to: bool,
) -> Result<Option<Saved>, Problem> {
    let original = Path::new(&original);
    let stem = stem_of(original);
    if next_to {
        if let Some(dir) = original.parent() {
            let p = free_path(dir, &stem, &ext);
            // a folder it cannot write to, such as a disc's, asks instead
            if fs::write(&p, &text).is_ok() {
                return Ok(Some(Saved {
                    path: p.to_string_lossy().into_owned(),
                    beside: true,
                }));
            }
        }
    }
    let kind = t(match ext.as_str() {
        "srt" | "vtt" => "dialog.subtitles",
        "md" => "dialog.markdown",
        "json" => "dialog.json",
        _ => "dialog.text",
    });
    let mut dialog = file_dialog(&app)
        .set_file_name(format!("{stem}.{ext}"))
        .add_filter(kind, &[ext.as_str()]);
    if let Some(dir) = original.parent().filter(|d| d.is_dir()) {
        dialog = dialog.set_directory(dir);
    }
    let Some(file) = dialog.blocking_save_file() else {
        return Ok(None);
    };
    let path = file.into_path().map_err(|e| e.to_string())?;
    fs::write(&path, text).map_err(|e| {
        Problem::new("save.failed")
            .with("name", stem_of(&path))
            .detail(e)
    })?;
    Ok(Some(Saved {
        path: path.to_string_lossy().into_owned(),
        beside: false,
    }))
}

#[derive(Deserialize)]
struct Export {
    original: String,
    ext: String,
    text: String,
}

/// Transcripts saved by their originals' names: each beside its original, or
/// all into dir when there is one. The paths written.
#[tauri::command]
async fn save_many(dir: Option<String>, files: Vec<Export>) -> Result<Vec<String>, Problem> {
    let mut written = Vec::new();
    for f in files {
        let original = Path::new(&f.original);
        let d = match &dir {
            Some(d) => PathBuf::from(d),
            None => original.parent().map(Path::to_path_buf).unwrap_or_default(),
        };
        let p = free_path(&d, &stem_of(original), &f.ext);
        let name = p
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        fs::write(&p, &f.text)
            .map_err(|e| Problem::new("save.failed").with("name", name).detail(e))?;
        written.push(p.to_string_lossy().into_owned());
    }
    Ok(written)
}

#[tauri::command]
async fn pick_folder(app: AppHandle) -> Result<Option<String>, Problem> {
    let Some(dir) = file_dialog(&app).blocking_pick_folder() else {
        return Ok(None);
    };
    Ok(Some(
        dir.into_path()
            .map_err(|e| e.to_string())?
            .to_string_lossy()
            .into_owned(),
    ))
}

/// One transcript off the recent list, with its audio in the cache; the
/// file it was made from is not touched.
#[tauri::command]
fn forget_recent(app: AppHandle, id: String) -> Result<(), Problem> {
    let data = data_dir(&app)?;
    recent::remove(&data, &id)?;
    if let Ok(dir) = audio_dir(&app) {
        let _ = fs::remove_file(audio::wav_in(&dir, &id));
    }
    Ok(())
}

/// A word of a transcript, as the window holds it: seconds.
#[derive(Deserialize)]
struct TimedWord {
    text: String,
    start: f64,
    end: f64,
}

/// The cues of a transcript's subtitle track, from the engine's pk_captions,
/// so that the app's .srt and .vtt are the CLI's: times in milliseconds.
#[tauri::command]
fn captions(
    words: Vec<TimedWord>,
    english: i32,
    duration: f64,
) -> Result<Vec<pktito::Cue>, Problem> {
    let ms = |s: f64| (s * 1000.0).round() as i64;
    let w: Vec<(String, i64, i64)> = words
        .into_iter()
        .map(|w| (w.text, ms(w.start), ms(w.end)))
        .collect();
    Ok(pktito::captions(&w, english, ms(duration))?)
}

/// pktito-cli on the PATH, from the macOS menu bar's Install Command Line
/// Tool: elsewhere the installers put it there. What happened, to say; ""
/// when the password was not given.
#[tauri::command]
async fn install_cli() -> Result<String, Problem> {
    #[cfg(target_os = "macos")]
    return tauri::async_runtime::spawn_blocking(link_cli)
        .await
        .map_err(|e| Problem::from(e.to_string()))?;
    #[cfg(not(target_os = "macos"))]
    Err(Problem::new("cli.on_path"))
}

/// A link to the bundle's pktito-cli in /usr/local/bin, which macOS has on
/// every PATH, as VS Code installs code. A folder the user can write to (an
/// Intel Mac's, under Homebrew) takes it at once; else macOS asks for an
/// administrator's password, through osascript's do shell script, which
/// gets the path as an argument, so that nothing in it is read as a command.
#[cfg(target_os = "macos")]
fn link_cli() -> Result<String, Problem> {
    use std::process::Command;
    const DST: &str = "/usr/local/bin/pktito-cli";
    let src = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .with_file_name("pktito-cli");
    if !src.is_file() {
        return Err(Problem::new("cli.missing").detail(src.display()));
    }
    if fs::read_link(DST).map(|t| t == src).unwrap_or(false) {
        return Ok(t("cli.already"));
    }
    let done = t("cli.installed");
    let _ = fs::create_dir_all("/usr/local/bin");
    if Command::new("ln")
        .arg("-sf")
        .arg(&src)
        .arg(DST)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
    {
        return Ok(done);
    }
    // the prompt is an AppleScript string: its quotes and backslashes escaped
    let prompt = t("cli.prompt").replace('\\', "\\\\").replace('"', "\\\"");
    let script = format!(
        "do shell script \"mkdir -p /usr/local/bin && ln -sf \" & quoted form of (item 1 of argv) & \" {DST}\" \
         with prompt \"{prompt}\" with administrator privileges"
    );
    let out = Command::new("osascript")
        .args(["-e", "on run argv", "-e", &script, "-e", "end run"])
        .arg(&src)
        .output()
        .map_err(|e| Problem::new("cli.failed").detail(e))?;
    if out.status.success() {
        return Ok(done);
    }
    let why = String::from_utf8_lossy(&out.stderr);
    // the dialog's Cancel: nothing to say
    if why.contains("(-128)") {
        return Ok(String::new());
    }
    Err(Problem::new("cli.failed").detail(why.trim()))
}

/// A finished transcript in one of Save's formats (format.rs), the bytes
/// pktito-cli writes for it too.
#[tauri::command]
fn text_of(
    ext: String,
    name: String,
    duration: f64,
    model: Option<String>,
    segments: Vec<transcribe::Segment>,
    times: bool,
) -> Result<String, Problem> {
    Ok(format::text(
        &format::Transcript {
            name: &name,
            duration,
            model: model.as_deref(),
            segments: &segments,
        },
        &ext,
        times,
    )?)
}

/// Show a file in the file manager, selected.
#[tauri::command]
fn reveal(path: String) -> Result<(), Problem> {
    reveal_in_folder(Path::new(&path))
}

/// The model in use, in Finder or Explorer: its file selected, or the
/// folder the app keeps its models in when none is there.
#[tauri::command]
fn reveal_model(app: AppHandle) -> Result<(), Problem> {
    let data = data_dir(&app)?;
    match model::usable(&data, &model::load_settings(&data)) {
        Some(file) => reveal_in_folder(&file),
        None => {
            let dir = data.join("models");
            fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            reveal_in_folder(&dir)
        }
    }
}

/// The window's menu on Windows, Restore, Move, Size, Minimize, Maximize
/// and Close, as a title bar's right-click or Alt+Space opens it: at the
/// pointer, or under the header's left end. The window draws no title bar
/// (main_window), so the page asks for it. The system's own menu, in the
/// system's language, its items enabled as the window's state allows; if
/// the window has none, the same six in English.
#[tauri::command]
fn window_menu(window: tauri::WebviewWindow, at_cursor: bool) {
    #[cfg(target_os = "windows")]
    unsafe {
        use std::ffi::c_void;
        #[repr(C)]
        struct Point {
            x: i32,
            y: i32,
        }
        #[link(name = "user32")]
        extern "system" {
            fn GetSystemMenu(hwnd: *mut c_void, revert: i32) -> *mut c_void;
            fn CreatePopupMenu() -> *mut c_void;
            fn AppendMenuW(menu: *mut c_void, flags: u32, id: usize, text: *const u16) -> i32;
            fn DestroyMenu(menu: *mut c_void) -> i32;
            fn EnableMenuItem(menu: *mut c_void, id: u32, flags: u32) -> i32;
            fn SetMenuDefaultItem(menu: *mut c_void, id: u32, by_position: u32) -> i32;
            fn TrackPopupMenu(
                menu: *mut c_void,
                flags: u32,
                x: i32,
                y: i32,
                reserved: i32,
                hwnd: *mut c_void,
                rect: *const c_void,
            ) -> i32;
            fn PostMessageW(hwnd: *mut c_void, msg: u32, wparam: usize, lparam: isize) -> i32;
            fn GetCursorPos(p: *mut Point) -> i32;
            fn ClientToScreen(hwnd: *mut c_void, p: *mut Point) -> i32;
            fn IsZoomed(hwnd: *mut c_void) -> i32;
            fn SetForegroundWindow(hwnd: *mut c_void) -> i32;
        }
        const SC_SIZE: u32 = 0xF000;
        const SC_MOVE: u32 = 0xF010;
        const SC_MINIMIZE: u32 = 0xF020;
        const SC_MAXIMIZE: u32 = 0xF030;
        const SC_CLOSE: u32 = 0xF060;
        const SC_RESTORE: u32 = 0xF120;
        const MF_GRAYED: u32 = 0x1;
        const MF_SEPARATOR: u32 = 0x800;
        const TPM_RIGHTBUTTON: u32 = 0x2;
        const TPM_RETURNCMD: u32 = 0x100;
        const WM_SYSCOMMAND: u32 = 0x0112;
        let Ok(hwnd) = window.hwnd() else { return };
        let h = hwnd.0 as *mut c_void;
        let mut menu = GetSystemMenu(h, 0);
        let own = menu.is_null();
        if own {
            menu = CreatePopupMenu();
            if menu.is_null() {
                return;
            }
            for (id, key) in [
                (SC_RESTORE, "window.restore"),
                (SC_MOVE, "window.move"),
                (SC_SIZE, "window.size"),
                (SC_MINIMIZE, "window.minimize"),
                (SC_MAXIMIZE, "window.maximize"),
                (0, ""),
                (SC_CLOSE, "window.close"),
            ] {
                let text = match id {
                    0 => String::new(),
                    SC_CLOSE => format!("{}\tAlt+F4", t(key)),
                    _ => t(key),
                };
                let w: Vec<u16> = text.encode_utf16().chain([0]).collect();
                AppendMenuW(
                    menu,
                    if id == 0 { MF_SEPARATOR } else { 0 },
                    id as usize,
                    w.as_ptr(),
                );
            }
        }
        let big = IsZoomed(h) != 0;
        for (id, on) in [
            (SC_RESTORE, big),
            (SC_MOVE, !big),
            (SC_SIZE, !big),
            (SC_MINIMIZE, true),
            (SC_MAXIMIZE, !big),
            (SC_CLOSE, true),
        ] {
            EnableMenuItem(menu, id, if on { 0 } else { MF_GRAYED });
        }
        SetMenuDefaultItem(menu, SC_CLOSE, 0);
        let mut p = Point { x: 0, y: 0 };
        if at_cursor {
            GetCursorPos(&mut p);
        } else {
            p.y = (60.0 * window.scale_factor().unwrap_or(1.0)) as i32; // the header's height
            ClientToScreen(h, &mut p);
        }
        SetForegroundWindow(h);
        let cmd = TrackPopupMenu(
            menu,
            TPM_RIGHTBUTTON | TPM_RETURNCMD,
            p.x,
            p.y,
            0,
            h,
            std::ptr::null(),
        );
        if cmd != 0 {
            PostMessageW(h, WM_SYSCOMMAND, cmd as usize, 0);
        }
        if own {
            DestroyMenu(menu);
        }
    }
    #[cfg(not(target_os = "windows"))]
    let _ = (window, at_cursor);
}

/// One of About's links, in the browser. An https address and nothing else,
/// so that the page can open no file or program by it.
#[tauri::command]
fn open_link(url: String) -> Result<(), Problem> {
    if !url.starts_with("https://") || url.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(format!("Not a web address: {url}").into());
    }
    open_in_browser(&url)
}

#[cfg(target_os = "macos")]
fn open_in_browser(url: &str) -> Result<(), Problem> {
    std::process::Command::new("open")
        .arg(url)
        .spawn()
        .map(|_| ())
        .map_err(|e| Problem::new("link.failed").detail(e))
}

/// The shell's own call, as reveal_in_folder's is, which hands the address
/// to the default browser. It returns above 32 when it worked, else an error
/// code (the Win32 documentation of ShellExecuteW).
#[cfg(target_os = "windows")]
fn open_in_browser(url: &str) -> Result<(), Problem> {
    use std::ffi::c_void;
    #[link(name = "shell32")]
    extern "system" {
        fn ShellExecuteW(
            hwnd: *mut c_void,
            op: *const u16,
            file: *const u16,
            params: *const u16,
            dir: *const u16,
            show: i32,
        ) -> isize;
    }
    const SW_SHOWNORMAL: i32 = 1;
    let wide = |s: &str| s.encode_utf16().chain(Some(0)).collect::<Vec<u16>>();
    let (op, file) = (wide("open"), wide(url));
    let r = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            op.as_ptr(),
            file.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    };
    if r <= 32 {
        return Err(Problem::new("link.failed").detail(format!("ShellExecuteW: {r}")));
    }
    Ok(())
}

/// xdg-open, which xdg-utils brings and the .deb recommends. Where it is
/// missing (WSL, a system with no desktop) the spawn's error, "No such file
/// or directory", reads as if the file were missing, so `missing` says what
/// is; any other error is `failed`, with what the system said.
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn xdg_open(
    arg: impl AsRef<std::ffi::OsStr>,
    missing: &'static str,
    failed: &'static str,
) -> Result<(), Problem> {
    std::process::Command::new("xdg-open")
        .arg(arg)
        .spawn()
        .map(|_| ())
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => Problem::new(missing),
            _ => Problem::new(failed).detail(e),
        })
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn open_in_browser(url: &str) -> Result<(), Problem> {
    xdg_open(url, "link.needs_xdg", "link.failed")
}

#[cfg(target_os = "macos")]
fn reveal_in_folder(path: &Path) -> Result<(), Problem> {
    std::process::Command::new("open")
        .arg("-R")
        .arg(path)
        .spawn()
        .map(|_| ())
        .map_err(|e| Problem::new("reveal.failed").detail(e))
}

/// The shell's own way. `explorer /select,"path"` started from the app
/// opened nothing in the Windows test (2026-10-21, three toasts): the shell
/// call is what Explorer itself answers. Given one item's full ID list and no
/// others, it opens the item's folder with the item selected.
#[cfg(target_os = "windows")]
fn reveal_in_folder(path: &Path) -> Result<(), Problem> {
    use std::ffi::c_void;
    use std::os::windows::ffi::OsStrExt;
    #[link(name = "shell32")]
    extern "system" {
        fn ILCreateFromPathW(path: *const u16) -> *mut c_void;
        fn ILFree(list: *mut c_void);
        fn SHOpenFolderAndSelectItems(
            folder: *const c_void,
            n: u32,
            items: *const *const c_void,
            flags: u32,
        ) -> i32;
    }
    #[link(name = "ole32")]
    extern "system" {
        fn CoInitializeEx(reserved: *mut c_void, model: u32) -> i32;
        fn CoUninitialize();
    }
    const COINIT_APARTMENTTHREADED: u32 = 0x2;
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    unsafe {
        // the window's thread has COM already (S_FALSE), which still wants
        // its CoUninitialize; another thread's model refuses (negative)
        let com = CoInitializeEx(std::ptr::null_mut(), COINIT_APARTMENTTHREADED);
        let list = ILCreateFromPathW(wide.as_ptr());
        let hr = if list.is_null() {
            -1
        } else {
            SHOpenFolderAndSelectItems(list, 0, std::ptr::null(), 0)
        };
        if !list.is_null() {
            ILFree(list);
        }
        if com >= 0 {
            CoUninitialize();
        }
        if hr < 0 {
            return Err(Problem::new("reveal.failed")
                .detail(format!("SHOpenFolderAndSelectItems: 0x{:08x}", hr as u32)));
        }
    }
    Ok(())
}

/// The file manager's own D-Bus call where there is one (GNOME's Files,
/// KDE's Dolphin), which selects it; else its folder, opened.
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn reveal_in_folder(path: &Path) -> Result<(), Problem> {
    use std::process::{Command, Stdio};
    let mut uri = String::from("file://");
    for b in path.to_string_lossy().bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b'-' | b'_' | b'.' | b'~' => {
                uri.push(b as char)
            }
            _ => uri.push_str(&format!("%{b:02X}")),
        }
    }
    let shown = Command::new("gdbus")
        .args([
            "call",
            "--session",
            "--dest",
            "org.freedesktop.FileManager1",
            "--object-path",
            "/org/freedesktop/FileManager1",
        ])
        .args([
            "--method",
            "org.freedesktop.FileManager1.ShowItems",
            &format!("['{uri}']"),
            "",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if shown {
        return Ok(());
    }
    xdg_open(
        path.parent().unwrap_or(Path::new("/")),
        "reveal.needs_xdg",
        "reveal.failed",
    )
}

/// `pktito-app --transcribe FILE [--model PKF] [--json]`: the window's
/// pipeline without the window, printing each segment, or with --json the
/// segments and their words as the window gets them; for checking a build.
fn headless(args: &[String]) -> Result<(), String> {
    let file = args
        .get(1)
        .ok_or("--transcribe FILE [--model PKF] [--json]")?;
    let json = args.iter().any(|a| a == "--json");
    let model_path = match args.iter().position(|a| a == "--model") {
        Some(i) => PathBuf::from(args.get(i + 1).ok_or("--model needs a path")?),
        None => std::env::var("PKTITO_MODEL")
            .map(PathBuf::from)
            .map_err(|_| "--model PKF, or PKTITO_MODEL")?,
    };
    let t0 = Instant::now();
    let x = decode::decode(Path::new(file)).map_err(|e| e.message())?;
    let fps = fps_of(Path::new(file));
    let t1 = Instant::now();
    let model = pktito::Model::load(&model_path)?;
    let mut asr = pktito::Asr::new(&model, transcribe::threads_wanted())?;
    let (segs, _) = transcribe::run(
        &model,
        &mut asr,
        &x,
        |_, segs| {
            for s in segs.iter().filter(|_| !json) {
                println!("{:>10.2}  {}", s.start, s.text);
            }
        },
        || false,
    )?;
    if json {
        println!("{}", serde_json::to_string(&segs).unwrap());
    }
    eprintln!(
        "{} segments, {:.3} s of audio{}: decoded in {:.3} s, transcribed in {:.3} s ({}, {}, {} threads)",
        segs.len(),
        x.len() as f64 / decode::RATE as f64,
        if fps > 0.0 { format!(", video at {fps:.3} fps") } else { String::new() },
        (t1 - t0).as_secs_f64(),
        t1.elapsed().as_secs_f64(),
        model.name(),
        pktito::backend(),
        asr.threads()
    );
    Ok(())
}

/// `pktito-app --download NAME DIR`: the window's download alone, resuming
/// what a stopped one kept and trying again as the window does.
fn download_alone(args: &[String]) -> Result<PathBuf, String> {
    let e = model::entry(args.get(1).ok_or("--download NAME DIR")?).ok_or("no such model")?;
    let dir = PathBuf::from(args.get(2).ok_or("--download NAME DIR")?);
    let job = Arc::new(download::Job::new(e, &dir));
    let (tx, rx) = std::sync::mpsc::channel();
    job.start(
        e,
        dir,
        |s| match s.step {
            "failed" => eprintln!(
                "\n{}{}",
                s.problem.as_ref().map(|p| p.line()).unwrap_or_default(),
                if s.retry { " (trying again)" } else { "" }
            ),
            _ => eprint!(
                "\r{} {} of {} MB   ",
                s.step,
                s.got / 1_000_000,
                s.total / 1_000_000
            ),
        },
        move |p| {
            let _ = tx.send(p);
        },
    );
    while job.alive() {
        std::thread::sleep(Duration::from_millis(100));
    }
    rx.try_recv()
        .map_err(|_| job.state().problem.map(|p| p.line()).unwrap_or_default())
}

/// Where the traffic lights go: AppKit puts their left edge at x and their
/// centre 2 px above y (14 px buttons 23 apart, measured on macOS 26), so
/// their centre is the header row's buttons', 31 px down, and they end at
/// x = 78 (ui/app.css, html[data-chrome=mac] and .pk-head).
#[cfg(target_os = "macos")]
const LIGHTS: (f64, f64) = (18.0, 33.0);

/// The window, as tauri.conf.json has it (which leaves it to this), on
/// macOS and Windows without a title bar of its own: the page runs up to
/// the window's top, and the header row of each screen is the title bar,
/// its empty space dragging the window. On macOS the traffic lights are in
/// its left end; on Windows the page draws minimize, maximize and close at
/// its right, as Windows 11 draws them (ui/app.css, .pk-caption). The page
/// is told which before its first script runs, as html[data-chrome].
/// Linux keeps the title bar its desktop draws.
fn main_window(app: &tauri::App) -> tauri::Result<tauri::WebviewWindow> {
    let config = app
        .config()
        .app
        .windows
        .iter()
        .find(|w| w.label == "main")
        .cloned()
        .unwrap_or_default();
    let builder = tauri::WebviewWindowBuilder::from_config(app.handle(), &config)?;
    #[cfg(target_os = "macos")]
    let builder = builder
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .hidden_title(true)
        .traffic_light_position(tauri::LogicalPosition::new(LIGHTS.0, LIGHTS.1))
        .initialization_script("window.__PK_CHROME = 'mac';");
    #[cfg(target_os = "windows")]
    let builder = builder
        .decorations(false)
        .shadow(true)
        .initialization_script("window.__PK_CHROME = 'win';");
    builder.build()
}

/// The size the window was given at launch, which a resize to it need not save.
static PLACED: Mutex<Option<(f64, f64)>> = Mutex::new(None);

/// The window, sized by fit_size before it is shown, in the middle of the
/// monitor's work area. tauri.conf.json makes it hidden until then.
fn place_window(w: &tauri::WebviewWindow, saved: Option<[f64; 2]>) {
    let monitor = w
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| w.primary_monitor().ok().flatten());
    let room = match (monitor, w.outer_size(), w.inner_size()) {
        (Some(m), Ok(outer), Ok(inner)) => {
            let scale = m.scale_factor();
            let area = m.work_area().size.to_logical::<f64>(scale);
            // the title bar and borders lie outside the size set
            let frame = (
                (outer.width.saturating_sub(inner.width)) as f64 / scale,
                (outer.height.saturating_sub(inner.height)) as f64 / scale,
            );
            Some((area.width - frame.0, area.height - frame.1))
        }
        _ => None,
    };
    let (width, height) = fit_size(saved, room);
    *PLACED.lock().unwrap() = Some((width, height));
    let _ = w.set_size(tauri::LogicalSize::new(width, height));
    let _ = w.center();
    let _ = w.show();
}

/// The window's size: as the user last left it, else 680 by 800, 65
/// characters a line of transcript and 20 lines of it. 720 by 1018, a sheet
/// of paper's proportion, took the work area's whole height on the commonest
/// screens, and shrunk in proportion it opened 577 px wide on a 1536 by 864
/// laptop, at 51 characters a line (the window study, 2026-10-23). It stays
/// within room, the work area less the window's frame: shorter where it
/// would take more than nine tenths of its height, 680 by 648 under a 1366
/// by 768 screen's taskbar, and narrower only on a screen narrower than it;
/// a size the user left is cut to fit. Neither goes below the window's
/// minimum, 480 by 480 (tauri.conf.json).
fn fit_size(saved: Option<[f64; 2]>, room: Option<(f64, f64)>) -> (f64, f64) {
    let [mut width, mut height] = saved.unwrap_or([680.0, 800.0]);
    if let Some((w, h)) = room {
        if saved.is_none() {
            height = height.min((h * 0.9).floor());
        }
        (width, height) = (width.min(w), height.min(h));
    }
    (width.max(480.0).round(), height.max(480.0).round())
}

/// The window's size as the user leaves it, saved for the next launch half a
/// second after the last resize: not while it is maximized, minimized or
/// full screen, nor the size it was given.
fn remember_size(window: &tauri::Window) {
    static LAST: AtomicU32 = AtomicU32::new(0);
    let off = |r: tauri::Result<bool>| r.unwrap_or(true);
    if off(window.is_maximized()) || off(window.is_minimized()) || off(window.is_fullscreen()) {
        return;
    }
    let (Ok(size), Ok(scale)) = (window.inner_size(), window.scale_factor()) else {
        return;
    };
    let size = size.to_logical::<f64>(scale);
    let size = (size.width.round(), size.height.round());
    if *PLACED.lock().unwrap() == Some(size) {
        return;
    }
    let n = LAST.fetch_add(1, Ordering::Relaxed) + 1;
    let app = window.app_handle().clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(500));
        if LAST.load(Ordering::Relaxed) != n {
            return;
        }
        if let Ok(data) = data_dir(&app) {
            let mut s = model::load_settings(&data);
            s.window = Some([size.0, size.1]);
            let _ = model::save_settings(&data, &s);
        }
    });
}

/// The window's big icon, which the taskbar and Alt+Tab show, from the exe's
/// own (tauri-build writes it as resource 32512). tao sets the small one
/// alone, and without a big one the taskbar shows what its cache holds for
/// the exe's path: a generic window, for an exe written over at one path
/// (the Windows test, 2026-10-21).
#[cfg(target_os = "windows")]
fn big_icon(window: &tauri::WebviewWindow) {
    use std::ffi::c_void;
    #[link(name = "user32")]
    extern "system" {
        fn LoadImageW(
            inst: *mut c_void,
            name: *const u16,
            kind: u32,
            cx: i32,
            cy: i32,
            load: u32,
        ) -> *mut c_void;
        fn GetSystemMetrics(index: i32) -> i32;
        fn SendMessageW(hwnd: *mut c_void, msg: u32, wparam: usize, lparam: isize) -> isize;
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn GetModuleHandleW(name: *const u16) -> *mut c_void;
    }
    const IMAGE_ICON: u32 = 1;
    const LR_SHARED: u32 = 0x8000;
    const SM_CXICON: i32 = 11;
    const SM_CYICON: i32 = 12;
    const WM_SETICON: u32 = 0x0080;
    const ICON_BIG: usize = 1;
    let Ok(hwnd) = window.hwnd() else { return };
    unsafe {
        let size = (GetSystemMetrics(SM_CXICON), GetSystemMetrics(SM_CYICON));
        let icon = LoadImageW(
            GetModuleHandleW(std::ptr::null()),
            32512 as *const u16,
            IMAGE_ICON,
            size.0,
            size.1,
            LR_SHARED,
        );
        if !icon.is_null() {
            SendMessageW(hwnd.0 as *mut c_void, WM_SETICON, ICON_BIG, icon as isize);
        }
    }
}

/// WebView2 as an app's window, not a browser's: its context menu cut to
/// what an app's text has, Cut, Copy, Paste and Select all where they
/// apply (it offered Print, Copy link to highlight, Emoji, passwords and
/// More tools: the Windows recheck, 2026-10-22), and none where none is
/// left; and its accelerator keys off, F5 and Ctrl+R's reload above all,
/// which lost the transcript open. The page stops the menu everywhere but
/// text fields and selected text (app.js), so these are the ones left.
#[cfg(target_os = "windows")]
fn tame_webview(window: &tauri::WebviewWindow) {
    let _ = window.with_webview(|pw| unsafe {
        use webview2_com::Microsoft::Web::WebView2::Win32::{
            ICoreWebView2Settings3, ICoreWebView2_11,
        };
        use windows_core::{Interface, PWSTR};
        let Ok(core) = pw.controller().CoreWebView2() else {
            return;
        };
        if let Ok(settings) = core
            .Settings()
            .and_then(|s| s.cast::<ICoreWebView2Settings3>())
        {
            let _ = settings.SetAreBrowserAcceleratorKeysEnabled(false);
        }
        let Ok(core) = core.cast::<ICoreWebView2_11>() else {
            return;
        };
        let handler =
            webview2_com::ContextMenuRequestedEventHandler::create(Box::new(|_, args| {
                let Some(args) = args else { return Ok(()) };
                let items = args.MenuItems()?;
                let mut n = 0u32;
                items.Count(&mut n)?;
                for i in (0..n).rev() {
                    let mut name = PWSTR::null();
                    items.GetValueAtIndex(i)?.Name(&mut name)?;
                    if !["cut", "copy", "paste", "selectAll"]
                        .contains(&webview2_com::take_pwstr(name).as_str())
                    {
                        items.RemoveValueAtIndex(i)?;
                    }
                }
                items.Count(&mut n)?;
                if n == 0 {
                    args.SetHandled(true)?;
                }
                Ok(())
            }));
        let mut token = 0i64;
        let _ = core.add_ContextMenuRequested(&handler, &mut token);
    });
}

/// The menu bar, on macOS alone, where it is outside the window and its Edit
/// menu is what gives a text field ⌘C and ⌘V. A Windows or Linux window has
/// none: there it only repeated the window's own controls and shortcuts,
/// which the page takes itself, and stayed light in dark mode (the Windows
/// test, 2026-10-21).
#[cfg(target_os = "macos")]
fn menu_bar(app: &tauri::App) -> tauri::Result<()> {
    use tauri::menu::{MenuBuilder, MenuItemBuilder, PredefinedMenuItem, SubmenuBuilder};
    let item = |id: &str, text: &str, key: Option<&str>| {
        let b = MenuItemBuilder::with_id(id, text);
        match key {
            Some(k) => b.accelerator(k).build(app),
            None => b.build(app),
        }
    };
    let file = SubmenuBuilder::new(app, t("menu.file"))
        .item(&item("add", &t("menu.add"), Some("CmdOrCtrl+O"))?)
        .separator()
        .item(&item("save-txt", &t("menu.save_txt"), Some("CmdOrCtrl+S"))?)
        .item(&item(
            "save-srt",
            &t("menu.save_srt"),
            Some("CmdOrCtrl+Shift+S"),
        )?)
        .item(&item("save-vtt", &t("menu.save_vtt"), None)?)
        .item(&item("save-md", &t("menu.save_md"), None)?)
        .item(&item("save-json", &t("menu.save_json"), None)?)
        .separator()
        .item(&PredefinedMenuItem::close_window(app, None)?)
        .build()?;
    let edit = SubmenuBuilder::new(app, t("menu.edit"))
        .item(&PredefinedMenuItem::undo(app, None)?)
        .item(&PredefinedMenuItem::redo(app, None)?)
        .separator()
        .item(&PredefinedMenuItem::cut(app, None)?)
        .item(&PredefinedMenuItem::copy(app, None)?)
        .item(&PredefinedMenuItem::paste(app, None)?)
        .item(&PredefinedMenuItem::select_all(app, None)?)
        .separator()
        .item(&item("find", &t("menu.find"), Some("CmdOrCtrl+F"))?)
        .build()?;
    // the window's own About, which has the credits, not the system's panel
    let pktito = SubmenuBuilder::new(app, "pktito")
        .item(&item("about", &t("menu.about"), None)?)
        .separator()
        .item(&item("install-cli", &t("menu.install_cli"), None)?)
        .separator()
        .item(&PredefinedMenuItem::hide(app, None)?)
        .item(&PredefinedMenuItem::hide_others(app, None)?)
        .separator()
        .item(&PredefinedMenuItem::quit(app, None)?)
        .build()?;
    app.set_menu(
        MenuBuilder::new(app)
            .item(&pktito)
            .item(&file)
            .item(&edit)
            .build()?,
    )?;
    app.on_menu_event(|app, event| {
        let _ = app.emit("menu", event.id().0.as_str());
    });
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(|a| a == "--download").unwrap_or(false) {
        match download_alone(&args) {
            Ok(p) => println!("\nverified: {}", p.display()),
            Err(why) => {
                eprintln!("\npktito: {why}");
                std::process::exit(1);
            }
        }
        return;
    }
    if args.first().map(|a| a == "--transcribe").unwrap_or(false) {
        if let Err(why) = headless(&args) {
            eprintln!("pktito: {why}");
            std::process::exit(1);
        }
        return;
    }
    // WebKitGTK's DMA-BUF renderer draws the window as stripes on a Pi 5
    // under Raspberry Pi OS 13 (WebKitGTK 2.54, Mesa 26.2's V3D, labwc); with
    // it off the window is right (the pi5a test, 2026-10-23). A value
    // already set is the user's.
    #[cfg(target_os = "linux")]
    if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }
    // the catalog, before the page's scripts run: its words are the page's
    // from the first frame (strings.rs)
    let words = tauri::plugin::Builder::<tauri::Wry, ()>::new("strings")
        .js_init_script(format!("window.PK_STRINGS = {};", strings::json()))
        .build();
    let builder = tauri::Builder::default().plugin(words);
    // a second start, a .pkf double-clicked while the window is open, hands
    // its files to the first and ends (macOS sends them as open events)
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    let builder = builder.plugin(tauri_plugin_single_instance::init(|app, argv, cwd| {
        hand_over(app, files_in(argv.into_iter().skip(1), Path::new(&cwd)));
    }));
    let app = builder
        .plugin(tauri_plugin_dialog::init())
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::Resized(_) = event {
                remember_size(window);
            }
        })
        .manage(AppState::default())
        .setup(|app| {
            // the audio no transcript on the recent list needs, from runs
            // before; before the window opens, so none of this run's goes too
            if let (Ok(data), Ok(dir)) = (data_dir(app.handle()), audio_dir(app.handle())) {
                audio::prune(
                    &dir,
                    &recent::list(&data)
                        .into_iter()
                        .map(|i| i.id)
                        .collect::<Vec<_>>(),
                );
            }
            #[cfg(target_os = "macos")]
            menu_bar(app)?;
            let w = main_window(app)?;
            #[cfg(target_os = "windows")]
            {
                big_icon(&w);
                tame_webview(&w);
            }
            let saved = data_dir(app.handle())
                .map(|d| model::load_settings(&d).window)
                .unwrap_or(None);
            place_window(&w, saved);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            status,
            set_prefs,
            choose_model,
            download_model,
            pause_download,
            cancel_download,
            use_model_file,
            open_model,
            initial_files,
            existing,
            pick_files,
            transcribe,
            stop,
            recent_list,
            open_recent,
            reload_audio,
            #[cfg(target_os = "linux")]
            audio_url,
            clear_recent,
            save_text,
            save_many,
            pick_folder,
            reveal,
            open_link,
            captions,
            text_of,
            install_cli,
            forget_recent,
            reveal_model,
            window_menu
        ])
        .build(tauri::generate_context!())
        .expect("pktito could not start");
    app.run(|app, event| match event {
        // a .pkf double-clicked in Finder or dropped on the Dock's icon
        #[cfg(target_os = "macos")]
        tauri::RunEvent::Opened { urls } => hand_over(
            app,
            urls.iter()
                .filter_map(|u| u.to_file_path().ok())
                .map(|p| p.to_string_lossy().into_owned())
                .collect(),
        ),
        _ => {
            let _ = app;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_named_are_whole_paths_as_written() {
        // the package's folder is the tests' working directory
        let args = ["Cargo.toml", "--flag", "no-such-file"].map(String::from);
        let got = files_in(args.into_iter(), Path::new(""));
        assert_eq!(got.len(), 1, "{got:?}");
        assert!(
            Path::new(&got[0]).is_absolute() && got[0].ends_with("Cargo.toml"),
            "{got:?}"
        );
        assert!(!got[0].starts_with(r"\\?\"), "{got:?}");
    }

    #[test]
    fn shorter_not_narrower() {
        assert_eq!(fit_size(None, Some((2560.0, 1400.0))), (680.0, 800.0));
        // 1366 by 768 under Windows 11's 48 px taskbar: nine tenths of 720
        assert_eq!(fit_size(None, Some((1366.0, 720.0))), (680.0, 648.0));
        // a screen narrower than the window
        assert_eq!(fit_size(None, Some((600.0, 1000.0))), (600.0, 800.0));
        // 1920 by 1080 at 150%
        assert_eq!(fit_size(None, Some((1280.0, 672.0))), (680.0, 604.0));
        // shorter than the window's minimum, which wins
        assert_eq!(fit_size(None, Some((910.0, 464.0))), (680.0, 480.0));
        assert_eq!(fit_size(None, None), (680.0, 800.0));
    }

    #[test]
    fn saved_size_cut_to_fit() {
        assert_eq!(
            fit_size(Some([900.0, 1300.0]), Some((1366.0, 720.0))),
            (900.0, 720.0)
        );
        assert_eq!(
            fit_size(Some([640.0, 600.0]), Some((1366.0, 720.0))),
            (640.0, 600.0)
        );
    }
}
