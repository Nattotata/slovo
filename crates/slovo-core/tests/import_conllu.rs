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
