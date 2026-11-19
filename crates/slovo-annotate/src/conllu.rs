//! CoNLL-U in and out.
//!
//! The writer emits the format `slovo import` reads: `# newdoc id`, `# sent_id`
//! and `# text` comments, ten tab-separated columns, blank line between
//! sentences.
//!
//! The reader is a small copy of the core's, kept here so this crate has no
//! dependency on `slovo-core` — the two share a *format*, not code, which is
//! what keeps them independently replaceable.

use std::io::BufRead;
use std::path::Path;

use crate::error::{Error, Result};
use crate::{Sentence, Token};

/// Write sentences as CoNLL-U.
pub fn write(sentences: &[Sentence]) -> String {
    let mut out = String::new();
    let mut last_doc: Option<&str> = None;

    for sentence in sentences {
        if let Some(doc) = sentence.newdoc_id.as_deref() {
            if last_doc != Some(doc) {
                out.push_str(&format!("# newdoc id = {doc}\n"));
                if let Some(title) = &sentence.newdoc_title {
                    out.push_str(&format!("# newdoc title = {title}\n"));
                }
                last_doc = Some(doc);
            }
        }
        if let Some(sent_id) = &sentence.sent_id {
            out.push_str(&format!("# sent_id = {sent_id}\n"));
        }
        if let Some(text) = &sentence.text {
            out.push_str(&format!("# text = {text}\n"));
        }

        for (i, token) in sentence.tokens.iter().enumerate() {
            out.push_str(&row(i + 1, token));
            out.push('\n');
        }
        out.push('\n');
    }

    out
}

fn row(id: usize, token: &Token) -> String {
    // The overrides ride in MISC, the standard home for token-level extras,
    // so the ten-column shape is unchanged and a reader that ignores MISC
    // still gets a valid row.
    let mut misc = Vec::new();
    if let Some(lemma) = &token.lemma_override {
        misc.push(format!("LemmaOverride={lemma}"));
    }
    if let Some(upos) = &token.upos_override {
        misc.push(format!("UposOverride={upos}"));
    }
    // Only write a weight when it differs from the default, so ordinary text
    // stays clean.
    if token.weight != crate::Weight::ONE {
        misc.push(format!("Weight={}", token.weight));
    }
    let misc = if misc.is_empty() {
        "_".to_string()
    } else {
        misc.join("|")
    };

    format!(
        "{id}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t_\t{}",
        token.form,
        token.lemma.as_deref().unwrap_or("_"),
        token.upos.as_deref().unwrap_or("_"),
        token.xpos.as_deref().unwrap_or("_"),
        token.feats.as_deref().unwrap_or("_"),
        token
            .head
            .map(|h| h.to_string())
            .unwrap_or_else(|| "_".into()),
        token.deprel.as_deref().unwrap_or("_"),
        misc,
    )
}

/// Pull `LemmaOverride` / `UposOverride` / `Weight` out of a MISC column.
fn parse_misc(misc: &str) -> (Option<String>, Option<String>, crate::Weight) {
    let mut lemma = None;
    let mut upos = None;
    let mut weight = crate::Weight::ONE;
    for field in misc.split('|') {
        if let Some(v) = field.strip_prefix("LemmaOverride=") {
            lemma = Some(v.to_string());
        } else if let Some(v) = field.strip_prefix("UposOverride=") {
            upos = Some(v.to_string());
        } else if let Some(v) = field.strip_prefix("Weight=") {
            if let Ok(value) = v.parse::<f64>() {
                weight = crate::Weight::new(value);
            }
        }
    }
    (lemma, upos, weight)
}

/// Read CoNLL-U into sentences. Used to consume a tagger's output.
pub fn parse_reader<R: BufRead>(reader: R, path: &str) -> Result<Vec<Sentence>> {
    let mut sentences = Vec::new();
    let mut tokens: Vec<Token> = Vec::new();
    let mut text = None;
    let mut sent_id = None;
    let mut newdoc_id = None;
    let mut newdoc_title = None;

    let flush = |tokens: &mut Vec<Token>,
                 text: &mut Option<String>,
                 sent_id: &mut Option<String>,
                 newdoc: &Option<String>,
                 title: &Option<String>,
                 out: &mut Vec<Sentence>| {
        if !tokens.is_empty() {
            out.push(Sentence {
                text: text.take(),
                sent_id: sent_id.take(),
                newdoc_id: newdoc.clone(),
                newdoc_title: title.clone(),
                tokens: std::mem::take(tokens),
            });
        } else {
            *text = None;
            *sent_id = None;
        }
    };

    for line in reader.lines() {
        let line = line.map_err(|source| Error::Io {
            path: Path::new(path).to_path_buf(),
            source,
        })?;
        let line = line.trim_end_matches('\r');

        if line.trim().is_empty() {
            flush(
                &mut tokens,
                &mut text,
                &mut sent_id,
                &newdoc_id,
                &newdoc_title,
                &mut sentences,
            );
            continue;
        }

        if let Some(rest) = line.strip_prefix('#') {
            let rest = rest.trim();
            if let Some(v) = rest.strip_prefix("text =") {
                text = Some(v.trim().to_string());
            } else if let Some(v) = rest.strip_prefix("sent_id =") {
                sent_id = Some(v.trim().to_string());
            } else if let Some(v) = rest.strip_prefix("newdoc id =") {
                if !tokens.is_empty() {
                    flush(
                        &mut tokens,
                        &mut text,
                        &mut sent_id,
                        &newdoc_id,
                        &newdoc_title,
                        &mut sentences,
                    );
                }
                newdoc_id = Some(v.trim().to_string());
            } else if let Some(v) = rest.strip_prefix("newdoc title =") {
                newdoc_title = Some(v.trim().to_string());
            }
            continue;
        }

        let mut cols: Vec<&str> = line.split('\t').collect();
        if cols.len() < 10 {
            cols = line.split_whitespace().collect();
        }
        if cols.len() < 10 {
            continue;
        }
        let id = cols[0];
        if id.contains('-') || id.contains('.') {
            continue;
        }

        let (lemma_override, upos_override, weight) = parse_misc(cols[9]);

        tokens.push(Token {
            form: cols[1].to_string(),
            lemma: opt(cols[2]),
            upos: opt(cols[3]),
            xpos: opt(cols[4]),
            feats: opt(cols[5]),
            head: cols[6].parse().ok(),
            deprel: opt(cols[7]),
            lemma_override,
            upos_override,
            weight,
        });
    }

    flush(
        &mut tokens,
        &mut text,
        &mut sent_id,
        &newdoc_id,
        &newdoc_title,
        &mut sentences,
    );
    Ok(sentences)
}

fn opt(value: &str) -> Option<String> {
    match value {
        "_" | "" => None,
        other => Some(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_sentences() {
        let sentences = vec![Sentence {
            sent_id: Some("book-0001".into()),
            newdoc_id: Some("book".into()),
            text: Some("The cat sat.".into()),
            tokens: vec![
                Token::new("The"),
                Token::new("cat"),
                Token::new("sat"),
                Token::new("."),
            ],
            newdoc_title: None,
        }];

        let out = write(&sentences);
        assert!(out.contains("# newdoc id = book"));
        assert!(out.contains("# sent_id = book-0001"));
        assert!(out.contains("# text = The cat sat."));
        assert!(out.contains("1\tThe\t_\t_\t_\t_\t_\t_\t_\t_"));

        let back = parse_reader(out.as_bytes(), "<test>").unwrap();
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].tokens.len(), 4);
        assert_eq!(back[0].tokens[1].form, "cat");
        assert_eq!(back[0].text.as_deref(), Some("The cat sat."));
    }

    #[test]
    fn round_trips_the_document_title() {
        let sentences = vec![Sentence {
            sent_id: Some("book-1-0001".into()),
            newdoc_id: Some("book-1".into()),
            newdoc_title: Some("A Long-Expected Party".into()),
            text: Some("Hi.".into()),
            tokens: vec![Token::new("Hi"), Token::new(".")],
        }];

        let out = write(&sentences);
        assert!(out.contains("# newdoc title = A Long-Expected Party"));

        let back = parse_reader(out.as_bytes(), "<test>").unwrap();
        assert_eq!(
            back[0].newdoc_title.as_deref(),
            Some("A Long-Expected Party")
        );
    }

    #[test]
    fn round_trips_lemma_overrides_in_misc() {
        let sentences = vec![Sentence {
            text: Some("he spake.".into()),
            tokens: vec![
                Token {
                    form: "spake".into(),
                    lemma: Some("spake".into()),
                    lemma_override: Some("speak".into()),
                    upos_override: Some("VERB".into()),
                    ..Token::default()
                },
                Token::new("."),
            ],
            ..Sentence::default()
        }];

        let out = write(&sentences);
        assert!(
            out.contains("LemmaOverride=speak"),
            "override missing from MISC:\n{out}"
        );
        assert!(out.contains("UposOverride=VERB"));

        let back = parse_reader(out.as_bytes(), "<test>").unwrap();
        let token = &back[0].tokens[0];
        // Both layers survive the round trip.
        assert_eq!(token.lemma.as_deref(), Some("spake"));
        assert_eq!(token.lemma_override.as_deref(), Some("speak"));
        assert_eq!(token.upos_override.as_deref(), Some("VERB"));
        // A token with no override reads back clean.
        assert_eq!(back[0].tokens[1].lemma_override, None);
    }

    #[test]
    fn newdoc_is_written_once_per_run() {
        let mk = |doc: &str| Sentence {
            newdoc_id: Some(doc.into()),
            tokens: vec![Token::new("x")],
            ..Sentence::default()
        };
        let out = write(&[mk("a"), mk("a"), mk("b")]);
        assert_eq!(out.matches("# newdoc id = a").count(), 1);
        assert_eq!(out.matches("# newdoc id = b").count(), 1);
    }
}
