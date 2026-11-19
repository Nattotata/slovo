# Slovo

A local, offline corpus workstation.
Bring your own text, build a searchable corpus, then explore it from a CLI and a GUI.

The engine is Rust.
The GUI is Tauri + SvelteKit.
The CLI is the reference frontend.

**Status:** early.
The CLI imports Universal Dependencies treebanks, downloads them from a catalog, prints frequency lists, runs CQP-subset pattern queries, and shows concordance lines with provenance.
The GUI is still the Tauri scaffold.
Design notes and the roadmap live in [`docs/DESIGN.md`](docs/DESIGN.md).
Linguistic and tooling terms are defined in [`docs/GLOSSARY.md`](docs/GLOSSARY.md).

## Quick start

Requires a Rust toolchain.
The flake provides one: `nix develop`.

```console
$ just build
$ slovo sources
## Catalog

| id | name | language | license | version | files |
|:---|:-----|:---------|:--------|:--------|------:|
| `ud/UD_English-EWT` | UD English EWT | en | CC BY-SA 4.0 | r2.19 | 3 |
| `ud/UD_English-GUM` | UD English GUM | en | CC BY-NC-SA 4.0 | r2.19 | 3 |
| `ud/UD_Slovak-SNK` | UD Slovak SNK | sk | CC BY-SA 4.0 | r2.19 | 3 |
| `ud/UD_Czech-CAC` | UD Czech CAC | cs | CC BY-SA 4.0 | r2.19 | 3 |

$ slovo -d ewt add ud/UD_English-EWT
Imported 3 document(s), 16,622 sentence(s), 254,820 token(s) from `ud/UD_English-EWT` into `ewt`.

$ slovo -d ewt freq -n 5
## Frequency — `ewt` (by word)

_254,820 tokens_

| # | value | freq | per million |
|--:|:------|-----:|------------:|
| 1 | the | 11,030 | 43285.5 |
| 2 | . | 10,899 | 42771.4 |
| 3 | , | 8,651 | 33949.5 |
| 4 | to | 6,294 | 24699.8 |
| 5 | and | 6,108 | 23969.9 |

$ slovo -d ewt 'take *'
## `take *` — dataset `ewt`

`take *` → `[word="take"] []+ within 5`

_254,820 tokens · 960 matches · 711 distinct_

| # | match | freq | per million |
|--:|:------|-----:|------------:|
| 1 | take a | 30 | 117.7 |
| 2 | take care | 27 | 106.0 |
| 3 | take the | 26 | 102.0 |
```

`just build` puts the binary at `target/debug/slovo`.
Install it on `PATH` with `cargo install --path crates/slovo-cli`.

The catalog is a local file, `catalog.json`.
Point `--catalog` or `SLOVO_CATALOG` at a different one to hot-swap it.
`slovo fetch <id>` downloads without importing; `slovo import <path>` imports a directory you already have.

## CLI

| Command | What it does |
|---|---|
| `slovo sources` | List the catalog |
| `slovo fetch <id>` | Download a dataset into a directory |
| `slovo add <id> -d NAME` | Download a dataset and import it |
| `slovo import <path> -d NAME` | Build a dataset from a `.conllu` file, or a directory of them |
| `slovo annotate <path>` | Turn raw `.txt` into CoNLL-U (tokenize; optional tagger) |
| `slovo datasets` | List installed datasets |
| `slovo info -d NAME` | Show dataset metadata and counts |
| `slovo freq -d NAME` | Frequency list of `--by word\|form\|lemma\|upos\|xpos`; `--rank frequency\|weight` |
| `slovo query -d NAME <pattern>` | Pattern search (CQP subset) |
| `slovo kwic -d NAME <pattern>` | Concordance lines, with source provenance |
| `slovo docs -d NAME` | List a dataset's source documents |
| `slovo doc -d NAME <id>` | Print a whole source document |
| `slovo -d NAME <pattern>` | Shorthand for `query` |
| `slovo serve` | Serve the HTTP API for the UI |

Global options: `-d/--dataset`, `-n/--limit` (default 20), `--format markdown|json|tsv` (default `markdown`).
`--catalog` and `SLOVO_CATALOG` point at a different catalog.
Query options: `--explain`, `--case-sensitive`, `--within <N>`, `--min-freq <N>`, `--examples <N>`.
KWIC options: `--case-sensitive`, `--within <N>`, `--context <N>` (tokens on each side, default 5).
Import options: `--language`, `--title`, `--description`, `--source`, `--license`.
Annotate options: `--output`, `--id`, `--tagger <CMD>`, `--lexicon <FILE>`, `--markdown`, `--weights <FILE>`, `--chapters`, `--single`, `--parse`.

## Query language

A subset of **CQP**, the IMS Corpus Workbench query language, plus a few aliases.

| You write | Expands to | Meaning |
|---|---|---|
| `take` | `[word="take"]` | the word |
| `take off` | two nodes | the phrase |
| `_` | `[]` | any single token |
| `*` | `[]+` | one or more tokens |
| `NOUN` | `[upos="NOUN"]` | part-of-speech tag |
| `[lemma="take"]` | *(unchanged)* | any form of *take* |
| `[lemma_override="take"]` | *(unchanged)* | the lexicon's correction |
| `(off\|up)` | *(unchanged)* | either |

Quantifiers attach to their node (`[]+`, `[]?`, `[]{1,3}`), and `within N` caps the match.
`--explain` always shows the expansion, so nothing is hidden.

## Concordance

A cluster query lists *which* phrases occur; a concordance shows *where*, so you can read the sentence and judge the meaning yourself.

`slovo kwic` prints one line per occurrence, the match centred, with document provenance:

```console
$ slovo -d ewt kwic 'take up'
## `take up` — dataset `ewt`

`take up` → `[word="take"] [word="up"] within 5`

_254,820 tokens · 141 matches · showing 50_

… and I   take up   the guitar again   [EWT #4,1203]
… she will   take up   the offer        [EWT #9,88]
… they   take up   with strangers       [EWT #12,410]
```

`slovo query <pattern> --examples N` attaches the same lines under each row of a cluster list, so `take *` can show you that `take off` is about departure and `take up with` is about association, without a second command.

Every hit carries its document id, sentence id and (when the source has one) `# sent_id`, so a result traces back to the text it came from.
Each UD `# newdoc id` block becomes its own document, so the id points at one source text, not a whole file.
`slovo docs` lists them; `slovo doc -d NAME <id>` prints one in full:

```console
$ slovo -d ewt kwic 'take off'
… I   take off   my coat at the door   [doc 4 · weblog-…-0001]
… The plane will   take off   at noon   [doc 4 · weblog-…-0002]

$ slovo -d ewt doc 4
## weblog-example.com_post-1 — `doc 4`

weblog-example.com_post-1-0001: I take off my coat at the door.
weblog-example.com_post-1-0002: The plane will take off at noon.
```

When the dataset was imported with `--source <url>`, a hit shows that URL instead of the doc id; nothing is fabricated when there is none.

## CLI examples

Run from the repository root. `cargo run -q -p slovo-cli --` is the same as `just run`.

```console
# Catalog
cargo run -q -p slovo-cli -- sources
cargo run -q -p slovo-cli -- sources --format json
cargo run -q -p slovo-cli -- sources --catalog /tmp/catalog.json

# Download
cargo run -q -p slovo-cli -- fetch ud/UD_English-EWT
cargo run -q -p slovo-cli -- fetch ud/UD_English-EWT --to /tmp/ewt
cargo run -q -p slovo-cli -- fetch ud/UD_English-EWT --force

# Annotate raw text into CoNLL-U (then import it as usual)
cargo run -q -p slovo-cli -- annotate book.txt -o book.conllu
cargo run -q -p slovo-cli -- annotate book.txt --chapters --id lotr -o book.conllu
cargo run -q -p slovo-cli -- annotate book.txt --tagger 'udpipe --tokenize --tag --lemma --outfile - model.udpipe' -o book.conllu
cargo run -q -p slovo-cli -- annotate book.txt --lexicon archaic.tsv -o book.conllu

# Download + import
cargo run -q -p slovo-cli -- -d ewt add ud/UD_English-EWT
cargo run -q -p slovo-cli -- -d ewt add ud/UD_English-EWT --language en

# Import a local directory
cargo run -q -p slovo-cli -- -d ewt import /tmp/ewt
cargo run -q -p slovo-cli -- -d ewt import /tmp/ewt --language en --description "UD English EWT"
cargo run -q -p slovo-cli -- -d ewt import /tmp/ewt --source https://github.com/UniversalDependencies/UD_English-EWT --license "CC BY-SA 4.0"

# Installed datasets
cargo run -q -p slovo-cli -- datasets
cargo run -q -p slovo-cli -- -d ewt info
cargo run -q -p slovo-cli -- -d ewt info --format json

# Frequency
cargo run -q -p slovo-cli -- -d ewt freq
cargo run -q -p slovo-cli -- -d ewt freq -n 10
cargo run -q -p slovo-cli -- -d ewt freq --by lemma
cargo run -q -p slovo-cli -- -d ewt freq --by upos
cargo run -q -p slovo-cli -- -d ewt freq --rank weight
cargo run -q -p slovo-cli -- -d ewt freq --format tsv
cargo run -q -p slovo-cli -- -d ewt freq --format json

# Query
cargo run -q -p slovo-cli -- -d ewt 'take *'
cargo run -q -p slovo-cli -- -d ewt query 'take *'
cargo run -q -p slovo-cli -- -d ewt query 'take _ off'
cargo run -q -p slovo-cli -- -d ewt query '[lemma="take"] up'
cargo run -q -p slovo-cli -- -d ewt query 'take NOUN'
cargo run -q -p slovo-cli -- -d ewt query 'take (off|up|out)'
cargo run -q -p slovo-cli -- -d ewt query 'take * within 3'
cargo run -q -p slovo-cli -- -d ewt query 'take *' --min-freq 5
cargo run -q -p slovo-cli -- -d ewt query 'take *' --examples 3
cargo run -q -p slovo-cli -- -d ewt query --explain 'take *'
cargo run -q -p slovo-cli -- -d ewt query 'take *' --format json

# Concordance (KWIC): every occurrence in context, with provenance
cargo run -q -p slovo-cli -- -d ewt kwic 'take up'
cargo run -q -p slovo-cli -- -d ewt kwic '[lemma="take"] up' --context 8
cargo run -q -p slovo-cli -- -d ewt kwic 'take *' --format json

# Drill into the source: list documents, then print one
cargo run -q -p slovo-cli -- -d ewt docs
cargo run -q -p slovo-cli -- -d ewt doc 12

# Same thing through just (quotes the pattern for you)
just query ewt 'take *'
just q ewt 'take *'
```

Environment overrides:

```console
SLOVO_CATALOG=/tmp/catalog.json cargo run -q -p slovo-cli -- sources
SLOVO_DATA_DIR=/tmp/slovo cargo run -q -p slovo-cli -- -d ewt info
cargo run -q -p slovo-cli -- -d /tmp/slovo/datasets/ewt.db info
```

`--catalog` is a subcommand option; `-d`, `-n` and `--format` are global.

## Annotation

Slovo's engine imports CoNLL-U and never runs NLP.
Raw text (`.txt`) is turned into CoNLL-U by `slovo annotate`, which does the mechanical work in plain Rust and leaves the linguistic layers to an external tagger:

```
.txt ─► chapters ─► sentences ─► tokens ─► [tagger] ─► .conllu ─► slovo import
```

Segmentation, sentence splitting and tokenization run with nothing installed.
Lemma and POS come from `--tagger "<cmd>"`, a command that reads text on stdin and prints CoNLL-U on stdout — UDPipe, Stanza, or a script of your own.
Without a tagger, the columns it would fill are left as `_`, and the corpus is still fully searchable by form.

Taggers fail on archaic forms (`spake` lemmatized as `spake`, not `speak`).
`--lexicon <file>` patches those from a TSV, **without overwriting anything**: the tagger's lemma stays in `lemma`, and the correction goes to a separate `lemma_override` column, so you can search either layer.

Markdown is handled natively: frontmatter supplies the title, `#` headings become weighted sections, fenced code (chord charts, snippets) is excluded, and inline markers (`::G::`, `**Chords**`) are stripped — including mid-word, so `mag::B::gots` becomes `maggots`.

The weights are a *ranking* aid, kept strictly separate from frequency: `freq` still counts occurrences, and `freq --rank weight` orders by summed structural weight without changing any count. A word in a heading outranks one in body text; it does not "occur more".

```console
$ slovo annotate song.md -o song.conllu                 # markdown auto-detected by .md
$ slovo annotate song.md --weights scheme.conf -o song.conllu
$ slovo annotate book.txt --chapters --id lotr -o lotr.conllu
$ slovo annotate book.txt --tagger 'udpipe --tokenize --tag --lemma --outfile - model.udpipe' -o lotr.conllu
$ slovo annotate book.txt --lexicon archaic.tsv -o lotr.conllu
$ slovo -d lotr import lotr.conllu --language en
```

The weight scheme is a small data file, so tiers are configurable without code:

```
# weights.conf
h1 = 6
h2 = 5
h3 = 4
body = 1
```

The lexicon is TSV, one entry per line, `#` comments, an optional third column forcing a UPOS:

```
# archaic verb forms
spake	speak
wrought	work
hath	have	AUX
```

`--chapters` splits at chapter headings, one document per chapter, and carries the heading as the document title.
The tagger is authoritative only for the columns Slovo does not produce; tokenization stays Slovo's, so its boundaries are stable across taggers.

## Data

Datasets live at `$SLOVO_DATA_DIR/datasets/<name>.db`.
`SLOVO_DATA_DIR` defaults to the platform data directory (`~/.local/share/slovo` on Linux).
`-d` also accepts a path to a `.db` file.

A dataset is a directory of source files plus a `manifest.json`.
The catalog is `catalog.json`, a separate local file.
The engine does not know about the catalog or the downloader; it only imports the directory.

One SQLite file per dataset.
No server, no separate index.
The `token` table stores one row per corpus position, which is the CWB positional-attribute model.

Importing the same file twice is a no-op: files are matched by path, and already-imported ones are skipped.
Adding more files to an existing dataset appends.

## Development

The `justfile` wraps the common commands.

```console
$ just                      # list recipes
$ just check                # fmt + clippy + tests
$ just test
$ just run -d ewt freq -n 5
$ just query ewt 'take *'   # quoted pattern, no shell globbing
$ just kwic ewt 'take up'   # concordance lines
$ just docs ewt             # list source documents
$ just doc ewt 12           # print one document
$ just ewt ~/corpora/UD_English-EWT
$ just songbook-build       # build the markdown example dataset
$ just songbook kwic 'sleep'
$ just serve                # HTTP API on :8090
$ just gui-dev              # Tauri dev server
```

Layout:

```
crates/slovo-dataset/  manifest and catalog types
crates/slovo-core/     engine: schema, store, ingest, query, registry
crates/slovo-annotate/ raw text → CoNLL-U (tokenize; pluggable tagger)
crates/slovo-service/  the operations CLI and API both call
crates/slovo-fetch/    downloader: catalog, HTTP, checksums
crates/slovo-cli/      the `slovo` binary (CLI + `serve`)
crates/slovo-server/   HTTP API over the engine
examples/songbook/     a worked markdown example (see below)
src/                   SvelteKit frontend
src-tauri/             Tauri shell (kept out of the Cargo workspace for now)
catalog.json           the default catalog
docs/DESIGN.md         design notes and roadmap
docs/GLOSSARY.md       linguistic and tooling terms
```

## Example: a songbook

`examples/songbook/` is a worked example of the markdown → corpus pipeline: 35 song files with frontmatter, section headings, chord charts and inline chord markers.
It is deliberately awkward, which is what makes it a useful test of the annotator.

```console
$ just songbook-build        # annotate examples/songbook/*.md, import as `songs`
$ just songbook docs
$ just songbook kwic 'sleep'
$ just songbook freq -n 10 --rank weight
```

The dataset builds into `examples/songbook/.build/` (gitignored), so it stays out of your real data directory.
See [`examples/songbook/README.md`](examples/songbook/README.md) for what the annotator does with each construct.

## UI

The frontend is one static SvelteKit build (`@sveltejs/adapter-static`): a client-rendered SPA that talks to the API.

```console
$ npm run build        # writes build/
```

Serve `build/` with anything, or bundle it into the Tauri shell (`just gui-build`; `tauri.conf.json`'s `beforeBuildCommand` runs this for you).

No Node runtime is needed. Embedding the built UI in the `slovo` binary is the planned path; not wired up yet.

## API

`slovo serve` runs an HTTP API over the same engine the CLI uses.
It is what lets the browser UI and a desktop shell share one transport.

```console
$ slovo serve                              # http://127.0.0.1:8090
$ slovo serve --host 0.0.0.0 --port 8090
```

| Route | Returns |
|---|---|
| `GET /api/health` | `{"status":"ok","version":"..."}` |
| `GET /api/datasets` | installed datasets and their token counts |
| `GET /api/datasets/{name}` | dataset metadata |
| `GET /api/datasets/{name}/freq?by=lemma&limit=20` | frequency list |
| `GET /api/datasets/{name}/query?pattern=take%20*&limit=20` | pattern search |
| `GET /api/datasets/{name}/kwic?pattern=take%20up&limit=50` | concordance lines |
| `GET /api/datasets/{name}/documents` | list source documents |
| `GET /api/datasets/{name}/documents/{id}` | one source document |

`query` accepts `examples=N` to attach concordance lines to each row; `kwic` accepts `context=N`.

Every response uses the same shapes as `--format json`, defined once in `slovo_core::api`,
so the CLI and the API cannot drift apart.

The CLI does not need the server: `slovo freq` calls the engine directly.
Only the UI goes over HTTP.

Two things to fix before exposing this beyond your machine:
there is no authentication (binding a non-loopback address prints a warning), and CORS is permissive.

## License

Dual-licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

The corpus data you import keeps its own license; `UD_English-EWT`, for example, is CC BY-SA 4.0.
