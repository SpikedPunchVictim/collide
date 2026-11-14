---
id: ts-0009-cu
kind: task
status: ready
title: "Planning contracts"
owners: []
links:
  - { type: blocked-by, to: ts-0008-mx }
---

## Goal

Defined first (part L), as `core` types with generated schemas: item `kinds` (with `spike` and its statuses `planned`, `running`, `results`, `concluded`, `dropped`, each mapped to a category), op effects, typed `links`, `owners` (the claim) and the checklist field type; the run budget; the payload of the five new task points (`task.after_status`, `task.before_op`, `task.after_op`, `task.before_move`, `task.after_move`): `change` (`op`, `args`, `from`, `to`, `diff`, `categoryMoved`, `via`, `actor`, `batch`, `effects`, `effectOf`, `renamed`, `checkout`, `files`) and `latestRun`; `actor.kind: agent` with `run` and `phase`, reserved now so run-made writes need no later schema change; `points:` (derived points) and `checklists:` as `pack.yaml` keys (`views:` comes with the View type in `ts-0026-es`); and `collide schema`, the first real `collide` subcommand, which lists every key and point of the generated schema. Fan-out `instances:` is `ts-0024-uv`'s; the provider messages that carry these types (`apply` `items[]`, `preview`, `watch`, `task.invalid`) are `ts-0010-yb`'s.

## Success criteria

- `collide schema` lists every new key and point above.
- A derived point that names an unknown core point is refused. So is one derived from another derived point, or one whose `before_*` or `after_*` side differs from its core point's.
- A `spike` kind, a `task` kind and an `epic` kind validate with their statuses. A status outside its kind's list fails.
- `change.diff` maps a changed field to `{from, to}` and a set to `{added, removed}`. `from` is null at create and `to` is null at delete.

## Tests

Write first: derived-point validation (unknown core point, derived from derived, wrong side); the new keys' schemas; the `change` shape at create, update and delete; a `collide schema` output test.

## Notes

- IMPLEMENTATION_PLAN.md, 0.4 (named points, derived points), 0.5 (`change`), 0.8 (task fields), 0.10 (kinds, the `spike` kind) and Stage 2's Planning contracts.
- Area: `core` (types, derived-point check), `host` (the `schema` subcommand).
- Done when the criteria hold. The catalogue that lists the points is `ts-0018-dd`; the gate-bound-to-`after_*` refusal is `ts-0012-ua`'s.
