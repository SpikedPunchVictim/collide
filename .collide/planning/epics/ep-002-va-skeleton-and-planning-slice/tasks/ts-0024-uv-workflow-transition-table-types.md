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
