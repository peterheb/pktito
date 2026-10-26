//! The audio as the window draws and plays it: the outline of its loudness
//! for the waveform, and a 16-bit WAV of exactly the samples pktito heard,
//! kept in the cache so that a transcript plays back in step with its
//! timestamps whatever the original's format, and whether or not the
//! webview can play it.

use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};

/// Slices in a waveform's outline: more than any window draws bars, so the
/// window takes the loudest of the slices under each bar. At 3 px a bar
/// (ui/app.js), 2400 fill a strip 7200 px wide.
pub const PEAKS: usize = 2400;

/// Most the cache keeps of the recent list's audio, its oldest let go
/// first: 16 kHz 16-bit audio is 115 MB an hour, so 18 hours of it. A
/// phone's 512 MB is four and a half: its storage holds its photos too,
/// and Android may empty the cache when it runs short.
#[cfg(not(target_os = "android"))]
const CACHE_BYTES: u64 = 2 << 30;
#[cfg(target_os = "android")]
const CACHE_BYTES: u64 = 512 << 20;

/// The loudness of x in n even slices: each slice's RMS in dB, over the 48
/// dB below the loudest slice, as 0 to 255. Decibels, not amplitude, so
/// that quiet speech still shows beside loud.
pub fn peaks(x: &[f32], n: usize) -> Vec<u8> {
    if x.is_empty() {
        return vec![0; n];
    }
    let db: Vec<f64> = (0..n)
        .map(|i| {
            let a = i * x.len() / n;
            let b = ((i + 1) * x.len() / n).max(a + 1);
            let sum: f64 = x[a..b].iter().map(|&v| v as f64 * v as f64).sum();
            10.0 * (sum / (b - a) as f64 + 1e-12).log10()
        })
        .collect();
    let top = db.iter().cloned().fold(f64::MIN, f64::max);
    db.iter()
        .map(|&d| (((d - (top - 48.0)) / 48.0).clamp(0.0, 1.0) * 255.0).round() as u8)
        .collect()
}

/// Where the cache keeps a transcript's audio.
pub fn wav_in(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("{id}.wav"))
}

/// x, 16 kHz mono, as a 16-bit WAV at dir/id.wav, written under another name
/// and renamed into place, so that a half-written file is never played.
pub fn save(dir: &Path, id: &str, x: &[f32], rate: u32) -> io::Result<PathBuf> {
    fs::create_dir_all(dir)?;
    let dst = wav_in(dir, id);
    let tmp = dir.join(format!("{id}.wav.part"));
    let mut w = BufWriter::new(File::create(&tmp)?);
    let data = (x.len() * 2) as u32;
    w.write_all(b"RIFF")?;
    w.write_all(&(36 + data).to_le_bytes())?;
    w.write_all(b"WAVEfmt ")?;
    w.write_all(&16u32.to_le_bytes())?;
    w.write_all(&1u16.to_le_bytes())?; // PCM
    w.write_all(&1u16.to_le_bytes())?; // one channel
    w.write_all(&rate.to_le_bytes())?;
    w.write_all(&(rate * 2).to_le_bytes())?;
    w.write_all(&2u16.to_le_bytes())?;
    w.write_all(&16u16.to_le_bytes())?;
    w.write_all(b"data")?;
    w.write_all(&data.to_le_bytes())?;
    let mut buf = Vec::with_capacity(1 << 16);
    for chunk in x.chunks(1 << 15) {
        buf.clear();
        for &v in chunk {
            buf.extend_from_slice(&((v.clamp(-1.0, 1.0) * 32767.0).round() as i16).to_le_bytes());
        }
        w.write_all(&buf)?;
    }
    w.into_inner().map_err(|e| e.into_error())?.sync_all()?;
    fs::rename(&tmp, &dst)?;
    Ok(dst)
}

/// At the start of a run: let go of the audio no transcript in `keep` (the
/// recent list, newest first) needs, then cap the rest.
pub fn prune(dir: &Path, keep: &[String]) {
    let Ok(list) = fs::read_dir(dir) else { return };
    let wanted: HashSet<&str> = keep.iter().map(|s| s.as_str()).collect();
    for f in list.flatten() {
        let name = f.file_name().to_string_lossy().into_owned();
        let id = name.strip_suffix(".wav").unwrap_or("");
        if !wanted.contains(id) {
            let _ = fs::remove_file(f.path());
        }
    }
    cap(dir, keep);
}

/// Let go of the oldest transcripts' audio in `keep` while the cache holds
/// more than CACHE_BYTES of it; this run's other audio stays till the next.
/// A transcript whose audio is gone plays again once its original is read
/// again.
pub fn cap(dir: &Path, keep: &[String]) {
    let mut total = 0u64;
    for id in keep {
        let p = wav_in(dir, id);
        let Ok(m) = fs::metadata(&p) else { continue };
        total += m.len();
        if total > CACHE_BYTES {
            let _ = fs::remove_file(p);
        }
    }
}
