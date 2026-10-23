//! The words the window shows, in one place: ui/locales/en.json, a key for
//! each, which the page reads (main.rs hands it over before the page's
//! scripts run) and Rust reads for the few things it draws itself: the menu
//! bar, the Windows system menu, the file dialogs' filters, macOS's password
//! prompt. English is the only catalog so far: a translation would be
//! another file of the same keys, chosen here by the system's language,
//! with English under it for any key it lacks. pktito-cli and the engine
//! stay English.
//!
//! What went wrong travels as a Problem, a key and its values, and the
//! particulars (an OS's error, a server's answer) apart from it, which stay
//! as they were said: the window words the one and folds the other away.

use serde::Serialize;
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::sync::OnceLock;

const EN: &str = include_str!("../../ui/locales/en.json");

/// The catalog, as JSON, for the page.
pub fn json() -> &'static str {
    EN
}

fn catalog() -> &'static Map<String, Value> {
    static C: OnceLock<Map<String, Value>> = OnceLock::new();
    C.get_or_init(|| serde_json::from_str(EN).expect("ui/locales/en.json is a JSON object"))
}

/// key's words, {name} filled from args; a key the catalog lacks is itself,
/// which the tests (all_keys_are_in_the_catalog) keep from happening. A
/// plural key's "other" form: Rust words nothing by count.
pub fn tf(key: &str, args: &[(&str, &str)]) -> String {
    let words = match catalog().get(key) {
        Some(Value::String(s)) => s.as_str(),
        Some(Value::Object(forms)) => forms.get("other").and_then(Value::as_str).unwrap_or(key),
        _ => key,
    };
    let mut out = words.to_string();
    for (name, value) in args {
        out = out.replace(&format!("{{{name}}}"), value);
    }
    out
}

pub fn t(key: &str) -> String {
    tf(key, &[])
}

/// What went wrong: the catalog's key for it, the values its words take, and
/// the particulars, as whatever reported it put them ("" for none).
#[derive(Clone, Debug, Default, Serialize)]
pub struct Problem {
    pub key: &'static str,
    pub args: BTreeMap<&'static str, String>,
    pub detail: String,
}

impl Problem {
    pub fn new(key: &'static str) -> Problem {
        Problem { key, ..Default::default() }
    }

    pub fn with(mut self, name: &'static str, value: impl Into<String>) -> Problem {
        self.args.insert(name, value.into());
        self
    }

    pub fn detail(mut self, detail: impl ToString) -> Problem {
        self.detail = detail.to_string();
        self
    }

    /// Its words, in English, for pktito-cli.
    pub fn words(&self) -> String {
        let args: Vec<(&str, &str)> = self.args.iter().map(|(k, v)| (*k, v.as_str())).collect();
        tf(self.key, &args)
    }

    /// Its words and its particulars, in a line, for pktito-cli.
    pub fn line(&self) -> String {
        match self.detail.as_str() {
            "" => self.words(),
            d => format!("{} ({})", self.words(), d.replace('\n', "; ")),
        }
    }
}

/// What should not happen and has no words of its own: "Something went
/// wrong", and what said so as the particulars.
impl From<String> for Problem {
    fn from(detail: String) -> Problem {
        Problem::new("problem.unexpected").detail(detail)
    }
}

impl From<&str> for Problem {
    fn from(detail: &str) -> Problem {
        Problem::from(detail.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use std::path::Path;

    /// Keys put together at run time, which no file names whole: each catalog
    /// model's name and languages (model.rs, Entry::key; app.js, labelOf).
    const MADE: &[&str] = &["catalog."];

    /// How a key is named, up to its opening quote: the page's data-t
    /// attributes and t calls, and Rust's t, tf, Problem::new and Unreadable.
    const NAMED: &[&str] = &[
        "data-t=\"", "data-t-html=\"", "data-t-aria-label=\"", "data-t-title=\"", "data-t-placeholder=\"",
        "t(\"", "tf(\"", "Problem::new(\"", "Unreadable(\"",
    ];

    /// The keys named in text: after each of NAMED, not inside a longer name
    /// (set(" is not t("), the key's characters, then its closing quote.
    fn named(text: &str) -> Vec<String> {
        let mut out = vec![];
        for pre in NAMED {
            let mut at = 0;
            while let Some(i) = text[at..].find(pre) {
                let (begin, start) = (at + i, at + i + pre.len());
                at = start;
                let word = |c: char| c.is_ascii_alphanumeric() || c == '_' || c == ':';
                if text[..begin].chars().next_back().is_some_and(word) {
                    continue;
                }
                let key: String = text[start..].chars().take_while(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '_' || *c == '.').collect();
                if !key.is_empty() && text[start + key.len()..].starts_with('"') {
                    out.push(key);
                }
            }
        }
        out
    }

    /// Every key the page (ui/index.html, ui/app.js) and the app's Rust name
    /// is in the catalog, and the catalog has none that nothing names.
    #[test]
    fn all_keys_are_in_the_catalog() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let mut files = vec![root.join("ui/index.html"), root.join("ui/app.js")];
        for e in std::fs::read_dir(root.join("src-tauri/src")).unwrap().flatten() {
            if e.path().extension().is_some_and(|x| x == "rs") {
                files.push(e.path());
            }
        }
        let texts: Vec<String> = files.iter().map(|f| std::fs::read_to_string(f).unwrap()).collect();
        let used: BTreeSet<String> = texts.iter().flat_map(|t| named(t)).collect();
        let have: BTreeSet<String> = catalog().keys().cloned().collect();
        let missing: Vec<_> = used.difference(&have).collect();
        assert!(missing.is_empty(), "named but not in ui/locales/en.json: {missing:?}");
        // a key chosen among others (t(on ? "a" : "b"), a match's arms) is
        // named only as its quoted self
        let quoted = |k: &str| texts.iter().any(|t| t.contains(&format!("\"{k}\"")));
        let unused: Vec<_> = have.iter().filter(|k| !quoted(k) && !MADE.iter().any(|m| k.starts_with(m))).collect();
        assert!(unused.is_empty(), "in ui/locales/en.json but named nowhere: {unused:?}");
    }

    #[test]
    fn values_fill_and_unknown_keys_are_themselves() {
        let unknown = "no.such.key";
        assert_eq!(tf(unknown, &[]), unknown);
        let p = Problem::new("problem.unexpected").detail("a\nb");
        assert!(p.line().ends_with("(a; b)"), "{}", p.line());
    }
}
