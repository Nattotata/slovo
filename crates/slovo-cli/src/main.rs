mod render;

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand, ValueEnum};

use slovo_core::query::{By, Rank};
use slovo_core::{concordance, ingest, query_lang, search};
use slovo_service::{AddOptions, Service};

#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
enum Format {
    Markdown,
    Json,
    Tsv,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum ByArg {
    Word,
    Form,
    Lemma,
    Upos,
    Xpos,
}

impl From<ByArg> for By {
    fn from(arg: ByArg) -> Self {
        match arg {
            ByArg::Word => By::Word,
            ByArg::Form => By::Form,
            ByArg::Lemma => By::Lemma,
            ByArg::Upos => By::Upos,
            ByArg::Xpos => By::Xpos,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
enum RankArg {
    Frequency,
    Weight,
}

impl From<RankArg> for Rank {
    fn from(arg: RankArg) -> Self {
        match arg {
            RankArg::Frequency => Rank::Frequency,
            RankArg::Weight => Rank::Weight,
        }
    }
}

#[derive(Parser)]
#[command(name = "slovo", version, about = "A local corpus workstation")]
struct Cli {
    /// Dataset name, or a path to a `.db` file
    #[arg(short = 'd', long, global = true, value_name = "NAME")]
    dataset: Option<String>,

    /// Maximum number of rows
    #[arg(
        short = 'n',
        long,
        global = true,
        default_value_t = 20,
        value_name = "N"
    )]
    limit: usize,

    /// Output format
    #[arg(long, global = true, value_enum, default_value_t = Format::Markdown)]
    format: Format,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Build a dataset from CoNLL-U files (a file, or a directory of them)
    Import {
        /// File or directory to read
        path: PathBuf,
        /// ISO language code, e.g. `en`
        #[arg(long)]
        language: Option<String>,
        /// Document title (defaults to the file name)
        #[arg(long)]
        title: Option<String>,
        /// Free-form description stored with the dataset
        #[arg(long)]
        description: Option<String>,
        /// Provenance: where the text came from (a URL or a path)
        #[arg(long)]
        source: Option<String>,
        /// Provenance: the source's license, e.g. `CC BY-SA 4.0`
        #[arg(long)]
        license: Option<String>,
    },
    /// Turn raw text into CoNLL-U (tokenize, split sentences, optional tagger)
    Annotate {
        /// Raw text file to read
        path: PathBuf,
        /// Where to write CoNLL-U (defaults to stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Document id prefix for `newdoc`/`sent_id`, e.g. `lotr`
        #[arg(long)]
        id: Option<String>,
        /// An external tagger command that prints CoNLL-U on stdout
        #[arg(long, value_name = "CMD")]
        tagger: Option<String>,
        /// A TSV of form→lemma corrections, applied non-destructively
        #[arg(long, value_name = "FILE")]
        lexicon: Option<PathBuf>,
        /// Read the input as markdown (auto-detected for `.md`)
        #[arg(long)]
        markdown: bool,
        /// Weight scheme for markdown structure: a small `key = value` file
        #[arg(long, value_name = "FILE")]
        weights: Option<PathBuf>,
        /// Keep the parser's head/deprel columns
        #[arg(long)]
        parse: bool,
        /// Split at chapter headings, one document per chapter
        #[arg(long)]
        chapters: bool,
        /// Treat the whole file as one document
        #[arg(long, conflicts_with = "chapters")]
        single: bool,
    },
    /// List installed datasets
    Datasets,
    /// Show details about a dataset
    Info,
    /// Frequency list of words, lemmas or tags
    Freq {
        /// Column to count
        #[arg(long, value_enum, default_value_t = ByArg::Word)]
        by: ByArg,
        /// Order by occurrence count, or by summed structural weight
        #[arg(long, value_enum, default_value_t = RankArg::Frequency)]
        rank: RankArg,
    },
    /// List the datasets in a catalog
    Sources {
        /// Catalog path or URL
        #[arg(long)]
        catalog: Option<String>,
    },
    /// Download a dataset into a directory
    Fetch {
        /// Dataset id, e.g. `ud/UD_English-EWT`
        id: String,
        /// Catalog path or URL
        #[arg(long)]
        catalog: Option<String>,
        /// Directory to download into (defaults to the Slovo data dir)
        #[arg(long)]
        to: Option<PathBuf>,
        /// Re-download even if files are already there
        #[arg(long)]
        force: bool,
    },
    /// Download a dataset and import it into a new database
    Add {
        /// Dataset id, e.g. `ud/UD_English-EWT`
        id: String,
        /// Catalog path or URL
        #[arg(long)]
        catalog: Option<String>,
        /// Directory to download into (defaults to the Slovo data dir)
        #[arg(long)]
        to: Option<PathBuf>,
        /// Re-download even if files are already there
        #[arg(long)]
        force: bool,
        /// ISO language code, e.g. `en`
        #[arg(long)]
        language: Option<String>,
        /// Document title (defaults to the file name)
        #[arg(long)]
        title: Option<String>,
        /// Free-form description stored with the dataset
        #[arg(long)]
        description: Option<String>,
    },

    /// Search for a token pattern (CQP subset)
    Query {
        /// Pattern, e.g. `take *` or `[lemma="take"] []+`
        pattern: String,
        /// Print the canonical CQP form and exit
        #[arg(long)]
        explain: bool,
        /// Match on the surface form, not the lower-cased form
        #[arg(long)]
        case_sensitive: bool,
        /// Maximum number of tokens a match may span
        #[arg(long, value_name = "N", default_value_t = 5)]
        within: u32,
        /// Drop rows occurring fewer than this many times
        #[arg(long, value_name = "N", default_value_t = 1)]
        min_freq: u64,
        /// Attach up to this many concordance examples to each row
        #[arg(long, value_name = "N", default_value_t = 0)]
        examples: usize,
    },

    /// Concordance lines (KWIC) for a token pattern
    Kwic {
        /// Pattern, e.g. `take *` or `[lemma="take"] []+`
        pattern: String,
        /// Match on the surface form, not the lower-cased form
        #[arg(long)]
        case_sensitive: bool,
        /// Maximum number of tokens a match may span
        #[arg(long, value_name = "N", default_value_t = 5)]
        within: u32,
        /// Tokens of context on each side of the match
        #[arg(long, value_name = "N", default_value_t = 5)]
        context: usize,
    },

    /// Print a source document, by id
    Doc {
        /// Document id, as shown in a concordance line
        id: i64,
    },

    /// List the documents in a dataset
    Docs,

    /// Shorthand: a bare pattern is a query
    #[command(external_subcommand)]
    Pattern(Vec<String>),

    /// Serve the HTTP API (for the web UI and the desktop shell)
    Serve {
        /// Address to bind
        #[arg(long, default_value = slovo_server::DEFAULT_HOST)]
        host: String,
        /// Port to bind
        #[arg(long, default_value_t = slovo_server::DEFAULT_PORT)]
        port: u16,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match &cli.command {
        Command::Import {
            path,
            language,
            title,
            description,
            source,
            license,
        } => cmd_import(
            &cli,
            path,
            language.clone(),
            title.clone(),
            description.clone(),
            source.clone(),
            license.clone(),
        ),
        Command::Annotate {
            path,
            output,
            id,
            tagger,
            lexicon,
            markdown,
            weights,
            parse,
            chapters,
            single,
        } => cmd_annotate(
            path,
            output.as_deref(),
            id.clone(),
            tagger.clone(),
            lexicon.as_deref(),
            *markdown,
            weights.as_deref(),
            *parse,
            *chapters,
            *single,
        ),
        Command::Datasets => cmd_datasets(&cli),
        Command::Info => cmd_info(&cli),
        Command::Freq { by, rank } => cmd_freq(&cli, (*by).into(), (*rank).into()),
        Command::Sources { catalog } => cmd_sources(&cli, catalog.clone()),
        Command::Fetch {
            id,
            catalog,
            to,
            force,
        } => cmd_fetch(&cli, id, catalog.clone(), to.clone(), *force),
        Command::Add {
            id,
            catalog,
            to,
            force,
            language,
            title,
            description,
        } => cmd_add(
            &cli,
            id,
            catalog.clone(),
            to.clone(),
            *force,
            language.clone(),
            title.clone(),
            description.clone(),
        ),
        Command::Query {
            pattern,
            explain,
            case_sensitive,
            within,
            min_freq,
            examples,
        } => cmd_query(
            &cli,
            pattern,
            *explain,
            *case_sensitive,
            *within,
            *min_freq,
            *examples,
        ),
        Command::Kwic {
            pattern,
            case_sensitive,
            within,
            context,
        } => cmd_kwic(&cli, pattern, *case_sensitive, *within, *context),
        Command::Doc { id } => cmd_doc(&cli, *id),
        Command::Docs => cmd_docs(&cli),
        Command::Pattern(args) => {
            let pattern = args.join(" ");
            cmd_query(&cli, &pattern, false, false, 5, 1, 0)
        }
        Command::Serve { host, port } => {
            slovo_server::serve(host, *port).map_err(|error| anyhow::anyhow!("{error}"))?;
            Ok(())
        }
    }
}

fn cmd_query(
    cli: &Cli,
    pattern: &str,
    explain: bool,
    case_sensitive: bool,
    within: u32,
    min_freq: u64,
    examples: usize,
) -> Result<()> {
    if explain {
        let query = query_lang::parse(pattern)?;
        let bound = query.within.unwrap_or(within);
        println!("{}", query_lang::canonical(&query, bound));
        return Ok(());
    }

    let name = dataset_name(cli)?;
    let opts = search::SearchOptions {
        pattern: pattern.to_string(),
        limit: cli.limit,
        case_sensitive,
        within,
        min_freq,
        examples,
        context: slovo_core::KWIC_CONTEXT,
    };
    let report = Service::new().query(&name, &opts)?;

    match cli.format {
        Format::Markdown => print!("{}", render::search_markdown(&report)),
        Format::Tsv => print!("{}", render::search_tsv(&report)),
        Format::Json => println!("{}", render::search_json(&report)),
    }
    Ok(())
}

fn cmd_kwic(
    cli: &Cli,
    pattern: &str,
    case_sensitive: bool,
    within: u32,
    context: usize,
) -> Result<()> {
    let name = dataset_name(cli)?;
    let opts = concordance::KwicOptions {
        pattern: pattern.to_string(),
        limit: cli.limit,
        case_sensitive,
        within,
        context,
    };
    let report = Service::new().kwic(&name, &opts)?;

    match cli.format {
        Format::Markdown => print!("{}", render::kwic_markdown(&report)),
        Format::Tsv => print!("{}", render::kwic_tsv(&report)),
        Format::Json => println!("{}", render::kwic_json(&report)),
    }
    Ok(())
}

fn cmd_doc(cli: &Cli, id: i64) -> Result<()> {
    let name = dataset_name(cli)?;
    let doc = Service::new().document(&name, id)?;

    match cli.format {
        Format::Markdown => print!("{}", render::doc_markdown(&doc)),
        Format::Tsv => print!("{}", render::doc_tsv(&doc)),
        Format::Json => println!("{}", serde_json::to_string_pretty(&doc)?),
    }
    Ok(())
}

fn cmd_docs(cli: &Cli) -> Result<()> {
    let name = dataset_name(cli)?;
    let docs = Service::new().documents(&name, cli.limit)?;

    match cli.format {
        Format::Markdown => print!("{}", render::docs_markdown(&name, &docs)),
        Format::Tsv => print!("{}", render::docs_tsv(&docs)),
        Format::Json => println!("{}", serde_json::to_string_pretty(&docs)?),
    }
    Ok(())
}

fn dataset_name(cli: &Cli) -> Result<String> {
    match &cli.dataset {
        Some(name) => Ok(name.clone()),
        None => bail!("no dataset selected; pass `-d NAME`"),
    }
}

fn cmd_import(
    cli: &Cli,
    path: &Path,
    language: Option<String>,
    title: Option<String>,
    description: Option<String>,
    source: Option<String>,
    license: Option<String>,
) -> Result<()> {
    let name = dataset_name(cli)?;
    let service = Service::new();
    let db_path = service.registry().target_path(&name);

    let opts = ingest::ImportOptions {
        dataset: name.clone(),
        language,
        description,
        title,
        source,
        license,
        version: None,
    };
    let stats = service
        .import(&opts, path)
        .with_context(|| format!("importing `{}`", path.display()))?;

    if cli.format == Format::Json {
        let payload = serde_json::json!({
            "dataset": name,
            "database": db_path.display().to_string(),
            "documents": stats.documents,
            "sentences": stats.sentences,
            "tokens": stats.tokens,
            "skipped": stats.skipped,
        });
        println!("{}", serde_json::to_string_pretty(&payload)?);
    } else {
        println!(
            "Imported {} document(s), {} sentence(s), {} token(s) into `{}`.",
            render::thousands(stats.documents),
            render::thousands(stats.sentences),
            render::thousands(stats.tokens),
            name
        );
        if stats.skipped > 0 {
            println!(
                "Skipped {} already-imported document(s).",
                render::thousands(stats.skipped)
            );
        }
        println!("Database: {}", db_path.display());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn cmd_annotate(
    path: &Path,
    output: Option<&Path>,
    id: Option<String>,
    tagger: Option<String>,
    lexicon: Option<&Path>,
    markdown: bool,
    weights: Option<&Path>,
    parse: bool,
    chapters: bool,
    single: bool,
) -> Result<()> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("reading `{}`", path.display()))?;

    // The id prefix names the documents; default to the file stem.
    let id_prefix = id.unwrap_or_else(|| {
        path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("doc")
            .to_string()
    });

    // Markdown is opt-in, or detected from the extension.
    let is_markdown = markdown
        || path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("md") || e.eq_ignore_ascii_case("markdown"));

    let scheme = match weights {
        Some(path) => load_weight_scheme(path)?,
        None => slovo_annotate::markdown::Scheme::default(),
    };

    // Markdown: one document per file, one segment per heading section.
    // Plain text: chapter splitting is opt-in, since plain text has no
    // guaranteed structure; `--single` forces one document regardless.
    let segments = if is_markdown {
        slovo_annotate::markdown::parse(&text, &id_prefix, &scheme).segments
    } else if chapters && !single {
        slovo_annotate::segment::split(
            &text,
            &slovo_annotate::segment::SplitOptions::new(id_prefix.clone()),
        )
    } else {
        vec![slovo_annotate::Segment::standalone(
            format!("{id_prefix}-1"),
            None,
            text.clone(),
        )]
    };

    if segments.is_empty() {
        bail!("no text found in `{}`", path.display());
    }

    let mut options = slovo_annotate::Options::new();
    options.parse = parse;
    if let Some(command) = tagger {
        options.tagger = Some(slovo_annotate::tagger::Tagger::new(command));
    }
    if let Some(path) = lexicon {
        let lex = slovo_annotate::lexicon::Lexicon::read(path)
            .with_context(|| format!("reading lexicon `{}`", path.display()))?;
        if lex.is_empty() {
            eprintln!("warning: lexicon `{}` has no entries", path.display());
        }
        options.lexicon = Some(lex);
    }

    let sentences = slovo_annotate::annotate(&segments, &options)?;
    let conllu = slovo_annotate::conllu::write(&sentences);

    // Distinct documents the run produced (segments may share one).
    let mut docs: Vec<&str> = segments.iter().map(|s| s.document()).collect();
    docs.sort_unstable();
    docs.dedup();

    match output {
        Some(path) => {
            std::fs::write(path, &conllu)
                .with_context(|| format!("writing `{}`", path.display()))?;
            let tokens: usize = sentences.iter().map(|s| s.tokens.len()).sum();
            eprintln!(
                "Annotated {} document(s), {} sentence(s), {} token(s) → {}",
                docs.len(),
                sentences.len(),
                tokens,
                path.display()
            );
        }
        None => print!("{conllu}"),
    }
    Ok(())
}

/// Load a weight scheme from a small `key = value` file.
///
/// Recognised keys: `h1`…`h6`, `body`. An unknown key is an error, so a typo
/// does not silently fall back to a default.
fn load_weight_scheme(path: &Path) -> Result<slovo_annotate::markdown::Scheme> {
    use slovo_annotate::{markdown::Scheme, Weight};

    let text = std::fs::read_to_string(path)
        .with_context(|| format!("reading weight scheme `{}`", path.display()))?;

    let mut scheme = Scheme::default();
    for (n, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            bail!(
                "{}:{}: expected `key = value`, got `{line}`",
                path.display(),
                n + 1
            );
        };
        let key = key.trim().to_lowercase();
        let value: f64 = value.trim().parse().with_context(|| {
            format!(
                "{}:{}: `{}` is not a number",
                path.display(),
                n + 1,
                value.trim()
            )
        })?;
        let weight = Weight::new(value);
        match key.as_str() {
            "h1" => scheme.headings[0] = weight,
            "h2" => scheme.headings[1] = weight,
            "h3" => scheme.headings[2] = weight,
            "h4" => scheme.headings[3] = weight,
            "h5" => scheme.headings[4] = weight,
            "h6" => scheme.headings[5] = weight,
            "body" => scheme.body = weight,
            other => bail!(
                "{}:{}: unknown weight key `{other}`; expected h1..h6 or body",
                path.display(),
                n + 1
            ),
        }
    }
    Ok(scheme)
}

fn cmd_datasets(cli: &Cli) -> Result<()> {
    let rows = Service::new().list()?;

    match cli.format {
        Format::Json => {
            let list: Vec<_> = rows
                .iter()
                .map(|row| serde_json::json!({ "name": row.name, "tokens": row.tokens }))
                .collect();
            println!("{}", serde_json::to_string_pretty(&list)?);
        }
        Format::Tsv => {
            println!("dataset\ttokens");
            for row in &rows {
                println!("{}\t{}", row.name, row.tokens);
            }
        }
        Format::Markdown => {
            if rows.is_empty() {
                println!(
                    "No datasets installed. Build one with:\n\n```\nslovo -d mycorpus import <path>\n```"
                );
            } else {
                println!("## Datasets\n");
                println!("| dataset | tokens |");
                println!("|:--------|-------:|");
                for row in &rows {
                    println!("| {} | {} |", row.name, render::thousands(row.tokens));
                }
            }
        }
    }
    Ok(())
}

fn cmd_info(cli: &Cli) -> Result<()> {
    let name = dataset_name(cli)?;
    let service = Service::new();
    let path = service.registry().resolve(&name)?;
    let info = service.info(&name)?;

    match cli.format {
        Format::Json => {
            let payload = serde_json::json!({
                "name": info.name,
                "language": info.language,
                "description": info.description,
                "tokens": info.tokens,
                "documents": info.documents,
                "sentences": info.sentences,
                "created_at": info.created_at,
                "database": path.display().to_string(),
            });
            println!("{}", serde_json::to_string_pretty(&payload)?);
        }
        Format::Tsv => {
            println!("key\tvalue");
            println!("name\t{}", info.name);
            if let Some(language) = &info.language {
                println!("language\t{language}");
            }
            if let Some(description) = &info.description {
                println!("description\t{description}");
            }
            println!("tokens\t{}", info.tokens);
            println!("documents\t{}", info.documents);
            println!("sentences\t{}", info.sentences);
            println!("created_at\t{}", info.created_at);
            println!("database\t{}", path.display());
        }
        Format::Markdown => {
            println!("## `{}`\n", info.name);
            if let Some(language) = &info.language {
                println!("- **language:** {language}");
            }
            if let Some(description) = &info.description {
                println!("- **description:** {description}");
            }
            println!("- **tokens:** {}", render::thousands(info.tokens));
            println!("- **documents:** {}", render::thousands(info.documents));
            println!("- **sentences:** {}", render::thousands(info.sentences));
            println!("- **created:** {}", info.created_at);
            println!("- **database:** `{}`", path.display());
        }
    }
    Ok(())
}

fn cmd_freq(cli: &Cli, by: By, rank: Rank) -> Result<()> {
    let name = dataset_name(cli)?;
    let report = Service::new().frequency_ranked(&name, by, cli.limit, rank)?;

    match cli.format {
        Format::Markdown => print!("{}", render::freq_markdown(&report)),
        Format::Tsv => print!("{}", render::freq_tsv(&report)),
        Format::Json => println!("{}", render::freq_json(&report)),
    }
    Ok(())
}

fn cmd_sources(cli: &Cli, catalog: Option<String>) -> Result<()> {
    let catalog = slovo_fetch::load_catalog(catalog.as_deref())
        .with_context(|| "loading the dataset catalog")?;

    match cli.format {
        Format::Json => println!("{}", serde_json::to_string_pretty(&catalog)?),
        Format::Tsv => {
            println!("id\tname\tlanguage\tlicense\tversion\tfiles");
            for dataset in &catalog.datasets {
                println!(
                    "{}\t{}\t{}\t{}\t{}\t{}",
                    dataset.id,
                    dataset.name,
                    dataset.language.as_deref().unwrap_or(""),
                    dataset.license.as_deref().unwrap_or(""),
                    dataset.version.as_deref().unwrap_or(""),
                    dataset.files.len()
                );
            }
        }
        Format::Markdown => {
            println!("## Catalog\n");
            println!("| id | name | language | license | version | files |");
            println!("|:---|:-----|:---------|:--------|:--------|------:|");
            for dataset in &catalog.datasets {
                println!(
                    "| `{}` | {} | {} | {} | {} | {} |",
                    dataset.id,
                    dataset.name,
                    dataset.language.as_deref().unwrap_or(""),
                    dataset.license.as_deref().unwrap_or(""),
                    dataset.version.as_deref().unwrap_or(""),
                    dataset.files.len()
                );
            }
        }
    }
    Ok(())
}

fn cmd_fetch(
    cli: &Cli,
    id: &str,
    catalog: Option<String>,
    to: Option<PathBuf>,
    force: bool,
) -> Result<()> {
    let to = to.unwrap_or_else(|| Service::new().registry().download_dir(id));
    let opts = slovo_fetch::FetchOptions {
        catalog,
        id: id.to_string(),
        to: to.clone(),
        force,
    };
    let manifest = slovo_fetch::fetch(&opts).with_context(|| format!("fetching `{id}`"))?;

    match cli.format {
        Format::Json => {
            let payload = serde_json::json!({
                "id": id,
                "name": manifest.name,
                "language": manifest.language,
                "license": manifest.license,
                "version": manifest.version,
                "source": manifest.source,
                "directory": to.display().to_string(),
                "files": manifest.files.iter().map(|f| f.name.clone()).collect::<Vec<_>>(),
            });
            println!("{}", serde_json::to_string_pretty(&payload)?);
        }
        Format::Tsv => {
            println!("key\tvalue");
            println!("id\t{id}");
            println!("name\t{}", manifest.name);
            if let Some(language) = &manifest.language {
                println!("language\t{language}");
            }
            if let Some(license) = &manifest.license {
                println!("license\t{license}");
            }
            if let Some(version) = &manifest.version {
                println!("version\t{version}");
            }
            println!("directory\t{}", to.display());
            println!("files\t{}", manifest.files.len());
        }
        Format::Markdown => {
            println!("## Fetched `{}`\n", manifest.name);
            println!("- **id:** `{id}`");
            if let Some(language) = &manifest.language {
                println!("- **language:** {language}");
            }
            if let Some(license) = &manifest.license {
                println!("- **license:** {license}");
            }
            if let Some(version) = &manifest.version {
                println!("- **version:** {version}");
            }
            println!("- **directory:** `{}`", to.display());
            println!("- **files:** {}", manifest.files.len());
            println!(
                "\nImport it with:\n\n```\nslovo -d <name> import {}\n```",
                to.display()
            );
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn cmd_add(
    cli: &Cli,
    id: &str,
    catalog: Option<String>,
    to: Option<PathBuf>,
    force: bool,
    language: Option<String>,
    title: Option<String>,
    description: Option<String>,
) -> Result<()> {
    let name = dataset_name(cli)?;
    let service = Service::new();
    let db_path = service.registry().target_path(&name);

    let outcome = service
        .add(
            &name,
            &AddOptions {
                source: id.to_string(),
                catalog,
                to,
                force,
                language,
                title,
                description,
            },
        )
        .with_context(|| format!("adding `{id}`"))?;

    let stats = outcome.stats;

    if cli.format == Format::Json {
        let payload = serde_json::json!({
            "dataset": name,
            "database": db_path.display().to_string(),
            "source": id,
            "documents": stats.documents,
            "sentences": stats.sentences,
            "tokens": stats.tokens,
            "skipped": stats.skipped,
            "directory": outcome.directory.display().to_string(),
        });
        println!("{}", serde_json::to_string_pretty(&payload)?);
    } else {
        println!(
            "Imported {} document(s), {} sentence(s), {} token(s) from `{}` into `{}`.",
            render::thousands(stats.documents),
            render::thousands(stats.sentences),
            render::thousands(stats.tokens),
            id,
            name
        );
        if stats.skipped > 0 {
            println!(
                "Skipped {} already-imported document(s).",
                render::thousands(stats.skipped)
            );
        }
        println!("Database: {}", db_path.display());
    }
    Ok(())
}
