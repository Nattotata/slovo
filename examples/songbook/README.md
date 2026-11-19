# Example: a songbook

A worked example of the markdown → corpus pipeline, using a real songbook:
35 song files, each with YAML frontmatter, `##`/`###` section headings, chord
charts in fenced code, and inline chord markers (`::G::`) spliced into the
lyrics.

It is here to show what `slovo annotate` does with messy real-world markdown,
and to give the tests and the justfile something concrete to run against.

## Run it

```console
$ just songbook-build        # annotate songs/*.md, import as dataset `songs`
$ just songbook docs         # list the songs
$ just songbook kwic 'sleep' # concordance
$ just songbook doc 1        # one whole song
$ just songbook freq -n 10 --rank weight
```

The built dataset lives in `examples/songbook/.build/` (gitignored), so it never
touches your real Slovo data directory.

## What the annotator handles here

| In the source | Becomes |
|---|---|
| `title: "Rhino"` frontmatter | the document title |
| `## Verse` | a weighted section (weight 5) |
| ```chords fenced block | excluded (chord notation, not lyrics) |
| `**Chords** D **Chords**` | dropped (notation, not text) |
| `::G::Hello` | `Hello` |
| `mag::B::gots` | `maggots` (markers spliced mid-word rejoin) |
| `don't` | `do` + `n't` (contraction splitting) |

One file is one document; its sections share it and number continuously. Search
results carry the source URL and the sentence id, so a hit traces back to the
song it came from.

## Why it is a good test

It is deliberately awkward: the chord syntax is not standard markdown, the
markers sit inside words, and the structure is inconsistent between files
(`nemesis.md` uses a different chord convention than `sleep.md`). Plain,
well-behaved markdown is the easy case; this is the one that finds the bugs.

## Note

These are the author's own songs. They are committed as a test fixture, not as
corpus content to redistribute.
