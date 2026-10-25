//! Where an MP3's sound starts, as each decoder hands it over, and the
//! samples to drop so that every decoder starts it in the same place.
//!
//! An MP3 encoder starts with samples of its own, its delay (576 from LAME
//! and from ffmpeg's encoder, which is LAME), and decoding adds 529 more.
//! LAME and ffmpeg write the delay, and the padding after the sound, in a
//! LAME tag, in an Info or Xing frame before the first frame of sound, and
//! a gapless decoder takes them out. The decoders pktito uses do not agree
//! (2026-10-25: LibriVox's Alice, chapter 1, MPEG-2 at 24 kHz with LAME
//! 3.96's tag, and three minutes of it as MPEG-1 at 44.1 kHz with ffmpeg's
//! tag and with none; AVFoundation's samples against ffmpeg's by
//! cross-correlation, Media Foundation's against AVFoundation's by
//! transcripts of them shifted by each candidate, to within 16 samples):
//!
//! - ffmpeg takes out the delay and its 529 when the tag is LAME's or its
//!   own ("Lavc", "Lavf"), and leaves its 529 in when there is no tag.
//! - AVFoundation always takes out its 529, and the encoder's delay only
//!   from LAME's tag: from ffmpeg's it leaves the 576 in.
//! - Media Foundation takes out its decoder's delay as AVFoundation does,
//!   but plays the Info frame as a frame of silence, 576 samples at 24 kHz
//!   and 1152 at 44.1 kHz, and leaves the encoder's delay in: Windows'
//!   transcript of Alice came out 48 ms late, of the 44.1 kHz copy 39 ms.
//!
//! So each decoder's samples are cut to one start: the sound's own where a
//! tag says where that is, and otherwise the decoder's delay taken out, as
//! AVFoundation and Media Foundation already do. Where the tag has the
//! length too, what follows the sound goes as well.
//!
//! The file is untrusted: the ID3 tags before the first frame are skipped
//! by their sizes, at most a few, and the first frame is looked for in a
//! bounded read after them, as two frames in a row; anything unexpected is
//! no cut, and the samples as decoded.

use crate::mp4::Edit;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// The decoder a file's samples came from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Reader {
    AvFoundation,
    MediaFoundation,
    Ffmpeg,
}

/// The decoders' own delay, which ffmpeg leaves in an untagged file's samples.
const DECODER_DELAY: u32 = 529;

/// How far into the file the first frame is looked for, after the ID3 tags.
const SEARCH: usize = 4096;

/// What the first frame of a Layer III file says about the sound.
#[derive(Debug, Clone, PartialEq)]
pub struct First {
    pub rate: u32,
    /// samples a frame: 1152 for MPEG-1, 576 for MPEG-2 and 2.5
    pub spf: u32,
    /// it is an Info or Xing frame, with no sound of its own
    pub info: bool,
    /// its frames of sound, from the Info frame
    pub frames: Option<u32>,
    /// the LAME tag's encoder delay and padding, in samples
    pub delay: Option<u32>,
    pub padding: Option<u32>,
    /// the LAME tag's first four bytes, the encoder's name: "LAME", "Lavc"
    pub encoder: [u8; 4],
}

/// The cut that brings the samples `by` decoded to the file's sound, or
/// None: not an MP3, or nothing to cut.
pub fn edit(path: &Path, by: Reader) -> Option<Edit> {
    cut(&first(path)?, by)
}

pub fn cut(f: &First, by: Reader) -> Option<Edit> {
    let lame = &f.encoder == b"LAME";
    let ffmpeg_reads = lame || &f.encoder == b"Lavc" || &f.encoder == b"Lavf";
    let skip = match (by, f.delay) {
        (Reader::Ffmpeg, Some(_)) if ffmpeg_reads => return None,
        (Reader::Ffmpeg, Some(d)) => DECODER_DELAY + d,
        (Reader::Ffmpeg, None) => DECODER_DELAY,
        (Reader::AvFoundation, Some(d)) if !lame => d,
        (Reader::AvFoundation, _) => return None,
        (Reader::MediaFoundation, d) => (if f.info { f.spf } else { 0 }) + d.unwrap_or(0),
    };
    if skip == 0 {
        return None;
    }
    // the sound's length, where the tag has it: what the encoder was given
    let keep = match (f.frames, f.delay, f.padding) {
        (Some(n), Some(d), Some(p)) => (n as u64 * f.spf as u64)
            .checked_sub(d as u64 + p as u64)
            .filter(|&k| k > 0)
            .map(|k| k as f64 / f.rate as f64),
        _ => None,
    };
    Some(Edit {
        lead: 0.0,
        skip: skip as f64 / f.rate as f64,
        keep,
    })
}

/// The first frame of the file, if it is MPEG audio Layer III.
pub fn first(path: &Path) -> Option<First> {
    let mut file = File::open(path).ok()?;
    let end = file.metadata().ok()?.len();
    // ID3v2 tags: "ID3", version, flags, a 28-bit size in four 7-bit bytes,
    // and a 10-byte footer when flagged
    let mut at = 0u64;
    for _ in 0..4 {
        let mut h = [0u8; 10];
        file.seek(SeekFrom::Start(at)).ok()?;
        if file.read_exact(&mut h).is_err() || &h[..3] != b"ID3" {
            break;
        }
        if h[6..10].iter().any(|&b| b >= 0x80) {
            return None;
        }
        let size = h[6..10].iter().fold(0u64, |s, &b| s << 7 | b as u64);
        at = at.checked_add(10 + size + (if h[5] & 0x10 != 0 { 10 } else { 0 }))?;
        if at >= end {
            return None;
        }
    }
    file.seek(SeekFrom::Start(at)).ok()?;
    let mut buf = Vec::new();
    file.take((SEARCH + 4096) as u64)
        .read_to_end(&mut buf)
        .ok()?;
    parse(&buf)
}

/// A Layer III frame header: its version, rate, samples a frame, length in
/// bytes, and whether a CRC and one channel follow.
struct Header {
    mpeg1: bool,
    rate: u32,
    spf: u32,
    len: usize,
    crc: bool,
    mono: bool,
}

fn header(b: &[u8]) -> Option<Header> {
    let h = u32::from_be_bytes(b.get(..4)?.try_into().ok()?);
    if h >> 21 != 0x7ff {
        return None;
    }
    let version = (h >> 19) & 3; // 3: MPEG-1, 2: MPEG-2, 0: MPEG-2.5
    if version == 1 || (h >> 17) & 3 != 1 {
        return None; // reserved, or not Layer III
    }
    let (bi, ri) = ((h >> 12) & 15, (h >> 10) & 3);
    if bi == 0 || bi == 15 || ri == 3 {
        return None; // free format, or a bad index
    }
    let mpeg1 = version == 3;
    const KBPS1: [u32; 15] = [
        0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320,
    ];
    const KBPS2: [u32; 15] = [0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160];
    let rate = [44100, 48000, 32000][ri as usize]
        >> (if mpeg1 {
            0
        } else if version == 2 {
            1
        } else {
            2
        });
    let kbps = if mpeg1 { KBPS1 } else { KBPS2 }[bi as usize];
    let spf = if mpeg1 { 1152 } else { 576 };
    let len = (spf / 8 * kbps * 1000 / rate + ((h >> 9) & 1)) as usize;
    Some(Header {
        mpeg1,
        rate,
        spf,
        len,
        crc: (h >> 16) & 1 == 0,
        mono: (h >> 6) & 3 == 3,
    })
}

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

/// The first of two frames in a row in the first SEARCH bytes, and its
/// Info frame and LAME tag if it is one.
pub fn parse(b: &[u8]) -> Option<First> {
    let (at, h) = (0..SEARCH.min(b.len())).find_map(|i| {
        let h = header(&b[i..])?;
        let next = header(b.get(i + h.len..)?)?;
        (h.len > 4 && next.mpeg1 == h.mpeg1 && next.rate == h.rate).then_some((i, h))
    })?;
    let frame = b.get(at..at + h.len)?;
    let side = match (h.mpeg1, h.mono) {
        (true, true) => 17,
        (true, false) => 32,
        (false, true) => 9,
        (false, false) => 17,
    };
    let x = 4 + (if h.crc { 2 } else { 0 }) + side;
    let mut f = First {
        rate: h.rate,
        spf: h.spf,
        info: false,
        frames: None,
        delay: None,
        padding: None,
        encoder: [0; 4],
    };
    if !matches!(frame.get(x..x + 4), Some(b"Info") | Some(b"Xing")) {
        return Some(f);
    }
    f.info = true;
    let flags = u32_at(frame, x + 4)?;
    let mut p = x + 8;
    if flags & 1 != 0 {
        f.frames = Some(u32_at(frame, p)?).filter(|&n| n > 0);
        p += 4;
    }
    p += (if flags & 2 != 0 { 4 } else { 0 }) + (if flags & 4 != 0 { 100 } else { 0 });
    p += if flags & 8 != 0 { 4 } else { 0 };
    // the LAME tag: nine bytes of the encoder's name, and 21 bytes in, the
    // delay and the padding in 12 bits each
    let Some(tag) = frame.get(p..p + 24) else {
        return Some(f);
    };
    if !tag[..9]
        .iter()
        .all(|c| c.is_ascii_graphic() || *c == b' ' || *c == 0)
        || tag[0] == 0
    {
        return Some(f);
    }
    f.encoder.copy_from_slice(&tag[..4]);
    let d = &tag[21..24];
    f.delay = Some((d[0] as u32) << 4 | (d[1] as u32) >> 4);
    f.padding = Some(((d[1] & 15) as u32) << 8 | d[2] as u32);
    Some(f)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// a Layer III header: MPEG-1 (or 2) at 44.1 (22.05) kHz, mono, 64 kbit/s
    fn head(mpeg1: bool) -> [u8; 4] {
        let v = if mpeg1 { 0b11 } else { 0b10 };
        let bi = if mpeg1 { 5 } else { 8 }; // 64 kbit/s in each table
        let h: u32 = 0x7ff << 21 | v << 19 | 1 << 17 | 1 << 16 | bi << 12 | 0 << 10 | 3 << 6;
        h.to_be_bytes()
    }

    fn frame_len(mpeg1: bool) -> usize {
        header(&head(mpeg1)).unwrap().len
    }

    /// an Info frame with every field and a LAME tag, then a frame of sound
    fn tagged(mpeg1: bool, encoder: &[u8; 9], delay: u32, padding: u32, frames: u32) -> Vec<u8> {
        let mut f = head(mpeg1).to_vec();
        f.resize(4 + (if mpeg1 { 17 } else { 9 }), 0);
        f.extend_from_slice(b"Info");
        f.extend_from_slice(&15u32.to_be_bytes());
        f.extend_from_slice(&frames.to_be_bytes());
        f.extend_from_slice(&[0; 4 + 100 + 4]);
        let mut tag = encoder.to_vec();
        tag.resize(21, 0);
        tag.extend_from_slice(&[
            (delay >> 4) as u8,
            ((delay & 15) << 4 | padding >> 8) as u8,
            padding as u8,
        ]);
        f.extend_from_slice(&tag);
        f.resize(frame_len(mpeg1), 0);
        let mut sound = head(mpeg1).to_vec();
        sound.resize(frame_len(mpeg1), 0);
        [f, sound.clone(), sound].concat()
    }

    #[test]
    fn info_frame_and_lame_tag() {
        let f = parse(&tagged(true, b"Lavc61.19", 576, 1008, 6892)).unwrap();
        assert_eq!((f.rate, f.spf, f.info), (44100, 1152, true));
        assert_eq!(
            (f.frames, f.delay, f.padding),
            (Some(6892), Some(576), Some(1008))
        );
        assert_eq!(&f.encoder, b"Lavc");
    }

    #[test]
    fn each_decoder_to_the_sound() {
        // ffmpeg's tag, MPEG-1: three minutes of sound, 6892 frames
        let f = parse(&tagged(true, b"Lavc61.19", 576, 1008, 6892)).unwrap();
        let sound = Some(180.0);
        // Media Foundation: the Info frame's silence and the delay
        let mf = cut(&f, Reader::MediaFoundation).unwrap();
        assert_eq!((mf.skip, mf.keep), ((1152.0 + 576.0) / 44100.0, sound));
        // AVFoundation reads LAME's tag, not ffmpeg's: the delay
        let av = cut(&f, Reader::AvFoundation).unwrap();
        assert_eq!((av.skip, av.keep), (576.0 / 44100.0, sound));
        // ffmpeg reads its own
        assert_eq!(cut(&f, Reader::Ffmpeg), None);
        // LAME's tag, MPEG-2: AVFoundation reads it; Media Foundation's frame is 576
        let f = parse(&tagged(false, b"LAME3.96r", 576, 1332, 26691)).unwrap();
        assert_eq!(cut(&f, Reader::AvFoundation), None);
        assert_eq!(cut(&f, Reader::Ffmpeg), None);
        let mf = cut(&f, Reader::MediaFoundation).unwrap();
        assert_eq!(mf.skip, 1152.0 / 22050.0);
    }

    #[test]
    fn untagged() {
        let mut sound = head(true).to_vec();
        sound.resize(frame_len(true), 0);
        let f = parse(&[vec![0; 7], sound.clone(), sound].concat()).unwrap();
        assert!(!f.info && f.delay.is_none());
        // ffmpeg alone leaves the decoder's delay in
        assert_eq!(cut(&f, Reader::Ffmpeg).unwrap().skip, 529.0 / 44100.0);
        assert_eq!(cut(&f, Reader::AvFoundation), None);
        assert_eq!(cut(&f, Reader::MediaFoundation), None);
    }

    #[test]
    fn not_mp3() {
        // an ADTS AAC header has Layer 0; a lone header, no second frame
        assert!(parse(&[0xff, 0xf1, 0x50, 0x80, 0, 0, 0, 0]).is_none());
        assert!(parse(&head(true)).is_none());
        assert!(parse(b"RIFF\0\0\0\0WAVEfmt ").is_none());
    }
}

#[cfg(test)]
mod files {
    /// `PKAPP_MP3_FILES=a.mp3:b.mp3 cargo test mp3_files -- --ignored --nocapture`: each file's first frame
    #[test]
    #[ignore]
    fn mp3_files() {
        for p in std::env::var("PKAPP_MP3_FILES")
            .unwrap_or_default()
            .split(':')
            .filter(|s| !s.is_empty())
        {
            println!("{p}: {:?}", super::first(std::path::Path::new(p)));
        }
    }
}
