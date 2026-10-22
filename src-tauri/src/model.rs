//! The models: which ones there are, where they live, and their download
//! from models.pktito.net, checked against the published size and SHA-256
//! before it is used. A download that stops (paused, or the connection
//! lost) keeps what it has, and the next one asks only for the rest.
//! PKTITO_MODEL_BASE points the download somewhere else (a mirror, or a
//! local server while testing).

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

pub const DEFAULT_BASE: &str = "https://models.pktito.net/";

#[derive(Clone, Copy, Serialize)]
pub struct Entry {
    pub name: &'static str,
    /// Its name in the window.
    pub label: &'static str,
    /// What it transcribes, in a line, for the window.
    pub about: &'static str,
    /// What it transcribes, as ISO 639-1 codes: the window offers the first
    /// model that speaks the system's language.
    pub languages: &'static [&'static str],
    pub bytes: u64,
    pub sha256: &'static str,
}

/// The models models.pktito.net serves: pktito-tools' conversions of NVIDIA's
/// checkpoints (CC-BY-4.0), as pktito e5cc94e writes them. What each one
/// does is from NVIDIA's model cards.
pub const CATALOG: &[Entry] = &[
    Entry {
        name: "parakeet-tdt-0.6b-v2",
        label: "English",
        about: "English only",
        languages: &["en"],
        bytes: 624_480_196,
        sha256: "8bfa6750b83cd7c447bad82057c7a12473563b72b3d35487fcca6da0786d83c7",
    },
    Entry {
        name: "parakeet-tdt-0.6b-v3",
        label: "Multilingual",
        about: "25 European languages",
        languages: &[
            "bg", "cs", "da", "de", "el", "en", "es", "et", "fi", "fr", "hr", "hu", "it", "lt", "lv", "mt", "nl", "pl",
            "pt", "ro", "ru", "sk", "sl", "sv", "uk",
        ],
        bytes: 647_552_644,
        sha256: "35a59c9a433d4da2289640db93e73c42478b576e49a293090ef61b4ae595152a",
    },
];

pub fn entry(name: &str) -> Option<&'static Entry> {
    CATALOG.iter().find(|e| e.name == name)
}

/// What the app remembers between runs: the catalog model in use, or a
/// model file of the user's own, used instead when set; how it saves; and
/// the window's size as the user last left it, in logical pixels.
#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub model: Option<String>,
    pub model_file: Option<PathBuf>,
    pub prefs: Prefs,
    pub window: Option<[f64; 2]>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Prefs {
    /// Save beside the file transcribed, with no dialog.
    pub next_to_original: bool,
    /// Each segment's time before its text, in .txt and .md.
    pub times: bool,
    /// Save each file's .txt beside it as soon as it is done.
    pub save_each: bool,
    /// Show a finished transcript as its subtitle track's cues, not its sentences.
    pub captions: bool,
    /// Times in frames, not seconds and hundredths.
    pub frames: bool,
    /// The frame rate an audio file's times are counted in.
    pub fps: f64,
    /// A video's frames counted at its own rate; false counts them at fps too.
    pub video_rate: bool,
}

impl Default for Prefs {
    fn default() -> Prefs {
        Prefs { next_to_original: true, times: false, save_each: false, captions: false, frames: false, fps: 24.0, video_rate: true }
    }
}

pub fn settings_path(data: &Path) -> PathBuf {
    data.join("settings.json")
}

pub fn load_settings(data: &Path) -> Settings {
    fs::read(settings_path(data))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

pub fn save_settings(data: &Path, s: &Settings) -> Result<(), String> {
    fs::create_dir_all(data).map_err(|e| e.to_string())?;
    fs::write(settings_path(data), serde_json::to_vec_pretty(s).unwrap()).map_err(|e| e.to_string())
}

/// Where e lives once it is downloaded into dir.
pub fn file_in(dir: &Path, e: &Entry) -> PathBuf {
    dir.join(format!("{}.pkf", e.name))
}

/// Where its download gathers until it checks.
pub fn part_in(dir: &Path, e: &Entry) -> PathBuf {
    dir.join(format!("{}.pkf.part", e.name))
}

/// Whether e is downloaded into dir. A download is renamed into place only
/// after its SHA-256 checks, so the size is enough to tell.
pub fn have(dir: &Path, e: &Entry) -> bool {
    fs::metadata(file_in(dir, e)).map(|m| m.len() == e.bytes).unwrap_or(false)
}

/// The bytes of e a download that stopped has kept.
pub fn partial(dir: &Path, e: &Entry) -> u64 {
    fs::metadata(part_in(dir, e)).map(|m| m.len().min(e.bytes)).unwrap_or(0)
}

/// A file's address names its contents: under the first twelve digits of its
/// SHA-256, so no address ever serves two files. A CDN can keep each one
/// forever, and a model converted again gets a new address instead of a
/// cached copy of the old one that fails the check.
pub fn download_url(e: &Entry) -> String {
    let base = std::env::var("PKTITO_MODEL_BASE").unwrap_or_else(|_| DEFAULT_BASE.to_string());
    let base = if base.ends_with('/') { base } else { base + "/" };
    format!("{base}{}/{}.pkf", &e.sha256[..12], e.name)
}

/// The server's name in an address, for saying which one cannot be reached.
pub fn host(url: &str) -> &str {
    let rest = url.split_once("://").map(|(_, r)| r).unwrap_or(url);
    rest.split('/').next().unwrap_or(rest)
}

/// What a download is told while it runs, from another thread.
#[derive(Default)]
pub struct Ctl {
    pub pause: AtomicBool,
    pub cancel: AtomicBool,
}

/// Why a download stopped short of a checked file.
pub enum Stop {
    Paused,
    Cancelled,
    /// The network or the server, for now: worth trying again by itself.
    /// What happened, in a line, and the request it happened to.
    Net { why: String, detail: String },
    /// What trying again will not mend by itself: what happened, in a few
    /// words, and the particulars.
    Hard { why: String, detail: String },
}

/// Download e into dir, from where a download that stopped left off,
/// reporting (bytes so far, whether they are all there and being checked)
/// as it goes; the finished file's path, once its size and SHA-256 are the
/// published ones.
pub fn download(e: &Entry, dir: &Path, ctl: &Ctl, mut progress: impl FnMut(u64, bool)) -> Result<PathBuf, Stop> {
    let part = part_in(dir, e);
    let write_err = |err: io::Error| written(&part, err);
    fs::create_dir_all(dir).map_err(write_err)?;
    let mut file = OpenOptions::new().create(true).truncate(false).read(true).write(true).open(&part).map_err(write_err)?;
    let mut got = file.metadata().map_err(write_err)?.len();
    if got > e.bytes {
        got = 0;
    }
    if got < e.bytes {
        got = match fetch(e, &part, &mut file, got, ctl, &mut progress) {
            Ok(got) => got,
            Err(stop) => {
                // nothing kept, nothing to leave behind
                if file.metadata().map(|m| m.len() == 0).unwrap_or(false) {
                    drop(file);
                    let _ = fs::remove_file(&part);
                }
                return Err(stop);
            }
        };
    }
    progress(got, true);
    file.sync_all().map_err(write_err)?;
    let digest = sha256(&mut file, ctl)?;
    drop(file);
    if digest != e.sha256 {
        let _ = fs::remove_file(&part);
        return Err(Stop::Hard {
            why: "Didn't match its checksum".into(),
            detail: format!("SHA-256 {digest}\nexpected {}", e.sha256),
        });
    }
    let dst = file_in(dir, e);
    fs::rename(&part, &dst).map_err(|err| written(&dst, err))?;
    Ok(dst)
}

/// The rest of e into file, which holds its first `got` bytes; how many it
/// holds once the server has sent them all.
fn fetch(
    e: &Entry, part: &Path, file: &mut File, mut got: u64, ctl: &Ctl, progress: &mut impl FnMut(u64, bool),
) -> Result<u64, Stop> {
    let url = download_url(e);
    let host = host(&url).to_string();
    // a connection that goes quiet is a connection lost: without a read
    // timeout, a network that goes away mid-download leaves it waiting
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(15))
        .timeout_read(Duration::from_secs(30))
        .build();
    let asked = got;
    let request = |what: &str| {
        let range = if asked > 0 { format!("\nRange: bytes={asked}-") } else { String::new() };
        format!("{what}\nGET {url}{range}")
    };
    let mut req = agent.get(&url);
    if got > 0 {
        req = req.set("Range", &format!("bytes={got}-"));
    }
    let resp = match req.call() {
        Ok(r) => r,
        // what the part holds is not the start of this file: start again
        Err(ureq::Error::Status(416, _)) if got > 0 => {
            file.set_len(0).map_err(|err| written(part, err))?;
            return fetch(e, part, file, 0, ctl, progress);
        }
        Err(ureq::Error::Status(code, r)) => {
            let what = format!("{code} {}", r.status_text());
            return Err(if code >= 500 || code == 429 {
                Stop::Net { why: format!("{host} isn't answering"), detail: request(&what) }
            } else {
                Stop::Hard { why: "Model not found".into(), detail: request(&what) }
            });
        }
        Err(ureq::Error::Transport(t)) => {
            // what ureq says, less the address it puts first: long, and the
            // same every time
            let mut what = t.kind().to_string();
            if let Some(m) = t.message() {
                what += &format!(": {m}");
            }
            if let Some(s) = std::error::Error::source(&t) {
                what += &format!(": {s}");
            }
            return Err(Stop::Net { why: format!("Can’t reach {host}"), detail: request(&what) });
        }
    };
    // a server that ignores the range sends the whole file: take it whole
    let resumed = resp.status() == 206
        && resp.header("Content-Range").map(|r| r.starts_with(&format!("bytes {got}-"))).unwrap_or(false);
    if !resumed {
        got = 0;
    }
    file.set_len(got).map_err(|err| written(part, err))?;
    file.seek(SeekFrom::Start(got)).map_err(|err| written(part, err))?;
    let mut body = resp.into_reader();
    let mut buf = vec![0u8; 256 << 10];
    loop {
        if ctl.cancel.load(Ordering::Relaxed) {
            return Err(Stop::Cancelled);
        }
        if ctl.pause.load(Ordering::Relaxed) {
            return Err(Stop::Paused);
        }
        let k = match body.read(&mut buf) {
            Ok(k) => k,
            Err(err) if err.kind() == io::ErrorKind::Interrupted => continue,
            Err(err) => {
                return Err(Stop::Net { why: "Connection lost".into(), detail: request(&err.to_string()) })
            }
        };
        if k == 0 {
            break;
        }
        if got + k as u64 > e.bytes {
            // a server that sends more than the file would fill the disk
            file.set_len(0).map_err(|err| written(part, err))?;
            return Err(Stop::Hard {
                why: format!("{host} sent too much"),
                detail: request(&format!("more than {} bytes", e.bytes)),
            });
        }
        file.write_all(&buf[..k]).map_err(|err| written(part, err))?;
        got += k as u64;
        progress(got, false);
    }
    if got < e.bytes {
        return Err(Stop::Net {
            why: "Connection lost".into(),
            detail: request(&format!("it ended at {got} of {} bytes", e.bytes)),
        });
    }
    Ok(got)
}

fn sha256(file: &mut File, ctl: &Ctl) -> Result<String, Stop> {
    let read_err = |err: io::Error| Stop::Hard { why: "Couldn't check the download".into(), detail: err.to_string() };
    file.seek(SeekFrom::Start(0)).map_err(read_err)?;
    let mut hash = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        if ctl.cancel.load(Ordering::Relaxed) {
            return Err(Stop::Cancelled);
        }
        let k = file.read(&mut buf).map_err(read_err)?;
        if k == 0 {
            break;
        }
        hash.update(&buf[..k]);
    }
    Ok(hash.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// A file that could not be written, for the window.
fn written(path: &Path, err: io::Error) -> Stop {
    let detail = format!("{err}\n{}", path.display());
    if err.kind() == io::ErrorKind::StorageFull {
        Stop::Hard { why: "Not enough disk space".into(), detail }
    } else {
        Stop::Hard { why: "Couldn't save the model".into(), detail }
    }
}
