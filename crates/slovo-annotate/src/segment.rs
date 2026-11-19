//! Segmentation: a file into documents.
//!
//! A book is not one document; it is chapters, and each chapter should be its
//! own `# newdoc id` so a concordance hit can point at it (see `DESIGN.md` §9).
//! This module splits a text file at chapter headings.
//!
//! Detection is a heuristic, because plain text has no structure: a line that
//! is short, standalone (surrounded by blank lines) and looks like a heading
//! starts a new segment. A file with no such line becomes a single segment.
//!
//! Callers who know the structure can bypass detection by supplying their own
//! [`Segment`]s.

use crate::Segment;

/// How to split a file into segments.
#[derive(Debug, Clone)]
pub struct SplitOptions {
    /// Document id prefix, e.g. `book` gives `book-1`, `book-2`.
    pub id_prefix: String,
    /// Maximum words in a line for it to count as a heading.
    pub max_heading_words: usize,
    /// Maximum characters in a line for it to count as a heading.
    pub max_heading_chars: usize,
}

impl SplitOptions {
    pub fn new(id_prefix: impl Into<String>) -> Self {
        Self {
            id_prefix: id_prefix.into(),
            max_heading_words: 10,
            max_heading_chars: 80,
        }
    }
}

/// Split raw text into segments at chapter headings.
pub fn split(text: &str, options: &SplitOptions) -> Vec<Segment> {
    let blocks = blocks(text);
    let mut segments: Vec<Segment> = Vec::new();
    let mut current_title: Option<String> = None;
    let mut current_text = String::new();
    let mut counter = 0usize;

    let flush = |segments: &mut Vec<Segment>,
                 title: &Option<String>,
                 body: &mut String,
                 counter: &mut usize| {
        if body.trim().is_empty() {
            body.clear();
            return;
        }
        *counter += 1;
        segments.push(Segment {
            id: format!("{}-{}", options.id_prefix, *counter),
            title: title.clone(),
            document_id: None,
            document_title: None,
            text: std::mem::take(body),
            weight: None,
        });
    };

    for block in blocks {
        if is_heading(&block, options) {
            flush(
                &mut segments,
                &current_title,
                &mut current_text,
                &mut counter,
            );
            current_title = Some(block.trim().to_string());
        } else {
            if !current_text.is_empty() {
                current_text.push_str("\n\n");
            }
            current_text.push_str(&block);
        }
    }
    flush(
        &mut segments,
        &current_title,
        &mut current_text,
        &mut counter,
    );

    // A file with no headings is one segment.
    if segments.is_empty() {
        let trimmed = text.trim();
        if !trimmed.is_empty() {
            segments.push(Segment {
                id: format!("{}-1", options.id_prefix),
                title: None,
                document_id: None,
                document_title: None,
                text: trimmed.to_string(),
                weight: None,
            });
        }
    }

    segments
}

/// Split into blank-line-separated blocks, keeping their text.
fn blocks(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    for line in text.lines() {
        if line.trim().is_empty() {
            if !current.trim().is_empty() {
                out.push(std::mem::take(&mut current));
            } else {
                current.clear();
            }
        } else {
            if !current.is_empty() {
                current.push('\n');
            }
            current.push_str(line);
        }
    }
    if !current.trim().is_empty() {
        out.push(current);
    }
    out
}

/// A heading is a short, single-line, non-sentence-like block.
fn is_heading(block: &str, options: &SplitOptions) -> bool {
    let trimmed = block.trim();
    if trimmed.is_empty() || trimmed.chars().count() > options.max_heading_chars {
        return false;
    }
    // Multi-line blocks are paragraphs, not headings.
    if trimmed.lines().count() != 1 {
        return false;
    }
    if trimmed.split_whitespace().count() > options.max_heading_words {
        return false;
    }
    // A heading does not end in sentence punctuation.
    if trimmed.ends_with(['.', '!', '?', ',', ';', ':']) {
        return false;
    }
    // It should look like words, not a stray symbol.
    trimmed.chars().any(|c| c.is_alphabetic())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_at_headings() {
        let text = "\
THE FELLOWSHIP

In a hole in the ground there lived a hobbit.

THE RING

Bilbo was very rich and very peculiar.";
        let segments = split(text, &SplitOptions::new("book"));
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].title.as_deref(), Some("THE FELLOWSHIP"));
        assert_eq!(segments[1].title.as_deref(), Some("THE RING"));
        assert!(segments[0].text.contains("a hobbit"));
        assert_eq!(segments[0].id, "book-1");
    }

    #[test]
    fn a_file_without_headings_is_one_segment() {
        let text = "Just a paragraph of prose. And another sentence here.";
        let segments = split(text, &SplitOptions::new("book"));
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].title, None);
    }

    #[test]
    fn sentences_are_not_headings() {
        let text = "The cat sat on the mat.";
        let segments = split(text, &SplitOptions::new("book"));
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].title, None);
    }
}
