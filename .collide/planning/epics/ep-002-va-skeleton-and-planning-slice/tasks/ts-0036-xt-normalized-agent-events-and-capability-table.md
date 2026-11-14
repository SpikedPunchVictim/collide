---
id: ts-0036-xt
kind: task
status: ready
title: "Normalized agent events and capability table"
owners: []
links:
  - { type: blocked-by, to: ts-0008-mx }
---

## Goal

In `core`, as types with schemas: the normalized events (message, tool call, permission request, `SessionBound`, `Usage`, `TurnEnd`, `FinalReport`, `Other`); the session start `Assign(uuid)`, `AgentAssigned` or `Resume(id)`; the permission request (its tool call, `kind`, `input: Option`, why it was asked, the agent's own options) and the answer `Allow`, `Deny { message: Option }` or `Cancel`; `Usage` with a scope (`Turn`, `Step`, `RunningTotal`), every number an `Option`, a cost with its currency and a `quality` (`Exact`, `Partial`, `Unreported`); `TurnEnd { reason, cancel_requested, empty }` with the reasons of 0.3; `FinalReport.source` (`ResultField`, `Reconstructed`); and the capability table of 0.3, 18 capabilities, each `Yes`, `No` or `Unknown`, with its sub-flags.

## Success criteria

- Serde round-trips pass for every event and answer type.
- A `Usage` number that is absent is `None`, never 0, and the schema says so.
- A capability that is not set is `Unknown`, and the table lists all 18 capabilities of 0.3.
- A `PermissionUnanswerable` event exists, and a `Cancel` answer parses.
- An unknown key fails in every type, and an unrecognized stream event is representable as `Other` with its raw type name.

## Tests

Write first: a round-trip test per type; the all-`Unknown` default table; the `None`-not-zero usage case; the unknown-key cases.

## Notes

- IMPLEMENTATION_PLAN.md, 0.3 (Two core traits; Normalized events; Permissions; Usage; A turn ends with; Capabilities), Stage 2: Skeleton, Contracts (the normalized events and the capability table). Spike S7's record: `.collide/planning/spikes/sp-0006-kq-agentadapter-acp/record.md`.
- Area: `core` (types and schemas).
- The traits that use these types are `ts-0019-vm`.
- Done when the criteria hold.
