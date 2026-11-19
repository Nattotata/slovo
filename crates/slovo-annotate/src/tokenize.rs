//! Tokenization.
//!
//! Splits a sentence into tokens the way UD expects (see `GLOSSARY.md`,
//! "Tokenization"): punctuation is its own token, contractions are torn apart
//! (`don't` → `do` + `n't`), and quoted material is separated.
//!
//! This is a rule-based tokenizer for whitespace-delimited languages with
//! English-style punctuation. It does not attempt to be universal; the point is
//! that the chain runs with nothing installed, and that a real tagger can
//! replace it wholesale later.

use crate::Token;

/// Suffixes that behave as their own token in English contractions.
const CONTRACTION_SUFFIXES: &[&str] = &["n't", "'s", "'re", "'ve", "'ll", "'d", "'m", "'em"];

/// Split a sentence into tokens.
pub fn tokenize(sentence: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    for raw in sentence.split_whitespace() {
        for piece in split_word(raw) {
            if !piece.is_empty() {
                tokens.push(Token::new(piece));
            }
        }
    }
    tokens
}

/// Split one whitespace-delimited chunk into one or more tokens.
fn split_word(word: &str) -> Vec<String> {
    // Leading punctuation (quotes, brackets) becomes its own token.
    let mut out = Vec::new();
    let chars: Vec<char> = word.chars().collect();
    let mut start = 0usize;

    // Leading openers.
    while start < chars.len() && is_opener(chars[start]) {
        out.push(chars[start].to_string());
        start += 1;
    }

    // Trailing closers and terminators.
    let mut end = chars.len();
    while end > start && (is_closer(chars[end - 1]) || is_terminator(chars[end - 1])) {
        end -= 1;
    }

    if start < end {
        let core: String = chars[start..end].iter().collect();
        out.extend(split_contraction(&core));
    }

    // Re-attach the trailing punctuation, each as its own token.
    for c in &chars[end..] {
        out.push(c.to_string());
    }

    out
}

/// Split an English contraction at its clitic boundary.
fn split_contraction(word: &str) -> Vec<String> {
    let lower = word.to_lowercase();

    // `n't`: "don't" -> "do" + "n't", "can't" -> "ca" + "n't".
    if let Some(idx) = lower.find("n't") {
        if idx > 0 {
            let stem: String = word.chars().take(idx).collect();
            let clitic: String = word.chars().skip(idx).collect();
            return vec![stem, clitic];
        }
    }

    // A trailing apostrophe-clitic: "'s", "'re", "'ve", "'ll", "'d", "'m".
    for suffix in CONTRACTION_SUFFIXES {
        if suffix == &"n't" {
            continue;
        }
        if lower.ends_with(suffix) && lower.len() > suffix.len() {
            let split = word.len() - suffix.len();
            let stem: String = word.chars().take(split).collect();
            let clitic: String = word.chars().skip(split).collect();
            if !stem.is_empty() {
                return vec![stem, clitic];
            }
        }
    }

    vec![word.to_string()]
}

fn is_opener(c: char) -> bool {
    matches!(c, '"' | '\u{201c}' | '(' | '[' | '{' | '\u{2018}')
}

fn is_closer(c: char) -> bool {
    matches!(c, '"' | '\u{201d}' | ')' | ']' | '}' | '\u{2019}')
}

fn is_terminator(c: char) -> bool {
    matches!(c, '.' | ',' | ';' | ':' | '!' | '?' | '\u{2026}')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn forms(sentence: &str) -> Vec<String> {
        tokenize(sentence).into_iter().map(|t| t.form).collect()
    }

    #[test]
    fn splits_plain_words() {
        assert_eq!(forms("the cat sat"), vec!["the", "cat", "sat"]);
    }

    #[test]
    fn separates_trailing_punctuation() {
        assert_eq!(forms("The cat sat."), vec!["The", "cat", "sat", "."]);
    }

    #[test]
    fn separates_commas_and_colons() {
        assert_eq!(
            forms("Well, yes: fine."),
            vec!["Well", ",", "yes", ":", "fine", "."]
        );
    }

    #[test]
    fn tears_apart_contractions() {
        assert_eq!(forms("don't"), vec!["do", "n't"]);
        assert_eq!(forms("can't"), vec!["ca", "n't"]);
        assert_eq!(forms("it's"), vec!["it", "'s"]);
        assert_eq!(forms("we'll"), vec!["we", "'ll"]);
        assert_eq!(forms("I'm"), vec!["I", "'m"]);
    }

    #[test]
    fn separates_quotes() {
        assert_eq!(
            forms(r#""Come here," he said."#),
            vec!["\"", "Come", "here", ",", "\"", "he", "said", "."]
        );
    }

    #[test]
    fn keeps_hyphenated_words_together() {
        // UD treats a hyphenated compound as one token by default.
        assert_eq!(forms("a well-known fact"), vec!["a", "well-known", "fact"]);
    }

    #[test]
    fn handles_archaic_forms() {
        assert_eq!(
            forms("thou hast wrought it"),
            vec!["thou", "hast", "wrought", "it"]
        );
    }
}
