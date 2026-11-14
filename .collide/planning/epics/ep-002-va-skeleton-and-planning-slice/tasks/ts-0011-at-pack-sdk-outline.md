---
id: ts-0011-at
kind: task
status: ready
title: "pack-sdk outline"
owners: []
links:
  - { type: blocked-by, to: ts-0010-yb }
---

## Goal

`@collide/pack-sdk`: `defineHook`, `defineProvider`, `defineObserver`, the `ctx` object and the generated types, with stubbed bodies.

## Success criteria

- `tsc --noEmit` passes, and the types are the generated ones.

## Tests

- Type tests against the schema.
