# Open Tickets

Tickets captured on 2026-03-27 after reviewing TreeBrief output for `/home/jan/work/life/src/life_cli/services/proposals.py`.

## Active

### TB-002 Make return rendering opt-in and limit it to true returns

Goal:
- Reduce noise from exit-like lines by showing actual function returns only when explicitly requested.

Requirements:
- Default output omits retained return lines.
- Add `--show-returns` to surface actual `return` statements.
- When enabled, show all actual returns, including early and nested returns.
- Do not treat `raise`, `yield`, `yield from`, `throw`, or similar exit-like constructs as returns.

Implementation Notes:
- The extraction rules and the product spec currently talk about broader exit surfaces, so this ticket requires both implementation and spec updates.
- Fixture output for at least Python and one non-Python language should pin the new policy.

Done When:
- `proposals.py` no longer shows `raise` lines in default output.
- `--show-returns` restores actual `return` statements without restoring `raise` or `yield`.
- CLI tests cover default and opt-in return behavior.
- `README.md` and `DEV/SPEC.md` reflect the new return policy.

### TB-003 Hide top-level assignments by default and replace them with consolidated skipped-range markers

Goal:
- Keep large files readable by default when they contain long runs of top-level constants or assignments.

Requirements:
- Hide top-level assignment and constant-style symbols by default.
- Replace each contiguous skipped run with one count-only placeholder line.
- Use a shape like `[42-73] Skipped top-level assignments/constants (13 items)`.
- Consolidate adjacent skipped items of the same kind into one placeholder instead of emitting one placeholder per line.
- Keep skipped-range markers in source order so omitted structure stays visible.
- Add `--show-top-level-symbols` as an escape hatch that restores explicit top-level symbol rendering.

Implementation Notes:
- This ticket changes the default Python behavior shown by `PROPOSAL_*` style constant blocks.
- The skip-marker logic should remain deterministic and avoid misleading empty gaps.

Done When:
- `proposals.py` default output collapses the `PROPOSAL_*` block into a single consolidated skipped-range line.
- `--show-top-level-symbols` restores the top-level symbol entries.
- Snapshot and CLI tests cover both default and escape-hatch behavior.
- `README.md` and `DEV/SPEC.md` document the default suppression and override flag.

### TB-004 Accept multiple input paths with stable ordering and deduplication

Goal:
- Let one TreeBrief invocation render several files or directories without repeated runs or duplicate output.

Requirements:
- Accept one or more positional paths, for example `treebrief path1 path2 path3`.
- Preserve the user-provided root order.
- Keep the existing deterministic traversal within each directory root.
- Deduplicate overlapping inputs so the same file is rendered once even if multiple roots reach it.

Implementation Notes:
- The CLI, input-resolution layer, and top-level app orchestration will all need small updates.
- Directory scans should remain deterministic even when mixed with explicit file arguments.

Done When:
- TreeBrief accepts multiple positional paths in one invocation.
- Output order follows input-root order, then deterministic in-root ordering.
- Overlapping roots or repeated file arguments do not produce duplicate sections.
- CLI tests cover mixed file and directory input, stable ordering, and deduplication.

## Completed

- TB-001 Rework definition headers, line ranges, and spacing. Closed on 2026-03-27 after validating the new header contract, line-number flag, docs, and `proposals.py` output.

## Deferred

- None.
