---
id: ts-0021-yg
kind: task
status: ready
title: "Planning slice: Collide's own items through the CLI"
owners: []
links:
  - { type: blocked-by, to: ts-0015-dp }
  - { type: blocked-by, to: ts-0020-tt }
---

## Goal

Switch Collide's own build tracking from hand edits to the CLI.

## Success criteria

- Agents and the owner read and update items only through `collide task` from here on.
- Stages 3–6 are split into tasks with `collide task create` as each starts.

## Notes

The point where Collide starts tracking its own build (part L).
