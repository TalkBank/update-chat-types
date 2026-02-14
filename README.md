# Update @Types header in CHAT files

[![Rust](https://github.com/TalkBank/update-chat-types/actions/workflows/rust.yml/badge.svg)](https://github.com/TalkBank/update-chat-types/actions/workflows/rust.yml)

A CLI tool that keeps `@Types` headers in [CHAT](https://talkbank.org/manuals/CHAT.html) corpus files (`.cha`) in sync with their canonical values defined in `0types.txt` files.

## How it works

Each directory containing `.cha` files can have a `0types.txt` file that defines the correct `@Types` header for that directory. Subdirectories without their own `0types.txt` inherit from the nearest ancestor that has one.

The tool walks the directory tree, reads the `0types.txt` files, then for each `.cha` file:

- **Replaces** the `@Types` line if it differs from the canonical value
- **Inserts** the `@Types` line if the file doesn't have one
- **Skips** the file if the header already matches

## Installation

### As a pre-commit hook (recommended)

The tool is designed to run as a git pre-commit hook in `*-data` repos. Run the bootstrap script once per machine:

```bash
# Build from source and install hook + binary:
./bootstrap.sh

# Or with a pre-built binary (for machines without Rust):
./bootstrap.sh --binary /path/to/update-chat-types
```

This does:
1. Installs the binary to `~/.talkbank/bin/update-chat-types`
2. Installs the hook to `~/.talkbank/hooks/pre-commit`
3. Sets `git config --global core.hooksPath ~/.talkbank/hooks`
4. Adds `~/.talkbank/bin` to PATH in your shell profile

After bootstrap, every `git commit` in a `*-data` repo will automatically fix `@Types` headers and include the fixes in the commit. Non-data repos are unaffected (the hook exits silently).

For remote setup via SSH:
```bash
scp update-chat-types user@machine:~/
ssh user@machine 'bash -s' < bootstrap.sh --binary ~/update-chat-types
```

### Manual install

```bash
cargo install --path .
```

## Usage

```bash
# Update all .cha files under a directory
update-chat-types --chat-dir /path/to/corpus

# Preview changes without modifying files
update-chat-types --chat-dir /path/to/corpus --dry-run
```

Output lists each modified file path relative to `--chat-dir`:
```
Updated 3 CHAT files:
  Eng-NA/Bates/010600a.cha
  Eng-NA/Bates/010600b.cha
  Eng-NA/Brown/eve01.cha
```

### Pre-commit hook behavior

The hook in `hooks/pre-commit`:
- **Only activates** in repos whose name matches `*-data`
- **Gracefully degrades** — if the binary isn't on PATH, prints a warning and allows the commit
- **Auto-stages fixes** — runs `git add -u -- '*.cha'` so @Types corrections are included in the commit
- **Only blocks commits** on tool errors (e.g., malformed `0types.txt`)

## Example

Given this directory structure:

```
corpus/
├── 0types.txt          # @Types:	long, toyplay, TD
├── session1.cha        # will use "long, toyplay, TD"
├── narratives/
│   ├── 0types.txt      # @Types:	long, narrative, TD
│   └── story1.cha      # will use "long, narrative, TD"
└── freeplay/
    └── play1.cha       # inherits "long, toyplay, TD" from parent
```

Running `update-chat-types --chat-dir corpus` updates all `.cha` files to match their respective `0types.txt`.

## Development

### Commands

```bash
cargo check            # Type-check without building
cargo test             # Run all unit + integration tests
cargo build            # Debug build
cargo build --release  # Optimized release build (LTO + stripped)
cargo bench            # Run Criterion benchmarks
cargo insta review     # Review pending snapshot changes
```

Run a single test:
```bash
cargo test <test_name>   # e.g. cargo test test_get_types
```

### Architecture

- **`src/main.rs`** — CLI entry point using `clap`. Calls `update_types_in_place()` and prints modified file paths.
- **`src/lib.rs`** — Core library. All public functions return `anyhow::Result`.
- **`hooks/pre-commit`** — Git pre-commit hook (installed via `core.hooksPath`).
- **`bootstrap.sh`** — One-time setup script to install the binary and hook.

Public API (4 functions):
- `get_types(&Path) -> Result<Option<String>>` — extract `@Types` header from a `.cha` file (streaming, stops after 30 lines or first utterance)
- `read_types_file(&Path) -> Result<String>` — read the `@Types` value from a `0types.txt` file
- `update_types_to_new_path(&Path, &Path, &str, bool) -> Result<bool>` — update a single file's `@Types` header via atomic temp file write
- `update_types_in_place(&Path, bool) -> Result<Vec<PathBuf>>` — orchestrator: walk directory, collect type mappings, update all `.cha` files, return paths of modified files

Key internal helper:
- `classify_header_line(&str, &str) -> HeaderAction` — pure function that classifies each header line as `Replace`, `AlreadyOk`, `Splice`, or `Continue`

### Performance

This tool is designed to be fast enough for use as a pre-commit hook, even on large corpora with thousands of files.

**Zero-cost unchanged files.** When a file's `@Types` already matches the canonical value, the tool reads only the header (~14 lines), determines no change is needed, and moves on. No temp file is created, no bytes are copied. This is the common case for pre-commit hooks where most files are already correct.

**Single directory walk.** The entire directory tree is traversed once with `WalkDir`. During that single pass, the tool simultaneously builds the type inheritance map, collects `0types.txt` locations, and gathers all `.cha` file paths. The previous implementation walked the tree twice.

**Raw byte copy after header.** When a file does need updating, only the header prefix (~14 lines) is parsed line-by-line. Once the `@Types` decision is made, the entire remainder of the file — which can be thousands of lines of transcript — is copied as raw bytes via `io::copy`, with no per-line UTF-8 decoding or re-encoding.

**No regex.** All header matching uses Rust byte-prefix patterns (`[b'@', b'T', b'y', b'p', b'e', b's', b':', ..]`), avoiding the cost of compiling and executing regex automata. This also eliminates `regex`, `regex-automata`, `regex-syntax`, and `aho-corasick` as dependencies.

**Atomic file writes.** Modified files are written to a `NamedTempFile` created in the same directory as the target, then atomically renamed via `persist()`. This prevents partial writes and avoids cross-device rename errors.

### Test structure

- **Unit tests** (`src/lib.rs`) — rstest parameterized tests for `classify_header_line`, `get_types`, `read_types_file`
- **Integration tests** (`tests/integration.rs`) — mutation tests using `TempDir` for filesystem isolation: replace, splice, noop, dry run, full directory walk, edge cases
- **Snapshot tests** (`tests/snapshots/`) — insta snapshots for replace and splice output verification

### Test data

- `fixtures/*.cha` (`small-types.cha`, `big-types.cha`, `no-types.cha`, `tiny-types.cha`) — unit test fixtures
- `fixtures/test-dir/` — nested directory structure with `0types.txt` and `.cha` files for testing directory inheritance

### CHAT format constraints

- The `@Types` header is always within the first ~30 lines of a `.cha` file, before any utterance lines (lines starting with `*`)
- `0types.txt` files contain the canonical `@Types:` value for all `.cha` files in that directory (and subdirectories without their own `0types.txt`)
