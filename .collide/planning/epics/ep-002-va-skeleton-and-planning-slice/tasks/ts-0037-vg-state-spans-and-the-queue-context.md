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

### What was built

- `core/src/state.rs`: `RunState` (`running`, `paused {cause}`, `integrating`, `ended {outcome}`, `failed`, `stopped`), `PhaseState` (`pending`, `running`, `paused {cause}`, `ended`, `failed {cause?}`, `stopped`, `lost`), `WorkspaceState` (a plain string: `provisioning`, `ready`, `in_use`, `idle`, `removable`, `removed`, `failed`), `PauseCause` (`ask_human`, `usage_limit`, `disk`) and `FailCause` (`disk`). Tagged by `state`.
- `core/src/span.rs`: `Span`, an enum tagged by `kind`: `run` (the root, no parent), then `phase`, `agent_turn`, `tool_call`, `hook`, `gate`, `provisioning`, `subprocess`, `wait` (each with a required `parent`) and `user_wait` (a required `parent` and `queue_item`).
- `core/src/context.rs`: `ContextRecord` with six required `Section` fields (`why_it_asks`, `what_it_would_do`, `task`, `what_led_here`, `history`, `agents_own_words`). A `Section` is `{status: "filled", text}` or `{status: "not_available"}`; the UI shows the second as `NOT_AVAILABLE`, "Not available in this view" (review: the wire tag is snake_case like every other tag).
- `core/src/non_empty.rs`: `NonEmpty`, a string refused when empty, with `minLength: 1` in the schema. Used for span ids, `queue_item`, the outcome and section text.
- Registered `RunState`, `PhaseState`, `WorkspaceState`, `Span` and `ContextRecord` in `core/src/schema.rs`; `pnpm gen` wrote `schemas/*` and `packages/pack-sdk/src/generated/*`.
- Reused: `events.rs` had the run, phase and workspace transitions as plain strings and said this item types them, so `run.transition`, `phase.transition` and `workspace.transition` now carry `RunState`, `PhaseState` and `WorkspaceState`. Fixtures in `core/tests/catalogue.rs`, `core/tests/provider_protocol.rs` and `packages/pack-sdk/test/provider-types.test.ts` changed to match. `workflow.rs`'s `PhaseKind` already names the five phase kinds and was left as it is. `experiment.transition` stays a string.
- Tests: `core/tests/state_spans_context.rs`.

### How each criterion is met

- A span without a kind or, when not the root, without a parent: unrepresentable. The kind is the serde tag, and only the `run` variant lacks `parent`. The schema is a `oneOf` that requires `parent` for every kind but `run`.
- A wait on the user without a Queue item id: the `user_wait` variant requires `queue_item`, and no other variant accepts it.
- A context record with a missing section: all six fields are required, so a missing one fails to parse. The marker is a distinct `status` value, not an empty string.
- `integrating` and `paused` with cause `disk`: variants of `RunState`.
- Round trips and refusals are tested for every type.

### Decisions for the owner

- The plan names no complete state lists, so the extras are mine: `running` for the run, `pending`, `running`, `ended` and `paused` for the phase, the `FailCause` of `disk` (from 0.2), and the `run` root kind of a span.
- Sections are text only for now. The structure inside them (diff, flagged command) and the "unverified" marking of the agent's words are left to the JSON-RPC item (`ts-0032-pp`) and the UI.
- Spans carry no attributes or times yet; the metrics stages add them.

### Not done by design

- No transitions or state machines (Stage 3a, `ts-0038-ng`).
- No check that a set of spans forms one tree (parent exists, one root, no cycle).
