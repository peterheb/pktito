//! A transcript in the formats Save writes, for the window and pktito-cli
//! alike, so that the two write the same bytes: text and Markdown of its
//! sentences, each with its time if asked; SRT and WebVTT of its subtitle
//! track, the engine's cues (pk_captions); and JSON, pktito/1, the
//! document pktito-c's --json writes too: the sentences with every word's
//! time, the cues as their lines, and in pktito-cli's the chunks.

use crate::model;
use crate::pktito;
use crate::transcribe::{Segment, Window};
use std::path::{Path, PathBuf};

/// The formats, by the extension each is saved under
pub const FORMATS: &[&str] = &["txt", "md", "srt", "vtt", "json"];

/// Where stem.ext can go in dir without writing over anything: stem.ext,
/// else "stem 2.ext" ("stem (2).ext" on Windows, as Explorer numbers
/// copies), and so on.
pub fn free_path(dir: &Path, stem: &str, ext: &str) -> PathBuf {
    let first = dir.join(format!("{stem}.{ext}"));
    if !first.exists() {
        return first;
    }
    (2..)
        .map(|n| {
            dir.join(if cfg!(windows) {
                format!("{stem} ({n}).{ext}")
            } else {
                format!("{stem} {n}.{ext}")
            })
        })
        .find(|p| !p.exists())
        .unwrap()
}

/// A transcript's name, its original's without the extension
pub fn stem_of(original: &Path) -> String {
    original
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "transcript".into())
}

/// What a transcript is written from: the file's name, the recording's
/// length in seconds, the model that made it, and its segments; and where
/// it shows its speakers (the window's switch, pktito-cli's --speakers),
/// their names, the name of speaker n at n - 1, "Speaker n" for one not
/// there or empty, and the diarizer that found them, where it is known.
/// Without names a file has no speakers, whatever its segments know of
/// them. A lone voice is named in the text only with name_lone, as
/// pktito-cli names it (pktito-c does too): the apps show one voice as they
/// show none (Peter, 11-15), and their JSON keeps its speaker all the same.
#[derive(Clone, Copy)]
pub struct Transcript<'a> {
    pub name: &'a str,
    pub duration: f64,
    pub model: Option<&'a str>,
    pub segments: &'a [Segment],
    pub names: Option<&'a [String]>,
    pub diarizer: Option<&'a str>,
    pub name_lone: bool,
}

impl Transcript<'_> {
    /// The speaker of a segment, where the file shows speakers.
    fn speaker(&self, s: &Segment) -> Option<u32> {
        self.names.and(s.speaker)
    }

    /// The transcript as its text and cues name its speakers: none for a
    /// lone voice, unless name_lone.
    fn shown(&self) -> Self {
        let lone = || {
            let mut who = self.segments.iter().filter_map(|s| s.speaker);
            who.next().is_none_or(|a| who.all(|b| b == a))
        };
        Transcript {
            names: self.names.filter(|_| self.name_lone || !lone()),
            ..*self
        }
    }

    /// Speaker n's name: the one given, else "Speaker n", as pktito-c writes
    /// it (its core/cli.c, --speakers).
    fn name_of(&self, n: u32) -> String {
        self.names
            .and_then(|names| names.get((n as usize).wrapping_sub(1)))
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map_or_else(|| format!("Speaker {n}"), str::to_string)
    }
}

/// t in a format: times before each sentence of .txt and .md if asked.
pub fn text(t: &Transcript, ext: &str, times: bool) -> Result<String, String> {
    text_windows(t, ext, times, &[])
}

/// text(), and in json the chunks pktito transcribed, where windows has
/// them (pktito-cli, for measuring the cuts; the apps' files leave them out).
pub fn text_windows(
    t: &Transcript,
    ext: &str,
    times: bool,
    windows: &[Window],
) -> Result<String, String> {
    // the JSON's speakers are the transcript's own, a lone voice's too
    let all = t;
    let t = &t.shown();
    let segs = t.segments;
    // the name that starts each segment beginning a turn, where the file shows
    // speakers: a turn is a speaker's segments in a row
    let turns: Vec<Option<String>> = segs
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let who = t.speaker(s)?;
            let before = i.checked_sub(1).and_then(|p| t.speaker(&segs[p]));
            (before != Some(who)).then(|| t.name_of(who))
        })
        .collect();
    Ok(match ext {
        // a turn starts after a blank line, with its speaker's name, as
        // pktito-c writes it (cli.c's print_sentences)
        "txt" => {
            let mut out = String::new();
            for (i, (s, name)) in segs.iter().zip(&turns).enumerate() {
                if i > 0 && name.is_some() {
                    out.push('\n');
                }
                if times {
                    out.push_str(&stamp(s.start));
                    out.push_str("  ");
                }
                if let Some(n) = name {
                    out.push_str(n);
                    out.push_str(": ");
                }
                out.push_str(&s.text);
                out.push('\n');
            }
            if out.is_empty() {
                out.push('\n');
            }
            out
        }
        "md" => {
            let paras: Vec<String> = segs
                .iter()
                .zip(&turns)
                .map(|(s, name)| {
                    let said = match name {
                        Some(n) => format!("**{}:** {}", md_esc(n), md_esc(&s.text)),
                        None => md_esc(&s.text),
                    };
                    if times {
                        format!("`{}` {said}", stamp(s.start))
                    } else {
                        said
                    }
                })
                .collect();
            format!("# {}\n\n{}\n", md_esc(t.name), paras.join("\n\n"))
        }
        "srt" => cues(t)?
            .iter()
            .enumerate()
            .map(|(i, c)| {
                format!(
                    "{}\n{} --> {}\n{}\n\n",
                    i + 1,
                    ms_stamp(c.start, ','),
                    ms_stamp(c.end, ','),
                    c.text
                )
            })
            .collect(),
        // with speakers each cue names its voice too, as pktito-c's does
        "vtt" => {
            let body: String = cues(t)?
                .iter()
                .enumerate()
                .map(|(i, c)| {
                    let voice = c.speaker.map_or(String::new(), |n| {
                        format!("<v {}>", vtt_text(&t.name_of(n)))
                    });
                    format!(
                        "{}\n{} --> {}\n{voice}{}\n\n",
                        i + 1,
                        ms_stamp(c.start, '.'),
                        ms_stamp(c.end, '.'),
                        vtt_text(&c.text)
                    )
                })
                .collect();
            format!("WEBVTT\n\n{body}")
        }
        "json" => json(all, &cues(all)?, windows),
        _ => return Err(format!("No such format: {ext}")),
    })
}

/// A cue as the files write it: its times in milliseconds, its text with a
/// line break where its second line begins, and an ellipsis after one cut
/// at a pause in mid-sentence (PK_CUE_PAUSE); and its words, n of the
/// transcript's from the first, for the Android app's Captions view to
/// follow and find them, which the files leave out.
#[derive(serde::Serialize)]
pub struct Cue {
    pub start: i64,
    pub end: i64,
    pub text: String,
    pub first: usize,
    pub n: usize,
    /// Whose words they are, where the transcript shows speakers: a change
    /// of speaker ends a cue, so a cue is one speaker's.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speaker: Option<u32>,
}

const PK_CUE_PAUSE: i32 = 8;

/// The subtitle track of t: English's rules for a model of English alone,
/// and for any other the engine's own guess whether its words are English.
/// With speakers, a change of speaker ends a cue, and the first word of each
/// turn carries its speaker's name, "Speaker 1: ", so that its line counts
/// the name (pk_captions; pktito-c's --srt writes the same). The Android
/// app's Captions view shows them as they are (android.rs).
pub fn cues(t: &Transcript) -> Result<Vec<Cue>, String> {
    let (timed, words, raw) = captioned(t)?;
    Ok(raw
        .iter()
        .map(|c| {
            let (first, n) = (c.first as usize, c.n as usize);
            let mut text = String::new();
            for (k, w) in timed.iter().enumerate().skip(first).take(n) {
                if k > first {
                    text.push(if k as i32 == c.line2 { '\n' } else { ' ' });
                }
                text.push_str(&w.0);
            }
            if c.flags & PK_CUE_PAUSE != 0 {
                text.push('\u{2026}');
            }
            Cue {
                start: c.start,
                end: c.end,
                text,
                first,
                n,
                speaker: words.get(first).and_then(|s| *s),
            }
        })
        .collect())
}

/// The words' own language, before any name goes in: English for a model
/// of English alone, else the engine's guess from all of them
/// (pk_language), as pktito-c's file_language has it.
fn language(t: &Transcript, timed: &[(String, i64, i64)]) -> Option<String> {
    let english = match t.model.and_then(model::entry) {
        Some(e) if e.languages == ["en"] => 1,
        _ => -1,
    };
    pktito::captions_lang(timed, english)
}

/// t's words as pk_captions is given them, a turn's first with its name in
/// front where t shows speakers (shown: not a lone voice's, unless asked);
/// each word's speaker; and the cues.
#[allow(clippy::type_complexity)]
fn captioned(
    t: &Transcript,
) -> Result<(Vec<(String, i64, i64)>, Vec<Option<u32>>, Vec<pktito::Cue>), String> {
    let t = &t.shown();
    let ms = |s: f64| (s * 1000.0).round() as i64;
    let words: Vec<(&crate::transcribe::Word, Option<u32>)> = t
        .segments
        .iter()
        .flat_map(|s| s.words.iter().map(move |w| (w, t.speaker(s))))
        .collect();
    let mut timed: Vec<(String, i64, i64)> = words
        .iter()
        .map(|(w, _)| (w.text.clone(), ms(w.start), ms(w.end)))
        .collect();
    let lang = language(t, &timed);
    let speaker: Option<Vec<i32>> = words
        .iter()
        .any(|(_, s)| s.is_some())
        .then(|| words.iter().map(|(_, s)| s.unwrap_or(0) as i32).collect());
    if let Some(spk) = &speaker {
        for i in 0..timed.len() {
            if words[i].1.is_some() && (i == 0 || spk[i] != spk[i - 1]) {
                timed[i].0 = format!("{}: {}", t.name_of(spk[i] as u32), timed[i].0);
            }
        }
    }
    let raw = pktito::captions(&timed, speaker.as_deref(), lang.as_deref(), ms(t.duration))?;
    Ok((timed, words.iter().map(|(_, s)| *s).collect(), raw))
}

/// The document pktito-c's --json writes too, pktito/1, byte for byte but
/// for what only pktito-c adds (--fast's "fast", each chunk's tokens and
/// -v's "run"): the file, its length, the model and the words' language;
/// with speakers, the diarizer where it is known and each speaker present,
/// by id, a lone voice's too; the segments, each with its words; the cues,
/// each with its speaker and its lines, the SRT's; and with windows the
/// chunks pktito transcribed (pktito-cli, for measuring the cuts), each
/// with its words as the first of the transcript's and how many. A segment
/// to a few lines, a word to a line, a cue to a line.
fn json(t: &Transcript, cues: &[Cue], windows: &[Window]) -> String {
    let j = |s: &str| serde_json::to_string(s).unwrap();
    let seg = |s: &Segment| {
        let words: Vec<String> = s
            .words
            .iter()
            .map(|w| {
                format!(
                    "      {{\"word\": {}, \"start\": {}, \"end\": {}}}",
                    j(&w.text),
                    r3(w.start),
                    r3(w.end)
                )
            })
            .collect();
        let who = t
            .speaker(s)
            .map_or(String::new(), |n| format!(", \"speaker\": {n}"));
        format!(
            "    {{\"start\": {}, \"end\": {}{who}, \"text\": {}, \"words\": [\n{}\n    ]}}",
            r3(s.start),
            r3(s.end),
            j(&s.text),
            words.join(",\n")
        )
    };
    let present: std::collections::BTreeSet<u32> =
        t.segments.iter().filter_map(|s| t.speaker(s)).collect();
    let speakers = if present.is_empty() {
        String::new()
    } else {
        let list: Vec<String> = present
            .iter()
            .map(|&n| format!("    {{\"id\": {n}, \"name\": {}}}", j(&t.name_of(n))))
            .collect();
        let diarizer = t
            .diarizer
            .map_or(String::new(), |d| format!(",\n  \"diarizer\": {}", j(d)));
        format!("{diarizer},\n  \"speakers\": [\n{}\n  ]", list.join(",\n"))
    };
    // each word's speaker, for its cue's: a lone voice's too, which the
    // cue's lines leave unnamed
    let spoken: Vec<Option<u32>> = t
        .segments
        .iter()
        .flat_map(|s| s.words.iter().map(move |_| t.speaker(s)))
        .collect();
    let cue = |c: &Cue| {
        let who = spoken
            .get(c.first)
            .copied()
            .flatten()
            .map_or(String::new(), |n| format!(", \"speaker\": {n}"));
        let lines: Vec<String> = c.text.split('\n').map(j).collect();
        format!(
            "    {{\"start\": {}, \"end\": {}{who}, \"lines\": [{}]}}",
            r3(c.start as f64 / 1000.0),
            r3(c.end as f64 / 1000.0),
            lines.join(", ")
        )
    };
    let timed: Vec<(String, i64, i64)> = t
        .segments
        .iter()
        .flat_map(|s| s.words.iter().map(|w| (w.text.clone(), 0, 0)))
        .collect();
    let lang = language(t, &timed).map_or_else(|| "null".into(), |l| j(&l));
    let segs: Vec<String> = t.segments.iter().map(seg).collect();
    let cues: Vec<String> = cues.iter().map(cue).collect();
    // each chunk: its stretch, the silence to the next, its words as the
    // first of the transcript's words (counted across the segments) and
    // how many (a sentence carried across a cut has words in two), and the
    // stretches of it heard again
    let mut first = 0;
    let chunks: Vec<String> = windows
        .iter()
        .enumerate()
        .map(|(i, w)| {
            let gap = windows.get(i + 1).map_or("null".into(), |n| r3(n.start - w.end));
            let retried = match w.retried {
                0 => String::new(),
                n => format!(", \"retried\": {n}"),
            };
            let line = format!(
                "    {{\"start\": {}, \"end\": {}, \"gap\": {}, \"first_word\": {}, \"words\": {}{retried}}}",
                r3(w.start),
                r3(w.end),
                gap,
                first,
                w.words
            );
            first += w.words;
            line
        })
        .collect();
    let chunks = if chunks.is_empty() {
        String::new()
    } else {
        format!(",\n  \"chunks\": [\n{}\n  ]", chunks.join(",\n"))
    };
    format!(
        "{{\n  \"format\": \"pktito/1\",\n  \"file\": {},\n  \"duration\": {},\n  \"model\": {},\n  \"language\": {lang}{speakers},\n  \"segments\": [\n{}\n  ],\n  \"captions\": [\n{}\n  ]{chunks}\n}}\n",
        j(t.name),
        r3(t.duration),
        t.model.filter(|m| !m.is_empty()).map(j).unwrap_or_else(|| "null".into()),
        segs.join(",\n"),
        cues.join(",\n"),
    )
}

/// A number to three decimals, written as JavaScript writes it: the
/// shortest digits that read back the same, "60" for 60.0, never "-0"
fn r3(x: f64) -> String {
    let v = (x * 1000.0).round() / 1000.0;
    format!("{}", if v == 0.0 { 0.0 } else { v })
}

/// hh:mm:ss of whole seconds
fn hms(s: f64) -> String {
    let s = s.max(0.0).floor() as u64;
    format!("{:02}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
}

/// hh:mm:ss.cc, the window's own time: hundredths, rounded
fn stamp(s: f64) -> String {
    let cs = (s * 100.0).round().max(0.0) as u64;
    format!("{}.{:02}", hms((cs / 100) as f64), cs % 100)
}

/// hh:mm:ss,mmm or hh:mm:ss.mmm of milliseconds, as SRT and WebVTT write one
fn ms_stamp(ms: i64, sep: char) -> String {
    let ms = ms.max(0);
    format!("{}{sep}{:03}", hms((ms / 1000) as f64), ms % 1000)
}

/// What Markdown would read as markup, as text: a backslash before each of
/// \ ` * _ [ ] < >, and before a heading's #, a list's number and period or
/// its - or + and space at the start
fn md_esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if matches!(c, '\\' | '`' | '*' | '_' | '[' | ']' | '<' | '>') {
            out.push('\\');
        }
        out.push(c);
    }
    let digits = out.bytes().take_while(u8::is_ascii_digit).count();
    let marker = out.starts_with('#')
        || (digits > 0 && out.as_bytes().get(digits) == Some(&b'.'))
        || out.starts_with("- ")
        || out.starts_with("+ ");
    if marker {
        out.insert(0, '\\');
    }
    out
}

/// WebVTT reads & < > as markup, so the words' are written as text
fn vtt_text(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcribe::Word;

    fn word(text: &str, start: f64, end: f64) -> Word {
        Word {
            start,
            end,
            text: text.into(),
        }
    }

    fn segment(words: Vec<Word>) -> Segment {
        let text = words
            .iter()
            .map(|w| w.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        Segment {
            start: words[0].start,
            end: words.last().unwrap().end,
            text,
            words,
            speaker: None,
        }
    }

    fn said(speaker: u32, words: Vec<Word>) -> Segment {
        Segment {
            speaker: Some(speaker),
            ..segment(words)
        }
    }

    /// Turns, a one-sentence interruption, and a name given to one speaker
    /// of two, in every format; and the same segments without names, which
    /// have no speakers in any.
    #[test]
    fn speakers_in_every_format() {
        let segs = vec![
            said(1, vec![word("Good", 0.5, 0.8), word("morning.", 0.8, 1.3)]),
            said(1, vec![word("Shall", 1.6, 1.9), word("we?", 1.9, 2.3)]),
            said(2, vec![word("Yes.", 2.6, 3.0)]),
            said(1, vec![word("Then", 3.4, 3.7), word("begin.", 3.7, 4.2)]),
        ];
        let names = vec!["Ada <A>".to_string()];
        let t = Transcript {
            name: "talk.wav",
            duration: 5.0,
            model: Some("parakeet-tdt-0.6b-v2"),
            segments: &segs,
            names: Some(&names),
            diarizer: Some("Nemotron-3-Diarization"),
            name_lone: false,
        };
        assert_eq!(
            text(&t, "txt", false).unwrap(),
            "Ada <A>: Good morning.\nShall we?\n\nSpeaker 2: Yes.\n\nAda <A>: Then begin.\n"
        );
        assert_eq!(
            text(&t, "txt", true).unwrap(),
            "00:00:00.50  Ada <A>: Good morning.\n00:00:01.60  Shall we?\n\n00:00:02.60  Speaker 2: Yes.\n\n00:00:03.40  Ada <A>: Then begin.\n"
        );
        assert_eq!(
            text(&t, "md", false).unwrap(),
            "# talk.wav\n\n**Ada \\<A\\>:** Good morning.\n\nShall we?\n\n**Speaker 2:** Yes.\n\n**Ada \\<A\\>:** Then begin.\n"
        );
        // a change of speaker ends a cue, and a turn's first cue names them;
        // the engine cuts the rest where it reads best (here at the pause)
        let c: Vec<(Option<u32>, String)> = cues(&t)
            .unwrap()
            .into_iter()
            .map(|c| (c.speaker, c.text))
            .collect();
        assert_eq!(
            c,
            [
                (Some(1), "Ada <A>: Good morning.".to_string()),
                (Some(1), "Shall we?".to_string()),
                (Some(2), "Speaker 2: Yes.".to_string()),
                (Some(1), "Ada <A>: Then begin.".to_string()),
            ]
        );
        let vtt = text(&t, "vtt", false).unwrap();
        assert!(
            vtt.contains("\n<v Ada &lt;A&gt;>Ada &lt;A&gt;: Good morning."),
            "{vtt}"
        );
        assert!(vtt.contains("\n<v Speaker 2>Speaker 2: Yes.\n"), "{vtt}");
        let srt = text(&t, "srt", false).unwrap();
        assert!(
            srt.contains("\nSpeaker 2: Yes.\n") && !srt.contains("<v"),
            "{srt}"
        );
        let json = text(&t, "json", false).unwrap();
        assert!(
            json.contains(
                "\n  \"diarizer\": \"Nemotron-3-Diarization\",\n  \"speakers\": [\n    {\"id\": 1, \"name\": \"Ada <A>\"},\n    {\"id\": 2, \"name\": \"Speaker 2\"}\n  ],\n"
            ),
            "{json}"
        );
        assert!(
            json.contains(
                "{\"start\": 2.6, \"end\": 3, \"speaker\": 2, \"text\": \"Yes.\", \"words\": [\n"
            ),
            "{json}"
        );
        assert!(
            json.contains("\"speaker\": 2, \"lines\": [\"Speaker 2: Yes.\"]}"),
            "{json}"
        );
        // no names: no speakers anywhere
        let plain = Transcript { names: None, ..t };
        assert_eq!(
            text(&plain, "txt", false).unwrap(),
            "Good morning.\nShall we?\nYes.\nThen begin.\n"
        );
        for ext in ["srt", "vtt", "json"] {
            let s = text(&plain, ext, false).unwrap();
            assert!(
                !s.contains("Speaker") && !s.contains("speaker"),
                "{ext}: {s}"
            );
        }
    }

    /// One voice: the apps' text and cues name no one, as with speakers off,
    /// and the JSON keeps the speaker; pktito-cli's (name_lone) name it, as
    /// pktito-c's do.
    #[test]
    fn a_lone_voice() {
        let segs = vec![
            said(1, vec![word("Good", 0.5, 0.8), word("morning.", 0.8, 1.3)]),
            said(1, vec![word("Shall", 1.6, 1.9), word("we?", 1.9, 2.3)]),
        ];
        let t = Transcript {
            name: "talk.wav",
            duration: 3.0,
            model: Some("parakeet-tdt-0.6b-v2"),
            segments: &segs,
            names: Some(&[]),
            diarizer: Some("Nemotron-3-Diarization"),
            name_lone: false,
        };
        assert_eq!(
            text(&t, "txt", false).unwrap(),
            "Good morning.\nShall we?\n"
        );
        assert!(!text(&t, "srt", false).unwrap().contains("Speaker"));
        let json = text(&t, "json", false).unwrap();
        assert!(
            json.contains("\"speakers\": [\n    {\"id\": 1, \"name\": \"Speaker 1\"}\n  ],"),
            "{json}"
        );
        assert!(
            json.contains("\"speaker\": 1, \"text\": \"Good morning.\""),
            "{json}"
        );
        assert!(
            json.contains("\"speaker\": 1, \"lines\": [\"Good morning.\"]"),
            "{json}"
        );
        let named = Transcript {
            name_lone: true,
            ..t
        };
        assert_eq!(
            text(&named, "txt", false).unwrap(),
            "Speaker 1: Good morning.\nShall we?\n"
        );
        assert!(text(&named, "json", false)
            .unwrap()
            .contains("\"lines\": [\"Speaker 1: Good morning."),);
    }

    /// The whole document, pktito/1, as pktito-c's --json writes it too
    /// (its tokens aside): with pktito-cli's chunks, one heard again.
    #[test]
    fn the_json_document() {
        let segs = vec![
            said(1, vec![word("Good", 0.5, 0.8), word("morning.", 0.8, 1.3)]),
            said(2, vec![word("Yes.", 2.6, 3.0)]),
        ];
        let t = Transcript {
            name: "talk.wav",
            duration: 4.0,
            model: Some("parakeet-tdt-0.6b-v2"),
            segments: &segs,
            names: Some(&[]),
            diarizer: Some("Nemotron-3-Diarization"),
            name_lone: true,
        };
        let windows = [
            Window {
                start: 0.25,
                end: 2.4,
                words: 2,
                retried: 0,
            },
            Window {
                start: 2.4,
                end: 4.0,
                words: 1,
                retried: 1,
            },
        ];
        assert_eq!(text_windows(&t, "json", false, &windows).unwrap(), DOCUMENT);
    }

    const DOCUMENT: &str = r#"{
  "format": "pktito/1",
  "file": "talk.wav",
  "duration": 4,
  "model": "parakeet-tdt-0.6b-v2",
  "language": "en",
  "diarizer": "Nemotron-3-Diarization",
  "speakers": [
    {"id": 1, "name": "Speaker 1"},
    {"id": 2, "name": "Speaker 2"}
  ],
  "segments": [
    {"start": 0.5, "end": 1.3, "speaker": 1, "text": "Good morning.", "words": [
      {"word": "Good", "start": 0.5, "end": 0.8},
      {"word": "morning.", "start": 0.8, "end": 1.3}
    ]},
    {"start": 2.6, "end": 3, "speaker": 2, "text": "Yes.", "words": [
      {"word": "Yes.", "start": 2.6, "end": 3}
    ]}
  ],
  "captions": [
    {"start": 0.5, "end": 1.8, "speaker": 1, "lines": ["Speaker 1: Good morning."]},
    {"start": 2.6, "end": 3.6, "speaker": 2, "lines": ["Speaker 2: Yes."]}
  ],
  "chunks": [
    {"start": 0.25, "end": 2.4, "gap": 0, "first_word": 0, "words": 2},
    {"start": 2.4, "end": 4, "gap": null, "first_word": 2, "words": 1, "retried": 1}
  ]
}
"#;

    #[test]
    fn times_as_the_window_writes_them() {
        assert_eq!(stamp(0.001), "00:00:00.00");
        assert_eq!(stamp(3725.456), "01:02:05.46");
        assert_eq!(stamp(-1.0), "00:00:00.00");
        assert_eq!(ms_stamp(2721, ','), "00:00:02,721");
        assert_eq!(ms_stamp(3_725_046, '.'), "01:02:05.046");
        assert_eq!(r3(60.0), "60");
        assert_eq!(r3(2.7214), "2.721");
        assert_eq!(r3(-0.0001), "0");
    }

    #[test]
    fn markup_written_as_text() {
        assert_eq!(
            md_esc("a *b* [c] <d> e_f `g` h\\i"),
            "a \\*b\\* \\[c\\] \\<d\\> e\\_f \\`g\\` h\\\\i"
        );
        assert_eq!(md_esc("# not a heading"), "\\# not a heading");
        assert_eq!(md_esc("1999. A year"), "\\1999. A year");
        assert_eq!(md_esc("- not a list"), "\\- not a list");
        assert_eq!(md_esc("1999 was a year"), "1999 was a year");
        assert_eq!(vtt_text("a < b & c > d"), "a &lt; b &amp; c &gt; d");
    }

    #[test]
    fn text_and_markdown() {
        let segs = vec![
            segment(vec![word("Hello", 0.0, 0.4), word("there.", 0.4, 0.9)]),
            segment(vec![word("# Two", 61.5, 62.0)]),
        ];
        let t = Transcript {
            name: "a_b.mp3",
            duration: 63.0,
            model: Some("parakeet-tdt-0.6b-v2"),
            segments: &segs,
            names: None,
            diarizer: None,
            name_lone: false,
        };
        assert_eq!(text(&t, "txt", false).unwrap(), "Hello there.\n# Two\n");
        assert_eq!(
            text(&t, "txt", true).unwrap(),
            "00:00:00.00  Hello there.\n00:01:01.50  # Two\n"
        );
        assert_eq!(
            text(&t, "md", true).unwrap(),
            "# a\\_b.mp3\n\n`00:00:00.00` Hello there.\n\n`00:01:01.50` \\# Two\n"
        );
        assert!(text(&t, "doc", false).is_err());
    }
}
