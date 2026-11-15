---
id: ts-0057-ck
kind: task
status: ready
title: "Daemon and shim link: framed messages, adopt, lost"
owners: []
links:
  - { type: blocked-by, to: ts-0054-cy }
  - { type: blocked-by, to: ts-0056-ag }
---

## Goal

The link between the daemon and its shims. The shim's socket and its length-framed messages to the daemon, the daemon reaping shims with tokio and reading totals from `exit.json`, adoption of running shims before the daemon binds its socket, and the phase states that follow from a shim that dies or stops answering. Replaces the stubs tagged `shim: daemon link` and `daemon: adopt shims`.

## Success criteria

- A phase that ends while the daemon is down keeps its exit code and totals, and the next phase starts with the app closed (Stage 3a criterion). A shim whose child exits while the daemon is down starts the daemon (0.2, Starting the daemon).
- A shim killed with SIGKILL makes its phase `lost`, even though the agent reported success. A SIGSTOPped shim is reported as unresponsive and does not stall other runs (Stage 3a criteria).
- A restarted daemon adopts the shims still running and reattaches to their phases before it binds its socket.
- A framed message cut in half is never read as whole, end to end over the real shim socket (`ts-0016-ka` tested the frame alone).
- The shim's socket follows the daemon's rules: a `0700` directory under the run directory and the same uid check (0.2, The socket).
- The daemon acknowledges an exit record, and the shim then stops serving it.
- No stub tagged `shim: daemon link` or `daemon: adopt shims` remains.

## Tests

Write first: python3 children under real shims. Daemon killed during a phase, then restarted; shim SIGKILLed; shim SIGSTOPped with a second run going on; a half frame over the socket.

## Notes

- IMPLEMENTATION_PLAN.md, Stage 3a; 0.2 (the shim; `lost`; Starting the daemon; The workflow engine needs a live owner between phases; Durability). Hostile-process tests are adapted from herdr-core.
- Area: `host` (`shim.rs`, `daemon.rs`).
- Stage 3a item (revision 13 scope, split 2026-11-15). Items of Stage 3a run one at a time, in id order, because they share `host` and the generated files.
