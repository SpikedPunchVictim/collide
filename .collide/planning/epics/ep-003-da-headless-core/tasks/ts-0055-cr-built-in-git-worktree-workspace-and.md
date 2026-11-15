---
id: ts-0055-cr
kind: task
status: active
title: "Built-in git worktree workspace and the disk floor"
owners: []
links:
  - { type: blocked-by, to: ts-0050-xc }
---

## Goal

The built-in workspace and the disk floor. `git worktree add` returning `{ path, gitDir }`; removal that refuses a dirty or unmerged worktree; the free-space check before a workspace is made and before each phase; and the disk space a workspace uses, recorded at phase end.

## Success criteria

- A workspace is made on a new branch in a scratch repository and its `{ path, gitDir }` are returned. Removing it while it holds an uncommitted change, or a commit not merged into its base, is refused with a message that says which.
- With free space under the floor, a workspace is not made and a phase does not start, and the run shows `paused: disk` with a Queue item (Stage 3a criterion; 0.2, Durability). The free-space reading is behind a trait, so tests set it.
- The disk space a workspace uses is recorded at phase end (0.13).
- The workspace moves through the `ts-0050-xc` workspace states.

## Tests

Write first: create and remove in a temporary git repository, both refusals, the floor with a fake free-space reader, the size recorded at phase end.

## Notes

- IMPLEMENTATION_PLAN.md, Stage 3a (Built-in workspace; Clocks and disk); 0.2 (Durability); 0.4 (workspace implementations; the built-in one); 0.13 (disk space per workspace).
- Area: `host`. Pack workspace hooks are Stage 7; this is the built-in one only.
- Stage 3a item (revision 13 scope, split 2026-11-15). Items of Stage 3a run one at a time, in id order, because they share `host` and the generated files.
