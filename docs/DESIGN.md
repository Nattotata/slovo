# Slovo — Design

A local, offline corpus workstation.
Bring your own text, build a searchable corpus, explore it from a CLI and a GUI.

Comparable tools: corpora.byu.edu, CQPweb / IMS Corpus Workbench, AntConc.
Slovo is local-first, Markdown-output, and shares one Rust core between the CLI and the GUI.

**Status:** M0–M1.5 implemented (see §14).

Vocabulary used here is defined in [`GLOSSARY.md`](GLOSSARY.md).

---

## 1. Decisions

- **Data** — openly licensed, pre-annotated corpora first: Universal Dependencies → Leipzig → raw text. BYU corpora are not redistributable.
- **Dataset boundary** — a dataset is a directory with a `manifest.json`; the catalog is a separate `catalog.json`; the engine never hard-codes a dataset.
- **Parsing** — yes. Ingest is parse → normalize → store. The canonical intermediate format is verticalized text: one token per line, as in CWB `.vrt`.
- **Storage** — SQLite, one file per dataset. No corpus container standard exists; the de facto standard is CWB storage plus CQP-syntax queries. One `token` row per corpus position is the CWB positional-attribute model.
- **CLI output** — Markdown by default; `--format json|tsv` for machines.
- **CLI input** — pattern strings with wildcards; a CQL subset later.
- **Scope** — frequency, concordance/KWIC, n-grams, collocations, keywords, cross-dataset comparison.
- **Provenance** — always dataset + document + source link/offsets; source text optional.
- **GUI** — Tauri + SvelteKit, a thin wrapper over `slovo-core`, not a subprocess of the CLI.
- **Languages** — English first, then Slovak and Czech. UD's universal tagset keeps the schema language-agnostic.
- **Cross-platform** — Tauri for the GUI, `cargo-dist` for CLI binaries, bundled SQLite, annotation models downloaded on demand.

---

## 2. Architecture

Both frontends call `slovo-core`.
The CLI does not shell out; the GUI does not parse the CLI's Markdown.

```
slovo-dataset     manifest · catalog types
slovo-core        store · ingest · query · statistics
slovo-fetch       catalog · downloader · checksums

  ├── slovo-cli   → `slovo` (core + fetch)
  └── slovo-tauri → Tauri commands → SvelteKit (core)
```

The CLI keeps `--format json` so the same operations are usable from scripts and other languages.

```
crates/slovo-dataset/  manifest and catalog types
crates/slovo-core/     engine
crates/slovo-fetch/    downloader
crates/slovo-cli/      the `slovo` binary
src-tauri/             Tauri shell (its own Cargo workspace for now)
src/                   SvelteKit frontend
catalog.json           the default catalog
```

Moving `src-tauri` into `crates/slovo-tauri` is an M5 task.

---

## 3. Data sources

Rule: if the text cannot be redistributed, support importing it but do not ship it.

| Source | Content | Format | License |
|---|---|---|---|
| [Universal Dependencies](https://universaldependencies.org/) | gold tokens, lemmas, POS, syntax; 200+ languages | CoNLL-U | mostly CC BY-SA |
| [GUM](https://gucorpling.org/gum/) | multi-layer English | CoNLL-U, TEI | CC BY-NC-SA 4.0 |
| [Leipzig (Wortschatz)](https://wortschatz.uni-leipzig.de/en/download) | sentences, word frequency, precomputed co-occurrence; 250+ languages | plain text, MySQL | CC BY (some NC) |
| [OPUS](https://opus.nlpl.eu/) | subtitles, EU/UN documents | TMX, plain | mostly open |
| [Project Gutenberg](https://www.gutenberg.org/) | public-domain books | `.txt` | public domain |
| [OANC / MASC](https://anc.org/) | contemporary American English | XML/XCES, CoNLL | open |
| Wikipedia dumps | broad text | XML | CC BY-SA |

BYU corpora (COCA, COHA, NOW) are licensed products; an importer for users who own a copy is acceptable, shipping them is not.
Google Books n-grams are pre-aggregated counts, not raw text.

### Recommended order

English first, then Slovak and Czech.

| | Universal Dependencies | Leipzig | Project Gutenberg |
|---|---|---|---|
| Tokenized/tagged? | gold | yes (varies) | no — must build it |
| Day-1 features | form, lemma, UPOS, feats, deps | sentences, freq, co-occurrence | raw characters |
| English size | ~255k words (EWT) | up to 1M sentences | ~1B words |
| Slovak / Czech | yes (SNK, PDT/CAC/CLTT) | yes (up to 1M sentences each) | mostly English-only |
| License | CC BY-SA (a few NC) | CC BY (some NC) | public domain |

Consequences:

1. Pre-annotated corpora need no NLP engine in v1; ingest is parse → insert. Raw text needs a tagger/lemmatizer first.
2. Annotation sets the feature ceiling. Surface-only corpora support concordance and raw n-grams; lemma collocations, POS-pattern queries and keyword analysis need lemmas/POS.
3. UD is for architecture, not scale. English-EWT is ~255k words, too small for credible per-million or collocation scores. Leipzig supplies scale.

UD uses a universal tagset, so English, Slovak and Czech share one schema.
Switching language loads a different treebank; the schema does not change.
`UD_Czech-PDT` is CC BY-NC-SA; use `Czech-CAC` / `Czech-CLTT` (CC BY-SA) for redistributable Czech.
`UD_Slovak-SNK` and `UD_English-EWT` are CC BY-SA.

Order: `UD_English-EWT` → Leipzig English → raw text + annotation engine.

The catalog is `catalog.json` in the repository root. It is a local file; `--catalog` or `SLOVO_CATALOG` points at another one.

---

## 4. Ingest

Two stages.

1. **Parse → canonical form.** Convert each input format to verticalized text: one token per line, TAB-separated `form, lemma, upos, xpos, feats, head, deprel`, with `<text>`/`<s>` boundary lines. This is the CWB `.vrt` convention and keeps ingest format-agnostic.
2. **Annotate if needed.** Raw text requires a tokenizer/lemmatizer/POS tagger. Already-annotated input (CoNLL-U, GUM) skips this.

Implemented: `.conllu`.
Later: `.txt`, `.vrt`/`.tsv`, TEI/XCES XML, Wikipedia dumps, TMX.

Annotation engines: start with rule/dictionary tools (`rust-stemmers`, `unicode-segmentation`) behind a pluggable interface; later `udpipe-rs` (65+ languages) or ONNX taggers.

Ingest is transactional and per-file.
If a directory contains a `manifest.json`, its metadata fills in the import options.
One file becomes one `document`; importing a directory creates one document per `.conllu` file.
Multiword tokens (`1-2`) and empty nodes (`5.1`) are skipped.

---

## 5. Storage

One SQLite file per dataset, via `rusqlite` with the `bundled` feature.

```sql
CREATE TABLE corpus (
  id INTEGER PRIMARY KEY, name TEXT UNIQUE NOT NULL, language TEXT,
  description TEXT, token_count INTEGER NOT NULL DEFAULT 0, created_at TEXT NOT NULL
);
CREATE TABLE document (
  id INTEGER PRIMARY KEY, corpus_id INTEGER NOT NULL REFERENCES corpus(id),
  external_id TEXT, title TEXT, source_url TEXT, license TEXT, meta TEXT
);
CREATE TABLE sentence (
  id INTEGER PRIMARY KEY, document_id INTEGER NOT NULL REFERENCES document(id),
  position INTEGER NOT NULL, text TEXT
);
CREATE TABLE token (
  id INTEGER PRIMARY KEY, sentence_id INTEGER NOT NULL REFERENCES sentence(id),
  position INTEGER NOT NULL, form TEXT NOT NULL, lower TEXT NOT NULL,
  lemma TEXT, upos TEXT, xpos TEXT, feats TEXT, head INTEGER, deprel TEXT
);
CREATE INDEX token_lower_idx ON token(lower);
CREATE INDEX token_lemma_idx ON token(lemma);
CREATE INDEX token_upos_idx  ON token(upos);
```

Notes:

- `token.id` is the corpus position; ordering is implicit. "The token after `take`" is a range/join, which is what CQL needs.
- One dataset per file means queries do not filter by `corpus_id`.
- Frequency lists are `GROUP BY` over `token`. Large n-gram counts will be materialized in an `ngram` table built during ingest.
- FTS5 indexes documents, not positions, so it cannot do collocation windows. Keep it for a future "search source text" feature only.
- If reads outgrow SQLite, DuckDB or a CWB-style inverted index drops in behind the same `Store` API.

Registry: `$SLOVO_DATA_DIR/datasets/<name>.db`, default `~/.local/share/slovo`.
`-d` also accepts a direct path to a `.db` file.

---

## 6. Query model

1. Simple searches (v1): terms, wildcards `*` (any tokens) and `?` (one character), case sensitivity.
2. CQL subset later: `[lemma="take"][upos="ADV"]{0,2}[upos="VERB"]`, quantifiers, structural constraints.

`take *` is a pattern with an open slot.
It finds `take`, captures the following token(s), and counts and ranks the matches.
It is not substring matching.

---

## 7. Dataset boundary

The app never hard-codes a dataset.
A dataset is a directory of source files plus a `manifest.json`.
The app imports that directory; it does not know which corpus it came from.

A *catalog* is a separate JSON file listing datasets that can be downloaded.
It is a plain data file, not code: it can be edited, swapped, or pointed at a different location without rebuilding the app.
The catalog ships in the repository as `catalog.json`, but the app can also read a URL or a local path.

The downloader is a separate crate (`slovo-fetch`), and the app's engine (`slovo-core`) does not depend on it.
The downloader only writes a dataset directory; the app imports it.
This keeps the network, the checksums, and the catalog out of the engine, and keeps the engine free of corpus-specific knowledge.

```text
catalog.json ──► slovo-fetch ──► dataset directory ──► slovo-core
```

The `slovo-dataset` crate owns the manifest and catalog types; both `slovo-fetch` and `slovo-core` depend on it.

---

## 8. CLI

```
slovo [GLOBAL OPTIONS] <COMMAND>

Datasets
  slovo sources [--catalog <path>]    list the catalog
  slovo fetch <id> [--catalog <path>] [--to <dir>]
                                      download a dataset
  slovo add <id> -d NAME [--catalog <path>]
                                      download + import
  slovo datasets                      list installed datasets
  slovo import <path> -d NAME         build a dataset from files/dir
  slovo info -d NAME                  size, token count, license, sources

Search & analysis
  slovo -d NAME <pattern>             shorthand for `search`        (M2)
  slovo search -d NAME <pattern>      pattern/wildcard search       (M2)
  slovo kwic   -d NAME <pattern>      concordance lines             (M3)
  slovo collocates -d NAME <word>     collocates + association scores (M4)
  slovo ngrams -d NAME --size 3       most frequent n-grams         (M4)
  slovo freq   -d NAME                frequency list                (done)
  slovo compare -d A -d B <pattern>   keyword analysis A vs B       (M4)
  slovo doc    -d NAME <doc-id>       print a source document       (M3)

Global options
  -d, --dataset <NAME>                dataset, or a path to a .db file
      --format <markdown|json|tsv>    output format                 [default: markdown]
  -n, --limit <N>                     max rows                      [default: 20]
```

Implemented: `import`, `datasets`, `info`, `freq`, `sources`, `fetch`, `add`.

Quote patterns: `slovo -d movies 'take *'`, or the shell expands `*` to filenames.

Target output for `slovo -d movies 'take *'`:

```markdown
## `take *` — dataset `movies`

_200,000,000 tokens · 12,345 distinct matches_

| # | match | freq | per million |
|---|-------|-----:|------------:|
| 1 | take off | 12,345 | 61.7 |
| 2 | take up | 9,871 | 49.4 |
| 3 | take out | 7,654 | 38.3 |
| 4 | take up with | 421 | 2.1 |
```

`--format json` is the stable contract for the GUI and scripts:

```json
{"query":"take *","dataset":"movies","tokens":200000000,
 "results":[{"match":"take off","freq":12345,"per_million":61.7}]}
```

`--examples N` attaches linkable concordance lines (§8).

---

## 9. Provenance

Three tiers.

1. **Always:** dataset name, document id/title, source URL/path, stored in `document`.
2. **Concordance hits:** document id plus token offsets. Deep link when `source_url` exists (`…/book.txt#L1204`), otherwise `slovo doc -d NAME <id>`.
3. **Optional:** store the source bytes in a `documents/` sidecar, enabled with `--keep-source`. Off by default.

Links are relative and portable.
Never fabricate a URL; print the id when there is none.

---

## 10. Feature scope

1. Frequency lists.
2. Concordance / KWIC.
3. N-grams / clusters.
4. Collocations: MI, t-score, log-likelihood (Dunning), Dice, logDice.
5. Keyword / compare: log ratio, log-likelihood.
6. Later: CQL, regex, POS/lemma patterns, subcorpus filters, dispersion (Gries' DP, Juilland's D).

---

## 11. GUI

- One search box (the CLI query syntax) and a dataset selector.
- Concordance view: KWIC table with sorting and source links.
- Side panels: collocates, frequency, collocation graph.
- Ingest screen: pick files, name the dataset, watch progress via Tauri events.
- Export the current view as Markdown through the same renderer as the CLI.

Core operations are `#[tauri::command]`s that take and return the same serde structs as `--format json`.

---

## 12. Cross-platform

- CLI: `cargo-dist` builds signed Linux/macOS/Windows binaries. `rusqlite` `bundled` removes the system SQLite dependency.
- GUI: Tauri's bundler targets all three.
- Annotation models: download on demand into the data dir.
- CI: `cargo test` plus `npm run check`.

---

## 13. Crates

| Need | Crate |
|---|---|
| CLI parsing | `clap` (derive) |
| SQLite | `rusqlite` (`bundled`) |
| Errors | `thiserror` (lib), `anyhow` (CLI) |
| Serialization | `serde`, `serde_json` |
| Parallel ingest | `rayon`, progress `indicatif` |
| Unicode / tokens | `unicode-segmentation`, `regex` |
| Stemming/lemmatizing | `rust-stemmers`; later `udpipe-rs` |
| CoNLL-U parsing | hand-rolled (the subset needed is small) |
| Verticalized/TSV/CSV | `csv` |
| XML (TEI/XCES) | `quick-xml` |
| Dates | `time` / `chrono` |
| HTTP | `ureq` (blocking, rustls) |
| Checksums | `sha2` |
| Archives | `tar`, `flate2` |

In use: `rusqlite` (bundled), `thiserror`, `dirs`, `clap` (derive), `anyhow`, `serde`, `serde_json`, `ureq`, `sha2`; dev: `tempfile`.

---

## 14. Roadmap

- **M0 — Skeleton.** ✅ Cargo workspace, `slovo-core` types, `slovo-cli`.
- **M1 — Ingest + frequency.** ✅ CoNLL-U importer (`UD_English-EWT`); ⬜ `.txt`/`.vrt` importers. ✅ `slovo freq`.
- **M1.5 — Downloader.** ✅ `catalog.json` + `slovo sources`/`fetch`/`add`; `slovo-dataset` and `slovo-fetch` crates; manifest-driven import.
- **M2 — Search + Markdown output.** ⬜ wildcards/phrases; generalize the Markdown renderer to search results.
- **M3 — Concordance + provenance.** ⬜ `slovo kwic`, source links, `slovo doc`.
- **M4 — Collocations + n-grams.** ⬜ statistics module with logDice.
- **M5 — GUI.** ⬜ wire Tauri commands to the M2–M4 core; move `src-tauri` into `crates/`.
- **M6 — Distribution.** ⬜ `cargo-dist`, docs, an example dataset.

Verified: `UD_English-EWT` imports to 254,820 tokens, matching UD's published count.

---

## 15. Open items

- Whether to store raw source text by default. `--keep-source` is designed but not implemented.
- Whether punctuation should be excluded from default frequency lists. It is included now; `.` and `,` rank second and third in EWT.
- Slovak and Czech need no schema change, but need per-language lemmatization if raw text is added.
- `README.md` and this file are the only docs; a `slovo` man page or `--help` examples can wait.
