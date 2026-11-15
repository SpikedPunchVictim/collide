---
id: ts-0024-uv
kind: task
status: active
title: "Workflow transition-table types"
owners: []
links:
  - { type: blocked-by, to: ts-0009-cu }
---

## Goal

The workflow part of `pack.yaml` as types with schemas: a workflow as a declared transition table; phase kinds `agent`, `fanout` (one role on several models, or `instances:` each naming any of a model, an agent definition, a profile and a prompt), `script` (with `agentWork: true`), `wait` (`interval:`, default 30 s, and `timeout:`, required) and `human`; `then:` (a phase name, `{ end: <outcome> }`, or a decision `{ script, on: { <token>: <target> } }` with `ask_human` as a target); `on_cold:`; `show:`; `accepts`; `on_start` and `on_end` ops with `when:`; workspace and integrate implementation names; and adopting (a start at a named phase and round). Every struct and variant is closed, and a variant with no fields is an empty object (`Wait {}`), never a unit variant (0.6).

## Success criteria

- A `wait` phase without `timeout:` fails validation. `interval:` defaults to 30 s and both parse as durations.
- A fan-out `instances:` list parses, and two identical instances are marked for refusal at launch (the check is a function over the type).
- A decision whose `on:` names an undeclared token, or a `then:` naming a phase that does not exist, fails validation.
- An unknown key fails at any depth, including inside a phase variant.
- The fixture workflows of `fixtures/packs/loop` and `fixtures/packs/pr-ci` and `collide/review` parse as transition tables.

## Tests

Write first: validation tests for each phase kind; the missing `timeout:`; a dangling `then:`; an undeclared decision token; identical instances; an unknown key in a phase.

## Notes

- IMPLEMENTATION_PLAN.md, 0.7 (A workflow is a declared transition table; phase kinds; `wait`; `then:`; `show:`; `accepts`; adopting), 0.4 (Lifecycle order), 0.3 (`on_cold`), 0.10 (`instances:`, owner 2026-11-13), Stage 2: Skeleton, Contracts.
- Area: `core` (workflow types and their structural checks).
- `ts-0012-ua` puts these types into `pack.yaml`. Running a workflow is Stage 8.
- Done when the criteria hold.

### Built

- `core/src/duration.rs`: `Duration` (`30s`, `120m`, `2h`, `1d`; a bare number is refused). `core` had none.
- `core/src/workflow.rs`: `Workflow`, `PhaseTable` (keeps declaration order), `Phase` (agent, script, wait, human; `PhaseKind` adds `fanout`), `Fanout` with `models` or `instances`, `Instance`, `Then`/`Target`/`Decision`, `OnCold`, `Sandbox`, `Accepts`, `WorkflowOp`, `Adoption`. Every struct is closed.
- `core/src/workflow_check.rs`: `validate` (dangling `then:`, `start`, undeclared and unhandled decision tokens, empty `on:`, fan-out source, wait timings, implementation names), `check_launch` (identical instances, a model listed twice), `check_adoption`.
- `workflows:` added to `PackManifest` (one field, so the built-in `collide` pack still loads); `Workflow` and `Adoption` registered for generation. `ts-0012-ua` still owns the rest of the manifest and runs `validate` per entry.
- Workflow parts written into `fixtures/packs/loop`, `fixtures/packs/pr-ci` and `packs/collide` (`review`). The reference pack's `loop` workflow parses and validates unchanged (tested through its `workflows:` key only).
- Tests: `core/tests/workflow.rs`.

### Edge decisions

- A phase has no `kind:`. As in the plan's reference example, the key says it: `role:` (agent; with `fanout:` it is a fan-out), `script:`, `wait: <hook>`, `human: <text>`. A key of another kind fails and is named.
- The plan names no key for the script, wait and human phases beyond their kinds; `wait: <hook path>` and `human: <text of the Queue item>` with `on: { token: target }` are this item's choice.
- "Undeclared token" needs a declaration, so a decision may carry `tokens:` (not in the plan). When present, `on:` must map exactly those; without it the keys of `on:` declare them. A token the script returns that is not in `on:` stays the engine's runtime `error` (`Decision::target_for` is `None`).
- `human` has no `then:`; its `on:` is its table.
- Phase order is declaration order, so it survives JSON text but not a `serde_json::Value` (sorted maps).

### Not done by design

- Roles, prompts, gates and scripts a phase names are not resolved (`ts-0012-ua`); running a workflow, `when:` CEL checks and the model-alias fan-out refusal are later stages.
