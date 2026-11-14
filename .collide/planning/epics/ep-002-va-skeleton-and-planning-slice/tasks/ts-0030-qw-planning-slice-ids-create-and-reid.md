---
id: ts-0030-qw
kind: task
status: ready
title: "Planning slice: ids, create and reid"
owners: []
links:
  - { type: blocked-by, to: ts-0015-dp }
  - { type: blocked-by, to: ts-0020-tt }
---

## Goal

The `create` op and the id rules, through the provider and the CLI (`collide task create`). `create` writes `<id>-<slug>.md` into the epic's `tasks/` folder (an epic into `epics/ep-NNN-xx-<slug>/epic.md`). The slug is the first six words of the title, as in the existing item files. The number is the next free one for the kind across every ref Collide can see, local branches and fetched remotes. The suffix is two letters, never `i`, `l` or `o`, drawn from a seeded generator (the seed is an argument, random by default). A drawn id that is taken is redrawn until it is free among every ref. `collide task reid <id>` gives an item a new suffix, rewrites the links that name it, and commits nothing.

## Success criteria

- The number is the next free one across refs, including a branch that exists only as a fetched remote ref.
- A fixed seed gives a pinned suffix sequence, and a test fails if the generator changes.
- A forced collision (the generator rigged to return a taken id first) is redrawn.
- Two branches that each create number 12 with different suffixes both load, and `collide task 12` lists both.
- Two files with one id: the second is a `task.invalid`, and `collide task reid` on it gives it a new suffix, rewrites every link that names it, and leaves nothing committed.
- `create` returns the files it touched.

## Tests

Write first: id allocation over a fixture repo with a remote branch; the seeded suffix pin; the forced-collision redraw; the repeated number after a merge; `reid` rewriting links across several files.

## Notes

- IMPLEMENTATION_PLAN.md, 0.10 (ids, the collision rules, `collide task reid`) and Stage 2: Skeleton, Tests (the Planning and slice tests).
- Area: `packs/plan` (provider), `host` (CLI wiring).
- Done when the criteria hold.
