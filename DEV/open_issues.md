# Open Issues

## Active

- None.

## Deferred

- Control-flow scaffolding is intentionally omitted in v1, so exits retained from elided blocks can appear adjacent to nearby nested definitions without the original `if` or `for` wrapper lines.
- Syntax-recovered files that yield no safe retained fragments currently render as `(no retained structure)` instead of an explicit parse-failure section when tree-sitter recovers without a hard parser failure.
