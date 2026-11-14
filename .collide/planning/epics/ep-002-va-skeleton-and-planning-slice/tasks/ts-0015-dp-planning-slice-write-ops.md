---
id: ts-0015-dp
kind: task
status: ready
title: "Planning slice: write ops"
owners: []
links:
  - { type: blocked-by, to: ts-0014-yc }
---

## Goal

The provider ops `setStatus`, `link`, `claim` and `unclaim` through protocol v0 `apply`. Every file write holds a per-root `flock` on a lock file in the root and goes by temp file, `fsync` and rename, so a reader never sees half a file and two writers never interleave. Files are edited line by line, never by re-serializing the YAML. `apply` answers `ok` (the new task, its version, the files touched and `items[]`), `conflict` for a stale `version` (a hash of the file's content), or `refused`. `claim` records the claimant, named in the op's arguments, in `owners`. Creating items and their ids is `ts-0030-qw`, and where a write lands and `--commit` are `ts-0031-xu`.

## Success criteria

- `setStatus active` changes only that item's `status` line (`git diff`).
- Concurrent `setStatus` calls on one file all report what is on disk (the `flock` test).
- A kill between the temp write and the rename leaves the old file whole.
- A `claim` on an item that already shows an owner is refused, unless the claimant is that owner. `unclaim` clears the owner.
- `link` adds a `blocked-by` entry and changes only the `links` lines.
- A write with a stale `version` returns `conflict` and changes nothing.

## Tests

Write first: the `git diff` test for `setStatus`; N concurrent `setStatus` calls whose every result is true to the final file; a failpoint that kills the writer between temp write and rename; the claim refusal; `link`; the stale version.

## Notes

- IMPLEMENTATION_PLAN.md, 0.10 (Provider writes are safe against each other; the slice's claim rule; Versions; Writes are operations) and Stage 2: Skeleton, Planning slice and its criteria.
- Area: `packs/plan` (the provider's write path, TypeScript).
- Done when the criteria hold.
