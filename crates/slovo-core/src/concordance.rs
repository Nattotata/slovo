//! Concordance / KWIC.
//!
//! A concordance is every occurrence of a pattern *in context*: the match
//! centred, with a few tokens on each side. It is what turns a cluster query
//! ("`get up` is followed by `with` 40 times") into something a human can read
//! ("here is the sentence, and here is what it meant").
//!
//! This module also owns the token-level matcher that [`crate::search`] uses,
//! because both need the same walk: read each sentence in order, run the
//! pattern against it, and report where it hit.

use crate::error::Result;
use crate::query_lang::{Atom, Node, Pattern, Quantifier, HARD_MAX_SPAN};
use crate::store::Store;

/// Tokens of context on each side of a match by default.
pub const KWIC_CONTEXT: usize = 5;

/// Which side of a match a slice of context comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Left,
    Right,
}

/// One occurrence of a pattern in the corpus.
///
/// Carries the sentence it came from and the token offsets inside that
/// sentence, so a result row can be traced back to its source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    pub sentence_id: i64,
    /// Token offset of the first matched token within the sentence (0-based).
    pub start: usize,
    /// Token offset one past the last matched token.
    pub end: usize,
    /// Tokens before the match, in reading order.
    pub left: String,
    /// The matched tokens, in their surface form.
    pub match_text: String,
    /// Tokens after the match, in reading order.
    pub right: String,
}

/// One line of a concordance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KwicLine {
    /// The document the hit belongs to.
    pub document_id: i64,
    pub document_title: Option<String>,
    /// Provenance: where the source text lives, when known.
    pub source_url: Option<String>,
    /// The document's external id (a path, or `<file>#<newdoc>`).
    pub document_external_id: Option<String>,
    pub sentence_id: i64,
    /// The sentence's `# sent_id`, when the source carried one.
    pub sent_id: Option<String>,
    /// The `# newdoc id` the sentence belongs to, when the source carried one.
    pub newdoc_id: Option<String>,
    pub left: String,
    pub match_text: String,
    pub right: String,
}

/// The outcome of a concordance query.
#[derive(Debug, Clone, PartialEq)]
pub struct KwicResult {
    pub pattern: String,
    /// The canonical CQP form the pattern expanded to.
    pub expanded: String,
    pub tokens: u64,
    /// Total hits, counting each span length separately (as in a cluster query).
    pub matches: u64,
    /// The concordance lines, capped at the requested limit.
    pub lines: Vec<KwicLine>,
}

/// How to run a concordance query.
#[derive(Debug, Clone)]
pub struct KwicOptions {
    pub pattern: String,
    pub limit: usize,
    pub case_sensitive: bool,
    pub within: u32,
    /// Tokens of context on each side of the match.
    pub context: usize,
}

impl KwicOptions {
    pub fn new(pattern: impl Into<String>) -> Self {
        Self {
            pattern: pattern.into(),
            limit: 50,
            case_sensitive: false,
            within: crate::query_lang::DEFAULT_WITHIN,
            context: KWIC_CONTEXT,
        }
    }
}

/// A token as the matcher sees it: the fields a query can constrain.
#[derive(Debug, Clone)]
pub(crate) struct Tok {
    pub form: String,
    pub lower: String,
    pub lemma: Option<String>,
    pub lemma_override: Option<String>,
    pub upos: Option<String>,
    pub upos_override: Option<String>,
    pub xpos: Option<String>,
}

/// Run a pattern over every sentence, calling `f` for each hit.
///
/// `f` receives the sentence id, the sentence's tokens, and the `[start, end)`
/// token range of the hit. Both the cluster search and the concordance walk
/// this same path.
pub(crate) fn for_each_hit<F>(
    store: &Store,
    pattern: &Pattern,
    case_sensitive: bool,
    within: u32,
    mut f: F,
) -> Result<u64>
where
    F: FnMut(i64, &[Tok], usize, usize),
{
    let span_cap = (within as usize).clamp(1, HARD_MAX_SPAN);
    let mut matches = 0u64;

    let mut stmt = store.conn().prepare(
        "SELECT sentence_id, form, lower, lemma, upos, xpos, lemma_override, upos_override
         FROM token
         ORDER BY sentence_id, position",
    )?;
    let mut rows = stmt.query([])?;

    let mut toks: Vec<Tok> = Vec::new();
    let mut current: Option<i64> = None;

    let mut flush = |sentence_id: i64, toks: &mut Vec<Tok>, matches: &mut u64| {
        if toks.is_empty() {
            return;
        }
        let matcher = Matcher {
            toks,
            case_sensitive,
        };
        for start in 0..toks.len() {
            let limit = (start + span_cap).min(toks.len());
            for end in matcher.pattern_ends(pattern, start, limit) {
                if end > start {
                    f(sentence_id, toks, start, end);
                    *matches += 1;
                }
            }
        }
        toks.clear();
    };

    while let Some(row) = rows.next()? {
        let sentence_id: i64 = row.get(0)?;
        if current != Some(sentence_id) {
            flush(current.unwrap_or(sentence_id), &mut toks, &mut matches);
            current = Some(sentence_id);
        }
        toks.push(Tok {
            form: row.get(1)?,
            lower: row.get(2)?,
            lemma: row.get(3)?,
            upos: row.get(4)?,
            xpos: row.get(5)?,
            lemma_override: row.get(6)?,
            upos_override: row.get(7)?,
        });
    }
    flush(current.unwrap_or_default(), &mut toks, &mut matches);

    Ok(matches)
}

/// A concordance: every hit, with document provenance, up to `limit`.
pub fn kwic(store: &Store, opts: &KwicOptions) -> Result<KwicResult> {
    let query = crate::query_lang::parse(&opts.pattern)?;
    let within = query.within.unwrap_or(opts.within);
    let expanded = crate::query_lang::canonical(&query, within);
    let total = store.token_count()?;

    let mut hits: Vec<(i64, Hit)> = Vec::new();
    let matches = for_each_hit(
        store,
        &query.pattern,
        opts.case_sensitive,
        within,
        |sid, toks, s, e| {
            if hits.len() < opts.limit {
                hits.push((
                    sid,
                    Hit {
                        sentence_id: sid,
                        start: s,
                        end: e,
                        left: context_around(toks, s, opts.context, Direction::Left),
                        match_text: toks[s..e]
                            .iter()
                            .map(|t| t.form.clone())
                            .collect::<Vec<_>>()
                            .join(" "),
                        right: context_around(toks, e, opts.context, Direction::Right),
                    },
                ));
            }
        },
    )?;

    let mut lines = Vec::with_capacity(hits.len());
    for (sentence_id, hit) in hits {
        let provenance = provenance(store, sentence_id)?;
        lines.push(KwicLine {
            document_id: provenance.document_id,
            document_title: provenance.document_title,
            source_url: provenance.source_url,
            document_external_id: provenance.document_external_id,
            sentence_id,
            sent_id: provenance.sent_id,
            newdoc_id: provenance.newdoc_id,
            left: hit.left,
            match_text: hit.match_text,
            right: hit.right,
        });
    }

    Ok(KwicResult {
        pattern: opts.pattern.clone(),
        expanded,
        tokens: total,
        matches,
        lines,
    })
}

/// Document and sentence provenance for a sentence.
struct Provenance {
    document_id: i64,
    document_title: Option<String>,
    source_url: Option<String>,
    document_external_id: Option<String>,
    sent_id: Option<String>,
    newdoc_id: Option<String>,
}

fn provenance(store: &Store, sentence_id: i64) -> Result<Provenance> {
    let row = store.conn().query_row(
        "SELECT d.id, d.title, d.source_url, d.external_id, s.external_id, s.newdoc_id
         FROM sentence s JOIN document d ON d.id = s.document_id
         WHERE s.id = ?1",
        rusqlite::params![sentence_id],
        |r| {
            Ok(Provenance {
                document_id: r.get(0)?,
                document_title: r.get(1)?,
                source_url: r.get(2)?,
                document_external_id: r.get(3)?,
                sent_id: r.get(4)?,
                newdoc_id: r.get(5)?,
            })
        },
    )?;
    Ok(row)
}

/// The `count` tokens before (`Left`) or after (`Right`) a boundary, joined
/// with single spaces. Truncated at the sentence edge.
pub(crate) fn context_around(toks: &[Tok], at: usize, count: usize, dir: Direction) -> String {
    if count == 0 {
        return String::new();
    }
    let (from, to) = match dir {
        Direction::Left => (at.saturating_sub(count), at.min(toks.len())),
        Direction::Right => (at.min(toks.len()), (at + count).min(toks.len())),
    };
    toks[from..to]
        .iter()
        .map(|t| t.form.as_str())
        .collect::<Vec<_>>()
        .join(" ")
}

/// All end positions reachable by matching `atom` at `pos`.
pub(crate) struct Matcher<'a> {
    pub toks: &'a [Tok],
    pub case_sensitive: bool,
}

fn eq_attr(actual: &str, wanted: &str, case_sensitive: bool) -> bool {
    if case_sensitive {
        actual == wanted
    } else {
        actual.to_lowercase() == wanted.to_lowercase()
    }
}

impl Matcher<'_> {
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
                    "lemma_override" => tok.lemma_override.as_deref(),
                    "upos" => tok.upos.as_deref(),
                    "upos_override" => tok.upos_override.as_deref(),
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

    pub(crate) fn pattern_ends(&self, pattern: &Pattern, pos: usize, limit: usize) -> Vec<usize> {
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
