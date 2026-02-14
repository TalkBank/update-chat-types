# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

See README.md for full architecture, API, and test documentation.

## Quick reference

```bash
cargo check          # Type-check
cargo test           # All tests
cargo bench          # Benchmarks
cargo insta review   # Snapshot review
```

## Conventions

- All public functions return `anyhow::Result`. Use `.with_context()` on I/O errors.
- No regex — use byte-prefix matching (`[b'@', b'T', b'y', ...]`).
- All path parameters use `&Path`, not `&str`.
- Integration tests must use `TempDir` for filesystem isolation — never mutate fixture files.
- The `classify_header_line` helper is `pub` (for benchmarks/tests) but is an internal detail.
