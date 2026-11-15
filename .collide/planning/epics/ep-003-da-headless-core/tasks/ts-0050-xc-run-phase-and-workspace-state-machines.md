---
id: ts-0050-xc
kind: task
status: active
title: "Run, phase and workspace state machines"
owners: []
links: []
---

## Goal

The transitions between the states of `core/src/state.rs`, as pure functions in `core`. Run: `running`, `paused` with its cause, `integrating`, `ended(<outcome>)`, `failed`, `stopped`. Phase: `pending`, `running`, `paused`, `ended`, `failed` (with cause `disk` when known), `stopped`, `lost`. Workspace: `provisioning → ready → in_use → idle → removable → removed`, plus `failed`, which leaves only through the `resume` hook. A refused transition is an error that names both states.

## Success criteria

- Each machine has one transition table. Every pair of states not in it is refused, and the error names both.
- Terminal states accept nothing: `ended`, `failed`, `stopped` and `lost` for a phase; `ended`, `failed` and `stopped` for a run.
- A phase whose shim died without `exit.json` goes to `lost` from `running` or `paused`, whatever the agent's stream reported (0.2).
- `paused` with cause `disk` is reachable from `running` and returns to `running`. A phase that ends with no `result` on a full volume goes to `failed` with cause `disk`, not `lost` (0.2, Durability).
- `ts-0038-ng`'s `apply_integrate_answer` moves a run into and out of `integrating` through this machine, not with its own checks.

## Tests

Write first: a table test per machine over every ordered pair of states (allowed or refused); terminal states refusing every move; the two disk cases; `integrating` reached and left only through the integrate answer.

## Notes

- IMPLEMENTATION_PLAN.md, Stage 3a; 0.7 (Outcomes and terminal states); 0.8 (Run, Phase, Workspace); 0.2 (Durability; a shim that dies without `exit.json`); 0.4 (`integrate` answering `pending`).
- Area: `core` only. No I/O and no clocks.
- Stage 3a item (revision 13 scope, split 2026-11-15). Items of Stage 3a run one at a time, in id order, because they share `host` and the generated files.
- Done when the criteria hold and `run_rules.rs` uses these transitions.
