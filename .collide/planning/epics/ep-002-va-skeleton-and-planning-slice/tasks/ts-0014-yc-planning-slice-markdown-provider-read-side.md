---
id: ts-0014-yc
kind: task
status: ready
title: "Planning slice: markdown provider, read side"
owners: []
links:
  - { type: blocked-by, to: ts-0009-cu }
  - { type: blocked-by, to: ts-0010-yb }
  - { type: blocked-by, to: ts-0028-hq }
---

## Goal

The markdown provider's read side, in TypeScript in `packs/plan`, on the SDK's working `defineProvider`. It reads the shared root at its default path, `.collide/planning/`, and in this stage only under `epics/`: `epics/<epic>/epic.md` and the `.md` files directly in `epics/<epic>/tasks/`. It never walks anywhere else, and it does not read `spikes/`, `proposals/` or `questions/` yet (Stage 4c adds them). Kinds `task` and `epic`; the epic comes from the folder (`part-of`, never from frontmatter); `blocked-by` links; ids of the form `ts-0012-ua`, always strings; an optional `alias` is shown and never linked to; task statuses are the five categories. Frontmatter is parsed with `yaml` in core mode (never `gray-matter`, never 1.1 mode), with duplicate keys and the alias limit on, every field's JavaScript type checked and no coercion or `Date`. The frontmatter block is capped before parsing (S5 suggests 64 KiB), not the file, and the body is never parsed as YAML. The body has its own cap, chosen here and recorded in the item's Notes. Protocol v0 `initialize`, `list` and `get`.

## Success criteria

- `list` returns every hand-written item under `epics/` in Collide's own `.collide/planning/`, with no `task.invalid`. It does not read `spikes/`.
- An item with an unknown kind or a broken link is counted, not dropped.
- A task file moved into another epic's folder is listed under that epic, with its links unchanged.
- Two files with one id are a `task.invalid`.
- An id that does not match `^[a-z]{2}-[0-9]{3,}-[a-z]{2}$` is a `task.invalid` that says so.
- Frontmatter with an unquoted hash such as `1e10`, a duplicate key, an alias bomb, or a frontmatter block over its cap is a `task.invalid`, and a quoted hash stays a string.
- A long body under the body cap is valid. A body over it is a `task.invalid`.
- `initialize` declares the kinds `task` and `epic`, the five categories as the task statuses, and `blocked-by` as the one blocking link.

## Tests

Write first: provider contract tests over fixture items (run the provider over stdio with the SDK's fake client); epic membership from the folder; a moved file; a duplicate id; a bad id; the frontmatter hazards; a frontmatter block over its cap beside a long body; `spikes/` present and ignored.

## Notes

- IMPLEMENTATION_PLAN.md, 0.10 (the markdown provider's layout, kinds and ids, frontmatter, Collide's own build tasks), 0.9 (YAML and frontmatter are untrusted input), Stage 2: Skeleton, Planning slice. Spike S5's record for the parser findings.
- Area: `packs/plan` (the provider, TypeScript, run by `node`).
- Done when the criteria hold, including `list` over this repository's own `epics/`.
