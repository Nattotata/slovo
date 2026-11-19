# List the available recipes.
default:
    @just --list

# Build the workspace (debug).
build:
    cargo build --workspace

# Build the workspace (release).
build-release:
    cargo build --workspace --release

# Run the test suite.
test:
    cargo test --workspace

# Format all Rust code.
fmt:
    cargo fmt --all

# Check formatting without writing.
fmt-check:
    cargo fmt --all -- --check

# Lint with clippy, warnings as errors.
lint:
    cargo clippy --workspace --all-targets -- -D warnings

# Format, lint and test.
check: fmt-check lint test

# Run the CLI, e.g. `just run -d ewt freq -n 5`.
run *args:
    cargo run -q -p slovo-cli -- {{args}}

# Query a dataset, e.g. `just query ewt 'take *'`.
query dataset pattern:
    cargo run -q -p slovo-cli -- -d {{dataset}} query '{{pattern}}'

# Short alias for `query`.
q dataset pattern:
    cargo run -q -p slovo-cli -- -d {{dataset}} query '{{pattern}}'

# Concordance lines, e.g. `just kwic ewt 'take up'`.
kwic dataset pattern:
    cargo run -q -p slovo-cli -- -d {{dataset}} kwic '{{pattern}}'

# List a dataset's source documents.
docs dataset:
    cargo run -q -p slovo-cli -- -d {{dataset}} docs

# Print a whole source document by id, e.g. `just doc ewt 12`.
doc dataset id:
    cargo run -q -p slovo-cli -- -d {{dataset}} doc {{id}}

# Import a UD treebank as dataset `ewt`, e.g. `just ewt ~/corpora/UD_English-EWT`.
ewt dir:
    cargo run -q -p slovo-cli -- -d ewt import {{dir}} --language en --description "UD English Web Treebank"

# Build the songbook example from examples/songbook/*.md into a local build dir.
# Safe to re-run from scratch; keeps the dataset out of your real data dir.
songbook-build:
    #!/usr/bin/env bash
    set -euo pipefail
    src=examples/songbook
    out="$src/.build"
    rm -rf "$out"
    mkdir -p "$out/conllu"
    for f in "$src"/*.md; do
      name=$(basename "$f" .md)
      [ "$name" = "README" ] && continue
      cargo run -q -p slovo-cli -- annotate "$f" -o "$out/conllu/$name.conllu"
    done
    SLOVO_DATA_DIR="$out/data" cargo run -q -p slovo-cli -- -d songs import "$out/conllu" \
      --language en --source "https://github.com/xof/songbook"
    echo
    echo "Dataset 'songs' built in $out/data."
    echo "Query it with: just songbook kwic 'sleep'  (or docs / doc / freq)"

# Run any slovo command against the songbook example dataset.
# Usage: just songbook <args…>   e.g. `just songbook kwic 'sleep'`, `just songbook docs`.
@songbook *args:
    SLOVO_DATA_DIR=examples/songbook/.build/data cargo run -q -p slovo-cli -- -d songs {{args}}

# List the catalog.
sources:
    cargo run -q -p slovo-cli -- sources

# Annotate raw text into CoNLL-U, e.g. `just annotate book.txt`.
annotate *args:
    cargo run -q -p slovo-cli -- annotate {{args}}

# Annotate a markdown file (weights from its headings).
md file:
    cargo run -q -p slovo-cli -- annotate {{file}} -o {{file}}.conllu

# Serve the HTTP API, e.g. `just serve` or `just serve --port 9000`.
serve *args:
    cargo run -q -p slovo-cli -- serve {{args}}

# Download a dataset, e.g. `just fetch ud/UD_English-EWT`.
fetch id:
    cargo run -q -p slovo-cli -- fetch {{id}}

# Download and import a dataset, e.g. `just add ud/UD_English-EWT -d ewt`.
add id *args:
    cargo run -q -p slovo-cli -- add {{id}} {{args}}

# Install the CLI into ~/.cargo/bin.
install:
    cargo install --path crates/slovo-cli

# Install frontend dependencies.
js-install:
    npm install

# Build the web UI (static) into build/.
ui-build:
    npm run build

# Run the Tauri dev server.
gui-dev: js-install
    npm run tauri dev

# Build the Tauri bundle (uses the static adapter).
gui-build: js-install
    npm run tauri build

# Remove build artifacts.
clean:
    cargo clean
