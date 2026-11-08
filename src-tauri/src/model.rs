//! The models: which ones there are, where they live, and their download
//! from models.pktito.net, checked against the published size and SHA-256
//! before it is used. A download that stops (paused, or the connection
//! lost) keeps what it has, and the next one asks only for the rest.
//! PKTITO_MODEL_BASE points the download somewhere else (a mirror, or a
//! local server while testing). Which of a model's editions runs here, and
//! what Manage models offers, is editions.rs's.

use crate::editions::{self, Device, Edition, Refusals, Runtime};
use crate::strings::Problem;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use std::time::Duration;

pub const DEFAULT_BASE: &str = "https://models.pktito.net/";

/// Whether this system is a Mac that runs a model's Core ML edition on the
/// Neural Engine: one on Core ML's floor (COREML) or later, macOS 15, at 3x
/// the CPU's speed for v2 on an M4 Pro with 0.9.5's encoder (AliceBench,
/// 349x to 117x, WER 0.059% apart, 2026-11-02); 0.9.6's, with the position
/// term folded, takes 16% less time on an M4 (AliceBench v2, 354x to 423x,
/// 6 words of 27,118 different; the ane fork's tools/ane/CLIMB.md,
/// 2026-11-03). The choice itself is editions::choose's; this says where a
/// user's own copy of a catalog model may borrow its encoder (encoder_of),
/// and where pktito-cli downloads and loads one.
pub fn neural() -> bool {
    cfg!(target_os = "macos")
        && CATALOG
            .iter()
            .filter_map(Entry::coreml)
            .any(|ed| editions::fit(ed, Device::this()) > 0)
}

/// This PC's NPU as QNN describes it (pktito::qnn_chip), on Windows on Arm,
/// asked once: QNN loads the NPU's library to say. None where it finds none
/// (a Snapdragon 7c, whose Hexagon QNN refuses), where the app's folder has
/// no QNN DLLs, and on every other system (Device::this).
pub fn chip() -> Option<&'static str> {
    #[cfg(all(windows, target_arch = "aarch64"))]
    {
        static CHIP: OnceLock<Option<String>> = OnceLock::new();
        CHIP.get_or_init(|| {
            crate::pktito::qnn_chip()
                .map_err(|why| eprintln!("pktito: no NPU: {why}"))
                .ok()
        })
        .as_deref()
    }
    #[cfg(not(all(windows, target_arch = "aarch64")))]
    None
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct Entry {
    pub name: &'static str,
    /// Its name, and what it transcribes in a line, for pktito-cli; the
    /// window's are ui/locales' catalog.KEY and catalog.KEY.about (strings.rs).
    pub key: &'static str,
    pub label: &'static str,
    pub about: &'static str,
    /// What it transcribes, as ISO 639-1 codes: the window offers the first
    /// model that speaks the system's language.
    pub languages: &'static [&'static str],
    /// Where its error rate in each language is, for a model of several:
    /// the window links to it under their list; "" for none.
    pub accuracy: &'static str,
    /// The ways to run it (editions::Edition): the CPU's, its whole .pkf,
    /// first, and one for each NPU it has an encoder for.
    pub editions: &'static [Edition],
}

impl Entry {
    /// The CPU's edition: every model has one (every_model_has_one_cpu_edition).
    pub fn cpu(&self) -> &'static Edition {
        self.editions
            .iter()
            .find(|ed| !ed.npu())
            .expect("a catalog model without a CPU edition")
    }

    /// The whole .pkf, which runs on any CPU.
    pub fn whole(&self) -> &'static Part {
        &self.cpu().model
    }

    /// The Core ML edition, the Neural Engine's, where it has one.
    pub fn coreml(&self) -> Option<&'static Edition> {
        self.editions
            .iter()
            .find(|ed| matches!(ed.runtime, Runtime::CoreMl { .. }))
    }

    /// The slim .pkf the NPU editions load, where it has one.
    pub fn slim(&self) -> Option<&'static Part> {
        self.editions.iter().find(|ed| ed.npu()).map(|ed| &ed.model)
    }
}

/// A file models.pktito.net serves, under the first twelve digits of its
/// SHA-256 (part_url): file is its name with its extension.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct Part {
    pub file: &'static str,
    pub bytes: u64,
    pub sha256: &'static str,
}

// The Core ML editions come from pktito's ane fork: the model file without
// its encoder (`pktito-tools slim`), which only the fork loads (its qnn
// branch too, Android's), and the encoder as a compiled Core ML model,
// int8, in an Apple Archive (tools/ane/encoder.py, then `aa archive`), made
// from the same checkpoint, whose SHA-256 each names and the fork holds
// them to. For v2 559 MB in all where its .pkf is 624; the same transcript
// as the .pkf with that encoder, which runs at 271x on an M1 iPad
// (AliceBench v2, against 0.9.5's 242x; the fork's tools/ane/CLIMB.md,
// 2026-11-03).
//
// The QNN editions' graphs come from pktito's qnn branch: compiled by
// Qualcomm's QNN on a PC from the whole .pkf for one chip (core/qnn.h,
// PKTITO_QNN_TARGET), and read with the slim file, the same file as the
// Core ML edition's. No phone compiles one: the Android app ships no
// compiler. For v2 727 MB with the slim file where its .pkf is 624. With
// the PC's graph the Galaxy S24 Ultra (SM8650) ran AliceBench at 167x and
// the Galaxy S25 Ultra (SM8750) at 206x, each phone's test.wav to every
// digit what its own compile of the graph wrote; with graph 1 the
// Googlebook (X1E80100) wrote the slim file's transcript word for word
// what the whole one wrote with the same graph (the qnn branch's
// tools/qnn/CLIMB.md, 2026-11-03). On Windows on Arm a graph ran a chapter
// at 155x to the CPU's 115x on a Snapdragon X Elite with 1/22 the CPU time
// (snappy, ch01, 2026-11-05).

/// Core ML's floors for the encoder: macOS 15 and iOS 18, whose Core ML
/// first has what core/ane.m asks of it (MLModelAsset's modelAssetWithURL:
/// and functionNamesWithCompletionHandler:, MLModelConfiguration's
/// functionName; the SDK's CoreML headers). A Mac on macOS 12 to 14, above
/// the app's floor, takes the CPU's edition.
const COREML: Runtime = Runtime::CoreMl {
    macos: (15, 0),
    ios: (18, 0),
};

/// A model's CPU edition, its whole .pkf.
const fn cpu(model: Part) -> Edition {
    Edition {
        id: "cpu",
        runtime: Runtime::Cpu,
        model,
        encoder: None,
    }
}

/// A model's QNN edition for one chip (Runtime::Qnn), its graph read with
/// the slim file.
const fn qnn(
    id: &'static str,
    chip: &'static str,
    systems: &'static [&'static str],
    slim: Part,
    graph: Part,
) -> Edition {
    Edition {
        id,
        runtime: Runtime::Qnn { chip, systems },
        model: slim,
        encoder: Some(graph),
    }
}

// the chips, in QNN's words (pk_qnn_chip): the SoC's number in QNN's table,
// its Hexagon's version, its VTCM in MB, and whether it has DLBC and a
// signed PD. A Snapdragon X's QNN on Windows names no SoC
// (2147483647,73,8,1,1 on snappy's X1E, 2026-11-05), and takes the
// X1E80100's graph by its Hexagon, as an X2 the SC8480XP's.
/// the Galaxy S24's Snapdragon 8 Gen 3
const SM8650: &str = "57,75,8,1,0";
/// the Galaxy S25's Snapdragon 8 Elite
const SM8750: &str = "69,79,8,1,0";
/// the Galaxy S26's Snapdragon 8 Elite Gen 5
const SM8850: &str = "87,81,8,1,0";
/// the Snapdragon X Elite, the Googlebook's on Android and snappy's on Windows
const X1E80100: &str = "60,73,8,1,1";
/// the Snapdragon X2 Elite (SoC 88, V81, 8 MB of VTCM), for Windows
const SC8480XP: &str = "88,81,8,1,1";
const ANDROID: &[&str] = &["android"];
const WINDOWS: &[&str] = &["windows"];
const BOTH: &[&str] = &["android", "windows"];

const V2_SLIM: Part = Part {
    file: "parakeet-tdt-0.6b-v2-slim.pkf",
    bytes: 12_115_524,
    sha256: "e582eb76db19e9fba4d9e15d45ce041dd9a230bac3c2970643aa6808bf966acb",
};

const V3_SLIM: Part = Part {
    file: "parakeet-tdt-0.6b-v3-slim.pkf",
    bytes: 35_187_972,
    sha256: "169ead8b6b98c67676d13c40bd592614f9d1a3b3783da027fe03967cd559b100",
};

/// The models models.pktito.net serves: pktito-tools' conversions of NVIDIA's
/// checkpoints (CC-BY-4.0), as pktito 22fa0b1 writes them. What each one
/// does is from NVIDIA's model cards.
pub const CATALOG: &[Entry] = &[
    Entry {
        name: "parakeet-tdt-0.6b-v2",
        key: "english",
        label: "English",
        about: "English only",
        languages: &["en"],
        accuracy: "",
        editions: &[
            cpu(Part {
                file: "parakeet-tdt-0.6b-v2.pkf",
                bytes: 624_480_196,
                sha256: "8bfa6750b83cd7c447bad82057c7a12473563b72b3d35487fcca6da0786d83c7",
            }),
            Edition {
                id: "coreml",
                runtime: COREML,
                model: V2_SLIM,
                encoder: Some(Part {
                    file: "parakeet-tdt-0.6b-v2-encoder.mlmodelc.aar",
                    bytes: 546_910_291,
                    sha256: "70272da7508aff518225caeaea3f0dc7dedb5bf0cc829da72876c9ecae5159a1",
                }),
            },
            // pktito-qnn-graph 2, compiled by pktito 88f5a6d on the 9950X with
            // QAIRT 2.51.0.260929 (2026-11-03; the S26's 2026-11-04)
            qnn(
                "qnn-sm8650",
                SM8650,
                ANDROID,
                V2_SLIM,
                Part {
                    file: "parakeet-tdt-0.6b-v2-sm8650.qnn",
                    bytes: 714_506_856,
                    sha256: "3f690d2cdd0ad6dc3b805f627f5c8e0b7701af20135bc275ebcc8f55877263a1",
                },
            ),
            qnn(
                "qnn-sm8750",
                SM8750,
                ANDROID,
                V2_SLIM,
                Part {
                    file: "parakeet-tdt-0.6b-v2-sm8750.qnn",
                    bytes: 713_568_872,
                    sha256: "40a5b75f37954b528f04944c57096ba0215783cea23b50ef659a981b5995456c",
                },
            ),
            qnn(
                "qnn-sm8850",
                SM8850,
                ANDROID,
                V2_SLIM,
                Part {
                    file: "parakeet-tdt-0.6b-v2-sm8850.qnn",
                    bytes: 714_412_648,
                    sha256: "08388c7f4b9cbb562330111ed6e0b026d00512168cee589b04578f50673b1aff",
                },
            ),
            qnn(
                "qnn-x1e80100",
                X1E80100,
                BOTH,
                V2_SLIM,
                Part {
                    file: "parakeet-tdt-0.6b-v2-x1e80100.qnn",
                    bytes: 714_523_240,
                    sha256: "dab2d4d69028caba976c0b82316f83ad49bd3d4bac01b39992cd66904d216513",
                },
            ),
            // compiled by pktito bbdcf60 on 2026-11-05, run on no X2 yet (the
            // qnn branch's tools/qnn/CLIMB.md)
            qnn(
                "qnn-sc8480xp",
                SC8480XP,
                WINDOWS,
                V2_SLIM,
                Part {
                    file: "parakeet-tdt-0.6b-v2-sc8480xp.qnn",
                    bytes: 714_445_416,
                    sha256: "726db8ecde7934510ab0193f52de93601c64506edf1804a240612a823e90d811",
                },
            ),
        ],
    },
    Entry {
        name: "parakeet-tdt-0.6b-v3",
        key: "multilingual",
        label: "Multilingual",
        about: "25 European languages",
        languages: &[
            "bg", "cs", "da", "de", "el", "en", "es", "et", "fi", "fr", "hr", "hu", "it", "lt",
            "lv", "mt", "nl", "pl", "pt", "ro", "ru", "sk", "sl", "sv", "uk",
        ],
        // the model card's table of each language's word error rate
        accuracy: "https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3#multilingual-asr",
        editions: &[
            cpu(Part {
                file: "parakeet-tdt-0.6b-v3.pkf",
                bytes: 647_552_644,
                sha256: "35a59c9a433d4da2289640db93e73c42478b576e49a293090ef61b4ae595152a",
            }),
            Edition {
                id: "coreml",
                runtime: COREML,
                model: V3_SLIM,
                encoder: Some(Part {
                    file: "parakeet-tdt-0.6b-v3-encoder.mlmodelc.aar",
                    bytes: 545_275_075,
                    sha256: "a7eef40e28b009fee25548fe869124d2b9ba7be8c00c2dcb1b2e8e8973524571",
                }),
            },
            qnn(
                "qnn-sm8650",
                SM8650,
                ANDROID,
                V3_SLIM,
                Part {
                    file: "parakeet-tdt-0.6b-v3-sm8650.qnn",
                    bytes: 714_506_856,
                    sha256: "84aae2af7273132997bb71af73550f1a9a037de35b3d45d9e95a9390a796324e",
                },
            ),
            qnn(
                "qnn-sm8750",
                SM8750,
                ANDROID,
                V3_SLIM,
                Part {
                    file: "parakeet-tdt-0.6b-v3-sm8750.qnn",
                    bytes: 713_568_872,
                    sha256: "d5cf8b7659ea176f66937a94e72c80bb11caac60d18811b0f500b6b1eaa73113",
                },
            ),
            qnn(
                "qnn-sm8850",
                SM8850,
                ANDROID,
                V3_SLIM,
                Part {
                    file: "parakeet-tdt-0.6b-v3-sm8850.qnn",
                    bytes: 714_412_648,
                    sha256: "68eb09b667c7f9f6f36bb7eb7812945078150d8ff9984c61ce169d1bdfe99eab",
                },
            ),
            qnn(
                "qnn-x1e80100",
                X1E80100,
                BOTH,
                V3_SLIM,
                Part {
                    file: "parakeet-tdt-0.6b-v3-x1e80100.qnn",
                    bytes: 714_523_240,
                    sha256: "1208ddc1b2c63e68568d692780297d142ad5f57ee3bb05dd86218347edd7809f",
                },
            ),
            qnn(
                "qnn-sc8480xp",
                SC8480XP,
                WINDOWS,
                V3_SLIM,
                Part {
                    file: "parakeet-tdt-0.6b-v3-sc8480xp.qnn",
                    bytes: 714_445_416,
                    sha256: "237a5a6c51abfcb969310d494f70b2663c568a43174b70b9fd3f6d624c6caa7a",
                },
            ),
        ],
    },
];

pub fn entry(name: &str) -> Option<&'static Entry> {
    CATALOG.iter().find(|e| e.name == name)
}

/// tauri.conf.json's identifier, which names the app's data folder
pub const IDENTIFIER: &str = "net.pktito.app";

/// The app's data folder, its settings, models and recent list: where Tauri
/// puts it for the window (app_data_dir: the system's data folder, then the
/// identifier), for pktito-cli, which has no Tauri to ask.
pub fn data_dir() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join(IDENTIFIER))
}

/// path, a model file the engine has loaded, the one in use: the catalog
/// model whose own file it is, however the path to it is written, else a
/// file of the user's
pub fn choose_file(data: &Path, path: &Path) -> Result<(), String> {
    let mut s = load_settings(data);
    let dir = data.join("models");
    match CATALOG.iter().find(|e| same_file(&file_in(&dir, e), path)) {
        Some(e) => {
            (s.model, s.model_file, s.model_file_is) = (Some(e.name.to_string()), None, None)
        }
        None => {
            s.model_file = Some(path.to_path_buf());
            // a copy of a catalog model, downloaded by hand: on a Mac its
            // Core ML encoder serves it too (encoder_of), and Manage models
            // offers it (Peter, 2026-11-02)
            s.model_file_is = if neural() {
                copy_of(path).map(|e| e.name.to_string())
            } else {
                None
            };
        }
    }
    save_settings(data, &s)
}

/// The catalog model the file at path is, byte for byte: its size, then its
/// SHA-256 (a second or two for 624 MB, once, when it is chosen).
pub fn copy_of(path: &Path) -> Option<&'static Entry> {
    let size = fs::metadata(path).ok()?.len();
    let e = CATALOG.iter().find(|e| e.whole().bytes == size)?;
    let mut file = File::open(path).ok()?;
    (sha256(&mut file, &Ctl::default()).ok()? == e.whole().sha256).then_some(e)
}

/// What e runs on in dir on this computer (editions::runs).
pub fn runs(dir: &Path, e: &Entry) -> Option<editions::Runs> {
    editions::runs(dir, e, Device::this(), &Refusals::read(dir))
}

/// The NPU's encoder for the model file at path: a catalog model's, where
/// what it runs on (runs) loads that file with one, on a Mac its Core ML
/// encoder and on Windows on Arm this PC's graph; or on a Mac, for a file of
/// the user's own that is a catalog model's copy (Settings::model_file_is),
/// that model's Core ML encoder.
pub fn encoder_of(data: &Path, path: &Path) -> Option<PathBuf> {
    let dir = data.join("models");
    for e in CATALOG {
        if let Some(r) = runs(&dir, e).filter(|r| same_file(&r.model, path)) {
            return r.encoder;
        }
    }
    let s = load_settings(data);
    let own = s.model_file.as_deref().is_some_and(|f| same_file(f, path));
    let e = entry(s.model_file_is.as_deref().filter(|_| neural() && own)?)?;
    have_encoder(&dir, e).then(|| encoder_in(&dir, e))
}

/// A transcription ran with the model file at path on an NPU: where that is
/// a catalog model's own slim file, its whole .pkf goes (editions::ran).
pub fn ran_on_npu(data: &Path, path: &Path) {
    let dir = data.join("models");
    for e in CATALOG {
        if let Some(r) = runs(&dir, e).filter(|r| same_file(&r.model, path)) {
            editions::ran(&dir, e, r.edition, path);
        }
    }
}

/// An NPU refused the encoder at encoder, a catalog model's (why):
/// recorded for this computer, and the encoder deleted (editions::refuse).
pub fn refuse_npu(encoder: &Path, why: &str) {
    let Some(dir) = encoder.parent() else {
        return;
    };
    for e in CATALOG {
        for ed in e.editions {
            if ed
                .encoder
                .as_ref()
                .is_some_and(|p| same_file(&editions::installed(dir, p), encoder))
            {
                editions::refuse(dir, e, ed, Device::this(), why);
            }
        }
    }
}

/// The catalog model whose file path is, whole or slim, in data's models.
pub fn catalog_file(data: &Path, path: &Path) -> Option<&'static Entry> {
    let dir = data.join("models");
    CATALOG
        .iter()
        .find(|e| same_file(&file_in(&dir, e), path) || same_file(&slim_in(&dir, e), path))
}

/// Whether a and b are one file: the same path, or the same once each is
/// canonical (through links and .., and on Windows in its \\?\ form)
pub fn same_file(a: &Path, b: &Path) -> bool {
    a == b || matches!((fs::canonicalize(a), fs::canonicalize(b)), (Ok(x), Ok(y)) if x == y)
}

/// The model file to transcribe with: the user's own, else the catalog
/// model in use, if it is on this computer.
pub fn usable(data: &Path, s: &Settings) -> Option<PathBuf> {
    if let Some(p) = s.model_file.as_ref().filter(|p| p.is_file()) {
        return Some(p.clone());
    }
    let e = entry(s.model.as_deref()?)?;
    file_of(&data.join("models"), e)
}

/// e's model file in dir that runs here (runs), if one does.
pub fn file_of(dir: &Path, e: &Entry) -> Option<PathBuf> {
    runs(dir, e).map(|r| r.model)
}

/// Whether e is here to transcribe with, in any edition (file_of).
pub fn have_any(dir: &Path, e: &Entry) -> bool {
    file_of(dir, e).is_some()
}

/// The files Manage models' offer for e fetches (editions::offer), in
/// order, each to where it lands (editions::landing); none where nothing is
/// offered.
pub fn wanted(dir: &Path, e: &Entry) -> Vec<Wanted> {
    editions::offer(dir, e, Device::this(), &Refusals::read(dir))
        .map(|o| o.files.iter().map(|p| Wanted::part(dir, e, p)).collect())
        .unwrap_or_default()
}

/// What a download of e that stopped kept, deleted: every file of e's that
/// what runs (runs) doesn't use, whole or in part, and of the ones it uses
/// a replacement's part, or one that checked and waits (editions::landing).
pub fn remove_parts(dir: &Path, e: &Entry) -> Result<(), String> {
    let now = runs(dir, e);
    let used = |p: &Part| {
        now.as_ref().is_some_and(|r| {
            r.model_part.file == p.file || r.edition.encoder.is_some_and(|q| q.file == p.file)
        })
    };
    for p in editions::parts(e) {
        if !used(p) {
            editions::remove_part(dir, p);
            continue;
        }
        let w = Wanted::part(dir, e, p);
        for f in [w.part, w.dst].iter().filter(|f| **f != dir.join(p.file)) {
            match fs::remove_file(f) {
                Err(err) if err.kind() != io::ErrorKind::NotFound => {
                    return Err(format!("{err}\n{}", f.display()))
                }
                _ => {}
            }
            unmark(f);
        }
    }
    Ok(())
}

/// The Core ML encoder beside the catalog model whose file path is (whole or
/// slim), where it has one.
pub fn encoder_for(dir: &Path, path: &Path) -> Option<PathBuf> {
    if !neural() {
        return None;
    }
    let e = CATALOG
        .iter()
        .find(|e| same_file(&file_in(dir, e), path) || same_file(&slim_in(dir, e), path))?;
    have_encoder(dir, e).then(|| encoder_in(dir, e))
}

/// The Core ML encoder out of its Apple Archive (`aa archive` of the
/// compiled model) into dst, by Apple Archive's own library (apple_archive),
/// on the Mac and on iOS alike: unpacked beside it first and renamed into
/// place whole, so that an encoder in place is a whole one (have_encoder),
/// and the archive deleted after. Only an archive whose SHA-256 checked
/// comes here (editions::settle).
pub fn unpack_encoder(archive: &Path, dst: &Path) -> Result<(), String> {
    let mut work = dst.as_os_str().to_owned();
    work.push(".unpacking");
    let work = PathBuf::from(work);
    let _ = fs::remove_dir_all(&work);
    fs::create_dir_all(&work).map_err(|err| format!("couldn't make {}: {err}", work.display()))?;
    let fail = |why: String| {
        let _ = fs::remove_dir_all(&work);
        Err(why)
    };
    if let Err(why) = apple_archive::extract(archive, &work) {
        return fail(why);
    }
    if !work.join("model.mil").is_file() {
        return fail(format!("{} holds no Core ML model", archive.display()));
    }
    // the encoder it replaces aside until the new one is in place, so that
    // one cut short between the two is put right (editions::recover)
    let mut old = dst.as_os_str().to_owned();
    old.push(".old");
    let old = PathBuf::from(old);
    let _ = fs::remove_dir_all(&old);
    if dst.exists() {
        fs::rename(dst, &old)
            .map_err(|err| format!("couldn't move {} aside: {err}", dst.display()))?;
    }
    fs::rename(&work, dst)
        .map_err(|err| format!("couldn't put {} in place: {err}", dst.display()))?;
    let _ = fs::remove_file(marker_of(dst));
    let _ = fs::rename(marker_of(archive), marker_of(dst));
    let _ = fs::remove_file(archive);
    let _ = fs::remove_dir_all(&old);
    Ok(())
}

/// Apple Archive's library (libAppleArchive, from macOS 11 and iOS 14),
/// which macOS's aa is a front for: one way to unpack an encoder on the Mac
/// and on iOS, which has no aa to run, so the Mac's tests cover the iPhone's.
#[cfg(any(target_os = "macos", target_os = "ios"))]
mod apple_archive {
    use std::ffi::{c_char, c_int, c_void, CString};
    use std::os::unix::ffi::OsStrExt;
    use std::path::Path;

    /// An AAByteStream or AAArchiveStream
    type Stream = *mut c_void;
    /// AA_FLAG_IGNORE_EPERM: an attribute the app may not set, such as an
    /// owner, left as it is rather than failing the whole
    const IGNORE_EPERM: u64 = 1 << 0;
    const O_RDONLY: c_int = 0;

    #[link(name = "AppleArchive")]
    unsafe extern "C" {
        fn AAFileStreamOpenWithPath(path: *const c_char, flags: c_int, mode: u16) -> Stream;
        fn AADecompressionInputStreamOpen(compressed: Stream, flags: u64, threads: c_int)
            -> Stream;
        fn AADecodeArchiveInputStreamOpen(
            stream: Stream,
            msg_data: *mut c_void,
            msg_proc: *const c_void,
            flags: u64,
            threads: c_int,
        ) -> Stream;
        fn AAExtractArchiveOutputStreamOpen(
            dir: *const c_char,
            msg_data: *mut c_void,
            msg_proc: *const c_void,
            flags: u64,
            threads: c_int,
        ) -> Stream;
        fn AAArchiveStreamProcess(
            from: Stream,
            to: Stream,
            msg_data: *mut c_void,
            msg_proc: *const c_void,
            flags: u64,
            threads: c_int,
        ) -> isize;
        fn AAArchiveStreamClose(s: Stream) -> c_int;
        fn AAByteStreamClose(s: Stream) -> c_int;
    }

    /// archive's entries into dir, which is there: aa extract -i archive -d dir.
    pub fn extract(archive: &Path, dir: &Path) -> Result<(), String> {
        let c = |p: &Path| {
            CString::new(p.as_os_str().as_bytes())
                .map_err(|_| format!("{} has a NUL in its name", p.display()))
        };
        let (from, to) = (c(archive)?, c(dir)?);
        let none = std::ptr::null_mut();
        let no_proc = std::ptr::null();
        // SAFETY: each stream reads the one opened before it and is closed
        // before it, each once; a NULL stream is none, which closes as
        // nothing; threads 0 is the library's choice
        let (done, closed) = unsafe {
            let file = AAFileStreamOpenWithPath(from.as_ptr(), O_RDONLY, 0);
            if file.is_null() {
                return Err(format!("couldn't open {}", archive.display()));
            }
            let inflate = AADecompressionInputStreamOpen(file, 0, 0);
            let decode = match inflate.is_null() {
                true => none,
                false => AADecodeArchiveInputStreamOpen(inflate, none, no_proc, 0, 0),
            };
            let out = match decode.is_null() {
                true => none,
                false => {
                    AAExtractArchiveOutputStreamOpen(to.as_ptr(), none, no_proc, IGNORE_EPERM, 0)
                }
            };
            let done = match out.is_null() {
                true => -1,
                false => AAArchiveStreamProcess(decode, out, none, no_proc, 0, 0),
            };
            // the extraction's last writes come as it closes, which says
            // whether they did (AAExtractArchiveOutputStreamOpen)
            let closed = match out.is_null() {
                true => -1,
                false => AAArchiveStreamClose(out),
            };
            AAArchiveStreamClose(decode);
            AAByteStreamClose(inflate);
            AAByteStreamClose(file);
            (done, closed)
        };
        if done < 0 || closed < 0 {
            return Err(format!("couldn't unpack {}", archive.display()));
        }
        Ok(())
    }
}

/// Elsewhere no edition has a Core ML encoder (editions::fit), so none
/// comes to be unpacked.
#[cfg(not(any(target_os = "macos", target_os = "ios")))]
mod apple_archive {
    use std::path::Path;

    pub fn extract(archive: &Path, _dir: &Path) -> Result<(), String> {
        Err(format!(
            "{}: a Core ML encoder unpacks only on Apple's systems",
            archive.display()
        ))
    }
}

/// What the app remembers between runs: the catalog model in use, or a
/// model file of the user's own, used instead when set; how it saves; and
/// the window's size as the user last left it, in logical pixels.
#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub model: Option<String>,
    pub model_file: Option<PathBuf>,
    /// The catalog model model_file is a copy of, where it is one (copy_of).
    pub model_file_is: Option<String>,
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
    /// Show a finished transcript as its subtitle track's cues, not its sentences.
    pub captions: bool,
    /// Times in frames, not seconds and hundredths.
    pub frames: bool,
    /// The frame rate an audio file's times are counted in.
    pub fps: f64,
    /// A video's frames counted at its own rate; false counts them at fps too.
    pub video_rate: bool,
    /// Playback's speed, one of the window's 0.75 to 2.
    pub speed: f64,
    /// Playback's volume, 0 to 1.
    pub volume: f64,
    /// A wide window's recent list's width in pixels; 0 for its default.
    pub side: f64,
}

impl Default for Prefs {
    fn default() -> Prefs {
        Prefs {
            next_to_original: true,
            times: false,
            captions: false,
            frames: false,
            fps: 24.0,
            video_rate: true,
            speed: 1.0,
            volume: 1.0,
            side: 0.0,
        }
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

/// Where e's Core ML encoder lives once it is unpacked into dir (a
/// directory, as Core ML compiles it), named as the iOS app names it.
pub fn encoder_in(dir: &Path, e: &Entry) -> PathBuf {
    dir.join(format!("{}-encoder.mlmodelc", e.name))
}

/// Where e's model file without its encoder lives (Entry::slim), which
/// transcribes only with an NPU's encoder.
pub fn slim_in(dir: &Path, e: &Entry) -> PathBuf {
    dir.join(e.slim().map_or("", |p| p.file))
}

/// Whether e's Core ML edition can run from dir: its encoder and a model
/// file it loads (editions::here_of).
pub fn have_neural(dir: &Path, e: &Entry) -> bool {
    e.coreml()
        .is_some_and(|ed| editions::here_of(dir, e, ed).is_some())
}

/// Whether e's Core ML encoder is in dir: put in place whole, after it was
/// unpacked, so its model file is enough to tell.
pub fn have_encoder(dir: &Path, e: &Entry) -> bool {
    encoder_in(dir, e).join("model.mil").is_file()
}

/// Where its download gathers until it checks.
pub fn part_in(dir: &Path, e: &Entry) -> PathBuf {
    dir.join(format!("{}.pkf.part", e.name))
}

/// Whether e is downloaded into dir. A download is renamed into place only
/// after its SHA-256 checks, so the size is enough to tell.
pub fn have(dir: &Path, e: &Entry) -> bool {
    Wanted::model(dir, e).here()
}

/// The bytes of e a download that stopped has kept.
pub fn partial(dir: &Path, e: &Entry) -> u64 {
    fs::metadata(part_in(dir, e))
        .map(|m| m.len().min(e.whole().bytes))
        .unwrap_or(0)
}

/// Delete e's downloaded file, to free its room. The catalog model in use,
/// if it was e, becomes another one that is downloaded, or none; the model
/// now in use, when that put another catalog model in its place (a file of
/// the user's own, set, stays in use instead).
pub fn delete(data: &Path, e: &Entry) -> Result<Option<&'static Entry>, String> {
    let dir = data.join("models");
    // every edition's files, the encoders' folders, what downloads of them
    // kept or left waiting, and e's refusals
    for p in editions::parts(e) {
        let at = editions::installed(&dir, p);
        let file = dir.join(p.file);
        editions::remove_part(&dir, p);
        for f in [&at, &file] {
            if f.exists() {
                return Err(format!("couldn't delete it\n{}", f.display()));
            }
        }
    }
    editions::forget(&dir, e);
    let mut s = load_settings(data);
    if s.model.as_deref() != Some(e.name) {
        return Ok(None);
    }
    let next = CATALOG
        .iter()
        .find(|x| x.name != e.name && have_any(&dir, x));
    s.model = next.map(|x| x.name.to_string());
    save_settings(data, &s)?;
    Ok(next.filter(|_| s.model_file.is_none()))
}

/// A file's address names its contents: under the first twelve digits of its
/// SHA-256, so no address ever serves two files. A CDN can keep each one
/// forever, and a model converted again gets a new address instead of a
/// cached copy of the old one that fails the check.
pub fn download_url(e: &Entry) -> String {
    part_url(e.whole())
}

/// The same for a Core ML edition's file, or a graph for an NPU.
pub fn part_url(p: &Part) -> String {
    format!("{}{}/{}", base(), &p.sha256[..12], p.file)
}

/// A file to download from models.pktito.net into a folder: one of an
/// edition's (editions::Edition), a .pkf, a Core ML encoder's archive or a
/// graph. It gathers in part, and is renamed to dst once its size and
/// SHA-256 are the published ones: in place, or beside the file it replaces
/// until that is let go (editions::landing).
#[derive(Clone)]
pub struct Wanted {
    /// what a download of it is called: its model's name
    pub name: String,
    pub url: String,
    pub bytes: u64,
    pub sha256: &'static str,
    pub part: PathBuf,
    pub dst: PathBuf,
}

impl Wanted {
    /// e's .pkf, the whole model, which runs on any CPU.
    pub fn model(dir: &Path, e: &Entry) -> Wanted {
        Wanted {
            name: e.name.into(),
            url: download_url(e),
            bytes: e.whole().bytes,
            sha256: e.whole().sha256,
            part: part_in(dir, e),
            dst: file_in(dir, e),
        }
    }

    /// The file of e's the catalog names file, of any of its editions;
    /// None for a name the catalog does not give e, so that a download asks
    /// for no other file (the Android app's, which names the files of the
    /// edition it wants).
    pub fn named(dir: &Path, e: &Entry, file: &str) -> Option<Wanted> {
        editions::parts(e)
            .into_iter()
            .find(|p| p.file == file)
            .map(|p| Wanted::part(dir, e, p))
    }

    /// A file of one of e's editions, into dir under its own name, or beside
    /// the one it replaces (editions::landing).
    pub fn part(dir: &Path, e: &Entry, p: &Part) -> Wanted {
        Wanted {
            name: e.name.into(),
            url: part_url(p),
            bytes: p.bytes,
            sha256: p.sha256,
            part: dir.join(format!("{}.part", p.file)),
            dst: editions::landing(dir, p),
        }
    }

    /// The bytes of it a download that stopped has kept.
    pub fn partial(&self) -> u64 {
        fs::metadata(&self.part)
            .map(|m| m.len().min(self.bytes))
            .unwrap_or(0)
    }

    /// Whether it is here, checked: renamed into place only after its
    /// SHA-256 checked, so the size is enough to tell.
    pub fn here(&self) -> bool {
        fs::metadata(&self.dst)
            .map(|m| m.len() == self.bytes)
            .unwrap_or(false)
    }
}

fn base() -> String {
    let base = std::env::var("PKTITO_MODEL_BASE").unwrap_or_else(|_| DEFAULT_BASE.to_string());
    if base.ends_with('/') {
        base
    } else {
        base + "/"
    }
}

/// The program user_agent names: the window's "pktito" (Android's too)
/// unless another of the app's programs says it is the one (pktito-cli).
static CLIENT: OnceLock<&'static str> = OnceLock::new();

/// This program's name in its downloads from now on.
pub fn set_client(name: &'static str) {
    let _ = CLIENT.set(name);
}

/// What a download says it is (User-Agent), "pktito/0.9.4 (Windows; arm64)":
/// the program, its version and the system, for the CDN's logs to count the
/// app's downloads by, where ureq's own ("ureq/2.12.1") said only that some
/// client asked. Nothing in it tells one computer or person from another.
pub fn user_agent() -> String {
    let os = match std::env::consts::OS {
        "macos" => "macOS",
        "windows" => "Windows",
        "linux" => "Linux",
        "android" => "Android",
        other => other,
    };
    let arch = match std::env::consts::ARCH {
        "aarch64" => "arm64",
        other => other,
    };
    let client = CLIENT.get().copied().unwrap_or("pktito");
    format!("{client}/{} ({os}; {arch})", env!("CARGO_PKG_VERSION"))
}

/// The server's name in an address, for saying which one cannot be reached.
pub fn host(url: &str) -> &str {
    let rest = url.split_once("://").map(|(_, r)| r).unwrap_or(url);
    rest.split('/').next().unwrap_or(rest)
}

/// The proxy the environment names for url, read as curl reads it:
/// HTTPS_PROXY (HTTP_PROXY for an http address), else ALL_PROXY, in either
/// case, and none for a host NO_PROXY lists. ureq reads the environment
/// only when asked, ALL_PROXY first and NO_PROXY never; unasked, a sandbox
/// whose only way out is its proxy saw every try at models.pktito.net fail
/// in TLS at an address that was not the server, while curl got the file
/// through the proxy (2026-10-24). A SOCKS ALL_PROXY needs ureq's
/// socks-proxy feature (Cargo.toml).
fn proxy_for(url: &str) -> Option<ureq::Proxy> {
    let var = |name: &str| {
        [name.to_string(), name.to_lowercase()]
            .iter()
            .find_map(|n| std::env::var(n).ok().filter(|v| !v.trim().is_empty()))
    };
    let first = if url.starts_with("http://") {
        "HTTP_PROXY"
    } else {
        "HTTPS_PROXY"
    };
    let p = var(first).or_else(|| var("ALL_PROXY"))?;
    let name = host(url)
        .rsplit_once(':')
        .map_or(host(url), |(h, _)| h)
        .to_lowercase();
    let skip = var("NO_PROXY").is_some_and(|list| {
        list.split(',')
            .map(|s| s.trim().trim_start_matches('.').to_lowercase())
            .any(|s| s == "*" || (!s.is_empty() && (name == s || name.ends_with(&format!(".{s}")))))
    });
    if skip {
        return None;
    }
    ureq::Proxy::new(p.trim()).ok()
}

/// What a model file pktito could not load means to its user, as the
/// catalog's key (strings.rs), from the engine's reason (core/model.c's
/// pk_model_load, os.c's pk_map_file): a newer pktito's model, or one for
/// another architecture, wants a newer app; a file that is not one at all
/// is said so; one that is, but cut short or damaged, is told apart from
/// it, as a download that stopped is. The window words it, and the
/// Android app (android.rs, trouble).
pub fn trouble(why: &str) -> &'static str {
    let has = |words: &[&str]| words.iter().any(|w| why.contains(w));
    if has(&["out of memory", "not enough memory"]) {
        "problem.memory"
    } else if has(&["the NPU can't run"]) {
        "model_file.npu"
    } else if has(&["Neural Engine can't run"]) {
        "model_file.neural_engine"
    } else if has(&["model file version", "the architecture is not"]) {
        "model_file.newer"
    } else if has(&[
        "not a pktito model file",
        "shorter than a header",
        "empty file",
        "no model bytes",
        "larger than any model file",
    ]) {
        "model_file.not_model"
    } else if has(&[
        "cannot map",
        "mmap:",
        "cannot tell its size",
        "No such file",
        "not found",
        "Permission denied",
        "denied",
    ]) {
        "model_file.cant_open"
    } else {
        "model_file.damaged"
    }
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
    /// What happened, and the request it happened to as its particulars.
    Net(Problem),
    /// What trying again will not mend by itself.
    Hard(Problem),
}

/// Download e into dir, from where a download that stopped left off,
/// reporting (bytes so far, whether they are all there and being checked)
/// as it goes; the finished file's path, once its size and SHA-256 are the
/// published ones.
pub fn download(
    e: &Entry,
    dir: &Path,
    ctl: &Ctl,
    progress: impl FnMut(u64, bool),
) -> Result<PathBuf, Stop> {
    download_file(&Wanted::model(dir, e), ctl, progress)
}

/// download() for any file models.pktito.net serves.
pub fn download_file(
    w: &Wanted,
    ctl: &Ctl,
    mut progress: impl FnMut(u64, bool),
) -> Result<PathBuf, Stop> {
    let part = w.part.clone();
    let dir = part.parent().unwrap_or(Path::new("."));
    let write_err = |err: io::Error| written(&part, err);
    fs::create_dir_all(dir).map_err(write_err)?;
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&part)
        .map_err(write_err)?;
    let mut got = file.metadata().map_err(write_err)?.len();
    if got > w.bytes {
        got = 0;
    }
    if got < w.bytes {
        got = match fetch(w, &part, &mut file, got, ctl, &mut progress) {
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
    if digest != w.sha256 {
        let _ = fs::remove_file(&part);
        return Err(Stop::Hard(
            Problem::new("download.damaged")
                .detail(format!("SHA-256 {digest}\nexpected {}", w.sha256)),
        ));
    }
    let dst = w.dst.clone();
    fs::rename(&part, &dst).map_err(|err| written(&dst, err))?;
    mark(&dst, w.sha256);
    Ok(dst)
}

/// A download of w that came another way, at got: the iOS app's, whose
/// transfers are URLSession's so that they go on while the app is away.
/// Checked against its SHA-256 and put where it lands with its record, as
/// fetch puts one; deleted where it doesn't check, or can't be put there.
pub fn land(w: &Wanted, got: &Path) -> Result<PathBuf, Problem> {
    let landed = (|| {
        let mut file = File::open(got)
            .map_err(|err| Stop::Hard(Problem::new("download.cant_check").detail(err)))?;
        let digest = sha256(&mut file, &Ctl::default())?;
        drop(file);
        if digest != w.sha256 {
            return Err(Stop::Hard(
                Problem::new("download.damaged")
                    .detail(format!("SHA-256 {digest}\nexpected {}", w.sha256)),
            ));
        }
        fs::rename(got, &w.dst).map_err(|err| written(&w.dst, err))?;
        mark(&w.dst, w.sha256);
        Ok(w.dst.clone())
    })();
    landed.map_err(|stop| {
        let _ = fs::remove_file(got);
        match stop {
            Stop::Hard(p) | Stop::Net(p) => p,
            // no Ctl here asks for either
            Stop::Paused | Stop::Cancelled => Problem::new("download.cant_check"),
        }
    })
}

/// Where the SHA-256 that a file of the catalog's checked against is kept,
/// beside it (its name with .sha256 after it), and beside a Core ML
/// encoder's folder the archive's it was unpacked from: so that a later
/// app can tell the file it has from a newer one its catalog names, even of
/// the same size, without reading 714 MB again. It says something only
/// beside its file.
pub fn marker_of(path: &Path) -> PathBuf {
    let mut m = path.as_os_str().to_owned();
    m.push(".sha256");
    PathBuf::from(m)
}

/// Record the SHA-256 path checked against (marker_of): kept if it can be,
/// a file without one being only a file whose version a later app must
/// read for itself.
fn mark(path: &Path, sha256: &str) {
    let _ = fs::write(marker_of(path), format!("{sha256}\n"));
}

/// Delete path's record too, where there is one (marker_of).
fn unmark(path: &Path) {
    let _ = fs::remove_file(marker_of(path));
}

/// The rest of w into file, which holds its first `got` bytes; how many it
/// holds once the server has sent them all.
fn fetch(
    w: &Wanted,
    part: &Path,
    file: &mut File,
    mut got: u64,
    ctl: &Ctl,
    progress: &mut impl FnMut(u64, bool),
) -> Result<u64, Stop> {
    let url = w.url.clone();
    let host = host(&url).to_string();
    // a connection that goes quiet is a connection lost: without a read
    // timeout, a network that goes away mid-download leaves it waiting
    let mut agent = ureq::AgentBuilder::new()
        .user_agent(&user_agent())
        .timeout_connect(Duration::from_secs(15))
        .timeout_read(Duration::from_secs(30));
    if let Some(p) = proxy_for(&url) {
        agent = agent.proxy(p);
    }
    let agent = agent.build();
    let asked = got;
    let request = |what: &str| {
        let range = if asked > 0 {
            format!("\nRange: bytes={asked}-")
        } else {
            String::new()
        };
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
            return fetch(w, part, file, 0, ctl, progress);
        }
        Err(ureq::Error::Status(code, r)) => {
            let what = format!("{code} {}", r.status_text());
            return Err(if code >= 500 || code == 429 {
                Stop::Net(
                    Problem::new("download.not_answering")
                        .with("host", host.clone())
                        .detail(request(&what)),
                )
            } else {
                Stop::Hard(Problem::new("download.not_found").detail(request(&what)))
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
            return Err(Stop::Net(
                Problem::new("download.cant_reach")
                    .with("host", host.clone())
                    .detail(request(&what)),
            ));
        }
    };
    // a server that ignores the range sends the whole file: take it whole
    let resumed = resp.status() == 206
        && resp
            .header("Content-Range")
            .map(|r| r.starts_with(&format!("bytes {got}-")))
            .unwrap_or(false);
    if !resumed {
        got = 0;
    }
    file.set_len(got).map_err(|err| written(part, err))?;
    file.seek(SeekFrom::Start(got))
        .map_err(|err| written(part, err))?;
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
                return Err(Stop::Net(
                    Problem::new("download.connection_lost").detail(request(&err.to_string())),
                ))
            }
        };
        if k == 0 {
            break;
        }
        if got + k as u64 > w.bytes {
            // a server that sends more than the file would fill the disk
            file.set_len(0).map_err(|err| written(part, err))?;
            return Err(Stop::Hard(
                Problem::new("download.too_much")
                    .with("host", host.clone())
                    .detail(request(&format!("more than {} bytes", w.bytes))),
            ));
        }
        file.write_all(&buf[..k])
            .map_err(|err| written(part, err))?;
        got += k as u64;
        progress(got, false);
    }
    if got < w.bytes {
        return Err(Stop::Net(Problem::new("download.connection_lost").detail(
            request(&format!("it ended at {got} of {} bytes", w.bytes)),
        )));
    }
    Ok(got)
}

pub(crate) fn sha256(file: &mut File, ctl: &Ctl) -> Result<String, Stop> {
    let read_err = |err: io::Error| Stop::Hard(Problem::new("download.cant_check").detail(err));
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
        Stop::Hard(Problem::new("download.disk_full").detail(detail))
    } else {
        Stop::Hard(Problem::new("download.cant_save").detail(detail))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A data folder of its own under the system's temporary one
    fn scratch(name: &str) -> PathBuf {
        let d =
            std::env::temp_dir().join(format!("pktito-model-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    /// A file whose SHA-256 checked has it beside it, and deleting its model
    /// deletes that too (marker_of)
    #[test]
    fn a_checked_file_keeps_its_sha256_until_deleted() {
        let data = scratch("marker");
        let dir = data.join("models");
        fs::create_dir_all(&dir).unwrap();
        let body = b"not a model, but the bytes it was published as";
        let sha = format!("{:x}", Sha256::digest(body));
        let w = Wanted {
            name: CATALOG[0].name.into(),
            url: String::new(),
            bytes: body.len() as u64,
            sha256: Box::leak(sha.clone().into_boxed_str()),
            part: dir.join("x.pkf.part"),
            dst: file_in(&dir, &CATALOG[0]),
        };
        fs::write(&w.part, body).unwrap();
        // the whole of it is here: no request is made
        assert_eq!(
            download_file(&w, &Ctl::default(), |_, _| {}).ok(),
            Some(w.dst.clone())
        );
        assert_eq!(
            fs::read_to_string(marker_of(&w.dst)).unwrap(),
            format!("{sha}\n")
        );
        delete(&data, &CATALOG[0]).unwrap();
        assert!(!w.dst.exists() && !marker_of(&w.dst).exists());
        let _ = fs::remove_dir_all(&data);
    }

    #[test]
    fn a_catalog_models_own_file_is_that_model() {
        let data = scratch("catalog");
        let e = &CATALOG[1];
        choose_file(&data, &file_in(&data.join("models"), e)).unwrap();
        let s = load_settings(&data);
        assert_eq!((s.model.as_deref(), s.model_file), (Some(e.name), None));
        let _ = fs::remove_dir_all(&data);
    }

    #[test]
    fn a_catalog_models_file_by_another_path_is_that_model() {
        let data = scratch("another-path");
        let e = &CATALOG[0];
        let models = data.join("models");
        fs::create_dir_all(&models).unwrap();
        fs::write(file_in(&models, e), b"its bytes").unwrap();
        choose_file(
            &data,
            &models
                .join("..")
                .join("models")
                .join(format!("{}.pkf", e.name)),
        )
        .unwrap();
        let s = load_settings(&data);
        assert_eq!((s.model.as_deref(), s.model_file), (Some(e.name), None));
        let _ = fs::remove_dir_all(&data);
    }

    #[test]
    fn any_other_file_is_the_users_own() {
        let data = scratch("own");
        let own = data.join("elsewhere").join("tuned.pkf");
        choose_file(&data, &own).unwrap();
        assert_eq!(load_settings(&data).model_file, Some(own));
        let _ = fs::remove_dir_all(&data);
    }

    /// e downloaded into data's models, as have() tells it: a file of its
    /// size, sparse, so that none of it is written
    fn downloaded(data: &Path, e: &Entry) {
        let dir = data.join("models");
        fs::create_dir_all(&dir).unwrap();
        File::create(file_in(&dir, e))
            .unwrap()
            .set_len(e.whole().bytes)
            .unwrap();
        assert!(have(&dir, e));
    }

    #[test]
    fn deleting_the_model_in_use_puts_another_downloaded_one_in_its_place() {
        let data = scratch("delete");
        let (v2, v3) = (&CATALOG[0], &CATALOG[1]);
        downloaded(&data, v2);
        downloaded(&data, v3);
        choose_file(&data, &file_in(&data.join("models"), v2)).unwrap();
        assert_eq!(delete(&data, v2).unwrap().map(|e| e.name), Some(v3.name));
        assert!(!have(&data.join("models"), v2));
        assert_eq!(load_settings(&data).model.as_deref(), Some(v3.name));
        // the last one: none in use, and the window back to choosing one
        assert!(delete(&data, v3).unwrap().is_none());
        let s = load_settings(&data);
        assert_eq!((usable(&data, &s), s.model), (None, None));
        let _ = fs::remove_dir_all(&data);
    }

    #[test]
    fn deleting_a_model_not_in_use_leaves_the_one_in_use() {
        let data = scratch("delete-other");
        let (v2, v3) = (&CATALOG[0], &CATALOG[1]);
        downloaded(&data, v2);
        downloaded(&data, v3);
        choose_file(&data, &file_in(&data.join("models"), v2)).unwrap();
        assert!(delete(&data, v3).unwrap().is_none());
        assert_eq!(load_settings(&data).model.as_deref(), Some(v2.name));
        // a file of the user's own in use: the catalog model under it gives
        // way, and the window has no change to tell
        let own = data.join("tuned.pkf");
        fs::write(&own, b"its bytes").unwrap();
        choose_file(&data, &own).unwrap();
        assert!(delete(&data, v2).unwrap().is_none());
        let s = load_settings(&data);
        assert_eq!((s.model, s.model_file), (None, Some(own)));
        let _ = fs::remove_dir_all(&data);
    }

    #[test]
    fn a_download_says_its_program_version_and_system() {
        // the shape the CDN's logs are read by: "pktito/0.9.4 (Windows; arm64)"
        let ua = user_agent();
        let (program, system) = ua.split_once(' ').unwrap();
        assert_eq!(program, format!("pktito/{}", env!("CARGO_PKG_VERSION")));
        let system = system.strip_prefix('(').and_then(|s| s.strip_suffix(')'));
        let (os, arch) = system.and_then(|s| s.split_once("; ")).unwrap();
        assert!(
            ["macOS", "Windows", "Linux", "Android"].contains(&os),
            "{ua}"
        );
        assert!(["arm64", "x86_64"].contains(&arch), "{ua}");
    }

    #[test]
    fn a_file_that_is_no_model_is_refused() {
        let data = scratch("junk");
        let junk = data.join("not-a-model.pkf");
        fs::write(&junk, b"just some bytes, not a pktito model").unwrap();
        assert!(crate::pktito::Model::load(&junk).is_err());
        let _ = fs::remove_dir_all(&data);
    }

    #[test]
    fn a_download_from_elsewhere_lands_checked_with_its_record() {
        let dir = scratch("land");
        let bytes = b"the bytes the catalog names";
        let sha: String = Sha256::digest(bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        let sha: &'static str = Box::leak(sha.into_boxed_str());
        let w = Wanted {
            name: "test".into(),
            url: String::new(),
            bytes: bytes.len() as u64,
            sha256: sha,
            part: dir.join("m.pkf.part"),
            dst: dir.join("m.pkf"),
        };
        let got = dir.join("m.pkf.download");
        fs::write(&got, bytes).unwrap();
        assert_eq!(land(&w, &got).unwrap(), w.dst);
        assert_eq!(fs::read(&w.dst).unwrap(), bytes);
        assert_eq!(
            fs::read_to_string(marker_of(&w.dst)).unwrap(),
            format!("{sha}\n")
        );
        assert!(!got.exists());
        // one that doesn't check is deleted, and what is in place stays
        fs::write(&got, b"other bytes").unwrap();
        assert_eq!(land(&w, &got).unwrap_err().key, "download.damaged");
        assert!(!got.exists());
        assert_eq!(fs::read(&w.dst).unwrap(), bytes);
        let _ = fs::remove_dir_all(&dir);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn an_encoder_unpacks_by_apple_archive_over_the_one_before() {
        let dir = scratch("unpack");
        // an encoder's shape: model.mil, and its weights in a folder
        let src = dir.join("src");
        fs::create_dir_all(src.join("weights")).unwrap();
        fs::write(src.join("model.mil"), b"program").unwrap();
        fs::write(src.join("weights/weight.bin"), vec![7u8; 1 << 20]).unwrap();
        // archived as models.pktito.net's are, by aa
        let archive = dir.join("e.mlmodelc.aar");
        let made = std::process::Command::new("/usr/bin/aa")
            .args(["archive", "-d"])
            .arg(&src)
            .arg("-o")
            .arg(&archive)
            .status()
            .unwrap();
        assert!(made.success());
        fs::write(marker_of(&archive), "new\n").unwrap();
        // the encoder it replaces
        let dst = dir.join("e.mlmodelc");
        fs::create_dir_all(&dst).unwrap();
        fs::write(dst.join("model.mil"), b"the one before").unwrap();
        fs::write(marker_of(&dst), "old\n").unwrap();
        unpack_encoder(&archive, &dst).unwrap();
        assert_eq!(fs::read(dst.join("model.mil")).unwrap(), b"program");
        assert_eq!(
            fs::read(dst.join("weights/weight.bin")).unwrap(),
            vec![7u8; 1 << 20]
        );
        assert_eq!(fs::read_to_string(marker_of(&dst)).unwrap(), "new\n");
        assert!(!archive.exists() && !marker_of(&archive).exists());
        for left in [".old", ".unpacking"] {
            assert!(!dir.join(format!("e.mlmodelc{left}")).exists());
        }
        // what isn't an archive leaves the encoder in place as it was
        fs::write(&archive, b"not an archive").unwrap();
        assert!(unpack_encoder(&archive, &dst).is_err());
        assert_eq!(fs::read(dst.join("model.mil")).unwrap(), b"program");
        assert!(!dir.join("e.mlmodelc.unpacking").exists());
        let _ = fs::remove_dir_all(&dir);
    }
}
