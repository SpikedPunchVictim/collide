---
id: ts-0013-nz
kind: task
status: ready
title: "CI: builds, lint, typecheck and structural checks"
owners: []
links:
  - { type: blocked-by, to: ts-0007-ga }
---

## Goal

CI runs build, clippy, lint and typecheck, the stub count per tag, the dependency-direction check and the no-names check.

## Success criteria

- A test fails if `core` gains a Tauri, SQLite or process dependency.
- A seeded `orchard` string in `core`, and a seeded `CLAUDECODE` or `ANTHROPIC_` string outside the Claude adapter, each fail CI.
- CI prints the stub count per tag, and every tag belongs to a stage.

## Tests

- The no-names check with its positive controls; the dependency-direction test.
