//! Pattern search over the token stream.
//!
//! The matcher walks each sentence and, for a pattern with a variable-length
//! gap, records **every** matching span length up to `within`. That is what
//! makes a cluster query useful: `take *` reports `take off` and
//! `take up with` as separate rows, not one row per maximal span.

use std::collections::HashMap;

use crate::error::Result;
use crate::query::per_million;
use crate::query_lang::{self, Atom, Node, Pattern, Quantifier, HARD_MAX_SPAN};
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
}

/// One row of a search result.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchRow {
    pub text: String,
    pub freq: u64,
    pub per_million: f64,
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

#[derive(Debug, Clone)]
struct Tok {
    form: String,
    lower: String,
    lemma: Option<String>,
    upos: Option<String>,
    xpos: Option<String>,
}

/// Run a search and rank the matched spans by frequency.
pub fn search(store: &Store, opts: &SearchOptions) -> Result<SearchResult> {
    let query = query_lang::parse(&opts.pattern)?;
    let within = query.within.unwrap_or(opts.within);
    let expanded = query_lang::canonical(&query, within);
    let total = store.token_count()?;
    let span_cap = (within as usize).clamp(1, HARD_MAX_SPAN);

    let mut counts: HashMap<String, u64> = HashMap::new();
    let mut matches = 0u64;

    let mut stmt = store.conn().prepare(
        "SELECT sentence_id, form, lower, lemma, upos, xpos
         FROM token
         ORDER BY sentence_id, position",
    )?;
    let mut rows = stmt.query([])?;

    let mut toks: Vec<Tok> = Vec::new();
    let mut current: Option<i64> = None;

    while let Some(row) = rows.next()? {
        let sentence_id: i64 = row.get(0)?;
        if current != Some(sentence_id) {
            if !toks.is_empty() {
                scan(
                    &toks,
                    &query.pattern,
                    opts.case_sensitive,
                    span_cap,
                    &mut counts,
                    &mut matches,
                );
                toks.clear();
            }
            current = Some(sentence_id);
        }
        toks.push(Tok {
            form: row.get(1)?,
            lower: row.get(2)?,
            lemma: row.get(3)?,
            upos: row.get(4)?,
            xpos: row.get(5)?,
        });
    }
    if !toks.is_empty() {
        scan(
            &toks,
            &query.pattern,
            opts.case_sensitive,
            span_cap,
            &mut counts,
            &mut matches,
        );
    }

    let mut out: Vec<SearchRow> = counts
        .into_iter()
        .filter(|(_, freq)| *freq >= opts.min_freq)
        .map(|(text, freq)| SearchRow {
            text,
            freq,
            per_million: per_million(freq, total),
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

fn scan(
    toks: &[Tok],
    pattern: &Pattern,
    case_sensitive: bool,
    span_cap: usize,
    counts: &mut HashMap<String, u64>,
    matches: &mut u64,
) {
    let matcher = Matcher {
        toks,
        case_sensitive,
    };
    for start in 0..toks.len() {
        let limit = (start + span_cap).min(toks.len());
        for end in matcher.pattern_ends(pattern, start, limit) {
            if end > start {
                let text = toks[start..end]
                    .iter()
                    .map(|t| t.lower.as_str())
                    .collect::<Vec<_>>()
                    .join(" ");
                *counts.entry(text).or_insert(0) += 1;
                *matches += 1;
            }
        }
    }
}

struct Matcher<'a> {
    toks: &'a [Tok],
    case_sensitive: bool,
}

fn eq_attr(actual: &str, wanted: &str, case_sensitive: bool) -> bool {
    if case_sensitive {
        actual == wanted
    } else {
        actual.to_lowercase() == wanted.to_lowercase()
    }
}

impl Matcher<'_> {
    /// All end positions reachable by matching `atom` at `pos`.
    fn atom_ends(&self, atom: &Atom, pos: usize, limit: usize) -> Vec<usize> {
        if pos >= limit || pos >= self.toks.len() {
            return Vec::new();
        }
        let tok = &self.toks[pos];

        let hit = match atom {
            Atom::Any => true,
            Atom::Word(word) => {
                if self.case_sensitive {
                    eq_attr(&tok.form, word, true)
                } else {
                    tok.lower == word.to_lowercase()
                }
            }
            Atom::Attr {
                name,
                negated,
                value,
            } => {
                let actual = match name.as_str() {
                    "word" | "form" => Some(tok.form.as_str()),
                    "lower" => Some(tok.lower.as_str()),
                    "lemma" => tok.lemma.as_deref(),
                    "upos" => tok.upos.as_deref(),
                    "xpos" => tok.xpos.as_deref(),
                    _ => None,
                };
                let matched = actual.is_some_and(|a| eq_attr(a, value, self.case_sensitive));
                matched != *negated
            }
            Atom::Group(inner) => return self.pattern_ends(inner, pos, limit),
        };

        if hit {
            vec![pos + 1]
        } else {
            Vec::new()
        }
    }

    fn node_ends(&self, node: &Node, pos: usize, limit: usize) -> Vec<usize> {
        match node.quantifier {
            Quantifier::One => self.atom_ends(&node.atom, pos, limit),
            Quantifier::Optional => {
                let mut ends = self.atom_ends(&node.atom, pos, limit);
                ends.push(pos);
                ends.sort_unstable();
                ends
            }
            Quantifier::OneOrMore => self.repeat(node, pos, 1, usize::MAX, limit),
            Quantifier::ZeroOrMore => self.repeat(node, pos, 0, usize::MAX, limit),
            Quantifier::Exactly(n) => self.repeat(node, pos, n as usize, n as usize, limit),
            Quantifier::Between(a, b) => self.repeat(node, pos, a as usize, b as usize, limit),
            Quantifier::AtLeast(n) => self.repeat(node, pos, n as usize, usize::MAX, limit),
            Quantifier::AtMost(m) => self.repeat(node, pos, 0, m as usize, limit),
        }
    }

    /// Repeated application of a node, collecting every reachable end between
    /// `min` and `max` repetitions.
    fn repeat(
        &self,
        node: &Node,
        start: usize,
        min: usize,
        max: usize,
        limit: usize,
    ) -> Vec<usize> {
        let mut result = Vec::new();
        if min == 0 {
            result.push(start);
        }

        let mut frontier = vec![start];
        let mut count = 0usize;
        while count < max && !frontier.is_empty() {
            count += 1;
            let mut next = Vec::new();
            for p in frontier {
                for end in self.atom_ends(&node.atom, p, limit) {
                    if end > p {
                        next.push(end);
                    }
                }
            }
            next.sort_unstable();
            next.dedup();
            if count >= min {
                result.extend(next.iter().copied());
            }
            frontier = next;
        }

        result.sort_unstable();
        result.dedup();
        result
    }

    fn pattern_ends(&self, pattern: &Pattern, pos: usize, limit: usize) -> Vec<usize> {
        match pattern {
            Pattern::Sequence(nodes) => {
                let mut current = vec![pos];
                for node in nodes {
                    let mut next = Vec::new();
                    for p in &current {
                        next.extend(self.node_ends(node, *p, limit));
                    }
                    next.sort_unstable();
                    next.dedup();
                    current = next;
                    if current.is_empty() {
                        break;
                    }
                }
                current
            }
            Pattern::Alternation(branches) => {
                let mut out = Vec::new();
                for branch in branches {
                    out.extend(self.pattern_ends(branch, pos, limit));
                }
                out.sort_unstable();
                out.dedup();
                out
            }
        }
    }
}
