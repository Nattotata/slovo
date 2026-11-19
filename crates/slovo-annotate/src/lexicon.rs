//! A lexicon of form → lemma corrections.
//!
//! Taggers are trained on modern text and fail on archaic forms: they lemmatize
//! `spake` as `spake`, not `speak`. A lexicon patches exactly those cases.
//!
//! It is **non-destructive**. The tagger's `lemma` is left as it was, and the
//! correction is written to a *separate* column (`lemma_override`), so you can
//! see what the model thought and what the lexicon says, side by side. A query
//! picks the layer it wants (`[lemma=…]` versus `[lemma_override=…]`).
//!
//! Format: one entry per line, TAB-separated, `#` starts a comment.
//!
//! ```text
//! # archaic verb forms
//! spake    speak
//! wrought    work
//! hath    have    AUX
//! ```
//!
//! The third column is optional and forces a UPOS tag, for cases where the
//! tagger also mis-tags (an archaic verb read as a noun). Matching is on the
//! token's surface form, case-insensitively.

use std::collections::HashMap;

use crate::error::{Error, Result};
use crate::Sentence;

/// One correction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub lemma: String,
    /// A forced UPOS, when the lexicon wants to correct the tag too.
    pub upos: Option<String>,
}

/// A form → correction map.
#[derive(Debug, Clone, Default)]
pub struct Lexicon {
    entries: HashMap<String, Entry>,
}

impl Lexicon {
    /// An empty lexicon, which changes nothing.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Parse a lexicon from TSV text.
    pub fn parse(text: &str) -> Self {
        let mut entries = HashMap::new();
        for line in text.lines() {
            let line = line.trim_end_matches('\r');
            if line.trim().is_empty() || line.trim_start().starts_with('#') {
                continue;
            }
            let mut cols = line.split('\t');
            let (Some(form), Some(lemma)) = (cols.next(), cols.next()) else {
                continue;
            };
            let form = form.trim();
            let lemma = lemma.trim();
            if form.is_empty() || lemma.is_empty() {
                continue;
            }
            let upos = cols
                .next()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string);
            entries.insert(
                form.to_lowercase(),
                Entry {
                    lemma: lemma.to_string(),
                    upos,
                },
            );
        }
        Self { entries }
    }

    /// Read a lexicon from a file.
    pub fn read(path: &std::path::Path) -> Result<Self> {
        let text = std::fs::read_to_string(path).map_err(|source| Error::Io {
            path: path.to_path_buf(),
            source,
        })?;
        Ok(Self::parse(&text))
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Look up a surface form.
    pub fn get(&self, form: &str) -> Option<&Entry> {
        self.entries.get(&form.to_lowercase())
    }

    /// Write corrections onto every matching token.
    ///
    /// `lemma` is left untouched; the correction goes to `lemma_override`, and
    /// the tag to `upos_override`, so nothing the tagger produced is lost.
    pub fn apply(&self, sentences: &mut [Sentence]) {
        if self.entries.is_empty() {
            return;
        }
        for sentence in sentences {
            for token in &mut sentence.tokens {
                if let Some(entry) = self.get(&token.form) {
                    token.lemma_override = Some(entry.lemma.clone());
                    if let Some(upos) = &entry.upos {
                        token.upos_override = Some(upos.clone());
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Token;

    const SAMPLE: &str = "\
# archaic forms
spake\tspeak
wrought\twork
hath\thave\tAUX

quoth\tquoth
";

    #[test]
    fn parses_entries_and_comments() {
        let lex = Lexicon::parse(SAMPLE);
        assert_eq!(lex.len(), 4);
        assert_eq!(lex.get("spake").unwrap().lemma, "speak");
        assert_eq!(lex.get("hath").unwrap().upos.as_deref(), Some("AUX"));
        assert!(lex.get("hath").unwrap().upos.is_some());
        assert!(lex.get("nothing").is_none());
    }

    #[test]
    fn matches_case_insensitively() {
        let lex = Lexicon::parse(SAMPLE);
        assert!(lex.get("Spake").is_some());
        assert!(lex.get("SPAKE").is_some());
    }

    #[test]
    fn applies_without_touching_the_tagger_lemma() {
        let lex = Lexicon::parse(SAMPLE);
        let mut sentences = vec![Sentence {
            tokens: vec![
                Token {
                    form: "spake".into(),
                    lemma: Some("spake".into()), // the tagger's wrong guess
                    ..Token::default()
                },
                Token {
                    form: "loudly".into(),
                    lemma: Some("loudly".into()),
                    ..Token::default()
                },
            ],
            ..Sentence::default()
        }];

        lex.apply(&mut sentences);

        let tokens = &sentences[0].tokens;
        // Original lemma is preserved.
        assert_eq!(tokens[0].lemma.as_deref(), Some("spake"));
        // Correction lands in its own column.
        assert_eq!(tokens[0].lemma_override.as_deref(), Some("speak"));
        // Unmatched tokens are untouched.
        assert_eq!(tokens[1].lemma_override, None);
    }

    #[test]
    fn a_forced_upos_lands_in_its_own_column() {
        let lex = Lexicon::parse(SAMPLE);
        let mut sentences = vec![Sentence {
            tokens: vec![Token {
                form: "hath".into(),
                lemma: Some("hath".into()),
                upos: Some("NOUN".into()), // tagger's wrong guess
                ..Token::default()
            }],
            ..Sentence::default()
        }];
        lex.apply(&mut sentences);

        let token = &sentences[0].tokens[0];
        assert_eq!(token.upos.as_deref(), Some("NOUN"), "tagger tag preserved");
        assert_eq!(token.upos_override.as_deref(), Some("AUX"));
    }

    #[test]
    fn an_empty_lexicon_changes_nothing() {
        let lex = Lexicon::empty();
        assert!(lex.is_empty());
        let mut sentences = vec![Sentence {
            tokens: vec![Token {
                form: "spake".into(),
                lemma: Some("spake".into()),
                ..Token::default()
            }],
            ..Sentence::default()
        }];
        lex.apply(&mut sentences);
        assert_eq!(sentences[0].tokens[0].lemma_override, None);
    }
}
