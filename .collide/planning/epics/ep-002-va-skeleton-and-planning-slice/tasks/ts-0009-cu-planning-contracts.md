---
id: ts-0009-cu
kind: task
status: done
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
- Types added (core modules `item`, `change`, `points`, `pack`; registry roots in `core/src/schema.rs`): `KindDecl` (with `StatusDecl`, `TransitionDecl`, `OpDecl` whose `movesTo` is the op effect), `LinkTypeDecl`, `PlanningFields` (`kind`, `links`, `owners`, `checklist`), `Change` (with `Actor`, `Via`, `FieldDiff`, `Batch`, `Effect`, `Rename`), `TaskPointPayload` (`change`, `latestRun`), `RunBudget`, `PackManifest`. The kinds of Collide's own items (`epic`, `task`, `spike`) are `item::planning_kinds()`, and `item::category_of` is the status check. The markdown provider must declare the same statuses, and adds the ops and transitions, which are empty here.
- `PackManifest` is the smallest piece of the `pack.yaml` struct that lists `points:` and `checklists:` as keys: it has those two keys and nothing else, and it denies unknown keys, so a real `pack.yaml` does not validate against it yet. `ts-0012-ua` extends this type (`pack.rs`) with the other keys instead of writing a second one. `ChecklistTemplate` is a map value, named by its key in `checklists:`. The plan does not say whether `checklists:` is a map or a list, so this is a decision to check in `ts-0012-ua`.
- Derived-point check: `points::check_derived_points(points, known_core_points)` returns every refusal. `points::CORE_POINTS` is the six task points, the start of the catalogue that `ts-0018-dd` completes; `ts-0018-dd` passes the full set. A name with no `before_` or `after_` side is also refused (`NoSide`). The gate-bound-to-`after_*` refusal is `ts-0012-ua`'s.
- `collide schema` prints the points, then every type of the generated input schemas that has keys, with its keys, sorted. It reads the registry, so a later item's type appears with no change to the command. `collide schema --path` (0.7) is not built; any other argument exits 2 with `usage: collide [schema]`. `collide-host` now depends on `collide-core` and `serde_json`.
- Not built: computing `change.diff` from the cache and an `apply` result (engine items), the `task.changed`, `task.renamed`, `task.removed`, `task.invalid` notifications, `apply` `items[]` and `preview` (`ts-0010-yb`), the one-hop links on `tasks[]` entries, and the rest of the 0.5 hook input around `change`.
- Interpretations: (1) `actor` is the plan's 0.5 shape, literally: a flat struct with `kind` (`person | workflow | agent`) and optional `run` and `phase`, tied to no kind. (2) `change.from` and `to` are `{status, category}` or null, and `latestRun` is `{id, workflow, outcome, state}`; the plan names neither shape, so both come from `docs/research/planning.md` 7.6. (3) `LinkTypeDecl.dir` is `out | in`: which end the storing item holds (`blocked-by` is `out`). The plan says only "their direction". (4) `Link.blocking` is optional, and absent means the link type's `blocking` applies, because the items in this repo write `blocked-by` links with no `blocking`. The ready rule (the slice provider items) reads the link type's declaration when a link has no `blocking`. (5) `RunBudget.usd` is any number; a positive total is a load check for `ts-0012-ua`. (6) `proposal` and `question` kinds are not in `planning_kinds()`: the plan lists only `epic`, `task` and `spike` for the slice. (7) `epic` uses the same five statuses as `task`.
