//! A transcript in the formats Save writes, for the window and pktito-cli
//! alike, so that the two write the same bytes: text and Markdown of its
//! sentences, each with its time if asked; SRT and WebVTT of its subtitle
//! track, the engine's cues (pk_captions); and JSON with every word's time
//! and the cues as their lines.

use crate::model;
use crate::pktito;
use crate::transcribe::Segment;
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
/// length in seconds, the model that made it, and its segments.
pub struct Transcript<'a> {
    pub name: &'a str,
    pub duration: f64,
    pub model: Option<&'a str>,
    pub segments: &'a [Segment],
}

/// t in a format: times before each sentence of .txt and .md if asked.
pub fn text(t: &Transcript, ext: &str, times: bool) -> Result<String, String> {
    let segs = t.segments;
    Ok(match ext {
        "txt" => {
            let lines: Vec<String> = segs
                .iter()
                .map(|s| {
                    if times {
                        format!("{}  {}", stamp(s.start), s.text)
                    } else {
                        s.text.clone()
                    }
                })
                .collect();
            lines.join("\n") + "\n"
        }
        "md" => {
            let paras: Vec<String> = segs
                .iter()
                .map(|s| {
                    if times {
                        format!("`{}` {}", stamp(s.start), md_esc(&s.text))
                    } else {
                        md_esc(&s.text)
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
        "vtt" => {
            let body: String = cues(t)?
                .iter()
                .enumerate()
                .map(|(i, c)| {
                    format!(
                        "{}\n{} --> {}\n{}\n\n",
                        i + 1,
                        ms_stamp(c.start, '.'),
                        ms_stamp(c.end, '.'),
                        vtt_text(&c.text)
                    )
                })
                .collect();
            format!("WEBVTT\n\n{body}")
        }
        "json" => json(t, &cues(t)?),
        _ => return Err(format!("No such format: {ext}")),
    })
}

/// A cue as the files write it: its times in milliseconds, its text with a
/// line break where its second line begins, and an ellipsis after one cut
/// at a pause in mid-sentence (PK_CUE_PAUSE)
#[derive(serde::Serialize)]
pub struct Cue {
    pub start: i64,
    pub end: i64,
    pub text: String,
}

const PK_CUE_PAUSE: i32 = 8;

/// The subtitle track of t: English's rules for a model of English alone,
/// and for any other the engine's own guess whether its words are English.
/// The Android app's Captions view shows them as they are (android.rs).
pub fn cues(t: &Transcript) -> Result<Vec<Cue>, String> {
    let ms = |s: f64| (s * 1000.0).round() as i64;
    let words: Vec<&crate::transcribe::Word> = t.segments.iter().flat_map(|s| &s.words).collect();
    let timed: Vec<(String, i64, i64)> = words
        .iter()
        .map(|w| (w.text.clone(), ms(w.start), ms(w.end)))
        .collect();
    let english = match t.model.and_then(model::entry) {
        Some(e) if e.languages == ["en"] => 1,
        _ => -1,
    };
    Ok(pktito::captions(&timed, english, ms(t.duration))?
        .iter()
        .map(|c| {
            let (first, n) = (c.first as usize, c.n as usize);
            let mut text = String::new();
            for (k, w) in words[first..first + n].iter().enumerate() {
                if k > 0 {
                    text.push(if (first + k) as i32 == c.line2 {
                        '\n'
                    } else {
                        ' '
                    });
                }
                text.push_str(&w.text);
            }
            if c.flags & PK_CUE_PAUSE != 0 {
                text.push('\u{2026}');
            }
            Cue {
                start: c.start,
                end: c.end,
                text,
            }
        })
        .collect())
}

/// A segment to a few lines, a word to a line, the cues as their lines
fn json(t: &Transcript, cues: &[Cue]) -> String {
    let j = |s: &str| serde_json::to_string(s).unwrap();
    let seg = |s: &Segment| {
        let words: Vec<String> = s
            .words
            .iter()
            .map(|w| {
                format!(
                    "      {{\"start\": {}, \"end\": {}, \"text\": {}}}",
                    r3(w.start),
                    r3(w.end),
                    j(&w.text)
                )
            })
            .collect();
        format!(
            "    {{\"start\": {}, \"end\": {}, \"text\": {}, \"words\": [\n{}\n    ]}}",
            r3(s.start),
            r3(s.end),
            j(&s.text),
            words.join(",\n")
        )
    };
    let cue = |c: &Cue| {
        let lines: Vec<&str> = c.text.split('\n').collect();
        format!(
            "    {{\"start\": {}, \"end\": {}, \"lines\": {}}}",
            r3(c.start as f64 / 1000.0),
            r3(c.end as f64 / 1000.0),
            serde_json::to_string(&lines).unwrap()
        )
    };
    let segs: Vec<String> = t.segments.iter().map(seg).collect();
    let cues: Vec<String> = cues.iter().map(cue).collect();
    format!(
        "{{\n  \"file\": {},\n  \"duration\": {},\n  \"model\": {},\n  \"segments\": [\n{}\n  ],\n  \"captions\": [\n{}\n  ]\n}}\n",
        j(t.name),
        r3(t.duration),
        t.model.filter(|m| !m.is_empty()).map(j).unwrap_or_else(|| "null".into()),
        segs.join(",\n"),
        cues.join(",\n")
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
        }
    }

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
