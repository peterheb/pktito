//! A recording's samples to timed segments: pktito's chunks (whole up to 30
//! s, else cut at the silences, the silence left out), each transcribed,
//! and split into sentences timed by pktito's word timestamps, which each
//! segment keeps, word by word, for the window to follow playback with.
//! Speech the model wrote no word for is transcribed again (RETRY_SPEECH_S).

use crate::pktito::{Asr, Chunk, Model, Tok};
use serde::{Deserialize, Serialize};

/// The longest stretch transcribed at once: pktito's default, measured
/// (its README, "Long recordings").
pub const MAX_CHUNK_S: f64 = 30.0;
/// A segment with no sentence end in this long is cut at a word anyway.
const MAX_SEGMENT_S: f64 = 20.0;

/// A stretch of a chunk that the model wrote no word for, inside a sentence,
/// though Silero heard at least this much speech in it, is transcribed
/// again on its own, once; between sentences, RETRY_BREAK_S is enough. The
/// English model gives nothing for the first chunk of
/// AliceBench's chapters 8 and 10 (0.897-28.112 s, 0.929-28.32 s), and keeps
/// only "This is a LibriVox recording" of the first chunk of chapters 2, 7,
/// 9 and 12, dropping the credits and titles after it: at every frame its
/// decoder visits, blank beats the next word (by 0.02 logits at the closest,
/// chapter 10), and NeMo's own float32 transcription of chapter 8's chunk
/// gives nothing too (NeMo 3.0.0, the original v2 checkpoint, 2026-10-26).
/// The multilingual model drops chapter 5's closing chunk, chapter 1's
/// closing credits, a title in chapter 5 and a line of chapter 10. Each,
/// transcribed again, gives its words; LibriSpeech's long40 and long300
/// have no such stretch with either model (pktito-9950x's count, the same
/// day). A chunk whose words cover its speech is never touched. Inside a
/// sentence, less is too little: at 1 s, a FLEURS Polish recording's "km",
/// spoken "kilometrów kwadratowych", left a second after it that gave
/// "metrów kwadratowych." again, a word and a full stop too many.
const RETRY_SPEECH_S: f64 = 2.0;
/// The same between sentences: after a word that ends one, at a chunk's start
/// or end, or for the whole chunk if it gave no words, cut in two at
/// quietest() (whole again, it would give nothing again). AliceBench's
/// chapter 1 gets back "End of chapter one", 1.6 s after "...finished off
/// the cake." with no word, which 2 s missed; the corpus (ls200, long40 and
/// long300 with both models, six FLEURS sets) is otherwise as at 2 s.
const RETRY_BREAK_S: f64 = 1.0;
/// The most of a recording transcribed again, as a share of it, or one
/// chunk if that is more: music Silero takes for speech, which gives no
/// words however it is cut, then costs at most a fifth more time.
const RETRY_SHARE: f64 = 0.2;
/// A stretch is transcribed again only if its speech is within this many dB
/// of the recording's: dropped speech is the voice the rest of the recording
/// is in, where a voice in the background, which Silero hears as speech too,
/// is quieter. AliceBench's dropped stretches lie -3.6 to +2.7 dB from their
/// chapters' speech; LibriSpeech long-form recordings with three other
/// speakers mixed in at 10 dB SNR had stretches retried down to -24 dB
/// (median -3.8), and the retries put the background's words into them. The
/// mixes' WER, with no retry, ungated, and gated at 4, 5 and 6 dB: at 10 dB
/// SNR 6.21%, 9.20%, 6.02%, 6.46%, 6.73%; at 5 dB 19.88%, 21.91%, 19.79%,
/// 20.78%, 21.36%; at 0 dB, the background as loud as the speaker, 56.44%,
/// 58.68%, 58.40% (24 recordings, 2026-10-26). Pink noise and a phone band
/// draw no retry at all, nor do two songs; a third's one gives no words.
const RETRY_LEVEL_DB: f64 = 4.0;

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
    let (e, n) = chunks.iter().fold((0.0, 0), |(e, n), c| {
        let (ce, cn) = power(c, x, c.start, c.end);
        (e + ce, n + cn)
    });
    let floor = db(e, n) - RETRY_LEVEL_DB;
    let mut done = 0i64;
    let mut all = Vec::new();
    for c in &chunks {
        if stop() {
            return Ok((all, false));
        }
        let toks = asr.run(&x[c.start as usize..c.end as usize])?;
        let toks = again(model, asr, x, c, toks, floor, &mut retries)?;
        let segs = sentences(model, &toks, c.start as f64 / rate, frame);
        done += c.end - c.start;
        progress(done as f64 / total as f64, &segs);
        all.extend(segs);
    }
    Ok((all, true))
}

/// A chunk's tokens with those of each stretch of it transcribed again
/// (RETRY_SPEECH_S, RETRY_BREAK_S), its speech no quieter than floor dB
/// (RETRY_LEVEL_DB), while the samples left to retry last: each stretch's
/// tokens at its own frames in the chunk, all of them in frame order.
fn again(
    model: &Model,
    asr: &mut Asr,
    x: &[f32],
    c: &Chunk,
    mut toks: Vec<Tok>,
    floor: f64,
    retries: &mut i64,
) -> Result<Vec<Tok>, String> {
    let loud = |s0: i64, s1: i64| {
        let (e, n) = power(c, x, s0, s1);
        db(e, n) >= floor
    };
    let rate = model.rate() as f64;
    let spf = (model.frame_seconds() * rate).round() as i64;
    let frames = (c.end - c.start + spf - 1) / spf;
    let (inside, at_break) = (
        (RETRY_SPEECH_S * rate) as i64,
        (RETRY_BREAK_S * rate) as i64,
    );
    let samples = |(f0, f1): (i64, i64)| (c.start + f0 * spf, (c.start + f1 * spf).min(c.end));
    let stretches = if toks.is_empty() {
        let (s0, s1) = (c.start, c.end);
        if c.heard(s0, s1) < at_break || !loud(s0, s1) || s1 - s0 > *retries {
            return Ok(toks);
        }
        *retries -= s1 - s0;
        let m = quietest(&x[s0 as usize..s1 as usize], rate) as i64 / spf;
        vec![(0, m), (m, frames)]
    } else {
        let words = model.words(&toks).into_iter().map(|w| {
            let text = model.text(&toks[w.tok as usize..(w.tok + w.ntok) as usize]);
            (
                w.start as i64,
                w.end as i64,
                text.ends_with(['.', '?', '!']),
            )
        });
        let mut chosen = Vec::new();
        for (f0, f1, after_break) in gaps(words, frames) {
            let (s0, s1) = samples((f0, f1));
            let enough = if after_break || f1 == frames {
                at_break
            } else {
                inside
            };
            if s1 - s0 <= *retries && c.heard(s0, s1) >= enough && loud(s0, s1) {
                *retries -= s1 - s0;
                chosen.push((f0, f1));
            }
        }
        chosen
    };
    for f in stretches {
        let (s0, s1) = samples(f);
        if s1 > s0 {
            let more = asr.run(&x[s0 as usize..s1 as usize])?;
            toks.extend(more.into_iter().map(|t| Tok {
                frame: t.frame + f.0 as i32,
                ..t
            }));
        }
    }
    toks.sort_by_key(|t| t.frame);
    Ok(toks)
}

/// The stretches of a chunk of n frames that none of its words covers:
/// before the first, between each two, and after the last. Each word is
/// [start, end) in frames, in order, and whether it ends a sentence; each
/// stretch is [start, end) and whether it follows a sentence's end, as the
/// chunk's start does.
fn gaps(words: impl Iterator<Item = (i64, i64, bool)>, n: i64) -> Vec<(i64, i64, bool)> {
    let mut out = Vec::new();
    let (mut at, mut after_break) = (0, true);
    for (start, end, ends) in words {
        if start > at {
            out.push((at, start, after_break));
        }
        at = at.max(end);
        after_break = ends;
    }
    if n > at {
        out.push((at, n, after_break));
    }
    out
}

/// The energy of x where Silero heard speech in [from, to) of a chunk, and
/// over how many samples.
fn power(c: &Chunk, x: &[f32], from: i64, to: i64) -> (f64, i64) {
    let (mut e, mut n) = (0.0, 0);
    for s in &c.speech {
        let (a, b) = (s.start.max(from), s.end.min(to));
        if b > a {
            e += x[a as usize..b as usize]
                .iter()
                .map(|&v| v as f64 * v as f64)
                .sum::<f64>();
            n += b - a;
        }
    }
    (e, n)
}

/// Mean power in dB, of energy e over n samples; very low for none.
fn db(e: f64, n: i64) -> f64 {
    10.0 * (e / n.max(1) as f64).max(1e-20).log10()
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
    fn the_stretches_no_word_covers() {
        // words at frames [3, 5), ending a sentence, and [9, 12) of 20:
        // before (the chunk's start a break), between (after the sentence's
        // end) and after (inside a sentence)
        assert_eq!(
            gaps([(3, 5, true), (9, 12, false)].into_iter(), 20),
            vec![(0, 3, true), (5, 9, true), (12, 20, false)]
        );
        // words that touch or overlap leave nothing between them
        assert_eq!(
            gaps(
                [(0, 4, false), (4, 9, false), (8, 20, true)].into_iter(),
                20
            ),
            vec![]
        );
        // no words: the whole chunk
        assert_eq!(gaps(std::iter::empty(), 7), vec![(0, 7, true)]);
    }

    #[test]
    fn speech_heard_in_a_stretch() {
        use crate::pktito::Span;
        let c = Chunk {
            start: 100,
            end: 1000,
            speech: vec![
                Span {
                    start: 100,
                    end: 300,
                },
                Span {
                    start: 500,
                    end: 900,
                },
            ],
        };
        assert_eq!(c.heard(100, 1000), 600);
        assert_eq!(c.heard(250, 600), 150);
        assert_eq!(c.heard(300, 500), 0);
    }
}
