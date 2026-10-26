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
    /// The file as it was transcribed: its size in bytes and when it was
    /// last modified, in seconds since 1970, so that a file changed since
    /// is not played against this transcript's times (changed). 0 where it
    /// is not known: a transcript from before these were kept, or a file
    /// whose place does not say.
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub modified: u64,
}

impl Item {
    /// Whether the file now size bytes long, modified when it says, is not
    /// the one transcribed: its size is another, or its time is, where
    /// both sides know them.
    pub fn changed(&self, size: u64, modified: u64) -> bool {
        let differ = |a: u64, b: u64| a != 0 && b != 0 && a != b;
        differ(self.size, size) || differ(self.modified, modified)
    }
}

/// A file's size and its last change, in seconds since 1970, as Item keeps
/// them; 0 for either its file system does not give.
pub fn stamp(path: &Path) -> (u64, u64) {
    let Ok(m) = fs::metadata(path) else {
        return (0, 0);
    };
    let modified = m
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs());
    (m.len(), modified)
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

/// A transcript's file found again where it is now (the app's Locate):
/// path, its size and its time in place of the ones it had.
pub fn relink(data: &Path, id: &str, path: &Path, size: u64, modified: u64) -> Result<(), String> {
    if !valid(id) {
        return Err("no such transcript".into());
    }
    let file = dir(data).join(format!("{id}.json"));
    let mut t: Transcript = fs::read(&file)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .ok_or("no such transcript")?;
    let mut items = list(data);
    let Some(i) = items.iter_mut().find(|i| i.id == id) else {
        return Err("no such transcript".into());
    };
    for item in [i, &mut t.item] {
        item.path = path.to_path_buf();
        item.size = size;
        item.modified = modified;
    }
    fs::write(&file, serde_json::to_vec(&t).unwrap()).map_err(|e| e.to_string())?;
    write_list(data, &items)
}

pub fn clear(data: &Path) {
    let _ = fs::remove_dir_all(dir(data));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str, path: &str) -> Item {
        serde_json::from_value(serde_json::json!({
            "id": id, "path": path, "name": "a.mp3", "duration": 60.0,
            "finished": 1, "took": 2.0, "model": "parakeet-tdt-0.6b-v2",
        }))
        .unwrap()
    }

    #[test]
    fn changed_where_both_know() {
        // a transcript from before these were kept knows neither
        let mut i = item("1", "/a.mp3");
        assert_eq!((i.size, i.modified), (0, 0));
        assert!(!i.changed(1000, 1700000000));
        i.size = 1000;
        i.modified = 1700000000;
        assert!(!i.changed(1000, 1700000000));
        assert!(i.changed(1001, 1700000000));
        assert!(i.changed(1000, 1700000001));
        // a place that does not say its time
        assert!(!i.changed(1000, 0));
    }

    #[test]
    fn relinked() {
        let data = std::env::temp_dir().join(format!("pkapp-recent-{}", std::process::id()));
        let _ = fs::remove_dir_all(&data);
        let t = Transcript {
            item: item("17", "content://a/1"),
            peaks: vec![1, 2],
            segments: Vec::new(),
        };
        add(&data, &t).unwrap();
        relink(&data, "17", Path::new("content://b/2"), 1234, 99).unwrap();
        let i = &list(&data)[0];
        assert_eq!(
            (i.path.to_str().unwrap(), i.size, i.modified),
            ("content://b/2", 1234, 99)
        );
        let t = load(&data, "17").ok().unwrap();
        assert_eq!(
            (t.item.path.to_str().unwrap(), t.item.size, t.peaks),
            ("content://b/2", 1234, vec![1, 2])
        );
        assert!(relink(&data, "18", Path::new("x"), 0, 0).is_err());
        let _ = fs::remove_dir_all(&data);
    }
}
