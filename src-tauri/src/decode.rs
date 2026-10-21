//! A media file's audio as 16 kHz mono float samples.
//!
//! macOS and Windows use the media libraries the OS already has
//! (AVFoundation, Media Foundation: native/decode_*.{m,c}); when the OS
//! cannot read a file and ffmpeg is on the PATH, ffmpeg tries it. Linux has
//! no one media library, so there ffmpeg is required. PKTITO_DECODER=ffmpeg
//! uses ffmpeg everywhere; PKTITO_DECODER=os the OS alone.

use std::env;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub const RATE: usize = 16000;

#[derive(Debug)]
pub enum DecodeError {
    /// The file has no audio to transcribe.
    NoAudio,
    /// It could not be read; the reason, for the user.
    Unreadable(String),
}

impl DecodeError {
    pub fn message(&self) -> String {
        match self {
            DecodeError::NoAudio => "No audio track in this file. Try the original recording.".into(),
            DecodeError::Unreadable(why) => why.clone(),
        }
    }
}

pub fn decode(path: &Path) -> Result<Vec<f32>, DecodeError> {
    let forced = env::var("PKTITO_DECODER").unwrap_or_default().to_lowercase();
    if forced == "ffmpeg" || !cfg!(any(target_os = "macos", target_os = "windows")) {
        return match ffmpeg() {
            Some(ff) => decode_ffmpeg(&ff, path),
            None => Err(DecodeError::Unreadable(
                "ffmpeg is needed to read media files here. Install it with your package manager.".into(),
            )),
        };
    }
    match decode_os(path) {
        Ok(x) => Ok(x),
        Err(DecodeError::NoAudio) => Err(DecodeError::NoAudio),
        Err(DecodeError::Unreadable(why)) => match (forced.as_str(), ffmpeg()) {
            ("os", _) => Err(DecodeError::Unreadable(format!("This file could not be read: {why}."))),
            (_, None) => Err(DecodeError::Unreadable(format!(
                "This file could not be read: {why}. Installing ffmpeg lets pktito try it another way."
            ))),
            (_, Some(ff)) => decode_ffmpeg(&ff, path),
        },
    }
}

/// ffmpeg on the PATH, if it is there.
fn ffmpeg() -> Option<PathBuf> {
    let name = if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" };
    env::split_paths(&env::var_os("PATH")?).map(|d| d.join(name)).find(|p| p.is_file())
}

fn decode_ffmpeg(ffmpeg: &Path, path: &Path) -> Result<Vec<f32>, DecodeError> {
    let mut child = Command::new(ffmpeg)
        .args(["-nostdin", "-hide_banner", "-loglevel", "error", "-i"])
        .arg(path)
        .args(["-map", "0:a:0", "-vn", "-ac", "1", "-ar", "16000", "-f", "f32le", "-"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| DecodeError::Unreadable(format!("ffmpeg could not be started: {e}")))?;
    let mut bytes = Vec::new();
    child.stdout.take().unwrap().read_to_end(&mut bytes).ok();
    let mut stderr = String::new();
    child.stderr.take().unwrap().read_to_string(&mut stderr).ok();
    let status = child.wait().map_err(|e| DecodeError::Unreadable(e.to_string()))?;
    if !status.success() {
        if stderr.contains("matches no streams") || stderr.contains("does not contain any stream") {
            return Err(DecodeError::NoAudio);
        }
        let first = stderr.lines().last().unwrap_or("ffmpeg could not read it").trim().to_string();
        return Err(DecodeError::Unreadable(format!("This file could not be read: {first}")));
    }
    Ok(bytes.chunks_exact(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect())
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn decode_os(path: &Path) -> Result<Vec<f32>, DecodeError> {
    use std::ffi::{c_char, c_int, CStr, CString};
    extern "C" {
        fn pkapp_decode_os(path: *const c_char, rate: c_int, out: *mut *mut f32, n: *mut usize, err: *mut c_char, errlen: usize) -> c_int;
    }
    extern "C" {
        fn free(p: *mut std::ffi::c_void);
    }
    let c = CString::new(path.to_string_lossy().as_bytes()).map_err(|_| DecodeError::Unreadable("bad file name".into()))?;
    let mut out: *mut f32 = std::ptr::null_mut();
    let mut n: usize = 0;
    let mut err = [0 as c_char; 512];
    let rc = unsafe { pkapp_decode_os(c.as_ptr(), RATE as c_int, &mut out, &mut n, err.as_mut_ptr(), err.len()) };
    match rc {
        0 => {
            let x = if n > 0 { unsafe { std::slice::from_raw_parts(out, n) }.to_vec() } else { Vec::new() };
            unsafe { free(out as *mut std::ffi::c_void) };
            if x.is_empty() {
                return Err(DecodeError::NoAudio);
            }
            Ok(x)
        }
        -1 => Err(DecodeError::NoAudio),
        _ => Err(DecodeError::Unreadable(unsafe { CStr::from_ptr(err.as_ptr()) }.to_string_lossy().into_owned())),
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn decode_os(_path: &Path) -> Result<Vec<f32>, DecodeError> {
    Err(DecodeError::Unreadable("no media library to use here".into()))
}
