---
id: ts-0037-vg
kind: task
status: active
title: "State, spans and the Queue context record"
owners: []
links:
  - { type: blocked-by, to: ts-0009-cu }
---

## Goal

State, event-log entry and span types in `core`. State: the run (`ended(<outcome>)`, `failed`, `stopped`, pauses with their cause such as `ask_human`, a usage limit and `paused: disk`, and the new `integrating`), the phase (kinds `agent`, `fanout`, `script`, `wait`, `human`; `lost`), and the workspace (`provisioning`, `ready`, `in_use`, `idle`, `removable`, `removed`, `failed`). Spans: every span has an id, a kind and a parent id (a root excepted), and a wait on the user is a span of its own kind with the id of the Queue item it waited on. The Queue item's context record: one record of six sections (why it asks, what it would do, the task, what led here, history, the agent's own words), each filled or `Not available in this view`.

## Success criteria

- A span without a kind, or without a parent id when it is not a root, does not compile or does not validate.
- A wait-on-the-user span without a Queue item id is invalid.
- A context record with a missing section is invalid: the section must say `Not available in this view`, so the UI can show the gap and never hide it.
- The run state has an `integrating` state, and a `paused` state with the cause `disk`.
- Serde round-trips pass for every type.

## Tests

Write first: the span validity cases; the context record with each section missing in turn; the state enum round trips.

## Notes

- IMPLEMENTATION_PLAN.md, 0.8 (Run, Phase, Workspace, Spans), 0.4 (`integrating`), 0.2 (`paused: disk`), 0.13 (spans, a wait is a span), 0.11 (the context view is a data contract), Stage 2: Skeleton, Contracts (state, events and spans; the context record).
- Area: `core` (types and schemas).
- Done when the criteria hold. The transitions that use `integrating` are `ts-0038-ng`; the state machines are Stage 3a.
