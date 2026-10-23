//! The recent list: each finished transcript, kept in the app's data folder
//! so that the window can open it again, newest first. One file a
//! transcript under recent/, and recent/index.json listing them.

use crate::strings::Problem;
use crate::transcribe::Segment;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// The most the list keeps; the oldest goes first.
const KEEP: usize = 50;

#[derive(Clone, Serialize, Deserialize)]
pub struct Item {
    pub id: String,
    /// The file transcribed.
    pub path: PathBuf,
    pub name: String,
    pub duration: f64,
    /// When it finished, in seconds since 1970.
    pub finished: u64,
    /// Seconds it took, reading the audio included.
    pub took: f64,
    /// Of that, transcribing it (0 for transcripts from before it was kept).
    #[serde(default)]
    pub transcribed: f64,
    pub model: String,
    /// A video's frame rate, 0 for audio (and for transcripts from before).
    #[serde(default)]
    pub fps: f64,
}

#[derive(Serialize, Deserialize)]
pub struct Transcript {
    #[serde(flatten)]
    pub item: Item,
    pub peaks: Vec<u8>,
    pub segments: Vec<Segment>,
}

fn dir(data: &Path) -> PathBuf {
    data.join("recent")
}

/// A name for a new transcript and its audio: the time it started, in
/// milliseconds, so the names sort as the work was done.
pub fn new_id() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
        .to_string()
}

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// An id as new_id makes them, and nothing that could name another file.
fn valid(id: &str) -> bool {
    !id.is_empty() && id.len() <= 24 && id.bytes().all(|b| b.is_ascii_digit())
}

pub fn list(data: &Path) -> Vec<Item> {
    fs::read(dir(data).join("index.json"))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

fn write_list(data: &Path, items: &[Item]) -> Result<(), String> {
    fs::write(
        dir(data).join("index.json"),
        serde_json::to_vec(items).unwrap(),
    )
    .map_err(|e| e.to_string())
}

/// Keep t at the top of the list, in place of any earlier transcript of the
/// same file; the ids of the transcripts let go.
pub fn add(data: &Path, t: &Transcript) -> Result<Vec<String>, String> {
    let d = dir(data);
    fs::create_dir_all(&d).map_err(|e| e.to_string())?;
    fs::write(
        d.join(format!("{}.json", t.item.id)),
        serde_json::to_vec(t).unwrap(),
    )
    .map_err(|e| e.to_string())?;
    let mut items = list(data);
    let mut gone: Vec<String> = Vec::new();
    items.retain(|i| {
        let keep = i.path != t.item.path && i.id != t.item.id;
        if !keep && i.id != t.item.id {
            gone.push(i.id.clone());
        }
        keep
    });
    items.insert(0, t.item.clone());
    gone.extend(items.drain(KEEP.min(items.len())..).map(|i| i.id));
    for id in &gone {
        let _ = fs::remove_file(d.join(format!("{id}.json")));
    }
    write_list(data, &items)?;
    Ok(gone)
}

pub fn load(data: &Path, id: &str) -> Result<Transcript, Problem> {
    if !valid(id) {
        return Err("no such transcript".into());
    }
    let b =
        fs::read(dir(data).join(format!("{id}.json"))).map_err(|_| Problem::new("recent.gone"))?;
    serde_json::from_slice(&b).map_err(|e| Problem::new("recent.unreadable").detail(e))
}

/// One transcript off the list, and its file.
pub fn remove(data: &Path, id: &str) -> Result<(), String> {
    if !valid(id) {
        return Err("no such transcript".into());
    }
    let mut items = list(data);
    items.retain(|i| i.id != id);
    let _ = fs::remove_file(dir(data).join(format!("{id}.json")));
    write_list(data, &items)
}

pub fn clear(data: &Path) {
    let _ = fs::remove_dir_all(dir(data));
}
