//! The service layer is what the CLI and the HTTP API both call, so these
//! tests cover the operations both depend on.

use slovo_core::ingest::ImportOptions;
use slovo_core::query::By;
use slovo_core::search::SearchOptions;
use slovo_service::{AddOptions, Service};

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

struct Fixture {
    _dir: tempfile::TempDir,
    service: Service,
    source: std::path::PathBuf,
}

fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("t.conllu");
    std::fs::write(&source, FIXTURE).unwrap();

    let service = Service::at(dir.path().join("data"));
    Fixture {
        _dir: dir,
        service,
        source,
    }
}

fn import_options() -> ImportOptions {
    let mut options = ImportOptions::new("t");
    options.language = Some("en".to_string());
    options
}

#[test]
fn list_info_frequency_and_query_work() {
    let fx = fixture();
    let stats = fx.service.import(&import_options(), &fx.source).unwrap();
    assert_eq!(stats.documents, 1);
    assert_eq!(stats.tokens, 10);

    let datasets = fx.service.list().unwrap();
    assert_eq!(datasets.len(), 1);
    assert_eq!(datasets[0].name, "t");
    assert_eq!(datasets[0].tokens, 10);

    let info = fx.service.info("t").unwrap();
    assert_eq!(info.language.as_deref(), Some("en"));
    assert_eq!(info.tokens, 10);
    assert_eq!(info.sentences, 2);

    let freq = fx.service.frequency("t", By::Lemma, 5).unwrap();
    assert_eq!(freq.dataset, "t");
    assert_eq!(freq.by, "lemma");
    assert!(!freq.results.is_empty());

    let found = fx
        .service
        .query(
            "t",
            &SearchOptions {
                pattern: "take *".to_string(),
                limit: 10,
                case_sensitive: false,
                within: 5,
                min_freq: 1,
                examples: 2,
                context: 3,
            },
        )
        .unwrap();
    assert!(found.matches > 0);
    assert!(found.results.iter().any(|row| row.text == "take up with"));
    // Examples came through the service too.
    let row = found
        .results
        .iter()
        .find(|row| row.text == "take up with")
        .unwrap();
    assert_eq!(row.examples.len(), 1);
    assert_eq!(row.examples[0].match_text, "take up with");

    let kwic = fx
        .service
        .kwic(
            "t",
            &slovo_core::concordance::KwicOptions {
                pattern: "take *".to_string(),
                limit: 10,
                case_sensitive: false,
                within: 5,
                context: 3,
            },
        )
        .unwrap();
    assert!(kwic.matches > 0);
    assert!(kwic
        .results
        .iter()
        .any(|line| line.match_text == "take up with"));
    // Provenance: the document the hit belongs to.
    assert!(kwic.results.iter().all(|line| line.document_id > 0));

    // The documents listing resolves, and each one opens by id.
    let docs = fx.service.documents("t", 10).unwrap();
    assert_eq!(docs.len(), 1);
    let doc = fx.service.document("t", docs[0].document_id).unwrap();
    assert_eq!(doc.sentences.len(), 2);
    assert_eq!(doc.sentences[0].text.as_deref(), Some("They take off."));
}

#[test]
fn importing_twice_does_not_double_the_corpus() {
    let fx = fixture();
    fx.service.import(&import_options(), &fx.source).unwrap();
    let again = fx.service.import(&import_options(), &fx.source).unwrap();

    assert_eq!(again.documents, 0);
    assert_eq!(again.skipped, 1);
    assert_eq!(fx.service.info("t").unwrap().tokens, 10);
}

#[test]
fn a_missing_dataset_is_reported() {
    let fx = fixture();
    let error = fx.service.info("nope").unwrap_err().to_string();
    assert!(error.contains("nope"), "{error}");
}

#[test]
fn add_reports_a_fetch_failure_rather_than_panicking() {
    let fx = fixture();
    let error = fx
        .service
        .add(
            "x",
            &AddOptions {
                source: "ud/UD_English-EWT".to_string(),
                catalog: Some("/nonexistent/catalog.json".to_string()),
                ..AddOptions::default()
            },
        )
        .unwrap_err()
        .to_string();

    assert!(error.contains("could not fetch"), "{error}");
}
