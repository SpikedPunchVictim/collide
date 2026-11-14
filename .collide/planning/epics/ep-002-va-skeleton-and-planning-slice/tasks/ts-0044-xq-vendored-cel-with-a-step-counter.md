---
id: ts-0044-xq
kind: task
status: ready
title: "Vendored cel with a step counter"
owners: []
links:
  - { type: blocked-by, to: ts-0007-ga }
---

## Goal

Vendor the `cel` crate at 0.15 into the repo with S5's step-counter patch (one tick at the top of `Value::resolve_val`, about 34 lines), exposed as a step budget a caller sets, and use the vendored copy from `core`. The per-task and per-pass budgets, the source cap, the list caps, `catch_unwind` around compile, and the static cost estimate belong to Stage 4b. Here the patch exists, is pinned and stops a runaway expression.

## Success criteria

- A runaway expression (a triple-nested macro over a few thousand elements) stops with a step-budget error instead of running on.
- An ordinary expression gives the same answer as with the unpatched crate, within the step budget.
- The vendored crate's version and the patch are recorded in a file beside it, so the patch can be offered upstream.
- `core` depends on the vendored copy, not the registry crate.

## Tests

Write first: the runaway-expression test (observed hanging or failing to stop against the unpatched crate, with a short timeout); an ordinary-view equality test.

## Notes

- IMPLEMENTATION_PLAN.md, 0.9 (CEL runs on untrusted task data; the step counter), 0.15 (Chosen crates), Stage 2: Skeleton, Tests (the step counter's patch, vendored, stops a runaway expression). S5's record: `.collide/planning/spikes/sp-0005-fv-schema-expressions/record.md`.
- Area: `vendor/cel` (or the repo's vendor directory), `core` (the dependency).
- Done when the criteria hold.
