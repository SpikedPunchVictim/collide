---
id: ts-0054-cy
kind: task
status: ready
title: "The shim: child, reap, journal, exit record and logs"
owners: []
links:
  - { type: blocked-by, to: ts-0051-wh }
---

## Goal

The shim of 0.2 (`host/src/shim.rs`), up to its link with the daemon. It spawns the child with `std::process::Command` and `pre_exec(setsid)`, reaps it with `libc::wait4` on the pid, writes the journal (one ordered file of `{seq, wall_ns, dir, bytes}`, each record written before the child sees the bytes), writes `exit.json` with the exit totals, keeps the terminal ring, and writes the file-backed child logs with their caps. Replaces the stubs tagged `shim: entry`, `shim: spawn child`, `shim: reap`, `shim: journal writer`, `shim: exit record` and `shim: pty ring`.

## Success criteria

- An agent's exit is never taken from std's or tokio's `wait`, and no tokio `Child` is dropped (measured S3). A test runs a python3 child that exits with a known code and the shim records that code.
- A session replayed from the shim's journal reaches the same state as the live one, and a journal cut short replays up to the last whole record (Stage 3a criterion). This reuses `ts-0039-ps`'s replay over a journal written by the real shim.
- Writing the child's stream and logs through a writer that fails with `ENOSPC` on demand does not crash the shim. On a full disk the shim keeps its exit record in memory and serves it until it is acknowledged (0.2, Durability).
- A stream line over the cap is cut and marked. A phase log over its cap stops growing, with a final marker (0.2, Durability).
- `exit.json` carries the exit totals of 0.13. A shim that cannot write it leaves no partial file.
- No stub tagged `shim:` remains except `shim: daemon link`, which is `ts-0057-ck`.

## Tests

Write first: python3 children (normal exit, non-zero exit, a burst of output, one over-long line), the journal's order against what the child received, the cut-short replay, the `ENOSPC` writer, `exit.json` written and kept in memory.

## Notes

- IMPLEMENTATION_PLAN.md, Stage 3a (The daemon and the shim); 0.2 (the shim and the journal; Durability); 0.13 (the shim's exit totals). The S3 record for `wait4` and setsid. Hostile-process tests are adapted from herdr-core.
- Area: `host` (`shim.rs`), using `core`'s journal and frame types (`ts-0016-ka`).
- Stage 3a item (revision 13 scope, split 2026-11-15). Items of Stage 3a run one at a time, in id order, because they share `host` and the generated files.
