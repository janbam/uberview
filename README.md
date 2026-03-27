# TreeBrief

TreeBrief is a CLI that prints a compact, source-ordered structural overview of source files and codebases.

It is designed for fast codebase orientation:

- definitions keep their original source order
- every retained definition carries a line range
- comments and docstrings are preserved generously
- explicit exits stay visible
- most ordinary executable code is omitted

The output reads like reduced source, not metadata.

## Supported Languages

- Python
- JavaScript
- TypeScript and TSX
- Rust

TreeBrief auto-detects languages by extension first, then filename patterns, then lightweight shebang sniffing when needed.

## Install And Run

```bash
cargo run -- <path>
```

Examples:

```bash
cargo run -- src/lib.rs
cargo run -- .
```

For a release-style local build:

```bash
cargo build --release
./target/release/treebrief <path>
```

## Output Shape

Each file becomes one section:

```text
=== src/example.py ===

1-12	class Example:
    """Docs."""
5-8	    def run(self) -> int:
        return 1
```

Rules:

- file sections start with `=== <path> ===`
- definition headers are prefixed with `start-end<TAB>`
- retained content stays in source order
- nested retained items keep their relative indentation
- files with no retained structure still emit a section with `(no retained structure)`

## Default Directory Ignore Rules

TreeBrief skips these directories during recursive scans:

- `.git`
- `node_modules`
- `dist`
- `build`
- `target`
- `.venv`
- `venv`
- `vendor`

## Development

Primary checks:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

The test suite includes:

- exact single-file output snapshots for all supported languages
- deterministic directory-scan assertions
- shebang detection for extensionless scripts
- syntax-recovery coverage
- concurrent end-to-end execution checks

## Repository Layout

- `src/` contains the CLI, traversal, detection, extraction, and rendering code.
- `tests/fixtures/` contains the multi-language fixture corpus and expected outputs.
- `DEV/` contains the durable session state, decisions, validation matrix, and open issues.

## Current Scope

TreeBrief is intentionally a reduced-source mapper, not a semantic analyzer.

Notable v1 constraints:

- no filtering flags or output shaping options
- no semantic resolution or type checking
- no machine-readable output contract
- no build-system awareness
