# Aggregated markdown search

A design note for using Slovo as the search back-end of a portfolio that
aggregates documentation from many project repos.
Written down so it does not have to be re-derived later.

**Status:** not implemented.
Everything below builds on what already exists (see [`DESIGN.md`](DESIGN.md));
the only missing piece is a `project` dimension and a filter for it.

Vocabulary is in [`GLOSSARY.md`](GLOSSARY.md).

---

## 1. The use case

Several repos, each with a `docs/` folder:

- `schematics` — Nix flake, KiCad files, PCB notes
- `room-modules` — Raspberry Pi NixOS, notes
- `puzzle-nodes` — Nix flake, `mpremote`, Python scripts, notes
- `uniquegames` — a website with docs worth indexing
- `sirius` — a small self-hosted site
- `totem` — Nix flake, keyboard firmware notes
- `nixos` — one repo, three hosts
- `portfolio` — the aggregator itself

The portfolio is the aggregator: it renders docs from all projects and shows
off a docs-as-code methodology.
Half the repos are private, so the aggregator **pulls docs from the VPS, not
from the repos**.

The goal here is search over those aggregated docs: one box, one query, results
that link back to the source.

---

## 2. The flow

```
project repo ──push──► docs/ archived ──► VPS docs store
                                              │
                              ┌───────────────┴───────────────┐
                              ▼                               ▼
                      portfolio rebuild              slovo annotate + import
                      (renders docs)                 (builds the search index)
                              │                               │
                              └──────────► portfolio site ◄────┘
                                           (docs + search page)
```

Properties that matter:

- **Push, not pull.** The VPS never clones the repos. Private stays private.
- **The VPS already stores docs** for the portfolio. Slovo rides that path; it
  does not need its own ingest pipeline.
- **Slovo runs as a step in the existing flow** (post-ingest, or a cron), not as
  its own service. It is invoked the same way the portfolio rebuild is.

---

## 3. Services, not a monolith

Separate services that share infrastructure, not one process:

```
docs-store.service     receives archives, triggers rebuilds   (already planned)
slovo-annotate         markdown → CoNLL-U                      (CLI, a job)
slovo-serve.service    read-only HTTP over the .db             (exists)
```

They share a machine and a filesystem and nothing else.
No service imports another's code; they coordinate through files
(`docs.db`) and signals.
Colocation on one VPS is a deployment choice, not an architecture choice:
the boundary is the **crate dependency graph**, and it is unchanged.

---

## 4. Tokenize per project, store as one dataset

This is the key decision, and it has two halves that are easy to conflate.

**Annotate per project**, because the project label is known by the aggregator,
not by the source file (see §5):

```console
$ slovo annotate ~/schematics/docs/*.md  --project schematics  -o /tmp/schematics.conllu
$ slovo annotate ~/puzzle-nodes/docs/*.md --project puzzle-nodes -o /tmp/puzzle-nodes.conllu
$ slovo annotate ~/totem/docs/*.md       --project totem         -o /tmp/totem.conllu
```

**Import as one dataset**, so cross-project search works:

```console
$ slovo -d docs import /tmp/schematics.conllu /tmp/puzzle-nodes.conllu /tmp/totem.conllu
```

Why not one dataset per project:

- Cross-project search ("where did I write about mpremote?") is the whole point
  of an aggregator. One dataset gives it for free.
- One dataset is one file to deploy, one server, one index.
- Filtering by project is a `WHERE`, which is cheap; separate datasets would
  make "search everything" a fan-out across N databases.

**Filtering is a column predicate, not a property of how you batched.**
Whether the tokens were produced in one call or many, they land in the same
table with the same columns, and a filter reads a column.

---

## 5. The project label comes from the aggregator, not the docs

A `project:` key in each doc's frontmatter would be wrong:

- it leaks the aggregator's concern into every repo's docs,
- it requires every project to cooperate,
- it breaks if a repo reorganizes,
- and it makes the doc non-portable.

The aggregator knows which files belong to which project, so it passes the
label **in**, at annotation time, via `--project`.
The source markdown is never modified.

This is the same principle as `# newdoc title` and the lexicon: metadata the
aggregator owns stays out of the source.

---

## 6. What is missing

Three small additions, each the same shape as what already exists
(`weight`, `lemma_override`):

| Piece | Where | Shape |
|---|---|---|
| `--project <name>` | `slovo-annotate` CLI | stamps the project onto every document it produces |
| `project` on `document` | `slovo-core` schema | a column, additive migration (v5) |
| `[project="…"]` | `slovo-core` query language | one new attribute in the matcher |

Carried through CoNLL-U the same way `# newdoc title` is (a comment), so the
core gains no dependency on the annotator.

Once in place, `kwic`, `freq`, `query` and `docs` are all filterable by project.

---

## 7. Markdown is the whole input

No new front-end is needed; the markdown support built for the songbook example
covers this case.

- **Frontmatter** supplies the document title.
- **Headings** become weighted sections, so a word in a heading outranks one in
  body text (`freq --rank weight`). Useful for docs: a term in a section title
  is usually the better hit.
- **Fenced code is a weight tier, not excluded** — see §8.
- **One file is one document**; its sections share it.

Note the tension with the songbook: there, chord charts are excluded; here, code
is content. So "code" must be a per-block-type rule in the weight scheme, not a
blanket "all fenced blocks are dropped".

---

## 8. Code blocks: lower weight, not skipped

Code is content. A docs search that silently drops code means "I know it is
there but search says no", which is unintuitive.

So code becomes a tier, like headings:

```
h1     6
h2     5
h3     4
body   1
code   0.3     # present, searchable, ranked low
```

It stays **data**, so a corpus can set `code = 0` to exclude it, or `chords =
drop` for the songbook, without a code change.

The scheme file grows a per-block-type rule:

```
# weights.conf
h1 = 6
h2 = 5
body = 1
code = 0.3
```

---

## 9. Cost

Measured on a release build (see the numbers in the session that produced this):

- **Annotate:** ~1.7M tokens/sec. A docs aggregator annotates in seconds.
- **Import:** ~320k tokens/sec; ~110 bytes of DB per token. 1M tokens ≈ 110MB.
- **Query:** linear in corpus size (full-token scan), not in matches.
  1M tokens ≈ 0.4s. A personal docs corpus (100k–1M tokens) is 15–400ms.
- **Memory:** ~9MB peak for a 1M-token query.

Comfortable with large headroom on a cheap VPS.
The escape hatch if reads ever outgrow SQLite is in `DESIGN.md` §5 (DuckDB or a
CWB-style inverted index behind the same `Store` API); it is not needed at this
scale.

---

## 10. Deployment

```
songbook-less VPS:
  docs-store.service   ← archives arrive, triggers rebuild
  slovo-annotate       ← runs per project, writes .conllu, imports .db
  slovo-serve.service  ← reads .db, serves HTTP
```

Things to get right:

1. **Rebuild the whole dataset each run.** The `.conllu` and the `.db` are
   disposable build artifacts. Ingest *skips* an already-imported file, so
   editing a doc would otherwise be ignored; rebuilding avoids needing an
   update path.
2. **Atomic swap.** Build into `docs.db.new`, then `mv` into place, so the
   read service never sees a half-written file.
3. **Auth.** The ingest path is a write path; the read API can be open.
   If the portfolio proxies search, `slovo serve` can stay on localhost and the
   auth problem disappears entirely.
4. **Deep links.** Hits carry `source_url` and `sent_id`, so a result links to
   a document. Linking to the exact line (`docs/pcb.md#L42`) is not implemented
   and would be a small addition.

---

## 11. Not in scope

- **The songbook / sirmidlaq site.** Songs are not docs and do not go through
  this pipeline; no search there. The songbook stays an example fixture.
- **CI-side tokenization.** Rejected: GitLab runners are Docker-based and not
  the author's hardware. The VPS tokenizes, as a job, which is not the same as
  the *server* tokenizing.
