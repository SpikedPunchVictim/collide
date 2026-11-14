---
id: ts-0011-at
kind: task
status: ready
title: "pack-sdk outline"
owners: []
links:
  - { type: blocked-by, to: ts-0010-yb }
  - { type: blocked-by, to: ts-0027-ar }
---

## Goal

`@collide/pack-sdk`: `defineHook`, `defineProvider`, `defineObserver`, the `ctx` object (`ctx.state`, `ctx.config`, `ctx.dataDir`, `ctx.report.span`, `.metric` and `.progress`, `ctx.ok`, `ctx.fail`, `ctx.refuse`) and the generated types, with stubbed bodies tagged `unimplemented("sdk: ...")`. The SDK ships as compiled JavaScript with `.d.ts` files and zero runtime dependencies, because Node refuses to strip types under `node_modules`.

## Success criteria

- `tsc --noEmit` passes, and the types are the generated ones, not a hand-written copy.
- The compiled output has no runtime dependency and uses erasable syntax only.
- Every stub carries a tag that belongs to a stage.

## Tests

Write first: type tests against the schema (a handler with a wrong result type fails to compile); a test that the compiled package's `package.json` lists no `dependencies`.

## Notes

- IMPLEMENTATION_PLAN.md, 0.6 (the SDK ships compiled; the context object), 0.15 (tagged stubs), Stage 2's `@collide/pack-sdk` bullet.
- Area: `packages/pack-sdk`.
- Done when the criteria hold. `defineProvider` becomes real in `ts-0028-hq`.
