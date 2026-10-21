//! A media file's audio as 16 kHz mono float samples.
//!
//! macOS and Windows use the media libraries the OS already has
//! (AVFoundation, Media Foundation: native/decode_*.{m,c}); when the OS
//! cannot read a file and ffmpeg is on the PATH, ffmpeg tries it. Linux has
//! no one media library, so there ffmpeg is required. PKTITO_DECODER=ffmpeg
//! uses ffmpeg everywhere; PKTITO_DECODER=os the OS alone.
//!
//! Each decoder hands over its samples at their own rate, and the same mix
//! (native/mix.h) brings them to one channel and 16 kHz, so a file reads the
//! same whichever decoder read it.

use std::env;
use std::ffi::{c_int, c_uint, c_void};
use std::io::{self, Read};
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

extern "C" {
    fn pkapp_mix_new(rate: c_uint) -> *mut c_void;
    fn pkapp_mix_free(m: *mut c_void);
    fn pkapp_mix_format_wave(m: *mut c_void, rate: c_uint, ch: c_uint, mask: c_uint) -> c_int;
    fn pkapp_mix_push(m: *mut c_void, x: *const f32, n: usize) -> c_int;
    fn pkapp_mix_take(m: *mut c_void, y: *mut *mut f32, n: *mut usize) -> c_int;
    fn free(p: *mut c_void);
}

fn out_of_memory() -> DecodeError {
    DecodeError::Unreadable("There is not enough memory to read this file.".into())
}

/// A malloc'd signal from C, into a Vec, and freed.
fn take_c(y: *mut f32, n: usize) -> Vec<f32> {
    let x = if n > 0 { unsafe { std::slice::from_raw_parts(y, n) }.to_vec() } else { Vec::new() };
    unsafe { free(y as *mut c_void) };
    x
}

/// What a WAV says of its samples.
struct WavFormat {
    rate: u32,
    ch: u32,
    /// WAVE's speaker mask; 0 for the usual layout
    mask: u32,
}

/// The mix (native/mix.h), for samples in a WAV's format.
struct Mix {
    m: *mut c_void,
    ch: usize,
}

impl Mix {
    fn new(f: &WavFormat) -> Result<Mix, DecodeError> {
        let m = Mix { m: unsafe { pkapp_mix_new(RATE as c_uint) }, ch: f.ch as usize };
        if m.m.is_null() || unsafe { pkapp_mix_format_wave(m.m, f.rate, f.ch, f.mask) } != 0 {
            return Err(DecodeError::Unreadable(format!(
                "This file's audio, {} channels at {} Hz, cannot be converted.",
                f.ch, f.rate
            )));
        }
        Ok(m)
    }

    /// Whole frames, interleaved.
    fn push(&mut self, x: &[f32]) -> Result<(), DecodeError> {
        match unsafe { pkapp_mix_push(self.m, x.as_ptr(), x.len() / self.ch) } {
            0 => Ok(()),
            _ => Err(out_of_memory()),
        }
    }

    fn take(self) -> Result<Vec<f32>, DecodeError> {
        let (mut y, mut n) = (std::ptr::null_mut(), 0);
        match unsafe { pkapp_mix_take(self.m, &mut y, &mut n) } {
            0 => Ok(take_c(y, n)),
            _ => Err(out_of_memory()),
        }
    }
}

impl Drop for Mix {
    fn drop(&mut self) {
        unsafe { pkapp_mix_free(self.m) };
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
        // the first audio stream as it is, every channel at its own rate, in
        // a WAV that says which; the mix takes it from there
        .args(["-map", "0:a:0", "-vn", "-c:a", "pcm_f32le", "-f", "wav", "-"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| DecodeError::Unreadable(format!("ffmpeg could not be started: {e}.")))?;
    // read on its own, so that a file ffmpeg complains about at length cannot
    // fill the pipe and stop it while its samples are being read
    let mut stderr = child.stderr.take().unwrap();
    let said = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = stderr.read_to_string(&mut s);
        s
    });
    let mut out = child.stdout.take().unwrap();
    let read = wav_format(&mut out).map(|f| read_samples(&mut out, &f));
    drop(out);
    let status = child.wait().map_err(|e| DecodeError::Unreadable(e.to_string()))?;
    let stderr = said.join().unwrap_or_default();
    if !status.success() {
        if stderr.contains("matches no streams") || stderr.contains("does not contain any stream") {
            return Err(DecodeError::NoAudio);
        }
        // ffmpeg's last line is the one that says why
        let why = stderr.lines().last().unwrap_or("ffmpeg could not read it").trim().trim_end_matches('.');
        return Err(DecodeError::Unreadable(format!("This file could not be read: {why}.")));
    }
    read.unwrap_or_else(|| Err(DecodeError::Unreadable("ffmpeg did not hand over what was asked of it.".into())))
}

/// The format of the float WAV ffmpeg writes, read up to its samples. A
/// pipe cannot be rewound, so the sizes ffmpeg leaves unknown in the header
/// are not needed: the samples run to the end.
fn wav_format(r: &mut impl Read) -> Option<WavFormat> {
    let mut head = [0u8; 12];
    r.read_exact(&mut head).ok()?;
    if &head[0..4] != b"RIFF" || &head[8..12] != b"WAVE" {
        return None;
    }
    let mut format = None;
    loop {
        let mut chunk = [0u8; 8];
        r.read_exact(&mut chunk).ok()?;
        let size = u32::from_le_bytes([chunk[4], chunk[5], chunk[6], chunk[7]]) as u64;
        match &chunk[0..4] {
            b"data" => return format,
            b"fmt " if (16..=64).contains(&size) => {
                let mut f = vec![0u8; (size + size % 2) as usize];
                r.read_exact(&mut f).ok()?;
                let u16_at = |i: usize| u16::from_le_bytes([f[i], f[i + 1]]);
                let u32_at = |i: usize| u32::from_le_bytes([f[i], f[i + 1], f[i + 2], f[i + 3]]);
                // IEEE float (3), or WAVE_FORMAT_EXTENSIBLE with a float subformat and a speaker mask
                let mask = match u16_at(0) {
                    3 => 0,
                    0xfffe if size >= 40 && u16_at(24) == 3 => u32_at(20),
                    _ => return None,
                };
                if u16_at(14) != 32 || u16_at(2) == 0 {
                    return None;
                }
                format = Some(WavFormat { rate: u32_at(4), ch: u16_at(2) as u32, mask });
            }
            b"fmt " => return None,
            _ => {
                io::copy(&mut r.take(size + size % 2), &mut io::sink()).ok()?;
            }
        }
    }
}

/// The samples after the header, through the mix a piece at a time.
fn read_samples(r: &mut impl Read, f: &WavFormat) -> Result<Vec<f32>, DecodeError> {
    let mut mix = Mix::new(f)?;
    let frame = 4 * f.ch as usize;
    let mut bytes = vec![0u8; (1 << 16) / frame * frame];
    let mut x = Vec::with_capacity(bytes.len() / 4);
    let mut have = 0; // bytes in hand: at most part of a frame carried over
    loop {
        let got = match r.read(&mut bytes[have..]) {
            Ok(0) => break,
            Ok(got) => got,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(DecodeError::Unreadable(format!("ffmpeg's output could not be read: {e}."))),
        };
        have += got;
        let whole = have / frame * frame;
        x.clear();
        x.extend(bytes[..whole].chunks_exact(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])));
        mix.push(&x)?;
        bytes.copy_within(whole..have, 0);
        have -= whole;
    }
    mix.take()
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn decode_os(path: &Path) -> Result<Vec<f32>, DecodeError> {
    use std::ffi::{c_char, CStr, CString};
    extern "C" {
        fn pkapp_decode_os(path: *const c_char, rate: c_int, out: *mut *mut f32, n: *mut usize, err: *mut c_char, errlen: usize) -> c_int;
    }
    let c = CString::new(path.to_string_lossy().as_bytes()).map_err(|_| DecodeError::Unreadable("bad file name".into()))?;
    let mut out: *mut f32 = std::ptr::null_mut();
    let mut n: usize = 0;
    let mut err = [0 as c_char; 512];
    let rc = unsafe { pkapp_decode_os(c.as_ptr(), RATE as c_int, &mut out, &mut n, err.as_mut_ptr(), err.len()) };
    match rc {
        0 => {
            let x = take_c(out, n);
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
