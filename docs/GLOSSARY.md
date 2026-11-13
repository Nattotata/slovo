# Slovo — Glossary

Terms used in this project's docs.
Grouped by topic, not alphabetically, so it reads in dependency order.
Examples use `UD_English-EWT` (254,820 tokens) where useful.

Assumed known: coherence vs cohesion, Gricean maxims, phonology/morphology basics.

---

## 1. Text and tokens

**Corpus** (plural *corpora*)
A body of text collected for linguistic analysis.
"Corpus" is a *use*, not a format: Slovo is built for the ones researchers want to query.

**Token**
A single occurrence of a word, punctuation mark, or other unit in running text.
`the cat sat .` is four tokens.
Slovo stores one `token` row per token, with an integer id that is its position in the corpus.

**Type**
A distinct vocabulary item, as opposed to occurrences of it.
`the cat sat on the mat` has 6 tokens but 5 types (`the` occurs twice).

**Type/token ratio (TTR)**
Types ÷ tokens.
A crude lexical-diversity measure; very sensitive to text length.

**Surface form**
The string exactly as it appears: `Took`.
Contrast with *lemma*, which is `take`.

**Tokenization / tokenizer**
Splitting raw text into tokens, deciding what counts as a word.
Punctuation is normally its own token, contractions are torn apart (`don't` → `do` + `n't`), and some tokens are multiword (*see below*).
Tokenization is the least glamorous annotation layer and the one that most affects every number downstream.

**Multiword token**
A surface sequence that behaves as one unit but is annotated as several, e.g. Spanish `vámonos` = `vamos` + `nos`.
In CoNLL-U these get a range id like `1-2` and Slovo skips that row, keeping the components.

**Empty node**
A CoNLL-U row with a decimal id (`5.1`) representing a word that is not overtly in the text (e.g. an elided subject).
Slovo skips these.

**Sentence**
A token span between sentence boundaries.
In CoNLL-U a sentence is a block of token lines separated by a blank line.

**Document**
The unit of provenance: one source text, with a title, URL, and license.
In Slovo, importing a file creates one document; importing a directory creates one per file.

---

## 2. Annotation

**Annotation** (n.) / **annotate** (v.)
Linguistic information attached to text: lemmas, POS tags, syntax, and so on.
An **annotated corpus** is text plus at least one annotation layer.

**Gold** (gold standard)
Annotation produced or verified by humans, as opposed to automatically.
Gold data is small, expensive, and the reference everyone measures against.
UD treebanks are gold; Slovo's v1 relies on them precisely so it needs no tagger of its own.

**Lemma** / **lemmatization** / **lemmatizer**
The lemma is the dictionary form of a word: `take` for *take, takes, took, taken, taking*.
Lemmatization is the process; a lemmatizer is the tool.
Lets you query `take` and catch all its inflections — essential for Slavic languages, where the same lexeme has far more forms than in English.

**Stem** / **stemming**
A cruder alternative: chopping affixes by rule (`studies` → `studi`).
Fast, no lexicon needed, but the output is not a real word.
Do not confuse a stem with a lemma.

**Part of speech (POS)**
Word class: noun, verb, adjective, and so on.
A POS tagger assigns them.

**UPOS / XPOS**
UPOS is the 17-tag *universal* set, shared across languages:
`NOUN, PROPN, VERB, ADJ, ADV, PRON, DET, ADP, AUX, CCONJ, SCONJ, NUM, PART, PUNCT, SYM, INTJ, X`.
XPOS is the language-specific tagset (`NN`, `VBD`, `JJ`, …).
Because UPOS is language-independent, a Slovo query on UPOS works unchanged on English, Slovak and Czech.
That is the whole reason UPOS is in the schema.

**Morphological features** (CoNLL-U `FEATS`)
Key–value morphology bundled into one column: `Number=Sing|Tense=Past`.
Finer-grained than POS, coarser than a full morphological analyser.

**Dependency** / **head** / **dependent**
A syntactic relation between two words, one of which (the *head*) governs the other (the *dependent*).
In `The cat sat`, `sat` is the head of `cat`.

**Dependency label** (CoNLL-U `DEPREL`)
The *name* of that relation: `nsubj` (nominal subject), `obj`, `det`, `amod`, `punct`, `root`.

**Dependency parsing**
Deriving the head/dependent structure automatically.

**Treebank**
A corpus annotated with syntactic structure, as trees (historically constituency trees; UD uses dependency trees).

---

## 3. Corpora and sources

**Universal Dependencies (UD)**
A project standardizing tokenization, lemmas, POS and syntax across 200+ languages.
Most useful property here: the **universal tagset**, so one schema fits many languages.
Slovo's v1 target is `UD_English-EWT`.

**Treebank names** (as used in the docs)
`EWT` — English Web Treebank, the English UD treebank; ~255k tokens.
`SNK` — Slovak National Corpus treebank.
`PDT` — Prague Dependency Treebank (Czech; note: CC BY-NC-SA, non-commercial).
`CAC`, `CLTT` — two more Czech treebanks, both CC BY-SA, so usable where PDT's NC clause is a problem.

**Genre / register**
The kind of text: weblog, news, email, review, fiction, speech.
`EWT` explicitly mixes five web genres, which makes it bad for `take *` frequency claims about English *in general* and fine for claims about *web English*.

**Balanced corpus**
A corpus whose genres are sampled in known, roughly equal proportions (Brown, BNC, COCA).
Distinct from a convenience sample like "whatever Wikipedia had".

**Subcorpus**
A slice of a corpus selected by metadata (year, genre, dialect).
Subcorpora are the input to *keyness* comparisons.

---

## 4. Formats and standards

**CoNLL-U**
The standard plain-text exchange format for UD treebanks.
One token per line, 10 TAB-separated columns:
`ID, FORM, LEMMA, UPOS, XPOS, FEATS, HEAD, DEPREL, DEPS, MISC`.
`#` lines are comments (`# text = …`); blank lines separate sentences.
Slovo's importer reads this and ignores `DEPS`/`MISC`.

**Verticalized text** / **`.vrt`**
One token per line, annotations in extra TAB-separated columns.
The native input format of the IMS Corpus Workbench, hence the CWB-compatible format.
Slovo's design uses it as the canonical intermediate format between "parse" and "store", because it keeps the importer independent of the source format.

**IMS Corpus Workbench (CWB)**
A long-standing corpus storage and query engine, optimized for billion-word corpora.
Stores the corpus as an ordered stream of token positions, each carrying *positional attributes*.
Its influence shows up in two things: Slovo's `token` table is that same model expressed in SQL, and CWB's query language is the ancestor of CQL.

**Positional attribute** (p-attribute)
A token-level annotation column in CWB: `word`, `lemma`, `pos`.
So UPOS/lemma/form are p-attributes; each is one column and one value per corpus position.

**Corpus position**
The integer index of a token in the corpus, in reading order.
In Slovo this is `token.id`.
Everything about concordance and windows reduces to arithmetic on positions.

**CQL / CQP syntax**
Corpus Query Language, the query syntax of CWB and Sketch Engine.
Conditions in brackets, quantifiers after them:
`[lemma="take"][upos="ADV"]{0,2}[upos="VERB"]`
means "a lemma *take*, followed within 0–2 adverbs by a verb".
It has been the de facto standard for decades; Slovo plans a subset rather than inventing a language.
Note the name collision: CQL also denotes a completely unrelated Library of Congress query language, and the CWB authors prefer "CQP syntax" for this one.

**TEI** / **XCES**
XML schemas for encoding texts and corpus annotation.
Relevant only when Slovo adds XML importers; they are interchange formats, not query formats.

---

## 5. Retrieval

**Frequency list**
Words (or lemmas, or tags) ranked by how often they occur.
`slovo freq --by lemma`.

**Normalized frequency** / **per million**
Raw counts are not comparable across corpora of different sizes, so they are scaled:
`count / total × 1,000,000`.
In `EWT` (254,820 tokens), `the` occurs 11,030 times → 43,285.5 per million.
Slovo prints both.

**Concordance**
A list of every occurrence of a search term *in context*, usually one hit per line.

**KWIC** (Key Word In Context)
The display convention for concordances: the search term is centered, with a fixed amount of context on each side.
```
… I told him to   take   off his coat …
… the plane will   take   off at noon …
```

**Node** / **collocate**
Node: the word you are studying.
Collocate: a word that co-occurs with the node more than chance would predict.
`take` is the node; `off` is a collocate.

**Window** / **span**
The span of tokens around the node searched for collocates, e.g. ±5.
Wider windows catch looser association; narrower ones catch fixed phrases.

**N-gram**
A sequence of *n* consecutive tokens: bigram (2), trigram (3).
Purely contiguous and surface-based; no notion of "meaningful".

**Cluster**
BYU's term for a contiguous sequence of 2–7 words.
Effectively an n-gram with a size range, used for the "list all phrases containing this word" view.

**Pattern** / **wildcard**
A query with an open slot.
`take *` means "the token `take` followed by any single token".
Slovo treats this as: find `take`, capture what follows, count and rank the captures.
It is not substring matching — `*` stands for tokens, not characters.

---

## 6. Statistics

**Association measure**
A formula scoring how strongly two words co-occur, correcting for the fact that frequent words co-occur with everything.
The numbers below assume a node `w1` and candidate collocate `w2`, with

- `O11` = observed co-occurrences inside the window
- `R1` = frequency of `w1`
- `C1` = frequency of `w2`
- `N` = the total over which frequency is measured

and the 2×2 table

```
              w2         ¬w2          row total
w1            O11        R1 - O11     R1
¬w1           C1 - O11   O22          N - R1
col total     C1         N - C1       N
```

with `O22 = N - R1 - C1 + O11` and the expected count `E11 = R1 × C1 / N`.

Caveat worth internalizing: what `N` means differs by tool (total tokens? total windows? total pairs?).
Change that and MI and log-likelihood change too, which is why cross-tool collocation numbers rarely match.

**Mutual information (MI)**
`MI = log2(O11 / E11)`
Measures association strength.
0 means "exactly as often as chance"; positive means more.
Badly inflated by rare pairs, since a single co-occurrence of two rare words yields a huge score.

**t-score**
`t = (O11 - E11) / √O11`
Favors frequent pairs and is stable on small samples.
The opposite bias to MI; reporting both is standard.

**Log-likelihood** (Dunning's G², `LL`)
`G² = 2 × Σ O_ij × ln(O_ij / E_ij)` over the four cells, with `0 × ln 0 = 0`.
A significance test rather than an effect size: it tells you a co-occurrence is *not* chance, not *how* strong the bond is.

**Dice**
`Dice = 2 × O11 / (R1 + C1)`
A normalized co-occurrence coefficient in [0, 1].

**logDice**
`logDice = 14 + log2(Dice)`
Sketch Engine's measure.
Bounded at 14 and, crucially, comparable across corpora of different sizes — which MI and t-score are not.

**Keyness** / **keyword analysis**
Comparing a subcorpus against a reference corpus to find words that are unusually frequent (or unusually rare) in it.
"Keywords" here means *statistically distinctive*, not *important*.

**Log ratio**
`log2( (f_target / N_target) / (f_reference / N_reference) )`
The usual effect-size companion to a keyness test: positive means over-represented in the target.

**Dispersion**
How evenly a word is spread through a corpus, as opposed to how often it occurs.
A word can occur 1,000 times but all in one document — frequency alone hides that.
**DP** (deviation of proportions, Gries): split the corpus into `n` equal parts, `DP = 0.5 × Σ |f_i/F - 1/n|`; 0 is perfectly even, 1 is maximally clustered.
**Juilland's D**: `1 - (SD / mean)` of the per-part frequencies; higher is more even.
Needs metadata (document boundaries, dates) to be meaningful, which is why Slovo stores `document` and `sentence` separately from `token`.

---

## 7. Tooling terms (not linguistic)

**SQLite**
An embedded SQL database — a single file, no server.
Slovo uses one file per dataset.

**SQL**
The query language for relational databases.
Frequency lists are SQL `GROUP BY` over the `token` table.

**FTS5**
SQLite's full-text search extension.
Indexes whole documents, so it cannot answer "what appears next to `take`" — that needs token positions, which is why Slovo's schema has them.

**crate** / **workspace**
Rust's unit of compilation (crate) and a set of crates built together (workspace).
Slovo has `slovo-dataset` (manifest/catalog types), `slovo-core` (engine), `slovo-fetch` (downloader), `slovo-cli` (binary), and later `slovo-tauri` (GUI).

**Dataset** (in Slovo)
A directory of source files plus a `manifest.json`.
The app imports the directory; it does not know which corpus it came from.

**Manifest**
The `manifest.json` inside a dataset directory: name, language, license, source, version, and file list.
The app reads it to fill in import metadata.

**Catalog**
A separate JSON file listing datasets that can be downloaded, with their URLs and checksums.
The app reads it from a local path or a URL; it is not hard-coded in the app.

**Downloader**
The `slovo-fetch` crate. It reads the catalog, downloads the files, and writes a dataset directory.
It does not know about the app's database.

**Tauri** / **SvelteKit**
The desktop shell (Rust + a system webview) and the frontend framework for the GUI.

**cargo-dist**
A tool that builds and publishes release binaries for a Rust project.

**SPDX expression**
A machine-readable license identifier.
`MIT OR Apache-2.0` means the user picks either.
