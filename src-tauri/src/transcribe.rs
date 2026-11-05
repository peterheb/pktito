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

// A line is a sentence (pk_sentences, core/sentences.c; libpktito's
// README, "Sentences", has its ends measured): at . ? ! or an ellipsis, not
// before a small letter or after an abbreviation the times say ends
// nothing; and one past 30 s is broken into parts of 10 to 20 s, after the
// last , ; : or dash there, else at the longest pause. That break was this
// file's before the library took it. Cutting at whichever word crossed
// 20 s, as before it, left 16 tails under 5 s on AliceBench's whole book
// with v2 ("of the mock turtle's heavy sobs."). Over 343 h of other English
// (lectures, two parliaments, meetings, doctor visits, courts, NASA; v2,
// simulated on pktito-c's words, 2026-11-04) the break came 2.8 times an
// hour, every one at a , ; : or dash (52%) or a pause of 0.3 s or more
// (47%), with 0.17 lines under 5 s beside a break an hour; the 20 s cut
// broke 11.4 an hour, 79% at neither. Cutting at 20 s / 6-14, at 40 s /
// 15-25, at 250 characters, at pauses alone, or into even parts each
// measured worse on one of those or on line length. With v3 on 10.3 h of
// LibriVox in German, French, Italian, Dutch, Polish, Portuguese and
// Spanish (2026-11-04): 16.4 breaks an hour, most in Cervantes's Spanish
// and in Dutch, where 13% and 9% of sentences run past 30 s; 1% at neither
// mark nor pause, against the 20 s cut's 78% (33.1 an hour).

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
        let held = open.words(model);
        // the recording's end, or where it stops: the sentence still open ends
        let end = c.last || stopping;
        let segs = sentences(model, c.toks, c.start as f64 / rate, frame, end, &mut open);
        // its words: those it ended in sentences and those it left open,
        // less the ones open from before it
        let words = (segs.iter().map(|s| s.words.len()).sum::<usize>() + open.words(model))
            .saturating_sub(held);
        windows.push(Window {
            start: c.start as f64 / rate,
            end: c.end as f64 / rate,
            words,
        });
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

/// The tokens a chunk ended with after its last sentence: the sentence it
/// ended in the middle of, its frames counted from the recording's start
/// as pk_sentences counts them; for each token, the seconds its chunk
/// started at and the frames that added, for the words' times, which stay
/// each chunk's own; and the words' language, once pk_language says.
#[derive(Default)]
struct Open {
    toks: Vec<Tok>,
    at: Vec<(f64, i32)>,
    lang: Option<String>,
    // the words so far, until pk_language names their language
    heard: Vec<String>,
}

impl Open {
    /// The open sentence as a segment, if there is one, and none open after.
    fn close(&mut self, model: &Model) -> Option<Segment> {
        if self.toks.is_empty() {
            return None;
        }
        let toks = std::mem::take(&mut self.toks);
        let at = std::mem::take(&mut self.at);
        Some(segment(model, &toks, &at))
    }

    /// How many words the open sentence has so far.
    fn words(&self, model: &Model) -> usize {
        model.words(&self.toks).len()
    }
}

/// Tokens as a segment: their text whole, and each word timed in seconds.
fn segment(model: &Model, toks: &[Tok], at: &[(f64, i32)]) -> Segment {
    let frame = model.frame_seconds();
    let time = |i: usize, f: i32| at[i].0 + (f - at[i].1) as f64 * frame;
    let words: Vec<Word> = model
        .words(toks)
        .iter()
        .map(|w| {
            let (first, last) = (w.tok as usize, (w.tok + w.ntok - 1) as usize);
            Word {
                start: time(first, w.start),
                end: time(last, w.end),
                text: model.text(&toks[first..=last]),
            }
        })
        .collect();
    Segment {
        start: words.first().map_or(0.0, |w| w.start),
        end: words.last().map_or(0.0, |w| w.end),
        text: model.text(toks),
        words,
    }
}

/// A chunk's tokens as sentences, after the tokens open before it
/// (pk_sentences); done, whether they end the recording. The tokens after
/// the last sentence are left open, for the next chunk.
fn sentences(
    model: &Model,
    toks: &[Tok],
    offset: f64,
    frame: f64,
    done: bool,
    open: &mut Open,
) -> Vec<Segment> {
    // the chunk's frames from the recording's start, as pk_sentences counts
    // them: its start in whole frames
    let base = (offset / frame).round() as i32;
    for t in toks {
        open.toks.push(Tok {
            frame: t.frame + base,
            ..*t
        });
        open.at.push((offset, base));
    }
    // the words' language, for pk_sentences's rules of one language's:
    // English for a model of English alone, else pk_language's once it
    // names one
    if open.lang.is_none() {
        match crate::model::entry(&model.name()) {
            Some(e) if e.languages == ["en"] => open.lang = Some("en".into()),
            _ => {
                open.heard.extend(
                    model
                        .words(toks)
                        .iter()
                        .map(|w| model.text(&toks[w.tok as usize..(w.tok + w.ntok) as usize])),
                );
                let heard: Vec<&str> = open.heard.iter().map(|w| w.trim()).collect();
                open.lang = crate::pktito::language(&heard);
                if open.lang.is_some() {
                    open.heard = Vec::new();
                }
            }
        }
    }
    let found = model
        .sentences(&open.toks, open.lang.as_deref(), done)
        .unwrap_or_default();
    let out: Vec<Segment> = found
        .iter()
        .map(|s| {
            let r = s.first as usize..(s.first + s.n) as usize;
            segment(model, &open.toks[r.clone()], &open.at[r])
        })
        .collect();
    let used = found.last().map_or(0, |s| (s.first + s.n) as usize);
    open.toks.drain(..used);
    open.at.drain(..used);
    out
}
