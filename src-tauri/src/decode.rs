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

use crate::strings::Problem;
use std::env;
use std::ffi::{c_int, c_uint, c_void};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub const RATE: usize = 16000;
/// The extensions of the files the app offers to open, and pktito-cli bench
/// takes from a folder.
pub const MEDIA: &[&str] = &[
    "mp3", "mp4", "m4a", "m4v", "mov", "wav", "aac", "aif", "aiff", "caf", "flac", "ogg", "opus",
    "webm", "mkv", "wma", "wmv", "avi", "3gp", "amr",
];

#[derive(Debug)]
pub enum DecodeError {
    /// The file has no audio to transcribe.
    NoAudio,
    /// It could not be read: what happened, as the catalog's key
    /// (strings.rs), and what said so.
    Unreadable(&'static str, String),
}

const UNREADABLE: &str = "decode.unreadable";
/// What ffmpeg, which reads nearly every format there is, cannot open as
/// media at all. The OS's media libraries say nothing so clear: Windows'
/// MF_E_UNSUPPORTED_BYTESTREAM_TYPE is also an Ogg file it has no reader
/// for, which ffmpeg reads.
const NOT_MEDIA: &str = "decode.not_media";

impl DecodeError {
    pub fn problem(&self) -> Problem {
        match self {
            DecodeError::NoAudio => Problem::new("decode.no_audio"),
            DecodeError::Unreadable(key, detail) => Problem::new(key).detail(detail),
        }
    }

    /// In a line, in English, for pktito-cli.
    pub fn message(&self) -> String {
        self.problem().line()
    }
}

pub fn decode(path: &Path) -> Result<Vec<f32>, DecodeError> {
    // the window queues only files that are there, so this is one moved
    // while it waited; Windows' own words for it are "(0x80070002)"
    if !path.is_file() {
        return Err(DecodeError::Unreadable("decode.not_found", String::new()));
    }
    let forced = env::var("PKTITO_DECODER").unwrap_or_default().to_lowercase();
    if forced == "ffmpeg" || !cfg!(any(target_os = "macos", target_os = "windows")) {
        return match ffmpeg() {
            Some(ff) => decode_ffmpeg(&ff, path),
            None => Err(DecodeError::Unreadable("decode.needs_ffmpeg", String::new())),
        };
    }
    match decode_os(path) {
        Ok(x) => Ok(edited(path, x)),
        Err(DecodeError::NoAudio) => Err(DecodeError::NoAudio),
        Err(DecodeError::Unreadable(why, detail)) => match (forced.as_str(), ffmpeg()) {
            ("os", _) => Err(DecodeError::Unreadable(why, detail)),
            (_, None) if why == UNREADABLE => Err(DecodeError::Unreadable("decode.unreadable_try_ffmpeg", detail)),
            (_, None) => Err(DecodeError::Unreadable(why, detail)),
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
    DecodeError::Unreadable("problem.memory", String::new())
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
            return Err(DecodeError::Unreadable(
                "decode.cant_convert",
                format!("{} channels at {} Hz", f.ch, f.rate),
            ));
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
/// An MP4, M4A or QuickTime file, the containers with edit lists that
/// Media Foundation reads (and the only ones whose frame rate it reads right).
fn mp4_family(path: &Path) -> bool {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    ["mp4", "m4a", "m4v", "mov", "3gp"].contains(&ext.as_str())
}

/// Media Foundation's samples of an MP4 as the file's edit list presents
/// them (mp4.rs), its encoder's priming and padding gone; AVFoundation and
/// ffmpeg apply the list themselves.
fn edited(path: &Path, x: Vec<f32>) -> Vec<f32> {
    if !cfg!(target_os = "windows") || !mp4_family(path) {
        return x;
    }
    match crate::mp4::audio_edit(path) {
        Some(e) => crate::mp4::apply(x, &e, RATE as f64),
        None => x,
    }
}

/// ffmpeg, from the PATH, or on Windows where WinGet, Scoop and Chocolatey
/// put it: an app started from the Start menu has the environment Explorer
/// started with, which a terminal's PATH may have outgrown.
fn ffmpeg() -> Option<PathBuf> {
    let name = if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" };
    let mut dirs: Vec<PathBuf> = env::var_os("PATH").map(|p| env::split_paths(&p).collect()).unwrap_or_default();
    if cfg!(windows) {
        for (var, sub) in [("LOCALAPPDATA", r"Microsoft\WinGet\Links"), ("USERPROFILE", r"scoop\shims"), ("ProgramData", r"chocolatey\bin")] {
            if let Some(base) = env::var_os(var) {
                dirs.push(PathBuf::from(base).join(sub));
            }
        }
    }
    dirs.into_iter().map(|d| d.join(name)).find(|p| p.is_file())
}

/// ffmpeg, without the console window Windows opens for a console program
/// that a windowed one starts
fn ffmpeg_command(ffmpeg: &Path) -> Command {
    #[allow(unused_mut)]
    let mut c = Command::new(ffmpeg);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        c.creation_flags(CREATE_NO_WINDOW);
    }
    c
}

fn decode_ffmpeg(ffmpeg: &Path, path: &Path) -> Result<Vec<f32>, DecodeError> {
    let mut child = ffmpeg_command(ffmpeg)
        .args(["-nostdin", "-hide_banner", "-loglevel", "error", "-i"])
        .arg(path)
        // the first audio stream as it is, every channel at its own rate, in
        // a WAV that says which; the mix takes it from there
        .args(["-map", "0:a:0", "-vn", "-c:a", "pcm_f32le", "-f", "wav", "-"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| DecodeError::Unreadable("decode.cant_start_ffmpeg", e.to_string()))?;
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
    let status = child.wait().map_err(|e| DecodeError::Unreadable(UNREADABLE, e.to_string()))?;
    let stderr = said.join().unwrap_or_default();
    if !status.success() {
        if stderr.contains("matches no streams") || stderr.contains("does not contain any stream") {
            return Err(DecodeError::NoAudio);
        }
        // ffmpeg's last line is the one that says why. Of a text file named
        // .mp3, ffmpeg 7 says "Error opening input files: Invalid data found
        // when processing input" (the Mac), and the Windows test's
        // "C:\pktito-test\fake.mp3: Invalid argument".
        let why = stderr.lines().last().unwrap_or("ffmpeg could not read it").trim().trim_end_matches('.');
        let opening = stderr.contains("Error opening input") || why.starts_with(&format!("{}: ", path.display()));
        if opening && (why.contains("Invalid data") || why.ends_with("Invalid argument")) {
            return Err(DecodeError::Unreadable(NOT_MEDIA, String::new()));
        }
        return Err(DecodeError::Unreadable(UNREADABLE, why.into()));
    }
    read.unwrap_or_else(|| Err(DecodeError::Unreadable(UNREADABLE, "ffmpeg returned no audio".into())))
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
            Err(e) => return Err(DecodeError::Unreadable(UNREADABLE, format!("ffmpeg's output: {e}"))),
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

/// A video's frame rate, for the window's times in frames: its first video
/// stream's, as the OS reads it from an MP4 or QuickTime file, else as
/// ffmpeg does; None for a file with no video, or that neither can say.
/// Media Foundation's reading of the other containers is not to be had: it
/// gave every Matroska file half its rate, and a 10 fps .wmv 30, where
/// ffmpeg was right on all of them (the Windows check, 2026-10-22).
pub fn frame_rate(path: &Path) -> Option<f64> {
    let os = if mp4_family(path) { rate_os(path) } else { 0.0 };
    let r = if os > 0.0 { Some(os) } else { ffmpeg().and_then(|ff| rate_ffmpeg(&ff, path)) };
    r.filter(|r| r.is_finite() && (1.0..=1000.0).contains(r))
}

/// ffmpeg's reading of the rate, from the line it prints for the file's
/// first video stream: "Stream #0:0...: Video: h264 ..., 23.98 fps, ...".
fn rate_ffmpeg(ffmpeg: &Path, path: &Path) -> Option<f64> {
    let out = ffmpeg_command(ffmpeg)
        .args(["-nostdin", "-hide_banner", "-i"])
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .output()
        .ok()?;
    let said = String::from_utf8_lossy(&out.stderr);
    let line = said.lines().find(|l| l.contains("Stream #") && l.contains(": Video:"))?;
    let at = line.find(" fps")?;
    line[..at].rsplit([' ', ',']).next()?.parse().ok()
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn rate_os(path: &Path) -> f64 {
    use std::ffi::{c_char, CString};
    extern "C" {
        fn pkapp_video_rate_os(path: *const c_char) -> f64;
    }
    match CString::new(path.to_string_lossy().as_bytes()) {
        Ok(c) => unsafe { pkapp_video_rate_os(c.as_ptr()) },
        Err(_) => 0.0,
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn rate_os(_path: &Path) -> f64 {
    0.0
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn decode_os(path: &Path) -> Result<Vec<f32>, DecodeError> {
    use std::ffi::{c_char, CStr, CString};
    extern "C" {
        fn pkapp_decode_os(path: *const c_char, rate: c_int, out: *mut *mut f32, n: *mut usize, err: *mut c_char, errlen: usize) -> c_int;
    }
    let c = CString::new(path.to_string_lossy().as_bytes()).map_err(|_| DecodeError::Unreadable(UNREADABLE, "bad file name".into()))?;
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
        _ => Err(DecodeError::Unreadable(UNREADABLE, unsafe { CStr::from_ptr(err.as_ptr()) }.to_string_lossy().into_owned())),
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn decode_os(_path: &Path) -> Result<Vec<f32>, DecodeError> {
    Err(DecodeError::Unreadable(UNREADABLE, "no media library to use here".into()))
}
