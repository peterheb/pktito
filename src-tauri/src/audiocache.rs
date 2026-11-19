//! A transcript's recording as pktito heard it, 16 kHz mono, kept in the
//! cache bit for bit, so that finding its speakers again, or transcribing it
//! again, reads it back instead of decoding the original once more: Android's
//! decoder took 20 to 32 s over a 39-minute podcast on a Galaxy S25 Ultra
//! (0.9.8 vc7, android-9950x, 2026-11-19), where its entry is a 150 MB file
//! to read.
//!
//! The samples are the decoder's own f32 bits, not audio.rs's 16-bit WAV for
//! the player: a transcript from them is the one a fresh decode gives, byte
//! for byte (audiocache's round_trip_is_exact). Each entry names the
//! recording it came from - where it was, its size and its modification time
//! - and a recording that has changed since is a miss, its entry let go.

use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

/// Most the cache keeps, its oldest transcripts' let go first: 16 kHz f32 is
/// 230 MB an hour, so about four and a half hours; an 87-minute recording is
/// 334 MB. A phone's storage holds its photos too, and Android may empty the
/// cache when it runs short.
const CACHE_BYTES: u64 = 1 << 30;

/// The first bytes of an entry, and its layout's version.
const MAGIC: &[u8; 8] = b"pkf32au\0";
const VERSION: u32 = 1;

/// The recording an entry was made from: where it was (a path, or the
/// content URI Android named it by), its size in bytes, and its modification
/// time as the caller reads it (any unit, so long as it is always the same).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Source {
    pub what: String,
    pub size: u64,
    pub modified: i64,
}

/// Where the cache keeps a transcript's samples.
pub fn path_in(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("{id}.f32"))
}

/// The samples kept for transcript id, if they were made from `src` as it is
/// now and are whole; None otherwise, and an entry for another recording, or
/// one that does not read, is let go.
pub fn load(dir: &Path, id: &str, src: &Source) -> Option<Vec<f32>> {
    let p = path_in(dir, id);
    let f = File::open(&p).ok()?;
    let len = f.metadata().ok()?.len();
    match read(BufReader::new(f), len, src) {
        Ok(Some(x)) => Some(x),
        Ok(None) | Err(_) => {
            let _ = fs::remove_file(&p);
            None
        }
    }
}

/// None when the entry is not src's, or is not as long as it says.
fn read(mut r: impl Read, len: u64, src: &Source) -> io::Result<Option<Vec<f32>>> {
    let mut magic = [0u8; 8];
    r.read_exact(&mut magic)?;
    if &magic != MAGIC
        || u32_of(&mut r)? != VERSION
        || u32_of(&mut r)? != crate::decode::RATE as u32
    {
        return Ok(None);
    }
    let size = u64_of(&mut r)?;
    let modified = u64_of(&mut r)? as i64;
    let what_len = u32_of(&mut r)? as u64;
    if what_len > len {
        return Ok(None);
    }
    let mut what = vec![0u8; what_len as usize];
    r.read_exact(&mut what)?;
    if size != src.size || modified != src.modified || what != src.what.as_bytes() {
        return Ok(None);
    }
    let n = u64_of(&mut r)?;
    let head = 8 + 4 + 4 + 8 + 8 + 4 + what_len + 8;
    if n.checked_mul(4).and_then(|b| b.checked_add(head)) != Some(len) {
        return Ok(None);
    }
    let mut b = vec![0u8; (n * 4) as usize];
    r.read_exact(&mut b)?;
    Ok(Some(
        b.as_chunks::<4>()
            .0
            .iter()
            .map(|&c| f32::from_le_bytes(c))
            .collect(),
    ))
}

fn u32_of(r: &mut impl Read) -> io::Result<u32> {
    let mut b = [0u8; 4];
    r.read_exact(&mut b)?;
    Ok(u32::from_le_bytes(b))
}

fn u64_of(r: &mut impl Read) -> io::Result<u64> {
    let mut b = [0u8; 8];
    r.read_exact(&mut b)?;
    Ok(u64::from_le_bytes(b))
}

/// x, transcript id's samples, made from src: written under another name and
/// renamed into place, so that a crash part way leaves no entry, only a
/// .part that the next prune lets go. Only a recording read to its end is
/// saved, never one a Stop or a failure cut short.
pub fn save(dir: &Path, id: &str, src: &Source, x: &[f32]) -> io::Result<PathBuf> {
    fs::create_dir_all(dir)?;
    let dst = path_in(dir, id);
    let tmp = dir.join(format!("{id}.f32.part"));
    let mut w = BufWriter::new(File::create(&tmp)?);
    w.write_all(MAGIC)?;
    w.write_all(&VERSION.to_le_bytes())?;
    w.write_all(&(crate::decode::RATE as u32).to_le_bytes())?;
    w.write_all(&src.size.to_le_bytes())?;
    w.write_all(&(src.modified as u64).to_le_bytes())?;
    w.write_all(&(src.what.len() as u32).to_le_bytes())?;
    w.write_all(src.what.as_bytes())?;
    w.write_all(&(x.len() as u64).to_le_bytes())?;
    let mut buf = Vec::with_capacity(1 << 17);
    for chunk in x.chunks(1 << 15) {
        buf.clear();
        for &v in chunk {
            buf.extend_from_slice(&v.to_le_bytes());
        }
        w.write_all(&buf)?;
    }
    w.into_inner().map_err(|e| e.into_error())?.sync_all()?;
    fs::rename(&tmp, &dst)?;
    Ok(dst)
}

/// Lets go of transcript id's samples: the transcript is deleted.
pub fn remove(dir: &Path, id: &str) {
    let _ = fs::remove_file(path_in(dir, id));
}

/// At the start of a run: let go of the samples of every transcript not in
/// `keep` (the recent list, newest first), and of any half-written entry,
/// then of the oldest kept ones while the cache holds more than CACHE_BYTES.
pub fn prune(dir: &Path, keep: &[String]) {
    prune_to(dir, keep, CACHE_BYTES)
}

fn prune_to(dir: &Path, keep: &[String], most: u64) {
    let Ok(list) = fs::read_dir(dir) else { return };
    let wanted: HashSet<&str> = keep.iter().map(|s| s.as_str()).collect();
    for f in list.flatten() {
        let name = f.file_name().to_string_lossy().into_owned();
        let id = name.strip_suffix(".f32").unwrap_or("");
        if !wanted.contains(id) {
            let _ = fs::remove_file(f.path());
        }
    }
    let mut total = 0u64;
    for id in keep {
        let p = path_in(dir, id);
        let Ok(m) = fs::metadata(&p) else { continue };
        total += m.len();
        if total > most {
            let _ = fs::remove_file(p);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> PathBuf {
        let d =
            std::env::temp_dir().join(format!("pkapp-audiocache-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn src() -> Source {
        Source {
            what: "content://media/external/audio/media/1000045337".into(),
            size: 29_910_000,
            modified: 1_763_500_000_123,
        }
    }

    /// Samples of every kind f32 has - signed zeros, subnormals, values past
    /// full scale, NaN with a payload - each back with the same bits.
    fn signal() -> Vec<f32> {
        let mut x: Vec<f32> = (0..100_003)
            .map(|i| ((i as f32) * 0.001).sin() * 0.7)
            .collect();
        x.extend([
            0.0,
            -0.0,
            f32::MIN_POSITIVE / 3.0,
            -1.5,
            1.0000001,
            f32::from_bits(0x7fc0_1234),
            f32::INFINITY,
        ]);
        x
    }

    #[test]
    fn round_trip_is_exact() {
        let d = dir("exact");
        let x = signal();
        save(&d, "t1", &src(), &x).unwrap();
        let y = load(&d, "t1", &src()).unwrap();
        assert_eq!(x.len(), y.len());
        assert!(x.iter().zip(&y).all(|(a, b)| a.to_bits() == b.to_bits()));
        assert!(!d.join("t1.f32.part").exists());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_changed_recording_misses() {
        let d = dir("changed");
        save(&d, "t1", &src(), &signal()).unwrap();
        for other in [
            Source {
                size: src().size + 1,
                ..src()
            },
            Source {
                modified: src().modified + 1,
                ..src()
            },
            Source {
                what: "content://other".into(),
                ..src()
            },
        ] {
            assert!(load(&d, "t1", &other).is_none());
            // and the stale entry is gone
            assert!(!path_in(&d, "t1").exists());
            save(&d, "t1", &src(), &signal()).unwrap();
        }
        assert!(load(&d, "t1", &src()).is_some());
        assert!(load(&d, "t2", &src()).is_none());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_cut_entry_misses() {
        let d = dir("cut");
        save(&d, "t1", &src(), &signal()).unwrap();
        let p = path_in(&d, "t1");
        let len = fs::metadata(&p).unwrap().len();
        let f = fs::OpenOptions::new().write(true).open(&p).unwrap();
        f.set_len(len - 4).unwrap();
        drop(f);
        assert!(load(&d, "t1", &src()).is_none());
        fs::write(&p, b"pkf32au").unwrap();
        assert!(load(&d, "t1", &src()).is_none());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn prune_keeps_the_newest_under_the_cap() {
        let d = dir("prune");
        let x = vec![0.25f32; 1000]; // about 4 kB an entry
        for id in ["a", "b", "c", "gone"] {
            save(&d, id, &src(), &x).unwrap();
        }
        fs::write(d.join("a.f32.part"), b"half").unwrap();
        let one = fs::metadata(path_in(&d, "a")).unwrap().len();
        // newest first: a, b, c; room for two
        prune_to(&d, &["a".into(), "b".into(), "c".into()], 2 * one);
        assert!(path_in(&d, "a").exists() && path_in(&d, "b").exists());
        assert!(!path_in(&d, "c").exists() && !path_in(&d, "gone").exists());
        assert!(!d.join("a.f32.part").exists());
        remove(&d, "a");
        assert!(!path_in(&d, "a").exists());
        let _ = fs::remove_dir_all(&d);
    }
}
