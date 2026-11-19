# Slovo — Design

A local, offline corpus workstation.
Bring your own text, build a searchable corpus, explore it from a CLI and a GUI.

Comparable tools: corpora.byu.edu, CQPweb / IMS Corpus Workbench, AntConc.
Slovo is local-first, Markdown-output, and shares one Rust core between the CLI and the GUI.

**Status:** M0–M3 implemented (see §14).

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

`slovo-core` is the engine and knows nothing about HTTP, the network or the CLI.

```
slovo-core        store · ingest · query · statistics

slovo-dataset     manifest · catalog types
slovo-fetch       catalog · downloader · checksums
slovo-annotate    raw text → CoNLL-U (its own crate, on purpose)
slovo-service     the operations every client shares
                  └── depends on core + dataset + fetch

slovo-cli         `slovo` — CLI, `serve`                  (frontend: people and scripts)
slovo-server      HTTP API (axum)                         (frontend: UIs)
```

`slovo-annotate` is deliberately its own crate and not part of `slovo-core`.
The engine imports CoNLL-U; turning raw text *into* CoNLL-U is a different job
with different dependencies (tokenizers, taggers, models), and keeping it apart
means the engine stays lean and the annotator can be replaced wholesale.
The two share a *format*, not code: `slovo-annotate` writes the same CoNLL-U the
core reads, and does not depend on it.

`slovo-service` exists because "download a catalog entry, then import it" used to be composed inside the CLI, so only the CLI could do it.
Reads were always fine; writes had leaked into a frontend.
The API and any later client now share that code.

Two transports, on purpose:

- **Direct.** The CLI calls `slovo-service` in-process. `slovo freq` never needs a server running.
- **HTTP.** The UI talks to `slovo-server`, so the same UI works in a browser and inside the Tauri shell.

Both emit the same JSON, defined once in `slovo_core::api`, so the CLI's `--format json` and the API cannot drift.

```
crates/slovo-dataset/  manifest and catalog types
crates/slovo-core/     engine (no HTTP, no network)
crates/slovo-annotate/ raw text → CoNLL-U (tokenize; pluggable tagger)
crates/slovo-service/  service layer
crates/slovo-fetch/    downloader
crates/slovo-server/   HTTP API
crates/slovo-cli/      the `slovo` binary
examples/songbook/     a worked markdown example
src/                   SvelteKit frontend
src-tauri/             Tauri shell (its own Cargo workspace for now)
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
Later: `.vrt`/`.tsv`, TEI/XCES XML, Wikipedia dumps, TMX.

Raw text is handled by `slovo-annotate`, a separate crate (§2). It owns the
mechanical stages — chapter segmentation, sentence splitting, tokenization — in
plain Rust, so the chain runs with nothing installed, and takes lemma/POS from
an external tagger command that speaks CoNLL-U on stdout (`--tagger`).
The annotator writes the same CoNLL-U the core reads; the core gains no
dependency on it.

**Markdown** is a front-end in the same crate, not a second engine. It reads
frontmatter (title, type), treats each heading as a weighted section, drops
fenced code, and strips inline markers (`::G::`, `**Chords**`) — including
mid-word, so `mag::B::gots` → `maggots`. One file is one document; its sections
share it. The weight tiers come from a data file, so a corpus can change them
without a code change.

A **weight** is a ranking aid, kept strictly apart from frequency: `freq` counts
occurrences, `freq --rank weight` sums weights. A heading word outranks a body
word without its frequency changing. Conflating the two would corrupt every
count downstream, so the two are separate columns and separate commands, and the
output labels which it is showing.

The tagger is authoritative only for the columns the annotator does not produce
(lemma, upos, xpos, feats, head, deprel). Tokenization stays the annotator's, so
token boundaries do not shift when the tagger changes. A token-count mismatch is
tolerated: lines that line up are merged, the rest stay `_`.

A tagger mis-handles archaic forms (`spake` → `spake`). `--lexicon <file>` takes
a TSV of corrections and applies them **non-destructively**: the tagger's lemma
stays in `lemma`, the correction goes to `lemma_override` (and `upos_override`),
carried through CoNLL-U in the MISC column. Queries pick the layer:
`[lemma=…]` reads the tagger, `[lemma_override=…]` the lexicon. Nothing is
overwritten, so "what the model thought" stays inspectable while you build the
lexicon — which is the point.

In-process tagging (`udpipe-rs`, ONNX) can slot in behind the same seam later;
today the external-command hook keeps models out of the build.

Ingest is transactional and per-file.
If a directory contains a `manifest.json`, its metadata fills in the import options.
One file becomes one `document`; importing a directory creates one document per `.conllu` file.
A file that carries `# newdoc id` comments (UD does) is split at each one, so each `newdoc` becomes its own document; its external id is `<file>#<newdoc>`.
Re-importing the same file is a no-op: documents are matched by `external_id`, so a corpus cannot silently double.
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
  id          INTEGER PRIMARY KEY, document_id INTEGER NOT NULL REFERENCES document(id),
  position    INTEGER NOT NULL, text TEXT,
  external_id TEXT,   -- the source's # sent_id
  newdoc_id   TEXT    -- the enclosing # newdoc id
);
CREATE TABLE token (
  id INTEGER PRIMARY KEY, sentence_id INTEGER NOT NULL REFERENCES sentence(id),
  position INTEGER NOT NULL, form TEXT NOT NULL, lower TEXT NOT NULL,
  lemma TEXT, upos TEXT, xpos TEXT, feats TEXT, head INTEGER, deprel TEXT,
  lemma_override TEXT,   -- a lexicon's correction; never overwrites `lemma`
  upos_override  TEXT,
  weight         REAL NOT NULL DEFAULT 1.0  -- structural ranking weight
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

Schema changes are additive: `SCHEMA_VERSION` is bumped and `Store::open` adds any missing column via `ALTER TABLE` (see `MIGRATIONS` in `schema.rs`), so an older dataset file keeps working without a re-import.

---

## 6. Query model

The query language is a subset of **CQP** (the IMS Corpus Workbench), the de facto standard for corpus queries.
CQPweb, Sketch Engine and KonText implement the same syntax, so queries copy across.
The CWB authors reject the name "CQL" because of a collision with a Library of Congress standard; Slovo says CQP.

Nodes and quantifiers:

| Syntax | Meaning |
|---|---|
| `[]` | any token |
| `take` | `[word="take"]` |
| `[lemma="take"]`, `[upos="NOUN"]` | constraint on an attribute |
| `?` `*` `+` `{n}` `{n,m}` `{n,}` `{,m}` | zero-or-one / zero-or-more / one-or-more / exactly n / n to m / at least n / up to m |
| `(...)` `\|` | grouping and alternation |
| `within N` | cap the match at N tokens |

Quantifiers attach to the preceding node, so `[lemma="take"] []+` is *take* followed by one or more tokens.

Aliases for the common cases:

| You write | Expands to | Meaning |
|---|---|---|
| `take` | `[word="take"]` | the word |
| `_` | `[]` | any single token |
| `*` | `[]+` | one or more tokens |
| `NOUN` | `[upos="NOUN"]` | part-of-speech tag |

The rules that keep it unambiguous:

* A quantifier must be **attached**: `[]+`, not `[] +`.
* A **standalone** `*` is therefore free to mean `[]+`.
* Whitespace separates nodes, so `take off` is a two-token sequence.

Deliberate deviations from CQP:

* **`*` as an alias.** CQP has no standalone `*`; it is only a postfix quantifier. This is the one piece of BYU-flavoured sugar, and `--explain` always shows the expansion.
* **Literal attribute values.** In CQP, `[word="take.*"]` is a regular expression. Slovo compares literally; `--regex` is planned.
* **Case-insensitive by default.** CQP is case-sensitive. Slovo matches on the lower-cased form, like `freq`; `--case-sensitive` opts in. A trailing `%c` is accepted and ignored.
* **All match lengths are counted.** CQP returns one match per start position. Slovo records every span length up to `within`, so `take *` reports `take off` and `take up with` as separate rows. This is what makes the output a cluster list rather than a list of maximal spans.

`take *` is a cluster query: find `take`, then capture one or more following tokens, and rank the captures.
It is not substring matching.

Unbounded `[]*` can match almost a whole sentence; `within` (default 5) is the guard.

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
  slovo annotate <path> -o FILE       raw text → CoNLL-U              (done)
  slovo info -d NAME                  size, token count, license, sources

Search & analysis
  slovo -d NAME <pattern>             shorthand for `query`         (done)
  slovo query -d NAME <pattern>       pattern search, CQP subset    (done)
  slovo kwic   -d NAME <pattern>      concordance lines             (done)
  slovo collocates -d NAME <word>     collocates + association scores (M4)
  slovo ngrams -d NAME --size 3       most frequent n-grams         (M4)
  slovo freq   -d NAME                frequency list                (done)
  slovo freq   -d NAME --rank weight  rank by structural weight      (done)
  slovo compare -d A -d B <pattern>   keyword analysis A vs B       (M4)
  slovo docs   -d NAME                list source documents         (done)
  slovo doc    -d NAME <doc-id>       print a source document       (done)

Query options
      --explain                       print canonical CQP and exit
      --case-sensitive                match the surface form
      --within <N>                    max tokens per match          [default: 5]
      --min-freq <N>                  drop rarer rows               [default: 1]
      --examples <N>                  attach concordance lines      [default: 0]

KWIC options
      --case-sensitive                match the surface form
      --within <N>                    max tokens per match          [default: 5]
      --context <N>                   tokens of context each side   [default: 5]

Global options
  -d, --dataset <NAME>                dataset, or a path to a .db file
      --format <markdown|json|tsv>    output format                 [default: markdown]
  -n, --limit <N>                     max rows                      [default: 20]
```

Implemented: `import`, `datasets`, `info`, `freq`, `sources`, `fetch`, `add`, `query` (plus the bare-pattern shorthand), `kwic`, `docs`, `doc`, `annotate`.

Quote patterns: `slovo -d movies 'take *'`, or the shell expands `*` to filenames.

Actual output for `slovo -d ewt 'take *'`:

```markdown
## `take *` — dataset `ewt`

`take *` → `[word="take"] []+ within 5`

_254,820 tokens · 960 matches · 711 distinct_

| # | match | freq | per million |
|--:|:------|-----:|------------:|
| 1 | take a | 30 | 117.7 |
| 2 | take care | 27 | 106.0 |
| 3 | take the | 26 | 102.0 |
| 4 | take care of | 17 | 66.7 |
| 5 | take it | 15 | 58.9 |
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
2. **Concordance hits:** document id, sentence id and token offsets. ✅ `slovo kwic` and `query --examples N` emit these, plus the document title, `source_url` and `# sent_id` when present. ✅ A UD `# newdoc id` block becomes its own document, so an id names one source text. ✅ `slovo docs` lists them and `slovo doc -d NAME <id>` prints one in full. ⬜ Deep-linking the URL (`…/book.txt#L1204`).
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
6. Structural ranking: sum a token's `weight` (from markdown headings, or any annotated structure) instead of counting it. A ranking aid, never a count.
7. Later: CQL, regex, POS/lemma patterns, subcorpus filters, dispersion (Gries' DP, Juilland's D).

---

## 11. GUI

The UI is a SvelteKit app that talks to `slovo-server` over HTTP.
That is the point of the server: the browser and the Tauri shell use one transport, so there is no `invoke`-versus-`fetch` fork, and no logic duplicated per target.

One static build, via `@sveltejs/adapter-static`.
It is a client-rendered SPA (`ssr = false`), so it needs no Node runtime — it can be served by any static host, bundled into the Tauri shell, or embedded in the `slovo` binary.

Decided: embed the built UI in the Rust binary (PocketBase-style, one process, one port). ⬜ not wired up yet.

Planned views:

- One search box (the CLI query syntax) and a dataset selector.
- Concordance view: KWIC table with sorting and source links.
- Side panels: collocates, frequency, collocation graph.
- Ingest screen: pick files or a catalog id, and watch progress (needs import jobs).
- Export the current view as Markdown through the same renderer as the CLI.

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
| Raw-text annotation | `slovo-annotate` (own crate): plain-Rust tokenizer; external tagger via command |
| CoNLL-U parsing | hand-rolled (the subset needed is small) |
| Verticalized/TSV/CSV | `csv` |
| XML (TEI/XCES) | `quick-xml` |
| Dates | `time` / `chrono` |
| HTTP | `ureq` (blocking, rustls) |
| HTTP server | `axum`, `tokio`, `tower-http` (CORS) |
| Checksums | `sha2` |
| Archives | `tar`, `flate2` |

In use: `rusqlite` (bundled), `thiserror`, `dirs`, `clap` (derive), `anyhow`, `serde`, `serde_json`, `ureq`, `sha2`, `axum`, `tokio`, `tower-http`; dev: `tempfile`, `tower`.

---

## 14. Roadmap

- **M0 — Skeleton.** ✅ Cargo workspace, `slovo-core` types, `slovo-cli`.
- **M1 — Ingest + frequency.** ✅ CoNLL-U importer (`UD_English-EWT`); ✅ `slovo-annotate`: `.txt` → CoNLL-U (tokenize, sentence split, chapter segmentation, pluggable tagger); ⬜ in-process tagger. ✅ `slovo freq`.
- **M1.5 — Downloader.** ✅ `catalog.json` + `slovo sources`/`fetch`/`add`; `slovo-dataset` and `slovo-fetch` crates; manifest-driven import.
- **M2 — Search + Markdown output.** ✅ CQP subset + aliases, `slovo query`, the bare-pattern shorthand, `--explain`, Markdown/JSON/TSV output.
- **M2.5 — HTTP API.** ✅ `slovo-server` + `slovo serve`; one JSON contract (`slovo_core::api`) shared by the CLI and the API; ✅ `slovo-service` holds the operations clients share. ⬜ auth, pagination, import jobs.
- **M3 — Concordance + provenance.** ✅ `slovo kwic`, `--examples N` on `query`, document/sentence provenance on every hit, per-`newdoc` documents, `slovo docs` and `slovo doc`; ⬜ source deep links.
- **M4 — Collocations + n-grams.** ⬜ statistics module with logDice.
- **M5 — GUI.** ⬜ wire the SvelteKit UI to `slovo-server`; decide who serves the UI (Node or the Rust binary); move `src-tauri` into `crates/`.
- **M6 — Distribution.** ⬜ `cargo-dist`, docs, an example dataset.

Verified: `UD_English-EWT` imports to 254,820 tokens, matching UD's published count.

---

## 15. Open items

- Whether to store raw source text by default. `--keep-source` is designed but not implemented.
- Whether punctuation should be excluded from default frequency lists. It is included now; `.` and `,` rank second and third in EWT.
- Slovak and Czech need no schema change, but need per-language lemmatization if raw text is added.
- `README.md` and this file are the only docs; a `slovo` man page or `--help` examples can wait.
