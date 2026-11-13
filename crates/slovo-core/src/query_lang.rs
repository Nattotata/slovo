//! Slovo's query language.
//!
//! A small subset of **CQP** (the IMS Corpus Workbench query language), plus a
//! few aliases for the cases people actually type most:
//!
//! | you write | expands to      | meaning              |
//! |-----------|-----------------|----------------------|
//! | `take`    | `[word="take"]` | the word *take*      |
//! | `_`       | `[]`            | any single token     |
//! | `*`       | `[]+`           | one or more tokens   |
//! | `NOUN`    | `[upos="NOUN"]` | part-of-speech tag   |
//!
//! The rules that keep it unambiguous:
//!
//! * A quantifier must be **attached** to the node it modifies: `[]+`, not `[] +`.
//! * A **standalone** `*` is therefore free to be the alias for `[]+`.
//! * Whitespace separates nodes, so `take off` is a two-token sequence.
//!
//! Everything else is CQP: `[]`, `?`, `*`, `+`, `{n}`, `{n,m}`, `{n,}`, `{,m}`,
//! `(...)`, `|`, `[attr="value"]`, `[attr!="value"]`, and a trailing `within N`.

use std::fmt;

/// Default cap on how many tokens a pattern may span.
pub const DEFAULT_WITHIN: u32 = 5;

/// Hard cap, regardless of `within`, so a bad query cannot run away.
pub const HARD_MAX_SPAN: usize = 20;

/// Attribute names that can be used inside `[...]`.
pub const ATTRS: &[&str] = &["word", "form", "lower", "lemma", "upos", "xpos"];

/// Universal POS tags, for the bare-tag alias (`NOUN` → `[upos="NOUN"]`).
pub const UPOS: &[&str] = &[
    "ADJ", "ADP", "ADV", "AUX", "CCONJ", "DET", "INTJ", "NOUN", "NUM", "PART", "PRON", "PROPN",
    "PUNCT", "SCONJ", "SYM", "VERB",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Atom {
    /// `[]`
    Any,
    /// A bare word.
    Word(String),
    /// `[name="value"]` or `[name!="value"]`
    Attr {
        name: String,
        negated: bool,
        value: String,
    },
    /// `( ... )`
    Group(Box<Pattern>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quantifier {
    One,
    /// `?`
    Optional,
    /// `*`
    ZeroOrMore,
    /// `+`
    OneOrMore,
    /// `{n}`
    Exactly(u32),
    /// `{n,m}`
    Between(u32, u32),
    /// `{n,}`
    AtLeast(u32),
    /// `{,m}`
    AtMost(u32),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    pub atom: Atom,
    pub quantifier: Quantifier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pattern {
    Sequence(Vec<Node>),
    Alternation(Vec<Pattern>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Query {
    pub pattern: Pattern,
    pub within: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryError {
    pub message: String,
    pub column: usize,
}

impl fmt::Display for QueryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} (at column {})", self.message, self.column)
    }
}

impl std::error::Error for QueryError {}

type QResult<T> = std::result::Result<T, QueryError>;

fn err<T>(message: impl Into<String>, column: usize) -> QResult<T> {
    Err(QueryError {
        message: message.into(),
        column,
    })
}

// ---------------------------------------------------------------- lexer

#[derive(Debug, Clone, PartialEq, Eq)]
enum Elem {
    Bracket(String),
    Bare(String),
    Any,
    Quant(char),
    Brace(String),
    LParen,
    RParen,
    Pipe,
}

/// One element plus whether whitespace preceded it (needed to tell an attached
/// quantifier from a standalone `*`).
#[derive(Debug, Clone)]
struct Lexed {
    elem: Elem,
    column: usize,
    after_space: bool,
}

fn lex(input: &str) -> QResult<(Vec<Lexed>, bool)> {
    let chars: Vec<char> = input.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    let mut after_space = true;
    let mut case_flag = false;

    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
            after_space = true;
            continue;
        }
        let column = i + 1;
        let elem = match c {
            '[' => {
                let mut j = i + 1;
                while j < chars.len() && chars[j] != ']' {
                    j += 1;
                }
                if j >= chars.len() {
                    return err("unclosed `[`", column);
                }
                let inner: String = chars[i + 1..j].iter().collect();
                i = j + 1;
                Elem::Bracket(inner)
            }
            '(' => {
                i += 1;
                Elem::LParen
            }
            ')' => {
                i += 1;
                Elem::RParen
            }
            '|' => {
                i += 1;
                Elem::Pipe
            }
            '{' => {
                let mut j = i + 1;
                while j < chars.len() && chars[j] != '}' {
                    j += 1;
                }
                if j >= chars.len() {
                    return err("unclosed `{`", column);
                }
                let inner: String = chars[i + 1..j].iter().collect();
                i = j + 1;
                Elem::Brace(inner)
            }
            '?' | '*' | '+' => {
                i += 1;
                Elem::Quant(c)
            }
            '_' => {
                i += 1;
                Elem::Any
            }
            _ => {
                let mut j = i;
                while j < chars.len()
                    && !chars[j].is_whitespace()
                    && !matches!(
                        chars[j],
                        '[' | ']' | '(' | ')' | '|' | '{' | '}' | '?' | '*' | '+'
                    )
                {
                    j += 1;
                }
                let run: String = chars[i..j].iter().collect();
                i = j;
                if run == "%c" {
                    case_flag = true;
                    after_space = false;
                    continue;
                }
                Elem::Bare(run)
            }
        };
        out.push(Lexed {
            elem,
            column,
            after_space,
        });
        after_space = false;
    }

    Ok((out, case_flag))
}

// --------------------------------------------------------------- parser

struct Parser {
    elems: Vec<Lexed>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Elem> {
        self.elems.get(self.pos).map(|l| &l.elem)
    }

    fn peek_space(&self) -> bool {
        self.elems
            .get(self.pos)
            .map(|l| l.after_space)
            .unwrap_or(true)
    }

    fn column(&self) -> usize {
        self.elems.get(self.pos).map(|l| l.column).unwrap_or(0)
    }

    fn parse_query(&mut self) -> QResult<Query> {
        let pattern = self.parse_alternation()?;

        let mut within = None;
        if let Some(Elem::Bare(word)) = self.peek() {
            if word == "within" {
                self.pos += 1;
                match self.peek().cloned() {
                    Some(Elem::Bare(value)) if value.eq_ignore_ascii_case("s") => {
                        self.pos += 1;
                        // Sentence-bounded is our default; no numeric cap.
                    }
                    Some(Elem::Bare(value)) => {
                        let column = self.column();
                        let n: u32 = value.parse().map_err(|_| QueryError {
                            message: format!("`within` wants a number, got `{value}`"),
                            column,
                        })?;
                        self.pos += 1;
                        within = Some(n);
                    }
                    _ => return err("`within` wants a number", self.column()),
                }
            }
        }

        if self.pos < self.elems.len() {
            return err(
                format!("unexpected {}", describe(self.peek().unwrap())),
                self.column(),
            );
        }
        Ok(Query { pattern, within })
    }

    fn parse_alternation(&mut self) -> QResult<Pattern> {
        let mut branches = vec![self.parse_sequence()?];
        while matches!(self.peek(), Some(Elem::Pipe)) {
            self.pos += 1;
            branches.push(self.parse_sequence()?);
        }
        if branches.len() == 1 {
            Ok(branches.pop().unwrap())
        } else {
            Ok(Pattern::Alternation(branches))
        }
    }

    fn parse_sequence(&mut self) -> QResult<Pattern> {
        let mut nodes = Vec::new();
        loop {
            match self.peek() {
                None | Some(Elem::RParen) | Some(Elem::Pipe) => break,
                Some(Elem::Bare(word)) if word == "within" => break,
                _ => {}
            }
            nodes.push(self.parse_node()?);
        }
        Ok(Pattern::Sequence(nodes))
    }

    fn parse_node(&mut self) -> QResult<Node> {
        // `*` standing alone is the alias for `[]+`.
        if let Some(Elem::Quant('*')) = self.peek() {
            if self.peek_space() {
                self.pos += 1;
                return Ok(Node {
                    atom: Atom::Any,
                    quantifier: Quantifier::OneOrMore,
                });
            }
        }

        let lexed = self.elems[self.pos].clone();
        let column = lexed.column;
        let atom = match &lexed.elem {
            Elem::Any => {
                self.pos += 1;
                Atom::Any
            }
            Elem::Bracket(inner) => {
                self.pos += 1;
                parse_bracket(inner, column)?
            }
            Elem::Bare(word) => {
                self.pos += 1;
                if UPOS.contains(&word.as_str()) {
                    Atom::Attr {
                        name: "upos".into(),
                        negated: false,
                        value: word.clone(),
                    }
                } else {
                    Atom::Word(word.clone())
                }
            }
            Elem::LParen => {
                self.pos += 1;
                let inner = self.parse_alternation()?;
                match self.peek() {
                    Some(Elem::RParen) => self.pos += 1,
                    _ => return err("missing `)`", column),
                }
                Atom::Group(Box::new(inner))
            }
            other => {
                return err(
                    format!("expected a token, found {}", describe(other)),
                    column,
                )
            }
        };

        let quantifier = self.parse_quantifier()?;
        Ok(Node { atom, quantifier })
    }

    fn parse_quantifier(&mut self) -> QResult<Quantifier> {
        // Only attached quantifiers count.
        if self.peek_space() {
            return Ok(Quantifier::One);
        }
        let column = self.column();
        match self.peek().cloned() {
            Some(Elem::Quant('?')) => {
                self.pos += 1;
                Ok(Quantifier::Optional)
            }
            Some(Elem::Quant('*')) => {
                self.pos += 1;
                Ok(Quantifier::ZeroOrMore)
            }
            Some(Elem::Quant('+')) => {
                self.pos += 1;
                Ok(Quantifier::OneOrMore)
            }
            Some(Elem::Brace(inner)) => {
                self.pos += 1;
                parse_brace(&inner, column)
            }
            _ => Ok(Quantifier::One),
        }
    }
}

fn parse_brace(inner: &str, column: usize) -> QResult<Quantifier> {
    let inner = inner.trim();
    let parts: Vec<&str> = inner.split(',').collect();
    let number = |s: &str| -> QResult<u32> {
        s.trim().parse::<u32>().map_err(|_| QueryError {
            message: format!("`{{{inner}}}` is not a valid repetition"),
            column,
        })
    };
    match parts.as_slice() {
        [n] => Ok(Quantifier::Exactly(number(n)?)),
        [a, b] if b.trim().is_empty() => Ok(Quantifier::AtLeast(number(a)?)),
        [a, b] if a.trim().is_empty() => Ok(Quantifier::AtMost(number(b)?)),
        [a, b] => {
            let (a, b) = (number(a)?, number(b)?);
            if a > b {
                return err(
                    format!("`{{{inner}}}` has its bounds the wrong way round"),
                    column,
                );
            }
            Ok(Quantifier::Between(a, b))
        }
        _ => err(format!("`{{{inner}}}` is not a valid repetition"), column),
    }
}

fn parse_bracket(inner: &str, column: usize) -> QResult<Atom> {
    // `%c` is CQP's case-insensitivity marker; Slovo is case-insensitive by
    // default, so accept and ignore it.
    let inner = inner.trim().trim_end_matches("%c").trim();
    if inner.is_empty() {
        return Ok(Atom::Any);
    }

    let (name, rest) = match inner.split_once("!=") {
        Some((name, rest)) => (name.trim(), (rest, true)),
        None => match inner.split_once('=') {
            Some((name, rest)) => (name.trim(), (rest, false)),
            None => {
                return err(
                    format!("`[{inner}]` should look like `[attr=\"value\"]`"),
                    column,
                )
            }
        },
    };
    let (rest, negated) = rest;

    if !ATTRS.contains(&name) {
        return err(
            format!(
                "unknown attribute `{name}`; try one of {}",
                ATTRS.join(", ")
            ),
            column,
        );
    }

    let value = rest.trim().trim_matches('"').to_string();
    Ok(Atom::Attr {
        name: name.to_string(),
        negated,
        value,
    })
}

fn describe(elem: &Elem) -> String {
    match elem {
        Elem::Bracket(_) => "`[...]`".into(),
        Elem::Bare(word) => format!("`{word}`"),
        Elem::Any => "`_`".into(),
        Elem::Quant(c) => format!("`{c}`"),
        Elem::Brace(_) => "`{...}`".into(),
        Elem::LParen => "`(`".into(),
        Elem::RParen => "`)`".into(),
        Elem::Pipe => "`|`".into(),
    }
}

// -------------------------------------------------------------- parsing

pub fn parse(input: &str) -> QResult<Query> {
    let (elems, _case_flag) = lex(input)?;
    if elems.is_empty() {
        return err("empty query", 1);
    }
    let mut parser = Parser { elems, pos: 0 };
    parser.parse_query()
}

// ------------------------------------------------------------ canonical

/// Render a query back to canonical CQP, for `--explain`.
pub fn canonical(query: &Query, within: u32) -> String {
    let mut out = render_pattern(&query.pattern);
    let bound = query.within.unwrap_or(within);
    out.push_str(&format!(" within {bound}"));
    out
}

fn render_pattern(pattern: &Pattern) -> String {
    match pattern {
        Pattern::Sequence(nodes) => nodes.iter().map(render_node).collect::<Vec<_>>().join(" "),
        Pattern::Alternation(branches) => branches
            .iter()
            .map(|branch| match branch {
                // Parentheses only where precedence needs them.
                Pattern::Sequence(nodes) if nodes.len() > 1 => {
                    format!("({})", render_pattern(branch))
                }
                _ => render_pattern(branch),
            })
            .collect::<Vec<_>>()
            .join(" | "),
    }
}

fn render_node(node: &Node) -> String {
    format!(
        "{}{}",
        render_atom(&node.atom),
        render_quantifier(node.quantifier)
    )
}

fn render_atom(atom: &Atom) -> String {
    match atom {
        Atom::Any => "[]".into(),
        Atom::Word(word) => format!("[word=\"{word}\"]"),
        Atom::Attr {
            name,
            negated,
            value,
        } => {
            let op = if *negated { "!=" } else { "=" };
            format!("[{name}{op}\"{value}\"]")
        }
        Atom::Group(inner) => format!("({})", render_pattern(inner)),
    }
}

fn render_quantifier(quantifier: Quantifier) -> String {
    match quantifier {
        Quantifier::One => String::new(),
        Quantifier::Optional => "?".into(),
        Quantifier::ZeroOrMore => "*".into(),
        Quantifier::OneOrMore => "+".into(),
        Quantifier::Exactly(n) => format!("{{{n}}}"),
        Quantifier::Between(a, b) => format!("{{{a},{b}}}"),
        Quantifier::AtLeast(n) => format!("{{{n},}}"),
        Quantifier::AtMost(m) => format!("{{,{m}}}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_ok(input: &str) -> Query {
        parse(input).unwrap_or_else(|e| panic!("{input}: {e}"))
    }

    #[test]
    fn bare_words_become_word_nodes() {
        let query = parse_ok("take off");
        assert_eq!(
            query.pattern,
            Pattern::Sequence(vec![
                Node {
                    atom: Atom::Word("take".into()),
                    quantifier: Quantifier::One
                },
                Node {
                    atom: Atom::Word("off".into()),
                    quantifier: Quantifier::One
                },
            ])
        );
    }

    #[test]
    fn standalone_star_is_one_or_more() {
        let query = parse_ok("take *");
        let Pattern::Sequence(nodes) = &query.pattern else {
            panic!()
        };
        assert_eq!(nodes[0].atom, Atom::Word("take".into()));
        assert_eq!(nodes[1].atom, Atom::Any);
        assert_eq!(nodes[1].quantifier, Quantifier::OneOrMore);
    }

    #[test]
    fn attached_star_is_a_quantifier() {
        let query = parse_ok("[]*");
        let Pattern::Sequence(nodes) = &query.pattern else {
            panic!()
        };
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].atom, Atom::Any);
        assert_eq!(nodes[0].quantifier, Quantifier::ZeroOrMore);
    }

    #[test]
    fn underscore_is_one_token() {
        let query = parse_ok("take _ off");
        let Pattern::Sequence(nodes) = &query.pattern else {
            panic!()
        };
        assert_eq!(nodes[1].atom, Atom::Any);
        assert_eq!(nodes[1].quantifier, Quantifier::One);
    }

    #[test]
    fn uppercase_tag_is_upos() {
        let query = parse_ok("take NOUN");
        let Pattern::Sequence(nodes) = &query.pattern else {
            panic!()
        };
        assert_eq!(
            nodes[1].atom,
            Atom::Attr {
                name: "upos".into(),
                negated: false,
                value: "NOUN".into()
            }
        );
    }

    #[test]
    fn bracketed_attributes_parse() {
        let query = parse_ok("[lemma=\"take\"] []{1,3} [upos!=\"PUNCT\"]");
        let Pattern::Sequence(nodes) = &query.pattern else {
            panic!()
        };
        assert_eq!(
            nodes[0].atom,
            Atom::Attr {
                name: "lemma".into(),
                negated: false,
                value: "take".into()
            }
        );
        assert_eq!(nodes[1].quantifier, Quantifier::Between(1, 3));
        assert!(matches!(nodes[2].atom, Atom::Attr { negated: true, .. }));
    }

    #[test]
    fn within_is_parsed() {
        let query = parse_ok("take * within 3");
        assert_eq!(query.within, Some(3));
    }

    #[test]
    fn alternation_and_groups_parse() {
        let query = parse_ok("(fast|quick) NOUN");
        assert!(matches!(query.pattern, Pattern::Sequence(_)));
    }

    #[test]
    fn canonical_form_is_cqp() {
        let query = parse_ok("take *");
        assert_eq!(canonical(&query, 5), "[word=\"take\"] []+ within 5");
    }

    #[test]
    fn errors_are_reported_with_a_column() {
        assert!(parse("[").is_err());
        assert!(parse("[nope=\"x\"]").is_err());
        assert!(parse("take (off").is_err());
        // A spaced `+` is not a node, unlike a spaced `*`.
        assert!(parse("[] +").is_err());
        assert!(parse("[] +")
            .unwrap_err()
            .message
            .contains("expected a token"));
    }
}
