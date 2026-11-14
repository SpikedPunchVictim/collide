---
id: ts-0018-dd
kind: task
status: ready
title: "Point and event catalogues"
owners: []
links:
  - { type: blocked-by, to: ts-0009-cu }
---

## Goal

The point and event catalogues in full, as data in `core`. Points: run (`run.before_start`, `run.after_end`, `run.before_integrate`, `run.after_integrate`), workspace (`workspace.after_create`, `workspace.before_remove`), phase (`phase.before_start`, `phase.after_end`), task (`task.before_status`, `task.after_status`, `task.before_op`, `task.after_op`, `task.before_move`, `task.after_move`) and experiment (`experiment.before_arm`, `experiment.after_arm`). Events: run and phase transitions; gate verdicts, permission requests and answers, `ask_human` and budgets; task changes and writes (`task.changed`, `task.renamed`, `task.removed`, `task.invalid`); workspace states; reported metrics; experiments; and the **intent and outcome** events around a side-effect step (an integrate, a source op, an `after_integrate` hook, a remote write). Each point and event names the stage that makes it fire, and carries `experiment` where 0.4 says so.

## Success criteria

- CI lists each catalogue point with the stage that makes it fire, and every stage named exists.
- A binding to a point that is not in the catalogue is refused. Adding a point is a schema change (the generated schema's point enumeration changes).
- An intent event names the step and what it will do. An outcome event names the step and its result. Both carry the `actor`.
- Every event type serializes with a sequence number, and a round-trip passes.

## Tests

Write first: the catalogue completeness test (every point of 0.4's table is present, none extra); the stage listing; the unknown-point refusal; intent and outcome round trips.

## Notes

- IMPLEMENTATION_PLAN.md, 0.4 (Named points; Observers, the event catalogue), 0.2 (intent and outcome), 0.8 (event log), Stage 2: Skeleton, Contracts (point and event catalogues; intent and outcome events) and Success Criteria (catalogue listing).
- Area: `core` (catalogue data and types).
- Done when the criteria hold. The state, span and context types are `ts-0037-vg`.
