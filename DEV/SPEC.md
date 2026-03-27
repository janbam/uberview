# TreeBrief Specification

## Name

**TreeBrief** is a CLI that prints a compact, source-ordered structural overview of a source file or whole codebase.

The name is intentional:

- **Tree**: it works on a file tree and is built around syntax trees
- **Brief**: it aims to show the smallest useful view that still gives an AI a reliable mental map of the code

## Purpose

TreeBrief should let an AI see the shape of a codebase without reading the full code first.

The output should feel like:

- the original source code
- with most executable code removed
- but with definitions, comments, and docstrings preserved
- and with actual `return` statements available on demand
- and with line ranges on every definition so the AI can do targeted follow-up reads

This is not a semantic analysis tool.
This is not an LSP client.
This is not a summarizer.

It is a reliable structural map.

## Goals

- Work on a single file or a whole directory tree
- Accept one or more input paths in a single invocation
- Auto-detect language
- Support Python, JavaScript, TypeScript, and Rust
- Preserve source order exactly
- Preserve nesting exactly
- Always show line ranges for definitions
- Preserve multiline definitions instead of truncating them
- Preserve docstrings and comments even when they are in slightly non-standard places
- Surface actual `return` statements when explicitly requested
- Be fast, deterministic, and safe to run in parallel
- Work from any current working directory

## Non-Goals

- Type checking
- Symbol resolution across files
- Build-system awareness
- Project-root assumptions
- Heavy configuration
- Deep filtering or shaping controls in v1
- Replacing direct source reads entirely

TreeBrief is meant to guide targeted source reads, not eliminate them.

## Primary Artifact

The primary artifact is human-formatted text.

JSON may exist internally or as an implementation aid, but it is not part of the primary product contract for v1.

## CLI

```text
treebrief [--show-line-numbers-for-all-items] [--show-returns] [--show-top-level-symbols] <path> [path...]
```

Where:

- each `<path>` may be a file or a directory
- a file produces one file overview
- a directory recursively scans supported code files and produces one overview per file
- multiple inputs are processed in the order the user provided them
- overlapping inputs are deduplicated so each concrete file is rendered once

The default invocation must be enough for normal use.

V1 may include a few small formatting or retained-surface toggles, but it should not depend on heavy filtering or restricting flags such as depth limits.

## Supported Languages

V1 must support:

- Python
- JavaScript
- TypeScript
- Rust

Language detection should be automatic.

Detection order:

1. file extension
2. well-known filename patterns
3. shebang or lightweight content sniffing when necessary

Expected file extensions:

- Python: `.py`
- JavaScript: `.js`, `.jsx`, `.mjs`, `.cjs`
- TypeScript: `.ts`, `.tsx`, `.mts`, `.cts`
- Rust: `.rs`

## Directory Traversal

When the input is a directory:

- recursively scan all supported files
- output files in deterministic relative-path order
- keep processing even if one file fails to parse
- report parse failures clearly in output or at the end

Default ignore behavior should skip obvious generated and dependency directories:

- `.git`
- `node_modules`
- `dist`
- `build`
- `target`
- `.venv`
- `venv`
- `vendor`

The tool must not require the user to run it from project root.

When several inputs are provided:

- preserve the caller's input-root order exactly
- keep the existing deterministic ordering within each directory root
- deduplicate overlaps so repeated files or broader later roots do not emit duplicate sections

## Output Contract

### High-Level Shape

Each file is emitted as a separate section.

Example:

```text
=== src/services/proposals.py ===

"""Proposal orchestration helpers."""

[12-148] Class: ProposalService
class ProposalService(BaseService):
    """Coordinates proposal creation and application."""

    [85-131] Method: create_task_create_proposal
    def create_task_create_proposal(
        title: str,
        due_at: datetime | None,
    ) -> Proposal:
        """Create and store a pending task-creation proposal."""
        # Capture duplicate-title context before apply.

        [109-116] Function: normalize_title
        def normalize_title(title: str) -> str:
```

### Formatting Rules

- File sections start with `=== <relative-path> ===`
- Output inside a file is strictly in source order
- Every retained definition block starts with `[start-end] Kind: name`
- Single-line ranges collapse to `[n]`
- Retained container nesting uses renderer-owned indentation, one level per retained scope
- Original indentation caused only by omitted runtime scopes is removed before rendering
- The retained signature and decorators follow the synthetic header as source text
- Nested retained containers get one blank line before their synthetic header
- One blank line follows each retained definition block
- Non-definition retained lines do not carry line numbers by default
- `--show-line-numbers-for-all-items` extends bracketed line numbers to all retained snippets
- Default output omits return-like control-flow lines
- `--show-returns` restores actual `return` statements only
- Default output collapses adjacent top-level assignment/constant-style symbols into skipped-range placeholders
- `--show-top-level-symbols` restores those top-level symbol definitions explicitly
- Non-definition retained lines are shown as source, not relabeled metadata
- Do not print labels such as `header:`, `doc:`, `comment:`, or `return:`
- Do not collapse multiline definitions into one line
- Do not reorder retained lines inside a definition

### Line Number Rules

Definitions must always include a line range.

This applies to:

- top-level definitions
- nested definitions
- local functions
- methods
- classes
- enums
- interfaces
- traits
- impl blocks
- type aliases
- top-level constants and similar named declarations when they are retained as definitions

Definition line ranges render in bracketed form such as `[12-148]`.
Single-line definition ranges collapse to `[12]`.

Non-definition lines such as comments and docstrings do not need line numbers in the default output.
When `--show-line-numbers-for-all-items` is enabled, retained snippets should use the same bracketed range format.

The line ranges are the main coordinate system that let an AI perform targeted follow-up reads.

## Reduced-Source Model

TreeBrief should be thought of as producing a **reduced source view**.

Within each file, it should retain:

- file-level docstrings and file-level comment blocks
- top-level named definitions
- nested named definitions
- docstrings and doc comments attached to definitions
- nearby comment blocks that plausibly describe a definition even if they are not in the language's canonical doc position
- inline and block comments inside retained definitions
- actual `return` statements inside retained definitions when `--show-returns` is enabled
- top-level assignment and constant-style symbol definitions when `--show-top-level-symbols` is enabled

It should omit:

- ordinary executable statements that are not retained comments or opt-in returns
- import lists and use statements unless future versions explicitly decide otherwise
- most local variable assignments
- control flow bodies except for nested definitions, comments, and opt-in returns
- top-level assignment and constant-style symbol runs by default, replacing each contiguous run with one skipped-range placeholder

The result should resemble the source file with most non-structural code removed.

## What Counts As A Definition

V1 should retain named structure-bearing constructs, including as applicable per language:

- module-level functions
- nested and local functions
- async functions
- classes
- methods
- constructors
- enums
- interfaces
- traits
- impl blocks
- namespaces or modules
- type aliases
- top-level constants and exported constants
- class fields or associated constants when they introduce relevant surface area
- macros and macro-like named declarations where they are definition-like

The guiding rule is pragmatic:

If a construct materially contributes to the visible structure of the file, it should appear.

## Top-Level Symbol Suppression

Large files often start with dense runs of assignment-style symbols that are structurally real but visually noisy.

By default, TreeBrief should collapse each contiguous top-level run of assignment or constant-style symbol definitions into one placeholder line:

- shape: `[start-end] Skipped top-level assignments/constants (N items)`
- range: covers the first through last omitted symbol in the run
- order: remains in source order relative to surrounding comments and retained definitions

When `--show-top-level-symbols` is enabled, TreeBrief should restore the explicit symbol definitions instead of the placeholder line.

## What Counts As A Retained Comment Or Docstring

The parser should be forgiving.

Retain comments and docstrings in all of these situations:

- standard documentation position directly above a definition
- standard documentation position directly inside a definition body
- separated from a definition only by blank lines
- separated only by decorators, attributes, annotations, modifiers, or visibility markers
- slightly displaced but still plausibly attached within a small bounded window
- ordinary inline body comments inside a retained definition

The main rule is:

Prefer showing a plausible nearby comment over dropping useful intent.

But also:

- do not let a child definition inherit a parent docstring
- do not scan through another definition boundary
- do not fabricate attachment certainty where the placement is ambiguous

When attribution is ambiguous, preserve the comment in source order instead of pretending it belongs to the wrong symbol.

## Return Surface

Inside retained definitions, TreeBrief should omit return-like control-flow lines by default so the reduced output stays focused on structure.

When `--show-returns` is enabled, V1 should retain:

- actual `return` statements in Python, JavaScript, TypeScript, and Rust
- nested and early `return` statements in source order

V1 should not treat any of these as returns:

- `raise`
- `yield`
- `yield from`
- `throw`
- Rust `?` expressions
- Rust implicit tail expressions

If a retained `return` statement spans multiple lines, emit the full statement in source form.

Retained `return` statements must remain in source order relative to surrounding comments and nested definitions.

## Source Preservation

Retained source should stay as close as practical to the original file.

That means:

- preserve original token text for comments, docstrings, and retained returns
- preserve multiline definitions as written
- preserve retained nesting while allowing rendering-owned indentation

Some normalization is acceptable:

- shared leading indentation may be removed so omitted scopes stop affecting rendered depth
- retained container depth may be rendered with one uniform indent step per level
- trailing whitespace may be removed
- line endings may be normalized

But the textual content should remain recognizably source-like.

## Parsing Strategy

TreeBrief should not use LSP.

V1 should use concrete syntax trees, not semantic tooling.

Recommended implementation:

- language: Rust
- parser core: `tree-sitter`
- grammars:
  - `tree-sitter-python`
  - `tree-sitter-javascript`
  - `tree-sitter-typescript`
  - `tree-sitter-rust`
- directory walker: `ignore`
- CLI parsing: `clap`
- error handling: `anyhow`

Why this stack:

- fast enough for whole-codebase scans
- resilient to syntax errors and incomplete files
- comment-aware and source-slice-friendly
- easy to ship as a single binary
- avoids per-language runtime dependencies

## Internal Extraction Model

The implementation should use a thin internal model.

Suggested concepts:

- file section
- definition node
- retained source fragment

Definition nodes need:

- kind
- name when available
- start line
- end line
- nesting depth
- retained child fragments in source order

Retained source fragments should be enough to render:

- definition header lines
- docstrings
- comment blocks
- opt-in return statements

The model should stay close to source slices rather than inventing a rich abstract schema too early.

## Error Handling

- A parse failure in one file must not abort a directory scan
- A single-file failure should exit non-zero with a clear message
- Empty output must never silently mean "probably failed"
- If a file parses but contains no retained structures, still emit the file header and a clear empty body

## Determinism And Parallelism

The tool should support parallel parsing internally.

But output must remain deterministic:

- files ordered by relative path
- items ordered by source position
- stable formatting across runs

Parallel execution must not depend on temp files named from the current working directory.

## Acceptance Criteria

The tool is successful when all of the following are true:

1. Running `treebrief <file>` prints a reduced-source overview for that file.
2. Running `treebrief <directory>` prints reduced-source overviews for all supported files recursively.
3. Running `treebrief <path1> <path2> ...` preserves root order and deduplicates overlaps.
4. The tool works from any current working directory.
5. Definition lines always include correct line ranges.
6. Multiline definitions are preserved instead of truncated.
7. Nested definitions are indented and remain in source order.
8. Comments and docstrings in slightly non-standard positions are still usually preserved.
9. Default output omits return-like lines, and `--show-returns` restores actual returns only.
10. Default output collapses contiguous top-level assignment/constant-style symbol runs, and `--show-top-level-symbols` restores them explicitly.
11. Output is deterministic.
12. Multiple concurrent runs do not interfere with each other.

## Test Matrix

The test corpus must include at least:

### Python

- module docstring
- class docstring
- method docstring
- nested function
- multiline function signature
- async function
- decorators
- comments inside functions
- hidden-by-default `return`, `yield`, and `raise`
- opt-in `return`
- top-level assignment-run suppression and `--show-top-level-symbols`
- intentionally misplaced but still nearby doc/comment blocks

### JavaScript And TypeScript

- exported functions
- arrow-function constants
- classes and methods
- interfaces and type aliases
- namespaces or modules where applicable
- multiline signatures
- JSDoc above definitions
- body comments
- hidden-by-default `return` and `throw`
- opt-in `return`
- nested functions and closures

### Rust

- module docs
- structs, enums, traits
- impl blocks and methods
- associated constants
- macros
- multiline `where` clauses
- doc comments
- body comments
- hidden-by-default `return`, `?`, and implicit tail-expression returns
- opt-in explicit `return`

### Cross-Cutting

- file outside current working directory
- directory scan from a parent path
- mixed file and directory inputs in one invocation
- overlapping roots and repeated file arguments
- concurrent runs
- syntax error recovery
- generated/dependency directories skipped by default

## Future Work

Possible future extensions, but explicitly not required for v1:

- optional machine-readable output
- finer-grained line-number controls
- import and dependency surface retention
- language support beyond the initial four
- editor integration

## Summary

TreeBrief should print a compact, faithful, source-ordered skeleton of a codebase.

The core contract is simple:

- every meaningful definition appears
- every definition has a line range
- comments and docstrings are preserved generously
- return statements are opt-in and limited to actual `return` statements
- everything stays in source order
- the result reads like source with most code removed
