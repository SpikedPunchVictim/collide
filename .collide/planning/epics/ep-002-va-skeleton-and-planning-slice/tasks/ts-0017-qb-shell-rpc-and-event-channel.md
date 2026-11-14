---
id: ts-0017-qb
kind: task
status: ready
title: "Shell rpc and event channel"
owners: []
links:
  - { type: blocked-by, to: ts-0032-pp }
  - { type: blocked-by, to: ts-0035-re }
---

## Goal

The shell's one generic `rpc` command and one event channel in `app/src-tauri`, as signatures with tagged stubs: the shell forwards each call to the daemon's socket and adds the token itself, so the token never reaches the webview; `ui_tools.*` methods go to the local handler of `ts-0035-re` and are never forwarded. In `app/`, a typed TypeScript client (`rpc(method, params)` and an event subscription) over the types of `ts-0032-pp`, with an in-memory fake owner behind the same interface, so UI tests run in a browser with no daemon.

## Success criteria

- `cargo build` of `app/src-tauri` and the app build pass.
- A test calls `rpc` against the fake owner and gets a typed result. An event pushed by the fake owner reaches a subscriber.
- The client's API has no place to pass a token, and a test over the types checks that.
- A `ui_tools.*` call is routed to the local handler, and a test shows the forward path is not called.
- Every stub carries a tag that belongs to a stage.

## Tests

Write first: the fake-owner round trip; the event subscription; the `ui_tools.*` routing test; a type test that no token parameter exists.

## Notes

- IMPLEMENTATION_PLAN.md, 0.2 (The UI speaks the owner's protocol; the diagram), 0.15 (Pass 1: the shell's `rpc` command and event channel), Stage 2: Skeleton (the shell's `rpc` command and event channel).
- Area: `app/src-tauri`, `app/` (the client and the fake owner).
- Done when the criteria hold.
