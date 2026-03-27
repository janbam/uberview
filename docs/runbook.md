# Uberview Runbook

## Purpose

This runbook covers the routine operational loop for Uberview: validate changes, inspect regressions, and prepare a release-quality binary.

## Local Validation

Run the full quality gate before pushing or merging:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

For a quick manual smoke test:

```bash
cargo run -- tests/fixtures/sample_project
```

## Release Build

Build the optimized binary locally:

```bash
cargo build --release
```

The resulting binary is:

```text
target/release/uberview
```

## Troubleshooting

### Output looks too sparse

- Confirm the input file uses a supported language.
- Check whether the file only contains executable statements with little structural surface.
- Run the CLI on the corresponding fixture sample to compare expected behavior.

### A directory scan seems incomplete

- Confirm the missing file is not under a default ignored directory such as `target/` or `node_modules/`.
- Check whether the file is extensionless and lacks a supported shebang.
- Run the CLI directly on the file to isolate detection from traversal.

### Syntax errors produce little output

- Uberview uses concrete syntax trees and will attempt recovery.
- Heavily malformed files may still reduce to `(no retained structure)` if no safe retained items remain.

### CI fails on formatting or linting

- Run the three local validation commands from this document.
- Fix `cargo fmt` issues first, then address clippy findings, then rerun tests.

## Change Review Checklist

- The reduced output still reads like source rather than metadata.
- Definition line ranges are still correct after parser changes.
- Comments, docstrings, and exit surfaces remain in source order.
- Directory scans remain deterministic.
- New grammar handling is covered by fixture-based tests.
