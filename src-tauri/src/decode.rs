//! A media file's audio as 16 kHz mono float samples.
//!
//! pktito's own reader (vendor/pktito, core/wav.h) reads WAV and AIFF on
//! every platform, so a server, a sandbox or a Windows without Media
//! Foundation (Server, the N editions) needs nothing more for them, and
//! Windows, whose Media Foundation has no AIFF reader, reads a Mac's AIFF.
//! Everything else goes to the media libraries macOS and Windows already
//! have (AVFoundation, Media Foundation: native/decode_*.{m,c}), and when
//! the OS cannot read a file and ffmpeg is on the PATH, ffmpeg tries it;
//! Linux has no one media library, so there ffmpeg reads everything but
//! WAV and AIFF. PKTITO_DECODER=ffmpeg
//! uses ffmpeg alone; PKTITO_DECODER=os the OS alone.
//!
//! Each decoder hands over its samples at their own rate, and the same mix
//! (native/mix.h) brings them to one channel and 16 kHz, so a file reads the
//! same whichever decoder read it. Where the decoders start a file's sound
//! in different places, its samples are cut to one start: an MP4's by its
//! edit list (mp4.rs), an MP3's by its LAME tag (mp3.rs).

use crate::strings::Problem;
use std::env;
use std::ffi::{c_char, c_int, c_uint, c_void, CStr, CString};
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
    let forced = env::var("PKTITO_DECODER")
        .unwrap_or_default()
        .to_lowercase();
    if forced != "ffmpeg" && forced != "os" {
        // more than two channels go to a reader that knows their speakers:
        // the OS's, where it has one, else ffmpeg, if it is there; asked
        // only then, since asking Windows loads Media Foundation
        let other = || {
            if cfg!(any(target_os = "macos", target_os = "windows")) {
                os_decoder()
            } else {
                ffmpeg().is_some()
            }
        };
        if let Some(read) = decode_wav(path, other) {
            return read;
        }
    }
    if forced == "ffmpeg" || !cfg!(any(target_os = "macos", target_os = "windows")) {
        return match ffmpeg() {
            Some(ff) => decode_ffmpeg(&ff, path),
            None => Err(DecodeError::Unreadable(
                "decode.needs_ffmpeg",
                String::new(),
            )),
        };
    }
    match decode_os(path) {
        Ok(x) => Ok(flac_cut(path, mp3_cut(path, edited(path, x), OS_READER))),
        Err(DecodeError::NoAudio) => Err(DecodeError::NoAudio),
        Err(DecodeError::Unreadable(why, detail)) => match (forced.as_str(), ffmpeg()) {
            ("os", _) => Err(DecodeError::Unreadable(why, detail)),
            (_, None) if why == UNREADABLE => Err(DecodeError::Unreadable(
                "decode.unreadable_try_ffmpeg",
                detail,
            )),
            (_, None) => Err(DecodeError::Unreadable(why, detail)),
            (_, Some(ff)) => decode_ffmpeg(&ff, path),
        },
    }
}

/// pktito's WAV reader's account of a file (core/wav.h).
#[repr(C)]
struct PkWavInfo {
    rate: u32,
    channels: u32,
    format: u32,
    bytes: u32,
    frames: u64,
}

extern "C" {
    fn pk_wav_open(
        path: *const c_char,
        info: *mut PkWavInfo,
        err: *mut c_char,
        len: usize,
    ) -> *mut c_void;
    fn pk_wav_read_frames(w: *mut c_void, out: *mut f32, n: usize) -> usize;
    fn pk_wav_status(w: *const c_void, err: *mut c_char, len: usize) -> c_int;
    fn pk_wav_close(w: *mut c_void);
}

/// A WAV or an AIFF, by pktito's reader: every format a recording comes in
/// (PCM, float, A-law and mu-law; WAV's extensible and RF64, AIFF-C's
/// uncompressed kinds), held to the library's
/// docs/MEMORY_SAFETY.md, and through the same mix as every other decoder's
/// samples. None for a file the reader does not open, which goes to the
/// OS's reader or ffmpeg as before. The reader names no speaker layout,
/// which is the usual one up to two channels; more than two go to the other
/// reader, which has the file's own, when there is one (`other`). A file
/// cut short gives what it has, as ffmpeg's does. On Windows the reader's
/// fopen takes the UTF-8 name through the manifest's code page (build.rs);
/// before Windows 10 1903, which ignores it, a name past ASCII opens
/// nothing here and goes to Media Foundation.
fn decode_wav(path: &Path, other: impl FnOnce() -> bool) -> Option<Result<Vec<f32>, DecodeError>> {
    let c_path = CString::new(path.as_os_str().as_encoded_bytes()).ok()?;
    let mut info = PkWavInfo {
        rate: 0,
        channels: 0,
        format: 0,
        bytes: 0,
        frames: 0,
    };
    let mut err = [0 as c_char; 256];
    let w = unsafe { pk_wav_open(c_path.as_ptr(), &mut info, err.as_mut_ptr(), err.len()) };
    if w.is_null() {
        return None;
    }
    struct Close(*mut c_void);
    impl Drop for Close {
        fn drop(&mut self) {
            unsafe { pk_wav_close(self.0) };
        }
    }
    let w = Close(w);
    if info.channels > 2 && other() {
        return None;
    }
    let f = WavFormat {
        rate: info.rate,
        ch: info.channels,
        mask: 0,
    };
    Some((|| {
        let mut mix = Mix::new(&f)?;
        let frames = 4096;
        let mut x = vec![0f32; frames * f.ch as usize];
        let mut total = 0;
        loop {
            let n = unsafe { pk_wav_read_frames(w.0, x.as_mut_ptr(), frames) };
            mix.push(&x[..n * f.ch as usize])?;
            total += n;
            if n < frames {
                break;
            }
        }
        if total == 0 {
            if unsafe { pk_wav_status(w.0, err.as_mut_ptr(), err.len()) } != 0 {
                let why = unsafe { CStr::from_ptr(err.as_ptr()) }
                    .to_string_lossy()
                    .into_owned();
                return Err(DecodeError::Unreadable(UNREADABLE, why));
            }
            return Err(DecodeError::NoAudio);
        }
        mix.take()
    })())
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
    let x = if n > 0 {
        unsafe { std::slice::from_raw_parts(y, n) }.to_vec()
    } else {
        Vec::new()
    };
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
        let m = Mix {
            m: unsafe { pkapp_mix_new(RATE as c_uint) },
            ch: f.ch as usize,
        };
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
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
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

/// The OS's decoder, for mp3.rs: AVFoundation, or Media Foundation.
const OS_READER: crate::mp3::Reader = if cfg!(target_os = "windows") {
    crate::mp3::Reader::MediaFoundation
} else {
    crate::mp3::Reader::AvFoundation
};

/// An MP3's samples from `by` cut to where its sound starts (mp3.rs), the
/// same place whichever decoder read it.
fn mp3_cut(path: &Path, x: Vec<f32>, by: crate::mp3::Reader) -> Vec<f32> {
    match crate::mp3::edit(path, by) {
        Some(e) => crate::mp4::apply(x, &e, RATE as f64),
        None => x,
    }
}

/// A FLAC's samples cut to the length its STREAMINFO names. macOS 26's
/// decoder fills the last block out to the stream's block size with
/// silence (120 samples more on AliceBench's chapter 1, Mac mini M4, macOS
/// 26.4.1), which moved the last words of six of its twelve chapters;
/// macOS 27 and ffmpeg end where the file does, and are left as they are.
fn flac_cut(path: &Path, mut x: Vec<f32>) -> Vec<f32> {
    let mut b = [0u8; 26];
    let read = std::fs::File::open(path).and_then(|mut f| f.read_exact(&mut b));
    if let Some((rate, total)) = read.ok().and_then(|_| flac_length(&b)) {
        // at 16 kHz, the samples themselves; at another rate, the mix's
        // count of them, to the nearest
        let n = ((total as u128 * RATE as u128 + rate as u128 / 2) / rate as u128) as usize;
        x.truncate(n);
    }
    x
}

/// A FLAC's sample rate and length in samples, from its STREAMINFO, the
/// block the format puts first: "fLaC", the block's header (type 0, 34
/// bytes), the block and frame sizes, then 20 bits of rate, 3 of channels,
/// 5 of sample size and 36 of samples. None for anything else, and for a
/// stream that gives no length (0).
fn flac_length(b: &[u8]) -> Option<(u32, u64)> {
    if b.len() < 26 || &b[..4] != b"fLaC" || b[4] & 0x7f != 0 || b[5..8] != [0, 0, 34] {
        return None;
    }
    let rate = (b[18] as u32) << 12 | (b[19] as u32) << 4 | (b[20] as u32) >> 4;
    let total = b[21..26].iter().fold(0u64, |t, &v| t << 8 | v as u64) & 0xf_ffff_ffff;
    (rate > 0 && total > 0).then_some((rate, total))
}

/// ffmpeg, from the PATH, or on Windows where WinGet, Scoop and Chocolatey
/// put it: an app started from the Start menu has the environment Explorer
/// started with, which a terminal's PATH may have outgrown.
fn ffmpeg() -> Option<PathBuf> {
    let name = if cfg!(windows) {
        "ffmpeg.exe"
    } else {
        "ffmpeg"
    };
    let mut dirs: Vec<PathBuf> = env::var_os("PATH")
        .map(|p| env::split_paths(&p).collect())
        .unwrap_or_default();
    if cfg!(windows) {
        for (var, sub) in [
            ("LOCALAPPDATA", r"Microsoft\WinGet\Links"),
            ("USERPROFILE", r"scoop\shims"),
            ("ProgramData", r"chocolatey\bin"),
        ] {
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
        .args([
            "-map",
            "0:a:0",
            "-vn",
            "-c:a",
            "pcm_f32le",
            "-f",
            "wav",
            "-",
        ])
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
    let status = child
        .wait()
        .map_err(|e| DecodeError::Unreadable(UNREADABLE, e.to_string()))?;
    let stderr = said.join().unwrap_or_default();
    if !status.success() {
        if stderr.contains("matches no streams") || stderr.contains("does not contain any stream") {
            return Err(DecodeError::NoAudio);
        }
        // ffmpeg's last line is the one that says why. Of a text file named
        // .mp3, ffmpeg 7 says "Error opening input files: Invalid data found
        // when processing input" (the Mac), and the Windows test's
        // "C:\pktito-test\fake.mp3: Invalid argument".
        let why = stderr
            .lines()
            .last()
            .unwrap_or("ffmpeg could not read it")
            .trim()
            .trim_end_matches('.');
        let opening = stderr.contains("Error opening input")
            || why.starts_with(&format!("{}: ", path.display()));
        if opening && (why.contains("Invalid data") || why.ends_with("Invalid argument")) {
            return Err(DecodeError::Unreadable(NOT_MEDIA, String::new()));
        }
        return Err(DecodeError::Unreadable(UNREADABLE, why.into()));
    }
    read.unwrap_or_else(|| {
        Err(DecodeError::Unreadable(
            UNREADABLE,
            "ffmpeg returned no audio".into(),
        ))
    })
    .map(|x| mp3_cut(path, x, crate::mp3::Reader::Ffmpeg))
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
                format = Some(WavFormat {
                    rate: u32_at(4),
                    ch: u16_at(2) as u32,
                    mask,
                });
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
            Err(e) => {
                return Err(DecodeError::Unreadable(
                    UNREADABLE,
                    format!("ffmpeg's output: {e}"),
                ))
            }
        };
        have += got;
        let whole = have / frame * frame;
        x.clear();
        x.extend(
            bytes[..whole]
                .chunks_exact(4)
                .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])),
        );
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
    let r = if os > 0.0 {
        Some(os)
    } else {
        ffmpeg().and_then(|ff| rate_ffmpeg(&ff, path))
    };
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
    let line = said
        .lines()
        .find(|l| l.contains("Stream #") && l.contains(": Video:"))?;
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
        fn pkapp_decode_os(
            path: *const c_char,
            rate: c_int,
            out: *mut *mut f32,
            n: *mut usize,
            err: *mut c_char,
            errlen: usize,
        ) -> c_int;
    }
    let c = CString::new(path.to_string_lossy().as_bytes())
        .map_err(|_| DecodeError::Unreadable(UNREADABLE, "bad file name".into()))?;
    let mut out: *mut f32 = std::ptr::null_mut();
    let mut n: usize = 0;
    let mut err = [0 as c_char; 512];
    let rc = unsafe {
        pkapp_decode_os(
            c.as_ptr(),
            RATE as c_int,
            &mut out,
            &mut n,
            err.as_mut_ptr(),
            err.len(),
        )
    };
    match rc {
        0 => {
            let x = take_c(out, n);
            if x.is_empty() {
                return Err(DecodeError::NoAudio);
            }
            Ok(x)
        }
        -1 => Err(DecodeError::NoAudio),
        -3 => Err(DecodeError::Unreadable(
            "decode.needs_media_foundation",
            String::new(),
        )),
        _ => Err(DecodeError::Unreadable(
            UNREADABLE,
            unsafe { CStr::from_ptr(err.as_ptr()) }
                .to_string_lossy()
                .into_owned(),
        )),
    }
}

/// Whether the OS's media library is there to read a file: macOS's always
/// is; Windows Server and the N editions have no Media Foundation until the
/// Media Feature Pack adds it (native/decode_windows.c).
#[cfg(target_os = "windows")]
fn os_decoder() -> bool {
    extern "C" {
        fn pkapp_os_decoder() -> c_int;
    }
    unsafe { pkapp_os_decoder() != 0 }
}

#[cfg(not(target_os = "windows"))]
fn os_decoder() -> bool {
    cfg!(target_os = "macos")
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn decode_os(_path: &Path) -> Result<Vec<f32>, DecodeError> {
    Err(DecodeError::Unreadable(
        UNREADABLE,
        "no media library to use here".into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flac_length_from_streaminfo() {
        // AliceBench's chapter 1: 16 kHz, 10,248,072 samples
        let ch1 = [
            0x66, 0x4c, 0x61, 0x43, 0x00, 0x00, 0x00, 0x22, 0x10, 0x00, 0x10, 0x00, 0x00, 0x07,
            0xae, 0x00, 0x19, 0x8a, 0x03, 0xe8, 0x00, 0xf0, 0x00, 0x9c, 0x5f, 0x88,
        ];
        assert_eq!(flac_length(&ch1), Some((16000, 10_248_072)));
        // a last block of STREAMINFO (0x80) is STREAMINFO still
        let mut last = ch1;
        last[4] = 0x80;
        assert_eq!(flac_length(&last), Some((16000, 10_248_072)));
        // no length given, another block first, not FLAC, too short
        let mut unknown = ch1;
        unknown[21] &= 0xf0;
        unknown[22..26].fill(0);
        assert_eq!(flac_length(&unknown), None);
        let mut padding = ch1;
        padding[4] = 1;
        assert_eq!(flac_length(&padding), None);
        assert_eq!(
            flac_length(b"RIFF\0\0\0\0WAVEfmt \x10\0\0\0\x01\0\x01\0\x80>\0\0"),
            None
        );
        assert_eq!(flac_length(&ch1[..20]), None);
    }
}
