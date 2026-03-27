# TreeBrief

TreeBrief is a CLI that prints a compact, source-ordered structural overview of source files and codebases.

It is designed for fast codebase orientation:

- definitions keep their original source order
- every retained definition carries a line range
- comments and docstrings are preserved generously
- actual return statements are available with `--show-returns`
- standalone top-level assignment/constant-style symbols are omitted by default and can be restored with `--show-top-level-symbols`
- most ordinary executable code is omitted

The output reads like reduced source, not metadata.

## Supported Languages

- Python
- JavaScript
- TypeScript and TSX
- Rust

TreeBrief auto-detects languages by extension first, then filename patterns, then lightweight shebang sniffing when needed.

## Install And Run

For a quick scan from the repository root:

```bash
cargo run src
```

For explicit argument passthrough, especially when using TreeBrief flags:

```bash
cargo run -- <path> [path...]
```

Examples:

```bash
cargo run src
cargo run -- src/lib.rs
cargo run -- .
cargo run -- src/lib.rs src/app.rs
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
- single-file invocations keep the path spelling you passed, except absolute paths under the current working directory are rendered relative to that directory
- directory scans render paths relative to each scanned root, so `cargo run src` prints `=== app.rs ===` while `cargo run .` prints `=== src/app.rs ===`
- each retained definition starts with `[start-end] Kind: name`
- single-line definition ranges collapse to `[n]`
- the retained signature and decorators stay below the synthetic header in source-like form
- retained definition blocks are separated by one blank line
- retained content stays in source order
- retained container nesting adds one renderer-owned indent level per scope
- original indentation caused only by omitted runtime scopes is stripped before rendering
- nested retained containers gain a blank line before their synthetic header
- non-definition retained lines stay unnumbered by default
- `--show-line-numbers-for-all-items` extends bracketed line numbers to all retained items
- default output omits return-like control-flow lines
- `--show-returns` restores actual `return` statements without restoring `raise`, `yield`, or `throw`
- default output keeps only structural container/callable definition headers plus comments and docstrings
- standalone top-level assignment/constant-style symbols are omitted by default
- `--show-top-level-symbols` restores those top-level symbol definitions explicitly
- one invocation may accept several file and directory roots
- output follows the caller's root order, then deterministic in-root ordering
- overlapping inputs are deduplicated so each file section appears once
- multi-root and directory scans keep going after individual file failures and render `!! parse failed: ...` under the affected file header
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
