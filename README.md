# TreeBrief

TreeBrief is a CLI that prints a compact, source-ordered structural overview of source files and codebases.

It is designed for fast codebase orientation:

- definitions keep their original source order
- every retained definition carries a line range
- comments and docstrings are preserved generously
- actual return statements are available with `--show-returns`
- top-level assignment/constant blocks collapse by default and can be restored with `--show-top-level-symbols`
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
cargo run -- --show-line-numbers-for-all-items src/lib.rs
cargo run -- --show-returns src/lib.rs
cargo run -- --show-top-level-symbols src/lib.rs
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

[1-12] Class: Example
class Example:
    """Docs."""
    [5-8] Method: run
    def run(self) -> int:
```

Rules:

- file sections start with `=== <path> ===`
- each retained definition starts with `[start-end] Kind: name`
- single-line definition ranges collapse to `[n]`
- the retained signature and decorators stay below the synthetic header in source-like form
- retained definition blocks are separated by one blank line
- retained content stays in source order
- nested retained items keep their relative indentation
- non-definition retained lines stay unnumbered by default
- `--show-line-numbers-for-all-items` extends bracketed line numbers to all retained items
- default output omits return-like control-flow lines
- `--show-returns` restores actual `return` statements without restoring `raise`, `yield`, or `throw`
- default output collapses adjacent top-level assignment/constant-style symbols into skipped-range markers
- `--show-top-level-symbols` restores those top-level symbol definitions explicitly
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

The repository pins Rust `1.88.0` via [`rust-toolchain.toml`](/home/jan/work/treebrief/rust-toolchain.toml) so local runs and CI use the same lint and formatting surface.

The test suite includes:

- exact single-file output snapshots for all supported languages
- deterministic directory-scan assertions
- shebang detection for extensionless scripts
- opt-in return-surface assertions
- syntax-recovery coverage
- concurrent end-to-end execution checks

## Repository Layout

- `src/` contains the CLI, traversal, detection, extraction, and rendering code.
- `tests/fixtures/` contains the multi-language fixture corpus and expected outputs.
- `DEV/` contains the durable session state, decisions, validation matrix, and open issues.

## Current Scope

TreeBrief is intentionally a reduced-source mapper, not a semantic analyzer.

Notable v1 constraints:

- no deep filtering or shaping controls beyond the small retained-surface flags above
- no semantic resolution or type checking
- no machine-readable output contract
- no build-system awareness
