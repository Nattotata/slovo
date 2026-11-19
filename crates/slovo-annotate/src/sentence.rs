//! Sentence splitting.
//!
//! Turns a block of text into sentences. Paragraph breaks are hard boundaries;
//! inside a paragraph, a sentence ends at `.`, `!` or `?` (possibly repeated,
//! as in `?!`) followed by whitespace and something that looks like a new
//! sentence.
//!
//! It is deliberately conservative about the classic traps:
//!
//! * common abbreviations (`Mr.`, `Dr.`, `St.`, `etc.`) do not end a sentence;
//! * a decimal number (`3.5`) does not end a sentence;
//! * a closing quote or bracket after the stop stays with the sentence
//!   (`… he said."`).
//!
//! It is not perfect on literary prose — ellipses and nested quotes in
//! dialogue are genuinely hard — but it is stable and predictable.

use crate::Sentence;

/// Abbreviations whose trailing dot is not a sentence end. Lower-cased,
/// without the dot.
const ABBREVIATIONS: &[&str] = &[
    "mr", "mrs", "ms", "dr", "prof", "st", "jr", "sr", "vs", "etc", "eg", "ie", "no", "fig", "vol",
    "ch", "pp", "ed", "ca", "cf", "al", "inc", "ltd", "co", "hon", "gen", "col", "capt", "lt",
    "sgt", "rev", "fr", "mt", "sq",
];

/// Split text into sentences. Paragraph breaks force a boundary.
pub fn split(text: &str) -> Vec<Sentence> {
    let mut sentences = Vec::new();

    for paragraph in text.split("\n\n") {
        let paragraph = paragraph.trim();
        if paragraph.is_empty() {
            continue;
        }
        // A single newline inside a paragraph is just wrapping; treat it as a
        // space so the sentence text reads naturally.
        let paragraph = paragraph.replace('\n', " ");
        for sentence_text in split_paragraph(&paragraph) {
            let sentence_text = sentence_text.trim();
            if sentence_text.is_empty() {
                continue;
            }
            sentences.push(Sentence {
                text: Some(sentence_text.to_string()),
                ..Sentence::default()
            });
        }
    }

    sentences
}

fn split_paragraph(paragraph: &str) -> Vec<String> {
    let chars: Vec<char> = paragraph.chars().collect();
    let mut sentences = Vec::new();
    let mut start = 0usize;
    let mut i = 0usize;

    while i < chars.len() {
        if is_terminator(chars[i]) {
            // Consume a run of terminators (`?!`, `...`).
            let mut j = i;
            while j < chars.len() && is_terminator(chars[j]) {
                j += 1;
            }
            // Pull any closing quotes/brackets into the sentence.
            let mut k = j;
            while k < chars.len() && is_closer(chars[k]) {
                k += 1;
            }

            if ends_sentence(&chars, i, k) {
                let sentence: String = chars[start..k].iter().collect();
                sentences.push(sentence);
                // Skip the following whitespace.
                while k < chars.len() && chars[k].is_whitespace() {
                    k += 1;
                }
                start = k;
                i = k;
                continue;
            }
            i = j;
            continue;
        }
        i += 1;
    }

    if start < chars.len() {
        let tail: String = chars[start..].iter().collect();
        if !tail.trim().is_empty() {
            sentences.push(tail);
        }
    }

    sentences
}

/// Does the terminator run ending at `end` (inclusive of closers) close a
/// sentence, given the text before `stop`?
fn ends_sentence(chars: &[char], stop: usize, after: usize) -> bool {
    // Nothing after the stop, or only whitespace: it ends the sentence.
    let rest = &chars[after..];
    if rest.iter().all(|c| c.is_whitespace()) {
        return true;
    }
    // Must be followed by whitespace, otherwise it is inside a token
    // (`3.5`, `www.example.com`).
    if !chars[after].is_whitespace() {
        return false;
    }

    // If the terminator is a lone `.`, check for an abbreviation or a number.
    if chars[stop] == '.' {
        if is_abbreviation(chars, stop) {
            return false;
        }
        if stop > 0 && chars[stop - 1].is_ascii_digit() {
            return false;
        }
    }

    true
}

/// Is the `.` at `dot` the end of a known abbreviation?
fn is_abbreviation(chars: &[char], dot: usize) -> bool {
    let mut word = String::new();
    let mut i = dot;
    while i > 0 {
        let c = chars[i - 1];
        if c.is_alphanumeric() {
            word.push(c.to_ascii_lowercase());
            i -= 1;
        } else {
            break;
        }
    }
    let word: String = word.chars().rev().collect();
    if word.is_empty() {
        return false;
    }
    ABBREVIATIONS.contains(&word.as_str())
}

fn is_terminator(c: char) -> bool {
    matches!(c, '.' | '!' | '?')
}

fn is_closer(c: char) -> bool {
    matches!(c, '"' | '\'' | ')' | ']' | '}' | '\u{2019}' | '\u{201d}')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(input: &str) -> Vec<String> {
        split(input).into_iter().filter_map(|s| s.text).collect()
    }

    #[test]
    fn splits_on_basic_terminators() {
        assert_eq!(
            texts("The cat sat. The dog barked! Did it run?"),
            vec!["The cat sat.", "The dog barked!", "Did it run?"]
        );
    }

    #[test]
    fn keeps_abbreviations_intact() {
        assert_eq!(
            texts("Mr. Smith went to Washington."),
            vec!["Mr. Smith went to Washington."]
        );
        assert_eq!(
            texts("Bring rope, food, etc. and a knife."),
            vec!["Bring rope, food, etc. and a knife."]
        );
    }

    #[test]
    fn keeps_decimals_intact() {
        assert_eq!(
            texts("It cost 3.5 gold pieces."),
            vec!["It cost 3.5 gold pieces."]
        );
    }

    #[test]
    fn pulls_closing_quotes_into_the_sentence() {
        assert_eq!(
            texts(r#""Come here," he said. "Now.""#),
            vec![r#""Come here," he said."#, r#""Now.""#]
        );
    }

    #[test]
    fn paragraph_breaks_are_boundaries() {
        let input = "One line\nstill one sentence.\n\nA new paragraph.";
        assert_eq!(
            texts(input),
            vec!["One line still one sentence.", "A new paragraph."]
        );
    }

    #[test]
    fn handles_repeated_terminators() {
        assert_eq!(
            texts("What?! You cannot mean it... surely."),
            vec!["What?!", "You cannot mean it...", "surely."]
        );
    }
}
