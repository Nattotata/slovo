//! Pattern search over the token stream.
//!
//! The matcher walks each sentence and, for a pattern with a variable-length
//! gap, records **every** matching span length up to `within`. That is what
//! makes a cluster query useful: `take *` reports `take off` and
//! `take up with` as separate rows, not one row per maximal span.
//!
//! Each row can also carry a few **examples**: the concordance line a match
//! came from, so a phrase can be traced back to the sentence that produced it
//! (see [`crate::concordance`] and [`SearchOptions::examples`]).

use std::collections::HashMap;

use crate::concordance::{context_around, for_each_hit, Direction, Hit, KWIC_CONTEXT};
use crate::error::Result;
use crate::query::per_million;
use crate::query_lang;
use crate::store::Store;

/// How to run a search.
#[derive(Debug, Clone)]
pub struct SearchOptions {
    pub pattern: String,
    pub limit: usize,
    /// Match on the surface form rather than the lower-cased form.
    pub case_sensitive: bool,
    /// Cap on how many tokens a match may span.
    pub within: u32,
    /// Drop rows occurring fewer than this many times.
    pub min_freq: u64,
    /// Attach up to this many concordance examples to each row. `0` disables.
    pub examples: usize,
    /// Tokens of context on each side of an example match.
    pub context: usize,
}

impl SearchOptions {
    /// Options for `pattern`, with the CLI defaults.
    pub fn new(pattern: impl Into<String>) -> Self {
        Self {
            pattern: pattern.into(),
            limit: 20,
            case_sensitive: false,
            within: query_lang::DEFAULT_WITHIN,
            min_freq: 1,
            examples: 0,
            context: KWIC_CONTEXT,
        }
    }
}

/// One row of a search result.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchRow {
    pub text: String,
    pub freq: u64,
    pub per_million: f64,
    /// A few concordance hits for this span, capped at `examples`.
    pub examples: Vec<Hit>,
}

/// The outcome of a search.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchResult {
    pub pattern: String,
    /// The canonical CQP form the pattern expanded to.
    pub expanded: String,
    pub tokens: u64,
    /// Total match instances, counting each span length separately.
    pub matches: u64,
    /// Number of distinct matched spans.
    pub distinct: usize,
    pub rows: Vec<SearchRow>,
}

/// Run a search and rank the matched spans by frequency.
pub fn search(store: &Store, opts: &SearchOptions) -> Result<SearchResult> {
    let query = query_lang::parse(&opts.pattern)?;
    let within = query.within.unwrap_or(opts.within);
    let expanded = query_lang::canonical(&query, within);
    let total = store.token_count()?;

    let mut acc = Accumulator::new(opts.examples, opts.context);
    let matches = for_each_hit(
        store,
        &query.pattern,
        opts.case_sensitive,
        within,
        |sentence_id, toks, start, end| {
            let text = toks[start..end]
                .iter()
                .map(|t| t.lower.as_str())
                .collect::<Vec<_>>()
                .join(" ");
            let hit = Hit {
                sentence_id,
                start,
                end,
                left: context_around(toks, start, acc.context, Direction::Left),
                match_text: toks[start..end]
                    .iter()
                    .map(|t| t.form.clone())
                    .collect::<Vec<_>>()
                    .join(" "),
                right: context_around(toks, end, acc.context, Direction::Right),
            };
            acc.record(text, hit);
        },
    )?;

    let mut out: Vec<SearchRow> = acc
        .counts
        .into_iter()
        .filter(|(_, freq)| *freq >= opts.min_freq)
        .map(|(text, freq)| {
            let examples = acc.examples.remove(&text).unwrap_or_default();
            SearchRow {
                text,
                freq,
                per_million: per_million(freq, total),
                examples,
            }
        })
        .collect();

    out.sort_by(|a, b| b.freq.cmp(&a.freq).then_with(|| a.text.cmp(&b.text)));
    let distinct = out.len();
    out.truncate(opts.limit);

    Ok(SearchResult {
        pattern: opts.pattern.clone(),
        expanded,
        tokens: total,
        matches,
        distinct,
        rows: out,
    })
}

/// Counts matched spans, and keeps a capped sample of their locations.
struct Accumulator {
    counts: HashMap<String, u64>,
    /// Concordance hits per span text, in the order they were seen.
    examples: HashMap<String, Vec<Hit>>,
    /// How many examples to keep per span. `0` means none.
    per_span: usize,
    /// Tokens of context on each side of an example.
    context: usize,
}

impl Accumulator {
    fn new(per_span: usize, context: usize) -> Self {
        Self {
            counts: HashMap::new(),
            examples: HashMap::new(),
            per_span,
            context,
        }
    }

    fn record(&mut self, text: String, hit: Hit) {
        *self.counts.entry(text.clone()).or_insert(0) += 1;
        if self.per_span == 0 {
            return;
        }
        let bucket = self.examples.entry(text).or_default();
        if bucket.len() < self.per_span {
            bucket.push(hit);
        }
    }
}
