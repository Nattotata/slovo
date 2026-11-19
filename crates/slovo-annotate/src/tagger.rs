//! The tagger hook: lemma and POS from an external command.
//!
//! Slovo does not ship a tagger. Instead, this passes text to a command you
//! configure and reads **CoNLL-U** back — the same format the core imports, so
//! anything that speaks CoNLL-U works: UDPipe, Stanza, a script of your own.
//!
//! The contract is narrow:
//!
//! * the command is run once per annotate call, with the whole corpus on
//!   **stdin** (one sentence per line, blank line between);
//! * it must print CoNLL-U on **stdout**;
//! * the token *forms* it returns are matched back to our sentences in order —
//!   a tokenizer we disagree with is a mismatch, so prefer a tagger configured
//!   to keep our boundaries, or accept that the fallback fills the gaps.
//!
//! Example, with UDPipe's CLI:
//!
//! ```text
//! slovo annotate book.txt -o book.conllu \
//!   --tagger 'udpipe --tokenize --tag --lemma --outfile - model.udpipe'
//! ```

use std::io::Write;
use std::process::{Command, Stdio};

use crate::conllu;
use crate::error::{Error, Result};
use crate::Sentence;

/// An external tagger, given as a shell command line.
#[derive(Debug, Clone)]
pub struct Tagger {
    command: String,
}

impl Tagger {
    pub fn new(command: impl Into<String>) -> Self {
        Self {
            command: command.into(),
        }
    }

    pub fn command(&self) -> &str {
        &self.command
    }

    /// Feed every sentence to the command and merge the returned annotations
    /// back onto `sentences`, matching by position.
    pub fn apply(&self, sentences: &mut [Sentence]) -> Result<()> {
        if sentences.is_empty() {
            return Ok(());
        }

        // One sentence per line, blank line between, so the tagger can keep
        // sentence boundaries even if it does not split them itself.
        let mut input = String::new();
        for sentence in sentences.iter() {
            input.push_str(sentence.text.as_deref().unwrap_or(""));
            input.push_str("\n\n");
        }

        let output = self.run(&input)?;
        let annotated = conllu::parse_reader(output.as_bytes(), self.command())?;

        merge(sentences, &annotated);
        Ok(())
    }

    fn run(&self, input: &str) -> Result<String> {
        // Run through the shell so users can pass arguments and pipes.
        let mut child = Command::new("sh")
            .arg("-c")
            .arg(&self.command)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|source| Error::Tagger {
                command: self.command.clone(),
                message: source.to_string(),
            })?;

        child
            .stdin
            .take()
            .expect("stdin was piped")
            .write_all(input.as_bytes())
            .map_err(|source| Error::Tagger {
                command: self.command.clone(),
                message: source.to_string(),
            })?;

        let output = child.wait_with_output().map_err(|source| Error::Tagger {
            command: self.command.clone(),
            message: source.to_string(),
        })?;

        if !output.status.success() {
            return Err(Error::Tagger {
                command: self.command.clone(),
                message: format!(
                    "exited with {}: {}",
                    output.status,
                    String::from_utf8_lossy(&output.stderr).trim()
                ),
            });
        }

        String::from_utf8(output.stdout).map_err(|source| Error::Tagger {
            command: self.command.clone(),
            message: format!("output was not valid UTF-8: {source}"),
        })
    }
}

/// Merge tagger output onto our sentences, by sentence order and token order.
///
/// Our tokenization is authoritative: only the columns we did not produce
/// (lemma, upos, xpos, feats, head, deprel) are taken from the tagger. A
/// sentence or token count mismatch is tolerated — we fill what lines up and
/// leave the rest as `_`.
fn merge(sentences: &mut [Sentence], annotated: &[Sentence]) {
    for (sentence, tagged) in sentences.iter_mut().zip(annotated) {
        for (token, source) in sentence.tokens.iter_mut().zip(&tagged.tokens) {
            token.lemma = source.lemma.clone();
            token.upos = source.upos.clone();
            token.xpos = source.xpos.clone();
            token.feats = source.feats.clone();
            token.head = source.head;
            token.deprel = source.deprel.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Token;

    fn sentence(text: &str, forms: &[&str]) -> Sentence {
        Sentence {
            text: Some(text.to_string()),
            tokens: forms.iter().map(|f| Token::new(*f)).collect(),
            ..Sentence::default()
        }
    }

    #[test]
    fn merges_lemma_and_pos_from_conllu() {
        // A tagger that echoes fixed CoNLL-U, so we test the merge, not a model.
        let command = "printf '# text = The cat sat.\\n1\\tThe\\tthe\\tDET\\tDT\\tDefinite=Def\\t2\\tdet\\t_\\t_\\n\
                       2\\tcat\\tcat\\tNOUN\\tNN\\tNumber=Sing\\t3\\tnsubj\\t_\\t_\\n\
                       3\\tsat\\tsit\\tVERB\\tVBD\\tTense=Past\\t0\\troot\\t_\\t_\\n\\n'";
        let tagger = Tagger::new(command);

        let mut sentences = vec![sentence("The cat sat.", &["The", "cat", "sat"])];
        tagger.apply(&mut sentences).unwrap();

        let tokens = &sentences[0].tokens;
        assert_eq!(tokens[1].lemma.as_deref(), Some("cat"));
        assert_eq!(tokens[1].upos.as_deref(), Some("NOUN"));
        assert_eq!(tokens[2].lemma.as_deref(), Some("sit"));
        assert_eq!(tokens[2].upos.as_deref(), Some("VERB"));
    }

    #[test]
    fn a_failing_command_is_reported() {
        let tagger = Tagger::new("exit 3");
        let mut sentences = vec![sentence("Hi.", &["Hi", "."])];
        let error = tagger.apply(&mut sentences).unwrap_err().to_string();
        assert!(
            error.contains("exit 3") || error.contains("exited"),
            "{error}"
        );
    }
}
