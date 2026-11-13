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

# Import a UD treebank as dataset `ewt`, e.g. `just ewt ~/corpora/UD_English-EWT`.
ewt dir:
    cargo run -q -p slovo-cli -- -d ewt import {{dir}} --language en --description "UD English Web Treebank"

# List the catalog.
sources:
    cargo run -q -p slovo-cli -- sources

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

# Run the Tauri dev server.
gui-dev: js-install
    npm run tauri dev

# Build the Tauri bundle.
gui-build: js-install
    npm run tauri build

# Remove build artifacts.
clean:
    cargo clean
