---
id: ts-0014-yc
kind: task
status: ready
title: "Planning slice: markdown provider, read side"
owners: []
links:
  - { type: blocked-by, to: ts-0009-cu }
  - { type: blocked-by, to: ts-0010-yb }
  - { type: blocked-by, to: ts-0011-at }
---

## Goal

The markdown provider's shared root at `.collide/planning/`, read in 0.10's layout, only the paths it names (`epics/<epic>/epic.md`, `epics/<epic>/tasks/`): kinds `task` and `epic`, epic membership from the folder, `blocked-by` links, ids of the form `ts-0012-ua`; frontmatter parsed with `yaml` in core mode (never `gray-matter`, never 1.1 mode), with the file size capped before parsing; protocol v0 `initialize`, `list` and `get`; a working `defineProvider`; `collide/plan` loads as a built-in pack from the build.

## Success criteria

- `list` returns every hand-written item in Collide's own `.collide/planning/`, with no `task.invalid`.
- An item with an unknown kind or a broken link is counted, not dropped.
- A task file moved into another epic's folder is listed under that epic, with its links unchanged.
- Two files with one id are a `task.invalid`.
- An id that does not match `^[a-z]{2}-[0-9]{3,}-[a-z]{2}$` is a `task.invalid` that says so.
- Frontmatter with an unquoted hash such as `1e10`, a duplicate key, an alias bomb, or a size over the cap is a `task.invalid`, and a quoted hash stays a string.

## Tests

- Provider contract tests over fixture items.
- Epic membership from the folder; a moved file; a duplicate id.

## Notes

IMPLEMENTATION_PLAN.md, 0.10: the markdown provider's layout and ids, and Collide's own build tasks.
