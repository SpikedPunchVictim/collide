---
id: ts-0019-vm
kind: task
status: ready
title: "core and host outlines with tagged stubs"
owners: []
links:
  - { type: blocked-by, to: ts-0010-yb }
  - { type: blocked-by, to: ts-0016-ka }
  - { type: blocked-by, to: ts-0018-dd }
---

## Goal

`core` traits (`AgentAdapter` and `AgentSession` in the two-trait shape of 0.3 with S7's changes P1–P9, `HookRunner`, `TaskProvider`), domain types, state enums, the workflow engine and implementation resolution as signatures with doc comments and tagged stubs; `host` outlines for the Claude adapter, the ACP engine and its per-agent profile, daemon, shim, store, hook executor, provider supervisor, sampler and CLI.

## Success criteria

- Every stub carries a tag that belongs to a stage.
- `AgentSession` is sans-IO: its methods take bytes and events and return events, with no I/O in `core`.
- `cargo build` and `cargo clippy` pass.

## Tests

- The stub-count check.
