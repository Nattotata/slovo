//! Concordance (KWIC) tests: the "trace a phrase back to its source" half of
//! a cluster query.

use slovo_core::{concordance, ingest, Store};

const FIXTURE: &str = "\
# sent_id = 1
# text = They take off.
1\tThey\tthey\tPRON\tPRP\t_\t2\tnsubj\t_\t_\n\
2\ttake\ttake\tVERB\tVBP\t_\t0\troot\t_\t_\n\
3\toff\toff\tADP\tIN\t_\t2\tprt\t_\t_\n\
4\t.\t.\tPUNCT\t.\t_\t2\tpunct\t_\t_\n\
\n\
# sent_id = 2
# text = They take up with him.
1\tThey\tthey\tPRON\tPRP\t_\t2\tnsubj\t_\t_\n\
2\ttake\ttake\tVERB\tVBP\t_\t0\troot\t_\t_\n\
3\tup\tup\tADP\tIN\t_\t2\tprt\t_\t_\n\
4\twith\twith\tADP\tIN\t_\t3\tprep\t_\t_\n\
5\thim\the\tPRON\tPRP\t_\t4\tpobj\t_\t_\n\
6\t.\t.\tPUNCT\t.\t_\t2\tpunct\t_\t_\n\
";

fn fixture() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("t.db");
    let src = dir.path().join("t.conllu");
    std::fs::write(&src, FIXTURE).unwrap();

    let mut store = Store::open(&db).unwrap();
    let mut options = ingest::ImportOptions::new("t");
    options.source = Some("https://example.org/t.txt".to_string());
    options.title = Some("Fixture".to_string());
    ingest::import_path(&mut store, &options, &src).unwrap();
    (dir, store)
}

fn opts(pattern: &str) -> concordance::KwicOptions {
    concordance::KwicOptions::new(pattern)
}

#[test]
fn kwic_reports_each_hit_with_context() {
    let (_dir, store) = fixture();
    let result = concordance::kwic(&store, &opts("take *")).unwrap();

    // Every span length is a hit, as in the cluster search.
    assert_eq!(result.matches, 6);
    assert_eq!(result.expanded, "[word=\"take\"] []+ within 5");

    let line = result
        .lines
        .iter()
        .find(|l| l.match_text == "take up with")
        .expect("take up with");
    assert_eq!(line.left, "They");
    assert_eq!(line.right, "him .");
}

#[test]
fn kwic_carries_document_provenance() {
    let (_dir, store) = fixture();
    let result = concordance::kwic(&store, &opts("take off")).unwrap();

    assert_eq!(result.lines.len(), 1);
    let line = &result.lines[0];
    assert!(line.document_id > 0);
    assert_eq!(line.document_title.as_deref(), Some("Fixture"));
    assert_eq!(
        line.source_url.as_deref(),
        Some("https://example.org/t.txt")
    );
    // Sentence-level ids travel with the hit, so `slovo doc` can be pointed at
    // the exact sentence.
    assert_eq!(line.sent_id.as_deref(), Some("1"));
}

#[test]
fn context_is_bounded_at_the_sentence_edge() {
    let (_dir, store) = fixture();
    let result = concordance::kwic(
        &store,
        &concordance::KwicOptions {
            context: 2,
            ..opts("take off")
        },
    )
    .unwrap();

    let line = &result.lines[0];
    // Sentence 1: "They take off ." — two tokens before `take`, one after.
    assert_eq!(line.left, "They");
    assert_eq!(line.right, ".");
}

#[test]
fn limit_caps_the_lines_but_not_the_match_count() {
    let (_dir, store) = fixture();
    let result = concordance::kwic(
        &store,
        &concordance::KwicOptions {
            limit: 2,
            ..opts("take *")
        },
    )
    .unwrap();

    assert_eq!(result.lines.len(), 2);
    assert_eq!(result.matches, 6, "count is over the whole corpus");
}

#[test]
fn a_bad_pattern_is_an_error() {
    let (_dir, store) = fixture();
    assert!(concordance::kwic(&store, &opts("[nope=\"x\"]")).is_err());
}
