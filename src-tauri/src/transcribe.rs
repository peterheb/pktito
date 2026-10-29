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
    let whole = asr.transcribe(x, |c| {
        let segs = sentences(model, c.toks, c.start as f64 / rate, frame);
        progress(c.done as f64 / c.total.max(1) as f64, &segs);
        all.extend(segs);
        !c.last && stop()
    })?;
    Ok((all, whole))
}

fn sentences(model: &Model, toks: &[Tok], offset: f64, frame: f64) -> Vec<Segment> {
    let words = model.words(toks);
    let mut out = Vec::new();
    let mut first = 0usize;
    let mut timed = Vec::new();
    for (i, w) in words.iter().enumerate() {
        let span = |a: usize| (words[a].tok as usize, (w.tok + w.ntok) as usize);
        let word = model.text(&toks[w.tok as usize..(w.tok + w.ntok) as usize]);
        let start = offset + words[first].start as f64 * frame;
        let end = offset + w.end as f64 * frame;
        let last = i + 1 == words.len();
        let cut = last || word.ends_with(['.', '?', '!']) || end - start >= MAX_SEGMENT_S;
        timed.push(Word {
            start: offset + w.start as f64 * frame,
            end,
            text: word,
        });
        if cut {
            let (a, b) = span(first);
            out.push(Segment {
                start,
                end,
                text: model.text(&toks[a..b]),
                words: std::mem::take(&mut timed),
            });
            first = i + 1;
        }
    }
    out
}
