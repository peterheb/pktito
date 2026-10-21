//! pktito's desktop app: drop audio or video files, read their transcripts.
//! The audio comes out through the OS's media libraries (or ffmpeg, see
//! decode.rs) and is transcribed on this computer by pktito; each finished
//! transcript is kept on the recent list, and its audio in the cache to
//! play it back against.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio;
mod decode;
mod download;
mod model;
mod pktito;
mod recent;
mod transcribe;

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::menu::{MenuBuilder, MenuItemBuilder, PredefinedMenuItem, SubmenuBuilder};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_dialog::{DialogExt, FileDialogBuilder, MessageDialogButtons, MessageDialogKind};

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
}

const MEDIA: &[&str] = &[
    "mp3", "mp4", "m4a", "m4v", "mov", "wav", "aac", "aif", "aiff", "caf", "flac", "ogg", "opus",
    "webm", "mkv", "wma", "wmv", "avi", "3gp", "amr",
];
const VIDEO: &[&str] = &["mp4", "m4v", "mov", "webm", "mkv", "wmv", "avi", "3gp"];

fn data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path().app_data_dir().map_err(|e| e.to_string())
}

/// Where the recent list's audio is kept: the webview may read this folder
/// and nothing else (tauri.conf.json, assetProtocol).
fn audio_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path().app_cache_dir().map(|d| d.join("audio")).map_err(|e| e.to_string())
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
    backend: String,
    platform: &'static str,
    media: &'static [&'static str],
    video: &'static [&'static str],
}

/// The model file to transcribe with: the user's own, else the catalog
/// model in use, if it is on this computer.
fn usable(data: &Path, s: &model::Settings) -> Option<PathBuf> {
    if let Some(p) = s.model_file.as_ref().filter(|p| p.is_file()) {
        return Some(p.clone());
    }
    let e = model::entry(s.model.as_deref()?)?;
    let dir = data.join("models");
    model::have(&dir, e).then(|| model::file_in(&dir, e))
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

/// Load the model at path into the engine, unless it is the one there.
fn ensure_engine<'a>(slot: &'a mut Option<Engine>, path: &Path) -> Result<&'a mut Engine, String> {
    if slot.as_ref().map(|e| e.path != path).unwrap_or(true) {
        *slot = None;
        let model = pktito::Model::load(path)?;
        let asr = pktito::Asr::new(&model, 0)?;
        *slot = Some(Engine { model, asr, path: path.to_path_buf() });
    }
    Ok(slot.as_mut().unwrap())
}

#[tauri::command]
fn status(app: AppHandle, state: State<'_, AppState>) -> Result<Status, String> {
    let data = data_dir(&app)?;
    let mut s = model::load_settings(&data);
    if s.model_file.as_ref().map(|p| !p.is_file()).unwrap_or(false) {
        // moved or deleted since: the catalog model is in use again, if there is one
        s.model_file = None;
        model::save_settings(&data, &s)?;
    }
    let dir = data.join("models");
    let download = state.download.lock().unwrap().as_ref().map(|j| j.state()).filter(|d| d.step != "done" && d.step != "cancelled");
    Ok(Status {
        ready: usable(&data, &s).is_some(),
        model: s.model,
        model_file: s.model_file,
        catalog: model::CATALOG
            .iter()
            .map(|e| Offer { entry: e, have: model::have(&dir, e), partial: model::partial(&dir, e) })
            .collect(),
        download,
        prefs: s.prefs,
        backend: pktito::backend(),
        platform: std::env::consts::OS,
        media: MEDIA,
        video: VIDEO,
    })
}

#[tauri::command]
fn set_prefs(app: AppHandle, prefs: model::Prefs) -> Result<(), String> {
    let data = data_dir(&app)?;
    let mut s = model::load_settings(&data);
    s.prefs = prefs;
    model::save_settings(&data, &s)
}

/// Use a catalog model that is on this computer.
#[tauri::command]
fn choose_model(app: AppHandle, name: String) -> Result<(), String> {
    let e = model::entry(&name).ok_or("no such model")?;
    let data = data_dir(&app)?;
    if !model::have(&data.join("models"), e) {
        return Err(format!("The {} model is not on this computer yet.", e.label));
    }
    let mut s = model::load_settings(&data);
    s.model = Some(name);
    s.model_file = None;
    model::save_settings(&data, &s)
}

/// Download a catalog model, or go on with its download; once it checks it
/// is the model in use. "download" events say how it goes.
#[tauri::command]
fn download_model(app: AppHandle, state: State<'_, AppState>, name: String) -> Result<(), String> {
    let e = model::entry(&name).ok_or("no such model")?;
    let dir = data_dir(&app)?.join("models");
    let mut slot = state.download.lock().unwrap();
    if let Some(j) = slot.as_ref() {
        if j.alive() && !j.ctl.cancel.load(Ordering::Relaxed) {
            if j.name() == name {
                j.resume_now();
                return Ok(());
            }
            return Err(format!("{} is downloading. Cancel it first.", j.name()));
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
fn cancel_download(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let slot = state.download.lock().unwrap();
    let Some(j) = slot.as_ref() else { return Ok(()) };
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
async fn use_model_file(app: AppHandle, state: State<'_, AppState>) -> Result<bool, String> {
    let Some(file) = file_dialog(&app).add_filter("pktito model", &["pkf"]).blocking_pick_file() else {
        return Ok(false);
    };
    let path = file.into_path().map_err(|e| e.to_string())?;
    {
        let mut slot = state.engine.lock().unwrap();
        ensure_engine(&mut slot, &path).map_err(|why| format!("This is not a model pktito can use: {why}"))?;
    }
    let data = data_dir(&app)?;
    let mut settings = model::load_settings(&data);
    settings.model_file = Some(path);
    model::save_settings(&data, &settings)?;
    Ok(true)
}

/// Files named on the command line (`pktito-app a.mp4 b.mp3`), to queue at start.
#[tauri::command]
fn initial_files() -> Vec<String> {
    std::env::args()
        .skip(1)
        .filter(|a| !a.starts_with('-') && Path::new(a).is_file())
        .map(|a| fs::canonicalize(&a).map(|p| p.to_string_lossy().into_owned()).unwrap_or(a))
        .collect()
}

#[tauri::command]
async fn pick_files(app: AppHandle) -> Result<Vec<String>, String> {
    let files = file_dialog(&app).add_filter("Audio or video", MEDIA).blocking_pick_files();
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
    took: f64,
    model: String,
    /// Its place on the recent list.
    recent: Option<String>,
    message: String,
}

#[tauri::command]
async fn transcribe(app: AppHandle, id: u32, path: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || job(&app, id, Path::new(&path)))
        .await
        .map_err(|e| e.to_string())
}

fn job(app: &AppHandle, id: u32, path: &Path) {
    let state = app.state::<AppState>();
    let send = |ev: JobEvent| {
        let _ = app.emit("job", JobEvent { id, ..ev });
    };
    let fail = |message: String| send(JobEvent { step: "error", message, ..Default::default() });
    // a Stop meant for this file's last run, which failed before it looked
    let _ = state.stop.compare_exchange(id, 0, Ordering::Relaxed, Ordering::Relaxed);
    let t0 = Instant::now();
    send(JobEvent { step: "reading", eta: -1.0, ..Default::default() });
    let x = match decode::decode(path) {
        Ok(x) => x,
        Err(e) => return fail(e.message()),
    };
    let duration = x.len() as f64 / decode::RATE as f64;
    let key = recent::new_id();
    let audio = audio_dir(app)
        .ok()
        .and_then(|d| audio::save(&d, &key, &x, decode::RATE as u32).ok())
        .map(|p| p.to_string_lossy().into_owned());
    let peaks = audio::peaks(&x, audio::PEAKS);
    let data = match data_dir(app) {
        Ok(d) => d,
        Err(why) => return fail(why),
    };
    let Some(model_path) = usable(&data, &model::load_settings(&data)) else {
        return fail("There is no model to transcribe with. Choose one first.".into());
    };
    let mut slot = state.engine.lock().unwrap();
    let loading = slot.as_ref().map(|e| e.path != model_path).unwrap_or(true);
    send(JobEvent {
        step: if loading { "loading" } else { "transcribing" },
        eta: -1.0,
        duration,
        peaks: peaks.clone(),
        audio,
        ..Default::default()
    });
    let Engine { model, asr, .. } = match ensure_engine(&mut slot, &model_path) {
        Ok(e) => e,
        Err(why) => return fail(format!("The model could not be loaded: {why}")),
    };
    if loading {
        send(JobEvent { step: "transcribing", eta: -1.0, duration, ..Default::default() });
    }
    let t1 = Instant::now();
    let r = transcribe::run(
        model,
        asr,
        &x,
        |done, segs| {
            // the pace so far, the model's loading left out
            let spent = t1.elapsed().as_secs_f64();
            let eta = if done < 1.0 { spent / done * (1.0 - done) } else { 0.0 };
            send(JobEvent { step: "transcribing", done, eta, duration, segments: segs.to_vec(), ..Default::default() });
        },
        || state.stop.load(Ordering::Relaxed) == id,
    );
    let name = model.name();
    drop(slot);
    let _ = state.stop.compare_exchange(id, 0, Ordering::Relaxed, Ordering::Relaxed);
    match r {
        Ok((segments, true)) => {
            let took = t0.elapsed().as_secs_f64();
            let item = recent::Item {
                id: key.clone(),
                path: path.to_path_buf(),
                name: path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
                duration,
                finished: recent::now(),
                took,
                model: name.clone(),
            };
            let kept = recent::add(&data, &recent::Transcript { item, peaks, segments });
            if let (Ok(gone), Ok(dir)) = (&kept, audio_dir(app)) {
                for g in gone {
                    let _ = fs::remove_file(audio::wav_in(&dir, g));
                }
                audio::cap(&dir, &recent::list(&data).into_iter().map(|i| i.id).collect::<Vec<_>>());
            }
            send(JobEvent { step: "done", done: 1.0, duration, took, model: name, recent: kept.ok().map(|_| key), ..Default::default() });
        }
        Ok((_, false)) => send(JobEvent { step: "stopped", duration, ..Default::default() }),
        Err(why) => fail(why),
    }
}

/// Stop a transcription after the chunk it is on.
#[tauri::command]
fn stop(state: State<'_, AppState>, id: u32) {
    state.stop.store(id, Ordering::Relaxed);
}

#[tauri::command]
fn recent_list(app: AppHandle) -> Result<Vec<recent::Item>, String> {
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
fn open_recent(app: AppHandle, id: String) -> Result<Opened, String> {
    let transcript = recent::load(&data_dir(&app)?, &id)?;
    let wav = audio::wav_in(&audio_dir(&app)?, &id);
    Ok(Opened { transcript, audio: wav.is_file().then(|| wav.to_string_lossy().into_owned()) })
}

/// A recent transcript's audio, read again from its original once the cache
/// has let it go.
#[tauri::command]
async fn reload_audio(app: AppHandle, id: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let t = recent::load(&data_dir(&app)?, &id)?;
        if !t.item.path.is_file() {
            return Err("Its file is not where it was, so it cannot play.".to_string());
        }
        let x = decode::decode(&t.item.path).map_err(|e| e.message())?;
        // 0.1 s: two of the encoder's frames; a file edited since is longer or shorter by more
        if (x.len() as f64 / decode::RATE as f64 - t.item.duration).abs() > 0.1 {
            return Err("Its file has changed since it was transcribed, so it cannot play.".to_string());
        }
        let p = audio::save(&audio_dir(&app)?, &id, &x, decode::RATE as u32).map_err(|e| e.to_string())?;
        Ok(p.to_string_lossy().into_owned())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Empty the recent list, once the user says so.
#[tauri::command]
async fn clear_recent(app: AppHandle) -> Result<bool, String> {
    let ask = app
        .dialog()
        .message("Its transcripts are deleted from this computer. Files you saved stay where they are.")
        .title("Clear the recent list?")
        .kind(MessageDialogKind::Warning)
        .buttons(MessageDialogButtons::OkCancelCustom("Clear".into(), "Cancel".into()));
    // owned by the window on Windows, as the file dialogs are (file_dialog)
    #[cfg(target_os = "windows")]
    let ask = match app.get_webview_window("main") {
        Some(w) => ask.parent(&w),
        None => ask,
    };
    let yes = ask.blocking_show();
    if yes {
        let data = data_dir(&app)?;
        let dir = audio_dir(&app)?;
        for i in recent::list(&data) {
            let _ = fs::remove_file(audio::wav_in(&dir, &i.id));
        }
        recent::clear(&data);
    }
    Ok(yes)
}

/// Where stem.ext can go in dir without writing over anything: stem.ext,
/// else "stem 2.ext" ("stem (2).ext" on Windows, as Explorer numbers
/// copies), and so on.
fn free_path(dir: &Path, stem: &str, ext: &str) -> PathBuf {
    let first = dir.join(format!("{stem}.{ext}"));
    if !first.exists() {
        return first;
    }
    (2..)
        .map(|n| dir.join(if cfg!(windows) { format!("{stem} ({n}).{ext}") } else { format!("{stem} {n}.{ext}") }))
        .find(|p| !p.exists())
        .unwrap()
}

fn stem_of(original: &Path) -> String {
    original.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "transcript".into())
}

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
async fn save_text(app: AppHandle, original: String, ext: String, text: String, next_to: bool) -> Result<Option<Saved>, String> {
    let original = Path::new(&original);
    let stem = stem_of(original);
    if next_to {
        if let Some(dir) = original.parent() {
            let p = free_path(dir, &stem, &ext);
            // a folder it cannot write to, such as a disc's, asks instead
            if fs::write(&p, &text).is_ok() {
                return Ok(Some(Saved { path: p.to_string_lossy().into_owned(), beside: true }));
            }
        }
    }
    let kind = match ext.as_str() {
        "srt" | "vtt" => "Subtitles",
        "md" => "Markdown",
        "json" => "JSON",
        _ => "Text",
    };
    let mut dialog = file_dialog(&app).set_file_name(format!("{stem}.{ext}")).add_filter(kind, &[ext.as_str()]);
    if let Some(dir) = original.parent().filter(|d| d.is_dir()) {
        dialog = dialog.set_directory(dir);
    }
    let Some(file) = dialog.blocking_save_file() else {
        return Ok(None);
    };
    let path = file.into_path().map_err(|e| e.to_string())?;
    fs::write(&path, text).map_err(|e| format!("It could not be saved: {e}"))?;
    Ok(Some(Saved { path: path.to_string_lossy().into_owned(), beside: false }))
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
async fn save_many(dir: Option<String>, files: Vec<Export>) -> Result<Vec<String>, String> {
    let mut written = Vec::new();
    for f in files {
        let original = Path::new(&f.original);
        let d = match &dir {
            Some(d) => PathBuf::from(d),
            None => original.parent().map(Path::to_path_buf).unwrap_or_default(),
        };
        let p = free_path(&d, &stem_of(original), &f.ext);
        fs::write(&p, &f.text).map_err(|e| format!("{} could not be saved: {e}", p.display()))?;
        written.push(p.to_string_lossy().into_owned());
    }
    Ok(written)
}

#[tauri::command]
async fn pick_folder(app: AppHandle) -> Result<Option<String>, String> {
    let Some(dir) = file_dialog(&app).blocking_pick_folder() else {
        return Ok(None);
    };
    Ok(Some(dir.into_path().map_err(|e| e.to_string())?.to_string_lossy().into_owned()))
}

/// Show a file in the file manager, selected.
#[tauri::command]
fn reveal(path: String) -> Result<(), String> {
    reveal_in_folder(Path::new(&path))
}

#[cfg(target_os = "macos")]
fn reveal_in_folder(path: &Path) -> Result<(), String> {
    std::process::Command::new("open").arg("-R").arg(path).spawn().map(|_| ()).map_err(|e| e.to_string())
}

/// The shell's own way. `explorer /select,"path"` started from the app
/// opened nothing in the Windows test (2026-10-21, three toasts): the shell
/// call is what Explorer itself answers. Given one item's full ID list and no
/// others, it opens the item's folder with the item selected.
#[cfg(target_os = "windows")]
fn reveal_in_folder(path: &Path) -> Result<(), String> {
    use std::ffi::c_void;
    use std::os::windows::ffi::OsStrExt;
    #[link(name = "shell32")]
    extern "system" {
        fn ILCreateFromPathW(path: *const u16) -> *mut c_void;
        fn ILFree(list: *mut c_void);
        fn SHOpenFolderAndSelectItems(folder: *const c_void, n: u32, items: *const *const c_void, flags: u32) -> i32;
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
        let hr = if list.is_null() { -1 } else { SHOpenFolderAndSelectItems(list, 0, std::ptr::null(), 0) };
        if !list.is_null() {
            ILFree(list);
        }
        if com >= 0 {
            CoUninitialize();
        }
        if hr < 0 {
            return Err(format!("Explorer could not show {} (0x{:08x}).", path.display(), hr as u32));
        }
    }
    Ok(())
}

/// The file manager's own D-Bus call where there is one (GNOME's Files,
/// KDE's Dolphin), which selects it; else its folder, opened.
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn reveal_in_folder(path: &Path) -> Result<(), String> {
    use std::process::{Command, Stdio};
    let mut uri = String::from("file://");
    for b in path.to_string_lossy().bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b'-' | b'_' | b'.' | b'~' => uri.push(b as char),
            _ => uri.push_str(&format!("%{b:02X}")),
        }
    }
    let shown = Command::new("gdbus")
        .args(["call", "--session", "--dest", "org.freedesktop.FileManager1", "--object-path", "/org/freedesktop/FileManager1"])
        .args(["--method", "org.freedesktop.FileManager1.ShowItems", &format!("['{uri}']"), ""])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if shown {
        return Ok(());
    }
    Command::new("xdg-open").arg(path.parent().unwrap_or(Path::new("/"))).spawn().map(|_| ()).map_err(|e| e.to_string())
}

/// `pktito-app --transcribe FILE [--model PKF] [--json]`: the window's
/// pipeline without the window, printing each segment, or with --json the
/// segments and their words as the window gets them; for checking a build.
fn headless(args: &[String]) -> Result<(), String> {
    let file = args.get(1).ok_or("--transcribe FILE [--model PKF] [--json]")?;
    let json = args.iter().any(|a| a == "--json");
    let model_path = match args.iter().position(|a| a == "--model") {
        Some(i) => PathBuf::from(args.get(i + 1).ok_or("--model needs a path")?),
        None => std::env::var("PKTITO_MODEL").map(PathBuf::from).map_err(|_| "--model PKF, or PKTITO_MODEL")?,
    };
    let t0 = Instant::now();
    let x = decode::decode(Path::new(file)).map_err(|e| e.message())?;
    let t1 = Instant::now();
    let model = pktito::Model::load(&model_path)?;
    let mut asr = pktito::Asr::new(&model, 0)?;
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
        "{} segments, {:.1} s of audio: decoded in {:.3} s, transcribed in {:.3} s ({}, {})",
        segs.len(),
        x.len() as f64 / decode::RATE as f64,
        (t1 - t0).as_secs_f64(),
        t1.elapsed().as_secs_f64(),
        model.name(),
        pktito::backend()
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
            "failed" => eprintln!("\n{}: {}{}", s.why, s.detail.replace('\n', "; "), if s.retry { " (trying again)" } else { "" }),
            _ => eprint!("\r{} {} of {} MB   ", s.step, s.got / 1_000_000, s.total / 1_000_000),
        },
        move |p| {
            let _ = tx.send(p);
        },
    );
    while job.alive() {
        std::thread::sleep(Duration::from_millis(100));
    }
    rx.try_recv().map_err(|_| job.state().why)
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
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .setup(|app| {
            // the audio no transcript on the recent list needs, from runs
            // before; before the window opens, so none of this run's goes too
            if let (Ok(data), Ok(dir)) = (data_dir(app.handle()), audio_dir(app.handle())) {
                audio::prune(&dir, &recent::list(&data).into_iter().map(|i| i.id).collect::<Vec<_>>());
            }
            let item = |id: &str, text: &str, key: Option<&str>| {
                let b = MenuItemBuilder::with_id(id, text);
                match key {
                    Some(k) => b.accelerator(k).build(app),
                    None => b.build(app),
                }
            };
            let file = SubmenuBuilder::new(app, "File")
                .item(&item("add", "Add Files…", Some("CmdOrCtrl+O"))?)
                .separator()
                .item(&item("save-txt", "Save as Text", Some("CmdOrCtrl+S"))?)
                .item(&item("save-srt", "Save as Subtitles", Some("CmdOrCtrl+Shift+S"))?)
                .item(&item("save-vtt", "Save as Web Subtitles", None)?)
                .item(&item("save-md", "Save as Markdown", None)?)
                .item(&item("save-json", "Save as JSON", None)?)
                .separator()
                .item(&PredefinedMenuItem::close_window(app, None)?)
                .build()?;
            let edit = SubmenuBuilder::new(app, "Edit")
                .item(&PredefinedMenuItem::undo(app, None)?)
                .item(&PredefinedMenuItem::redo(app, None)?)
                .separator()
                .item(&PredefinedMenuItem::cut(app, None)?)
                .item(&PredefinedMenuItem::copy(app, None)?)
                .item(&PredefinedMenuItem::paste(app, None)?)
                .item(&PredefinedMenuItem::select_all(app, None)?)
                .separator()
                .item(&item("find", "Find…", Some("CmdOrCtrl+F"))?)
                .build()?;
            let menu = MenuBuilder::new(app);
            #[cfg(target_os = "macos")]
            let menu = menu.item(
                &SubmenuBuilder::new(app, "pktito")
                    .item(&PredefinedMenuItem::about(app, None, None)?)
                    .separator()
                    .item(&PredefinedMenuItem::hide(app, None)?)
                    .item(&PredefinedMenuItem::hide_others(app, None)?)
                    .separator()
                    .item(&PredefinedMenuItem::quit(app, None)?)
                    .build()?,
            );
            let menu = menu.item(&file).item(&edit).build()?;
            app.set_menu(menu)?;
            app.on_menu_event(|app, event| {
                let _ = app.emit("menu", event.id().0.as_str());
            });
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
            initial_files,
            pick_files,
            transcribe,
            stop,
            recent_list,
            open_recent,
            reload_audio,
            clear_recent,
            save_text,
            save_many,
            pick_folder,
            reveal
        ])
        .run(tauri::generate_context!())
        .expect("pktito could not start");
}
