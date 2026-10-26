//! A recording from a time on, for a player that does not seek it to where
//! it is asked: the bytes of a file that begins there, cut from the
//! original's, which the player plays from its start, so that its decoder
//! starts afresh at the cut and its clock at 0 there.
//!
//! Android's player, handed the original (2026-10-26, a 4442 s recording,
//! twelve seeks each on Android 16's emulator and on a Galaxy S25 Ultra's
//! Android 17, decoded after the seek against decoded from the start;
//! ~/android-study/mc/seekdecode): an MP3 with a Xing or Info table seeks
//! by its hundred points and lands up to 15 s from where it says it is, a
//! VBR one without the table by its bitrate, minutes off; Android 16 lands
//! an Ogg file up to 7.5 s late, its table of pages thinned to 512; the
//! S25's Opus decoder plays 80 ms early after any seek. FLAC, MP4's AAC
//! and WAV land where asked, and are played as they are.
//!
//! A view of an MP3 is its frames from two before the one the time is in
//! (a frame's data may begin in the frame before it: the bit reservoir),
//! without its ID3 tag and its Xing or Info frame. A view of an Ogg file
//! is its header pages, then its pages from the one the time is in: for
//! Opus from the packet 80 ms before it, the decoder's settling (RFC 7845,
//! 4.6), the rest of that packet's page made a page of its own; for Vorbis
//! from the page's start. Played from its start, each view was the
//! from-start decode's samples from where start puts its first, on both
//! devices, up to a constant its file's headers give (mc/views.py,
//! headdecode.c, viewcheck.py), which start takes in.
//!
//! The file is untrusted: frames and pages are read by their own lengths,
//! each checked, and a file that stops making sense ends its index there,
//! the views of a time after it cut at its last good frame or page.

use crate::mp3;
use std::fs::File;
use std::io;
use std::sync::Arc;

/// A recording's frames or pages, read once for every view of it.
pub struct Index {
    file: File,
    len: u64,
    kind: Kind,
}

enum Kind {
    Mp3(Frames),
    Opus(Pages),
    Vorbis(Pages),
}

/// An MP3's frames of sound, every CHECK-th one's offset kept.
struct Frames {
    marks: Vec<u64>,
    count: u64,
    mpeg1: bool,
    rate: u32,
    spf: u32,
    /// The encoder's delay the LAME tag gives: Android's decoder takes it
    /// out of the whole file, not out of a view, which has no tag. A view
    /// of a tagged file's frame f starts at f * spf - delay samples into
    /// the recording: 576 in ffmpeg's tags and LAME's, a view 36.000 ms
    /// earlier than its frame at 16 kHz and 13.061 ms at 44.1 kHz, on both
    /// devices; an untagged one's at its frame (mc/views.py's views).
    delay: u32,
}

/// The first bytes of files of other kinds Android plays: WAV (and its
/// RIFX and RF64), AIFF, FLAC, Matroska and WebM, AMR, CAF, WavPack, APE;
/// MP4's have "ftyp" four bytes in.
const OTHERS: [&[u8]; 10] = [
    b"RIFF",
    b"RIFX",
    b"RF64",
    b"FORM",
    b"fLaC",
    b"\x1a\x45\xdf\xa3",
    b"#!AMR",
    b"caff",
    b"wvpk",
    b"MAC ",
];

/// Every CHECK-th frame's offset is kept; a view walks to its frame from
/// the last one kept, at most CHECK - 1 frames. A 74-minute MP3 at 44.1
/// kHz has 192,000 frames.
const CHECK: u64 = 16;

/// An Ogg file's audio pages, after its header pages.
struct Pages {
    /// where the first audio page is: the header pages are the file's
    /// bytes before it, which every view begins with
    head: u64,
    pages: Arc<Vec<Page>>,
    rate: u32,
    /// Opus's pre-skip: the samples a decoder drops from the stream's
    /// start, which a granule counts
    preskip: u32,
    /// Vorbis's long block, in samples
    long: u32,
}

#[derive(Clone, Copy)]
struct Page {
    at: u64,
    size: usize,
    /// the samples decoded once the last packet that ends on it is, from
    /// the stream's start; -1 where none ends on it
    granule: i64,
    /// it begins with the rest of a packet begun on the page before
    rest: bool,
}

/// The view: the file's bytes before head, then page, then the file's
/// from body on, its pages' granules lowered where shift says.
pub struct View {
    file: File,
    /// Where its first sample is in the recording, in seconds: the time a
    /// player gives in the view, plus start, is the recording's.
    pub start: f64,
    head: u64,
    page: Vec<u8>,
    body: u64,
    len: u64,
    shift: Option<Shift>,
}

/// An Opus view's pages, each granule lowered by the count at the view's
/// first packet, so that the view is a stream from 0. Android's player
/// counted a view of the file's own granules from 0 for its first page
/// and from the file's start for the next, a seek to 134.7 s showing
/// 134.4 s half a second in (Android 16's emulator, 2026-10-26), where
/// MP3 and Vorbis views count from 0 throughout.
struct Shift {
    pages: Arc<Vec<Page>>,
    /// the page the body begins with
    from: usize,
    by: i64,
}

impl Index {
    /// The index of an MP3 or an Ogg file of Opus or Vorbis; None for one
    /// of another kind, which is played as it is. file is read from its
    /// start, and kept for the views.
    pub fn read(mut file: File) -> Option<Index> {
        let len = file.metadata().ok()?.len();
        let mut magic = [0u8; 12];
        read_at(&file, &mut magic, 0).ok()?;
        let kind = if &magic[..4] == b"OggS" {
            ogg(&mut Window::new(&file, len))?
        } else if OTHERS.iter().any(|m| magic.starts_with(m)) || &magic[4..8] == b"ftyp" {
            // a container of another kind can hold what looks like two
            // MP3 frames in a row, as 16-bit PCM can
            return None;
        } else {
            Kind::Mp3(frames(&mut file, len)?)
        };
        Some(Index { file, len, kind })
    }

    /// The view from t seconds into the recording, held to its length.
    pub fn view(&self, t: f64) -> io::Result<View> {
        let t = if t.is_finite() { t.max(0.0) } else { 0.0 };
        let file = self.file.try_clone()?;
        let mut v = View {
            file,
            start: 0.0,
            head: 0,
            page: Vec::new(),
            body: 0,
            len: self.len,
            shift: None,
        };
        match &self.kind {
            Kind::Mp3(f) => {
                let k = ((t * f.rate as f64 + f.delay as f64) / f.spf as f64) as u64;
                let k = k.saturating_sub(2).min(f.count - 1);
                v.body = f.offset(&self.file, self.len, k)?;
                v.start = (k as f64 * f.spf as f64 - f.delay as f64) / f.rate as f64;
            }
            Kind::Opus(p) => opus_view(p, &self.file, t, &mut v)?,
            Kind::Vorbis(p) => vorbis_view(p, t, &mut v),
        }
        Ok(v)
    }
}

impl View {
    pub fn size(&self) -> u64 {
        self.head + self.page.len() as u64 + (self.len - self.body)
    }

    /// The view's bytes from pos into buf, as many as there are: 0 at its
    /// end.
    pub fn read_at(&self, pos: u64, buf: &mut [u8]) -> io::Result<usize> {
        let page_end = self.head + self.page.len() as u64;
        if pos < self.head {
            let n = buf.len().min((self.head - pos) as usize);
            read_at(&self.file, &mut buf[..n], pos)?;
            Ok(n)
        } else if pos < page_end {
            let from = (pos - self.head) as usize;
            let n = buf.len().min(self.page.len() - from);
            buf[..n].copy_from_slice(&self.page[from..from + n]);
            Ok(n)
        } else {
            let at = self.body + (pos - page_end);
            if at >= self.len {
                return Ok(0);
            }
            if let Some(s) = &self.shift {
                return s.read_at(&self.file, at, buf);
            }
            let n = buf.len().min((self.len - at) as usize);
            read_at(&self.file, &mut buf[..n], at)?;
            Ok(n)
        }
    }
}

impl Shift {
    /// The pages' bytes from the file's offset at into buf, each page read
    /// whole for its granule and checksum to be made anew.
    fn read_at(&self, file: &File, at: u64, buf: &mut [u8]) -> io::Result<usize> {
        let pages = &self.pages[self.from..];
        let mut i = pages.partition_point(|p| p.at + p.size as u64 <= at);
        let (mut done, mut at) = (0, at);
        while done < buf.len() && i < pages.len() {
            let p = pages[i];
            let mut b = vec![0u8; p.size];
            read_at(file, &mut b, p.at)?;
            if p.granule >= 0 {
                b[6..14].copy_from_slice(&(p.granule - self.by).to_le_bytes());
                b[22..26].fill(0);
                let crc = ogg_crc(&b);
                b[22..26].copy_from_slice(&crc.to_le_bytes());
            }
            let from = (at - p.at) as usize;
            let n = (buf.len() - done).min(p.size - from);
            buf[done..done + n].copy_from_slice(&b[from..from + n]);
            done += n;
            at += n as u64;
            i += 1;
        }
        Ok(done)
    }
}

/// All of buf from the file at offset, or an error.
fn read_at(file: &File, buf: &mut [u8], offset: u64) -> io::Result<()> {
    #[cfg(unix)]
    {
        std::os::unix::fs::FileExt::read_exact_at(file, buf, offset)
    }
    #[cfg(windows)]
    {
        let mut done = 0;
        while done < buf.len() {
            let n = std::os::windows::fs::FileExt::seek_read(
                file,
                &mut buf[done..],
                offset + done as u64,
            )?;
            if n == 0 {
                return Err(io::ErrorKind::UnexpectedEof.into());
            }
            done += n;
        }
        Ok(())
    }
}

/// A file read a window of its bytes at a time, for the scans of its
/// frames and pages: one read a window, not one a frame, where a 74-minute
/// MP3 has 192,000.
struct Window<'a> {
    file: &'a File,
    len: u64,
    at: u64,
    buf: Vec<u8>,
}

const WINDOW: usize = 1 << 16;

impl<'a> Window<'a> {
    fn new(file: &'a File, len: u64) -> Window<'a> {
        Window {
            file,
            len,
            at: 0,
            buf: Vec::new(),
        }
    }

    /// The n bytes at pos, None past the file's end.
    fn get(&mut self, pos: u64, n: usize) -> Option<&[u8]> {
        let end = pos.checked_add(n as u64).filter(|&e| e <= self.len)?;
        if pos < self.at || end > self.at + self.buf.len() as u64 {
            let size = (WINDOW.max(n) as u64).min(self.len - pos) as usize;
            self.buf.resize(size, 0);
            read_at(self.file, &mut self.buf, pos).ok()?;
            self.at = pos;
        }
        let i = (pos - self.at) as usize;
        Some(&self.buf[i..i + n])
    }

    /// Up to n bytes at pos, as many as the file has.
    fn some(&mut self, pos: u64, n: usize) -> Option<&[u8]> {
        let n = n.min(self.len.saturating_sub(pos) as usize);
        self.get(pos, n)
    }
}

/// The frame after a frame ends at `at`: there, if one like the first is,
/// or else the first of two in a row in the bytes that follow, as the
/// first was found (mp3.rs, parse); None at the end or past what that
/// finds. Its offset and length.
fn frame_at(w: &mut Window, at: u64, mpeg1: bool, rate: u32) -> Option<(u64, u64)> {
    let len = w.len;
    if let Some(f) = w.get(at, 4).and_then(mp3::header) {
        if f.mpeg1 == mpeg1 && f.rate == rate {
            return (at + f.len as u64 <= len).then_some((at, f.len as u64));
        }
    }
    let (i, f) = mp3::two_in_a_row(w.some(at, 8192)?)?;
    (f.mpeg1 == mpeg1 && f.rate == rate).then_some((at + i as u64, f.len as u64))
}

fn frames(file: &mut File, len: u64) -> Option<Frames> {
    let l = mp3::located(file)?;
    let mut w = Window::new(file, len);
    let (mpeg1, rate, spf) = (l.header.mpeg1, l.header.rate, l.header.spf);
    // an Info or Xing frame holds no sound, and is no view's
    let mut at = l.at + if l.first.info { l.header.len as u64 } else { 0 };
    let mut f = Frames {
        marks: Vec::new(),
        count: 0,
        mpeg1,
        rate,
        spf,
        delay: if l.first.info {
            l.first.delay.unwrap_or(0)
        } else {
            0
        },
    };
    while let Some((frame, n)) = frame_at(&mut w, at, mpeg1, rate) {
        if f.count % CHECK == 0 {
            f.marks.push(frame);
        }
        f.count += 1;
        at = frame + n;
    }
    (f.count > 0).then_some(f)
}

impl Frames {
    /// The offset of frame k, k < count, walked to as frames() found it.
    fn offset(&self, file: &File, len: u64, k: u64) -> io::Result<u64> {
        let mut w = Window::new(file, len);
        let mut at = self.marks[(k / CHECK) as usize];
        for _ in 0..k % CHECK {
            let (frame, n) = frame_at(&mut w, at, self.mpeg1, self.rate)
                .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidData))?;
            at = frame + n;
        }
        frame_at(&mut w, at, self.mpeg1, self.rate)
            .map(|(frame, _)| frame)
            .ok_or_else(|| io::ErrorKind::InvalidData.into())
    }
}

/// An Ogg page's header at at: its size, granule, whether it begins with
/// a packet's rest, its serial, and its segments' lengths.
fn page_at(w: &mut Window, at: u64) -> Option<(Page, u32, Vec<u8>)> {
    let h: [u8; 27] = w.get(at, 27)?.try_into().ok()?;
    if &h[..4] != b"OggS" || h[4] != 0 {
        return None;
    }
    let lacing = w.get(at + 27, h[26] as usize)?.to_vec();
    let size = 27 + lacing.len() + lacing.iter().map(|&l| l as usize).sum::<usize>();
    if at + size as u64 > w.len {
        return None;
    }
    let page = Page {
        at,
        size,
        granule: i64::from_le_bytes(h[6..14].try_into().unwrap()),
        rest: h[5] & 1 != 0,
    };
    Some((
        page,
        u32::from_le_bytes(h[14..18].try_into().unwrap()),
        lacing,
    ))
}

/// An Ogg file's pages, if it is one stream of Opus or of Vorbis whose
/// sound begins a page of its own, as both formats have it (RFC 7845, 3;
/// Vorbis I, A.2): two header packets for Opus, three for Vorbis, then
/// the pages of sound. A file of more streams, or chained, is played as
/// it is.
fn ogg(w: &mut Window) -> Option<Kind> {
    let (first, serial, lacing) = page_at(w, 0)?;
    let packet = w
        .get(
            27 + lacing.len() as u64,
            30.min(first.size - 27 - lacing.len()),
        )?
        .to_vec();
    let opus = packet.starts_with(b"OpusHead") && packet.len() >= 19;
    let vorbis = packet.starts_with(b"\x01vorbis") && packet.len() >= 30;
    if !opus && !vorbis {
        return None;
    }
    let (rate, preskip, long) = if opus {
        (
            48000,
            u16::from_le_bytes([packet[10], packet[11]]) as u32,
            0,
        )
    } else {
        let rate = u32::from_le_bytes(packet[12..16].try_into().unwrap());
        (rate, 0, 1u32 << (packet[28] >> 4))
    };
    if rate == 0 || !(64..=8192).contains(&long) && vorbis {
        return None;
    }
    let mut headers = if opus { 2 } else { 3 };
    let mut p = Pages {
        head: 0,
        pages: Arc::new(Vec::new()),
        rate,
        preskip,
        long,
    };
    let mut pages = Vec::new();
    let mut at = 0;
    while let Some((page, s, lacing)) = page_at(w, at) {
        if s != serial {
            return None;
        }
        if headers > 0 {
            // the packets that end on a page: its segments under 255
            let ends = lacing.iter().filter(|&&l| l < 255).count();
            if ends > headers || ends == headers && lacing.last() == Some(&255) {
                return None; // sound on a header's page
            }
            headers -= ends;
            if headers == 0 {
                p.head = at + page.size as u64;
            }
        } else {
            pages.push(page);
        }
        at += page.size as u64;
    }
    if pages.is_empty() {
        return None;
    }
    p.pages = Arc::new(pages);
    Some(if opus { Kind::Opus(p) } else { Kind::Vorbis(p) })
}

impl Pages {
    /// The page the sample at s (a granule's count) is decoded by: the
    /// first whose granule passes it, or the last.
    fn holding(&self, s: i64) -> usize {
        self.pages
            .iter()
            .position(|p| p.granule > s)
            .unwrap_or(self.pages.len() - 1)
    }

    /// The granule before page j's: the samples decoded before its first
    /// packet that begins on it, 0 before the first.
    fn before(&self, j: usize) -> i64 {
        self.pages[..j]
            .iter()
            .rev()
            .map(|p| p.granule)
            .find(|&g| g >= 0)
            .unwrap_or(0)
    }
}

/// A Vorbis view from the page t is decoded by, back past pages that begin
/// with a packet's rest. A decoder makes nothing of the first packet it
/// reads, which only begins the overlap its next one finishes: a view's
/// first sample is the first packet's last half and the one before's, which
/// for two long blocks is a long block's half after the page before's
/// granule, 21.333 ms at 48 kHz with 2048-sample blocks on both devices
/// (mc/views.py, viewcheck.py); a short block there makes it 9 to 19 ms
/// less, which is taken as it comes.
fn vorbis_view(p: &Pages, t: f64, v: &mut View) {
    let mut j = p.holding((t * p.rate as f64) as i64);
    while j > 0 && p.pages[j].rest {
        j -= 1;
    }
    v.head = p.head;
    v.body = p.pages[j].at;
    // the first page is the file's, played as the whole file is
    v.start = if j == 0 {
        0.0
    } else {
        (p.before(j) + p.long as i64 / 2) as f64 / p.rate as f64
    };
}

/// An Opus packet's samples at 48 kHz, from its first byte and, for a
/// packet of several frames, its second (RFC 6716, 3.1).
fn opus_samples(packet: &[u8]) -> Option<i64> {
    let toc = *packet.first()?;
    let config = toc >> 3;
    let frame = match config {
        0..=11 => [480, 960, 1920, 2880][(config & 3) as usize],
        12..=15 => [480, 960][(config & 1) as usize],
        _ => [120, 240, 480, 960][(config & 3) as usize],
    };
    let frames = match toc & 3 {
        0 => 1,
        1 | 2 => 2,
        _ => (*packet.get(1)? & 0x3f) as i64,
    };
    Some(frame * frames)
}

/// An Opus view from the packet 80 ms before t, that page's rest a page of
/// its own, every granule after lowered by the count at that packet
/// (Shift), so that the view is a stream from 0. The view's OpusHead keeps
/// the pre-skip, which its decoder takes from the view's start as the
/// whole file's took from its own: the view's first sample heard is the
/// one at its first packet's granule count, without the pre-skip taken
/// off, 6.5 ms (312 samples) later than views from a page's start had it
/// with it taken off (mc/views.py, viewcheck.py). A file whose first
/// packet does not start at granule 0 (RFC 7845, 4.5, a stream begun
/// mid-way) is taken as if it did, but for the view of its first page.
fn opus_view(p: &Pages, file: &File, t: f64, v: &mut View) -> io::Result<()> {
    v.head = p.head;
    let target = ((t - 0.08).max(0.0) * 48000.0) as i64 + p.preskip as i64;
    let mut j = p.holding(target);
    // the packets that begin on page j, each's offset in it and granule
    // count at its start, timed back from the page's granule
    loop {
        let page = p.pages[j];
        let mut b = vec![0u8; page.size];
        read_at(file, &mut b, page.at)?;
        let nseg = b[26] as usize;
        let lacing = &b[27..27 + nseg];
        let mut starts = Vec::new(); // (segment, byte offset in the page)
        let mut ends = Vec::new(); // (byte offset of the packet's start, its end)
        let (mut seg, mut at) = (0, 27 + nseg);
        // the rest of a packet begun before: its segments up to its end
        if page.rest {
            while seg < nseg && lacing[seg] == 255 {
                at += 255;
                seg += 1;
            }
            if seg < nseg {
                at += lacing[seg] as usize;
                seg += 1;
            }
        }
        while seg < nseg {
            starts.push((seg, at));
            let from = at;
            while seg < nseg && lacing[seg] == 255 {
                at += 255;
                seg += 1;
            }
            if seg < nseg {
                at += lacing[seg] as usize;
                seg += 1;
                ends.push((from, at));
            }
        }
        // ends are the packets that end on it: back from its granule
        let mut count = page.granule;
        let mut at_start = vec![0i64; starts.len()];
        let complete = ends.len();
        if starts.len() > complete {
            // the last begins here and ends on a page after: it starts at the granule
            at_start[complete] = count;
        }
        let mut ok = page.granule >= 0 || complete == 0;
        for k in (0..complete).rev() {
            let (from, to) = ends[k];
            match opus_samples(&b[from..to]) {
                Some(n) => count -= n,
                None => ok = false,
            }
            at_start[k] = count;
        }
        if page.granule < 0 && complete == 0 && !starts.is_empty() {
            // no packet ends on it: its one packet starts where the last page left off
            at_start[0] = p.before(j);
        }
        let k = (0..starts.len()).rev().find(|&k| at_start[k] <= target);
        if let (true, Some(k)) = (ok, k) {
            let by = at_start[k];
            let whole = k == 0 && !page.rest;
            if whole {
                v.body = page.at;
            } else {
                v.page = repage(
                    &b,
                    starts[k].0,
                    starts[k].1,
                    (complete > k).then(|| page.granule - by),
                );
                v.body = page.at + page.size as u64;
            }
            // the first page as it is is the file, played as the whole file is
            if j == 0 && whole {
                return Ok(());
            }
            let last = p.pages[p.pages.len() - 1];
            v.len = last.at + last.size as u64;
            v.shift = Some(Shift {
                pages: p.pages.clone(),
                from: j + !whole as usize,
                by,
            });
            v.start = by as f64 / 48000.0;
            return Ok(());
        }
        if j == 0 {
            v.body = page.at;
            v.start = 0.0;
            return Ok(());
        }
        j -= 1;
    }
}

/// Page b from its segment seg on, which begins a packet at byte at: its
/// header with the segments left, granule where a packet still ends on it,
/// else -1, which says none does, and its checksum made anew.
fn repage(b: &[u8], seg: usize, at: usize, granule: Option<i64>) -> Vec<u8> {
    let nseg = b[26] as usize;
    let mut out = Vec::with_capacity(b.len() - at + 27 + nseg - seg);
    out.extend_from_slice(&b[..27]);
    out[5] &= !1; // it begins with a packet; an end-of-stream flag stays
    out[6..14].copy_from_slice(&granule.unwrap_or(-1).to_le_bytes());
    out[22..26].fill(0);
    out[26] = (nseg - seg) as u8;
    out.extend_from_slice(&b[27 + seg..27 + nseg]);
    out.extend_from_slice(&b[at..]);
    let crc = ogg_crc(&out);
    out[22..26].copy_from_slice(&crc.to_le_bytes());
    out
}

/// Ogg's checksum: CRC-32, polynomial 0x04c11db7, unreflected, from 0,
/// over the page with its checksum's bytes 0; a byte at a time by a table,
/// for each page an Opus view reads.
fn ogg_crc(b: &[u8]) -> u32 {
    const TABLE: [u32; 256] = {
        let mut t = [0u32; 256];
        let mut i = 0;
        while i < 256 {
            let mut c = (i as u32) << 24;
            let mut k = 0;
            while k < 8 {
                c = if c & 0x8000_0000 != 0 {
                    c << 1 ^ 0x04c1_1db7
                } else {
                    c << 1
                };
                k += 1;
            }
            t[i] = c;
            i += 1;
        }
        t
    };
    b.iter().fold(0, |crc, &x| {
        crc << 8 ^ TABLE[((crc >> 24) as u8 ^ x) as usize]
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// bytes as a file, open, its name gone once it is
    fn file(bytes: &[u8]) -> File {
        static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let p = std::env::temp_dir().join(format!("pkapp-view-{}-{n}", std::process::id()));
        File::create(&p).unwrap().write_all(bytes).unwrap();
        let f = File::open(&p).unwrap();
        std::fs::remove_file(&p).unwrap();
        f
    }

    fn all(v: &View) -> Vec<u8> {
        let mut b = vec![0u8; v.size() as usize];
        let mut at = 0;
        let len = b.len();
        while at < len {
            let n = v
                .read_at(at as u64, &mut b[at..at + 1000.min(len - at)])
                .unwrap();
            assert!(n > 0);
            at += n;
        }
        assert_eq!(v.read_at(at as u64, &mut [0u8; 8]).unwrap(), 0);
        b
    }

    /// MPEG-1 Layer III, 44.1 kHz, mono, 64 kbit/s: 208 bytes a frame
    const HEAD: u32 = 0x7ff << 21 | 0b11 << 19 | 1 << 17 | 1 << 16 | 5 << 12 | 3 << 6;
    const FRAME: usize = 208;

    /// frame n of sound: its header, then n
    fn sound(n: u32) -> Vec<u8> {
        let mut f = HEAD.to_be_bytes().to_vec();
        f.extend_from_slice(&n.to_be_bytes());
        f.resize(FRAME, 0);
        f
    }

    /// an ID3 tag, an Info frame with ffmpeg's LAME tag (a delay of 576),
    /// and frames of sound, 100 bytes of something else after the 200th
    fn mp3(frames: u32) -> Vec<u8> {
        let mut b = b"ID3\x04\0\0\0\0\0\x64".to_vec();
        b.resize(10 + 100, 0);
        let mut info = HEAD.to_be_bytes().to_vec();
        info.resize(4 + 17, 0);
        info.extend_from_slice(b"Info");
        info.extend_from_slice(&1u32.to_be_bytes());
        info.extend_from_slice(&frames.to_be_bytes());
        let mut tag = b"Lavc61.19".to_vec();
        tag.resize(21, 0);
        tag.extend_from_slice(&[576u32 >> 4, (576 & 15) << 4, 0].map(|x| x as u8));
        info.extend_from_slice(&tag);
        info.resize(FRAME, 0);
        b.extend_from_slice(&info);
        for n in 0..frames {
            b.extend_from_slice(&sound(n));
            if n == 199 {
                b.extend_from_slice(&[0x55; 100]);
            }
        }
        b
    }

    #[test]
    fn mp3_views() {
        let ix = Index::read(file(&mp3(400))).unwrap();
        let Kind::Mp3(f) = &ix.kind else {
            panic!("not an MP3")
        };
        assert_eq!((f.count, f.delay, f.spf, f.rate), (400, 576, 1152, 44100));
        // 10 s: frame 383 holds it, two frames back
        let v = ix.view(10.0).unwrap();
        assert_eq!(v.start, (381.0 * 1152.0 - 576.0) / 44100.0);
        let b = all(&v);
        assert_eq!(&b[..8], &sound(381)[..8]);
        assert_eq!(b.len(), (400 - 381) * FRAME);
        // past the 100 bytes, frame 250 at 6.5 s
        let v = ix.view(250.0 * 1152.0 / 44100.0).unwrap();
        assert_eq!(&all(&v)[..8], &sound(248)[..8]);
        // its start, before the delay's samples; and held to the last frame
        let v = ix.view(0.0).unwrap();
        assert_eq!(v.start, -576.0 / 44100.0);
        assert_eq!(&all(&v)[..8], &sound(0)[..8]);
        assert_eq!(&all(&ix.view(1e9).unwrap())[..8], &sound(399)[..8]);
    }

    #[test]
    fn not_a_view() {
        assert!(Index::read(file(b"RIFF\x24\0\0\0WAVEfmt \x10\0\0\0")).is_none());
        // a WAV whose samples hold two frames of MP3 in a row
        let mut wav = b"RIFF\0\0\0\0WAVEdata".to_vec();
        wav.extend_from_slice(&[sound(0), sound(1), sound(2)].concat());
        assert!(Index::read(file(&wav)).is_none());
        assert!(Index::read(file(b"fLaC\0\0\0\x22")).is_none());
        let mut m4a = b"\0\0\0\x20ftypM4A ".to_vec();
        m4a.extend_from_slice(&[sound(0), sound(1)].concat());
        assert!(Index::read(file(&m4a)).is_none());
    }

    /// an Ogg page of stream 7, its checksum made
    fn page(seq: u32, granule: i64, flags: u8, lacing: &[u8], body: &[u8]) -> Vec<u8> {
        let mut p = b"OggS\0".to_vec();
        p.push(flags);
        p.extend_from_slice(&granule.to_le_bytes());
        p.extend_from_slice(&7u32.to_le_bytes());
        p.extend_from_slice(&seq.to_le_bytes());
        p.extend_from_slice(&[0; 4]);
        p.push(lacing.len() as u8);
        p.extend_from_slice(lacing);
        p.extend_from_slice(body);
        let crc = ogg_crc(&p);
        p[22..26].copy_from_slice(&crc.to_le_bytes());
        p
    }

    /// 20 ms of CELT, packet n: 10 bytes, its TOC byte, then n
    fn celt(n: u32) -> Vec<u8> {
        let mut p = vec![31 << 3];
        p.extend_from_slice(&n.to_be_bytes());
        p.resize(10, 0);
        p
    }

    /// OpusHead (a pre-skip of 312) and OpusTags, then pages of 50 packets
    /// of 20 ms, a second each; page 3's last packet is 600 bytes, which
    /// page 4 ends
    fn opus(pages: u32) -> Vec<u8> {
        let mut head = b"OpusHead\x01\x01".to_vec();
        head.extend_from_slice(&312u16.to_le_bytes());
        head.extend_from_slice(&48000u32.to_le_bytes());
        head.extend_from_slice(&[0, 0, 0]);
        let mut b = page(0, 0, 2, &[head.len() as u8], &head);
        let tags = b"OpusTags\0\0\0\0\0\0\0\0";
        b.extend_from_slice(&page(1, 0, 0, &[tags.len() as u8], tags));
        let mut n = 0;
        for k in 0..pages {
            let (mut lacing, mut body) = (Vec::new(), Vec::new());
            let first = if k == 4 {
                // the rest of page 3's last packet: 600 - 2 * 255
                lacing.push(90);
                body.extend_from_slice(&[0; 90]);
                1
            } else {
                0
            };
            for i in first..50 {
                if k == 3 && i == 49 {
                    let mut big = celt(n);
                    big.resize(600, 0);
                    lacing.extend_from_slice(&[255, 255]);
                    body.extend_from_slice(&big[..510]);
                } else {
                    lacing.push(10);
                    body.extend_from_slice(&celt(n));
                }
                n += 1;
            }
            let granule = (k as i64 + 1) * 48000 - if k == 3 { 960 } else { 0 };
            b.extend_from_slice(&page(
                2 + k,
                granule,
                if k == 4 { 1 } else { 0 },
                &lacing,
                &body,
            ));
        }
        b
    }

    #[test]
    fn opus_views() {
        let bytes = opus(8);
        let ix = Index::read(file(&bytes)).unwrap();
        let Kind::Opus(p) = &ix.kind else {
            panic!("not Opus")
        };
        assert_eq!((p.preskip, p.pages.len()), (312, 8));
        let head = p.head as usize;
        assert_eq!(&bytes[head..head + 4], b"OggS");
        // 2.5 s: 2.42 s and the pre-skip is 116,472 samples in, packet 21
        // of page 2, at 116,160
        let v = ix.view(2.5).unwrap();
        assert_eq!(v.start, 116160.0 / 48000.0);
        let b = all(&v);
        assert_eq!(&b[..head], &bytes[..head]);
        // its pages: the first made of page 2's rest, from 0; every one a
        // page whose checksum holds, its granule lowered by 116,160
        let pages = pages_of(&b[head..]);
        assert_eq!(pages.len(), 1 + 5);
        let made = &pages[0];
        assert_eq!((made[5], made[26]), (0, 29));
        assert_eq!(granule(made), 144000 - 116160);
        assert_eq!(&made[27 + 29..27 + 29 + 10], &celt(100 + 21)[..]);
        for (n, q) in pages[1..].iter().enumerate() {
            let at = p.pages[3 + n].at as usize;
            let file = &bytes[at..at + q.len()];
            let g = granule(file);
            assert_eq!(granule(q), if g < 0 { g } else { g - 116160 });
            assert_eq!(&q[27..], &file[27..]);
        }
        // in the packet page 4 ends: page 3's last, which begins at its granule
        let v = ix.view(4.0 + 0.08 + 0.01 - 312.0 / 48000.0).unwrap();
        assert_eq!(v.start, (4.0 * 48000.0 - 960.0) / 48000.0);
        let pages = pages_of(&all(&v)[head..]);
        assert_eq!((pages[0][26], granule(&pages[0])), (2, -1));
        assert_eq!(&pages[0][27 + 2..27 + 2 + 10], &celt(199)[..]);
        // page 4, which ends it: its packets from 960 on
        assert_eq!(granule(&pages[1]), 5 * 48000 - (4 * 48000 - 960));
        // a page's first packet: the page as it is, but its granule
        let v = ix.view(3.0 + 0.08 - 312.0 / 48000.0).unwrap();
        assert_eq!(v.start, 3.0);
        let pages = pages_of(&all(&v)[head..]);
        assert_eq!(granule(&pages[0]), 4 * 48000 - 960 - 3 * 48000);
        let at = p.pages[3].at as usize;
        assert_eq!(&pages[0][27..], &bytes[at + 27..at + pages[0].len()]);
        // the start is the file's
        let v = ix.view(0.0).unwrap();
        assert_eq!(v.start, 0.0);
        assert_eq!(all(&v), bytes);
    }

    /// b's Ogg pages, each checked: its checksum holds
    fn pages_of(mut b: &[u8]) -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        while !b.is_empty() {
            assert_eq!(&b[..4], b"OggS");
            let nseg = b[26] as usize;
            let size = 27 + nseg + b[27..27 + nseg].iter().map(|&l| l as usize).sum::<usize>();
            let mut zeroed = b[..size].to_vec();
            zeroed[22..26].fill(0);
            assert_eq!(ogg_crc(&zeroed).to_le_bytes(), b[22..26]);
            out.push(b[..size].to_vec());
            b = &b[size..];
        }
        out
    }

    fn granule(page: &[u8]) -> i64 {
        i64::from_le_bytes(page[6..14].try_into().unwrap())
    }

    /// Vorbis's three headers, its blocks 256 and 2048 at 48 kHz, then
    /// pages of a second
    fn vorbis(pages: u32) -> Vec<u8> {
        let mut id = b"\x01vorbis\0\0\0\0\x01".to_vec();
        id.extend_from_slice(&48000u32.to_le_bytes());
        id.extend_from_slice(&[0; 12]);
        id.extend_from_slice(&[0xb8, 1]);
        let mut b = page(0, 0, 2, &[id.len() as u8], &id);
        let comment = b"\x03vorbis\0\0\0\0\0\0\0\0\x01";
        let setup = [b"\x05vorbis".as_slice(), &[0; 40]].concat();
        b.extend_from_slice(&page(
            1,
            0,
            0,
            &[comment.len() as u8, setup.len() as u8],
            &[comment.as_slice(), &setup].concat(),
        ));
        for k in 0..pages {
            b.extend_from_slice(&page(
                2 + k,
                (k as i64 + 1) * 48000,
                0,
                &[100; 20],
                &[k as u8; 2000],
            ));
        }
        b
    }

    #[test]
    fn vorbis_views() {
        let bytes = vorbis(6);
        let ix = Index::read(file(&bytes)).unwrap();
        let Kind::Vorbis(p) = &ix.kind else {
            panic!("not Vorbis")
        };
        assert_eq!((p.long, p.rate, p.pages.len()), (2048, 48000, 6));
        let head = p.head as usize;
        // 2.5 s: page 2, a long block's half after page 1's granule
        let v = ix.view(2.5).unwrap();
        assert_eq!(v.start, (96000.0 + 1024.0) / 48000.0);
        let b = all(&v);
        assert_eq!(&b[..head], &bytes[..head]);
        assert_eq!(&b[head..], &bytes[p.pages[2].at as usize..]);
        let v = ix.view(0.5).unwrap();
        assert_eq!((v.start, all(&v)), (0.0, bytes));
    }

    #[test]
    fn ogg_not_a_view() {
        // sound on OpusTags' page
        let b = opus(2);
        let tags_at = 27 + 1 + 19;
        let tags = b"OpusTags\0\0\0\0\0\0\0\0";
        let mut c = b[..tags_at].to_vec();
        c.extend_from_slice(&page(
            1,
            960,
            0,
            &[tags.len() as u8, 10],
            &[tags.as_slice(), &celt(0)].concat(),
        ));
        c.extend_from_slice(&b[tags_at + 27 + 1 + tags.len()..]);
        assert!(Opus(&c).is_none());
        // and the same file with a second stream's page in it
        let mut other = page(9, 48000, 0, &[10], &celt(0));
        other[14] = 8;
        let c = [b.as_slice(), &other].concat();
        assert!(Opus(&c).is_none());
        assert!(Opus(&b).is_some());
    }

    /// the file's index, if it is Opus
    #[allow(non_snake_case)]
    fn Opus(b: &[u8]) -> Option<()> {
        matches!(Index::read(file(b))?.kind, Kind::Opus(_)).then_some(())
    }
}

#[cfg(test)]
mod files {
    use super::*;

    /// `PKAPP_VIEW_FILES=a.mp3:b.opus PKAPP_VIEW_TIMES=30,60 PKAPP_VIEW_OUT=dir
    /// cargo test view_files -- --ignored`: each file's views from each
    /// time, as ~/android-study/mc/views.py writes its own (dir/NAME.K.EXT,
    /// SECONDS long and a little more, and dir/NAME.txt: K, the time, the
    /// view's start and where its body begins), for headdecode and
    /// viewcheck.py on a device
    #[test]
    #[ignore]
    fn view_files() {
        const SECONDS: f64 = 23.0;
        let var = |k| std::env::var(k).unwrap_or_default();
        let times: Vec<f64> = var("PKAPP_VIEW_TIMES")
            .split(',')
            .filter_map(|t| t.parse().ok())
            .collect();
        let out = std::path::PathBuf::from(var("PKAPP_VIEW_OUT"));
        for p in var("PKAPP_VIEW_FILES").split(':').filter(|s| !s.is_empty()) {
            let path = std::path::Path::new(p);
            let ix = Index::read(File::open(path).unwrap()).expect("not a view's kind");
            let seconds = match &ix.kind {
                Kind::Mp3(f) => f.count as f64 * f.spf as f64 / f.rate as f64,
                Kind::Opus(p) | Kind::Vorbis(p) => {
                    p.pages.last().unwrap().granule as f64 / p.rate as f64
                }
            };
            let (stem, ext) = (
                path.file_stem().unwrap().to_str().unwrap(),
                path.extension().unwrap().to_str().unwrap(),
            );
            let mut lines = String::new();
            for (k, &t) in times.iter().enumerate() {
                let v = ix.view(t).unwrap();
                let body = (ix.len as f64 * (SECONDS + 2.0) / seconds) as u64 + 65536;
                let n = v.size().min(v.head + v.page.len() as u64 + body) as usize;
                let mut b = vec![0u8; n];
                let mut at = 0;
                while at < n {
                    at += v.read_at(at as u64, &mut b[at..]).unwrap();
                }
                std::fs::write(out.join(format!("{stem}.{k}.{ext}")), &b).unwrap();
                lines += &format!("{k} {t:.3} {:.6} {}\n", v.start, v.body);
            }
            std::fs::write(out.join(format!("{stem}.txt")), lines).unwrap();
            println!("{p}: {} views", times.len());
        }
    }
}
