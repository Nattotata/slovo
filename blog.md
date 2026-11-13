# One day from Tauri scaffold to corpus server

*2026-11-13*

Slovo is a local corpus workstation: bring your own text, build a searchable corpus, query it.
This is a log of the day's work, including the decisions I reversed.

## The starting point

`slovo` was a Tauri + SvelteKit scaffold with an empty `greet` command and an abandoned TypeScript prototype (`better-sqlite3` + `wink-nlp`).
The prototype's imports were not even in `package.json` — it had never run, and it contradicted the "I want Rust" goal.
Deleted it.

## The decision that shaped everything else

Storage.

There is no corpus container standard.
The de facto one is the IMS Corpus Workbench: an ordered stream of token positions, each carrying word, lemma and POS.
That is exactly a SQL table with an integer primary key.

So SQLite is not a compromise, it is the CWB data model with better tooling.
One file per dataset, one row per token, `token.id` is the corpus position, and everything else — concordance, collocations, dispersion — is arithmetic on positions.

## Build order

1. **Engine.** `slovo-core`: schema, store, CoNLL-U importer, frequency queries.
2. **CLI.** `slovo import | datasets | info | freq`.
3. **Downloader.** `slovo-dataset` (catalog and manifest types) plus `slovo-fetch` (HTTP, checksums).
4. **Query language.** A CQP subset with aliases for the common cases.
5. **Server.** `slovo-server` plus `slovo serve`.
6. **Service layer.** `slovo-service`: the operations the CLI and the API both call.

## The data boundary

The rule: if the text cannot be redistributed, support importing it but never ship it.
BYU's corpora are licensed products, so they get an importer at most.

That pushed the work toward openly licensed, already-annotated corpora.
Starting with Universal Dependencies was the right call for two reasons:

- It is pre-annotated, so v1 needed **no NLP engine at all** — import is "parse, insert rows".
- Its tagset is universal, so English, Slovak and Czech share one schema.

The dataset boundary is deliberate: a dataset is a directory with a `manifest.json`, and a catalog is a separate `catalog.json`.
The engine never hard-codes a corpus.
The catalog pins each treebank to an immutable release tag (`r2.19`) rather than `master`, with SHA-256 checksums, because branches move.

## What I got wrong, and corrected

### The license

`LICENSE` was AGPL-3.0 while `package.json` and the Cargo manifest both said MIT.
AGPL is the strongest common copyleft: it is a barrier for corporate use regardless of how the code is used, and Slovo is an application rather than a library people link into.

Settled on the Rust convention, `MIT OR Apache-2.0`, with `LICENSE-MIT` and `LICENSE-APACHE`.
Along the way I introduced a bug: a `[workspace]` table inserted before `authors` and `edition` in `src-tauri/Cargo.toml`, which silently moved both keys out of `[package]`.

### `take *`

The original brief wanted `take *` to list `take off`, `take up with`.

That is BYU/COCA syntax, where `*` means **exactly one word** — so `take up with` could never appear.
The design doc's example table mixed BYU's symbol with a "one or more" meaning that neither BYU nor CQP has.

The standard is CQP: `[]` is any token, and `*` is a postfix quantifier meaning "zero or more", applied to the preceding node.
So the CQP spelling is `take []+`.

Resolution: CQP stays canonical, a **standalone** `*` is an alias for `[]+`, and `--explain` always shows the expansion:

```console
$ slovo query --explain 'take (off|up)'
[word="take"] ([word="off"] | [word="up"]) within 5
```

The rule that keeps it unambiguous: a quantifier must be attached (`[]+`, not `[] +`), so a standalone `*` is free to mean something else.

### Case sensitivity

I recommended case-insensitive by default, reversed to CQP's case-sensitive default, then shipped case-insensitive with `--case-sensitive` to opt in, because `freq` was already insensitive and consistency within the tool matters more than consistency with CQP here.
It is documented as a deliberate deviation, and it is a one-line change.

### Match lengths

CQP returns one match per start position.
Slovo enumerates **every** span length up to `within`.

That is the deviation that makes the output useful: `take *` reports `take off` and `take up with` as separate rows instead of one maximal span.

```console
$ slovo -d ewt 'take *'
```

|   # | match     | freq | per million |
| --: | :-------- | ---: | ----------: |
|   1 | take a    |   30 |       117.7 |
|   2 | take care |   27 |       106.0 |
|   3 | take the  |   26 |       102.0 |

### Doubling

Importing the same directory twice silently doubled the corpus — one dataset showed 509,640 tokens instead of 254,820.
Import is now idempotent by source path and reports what it skipped.

### Node, then not Node

I first switched the UI to `@sveltejs/adapter-node` for self-hosting.

Then realised the flexibility I was reaching for comes from the **server**, not from Node.
Node only decides who serves the HTML, and a static build is deployable in more places: any static host, or embedded in the Rust binary.
The CLI, a daemon, an nvim plugin and a desktop shell are all unaffected by the choice either way.

Dropped it; one static build again.
(Also worth knowing: `adapter-node` v6 requires SvelteKit 3, which is prerelease, so it would have needed pinning to v5 anyway.)

### The service layer

`add` was composed inside the CLI: fetch, then import.
That meant only the CLI could do it, and the API could not offer an import endpoint without duplicating the orchestration.

Hoisted into `slovo-service`, which the CLI and the HTTP API both call.
Reads were already fine; writes were the part that had leaked into a frontend.

## The server

`slovo serve` runs an HTTP API over the same engine, on `127.0.0.1:8090` by default.
It exists so the browser UI and the desktop shell share one transport, instead of a `fetch`-versus-`invoke` fork per target.

The CLI does not need it. `slovo freq` calls the engine in-process, so nothing breaks if the server is not running.

The part that keeps this honest: both the CLI's `--format json` and the API build their JSON from one place, `slovo_core::api`, so they cannot drift.

```
GET /api/health
GET /api/datasets
GET /api/datasets/{name}
GET /api/datasets/{name}/freq?by=lemma&limit=20
GET /api/datasets/{name}/query?pattern=take%20*&limit=20
```

Not ready for anything but a loopback address: no authentication yet, permissive CORS, one SQLite connection per request, and no pagination beyond `limit`.

## Numbers

- 6 crates: `slovo-dataset`, `slovo-core`, `slovo-service`, `slovo-server`, `slovo-fetch`, `slovo-cli`.
- 39 tests; `cargo clippy --workspace --all-targets -- -D warnings` clean; `cargo fmt --check` clean.
- `UD_English-EWT` imports to 254,820 tokens — matching UD's published count.
- `take *` over EWT: 960 matches, 711 distinct, about 0.6 s including process start.

## What is next

- **Concordance / KWIC.** The thing that makes source links and provenance meaningful.
- **Import jobs and progress.** Importing takes minutes; the API needs job state and a progress stream, which also replaces the plan for Tauri event progress.
- **Auth, pagination, connection pooling.** Prerequisites for exposing the API anywhere but localhost.
- **Collocations and n-gram statistics.** MI, t-score, log-likelihood, logDice.
- **Embedding the built UI in the `slovo` binary.** One process, one port, PocketBase-style.
