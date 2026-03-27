# Open Tickets

Tickets captured on 2026-03-27 after reviewing TreeBrief output for `/home/jan/work/life/src/life_cli/services/proposals.py`.

## Active

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

- TB-003 Hide top-level assignments by default and replace them with consolidated skipped-range markers. Closed on 2026-03-27 after validating the placeholder behavior and the `--show-top-level-symbols` escape hatch against fixtures and `proposals.py`.
- TB-002 Make return rendering opt-in and limit it to true returns. Closed on 2026-03-27 after validating the default suppression and `--show-returns` behavior against fixtures and `proposals.py`.
- TB-001 Rework definition headers, line ranges, and spacing. Closed on 2026-03-27 after validating the new header contract, line-number flag, docs, and `proposals.py` output.

## Deferred

- None.
