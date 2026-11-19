//! Output rendering. Markdown is the default; TSV and JSON are for machines.
//!
//! The JSON shapes come from [`slovo_core::api`], so the CLI and the HTTP API
//! cannot drift apart.

use slovo_core::api::{
    DocumentEntry, DocumentSummaryEntry, FreqResponse, KwicResponse, SearchResponse,
};

pub fn freq_markdown(report: &FreqResponse) -> String {
    let mut out = String::new();
    let ranked = report.rank == "weight";
    let title = if ranked { "Ranked" } else { "Frequency" };
    out.push_str(&format!(
        "## {title} — `{}` (by {})\n\n",
        report.dataset, report.by
    ));

    if ranked {
        out.push_str(&format!(
            "_{} tokens · ordered by structural weight, not occurrence count_\n\n",
            thousands(report.tokens)
        ));
    } else {
        out.push_str(&format!("_{} tokens_\n\n", thousands(report.tokens)));
    }

    if report.results.is_empty() {
        out.push_str("_no matching tokens_\n");
        return out;
    }

    let score_header = if ranked { "weight" } else { "freq" };
    let columns = [
        Column::right(
            "#",
            (1..=report.results.len()).map(|i| i.to_string()).collect(),
        ),
        Column::left(
            "value",
            report
                .results
                .iter()
                .map(|e| escape_markdown(&e.value))
                .collect(),
        ),
        Column::right(
            score_header,
            report.results.iter().map(|e| thousands(e.freq)).collect(),
        ),
        // Per-million is meaningless for a weighted ranking.
        Column::right(
            "per million",
            report
                .results
                .iter()
                .map(|e| {
                    if ranked {
                        "-".to_string()
                    } else {
                        format!("{:.1}", e.per_million)
                    }
                })
                .collect(),
        ),
    ];
    out.push_str(&render_table(&columns));
    out
}

pub fn freq_tsv(report: &FreqResponse) -> String {
    let score_header = if report.rank == "weight" {
        "weight"
    } else {
        "freq"
    };
    let mut out = format!("value\t{score_header}\tper_million\n");
    for entry in &report.results {
        if report.rank == "weight" {
            out.push_str(&format!("{}\t{}\t\n", entry.value, entry.freq));
        } else {
            out.push_str(&format!(
                "{}\t{}\t{:.2}\n",
                entry.value, entry.freq, entry.per_million
            ));
        }
    }
    out
}

pub fn freq_json(report: &FreqResponse) -> String {
    serde_json::to_string_pretty(report).expect("serialising a report cannot fail")
}

pub fn search_markdown(report: &SearchResponse) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "## `{}` — dataset `{}`\n\n",
        report.query, report.dataset
    ));

    if report.expanded != report.query {
        out.push_str(&format!("`{}` → `{}`\n\n", report.query, report.expanded));
    }

    out.push_str(&format!(
        "_{} tokens · {} matches · {} distinct_\n\n",
        thousands(report.tokens),
        thousands(report.matches),
        thousands(report.distinct as u64)
    ));

    if report.results.is_empty() {
        out.push_str("_no matches_\n");
        return out;
    }

    let columns = [
        Column::right(
            "#",
            (1..=report.results.len()).map(|i| i.to_string()).collect(),
        ),
        Column::left(
            "match",
            report
                .results
                .iter()
                .map(|e| escape_markdown(&e.text))
                .collect(),
        ),
        Column::right(
            "freq",
            report.results.iter().map(|e| thousands(e.freq)).collect(),
        ),
        Column::right(
            "per million",
            report
                .results
                .iter()
                .map(|e| format!("{:.1}", e.per_million))
                .collect(),
        ),
    ];
    out.push_str(&render_table(&columns));

    if report.results.iter().any(|e| !e.examples.is_empty()) {
        out.push_str("\n### Examples\n");
        for entry in report.results.iter().filter(|e| !e.examples.is_empty()) {
            out.push_str(&format!("\n**{}**\n\n", escape_markdown(&entry.text)));
            for example in &entry.examples {
                out.push_str(&format!("- {}\n", render_kwic_line(example)));
            }
        }
    }

    out
}

/// One concordance line as ``left **match** right``.
fn render_kwic_line(example: &slovo_core::api::ExampleEntry) -> String {
    let mut parts = Vec::new();
    if !example.left.is_empty() {
        parts.push(escape_markdown(&example.left));
    }
    parts.push(format!("**{}**", escape_markdown(&example.match_text)));
    if !example.right.is_empty() {
        parts.push(escape_markdown(&example.right));
    }
    parts.join(" ")
}

pub fn kwic_markdown(report: &KwicResponse) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "## `{}` — dataset `{}`\n\n",
        report.query, report.dataset
    ));

    if report.expanded != report.query {
        out.push_str(&format!("`{}` → `{}`\n\n", report.query, report.expanded));
    }

    out.push_str(&format!(
        "_{} tokens · {} matches · showing {}_\n\n",
        thousands(report.tokens),
        thousands(report.matches),
        thousands(report.results.len() as u64)
    ));

    if report.results.is_empty() {
        out.push_str("_no matches_\n");
        return out;
    }

    // Left/right columns are padded so the match reads as a centred column.
    let left_width = report
        .results
        .iter()
        .map(|line| line.left.chars().count())
        .max()
        .unwrap_or(0);
    let match_width = report
        .results
        .iter()
        .map(|line| line.match_text.chars().count())
        .max()
        .unwrap_or(0);

    for line in &report.results {
        out.push_str(&format!(
            "{}   {}   {}{}\n",
            pad_left(&line.left, left_width),
            pad_right(&line.match_text, match_width),
            escape_markdown(&line.right),
            provenance_suffix(line),
        ));
    }
    out
}

fn provenance_suffix(line: &slovo_core::api::KwicEntry) -> String {
    // Prefer a real link; otherwise point at `slovo doc`, which can resolve
    // the document id. The sentence id makes the line precise.
    let where_ = match &line.source_url {
        Some(url) => url.clone(),
        None => format!("doc {}", line.document_id),
    };
    let sent = line
        .sent_id
        .clone()
        .unwrap_or_else(|| format!("#{}", line.sentence_id));
    format!("   [{where_} · {sent}]")
}

fn pad_left(text: &str, width: usize) -> String {
    let fill = width.saturating_sub(text.chars().count());
    format!("{}{text}", " ".repeat(fill))
}

fn pad_right(text: &str, width: usize) -> String {
    let fill = width.saturating_sub(text.chars().count());
    format!("{text}{}", " ".repeat(fill))
}

pub fn kwic_tsv(report: &KwicResponse) -> String {
    let mut out = String::from("left\tmatch\tright\tdocument_id\tsentence_id\tsource_url\n");
    for line in &report.results {
        out.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\n",
            line.left,
            line.match_text,
            line.right,
            line.document_id,
            line.sentence_id,
            line.source_url.as_deref().unwrap_or(""),
        ));
    }
    out
}

pub fn kwic_json(report: &KwicResponse) -> String {
    serde_json::to_string_pretty(report).expect("serialising a report cannot fail")
}

pub fn doc_markdown(doc: &DocumentEntry) -> String {
    let mut out = String::new();
    let title = doc.title.as_deref().unwrap_or("(untitled)");
    out.push_str(&format!(
        "## {} — `doc {}`\n\n",
        escape_markdown(title),
        doc.document_id
    ));

    if let Some(external) = &doc.external_id {
        out.push_str(&format!("- **source:** `{external}`\n"));
    }
    if let Some(url) = &doc.source_url {
        out.push_str(&format!("- **url:** {url}\n"));
    }
    if let Some(license) = &doc.license {
        out.push_str(&format!("- **license:** {license}\n"));
    }
    out.push_str(&format!(
        "- **sentences:** {}\n\n",
        thousands(doc.sentences.len() as u64)
    ));

    for sentence in &doc.sentences {
        let label = sentence
            .sent_id
            .clone()
            .unwrap_or_else(|| format!("#{}", sentence.sentence_id));
        match &sentence.text {
            Some(text) => out.push_str(&format!("{label}: {text}\n")),
            None => out.push_str(&format!("{label}:\n")),
        }
    }
    out
}

pub fn doc_tsv(doc: &DocumentEntry) -> String {
    let mut out = String::from("sentence_id\tsent_id\ttext\n");
    for sentence in &doc.sentences {
        out.push_str(&format!(
            "{}\t{}\t{}\n",
            sentence.sentence_id,
            sentence.sent_id.as_deref().unwrap_or(""),
            sentence.text.as_deref().unwrap_or(""),
        ));
    }
    out
}

pub fn docs_markdown(dataset: &str, docs: &[DocumentSummaryEntry]) -> String {
    let mut out = format!("## Documents — `{dataset}`\n\n");

    if docs.is_empty() {
        out.push_str("_no documents_\n");
        return out;
    }

    let columns = [
        Column::right(
            "id",
            docs.iter().map(|d| d.document_id.to_string()).collect(),
        ),
        Column::left(
            "title",
            docs.iter()
                .map(|d| escape_markdown(d.title.as_deref().unwrap_or("(untitled)")))
                .collect(),
        ),
        Column::right(
            "sentences",
            docs.iter().map(|d| thousands(d.sentences)).collect(),
        ),
        Column::right("tokens", docs.iter().map(|d| thousands(d.tokens)).collect()),
    ];
    out.push_str(&render_table(&columns));
    out
}

pub fn docs_tsv(docs: &[DocumentSummaryEntry]) -> String {
    let mut out = String::from("id\ttitle\tsentences\ttokens\tsource_url\n");
    for doc in docs {
        out.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\n",
            doc.document_id,
            doc.title.as_deref().unwrap_or(""),
            doc.sentences,
            doc.tokens,
            doc.source_url.as_deref().unwrap_or(""),
        ));
    }
    out
}

pub fn search_tsv(report: &SearchResponse) -> String {
    let mut out = String::from("match\tfreq\tper_million\n");
    for entry in &report.results {
        out.push_str(&format!(
            "{}\t{}\t{:.2}\n",
            entry.text, entry.freq, entry.per_million
        ));
    }
    out
}

pub fn search_json(report: &SearchResponse) -> String {
    serde_json::to_string_pretty(report).expect("serialising a report cannot fail")
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Align {
    Left,
    Right,
}

struct Column {
    header: &'static str,
    align: Align,
    cells: Vec<String>,
}

impl Column {
    fn left(header: &'static str, cells: Vec<String>) -> Self {
        Self {
            header,
            align: Align::Left,
            cells,
        }
    }

    fn right(header: &'static str, cells: Vec<String>) -> Self {
        Self {
            header,
            align: Align::Right,
            cells,
        }
    }
}

/// Render a padded Markdown table, so the raw output is readable too.
fn render_table(columns: &[Column]) -> String {
    let widths: Vec<usize> = columns
        .iter()
        .map(|column| {
            let widest = column
                .cells
                .iter()
                .map(|cell| cell.chars().count())
                .max()
                .unwrap_or(0);
            column.header.chars().count().max(widest).max(3)
        })
        .collect();

    let pad = |text: &str, width: usize, align: Align| {
        let fill = width.saturating_sub(text.chars().count());
        match align {
            Align::Left => format!("{text}{}", " ".repeat(fill)),
            Align::Right => format!("{}{text}", " ".repeat(fill)),
        }
    };

    let mut out = String::new();

    out.push('|');
    for (column, width) in columns.iter().zip(&widths) {
        out.push_str(&format!(" {} |", pad(column.header, *width, column.align)));
    }
    out.push('\n');

    out.push('|');
    for (column, width) in columns.iter().zip(&widths) {
        let dashes = width.saturating_sub(1);
        let cell = match column.align {
            Align::Left => format!(":{}", "-".repeat(dashes)),
            Align::Right => format!("{}:", "-".repeat(dashes)),
        };
        out.push_str(&format!(" {cell} |"));
    }
    out.push('\n');

    let height = columns.first().map(|c| c.cells.len()).unwrap_or(0);
    for row in 0..height {
        out.push('|');
        for (column, width) in columns.iter().zip(&widths) {
            let cell = column.cells.get(row).map(String::as_str).unwrap_or("");
            out.push_str(&format!(" {} |", pad(cell, *width, column.align)));
        }
        out.push('\n');
    }

    out
}

/// `254820` → `254,820`.
pub fn thousands(n: u64) -> String {
    let digits = n.to_string();
    let len = digits.len();
    let mut out = String::with_capacity(len + len / 3);
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (len - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

fn escape_markdown(value: &str) -> String {
    value.replace('|', "\\|")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_digits() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(1000), "1,000");
        assert_eq!(thousands(254820), "254,820");
        assert_eq!(thousands(1_000_000), "1,000,000");
    }

    #[test]
    fn escapes_pipes_in_markdown_tables() {
        assert_eq!(escape_markdown("a|b"), "a\\|b");
    }

    #[test]
    fn table_columns_are_padded_to_equal_width() {
        let table = render_table(&[
            Column::right("#", vec!["1".into(), "10".into()]),
            Column::left("match", vec!["take a".into(), "take care of".into()]),
            Column::right("freq", vec!["3".into(), "1,234".into()]),
        ]);

        let lines: Vec<&str> = table.lines().collect();
        assert_eq!(lines[0], "|   # | match        |  freq |");
        assert_eq!(lines[1], "| --: | :----------- | ----: |");
        assert_eq!(lines[2], "|   1 | take a       |     3 |");
        assert_eq!(lines[3], "|  10 | take care of | 1,234 |");

        let widths: Vec<usize> = lines.iter().map(|line| line.chars().count()).collect();
        assert!(
            widths.windows(2).all(|pair| pair[0] == pair[1]),
            "ragged table:\n{table}"
        );
    }
}
