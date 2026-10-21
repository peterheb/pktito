//! The model: which one, where it lives, and its first-run download from
//! models.pktito.net, checked against the published size and SHA-256 before
//! it is used. PKTITO_MODEL_BASE points the download somewhere else (a
//! mirror, or a local server while testing).

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

pub const DEFAULT_BASE: &str = "https://models.pktito.net/";

#[derive(Clone, Copy, Serialize)]
pub struct Entry {
    pub name: &'static str,
    pub label: &'static str,
    pub bytes: u64,
    pub sha256: &'static str,
}

/// The models models.pktito.net serves: pktito-tools' conversions of NVIDIA's
/// checkpoints (CC-BY-4.0), as pktito e5cc94e writes them.
pub const CATALOG: &[Entry] = &[
    Entry {
        name: "parakeet-tdt-0.6b-v2",
        label: "English",
        bytes: 624_480_196,
        sha256: "8bfa6750b83cd7c447bad82057c7a12473563b72b3d35487fcca6da0786d83c7",
    },
    Entry {
        name: "parakeet-tdt-0.6b-v3",
        label: "25 European languages",
        bytes: 647_552_644,
        sha256: "35a59c9a433d4da2289640db93e73c42478b576e49a293090ef61b4ae595152a",
    },
];

pub fn entry(name: &str) -> Option<&'static Entry> {
    CATALOG.iter().find(|e| e.name == name)
}

/// What the app remembers between runs: the model file it uses.
#[derive(Default, Serialize, Deserialize)]
pub struct Settings {
    pub model_path: Option<PathBuf>,
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

/// A file's address names its contents: under the first twelve digits of its
/// SHA-256, so no address ever serves two files. A CDN can keep each one
/// forever, and a model converted again gets a new address instead of a
/// cached copy of the old one that fails the check.
pub fn download_url(e: &Entry) -> String {
    let base = std::env::var("PKTITO_MODEL_BASE").unwrap_or_else(|_| DEFAULT_BASE.to_string());
    let base = if base.ends_with('/') { base } else { base + "/" };
    format!("{base}{}/{}.pkf", &e.sha256[..12], e.name)
}

/// Download e into dir, reporting (bytes so far, total) as it goes; the
/// finished file's path, once its size and SHA-256 are the published ones.
pub fn download(e: &Entry, dir: &Path, mut progress: impl FnMut(u64, u64)) -> Result<PathBuf, String> {
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let dst = dir.join(format!("{}.pkf", e.name));
    let part = dir.join(format!("{}.pkf.part", e.name));
    let url = download_url(e);
    let result = (|| -> Result<(), String> {
        let resp = ureq::get(&url).call().map_err(|err| match err {
            ureq::Error::Status(code, _) => format!("The model server answered {code} for {url}"),
            other => format!("{other}"),
        })?;
        let mut body = resp.into_reader();
        let mut file = fs::File::create(&part).map_err(|e| e.to_string())?;
        let mut hash = Sha256::new();
        let mut buf = vec![0u8; 1 << 20];
        let (mut got, mut shown) = (0u64, 0u64);
        loop {
            let k = body.read(&mut buf).map_err(|e| e.to_string())?;
            if k == 0 {
                break;
            }
            hash.update(&buf[..k]);
            file.write_all(&buf[..k]).map_err(|e| e.to_string())?;
            got += k as u64;
            if got - shown >= e.bytes / 200 || got == e.bytes {
                progress(got, e.bytes);
                shown = got;
            }
        }
        file.sync_all().map_err(|e| e.to_string())?;
        let digest = hash.finalize().iter().map(|b| format!("{b:02x}")).collect::<String>();
        if got != e.bytes || digest != e.sha256 {
            return Err("the download did not match the published file".into());
        }
        Ok(())
    })();
    match result {
        Ok(()) => {
            fs::rename(&part, &dst).map_err(|e| e.to_string())?;
            Ok(dst)
        }
        Err(why) => {
            let _ = fs::remove_file(&part);
            Err(why)
        }
    }
}
