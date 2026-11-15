---
id: ts-0018-dd
kind: task
status: done
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
- Points (`core/src/points.rs`, `Point`; stage per the plan's Named points table and the stage bullets that build each one):
  - Stage 7 (hooks, workspaces, record): `run.before_start`, `run.after_end`, `workspace.after_create`, `workspace.before_remove`.
  - Stage 8 (workflow engine): `run.before_integrate`, `run.after_integrate` (with the fake `integrate`; Stage 9a wires the real accept), `phase.before_start`, `phase.after_end`, `task.before_status`, `task.after_status`, `task.before_op`, `task.after_op`.
  - Stage 12 (second pack): `task.before_move`, `task.after_move` (the Share move op and its points).
  - Stage 15 (experiments): `experiment.before_arm`, `experiment.after_arm`.
- Events (`core/src/events.rs`, `EventKind`; stage per the event-catalogue bullet and the stage that emits it):
  - Stage 3b: `permission.requested`, `permission.answered`.
  - Stage 4b (planning source): `task.changed`, `task.renamed`, `task.removed`, `task.invalid`.
  - Stage 3a (the `core` Run, Phase and Workspace state machines, the store and event log, the built-in workspace): `run.transition`, `phase.transition`, `workspace.transition`.
  - Stage 7: `metric.reported` (report lines, which the hook report files of Stage 7 carry).
  - Stage 8 (workflow engine): `gate.verdict`, `ask_human.asked`, `ask_human.answered`, `budget.exceeded`, `task.written`, `step.intent`, `step.outcome`.
  - Stage 15: `experiment.transition`.
- Shape: `Event` is the envelope (`seq`, `wall_ms`, `actor`, `experiment`, `body`). `EventBody` is internally tagged by `event`, with one struct variant per event. Typed enums, all lowercase in JSON: `PermissionAnswer` (allow, deny, cancel), `StepKind` (integrate, source_op, after_integrate_hook, remote_write), `BudgetScope` (run, phase, role), `StepResult` (succeeded, failed). A merge that finds its change already at the target is `succeeded` (0.2). Other states are strings, for `ts-0037-vg` to type.
  - `phase.transition` carries `round`, and `log` and `transcript` are nullable until they exist.
  - `gate.verdict` carries `pack`, `phase` or `point` (a gate on a named point, 0.4), and `message` (default empty).
  - `ask_human.*` carries `queue_item`, the Queue item the run waits on (0.13).
  - `step.outcome` carries `detail` (default empty).
  - Reused types: `Actor`, `Change`, `Rename`, `HookExperiment`, `HookStatus`, `ReportLine`, `TaskChanged`, `TaskRemoved`, `TaskInvalid`, `Point`.
- `ObserverEvent` is now a transparent newtype over `Event`, so an observer line is the envelope and the registry keeps the name `ObserverEvent` (its schema refers to `Event`). The SDK's `ObserverEventOutput` changed shape; `provider-types.test.ts` was updated to match. `observer.ts` needed no change.
- `CORE_POINTS` is gone. `check_derived_points` takes `&[Point]`; an unknown `on` is refused, and `Point::parse` refuses a name outside the catalogue with an error that names it.
- `collide schema` lists each point and event with its stage (`  run.before_start  stage 7`), before the types.
- Plan gaps for the owner (plan not edited):
  - `task.removed` and `task.renamed` are named in 0.10 and 0.4, but no stage says which provider emits them for the markdown source; 4b is an inference from the `watch` bullets.
  - Stage 8 builds `Change` in full, but the planning slice's `collide task <id> setStatus` (Stage 2) and the thin loop's scratch-task writes (Stage 3c) write before it exists. `task.written` therefore has no emitting stage before 8.
  - `run.before_integrate` and `run.after_integrate` are built in Stage 8 (fake integrate) and wired in 9a. Stage 9a's bullets also name them, so the point's owner stage is ambiguous.
  - Observer delivery is Stage 12, but events fire from earlier stages. Until 12, nothing receives them; the plan does not say so.
  - `run.transition`, `phase.transition` and `workspace.transition` are mapped to Stage 3a, where the state machines and event log first exist. Stage 8 later drives them through the transition table.
