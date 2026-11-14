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

The markdown provider's shared root at `.collide/planning/`, read in 0.10's layout, only the paths it names (`epics/<epic>/epic.md`, `epics/<epic>/tasks/`): kinds `task` and `epic`, epic membership from the folder, `blocked-by` links, ids of the form `ts-0012-ua`; protocol v0 `initialize`, `list` and `get`; a working `defineProvider`; `collide/plan` loads as a built-in pack from the build.

## Success criteria

- `list` returns every hand-written item in Collide's own `.collide/planning/`, with no `task.invalid`.
- An item with an unknown kind or a broken link is counted, not dropped.
- A task file moved into another epic's folder is listed under that epic, with its links unchanged.
- Two files with one id are a `task.invalid`.

## Tests

- Provider contract tests over fixture items.
- Epic membership from the folder; a moved file; a duplicate id.

## Notes

IMPLEMENTATION_PLAN.md, 0.10: the markdown provider's layout and ids, and Collide's own build tasks.
