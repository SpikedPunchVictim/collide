---
id: ts-0019-vm
kind: task
status: ready
title: "core outlines with tagged stubs"
owners: []
links:
  - { type: blocked-by, to: ts-0010-yb }
  - { type: blocked-by, to: ts-0027-ar }
  - { type: blocked-by, to: ts-0016-ka }
  - { type: blocked-by, to: ts-0018-dd }
  - { type: blocked-by, to: ts-0036-xt }
  - { type: blocked-by, to: ts-0037-vg }
  - { type: blocked-by, to: ts-0013-nz }
---

## Goal

The `core` outlines for Milestones A and B, as signatures with doc comments and tagged stubs (`todo!("engine: fan-out join")`), with each tag added to the stub-tag registry. Traits: `AgentAdapter` (stateless: capabilities, a launch spec from a role and phase, building a session, the tool's config paths, an interactive command) and `AgentSession` (sans-IO: `start`, `feed(line, wall_ns)`, `send_turn`, `answer`, `cancel`, `finish` and `replay`, each returning events and the bytes to write), `HookRunner` and `TaskProvider`. S7's changes P1 to P9 are in: two traits, `SessionStart` and `SessionBound`, permission answers `Allow`, `Deny` and `Cancel`, `Usage.quality`, the capability table, `TurnEnd`, and `model.id` opaque beside `model.name` in the model reference type. Domain types (board, line, task, run, phase, workspace, experiment of 0.8), the state enums of `ts-0037-vg`, the workflow engine and implementation resolution as signatures. No ACP engine and no ACP profile: they belong to `host` and Stage 3d, and are not outlined here.

## Success criteria

- Every stub carries a tag that belongs to a stage.
- `AgentSession` is sans-IO: its methods take bytes, a line and a stored time, and return events and bytes. A test implements it with no I/O crate, and `core` reads no clock.
- `cargo build` and `cargo clippy` pass.
- A role that requires a capability the adapter says `No` or `Unknown` for is refused by a function that exists as a stub with a doc comment naming the rule.

## Tests

Write first: the stub-count check over these tags (from `ts-0013-nz`); a compile test that implements `AgentSession` in a test module with no I/O dependency.

## Notes

- IMPLEMENTATION_PLAN.md, 0.3 (Two core traits; Capabilities; Who picks what), 0.2 (the crates), 0.8, 0.15 (Tagged stubs), Stage 2: Skeleton (`core` outlines).
- Area: `core`.
- Done when the criteria hold. The `host` outlines are `ts-0040-kd` and `ts-0041-xf`.
