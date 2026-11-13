mod render;

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand, ValueEnum};

use slovo_core::query::By;
use slovo_core::{ingest, query_lang, search};
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
    },

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
        } => cmd_import(
            &cli,
            path,
            language.clone(),
            title.clone(),
            description.clone(),
        ),
        Command::Datasets => cmd_datasets(&cli),
        Command::Info => cmd_info(&cli),
        Command::Freq { by } => cmd_freq(&cli, (*by).into()),
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
        } => cmd_query(&cli, pattern, *explain, *case_sensitive, *within, *min_freq),
        Command::Pattern(args) => {
            let pattern = args.join(" ");
            cmd_query(&cli, &pattern, false, false, 5, 1)
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
    };
    let report = Service::new().query(&name, &opts)?;

    match cli.format {
        Format::Markdown => print!("{}", render::search_markdown(&report)),
        Format::Tsv => print!("{}", render::search_tsv(&report)),
        Format::Json => println!("{}", render::search_json(&report)),
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
) -> Result<()> {
    let name = dataset_name(cli)?;
    let service = Service::new();
    let db_path = service.registry().target_path(&name);

    let opts = ingest::ImportOptions {
        dataset: name.clone(),
        language,
        description,
        title,
        source: None,
        license: None,
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

fn cmd_freq(cli: &Cli, by: By) -> Result<()> {
    let name = dataset_name(cli)?;
    let report = Service::new().frequency(&name, by, cli.limit)?;

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
