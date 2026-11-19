//! The annotation engine: raw text to CoNLL-U.
//!
//! Slovo's core is a corpus *engine*: it imports CoNLL-U and never runs NLP.
//! This crate is the other side of that boundary — it turns raw `.txt` into the
//! CoNLL-U the core consumes, and it lives apart so the core stays free of
//! taggers, models and their weight.
//!
//! The pipeline (see [`annotate`]):
//!
//! ```text
//! .txt ─► segment (chapters) ─► split sentences ─► tokenize
//!      ─► [tagger: lemma + POS]  ─► write CoNLL-U
//! ```
//!
//! Segmentation, sentence splitting and tokenization are done here in plain
//! Rust, so the chain runs with nothing installed. Lemma and POS come from an
//! external command ([`tagger`]); with none configured, the fallback fills in
//! what it can and leaves the rest as `_`.

pub mod conllu;
pub mod error;
pub mod lexicon;
pub mod markdown;
pub mod segment;
pub mod sentence;
pub mod tagger;
pub mod tokenize;

pub use error::{Error, Result};

/// A token's ranking weight: how much an occurrence should count when ranking,
/// as opposed to how often it occurs.
///
/// It is *not* a frequency. `freq` counts occurrences; a weighted ranking sums
/// these. Kept as a small wrapper so the distinction is visible in the types,
/// and so equality is exact (weights come from a fixed scheme, not arithmetic).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Weight(f64);

/// `Weight` is `Eq` because values come from a fixed scheme and are compared
/// exactly, never accumulated. (Arithmetic on weights would forfeit this.)
impl Eq for Weight {}

impl Weight {
    pub const ONE: Weight = Weight(1.0);

    pub const fn new(value: f64) -> Self {
        Weight(value)
    }

    pub fn value(self) -> f64 {
        self.0
    }
}

impl Default for Weight {
    fn default() -> Self {
        Weight::ONE
    }
}

impl std::fmt::Display for Weight {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Trim a trailing `.0` so `1` prints as `1`, not `1.0`.
        if self.0.fract() == 0.0 {
            write!(f, "{}", self.0 as i64)
        } else {
            write!(f, "{}", self.0)
        }
    }
}

/// One annotated token, ready to be written as a CoNLL-U row.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Token {
    pub form: String,
    pub lemma: Option<String>,
    pub upos: Option<String>,
    pub xpos: Option<String>,
    pub feats: Option<String>,
    pub head: Option<i64>,
    pub deprel: Option<String>,
    /// A lexicon's lemma correction, kept apart from `lemma` so nothing the
    /// tagger produced is lost. Written to MISC as `LemmaOverride=…`.
    pub lemma_override: Option<String>,
    /// A lexicon's UPOS correction, likewise non-destructive.
    pub upos_override: Option<String>,
    /// Ranking weight from the source's structure (e.g. a markdown heading).
    /// Defaults to `Weight::ONE`; carried in MISC as `Weight=…`.
    pub weight: Weight,
}

impl Token {
    pub fn new(form: impl Into<String>) -> Self {
        Self {
            form: form.into(),
            ..Self::default()
        }
    }
}

/// One sentence: its text, its tokens, and where it came from.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Sentence {
    /// `# sent_id`, e.g. `book-ch1-0001`.
    pub sent_id: Option<String>,
    /// `# text`, the original sentence.
    pub text: Option<String>,
    /// The document this sentence belongs to; written as `# newdoc id` on the
    /// first sentence of a run. One per chapter or file.
    pub newdoc_id: Option<String>,
    /// Human-readable document title, written as `# newdoc title`.
    pub newdoc_title: Option<String>,
    pub tokens: Vec<Token>,
}

/// One input unit: a chapter, a whole file, or a markdown section.
///
/// A segment belongs to a *document*. For plain text each segment is its own
/// document (a chapter, a file); for markdown many segments — one per heading —
/// share one document (a song), and each carries its own weight.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Segment {
    /// The segment's own id; becomes the document id when `document_id` is set,
    /// otherwise it is a fallback.
    pub id: String,
    /// Human title, kept in a comment for provenance.
    pub title: Option<String>,
    /// The document this segment belongs to. `None` means "its own document"
    /// (id = `id`), which is what plain-text segments do.
    pub document_id: Option<String>,
    /// The document's title. Falls back to `title`.
    pub document_title: Option<String>,
    /// The segment's raw text.
    pub text: String,
    /// Structural weight for every token in this segment (e.g. a markdown
    /// heading's level). `None` means the default. One value per segment,
    /// because a section's body shares a single weight and a per-token vector
    /// would drift out of step with the tokenizer. See [`Weight`].
    pub weight: Option<Weight>,
}

impl Segment {
    /// A segment that is its own document (the plain-text case).
    pub fn standalone(
        id: impl Into<String>,
        title: Option<String>,
        text: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            title,
            document_id: None,
            document_title: None,
            text: text.into(),
            weight: None,
        }
    }

    /// The document id this segment resolves to.
    pub fn document(&self) -> &str {
        self.document_id.as_deref().unwrap_or(&self.id)
    }

    /// The document title this segment resolves to.
    pub fn document_title(&self) -> Option<&str> {
        self.document_title.as_deref().or(self.title.as_deref())
    }
}

/// How to annotate.
#[derive(Debug, Clone, Default)]
pub struct Options {
    /// An external tagger command, if any (see [`tagger::Tagger`]).
    pub tagger: Option<tagger::Tagger>,
    /// A lemma/POS correction table, applied after the tagger.
    pub lexicon: Option<lexicon::Lexicon>,
    /// Keep the parser's head/deprel columns. Off by default: concordance work
    /// does not need syntax, and dropping it lets a tokenizer-only run stand in.
    pub parse: bool,
}

impl Options {
    pub fn new() -> Self {
        Self::default()
    }
}

/// Annotate one segment's text, numbering its sentences from `start`.
///
/// The sentence counter is passed in so segments belonging to the same document
/// number continuously (`song-0001`, `song-0002`, … across verse and chorus),
/// rather than restarting per section.
pub fn annotate_segment(
    segment: &Segment,
    options: &Options,
    start: usize,
) -> Result<Vec<Sentence>> {
    let doc_id = segment.document();
    let doc_title = segment.document_title().map(str::to_string);

    let mut sentences = sentence::split(&segment.text);
    for (i, sentence) in sentences.iter_mut().enumerate() {
        sentence.newdoc_id = Some(doc_id.to_string());
        sentence.newdoc_title = doc_title.clone();
        sentence.sent_id = Some(format!("{doc_id}-{:04}", start + i + 1));
        sentence.tokens = tokenize::tokenize(sentence.text.as_deref().unwrap_or(""));
    }

    // A structural weight applies to the whole segment (a markdown heading's
    // level, say). Set before a tagger runs, so every token carries it.
    if let Some(weight) = segment.weight {
        for sentence in &mut sentences {
            for token in &mut sentence.tokens {
                token.weight = weight;
            }
        }
    }

    if let Some(tagger) = &options.tagger {
        tagger.apply(&mut sentences)?;
    }

    // The lexicon corrects the tagger; it never runs before it, and it never
    // overwrites what the tagger produced (see `lexicon`).
    if let Some(lexicon) = &options.lexicon {
        lexicon.apply(&mut sentences);
    }

    if !options.parse {
        for sentence in &mut sentences {
            for token in &mut sentence.tokens {
                token.head = None;
                token.deprel = None;
            }
        }
    }

    Ok(sentences)
}

/// Annotate every segment, in order.
///
/// Segments sharing a `document_id` form one document: their sentences number
/// continuously and the `# newdoc id` is written once.
pub fn annotate(segments: &[Segment], options: &Options) -> Result<Vec<Sentence>> {
    let mut out = Vec::new();
    let mut current: Option<String> = None;
    let mut counter = 0usize;

    for segment in segments {
        let doc = segment.document().to_string();
        if current.as_deref() != Some(doc.as_str()) {
            current = Some(doc);
            counter = 0;
        }
        let sentences = annotate_segment(segment, options, counter)?;
        counter += sentences.len();
        out.extend(sentences);
    }

    Ok(out)
}
