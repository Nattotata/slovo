# Slovo

A local, offline corpus workstation.
Bring your own text, build a searchable corpus, then explore it from a CLI and a GUI.

The engine is Rust.
The GUI is Tauri + SvelteKit.
The CLI is the reference frontend.

**Status:** early.
The CLI imports Universal Dependencies treebanks, downloads them from a catalog, prints frequency lists, and runs CQP-subset pattern queries.
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
| `slovo datasets` | List installed datasets |
| `slovo info -d NAME` | Show dataset metadata and counts |
| `slovo freq -d NAME` | Frequency list of `--by word\|form\|lemma\|upos\|xpos` |
| `slovo query -d NAME <pattern>` | Pattern search (CQP subset) |
| `slovo -d NAME <pattern>` | Shorthand for `query` |
| `slovo serve` | Serve the HTTP API for the UI |

Global options: `-d/--dataset`, `-n/--limit` (default 20), `--format markdown|json|tsv` (default `markdown`).
`--catalog` and `SLOVO_CATALOG` point at a different catalog.
Query options: `--explain`, `--case-sensitive`, `--within <N>`, `--min-freq <N>`.

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
| `(off\|up)` | *(unchanged)* | either |

Quantifiers attach to their node (`[]+`, `[]?`, `[]{1,3}`), and `within N` caps the match.
`--explain` always shows the expansion, so nothing is hidden.

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

# Download + import
cargo run -q -p slovo-cli -- -d ewt add ud/UD_English-EWT
cargo run -q -p slovo-cli -- -d ewt add ud/UD_English-EWT --language en

# Import a local directory
cargo run -q -p slovo-cli -- -d ewt import /tmp/ewt
cargo run -q -p slovo-cli -- -d ewt import /tmp/ewt --language en --description "UD English EWT"

# Installed datasets
cargo run -q -p slovo-cli -- datasets
cargo run -q -p slovo-cli -- -d ewt info
cargo run -q -p slovo-cli -- -d ewt info --format json

# Frequency
cargo run -q -p slovo-cli -- -d ewt freq
cargo run -q -p slovo-cli -- -d ewt freq -n 10
cargo run -q -p slovo-cli -- -d ewt freq --by lemma
cargo run -q -p slovo-cli -- -d ewt freq --by upos
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
cargo run -q -p slovo-cli -- -d ewt query --explain 'take *'
cargo run -q -p slovo-cli -- -d ewt query 'take *' --format json

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
$ just ewt ~/corpora/UD_English-EWT
$ just serve                # HTTP API on :8090
$ just gui-dev              # Tauri dev server
```

Layout:

```
crates/slovo-dataset/  manifest and catalog types
crates/slovo-core/     engine: schema, store, ingest, query, registry
crates/slovo-service/  the operations CLI and API both call
crates/slovo-fetch/    downloader: catalog, HTTP, checksums
crates/slovo-cli/      the `slovo` binary (CLI + `serve`)
crates/slovo-server/   HTTP API over the engine
src/                   SvelteKit frontend
src-tauri/             Tauri shell (kept out of the Cargo workspace for now)
catalog.json           the default catalog
docs/DESIGN.md         design notes and roadmap
docs/GLOSSARY.md       linguistic and tooling terms
```

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
