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

/// A segment with no sentence end in this long is cut at a word anyway.
const MAX_SEGMENT_S: f64 = 20.0;

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
    mut progress: impl FnMut(f64, &[Segment]),
    stop: impl Fn() -> bool,
) -> Result<(Vec<Segment>, bool), String> {
    let rate = model.rate() as f64;
    let frame = model.frame_seconds();
    let mut all = Vec::new();
    if stop() {
        return Ok((all, false));
    }
    let mut open = Open::default();
    let mut done = 0.0;
    let whole = asr.transcribe(x, |c| {
        let stopping = !c.last && stop();
        let mut segs = sentences(model, c.toks, c.start as f64 / rate, frame, &mut open);
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
    Ok((all, whole))
}

/// The sentence a chunk ended in the middle of: its tokens, for its text
/// across the cut (the model's tokens detokenized whole, as within a
/// chunk), its words, timed, and when it began.
#[derive(Default)]
struct Open {
    toks: Vec<Tok>,
    words: Vec<Word>,
    start: f64,
}

impl Open {
    /// The open sentence as a segment, if there is one, and none open after.
    fn close(&mut self, model: &Model) -> Option<Segment> {
        if self.words.is_empty() {
            return None;
        }
        let words = std::mem::take(&mut self.words);
        Some(Segment {
            start: self.start,
            end: words.last().map_or(self.start, |w| w.end),
            text: model.text(&std::mem::take(&mut self.toks)),
            words,
        })
    }
}

/// A chunk's tokens as sentences, after the one open before it: a segment
/// ends at a word ending in . ? or !, or once it runs MAX_SEGMENT_S; the
/// words after the last such end are left open, for the next chunk.
fn sentences(model: &Model, toks: &[Tok], offset: f64, frame: f64, open: &mut Open) -> Vec<Segment> {
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
        let cut = text.ends_with(['.', '?', '!']) || end - open.start >= MAX_SEGMENT_S;
        open.toks.extend_from_slice(run);
        open.words.push(Word { start, end, text });
        if cut {
            out.extend(open.close(model));
        }
    }
    out
}
