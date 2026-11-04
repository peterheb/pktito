//! A recording's samples to timed segments: pktito's transcription of a
//! whole recording (pk_asr_transcribe: chunks of at most 30 s cut at the
//! silences, the silence left out, and speech the model wrote no word for
//! heard again), split into sentences timed by pktito's word timestamps,
//! which each segment keeps, word by word, for the window to follow
//! playback with. The retry was measured here first (2026-10-26) and went
//! into pktito with its numbers (its core/transcribe.c and README,
//! "Transcribing a recording"); on AliceBench both models' tokens came out
//! of pktito's the same as of this file's own, every one (2026-10-29).

use crate::pktito::{Asr, Model, Tok};
use serde::{Deserialize, Serialize};

/// A line is a sentence, up to this long; a longer one is broken into parts
/// of PART_MIN_S to PART_MAX_S. Cutting at whichever word crossed 20 s, as
/// before, left 16 tails under 5 s on AliceBench's whole book with v2 ("of
/// the mock turtle's heavy sobs."); 15 of its 1,687 sentences run past 30 s.
/// Over 343 h of other English (lectures, two parliaments, meetings, doctor
/// visits, courts, NASA; v2, simulated on pktito-c's words, 2026-11-04) this
/// rule broke 2.8 sentences an hour, every break at a , ; : or dash (52%) or
/// a pause of 0.3 s or more (47%), with 0.17 lines under 5 s beside a break
/// an hour; the 20 s cut broke 11.4 an hour, 79% at neither. Cutting at
/// 20 s / 6-14, at 40 s / 15-25, at 250 characters, at pauses alone, or into
/// even parts each measured worse on one of those or on line length. With
/// v3 on 10.3 h of LibriVox in German, French, Italian, Dutch, Polish,
/// Portuguese and Spanish (the app's own lines, 2026-11-04): 16.4 breaks an
/// hour, most in Cervantes's Spanish and in Dutch, where 13% and 9% of
/// sentences run past 30 s; 1% at neither mark nor pause, against the 20 s
/// cut's 78% (33.1 an hour).
const SENTENCE_MAX_S: f64 = 30.0;
const PART_MIN_S: f64 = 10.0;
const PART_MAX_S: f64 = 20.0;

/// PKTITO_THREADS, for finding out what a machine does on fewer threads (the
/// engine CLI's -t); unset, 0: one a core, and pktito's choice among them.
pub fn threads_wanted() -> i32 {
    std::env::var("PKTITO_THREADS")
        .ok()
        .and_then(|t| t.trim().parse().ok())
        .unwrap_or(0)
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Word {
    pub start: f64,
    pub end: f64,
    pub text: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Segment {
    pub start: f64,
    pub end: f64,
    pub text: String,
    pub words: Vec<Word>,
}

/// Transcribe x, calling progress(fraction of the audio done, the segments
/// just finished) after each chunk; stop(), before the first and after each
/// but the last, says whether to stop there. The segments, and whether they
/// are all of it.
///
/// A sentence a chunk ends in the middle of is carried into the next: a
/// chunk's last word ends a segment only where a sentence ends or the
/// recording does, as any other word does. pktito cuts its chunks at the
/// silences, of at most 30 s, so about half of them fall inside a sentence
/// (long40: 150 of 310 cuts with v2, 133 with v3, libpktito's count), and
/// each became a sentence's end on the screen; with the far side of a cut
/// heard as context (libpktito's stitch) the model rarely punctuates one
/// (the 26 English test clips: false sentence ends at the cuts, v2 37% to
/// 1%, v3 19% to 4%), so a chunk that ends with no . ? or ! is almost
/// always one still in mid-sentence.
pub fn run(
    model: &Model,
    asr: &mut Asr,
    x: &[f32],
    progress: impl FnMut(f64, &[Segment]),
    stop: impl Fn() -> bool,
) -> Result<(Vec<Segment>, bool), String> {
    run_windows(model, asr, x, progress, stop).map(|(segs, _, whole)| (segs, whole))
}

/// A chunk pktito transcribed, for measuring its cuts (pktito-cli's -f
/// json): the stretch of the recording whose words are its own (pk_chunk_out.at),
/// in seconds, and how many of the transcript's words are its, in order.
#[derive(Clone, Copy)]
pub struct Window {
    pub start: f64,
    pub end: f64,
    pub words: usize,
}

/// run(), and each chunk's window.
pub fn run_windows(
    model: &Model,
    asr: &mut Asr,
    x: &[f32],
    mut progress: impl FnMut(f64, &[Segment]),
    stop: impl Fn() -> bool,
) -> Result<(Vec<Segment>, Vec<Window>, bool), String> {
    let rate = model.rate() as f64;
    let frame = model.frame_seconds();
    let mut all = Vec::new();
    let mut windows = Vec::new();
    if stop() {
        return Ok((all, windows, false));
    }
    let mut open = Open::default();
    let mut done = 0.0;
    let whole = asr.transcribe(x, |c| {
        let stopping = !c.last && stop();
        let held = open.words.len();
        let mut segs = sentences(model, c.toks, c.start as f64 / rate, frame, &mut open);
        // its words: those it ended in sentences and those it left open,
        // less the ones open from before it
        let words = segs.iter().map(|s| s.words.len()).sum::<usize>() + open.words.len() - held;
        windows.push(Window {
            start: c.start as f64 / rate,
            end: c.end as f64 / rate,
            words,
        });
        // the recording's end, or where it stops: the sentence still open ends
        if c.last || stopping {
            segs.extend(open.close(model));
        }
        done = c.done as f64 / c.total.max(1) as f64;
        progress(done, &segs);
        all.extend(segs);
        stopping
    })?;
    // a sentence still open where pktito ended with no last chunk to close it
    let rest: Vec<Segment> = open.close(model).into_iter().collect();
    if !rest.is_empty() {
        progress(done, &rest);
        all.extend(rest);
    }
    Ok((all, windows, whole))
}

/// The sentence a chunk ended in the middle of: its tokens, for its text
/// across the cut (the model's tokens detokenized whole, as within a
/// chunk), its words, timed, and when it began.
#[derive(Default)]
struct Open {
    toks: Vec<Tok>,
    words: Vec<Word>,
    // each word's count of toks, to break the sentence between two words
    ntoks: Vec<usize>,
    start: f64,
}

impl Open {
    /// The open sentence as a segment, if there is one, and none open after.
    fn close(&mut self, model: &Model) -> Option<Segment> {
        if self.words.is_empty() {
            return None;
        }
        let words = std::mem::take(&mut self.words);
        self.ntoks.clear();
        Some(Segment {
            start: self.start,
            end: words.last().map_or(self.start, |w| w.end),
            text: model.text(&std::mem::take(&mut self.toks)),
            words,
        })
    }

    /// The open sentence's first n words as a segment; the rest stay open.
    fn close_part(&mut self, model: &Model, n: usize) -> Segment {
        let nt: usize = self.ntoks[..n].iter().sum();
        let words: Vec<Word> = self.words.drain(..n).collect();
        self.ntoks.drain(..n);
        let toks: Vec<Tok> = self.toks.drain(..nt).collect();
        let seg = Segment {
            start: self.start,
            end: words.last().map_or(self.start, |w| w.end),
            text: model.text(&toks),
            words,
        };
        self.start = self.words.first().map_or(seg.end, |w| w.start);
        seg
    }

    /// Where to break a sentence that ran past SENTENCE_MAX_S: after the
    /// last word ending in , ; : or a dash whose part would run PART_MIN_S
    /// to PART_MAX_S; with none, at the longest pause between two words in
    /// that span (or, with no word ending in it, anywhere). How many words
    /// go in the first part.
    fn part_end(&self) -> usize {
        let last = self.words.len() - 1;
        let span = |i: usize| {
            let t = self.words[i].end - self.start;
            (PART_MIN_S..=PART_MAX_S).contains(&t)
        };
        let pause = |i: usize| self.words[i + 1].start - self.words[i].end;
        if let Some(i) = (0..last)
            .rev()
            .find(|&i| span(i) && ends(&self.words[i].text, &[',', ';', ':', '—', '–']))
        {
            return i + 1;
        }
        let mut cands: Vec<usize> = (0..last).filter(|&i| span(i)).collect();
        if cands.is_empty() {
            cands = (0..last).collect();
        }
        // the longest pause; the later of two the same
        let i = cands
            .into_iter()
            .fold(0, |b, i| if pause(i) >= pause(b) { i } else { b });
        i + 1
    }
}

/// Whether a word ends in one of these, before any closing quote or bracket.
fn ends(text: &str, marks: &[char]) -> bool {
    text.trim_end_matches(['"', '\'', '”', '’', ')', ']'])
        .ends_with(marks)
}

/// A chunk's tokens as sentences, after the one open before it: a segment
/// ends at a word ending in . ? or !, and one that runs past SENTENCE_MAX_S
/// first is broken (Open::part_end); the words after the last end are left
/// open, for the next chunk.
fn sentences(
    model: &Model,
    toks: &[Tok],
    offset: f64,
    frame: f64,
    open: &mut Open,
) -> Vec<Segment> {
    let words = model.words(toks);
    let mut out = Vec::new();
    for w in &words {
        let run = &toks[w.tok as usize..(w.tok + w.ntok) as usize];
        let text = model.text(run);
        let start = offset + w.start as f64 * frame;
        let end = offset + w.end as f64 * frame;
        if open.words.is_empty() {
            open.start = start;
        }
        let sentence_end = ends(&text, &['.', '?', '!']);
        open.toks.extend_from_slice(run);
        open.ntoks.push(run.len());
        open.words.push(Word { start, end, text });
        // the length first: a sentence whose last word took it past
        // SENTENCE_MAX_S, or came after a long silence ("Um", 136 s of
        // nothing, "Order." in a Northern Ireland Assembly sitting), is broken
        // like any other
        if end - open.start > SENTENCE_MAX_S && open.words.len() > 1 {
            let n = open.part_end();
            out.push(open.close_part(model, n));
        }
        if sentence_end {
            out.extend(open.close(model));
        }
    }
    out
}
