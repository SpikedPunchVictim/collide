---
id: ts-0012-ua
kind: task
status: ready
title: "pack.yaml contract"
owners: []
links:
  - { type: blocked-by, to: ts-0023-ch }
  - { type: blocked-by, to: ts-0024-uv }
  - { type: blocked-by, to: ts-0026-es }
---

## Goal

`pack.yaml` in full, as one type with its schema: `apiVersion`, `name`, `version`, `requires` (`sdk`, `tools`, `agents`, `modelName`), `config` (one JSON Schema per key, `secret`, `overridable`), roles, prompts and definitions, hooks and implementations (`workspace`, `integrate`, `sync`, with their sub-hooks and timeouts), gates, bindings with `when:` and `timeout:`, `points:`, `views:` and `profiles:` (the View and Profile types of `ts-0026-es`), `checklists:`, observers with `events:`, `record` paths, `protectedPaths`, `metrics` and `experiment` (the keys used only in Milestones C and D), and `workflows` from `ts-0024-uv`. Replace the placeholder manifests of the five pack directories with real ones.

## Success criteria

- The two built-in packs' manifests (`packs/collide`, `packs/plan`), the reference pack's fixture copy and both fixture packs' manifests validate against the generated schema.
- A manifest with an unknown key fails, at any depth.
- A required string field has `minLength: 1`.
- A gate bound to an `after_*` point is refused at load, with a message that names the point.
- A `script`, `wait` or hook path is relative to the pack directory, and `repo:` marks a path in the line's snapshot.

## Tests

Write first: manifest validation over all five manifests plus invalid fixtures, including S5's hazards read through the loader of `ts-0023-ch` (`no`, `0.10`, a duplicate key, an alias bomb, deep nesting); a gate on an `after_*` point.

## Notes

- IMPLEMENTATION_PLAN.md, 0.4 (Contribute, named points, gates), 0.7 (the manifest, its example, paths), 0.9. The S5 record: `.collide/planning/spikes/sp-0005-fv-schema-expressions/record.md`.
- Area: `core` (manifest type, load-time checks), the five `pack.yaml` files.
- `board.yaml`, `config.yaml`, views and the lock files are `ts-0025-st` and `ts-0026-es`.
- Done when the criteria hold and `collide schema` lists the manifest keys.
