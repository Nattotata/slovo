//! Output rendering. Markdown is the default; TSV and JSON are for machines.
//!
//! The JSON shapes come from [`slovo_core::api`], so the CLI and the HTTP API
//! cannot drift apart.

use slovo_core::api::{FreqResponse, SearchResponse};

pub fn freq_markdown(report: &FreqResponse) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "## Frequency — `{}` (by {})\n\n",
        report.dataset, report.by
    ));
    out.push_str(&format!("_{} tokens_\n\n", thousands(report.tokens)));

    if report.results.is_empty() {
        out.push_str("_no matching tokens_\n");
        return out;
    }

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
    out
}

pub fn freq_tsv(report: &FreqResponse) -> String {
    let mut out = String::from("value\tfreq\tper_million\n");
    for entry in &report.results {
        out.push_str(&format!(
            "{}\t{}\t{:.2}\n",
            entry.value, entry.freq, entry.per_million
        ));
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
