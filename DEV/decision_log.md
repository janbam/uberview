# Decision Log

## 2026-03-27

- Use Rust as the implementation language because the spec explicitly recommends a Rust + `tree-sitter` stack and the repo is otherwise greenfield.
- Keep the internal model intentionally thin: file sections, definition items, and verbatim retained fragments ordered by source position.
- Prefer a deterministic local-first implementation with GitHub-backed milestone integration when the repo workflow is ready, rather than waiting on remote setup before coding starts.
- Preserve indentation during rendering by reconstructing the leading whitespace that tree-sitter node spans omit on the first retained line.
- Include opening body delimiters such as `{` or `(` in retained headers when the grammar exposes the body as a separate child node.
- Lock the CLI contract with fixture-based exact output tests per language, then cover traversal, shebang detection, syntax recovery, and concurrent execution at the binary boundary.
- Pin the repository toolchain to Rust `1.88.0` so local development and CI share the same `clippy`/`rustfmt` behavior instead of drifting with `stable`.
