---
id: ts-0028-hq
kind: task
status: done
title: "pack-sdk: working defineProvider"
owners: []
links:
  - { type: blocked-by, to: ts-0011-at }
---

## Goal

Make `defineProvider` work for real in `@collide/pack-sdk`: a JSON-RPC server on stdin and stdout, in the framing `ts-0010-yb` recorded, that dispatches `initialize` (passing the pack's settings), `list`, `get`, `apply`, and the optional `watch` and `preview` to the handlers a pack gives it. This is the first working piece of the TypeScript provider path, the extension model's main risk.

## Success criteria

- A fake client over streams gets answers to `initialize`, `list`, `get` and `apply` from a provider written with the SDK.
- An unknown method gets a JSON-RPC "method not found" error, a handler that throws gets an error response and the process keeps serving, and a malformed line gets a parse error and does not end the process.
- The package still has no runtime dependency.

## Tests

Write first: a conformance test with an in-memory fake client for each method; the unknown-method, throwing-handler and malformed-line cases.

## Notes

- IMPLEMENTATION_PLAN.md, 0.10 (Protocol), 0.6, Stage 2's planning slice (provider protocol v0 and a working `defineProvider`).
- Area: `packages/pack-sdk`.
- Done when the criteria hold and the stub tag for `defineProvider` is gone from the registry count.
