# Slovo

A local, offline corpus workstation.
Bring your own text, build a searchable corpus, then explore it from a CLI and a GUI.

The engine is Rust.
The GUI is Tauri + SvelteKit.
The CLI is the reference frontend.

**Status:** early.
The CLI imports Universal Dependencies treebanks, downloads them from a catalog, and prints frequency lists.
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

Global options: `-d/--dataset`, `-n/--limit` (default 20), `--format markdown|json|tsv` (default `markdown`).
`--catalog` and `SLOVO_CATALOG` point at a different catalog.

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

## Development

The `justfile` wraps the common commands.

```console
$ just                  # list recipes
$ just check            # fmt + clippy + tests
$ just test
$ just run -d ewt freq -n 5
$ just ewt ~/corpora/UD_English-EWT
$ just gui-dev          # Tauri dev server
```

Layout:

```
crates/slovo-dataset/  manifest and catalog types
crates/slovo-core/     engine: schema, store, ingest, query, registry
crates/slovo-fetch/    downloader: catalog, HTTP, checksums
crates/slovo-cli/      the `slovo` binary
src-tauri/             Tauri shell (kept out of the Cargo workspace for now)
src/                   SvelteKit frontend
catalog.json           the default catalog
docs/DESIGN.md         design notes and roadmap
docs/GLOSSARY.md       linguistic and tooling terms
```

## License

Dual-licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

The corpus data you import keeps its own license; `UD_English-EWT`, for example, is CC BY-SA 4.0.
