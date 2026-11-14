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

Ops `create`, `setStatus`, `link`, `claim` and `unclaim` through protocol v0 `apply`.

## Success criteria

- `setStatus active` changes only that item's `status` line (`git diff`).
- `create` writes `<id>-<slug>.md` into the epic's `tasks/` folder. The number is the next free one for the kind across every ref Collide can see; the suffix is two random letters (no `i`, `l`, `o`).

## Tests

- Id allocation: the next number across refs, including a fetched remote branch; a suffix that collides is redrawn; apply returns the files touched.
