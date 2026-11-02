//! How far a transcript is from a reference transcript of the same
//! recording, as pktito-cli's bench says it (AliceBench): the same, or by
//! two numbers. The word error rate, as published figures count it: the
//! words that would have to be changed, added or removed, over the
//! reference's words, with case and punctuation set aside. And the
//! character edits, on the text as it is, which cost what a reader sees: a
//! comma is one, "tonight" for "to night" one (a word error rate counts it
//! as two words), a word in other case one, a word heard wrong several. Both read the text alone,
//! not how it is split into sentences or lines, so a change to the
//! sentences moves neither (Peter, 2026-11-02: an engine that differs from
//! the reference by a comma here and there costs a comma's worth).

/// A transcript against its reference.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct Score {
    /// the two texts the same, but for how they are split
    pub identical: bool,
    /// the reference's words, case and punctuation set aside
    pub words: usize,
    /// the words changed, added or removed from the reference's
    pub word_errors: usize,
    /// word_errors over words, in percent
    pub wer: f64,
    /// characters changed, added or removed from the reference's text
    pub char_edits: usize,
    /// the places they differ, the first few: the reference's words, then
    /// the transcript's, each with a word either side
    pub diffs: Vec<(String, String)>,
}

/// How many of the differing places are kept, for a reader to see what kind
/// of difference it is.
const DIFFS_KEPT: usize = 8;

/// got against reference: each read as its words, whitespace of any kind
/// (line ends, the sentences' splits) between them.
pub fn score(got: &str, reference: &str) -> Score {
    let a: Vec<&str> = reference.split_whitespace().collect();
    let b: Vec<&str> = got.split_whitespace().collect();

    // the word error rate: the words lowercased, their punctuation off
    let plain = |w: &&str| -> String {
        w.chars()
            .filter(|c| c.is_alphanumeric() || *c == '\'')
            .flat_map(char::to_lowercase)
            .collect()
    };
    let pa: Vec<String> = a.iter().map(plain).filter(|w| !w.is_empty()).collect();
    let pb: Vec<String> = b.iter().map(plain).filter(|w| !w.is_empty()).collect();
    let word_errors = distance(&pa, &pb);

    // the character edits: the words as they are aligned, and each place
    // they differ costed by its characters, the words joined by spaces
    let mut char_edits = 0;
    let mut diffs = Vec::new();
    for (i, j, k, l) in differing(&a, &b) {
        let x = a[i..j].join(" ");
        let y = b[k..l].join(" ");
        // words added or removed take a space with them, where there is a
        // word beside them to have been spaced from
        let space = usize::from((y.is_empty() && j - i < a.len()) || (x.is_empty() && l - k < b.len()));
        // case aside, then a word whose case alone changed one edit, as a
        // comma is: each letter of it, a title read in capitals by one
        // engine and not the other cost more than a word heard wrong
        // (AliceBench, the Neural Engine against the CPU, 2026-11-02: 141
        // edits so, 81 this way; chapter 2's 51 now 17)
        let xc: Vec<char> = x.chars().flat_map(char::to_lowercase).collect();
        let yc: Vec<char> = y.chars().flat_map(char::to_lowercase).collect();
        char_edits += distance(&xc, &yc) + space + recased(&a[i..j], &b[k..l]);
        if diffs.len() < DIFFS_KEPT {
            // the place's words marked as one, a word either side
            let around = |w: &[&str], s: usize, e: usize| {
                let mut t = Vec::new();
                if s > 0 {
                    t.push(w[s - 1].to_string());
                }
                t.push(format!("»{}«", w[s..e].join(" ")));
                if e < w.len() {
                    t.push(w[e].to_string());
                }
                t.join(" ")
            };
            diffs.push((around(&a, i, j), around(&b, k, l)));
        }
    }
    Score {
        identical: a == b,
        words: pa.len(),
        word_errors,
        wer: if pa.is_empty() { 0.0 } else { 100.0 * word_errors as f64 / pa.len() as f64 },
        char_edits,
        diffs,
    }
}

/// The words of a whose case alone changed in b: each matched to a word of
/// b that is it but for case, once.
fn recased(a: &[&str], b: &[&str]) -> usize {
    let mut used = vec![false; b.len()];
    a.iter()
        .filter(|w| {
            let at = b.iter().enumerate().position(|(n, v)| {
                !used[n] && v != *w && v.to_lowercase() == w.to_lowercase()
            });
            at.map(|n| used[n] = true).is_some()
        })
        .count()
}

/// The edit distance between two sequences (Levenshtein's): the fewest
/// items changed, added or removed to make b of a. Two rows, so a long
/// chapter's words, or a place's characters, take little memory.
fn distance<T: PartialEq>(a: &[T], b: &[T]) -> usize {
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, x) in a.iter().enumerate() {
        let mut diag = row[0];
        row[0] = i + 1;
        for (j, y) in b.iter().enumerate() {
            let up = row[j + 1];
            row[j + 1] = if x == y { diag } else { 1 + diag.min(up).min(row[j]) };
            diag = up;
        }
    }
    row[b.len()]
}

/// The places a and b differ, as their words aligned by the fewest edits:
/// each a stretch a[i..j] that b has as b[k..l], between words both have.
/// The whole table is kept to read the alignment back, a byte a cell: a
/// chapter of AliceBench (3,000 to 4,000 words) takes 9 to 16 MB, once.
fn differing(a: &[&str], b: &[&str]) -> Vec<(usize, usize, usize, usize)> {
    // trim what they share at either end, which is most of it
    let pre = a.iter().zip(b).take_while(|(x, y)| x == y).count();
    let suf = a[pre..].iter().rev().zip(b[pre..].iter().rev()).take_while(|(x, y)| x == y).count();
    let (a2, b2) = (&a[pre..a.len() - suf], &b[pre..b.len() - suf]);
    let (n, m) = (a2.len(), b2.len());
    if n == 0 && m == 0 {
        return Vec::new();
    }
    // step[i][j]: how cell (i, j) was reached: 0 a match, 1 a change, 2 a
    // word of a removed, 3 a word of b added
    let mut step = vec![0u8; (n + 1) * (m + 1)];
    let mut row: Vec<usize> = (0..=m).collect();
    step[1..=m].fill(3);
    for i in 1..=n {
        let mut diag = row[0];
        row[0] = i;
        step[i * (m + 1)] = 2;
        for j in 1..=m {
            let up = row[j];
            let (cost, how) = if a2[i - 1] == b2[j - 1] {
                (diag, 0)
            } else if diag <= up && diag <= row[j - 1] {
                (diag + 1, 1)
            } else if up <= row[j - 1] {
                (up + 1, 2)
            } else {
                (row[j - 1] + 1, 3)
            };
            row[j] = cost;
            step[i * (m + 1) + j] = how;
            diag = up;
        }
    }
    // back from the end, a run of edits between two matches one place
    let (mut i, mut j) = (n, m);
    let mut places = Vec::new();
    let mut end: Option<(usize, usize)> = None;
    while i > 0 || j > 0 {
        let how = step[i * (m + 1) + j];
        if how == 0 {
            if let Some((ei, ej)) = end.take() {
                places.push((pre + i, pre + ei, pre + j, pre + ej));
            }
            i -= 1;
            j -= 1;
        } else {
            end.get_or_insert((i, j));
            match how {
                1 => {
                    i -= 1;
                    j -= 1;
                }
                2 => i -= 1,
                _ => j -= 1,
            }
        }
    }
    if let Some((ei, ej)) = end {
        places.push((pre, pre + ei, pre, pre + ej));
    }
    places.reverse();
    places
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_text_split_differently_is_identical() {
        let s = score("Alice was beginning.\nTo get very tired.", "Alice was beginning. To get\nvery tired.");
        assert!(s.identical);
        assert_eq!((s.word_errors, s.char_edits), (0, 0));
        assert!(s.diffs.is_empty());
    }

    #[test]
    fn a_comma_is_one_character_and_no_word() {
        let s = score("Oh dear, oh dear!", "Oh dear oh dear!");
        assert!(!s.identical);
        assert_eq!((s.word_errors, s.char_edits), (0, 1));
        assert_eq!(s.diffs, vec![("Oh »dear« oh".into(), "Oh »dear,« oh".into())]);
    }

    #[test]
    fn a_split_word_is_one_character_and_two_words() {
        let s = score("I'll go to night.", "I'll go tonight.");
        assert_eq!((s.word_errors, s.char_edits), (2, 1));
    }

    #[test]
    fn a_word_heard_wrong_costs_its_letters() {
        let s = score("the white rabbit hurried", "the white rabbit worried");
        assert_eq!((s.word_errors, s.char_edits), (1, 2));
        assert_eq!(s.words, 4);
        assert!((s.wer - 25.0).abs() < 1e-9);
    }

    #[test]
    fn a_word_added_or_dropped_costs_it_and_its_space() {
        let s = score("down down down the well", "down down the well");
        assert_eq!((s.word_errors, s.char_edits), (1, 5));
        let s = score("down the well", "down down the well");
        assert_eq!((s.word_errors, s.char_edits), (1, 5));
        assert_eq!(s.diffs, vec![("down »down« the".into(), "down »« the".into())]);
    }

    #[test]
    fn a_place_of_several_words_is_marked_as_one() {
        let s = score("or surface breathing animals", "or surface-breathing animals");
        assert_eq!(s.diffs, vec![("or »surface-breathing« animals".into(), "or »surface breathing« animals".into())]);
    }

    #[test]
    fn a_word_in_other_case_is_one_edit() {
        let s = score("BRAD BUSH ALICE'S ADVENTURES BY LOWIS CARROL", "Brad Bush Alice's Adventures by Lewis Carroll");
        // five words recased, "Lewis" and "Carroll" a letter each
        assert_eq!(s.char_edits, 5 + 2);
        assert_eq!(s.word_errors, 2);
        let s = score("the King had", "the king had");
        assert_eq!(s.char_edits, 1);
    }

    #[test]
    fn places_apart_are_counted_apart() {
        let s = score("A, b c d e f g h, i", "A b c d e f g h i");
        assert_eq!(s.char_edits, 2);
        assert_eq!(s.diffs.len(), 2);
    }

    #[test]
    fn an_empty_transcript_is_every_word_wrong() {
        let s = score("", "one two three");
        assert_eq!(s.word_errors, 3);
        assert!((s.wer - 100.0).abs() < 1e-9);
        assert_eq!(s.char_edits, "one two three".len());
    }
}
