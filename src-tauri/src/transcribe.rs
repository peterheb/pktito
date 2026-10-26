//! A recording's samples to timed segments: pktito's chunks (whole up to 30
//! s, else cut at the silences, the silence left out), each transcribed,
//! and split into sentences timed by pktito's word timestamps, which each
//! segment keeps, word by word, for the window to follow playback with. A
//! chunk of speech that comes back with no words is transcribed again in
//! two halves (RETRY_SPEECH_S).

use crate::pktito::{Asr, Chunk, Model, Tok};
use serde::{Deserialize, Serialize};

/// The longest stretch transcribed at once: pktito's default, measured
/// (its README, "Long recordings").
pub const MAX_CHUNK_S: f64 = 30.0;
/// A segment with no sentence end in this long is cut at a word anyway.
const MAX_SEGMENT_S: f64 = 20.0;

/// A chunk that comes back with no words, though Silero heard at least this
/// much speech in it, is transcribed again, cut in two at quietest(). The
/// English model gives nothing for the first chunk of AliceBench's chapters
/// 8 and 10 (0.897-28.112 s and 0.929-28.32 s): the LibriVox notice, the
/// title and two sentences of chapter 8, lost. At every frame its decoder
/// visits, blank beats the first word, by 0.02 logits at the closest, and
/// NeMo's own float32 transcription of the same cut gives nothing too
/// (NeMo 3.0.0, the original v2 checkpoint, 2026-10-26). Each half gives
/// its words: 24 and 13 for chapter 10, 29 and 35 for chapter 8. A chunk
/// with words is never touched, so every other transcript stays as it was.
const RETRY_SPEECH_S: f64 = 2.0;
/// The most of a recording retried, as a share of it, or one chunk if that
/// is more: music Silero takes for speech, which gives no words however it
/// is cut, then costs at most a fifth more time.
const RETRY_SHARE: f64 = 0.2;

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
/// just finished) after each chunk; stop() before each says whether to stop
/// there. The segments, and whether they are all of it.
pub fn run(
    model: &Model,
    asr: &mut Asr,
    x: &[f32],
    mut progress: impl FnMut(f64, &[Segment]),
    stop: impl Fn() -> bool,
) -> Result<(Vec<Segment>, bool), String> {
    let rate = model.rate() as f64;
    let frame = model.frame_seconds();
    let chunks = model.chunks(x, MAX_CHUNK_S)?;
    let total: i64 = chunks.iter().map(|c| c.end - c.start).sum::<i64>().max(1);
    let mut retries = (total as f64 * RETRY_SHARE).max(MAX_CHUNK_S * rate) as i64;
    let mut done = 0i64;
    let mut all = Vec::new();
    for c in &chunks {
        if stop() {
            return Ok((all, false));
        }
        let (a, b) = (c.start as usize, c.end as usize);
        let toks = asr.run(&x[a..b])?;
        let segs = if toks.is_empty() && retry(c, rate, retries) {
            retries -= c.end - c.start;
            let m = a + quietest(&x[a..b], rate);
            let mut segs = sentences(model, &asr.run(&x[a..m])?, a as f64 / rate, frame);
            segs.extend(sentences(
                model,
                &asr.run(&x[m..b])?,
                m as f64 / rate,
                frame,
            ));
            segs
        } else {
            sentences(model, &toks, a as f64 / rate, frame)
        };
        done += c.end - c.start;
        progress(done as f64 / total as f64, &segs);
        all.extend(segs);
    }
    Ok((all, true))
}

/// Whether a chunk that gave no words is transcribed again: if Silero heard
/// RETRY_SPEECH_S of speech in it, and the retries left cover it.
fn retry(c: &Chunk, rate: f64, retries_left: i64) -> bool {
    c.speech as f64 >= RETRY_SPEECH_S * rate && c.end - c.start <= retries_left
}

/// Where a chunk is cut for its retry, from its start: the middle of its
/// quietest 300 ms whose start lies in its middle third, so that neither half
/// is short and the cut falls in a pause where there is one; the middle
/// where the chunk is too short for that.
fn quietest(x: &[f32], rate: f64) -> usize {
    let win = (0.3 * rate) as usize;
    let step = (0.05 * rate) as usize;
    let mut best = (f64::INFINITY, x.len() / 2);
    let mut at = x.len() / 3;
    while at + win <= x.len() && at < 2 * x.len() / 3 {
        let e: f64 = x[at..at + win].iter().map(|&v| v as f64 * v as f64).sum();
        if e < best.0 {
            best = (e, at + win / 2);
        }
        at += step.max(1);
    }
    best.1
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cut_in_the_middle_thirds_quietest() {
        // 9 s at 1 kHz, loud but for a pause at 4.0-4.4 s: cut in the pause
        let rate = 1000.0;
        let mut x = vec![0.5f32; 9000];
        x[4000..4400].fill(0.0);
        let m = quietest(&x, rate);
        assert!((4000..4400).contains(&m), "cut at {m}");
        // no pause: the first of the equally loud windows, in the middle third
        let m = quietest(&vec![0.5f32; 9000], rate);
        assert!((3000..6000).contains(&m), "cut at {m}");
        // too short for a 300 ms window to start in its middle third and
        // fit: the middle
        assert_eq!(quietest(&[0.5f32; 400], rate), 200);
    }

    #[test]
    fn retried_with_speech_and_room_left() {
        let rate = 16000.0;
        let c = Chunk {
            start: 0,
            end: 28 * 16000,
            speech: 20 * 16000,
        };
        assert!(retry(&c, rate, 30 * 16000));
        // too little speech heard: silence or noise
        let quiet = Chunk { speech: 16000, ..c };
        assert!(!retry(&quiet, rate, 30 * 16000));
        // the retries spent
        assert!(!retry(&c, rate, 27 * 16000));
    }
}
