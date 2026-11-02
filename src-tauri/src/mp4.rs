//! The edit an MP4's first audio track carries, which Media Foundation does
//! not apply (decode_windows.c reads every sample the track holds).
//!
//! An AAC encoder starts with samples of its own, its priming: 1024 from
//! ffmpeg's, 2112 from Apple's. The file's edit list says where the sound
//! itself starts in the track and how long it lasts (or, from Apple's
//! encoders, an iTunSMPB tag), and AVFoundation and ffmpeg play the track
//! from there; Media Foundation plays all of it, so
//! every time in a transcript of it came out late by the priming, 64 ms for
//! ffmpeg's encoder at 16 kHz, and the length long by the padding at the
//! end (the Windows test, 2026-10-22: speech-2997.mp4 decoded to 961,536
//! samples at 16 kHz where the speech is 960,000).
//!
//! The file is untrusted: every size is checked against what holds it,
//! the boxes are read to a bounded depth, and the movie box read whole only
//! up to MOOV_MAX. Anything unexpected is no edit, and the track as decoded.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// Larger than any moov of a file a person transcribes; a bigger one is not read.
const MOOV_MAX: u64 = 64 << 20;

/// The most silence an empty edit may put before the sound, in seconds.
/// apply() makes it as samples, 64 KB a second at 16 kHz, where the day the
/// edit's other times may run to would be 5.5 GB; a file that delays its
/// sound longer than this is no edit.
const LEAD_MAX: f64 = 600.0;

/// What the edit list makes of the decoded track, in seconds: silence before
/// it (an empty edit: the sound starts that late), the decoded sound to drop
/// from its start (the priming), and how much to keep after that, if it says.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Edit {
    pub lead: f64,
    pub skip: f64,
    pub keep: Option<f64>,
}

/// The first audio track's edit, if the file is an MP4 with one that is one
/// segment of the track, after any empty ones.
pub fn audio_edit(path: &Path) -> Option<Edit> {
    audio_edit_in(&mut File::open(path).ok()?)
}

/// The same of a file already open, as Android hands one over (decode.rs).
pub fn audio_edit_in(f: &mut File) -> Option<Edit> {
    let end = f.metadata().ok()?.len();
    let mut at = 0u64;
    while at + 8 <= end {
        f.seek(SeekFrom::Start(at)).ok()?;
        let mut h = [0u8; 16];
        f.read_exact(&mut h[..8]).ok()?;
        let (mut size, head) = (u32::from_be_bytes(h[..4].try_into().ok()?) as u64, 8u64);
        let head = match size {
            1 => {
                f.read_exact(&mut h[8..16]).ok()?;
                size = u64::from_be_bytes(h[8..16].try_into().ok()?);
                16
            }
            0 => {
                size = end - at;
                head
            }
            _ => head,
        };
        if size < head || at.checked_add(size)? > end {
            return None;
        }
        if &h[4..8] == b"moov" {
            let len = size - head;
            if len > MOOV_MAX {
                return None;
            }
            let mut moov = vec![0u8; len as usize];
            f.read_exact(&mut moov).ok()?;
            return from_moov(&moov);
        }
        at += size;
    }
    None
}

/// The boxes in b, each as (type, content): None for one that does not fit.
fn boxes(mut b: &[u8]) -> Option<Vec<([u8; 4], &[u8])>> {
    let mut out = Vec::new();
    while b.len() >= 8 {
        let size = u32::from_be_bytes(b[..4].try_into().ok()?) as u64;
        let kind: [u8; 4] = b[4..8].try_into().ok()?;
        let (size, head) = match size {
            1 if b.len() >= 16 => (u64::from_be_bytes(b[8..16].try_into().ok()?), 16),
            1 => return None,
            0 => (b.len() as u64, 8),
            _ => (size, 8),
        };
        if size < head || size > b.len() as u64 {
            return None;
        }
        out.push((kind, &b[head as usize..size as usize]));
        b = &b[size as usize..];
    }
    Some(out)
}

fn child<'a>(b: &'a [u8], kind: &[u8; 4]) -> Option<&'a [u8]> {
    boxes(b)?
        .into_iter()
        .find(|(k, _)| k == kind)
        .map(|(_, c)| c)
}

fn u32_at(b: &[u8], i: usize) -> Option<u32> {
    Some(u32::from_be_bytes(b.get(i..i + 4)?.try_into().ok()?))
}

fn u64_at(b: &[u8], i: usize) -> Option<u64> {
    Some(u64::from_be_bytes(b.get(i..i + 8)?.try_into().ok()?))
}

/// mvhd's and mdhd's timescale: after the version and flags, two times of
/// 4 bytes (version 0) or 8 (version 1)
fn timescale(full: &[u8]) -> Option<u32> {
    let ts = u32_at(full, if *full.first()? == 1 { 20 } else { 12 })?;
    (ts > 0).then_some(ts)
}

fn from_moov(moov: &[u8]) -> Option<Edit> {
    let movie = timescale(child(moov, b"mvhd")?)? as f64;
    for (kind, trak) in boxes(moov)? {
        if &kind != b"trak" {
            continue;
        }
        let mdia = child(trak, b"mdia")?;
        // hdlr: version and flags, pre_defined, then the handler's type
        if child(mdia, b"hdlr").and_then(|h| h.get(8..12)) != Some(&b"soun"[..]) {
            continue;
        }
        // the first audio track decides, edit or none
        let media = timescale(child(mdia, b"mdhd")?)? as f64;
        let Some(elst) = child(trak, b"edts").and_then(|e| child(e, b"elst")) else {
            return smpb(moov, trak).map(|(delay, len)| Edit {
                lead: 0.0,
                skip: delay / media,
                keep: Some(len / media),
            });
        };
        let v1 = *elst.first()? == 1;
        let (n, size) = (u32_at(elst, 4)? as usize, if v1 { 20 } else { 12 });
        let mut lead = 0.0;
        let mut found = None;
        for i in 0..n.min(64) {
            let e = 8 + i * size;
            let (dur, time) = if v1 {
                (u64_at(elst, e)? as f64, u64_at(elst, e + 8)? as i64)
            } else {
                (u32_at(elst, e)? as f64, u32_at(elst, e + 4)? as i32 as i64)
            };
            match (time, found) {
                (-1, None) => lead += dur / movie, // an empty edit: nothing plays for dur
                (t, None) if t >= 0 => found = Some((t as f64 / media, dur / movie)),
                _ => return None, // more than one segment: left as it is
            }
        }
        let (skip, keep) = found?;
        let ok = |s: f64| s.is_finite() && (0.0..=86400.0).contains(&s);
        return (ok(lead) && lead <= LEAD_MAX && ok(skip) && ok(keep)).then_some(Edit {
            lead,
            skip,
            keep: (keep > 0.0).then_some(keep),
        });
    }
    None
}

/// Apple's gapless tag, iTunSMPB, in udta/meta/ilst's "----" items (the
/// movie's or the track's): its second and fourth hex fields, the priming
/// and the sound's length in samples of the track's timescale, as in
/// " 00000000 00000840 000002C0 000000000015F900 ..." (2112 and 1,440,000
/// at 24 kHz, HE-AAC's core rate, from afconvert).
fn smpb(moov: &[u8], trak: &[u8]) -> Option<(f64, f64)> {
    for holder in [moov, trak] {
        // meta is a full box: its version and flags before its boxes
        let Some(ilst) = child(holder, b"udta")
            .and_then(|u| child(u, b"meta"))
            .and_then(|m| m.get(4..))
            .and_then(|m| child(m, b"ilst"))
        else {
            continue;
        };
        for (kind, item) in boxes(ilst)? {
            if &kind != b"----"
                || child(item, b"name").and_then(|n| n.get(4..)) != Some(&b"iTunSMPB"[..])
            {
                continue;
            }
            // data: a type and a locale, then the text
            let text = std::str::from_utf8(child(item, b"data")?.get(8..)?).ok()?;
            let f: Vec<u64> = text
                .split_whitespace()
                .take(4)
                .map(|h| u64::from_str_radix(h, 16).ok())
                .collect::<Option<_>>()?;
            return (f.len() == 4 && f[3] > 0).then(|| (f[1] as f64, f[3] as f64));
        }
    }
    None
}

/// x, at rate, as the edit presents it. The silence before it is the one
/// length here that x does not bound, so it is held to LEAD_MAX, and if
/// even that much memory can't be had, x is left as decoded.
pub fn apply(x: Vec<f32>, e: &Edit, rate: f64) -> Vec<f32> {
    let skip = ((e.skip * rate).round() as usize).min(x.len());
    let keep = e.keep.map_or(x.len() - skip, |k| {
        ((k * rate).round() as usize).min(x.len() - skip)
    });
    let lead = (e.lead.clamp(0.0, LEAD_MAX) * rate).round() as usize;
    let mut y = Vec::new();
    if y.try_reserve_exact(lead.saturating_add(keep)).is_err() {
        return x;
    }
    y.resize(lead, 0.0);
    y.extend_from_slice(&x[skip..skip + keep]);
    y
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bx(kind: &[u8; 4], content: &[u8]) -> Vec<u8> {
        let mut v = ((content.len() + 8) as u32).to_be_bytes().to_vec();
        v.extend_from_slice(kind);
        v.extend_from_slice(content);
        v
    }

    /// a moov with one audio track at 16 kHz, its edits (duration, media time) in a 1000 Hz movie
    fn moov(edits: &[(u32, i32)], handler: &[u8; 4]) -> Vec<u8> {
        let mut mvhd = vec![0u8; 20];
        mvhd[12..16].copy_from_slice(&1000u32.to_be_bytes());
        let mut mdhd = vec![0u8; 20];
        mdhd[12..16].copy_from_slice(&16000u32.to_be_bytes());
        let mut hdlr = vec![0u8; 20];
        hdlr[8..12].copy_from_slice(handler);
        let mut elst = vec![0u8; 4];
        elst.extend_from_slice(&(edits.len() as u32).to_be_bytes());
        for &(d, t) in edits {
            elst.extend_from_slice(&d.to_be_bytes());
            elst.extend_from_slice(&t.to_be_bytes());
            elst.extend_from_slice(&0x0001_0000u32.to_be_bytes());
        }
        let mdia = [bx(b"mdhd", &mdhd), bx(b"hdlr", &hdlr)].concat();
        let trak = [bx(b"edts", &bx(b"elst", &elst)), bx(b"mdia", &mdia)].concat();
        [bx(b"mvhd", &mvhd), bx(b"trak", &trak)].concat()
    }

    #[test]
    fn priming_and_padding() {
        // ffmpeg's: 1024 samples of priming, 60 s of sound
        let e = from_moov(&moov(&[(60000, 1024)], b"soun")).unwrap();
        assert_eq!(
            e,
            Edit {
                lead: 0.0,
                skip: 0.064,
                keep: Some(60.0)
            }
        );
        let x: Vec<f32> = (0..961_536).map(|i| i as f32).collect();
        let y = apply(x, &e, 16000.0);
        assert_eq!((y.len(), y[0]), (960_000, 1024.0));
    }

    #[test]
    fn empty_edit_then_sound() {
        let e = from_moov(&moov(&[(500, -1), (2000, 0)], b"soun")).unwrap();
        assert_eq!(
            e,
            Edit {
                lead: 0.5,
                skip: 0.0,
                keep: Some(2.0)
            }
        );
        let y = apply(vec![1.0; 48000], &e, 16000.0);
        assert_eq!((y.len(), y[7999], y[8000]), (40000, 0.0, 1.0));
    }

    #[test]
    fn itunes_gapless_tag() {
        // a track with no edit list and afconvert's tag in the movie's udta
        let mut m = moov(&[], b"soun");
        let trak_at = m.windows(4).position(|w| w == b"trak").unwrap() - 4;
        let mut trak = m.split_off(trak_at);
        // the track without its edts
        let edts_at = trak.windows(4).position(|w| w == b"edts").unwrap() - 4;
        let edts_len = u32::from_be_bytes(trak[edts_at..edts_at + 4].try_into().unwrap()) as usize;
        trak.drain(edts_at..edts_at + edts_len);
        let tl = trak.len() as u32;
        trak[..4].copy_from_slice(&tl.to_be_bytes());
        let text = b" 00000000 00000840 000002C0 000000000015F900 00000000";
        let mut data = vec![0u8; 8];
        data.extend_from_slice(text);
        let item = [
            bx(b"mean", b"\0\0\0\0com.apple.iTunes"),
            bx(b"name", b"\0\0\0\0iTunSMPB"),
            bx(b"data", &data),
        ]
        .concat();
        let meta = [vec![0u8; 4], bx(b"ilst", &bx(b"----", &item))].concat();
        m.extend_from_slice(&bx(b"udta", &bx(b"meta", &meta)));
        m.extend_from_slice(&trak);
        let e = from_moov(&m).unwrap();
        assert_eq!(
            e,
            Edit {
                lead: 0.0,
                skip: 2112.0 / 16000.0,
                keep: Some(1_440_000.0 / 16000.0)
            }
        );
    }

    #[test]
    fn none_for_what_it_does_not_know() {
        assert_eq!(from_moov(&moov(&[(1000, 0), (1000, 2000)], b"soun")), None); // two segments
        assert_eq!(from_moov(&moov(&[(1000, 0)], b"vide")), None); // no audio track
        assert_eq!(from_moov(&moov(&[], b"soun")), None); // no edit
        let mut bad = moov(&[(60000, 1024)], b"soun");
        bad.truncate(bad.len() - 5);
        assert_eq!(from_moov(&bad), None); // cut short
        assert_eq!(from_moov(&[0, 0, 0, 1, b'm', b'v', b'h', b'd']), None); // a size it cannot have
                                                                            // a sound delayed past LEAD_MAX, 601 s
        assert_eq!(from_moov(&moov(&[(601_000, -1), (2000, 0)], b"soun")), None);
    }

    #[test]
    fn lead_held_to_its_most() {
        // a day of silence asked for, LEAD_MAX's made: at 10 Hz, 6000 samples
        let e = Edit {
            lead: 86400.0,
            skip: 0.0,
            keep: None,
        };
        let y = apply(vec![1.0; 10], &e, 10.0);
        assert_eq!((y.len(), y[5999], y[6000]), (6010, 0.0, 1.0));
        // nothing for a lead that is not a number
        let e = Edit {
            lead: f64::NAN,
            ..e
        };
        assert_eq!(apply(vec![1.0; 10], &e, 10.0).len(), 10);
    }
}

#[cfg(test)]
mod files {
    /// `PKAPP_MP4_FILES=a.mp4:b.m4a cargo test probe_files -- --ignored --nocapture`: each file's edit
    #[test]
    #[ignore]
    fn probe_files() {
        for p in std::env::var("PKAPP_MP4_FILES")
            .unwrap_or_default()
            .split(':')
            .filter(|s| !s.is_empty())
        {
            println!("{p}: {:?}", super::audio_edit(std::path::Path::new(p)));
        }
    }
}
