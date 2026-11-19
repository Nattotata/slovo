use slovo_core::query::By;
use slovo_core::{ingest, Store};

// Two sentences, eight tokens. "The"/"the" appears twice.
const FIXTURE: &str = "\
# sent_id = 1
# text = The cat sat.
1\tThe\tthe\tDET\tDT\tDefinite=Def\t2\tdet\t_\t_\n\
2\tcat\tcat\tNOUN\tNN\tNumber=Sing\t3\tnsubj\t_\t_\n\
3\tsat\tsit\tVERB\tVBD\tTense=Past\t0\troot\t_\t_\n\
4\t.\t.\tPUNCT\t.\t_\t3\tpunct\t_\t_\n\
\n\
# sent_id = 2
# text = The dogs barked.
1\tThe\tthe\tDET\tDT\tDefinite=Def\t2\tdet\t_\t_\n\
2\tdogs\tdog\tNOUN\tNNS\tNumber=Plur\t3\tnsubj\t_\t_\n\
3\tbarked\tbark\tVERB\tVBD\tTense=Past\t0\troot\t_\t_\n\
4\t.\t.\tPUNCT\t.\t_\t3\tpunct\t_\t_\n\
";

#[test]
fn imports_a_treebank_and_answers_a_frequency_query() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("sample.db");
    let src = dir.path().join("sample.conllu");
    std::fs::write(&src, FIXTURE).unwrap();

    let mut store = Store::open(&db).unwrap();
    let opts = ingest::ImportOptions::new("sample");
    let stats = ingest::import_path(&mut store, &opts, &src).unwrap();

    assert_eq!(stats.documents, 1);
    assert_eq!(stats.sentences, 2);
    assert_eq!(stats.tokens, 8);

    assert_eq!(store.token_count().unwrap(), 8);
    let counts = store.stats().unwrap();
    assert_eq!(counts.documents, 1);
    assert_eq!(counts.sentences, 2);
    assert_eq!(counts.tokens, 8);

    let words = slovo_core::query::frequency(&store, By::Word, 10).unwrap();
    // "the" and "." both occur twice; ties break alphabetically.
    assert_eq!(words[0].count, 2);
    assert_eq!(words.len(), 6, "the, ., cat, sat, dogs, barked");
    let the = words.iter().find(|r| r.value == "the").unwrap();
    assert_eq!(the.count, 2);

    let lemmas = slovo_core::query::frequency(&store, By::Lemma, 10).unwrap();
    assert!(lemmas.iter().any(|r| r.value == "sit" && r.count == 1));

    let upos = slovo_core::query::frequency(&store, By::Upos, 10).unwrap();
    // DET and PUNCT both occur twice; "DET" sorts first.
    assert_eq!(upos[0].value, "DET");
    assert_eq!(upos[0].count, 2);
    assert!(upos.iter().any(|r| r.value == "PUNCT" && r.count == 2));
}

#[test]
fn re_importing_the_same_file_is_a_no_op() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("sample.db");
    let src = dir.path().join("sample.conllu");
    std::fs::write(&src, FIXTURE).unwrap();

    let mut store = Store::open(&db).unwrap();
    let opts = ingest::ImportOptions::new("sample");

    let first = ingest::import_path(&mut store, &opts, &src).unwrap();
    assert_eq!(first.documents, 1);
    assert_eq!(first.skipped, 0);
    assert_eq!(store.token_count().unwrap(), 8);

    let second = ingest::import_path(&mut store, &opts, &src).unwrap();
    assert_eq!(second.documents, 0);
    assert_eq!(second.skipped, 1);
    assert_eq!(store.token_count().unwrap(), 8, "corpus must not double");
}

// Two `newdoc` blocks in one file become two documents.
const MULTIDOC: &str = "\
# newdoc id = doc-a
# sent_id = doc-a-1
# text = The cat sat.
1\tThe\tthe\tDET\tDT\t_\t2\tdet\t_\t_\n\
2\tcat\tcat\tNOUN\tNN\t_\t3\tnsubj\t_\t_\n\
3\tsat\tsit\tVERB\tVBD\t_\t0\troot\t_\t_\n\
\n\
# newdoc id = doc-b
# sent_id = doc-b-1
# text = Dogs bark.
1\tDogs\tdog\tNOUN\tNNS\t_\t2\tnsubj\t_\t_\n\
2\tbark\tbark\tVERB\tVBP\t_\t0\troot\t_\t_\n\
";

#[test]
fn a_newdoc_split_creates_one_document_per_source_text() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("multi.db");
    let src = dir.path().join("multi.conllu");
    std::fs::write(&src, MULTIDOC).unwrap();

    let mut store = Store::open(&db).unwrap();
    let stats =
        ingest::import_path(&mut store, &ingest::ImportOptions::new("multi"), &src).unwrap();
    assert_eq!(stats.documents, 2, "one document per newdoc");
    assert_eq!(stats.sentences, 2);

    let docs = slovo_core::document::documents(&store, 10).unwrap();
    assert_eq!(docs.len(), 2);
    assert_eq!(docs[0].title.as_deref(), Some("doc-a"));
    assert_eq!(docs[1].title.as_deref(), Some("doc-b"));
    assert_eq!(docs[0].sentences, 1);
    assert_eq!(docs[0].tokens, 3);

    // The whole first document comes back by id.
    let doc = slovo_core::document::document(&store, docs[0].id).unwrap();
    assert_eq!(doc.sentences.len(), 1);
    assert_eq!(doc.sentences[0].sent_id.as_deref(), Some("doc-a-1"));
    assert_eq!(doc.sentences[0].text.as_deref(), Some("The cat sat."));
    assert!(doc.external_id.as_deref().unwrap().ends_with("#doc-a"));

    // Re-importing the same file is still a no-op.
    let again =
        ingest::import_path(&mut store, &ingest::ImportOptions::new("multi"), &src).unwrap();
    assert_eq!(again.documents, 0);
    assert_eq!(again.skipped, 1);
    assert_eq!(store.token_count().unwrap(), 5);
}

#[test]
fn a_missing_document_is_reported() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("sample.db");
    let store = Store::open(&db).unwrap();
    let error = slovo_core::document::document(&store, 999)
        .unwrap_err()
        .to_string();
    assert!(error.contains("999"), "{error}");
}
