//! A recording's samples to timed segments: pktito's chunks (whole up to 30
//! s, else cut at the silences), each transcribed, and split into sentences
//! timed by pktito's word timestamps.

use crate::pktito::{Asr, Model, Tok};
use serde::Serialize;

/// The longest stretch transcribed at once: pktito's default, measured
/// (its README, "Long recordings").
pub const MAX_CHUNK_S: f64 = 30.0;
/// A segment with no sentence end in this long is cut at a word anyway.
const MAX_SEGMENT_S: f64 = 20.0;

#[derive(Clone, Serialize)]
pub struct Segment {
    pub start: f64,
    pub end: f64,
    pub text: String,
}

/// Transcribe x, calling progress(fraction of the audio done, the segments
/// just finished) after each chunk.
pub fn run(
    model: &Model, asr: &mut Asr, x: &[f32], mut progress: impl FnMut(f64, &[Segment]),
) -> Result<Vec<Segment>, String> {
    let rate = model.rate() as f64;
    let frame = model.frame_seconds();
    let chunks = model.chunks(x, MAX_CHUNK_S)?;
    let total: i64 = chunks.iter().map(|c| c.end - c.start).sum::<i64>().max(1);
    let mut done = 0i64;
    let mut all = Vec::new();
    for c in &chunks {
        let toks = asr.run(&x[c.start as usize..c.end as usize])?;
        let segs = sentences(model, &toks, c.start as f64 / rate, frame);
        done += c.end - c.start;
        progress(done as f64 / total as f64, &segs);
        all.extend(segs);
    }
    Ok(all)
}

fn sentences(model: &Model, toks: &[Tok], offset: f64, frame: f64) -> Vec<Segment> {
    let words = model.words(toks);
    let mut out = Vec::new();
    let mut first = 0usize;
    for (i, w) in words.iter().enumerate() {
        let span = |a: usize| (words[a].tok as usize, (w.tok + w.ntok) as usize);
        let word = model.text(&toks[w.tok as usize..(w.tok + w.ntok) as usize]);
        let start = offset + words[first].start as f64 * frame;
        let end = offset + w.end as f64 * frame;
        let last = i + 1 == words.len();
        if last || word.ends_with(['.', '?', '!']) || end - start >= MAX_SEGMENT_S {
            let (a, b) = span(first);
            out.push(Segment { start, end, text: model.text(&toks[a..b]) });
            first = i + 1;
        }
    }
    out
}
