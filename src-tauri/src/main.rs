//! pktito's desktop app: drop audio or video files, read their transcripts.
//! The audio comes out through the OS's media libraries (or ffmpeg, see
//! decode.rs) and is transcribed on this computer by pktito.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod decode;
mod model;
mod pktito;
mod transcribe;

use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::menu::{MenuBuilder, MenuItemBuilder, PredefinedMenuItem, SubmenuBuilder};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;

/// The model and a transcriber over it, loaded once and kept.
struct Engine {
    model: pktito::Model,
    asr: pktito::Asr,
    path: PathBuf,
}

#[derive(Default)]
struct AppState {
    engine: Mutex<Option<Engine>>,
}

const MEDIA: &[&str] = &[
    "mp3", "mp4", "m4a", "m4v", "mov", "wav", "aac", "aif", "aiff", "caf", "flac", "ogg", "opus",
    "webm", "mkv", "wma", "wmv", "avi", "3gp", "amr",
];
const VIDEO: &[&str] = &["mp4", "m4v", "mov", "webm", "mkv", "wmv", "avi", "3gp"];

fn data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path().app_data_dir().map_err(|e| e.to_string())
}

#[derive(Serialize)]
struct ModelInfo {
    name: String,
    path: String,
}

#[derive(Serialize)]
struct Status {
    model: Option<ModelInfo>,
    catalog: &'static [model::Entry],
    backend: String,
    media: &'static [&'static str],
    video: &'static [&'static str],
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
fn status(app: AppHandle, state: State<AppState>) -> Result<Status, String> {
    let settings = model::load_settings(&data_dir(&app)?);
    let mut model_info = None;
    if let Some(p) = settings.model_path.filter(|p| p.is_file()) {
        let mut slot = state.engine.lock().unwrap();
        if let Ok(e) = ensure_engine(&mut slot, &p) {
            model_info = Some(ModelInfo { name: e.model.name(), path: p.to_string_lossy().into_owned() });
        }
    }
    Ok(Status { model: model_info, catalog: model::CATALOG, backend: pktito::backend(), media: MEDIA, video: VIDEO })
}

#[derive(Clone, Serialize)]
struct DownloadProgress {
    name: String,
    got: u64,
    total: u64,
}

#[tauri::command]
async fn download_model(app: AppHandle, name: String) -> Result<(), String> {
    let entry = model::entry(&name).ok_or("no such model")?;
    let data = data_dir(&app)?;
    let app2 = app.clone();
    let path = tauri::async_runtime::spawn_blocking(move || {
        model::download(entry, &data.join("models"), |got, total| {
            let _ = app2.emit("model-progress", DownloadProgress { name: entry.name.into(), got, total });
        })
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|why| format!("The model could not be downloaded: {why}. Check the connection and try again."))?;
    let data = data_dir(&app)?;
    let mut settings = model::load_settings(&data);
    settings.model_path = Some(path);
    model::save_settings(&data, &settings)
}

/// A .pkf already on this computer, checked by loading it.
#[tauri::command]
async fn use_model_file(app: AppHandle, state: State<'_, AppState>) -> Result<bool, String> {
    let Some(file) = app.dialog().file().add_filter("pktito model", &["pkf"]).blocking_pick_file() else {
        return Ok(false);
    };
    let path = file.into_path().map_err(|e| e.to_string())?;
    {
        let mut slot = state.engine.lock().unwrap();
        ensure_engine(&mut slot, &path).map_err(|why| format!("This is not a model pktito can use: {why}"))?;
    }
    let data = data_dir(&app)?;
    let mut settings = model::load_settings(&data);
    settings.model_path = Some(path);
    model::save_settings(&data, &settings)?;
    Ok(true)
}

/// Files named on the command line (`pktito-app a.mp4 b.mp3`), to queue at start.
#[tauri::command]
fn initial_files() -> Vec<String> {
    std::env::args()
        .skip(1)
        .filter(|a| !a.starts_with('-') && Path::new(a).is_file())
        .map(|a| std::fs::canonicalize(&a).map(|p| p.to_string_lossy().into_owned()).unwrap_or(a))
        .collect()
}

#[tauri::command]
async fn pick_files(app: AppHandle) -> Result<Vec<String>, String> {
    let files = app.dialog().file().add_filter("Audio or video", MEDIA).blocking_pick_files();
    Ok(files
        .unwrap_or_default()
        .into_iter()
        .filter_map(|f| f.into_path().ok())
        .map(|p| p.to_string_lossy().into_owned())
        .collect())
}

#[derive(Clone, Serialize)]
struct JobEvent {
    id: u32,
    step: &'static str,
    pct: u32,
    duration: f64,
    segments: Vec<transcribe::Segment>,
    message: String,
}

#[tauri::command]
async fn transcribe(app: AppHandle, id: u32, path: String) -> Result<(), String> {
    let app2 = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let emit = |step: &'static str, pct: u32, duration: f64, segments: Vec<transcribe::Segment>, message: String| {
            let _ = app2.emit("job", JobEvent { id, step, pct, duration, segments, message });
        };
        let state = app2.state::<AppState>();
        let settings = model::load_settings(&data_dir(&app2)?);
        let model_path = settings.model_path.ok_or("no model chosen yet")?;
        emit("extracting", 0, 0.0, vec![], String::new());
        let x = match decode::decode(Path::new(&path)) {
            Ok(x) => x,
            Err(e) => {
                emit("error", 0, 0.0, vec![], e.message());
                return Ok(());
            }
        };
        let duration = x.len() as f64 / decode::RATE as f64;
        emit("transcribing", 0, duration, vec![], String::new());
        let mut slot = state.engine.lock().unwrap();
        let engine = match ensure_engine(&mut slot, &model_path) {
            Ok(e) => e,
            Err(why) => {
                emit("error", 0, duration, vec![], why);
                return Ok(());
            }
        };
        let Engine { model, asr, .. } = engine;
        let result = transcribe::run(model, asr, &x, |frac, segs| {
            emit("transcribing", (frac * 100.0).floor() as u32, duration, segs.to_vec(), String::new());
        });
        match result {
            Ok(_) => emit("done", 100, duration, vec![], String::new()),
            Err(why) => emit("error", 0, duration, vec![], why),
        }
        Ok::<(), String>(())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Save text where the user says, as name.ext; the path, or None if cancelled.
#[tauri::command]
async fn save_text(app: AppHandle, name: String, ext: String, text: String) -> Result<Option<String>, String> {
    let filter = if ext == "srt" { "Subtitles" } else { "Text" };
    let Some(file) = app
        .dialog()
        .file()
        .set_file_name(format!("{name}.{ext}"))
        .add_filter(filter, &[ext.as_str()])
        .blocking_save_file()
    else {
        return Ok(None);
    };
    let path = file.into_path().map_err(|e| e.to_string())?;
    std::fs::write(&path, text).map_err(|e| format!("It could not be saved: {e}"))?;
    Ok(Some(path.to_string_lossy().into_owned()))
}

/// `pktito-app --transcribe FILE [--model PKF]`: the window's pipeline
/// without the window, printing each segment; for checking a build.
fn headless(args: &[String]) -> Result<(), String> {
    let file = args.get(1).ok_or("--transcribe FILE [--model PKF]")?;
    let model_path = match args.iter().position(|a| a == "--model") {
        Some(i) => PathBuf::from(args.get(i + 1).ok_or("--model needs a path")?),
        None => std::env::var("PKTITO_MODEL").map(PathBuf::from).map_err(|_| "--model PKF, or PKTITO_MODEL")?,
    };
    let t0 = std::time::Instant::now();
    let x = decode::decode(Path::new(file)).map_err(|e| e.message())?;
    let t1 = std::time::Instant::now();
    let model = pktito::Model::load(&model_path)?;
    let mut asr = pktito::Asr::new(&model, 0)?;
    let segs = transcribe::run(&model, &mut asr, &x, |_, segs| {
        for s in segs {
            println!("{:>10.2}  {}", s.start, s.text);
        }
    })?;
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

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(|a| a == "--download").unwrap_or(false) {
        // `--download NAME DIR`: the first-run download alone, checked as the window checks it
        let r = (|| -> Result<PathBuf, String> {
            let e = model::entry(args.get(1).ok_or("--download NAME DIR")?).ok_or("no such model")?;
            let dir = PathBuf::from(args.get(2).ok_or("--download NAME DIR")?);
            model::download(e, &dir, |got, total| eprint!("\r{} of {} MB", got / 1_000_000, total / 1_000_000))
        })();
        match r {
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
            let add = MenuItemBuilder::with_id("add", "Add files…").accelerator("CmdOrCtrl+O").build(app)?;
            let txt = MenuItemBuilder::with_id("save-txt", "Save as .txt…").accelerator("CmdOrCtrl+S").build(app)?;
            let srt = MenuItemBuilder::with_id("save-srt", "Save as .srt…").accelerator("CmdOrCtrl+Shift+S").build(app)?;
            let file = SubmenuBuilder::new(app, "File")
                .item(&add)
                .separator()
                .item(&txt)
                .item(&srt)
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
        .invoke_handler(tauri::generate_handler![status, download_model, use_model_file, initial_files, pick_files, transcribe, save_text])
        .run(tauri::generate_context!())
        .expect("pktito could not start");
}
