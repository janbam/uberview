# Test Matrix

## Planned Validation

| Area | Scenario | Status |
| --- | --- | --- |
| CLI | `uberview <file>` produces one file section | Passed |
| CLI | `uberview <directory>` produces deterministic multi-file output | Passed |
| Detection | Extension-based language detection for all supported languages | Passed |
| Detection | Filename and shebang/content sniffing fallback where relevant | Passed |
| Python | Docstrings, decorators, nested defs, multiline signatures, exits | Passed |
| JavaScript | Exported functions, arrow constants, classes, JSDoc, exits | Passed |
| TypeScript | Interfaces, type aliases, namespaces/modules, nested functions | Passed |
| Rust | Structs, enums, traits, impls, macros, associated consts, tail returns | Passed |
| Robustness | Syntax error recovery in a directory scan | Passed |
| Robustness | Broken single-file input exits non-zero with a clear parse error | Passed |
| Robustness | Parse failure line/column reporting is pinned for multiple broken-file shapes | Passed |
| Robustness | Unsupported/generated directories are skipped | Passed |
| Robustness | Concurrent runs do not interfere | Passed |
| Quality | `cargo fmt`, `cargo clippy`, `cargo test` | Passed |

## Notes

- Exact single-file snapshots cover:
  - `tests/fixtures/sample_project/src/python_sample.py`
  - `tests/fixtures/sample_project/src/javascript_sample.js`
  - `tests/fixtures/sample_project/src/typescript_sample.ts`
  - `tests/fixtures/sample_project/src/rust_sample.rs`
- Exact parse-failure assertions cover:
  - `tests/fixtures/sample_project/src/python_broken.py`
  - `tests/fixtures/broken/python_unexpected_token.py`
- Directory validation covers:
  - deterministic relative-path ordering
  - shebang detection for `tests/fixtures/sample_project/scripts/python_tool`
  - explicit parse-failure rendering for `tests/fixtures/sample_project/src/python_broken.py`
  - default ignore behavior for `node_modules/` and `target/`
- Final verified commands:
  - `cargo fmt --check`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test`
  - `cargo run --quiet -- tests/fixtures/sample_project`
