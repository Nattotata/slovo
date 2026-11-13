//! Output rendering. Markdown is the default; TSV and JSON are for machines.

use serde::Serialize;

use slovo_core::query::per_million;
use slovo_core::FreqRow;

/// A frequency query, ready to be rendered or serialised.
#[derive(Serialize)]
pub struct FreqReport<'a> {
    pub dataset: &'a str,
    pub by: &'a str,
    pub tokens: u64,
    pub results: Vec<FreqEntry>,
}

#[derive(Serialize)]
pub struct FreqEntry {
    pub value: String,
    pub freq: u64,
    pub per_million: f64,
}

impl<'a> FreqReport<'a> {
    pub fn new(dataset: &'a str, by: &'a str, tokens: u64, rows: &[FreqRow]) -> Self {
        Self {
            dataset,
            by,
            tokens,
            results: rows
                .iter()
                .map(|row| FreqEntry {
                    value: row.value.clone(),
                    freq: row.count,
                    per_million: per_million(row.count, tokens),
                })
                .collect(),
        }
    }
}

pub fn freq_markdown(report: &FreqReport) -> String {
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

    out.push_str("| # | value | freq | per million |\n");
    out.push_str("|--:|:------|-----:|------------:|\n");
    for (i, entry) in report.results.iter().enumerate() {
        out.push_str(&format!(
            "| {} | {} | {} | {:.1} |\n",
            i + 1,
            escape_markdown(&entry.value),
            thousands(entry.freq),
            entry.per_million
        ));
    }
    out
}

pub fn freq_tsv(report: &FreqReport) -> String {
    let mut out = String::from("value\tfreq\tper_million\n");
    for entry in &report.results {
        out.push_str(&format!(
            "{}\t{}\t{:.2}\n",
            entry.value, entry.freq, entry.per_million
        ));
    }
    out
}

pub fn freq_json(report: &FreqReport) -> String {
    serde_json::to_string_pretty(report).expect("serialising a report cannot fail")
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
}
