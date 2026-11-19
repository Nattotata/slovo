//! A minimal CoNLL-U reader.
//!
//! CoNLL-U is the standard exchange format for Universal Dependencies
//! treebanks: tab-separated columns, one token per line, sentences separated
//! by blank lines, comments prefixed with `#`.
//!
//! Columns: ID, FORM, LEMMA, UPOS, XPOS, FEATS, HEAD, DEPREL, DEPS, MISC.
//! We keep the columns needed for corpus work and ignore the rest. Multiword
//! tokens (`1-2`) and empty nodes (`5.1`) are skipped.

use std::io::BufRead;
use std::path::Path;

use crate::error::{Error, Result};

/// One token, as far as Slovo cares about it.
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub form: String,
    pub lemma: Option<String>,
    pub upos: Option<String>,
    pub xpos: Option<String>,
    pub feats: Option<String>,
    pub head: Option<i64>,
    pub deprel: Option<String>,
    /// A non-destructive lemma correction, from MISC `LemmaOverride=…`.
    /// Kept apart from `lemma` so the tagger's output stays inspectable.
    pub lemma_override: Option<String>,
    /// A non-destructive UPOS correction, from MISC `UposOverride=…`.
    pub upos_override: Option<String>,
    /// Structural ranking weight, from MISC `Weight=…`. `1.0` when absent.
    pub weight: f64,
}

impl Default for Token {
    fn default() -> Self {
        Self {
            form: String::new(),
            lemma: None,
            upos: None,
            xpos: None,
            feats: None,
            head: None,
            deprel: None,
            lemma_override: None,
            upos_override: None,
            weight: 1.0,
        }
    }
}

/// One sentence and its tokens, in order.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Sentence {
    pub text: Option<String>,
    /// The `# sent_id = ...` value, when the file has one.
    pub sent_id: Option<String>,
    /// The enclosing `# newdoc id = ...` value, when the file has one.
    ///
    /// In UD this marks a document boundary *inside* the file; for EWT it is
    /// the original web-document id. Slovo uses it for provenance.
    pub newdoc_id: Option<String>,
    /// A human-readable document title, from `# newdoc title = ...`.
    pub newdoc_title: Option<String>,
    pub tokens: Vec<Token>,
}

/// Parse a whole CoNLL-U file. `path` is only used for error messages.
pub fn parse_file(path: &Path) -> Result<Vec<Sentence>> {
    let file = std::fs::File::open(path).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })?;
    parse_reader(std::io::BufReader::new(file), path)
}

/// Parse CoNLL-U from any buffered reader.
pub fn parse_reader<R: BufRead>(reader: R, path: &Path) -> Result<Vec<Sentence>> {
    let mut sentences = Vec::new();
    let mut tokens: Vec<Token> = Vec::new();
    let mut text: Option<String> = None;
    let mut sent_id: Option<String> = None;
    let mut newdoc_id: Option<String> = None;
    let mut newdoc_title: Option<String> = None;

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

    for (i, line) in reader.lines().enumerate() {
        let line = line.map_err(|source| Error::Io {
            path: path.to_path_buf(),
            source,
        })?;
        let line = line.trim_end_matches('\r');
        let lineno = i + 1;

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
            if let Some(value) = rest.strip_prefix("text =") {
                text = Some(value.trim().to_string());
            } else if let Some(value) = rest.strip_prefix("sent_id =") {
                sent_id = Some(value.trim().to_string());
            } else if let Some(value) = rest.strip_prefix("newdoc id =") {
                // A new document starts here; close off the previous one.
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
                newdoc_id = Some(value.trim().to_string());
                newdoc_title = None;
            } else if let Some(value) = rest.strip_prefix("newdoc title =") {
                newdoc_title = Some(value.trim().to_string());
            }
            continue;
        }

        // CoNLL-U is tab-separated, but be forgiving about space-separated files.
        let mut cols: Vec<&str> = line.split('\t').collect();
        if cols.len() < 10 {
            cols = line.split_whitespace().collect();
        }
        if cols.len() < 10 {
            return Err(Error::Conllu {
                path: path.to_path_buf(),
                line: lineno,
                message: format!("expected 10 columns, found {}", cols.len()),
            });
        }

        let id = cols[0];
        if id.contains('-') || id.contains('.') {
            continue; // multiword token or empty node
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

/// CoNLL-U uses `_` for "no value".
fn opt(value: &str) -> Option<String> {
    match value {
        "_" | "" => None,
        other => Some(other.to_string()),
    }
}

/// Pull `LemmaOverride` / `UposOverride` / `Weight` out of a MISC column.
fn parse_misc(misc: &str) -> (Option<String>, Option<String>, f64) {
    let mut lemma = None;
    let mut upos = None;
    let mut weight = 1.0;
    for field in misc.split('|') {
        if let Some(v) = field.strip_prefix("LemmaOverride=") {
            lemma = Some(v.to_string());
        } else if let Some(v) = field.strip_prefix("UposOverride=") {
            upos = Some(v.to_string());
        } else if let Some(v) = field.strip_prefix("Weight=") {
            if let Ok(value) = v.parse::<f64>() {
                weight = value;
            }
        }
    }
    (lemma, upos, weight)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
# sent_id = 1
# text = The cat sat.
1\tThe\tthe\tDET\tDT\tDefinite=Def\t2\tdet\t_\t_
2\tcat\tcat\tNOUN\tNN\tNumber=Sing\t3\tnsubj\t_\t_
3\tsat\tsit\tVERB\tVBD\tTense=Past\t0\troot\t_\t_
4\t.\t.\tPUNCT\t.\t_\t3\tpunct\t_\t_

# sent_id = 2
# text = Dogs bark.
1\tDogs\tdog\tNOUN\tNNS\tNumber=Plur\t2\tnsubj\t_\t_
2\tbark\tbark\tVERB\tVBP\tTense=Pres\t0\troot\t_\t_
";

    #[test]
    fn parses_sentences_and_tokens() {
        let path = Path::new("<test>");
        let sentences = parse_reader(SAMPLE.as_bytes(), path).unwrap();
        assert_eq!(sentences.len(), 2);

        assert_eq!(sentences[0].text.as_deref(), Some("The cat sat."));
        assert_eq!(sentences[0].tokens.len(), 4);
        assert_eq!(sentences[0].tokens[0].form, "The");
        assert_eq!(sentences[0].tokens[0].lemma.as_deref(), Some("the"));
        assert_eq!(sentences[0].tokens[0].upos.as_deref(), Some("DET"));

        assert_eq!(sentences[1].tokens[0].lemma.as_deref(), Some("dog"));
        assert_eq!(sentences[1].tokens[0].head, Some(2));
    }

    #[test]
    fn skips_multiword_tokens_and_empty_nodes() {
        // Spanish "vámonos" as a multiword token, plus an empty node.
        let sample = "1-2\tvámonos\t_\t_\t_\t_\t_\t_\t_\t_\n\
                      1\tvamos\tir\tVERB\t_\t_\t0\troot\t_\t_\n\
                      2\tnos\tnosotros\tPRON\t_\t_\t1\tobj\t_\t_\n\
                      2.1\t_\t_\t_\t_\t_\t_\t_\t_\tSpaceAfter=No\n";
        let sentences = parse_reader(sample.as_bytes(), Path::new("<test>")).unwrap();
        assert_eq!(sentences.len(), 1);
        assert_eq!(sentences[0].tokens.len(), 2);
        assert_eq!(sentences[0].tokens[0].form, "vamos");
        assert_eq!(sentences[0].tokens[1].form, "nos");
    }

    #[test]
    fn reads_lemma_and_upos_overrides_from_misc() {
        let sample = "1\the\the\tPRON\tPRP\t_\t2\tnsubj\t_\t_\n\
                      2\tspake\tspake\tNOUN\tNN\t_\t0\troot\t_\tLemmaOverride=speak|UposOverride=VERB\n";
        let sentences = parse_reader(sample.as_bytes(), Path::new("<test>")).unwrap();
        let token = &sentences[0].tokens[1];
        // The tagger's guess is preserved...
        assert_eq!(token.lemma.as_deref(), Some("spake"));
        assert_eq!(token.upos.as_deref(), Some("NOUN"));
        // ...and the correction sits beside it.
        assert_eq!(token.lemma_override.as_deref(), Some("speak"));
        assert_eq!(token.upos_override.as_deref(), Some("VERB"));
        // A token with no override reads back clean.
        assert_eq!(sentences[0].tokens[0].lemma_override, None);
    }

    #[test]
    fn reads_weight_from_misc_and_defaults_to_one() {
        let sample = "1\tthe\tthe\tDET\tDT\t_\t_\t_\t_\t_\n\
                      2\tchorus\tchorus\tNOUN\tNN\t_\t_\t_\t_\tWeight=4\n";
        let sentences = parse_reader(sample.as_bytes(), Path::new("<test>")).unwrap();
        assert_eq!(sentences[0].tokens[0].weight, 1.0);
        assert_eq!(sentences[0].tokens[1].weight, 4.0);
    }

    #[test]
    fn reports_bad_column_count() {
        let sample = "1\tonly\ttwo\n";
        let err = parse_reader(sample.as_bytes(), Path::new("<test>")).unwrap_err();
        assert!(matches!(err, Error::Conllu { line: 1, .. }));
    }

    #[test]
    fn captures_sent_id_and_newdoc() {
        let sample = "\
# newdoc id = doc-a
# sent_id = doc-a-1
# text = First.
1\tFirst\tfirst\tADJ\tJJ\t_\t0\troot\t_\t_

# sent_id = doc-a-2
# text = Second.
1\tSecond\tsecond\tADJ\tJJ\t_\t0\troot\t_\t_

# newdoc id = doc-b
# sent_id = doc-b-1
# text = Third.
1\tThird\tthird\tADJ\tJJ\t_\t0\troot\t_\t_
";
        let sentences = parse_reader(sample.as_bytes(), Path::new("<test>")).unwrap();
        assert_eq!(sentences.len(), 3);
        assert_eq!(sentences[0].sent_id.as_deref(), Some("doc-a-1"));
        assert_eq!(sentences[0].newdoc_id.as_deref(), Some("doc-a"));
        assert_eq!(sentences[1].newdoc_id.as_deref(), Some("doc-a"));
        assert_eq!(sentences[2].newdoc_id.as_deref(), Some("doc-b"));
    }
}
