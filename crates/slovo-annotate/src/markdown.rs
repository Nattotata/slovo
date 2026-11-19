//! Markdown front-end.
//!
//! Parses markdown into [`Segment`]s and a per-token [`Weight`], so a corpus
//! built from prose (or a songbook) can rank by document structure.
//!
//! It handles the constructs this project's sources actually use, not the whole
//! of CommonMark:
//!
//! * **YAML frontmatter** (`---` … `---`) is read for `title`, and dropped from
//!   the text.
//! * **Headings** (`#`…`######`) start a section; the heading text is the
//!   segment's title, and its level sets a weight boost.
//! * **Fenced code blocks** (` ``` `, ` ```chords `) are excluded from the
//!   searchable text entirely — chord charts and snippets are not lyrics.
//! * **Inline chord markers** (`::G::`, `::A#::`) are stripped. They are often
//!   spliced *into* a word (`mag::B::gots` → `maggots`), so removal rejoins the
//!   fragments rather than splitting the word.
//! * **Emphasis markers** (`**Chords**`, `*Em*`) lose their delimiters, keeping
//!   the text between them.
//!
//! Everything else is treated as paragraph text.

use crate::{Segment, Weight};

/// What a markdown document parsed into.
#[derive(Debug, Clone, PartialEq)]
pub struct Markdown {
    /// Frontmatter title, when the file has one.
    pub title: Option<String>,
    /// One segment per heading section. Text before the first heading, if any,
    /// becomes a leading segment with no title.
    ///
    /// Each segment carries its structural weight (the heading level's boost);
    /// the heading text itself becomes the segment title and is *not* part of
    /// the segment's searchable text.
    pub segments: Vec<Segment>,
    /// Frontmatter `type`, when present (e.g. `song`).
    pub doc_type: Option<String>,
}

/// Parse markdown into segments.
///
/// `id_prefix` names the document (`<prefix>`, one per file) and its generated
/// segment ids. Every heading section becomes a segment of that one document.
pub fn parse(text: &str, id_prefix: &str, scheme: &Scheme) -> Markdown {
    let (frontmatter, body) = split_frontmatter(text);
    let title = frontmatter.get("title").cloned();
    let doc_type = frontmatter.get("type").cloned();

    let mut segments: Vec<Segment> = Vec::new();

    let mut current_title: Option<String> = None;
    let mut current_level: u8 = 0;
    let mut current_text = String::new();
    let mut section = 0usize;

    let flush = |segments: &mut Vec<Segment>,
                 title: &Option<String>,
                 level: u8,
                 text: &mut String,
                 section: &mut usize| {
        if text.trim().is_empty() {
            text.clear();
            return;
        }
        *section += 1;
        let weight = scheme.weight_for_heading(level);
        segments.push(Segment {
            id: format!("{id_prefix}-s{section}"),
            title: title.clone(),
            // All sections belong to the one document named by the file; the
            // document title is filled in after parsing.
            document_id: Some(id_prefix.to_string()),
            document_title: None,
            text: std::mem::take(text),
            weight: Some(weight),
        });
    };

    let mut in_code = false;
    let mut code_fence = String::new();

    for line in body.lines() {
        // Fenced code: the whole block is dropped.
        if let Some(fence) = fence_open(line) {
            if in_code {
                // Closing fence: only if it matches the opening length.
                if line.trim_start().starts_with(&code_fence) {
                    in_code = false;
                }
            } else {
                in_code = true;
                code_fence = fence;
            }
            continue;
        }
        if in_code {
            continue;
        }

        // Headings start a new segment.
        if let Some((level, text)) = heading(line) {
            flush(
                &mut segments,
                &current_title,
                current_level,
                &mut current_text,
                &mut section,
            );
            current_title = Some(text);
            current_level = level;
            continue;
        }

        // A blank line ends a paragraph but not the section.
        if line.trim().is_empty() {
            if !current_text.is_empty() {
                current_text.push_str("\n\n");
            }
            continue;
        }

        // Notation lines (chord brackets, and the like) are not text.
        if scheme.is_notation(line) {
            continue;
        }

        let cleaned = strip_inline_markup(line);
        if !current_text.is_empty() && !current_text.ends_with("\n\n") {
            current_text.push('\n');
        }
        current_text.push_str(&cleaned);
    }

    flush(
        &mut segments,
        &current_title,
        current_level,
        &mut current_text,
        &mut section,
    );

    // The document title is the frontmatter title, shared by every section.
    // `Segment.title` stays the section heading.
    if title.is_some() {
        for segment in &mut segments {
            segment.document_title = title.clone();
        }
    }

    Markdown {
        title,
        segments,
        doc_type,
    }
}

/// Heading level → weight boost, as data.
#[derive(Debug, Clone, PartialEq)]
pub struct Scheme {
    /// Weight for a token in an `#` heading, then `##`, then `###`, etc.
    /// Index 0 is `#`, index 5 is `######`.
    pub headings: [Weight; 6],
    /// Weight for ordinary paragraph text.
    pub body: Weight,
    /// Literal markers that, when they wrap a line, mean the line is notation
    /// rather than text. A line consisting only of `<open>…<close>` is dropped.
    ///
    /// The default handles this project's songbook convention, where a chord
    /// line is bracketed: `**Chords** D **Chords**`. It is data, so another
    /// corpus can change it without a code change.
    pub drop_wrappers: Vec<(String, String)>,
}

impl Default for Scheme {
    fn default() -> Self {
        Self {
            headings: [
                Weight::new(6.0),
                Weight::new(5.0),
                Weight::new(4.0),
                Weight::new(3.0),
                Weight::new(2.0),
                Weight::new(2.0),
            ],
            body: Weight::new(1.0),
            drop_wrappers: vec![("**Chords**".to_string(), "**Chords**".to_string())],
        }
    }
}

impl Scheme {
    /// The weight for text at a given heading level (`0` = body text).
    fn weight_for_heading(&self, level: u8) -> Weight {
        if level == 0 {
            return self.body;
        }
        let index = (level as usize).saturating_sub(1).min(5);
        self.headings[index]
    }

    /// Is this line wrapped in a notation marker, and therefore not text?
    fn is_notation(&self, line: &str) -> bool {
        let trimmed = line.trim();
        self.drop_wrappers.iter().any(|(open, close)| {
            trimmed.starts_with(open.as_str())
                && trimmed.ends_with(close.as_str())
                && trimmed.len() >= open.len() + close.len()
        })
    }
}

/// Split `---\n…\n---` frontmatter from the body, returning simple key → value
/// pairs. Only top-level `key: value` lines are read; nested maps are ignored.
fn split_frontmatter(text: &str) -> (std::collections::HashMap<String, String>, &str) {
    let mut map = std::collections::HashMap::new();
    let trimmed = text.strip_prefix('\u{feff}').unwrap_or(text);

    let mut lines = trimmed.lines();
    if lines.next().map(str::trim) != Some("---") {
        return (map, text);
    }

    let mut offset = 4; // past the opening "---\n"
    for line in &mut lines {
        offset += line.len() + 1;
        if line.trim() == "---" {
            let body = trimmed.get(offset..).unwrap_or("");
            return (map, body);
        }
        // Top-level keys only: no leading whitespace.
        if line.starts_with(char::is_whitespace) {
            continue;
        }
        if let Some((key, value)) = line.split_once(':') {
            let key = key.trim();
            if key.is_empty() {
                continue;
            }
            let value = value.trim().trim_matches(['"', '\''].as_ref());
            if !value.is_empty() {
                map.insert(key.to_string(), value.to_string());
            }
        }
    }

    // No closing fence: treat the whole thing as body.
    (map, text)
}

/// Is this line a fence opening? Returns the fence string (` ``` ` or `~~~`).
fn fence_open(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    if let Some(rest) = trimmed.strip_prefix("```") {
        // Keep the exact run of backticks so a longer block is matched.
        let extra: String = rest.chars().take_while(|c| *c == '`').collect();
        return Some(format!("```{extra}"));
    }
    if trimmed.starts_with("~~~") {
        return Some("~~~".to_string());
    }
    None
}

/// Parse a heading line into `(level, text)`.
fn heading(line: &str) -> Option<(u8, String)> {
    let trimmed = line.trim_start();
    let hashes = trimmed.chars().take_while(|c| *c == '#').count();
    if hashes == 0 || hashes > 6 {
        return None;
    }
    let rest = &trimmed[hashes..];
    // A heading needs a space (or be empty) after the hashes.
    if !rest.is_empty() && !rest.starts_with(' ') {
        return None;
    }
    let text = strip_inline_markup(rest.trim());
    Some((hashes as u8, text))
}

/// Remove inline chord markers and emphasis delimiters, rejoining fragments.
///
/// `mag::B::gots` → `maggots`, `**Chords** D **Chords**` → `Chords D Chords`.
pub fn strip_inline_markup(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;

    while let Some(start) = rest.find("::") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        match after.find("::") {
            Some(end) => {
                // Drop `::chord::`; the text on either side rejoins.
                rest = &after[end + 2..];
            }
            None => {
                // A lone `::` is literal text.
                out.push_str("::");
                rest = after;
            }
        }
    }
    out.push_str(rest);

    strip_emphasis(&out)
}

/// Drop `**` and `*` delimiters, keeping the text between them.
fn strip_emphasis(text: &str) -> String {
    text.replace("**", "").replace('*', "")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_inline_chords_rejoining_fragments() {
        assert_eq!(strip_inline_markup("::G::Hello"), "Hello");
        assert_eq!(strip_inline_markup("mag::B::gots"), "maggots");
        assert_eq!(strip_inline_markup("wai::A#::ting"), "waiting");
        assert_eq!(
            strip_inline_markup("::G::Up close in a ::Em::hair"),
            "Up close in a hair"
        );
        assert_eq!(strip_inline_markup("plain text"), "plain text");
    }

    #[test]
    fn strips_emphasis_keeping_text() {
        assert_eq!(
            strip_inline_markup("**Chords** D **Chords**"),
            "Chords D Chords"
        );
        assert_eq!(strip_inline_markup("*Em* here"), "Em here");
    }

    #[test]
    fn parses_frontmatter() {
        let text = "---\ntitle: \"Rhino\"\ntype: song\n---\n\nbody here\n";
        let md = parse(text, "rhino", &Scheme::default());
        assert_eq!(md.title.as_deref(), Some("Rhino"));
        assert_eq!(md.doc_type.as_deref(), Some("song"));
        assert_eq!(md.segments.len(), 1);
        assert_eq!(md.segments[0].text.trim(), "body here");
        // The one document is named by the file; the title comes from frontmatter.
        assert_eq!(md.segments[0].document(), "rhino");
        assert_eq!(md.segments[0].document_title(), Some("Rhino"));
    }

    #[test]
    fn headings_start_segments_with_weights() {
        let text = "\
## Verse

Up close in a hair

### Chorus

All you want is sleep";
        let md = parse(text, "song", &Scheme::default());
        assert_eq!(md.segments.len(), 2);
        assert_eq!(md.segments[0].title.as_deref(), Some("Verse"));
        assert_eq!(md.segments[1].title.as_deref(), Some("Chorus"));

        // `##` is weight 5, `###` is 4; both boost above plain body (1).
        assert_eq!(md.segments[0].weight, Some(Weight::new(5.0)));
        assert_eq!(md.segments[1].weight, Some(Weight::new(4.0)));
        assert!(md.segments.iter().all(|s| s.weight.unwrap().value() > 1.0));

        // Both sections are the same document.
        assert_eq!(md.segments[0].document(), md.segments[1].document());

        // The heading text is a title, not searchable body text.
        assert!(!md.segments[0].text.contains("Verse"));
        assert!(md.segments[0].text.contains("Up close in a hair"));
    }

    #[test]
    fn fenced_code_is_excluded() {
        let text = "\
```chords
G: 3xxx30
Am: 5xxx53
```

Real lyric line";
        let md = parse(text, "song", &Scheme::default());
        let joined: String = md.segments.iter().map(|s| s.text.clone()).collect();
        assert!(!joined.contains("3xxx30"), "chord chart leaked: {joined}");
        assert!(joined.contains("Real lyric line"));
    }

    #[test]
    fn body_text_gets_the_body_weight() {
        let text = "just a plain line";
        let md = parse(text, "song", &Scheme::default());
        assert_eq!(md.segments.len(), 1);
        assert_eq!(md.segments[0].weight, Some(Weight::new(1.0)));
    }

    #[test]
    fn notation_lines_are_dropped() {
        let text = "\
**Chords** D **Chords**

Silent upper hills consumed by snow";
        let md = parse(text, "song", &Scheme::default());
        let joined: String = md.segments.iter().map(|s| s.text.clone()).collect();
        assert!(!joined.contains("Chords"), "chord line leaked: {joined}");
        assert!(joined.contains("Silent upper hills"));
    }
}
