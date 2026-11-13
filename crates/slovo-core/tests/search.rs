use slovo_core::{ingest, search, SearchOptions, Store};

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
    ingest::import_path(&mut store, &ingest::ImportOptions::new("t"), &src).unwrap();
    (dir, store)
}

fn opts(pattern: &str) -> SearchOptions {
    SearchOptions {
        pattern: pattern.to_string(),
        limit: 20,
        case_sensitive: false,
        within: 5,
        min_freq: 1,
    }
}

#[test]
fn cluster_query_reports_every_span_length() {
    let (_dir, store) = fixture();
    let result = search::search(&store, &opts("take *")).unwrap();

    // Every matched span length is counted separately.
    assert_eq!(result.matches, 6);
    assert_eq!(result.distinct, 6);
    assert_eq!(result.expanded, "[word=\"take\"] []+ within 5");

    let text_of = |text: &str| {
        result
            .rows
            .iter()
            .find(|r| r.text == text)
            .unwrap_or_else(|| panic!("missing `{text}`"))
    };
    assert_eq!(text_of("take off").freq, 1);
    assert_eq!(text_of("take up").freq, 1);
    assert_eq!(text_of("take up with").freq, 1);
    assert_eq!(text_of("take up with him").freq, 1);
}

#[test]
fn within_bounds_the_span() {
    let (_dir, store) = fixture();
    let result = search::search(&store, &opts("take * within 2")).unwrap();

    assert_eq!(result.matches, 2, "only two-token spans");
    assert!(result.rows.iter().all(|r| r.text.split(' ').count() == 2));
}

#[test]
fn underscore_requires_exactly_one_token() {
    let (_dir, store) = fixture();
    let result = search::search(&store, &opts("take _ off")).unwrap();

    assert_eq!(result.matches, 0, "nothing sits between take and off");

    let result = search::search(&store, &opts("take _ with")).unwrap();
    assert_eq!(result.matches, 1);
    assert_eq!(result.rows[0].text, "take up with");
}

#[test]
fn lemma_search_catches_inflections() {
    let (_dir, store) = fixture();
    let result = search::search(&store, &opts("[lemma=\"take\"] off")).unwrap();
    assert_eq!(result.matches, 1);
    assert_eq!(result.rows[0].text, "take off");
}

#[test]
fn upos_search_uses_the_tag() {
    let (_dir, store) = fixture();
    let result = search::search(&store, &opts("take NOUN")).unwrap();
    assert_eq!(result.matches, 0, "no noun directly after take here");

    let result = search::search(&store, &opts("take ADP")).unwrap();
    assert_eq!(result.matches, 2);
}

#[test]
fn case_sensitivity_is_opt_in() {
    let (_dir, store) = fixture();
    // Fixture has `They`, never `they`, so a case-sensitive query misses.
    let insensitive = search::search(&store, &opts("they")).unwrap();
    assert_eq!(insensitive.matches, 2);

    let sensitive = search::search(
        &store,
        &SearchOptions {
            case_sensitive: true,
            ..opts("they")
        },
    )
    .unwrap();
    assert_eq!(sensitive.matches, 0);

    let sensitive = search::search(
        &store,
        &SearchOptions {
            case_sensitive: true,
            ..opts("They")
        },
    )
    .unwrap();
    assert_eq!(sensitive.matches, 2);
}

#[test]
fn min_freq_filters_rows() {
    let (_dir, store) = fixture();
    let all = search::search(&store, &opts("take *")).unwrap();
    let filtered = search::search(
        &store,
        &SearchOptions {
            min_freq: 2,
            ..opts("take *")
        },
    )
    .unwrap();

    assert!(all.distinct > 0);
    assert_eq!(filtered.rows.len(), 0, "nothing occurs twice");
}

#[test]
fn a_bad_pattern_is_an_error() {
    let (_dir, store) = fixture();
    assert!(search::search(&store, &opts("[nope=\"x\"]")).is_err());
}
