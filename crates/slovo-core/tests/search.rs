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
    SearchOptions::new(pattern)
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

#[test]
fn weight_ranking_differs_from_frequency() {
    use slovo_core::query::By;
    use slovo_core::query::{frequency_ranked, Rank};

    // Two tokens: `body` twice at weight 1, `title` once at weight 9.
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("w.db");
    let src = dir.path().join("w.conllu");
    std::fs::write(
        &src,
        "1\tbody\tbody\tNOUN\tNN\t_\t_\t_\t_\t_\n\
         2\tbody\tbody\tNOUN\tNN\t_\t_\t_\t_\t_\n\
         3\ttitle\ttitle\tNOUN\tNN\t_\t_\t_\t_\tWeight=9\n",
    )
    .unwrap();

    let mut store = Store::open(&db).unwrap();
    ingest::import_path(&mut store, &ingest::ImportOptions::new("w"), &src).unwrap();

    let by_freq = frequency_ranked(&store, By::Word, 10, Rank::Frequency).unwrap();
    assert_eq!(by_freq[0].value, "body", "frequency counts occurrences");
    assert_eq!(by_freq[0].count, 2);

    let by_weight = frequency_ranked(&store, By::Word, 10, Rank::Weight).unwrap();
    assert_eq!(by_weight[0].value, "title", "weight outranks count");
    assert_eq!(by_weight[0].count, 9);
    // `body` still appears, at its summed weight of 2.
    let body = by_weight.iter().find(|r| r.value == "body").unwrap();
    assert_eq!(body.count, 2);
}

// The tagger mis-lemmatizes `spake`; a lexicon corrects it in a separate column.
const OVERRIDE_FIXTURE: &str = "\
# sent_id = 1
# text = He spake.
1\tHe\the\tPRON\tPRP\t_\t2\tnsubj\t_\t_\n\
2\tspake\tspake\tNOUN\tNN\t_\t0\troot\t_\tLemmaOverride=speak|UposOverride=VERB\n\
3\t.\t.\tPUNCT\t.\t_\t2\tpunct\t_\t_\n\
";

fn override_fixture() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("o.db");
    let src = dir.path().join("o.conllu");
    std::fs::write(&src, OVERRIDE_FIXTURE).unwrap();

    let mut store = Store::open(&db).unwrap();
    ingest::import_path(&mut store, &ingest::ImportOptions::new("o"), &src).unwrap();
    (dir, store)
}

#[test]
fn lemma_and_lemma_override_are_separate_layers() {
    let (_dir, store) = override_fixture();

    // The tagger's lemma is what it is: `spake`.
    let by_lemma = search::search(&store, &opts("[lemma=\"speak\"]")).unwrap();
    assert_eq!(by_lemma.matches, 0, "tagger's lemma is spake, not speak");
    let tagger_guess = search::search(&store, &opts("[lemma=\"spake\"]")).unwrap();
    assert_eq!(tagger_guess.matches, 1);

    // The correction is a distinct layer.
    let by_override = search::search(&store, &opts("[lemma_override=\"speak\"]")).unwrap();
    assert_eq!(by_override.matches, 1);
    assert_eq!(by_override.rows[0].text, "spake");

    // The surface form is untouched by either layer.
    let by_word = search::search(&store, &opts("spake")).unwrap();
    assert_eq!(by_word.matches, 1);

    // UPOS corrections are separate too.
    assert_eq!(
        search::search(&store, &opts("[upos=\"NOUN\"]"))
            .unwrap()
            .matches,
        1
    );
    assert_eq!(
        search::search(&store, &opts("[upos_override=\"VERB\"]"))
            .unwrap()
            .matches,
        1
    );
}

#[test]
fn examples_trace_a_row_back_to_its_sentence() {
    let (_dir, store) = fixture();
    let result = search::search(
        &store,
        &SearchOptions {
            examples: 3,
            context: 2,
            ..opts("take *")
        },
    )
    .unwrap();

    let row = result
        .rows
        .iter()
        .find(|r| r.text == "take up with")
        .expect("take up with");
    assert_eq!(row.examples.len(), 1);

    let hit = &row.examples[0];
    // Sentence 2: "They take up with him."
    assert_eq!(hit.match_text, "take up with");
    assert_eq!(hit.left, "They");
    assert_eq!(hit.right, "him .");
    assert_eq!(hit.start, 1);
    assert_eq!(hit.end, 4);
}

#[test]
fn examples_are_off_by_default() {
    let (_dir, store) = fixture();
    let result = search::search(&store, &opts("take *")).unwrap();
    assert!(result.rows.iter().all(|r| r.examples.is_empty()));
}

#[test]
fn examples_are_capped_per_row() {
    let (_dir, store) = fixture();
    let result = search::search(
        &store,
        &SearchOptions {
            examples: 1,
            ..opts("take _")
        },
    )
    .unwrap();
    // "take up" occurs in sentence 2 only; but each span keeps at most one hit.
    assert!(result.rows.iter().all(|r| r.examples.len() <= 1));
}
