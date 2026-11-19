---
id: ts-0142-ua
kind: task
status: ready
title: "CLI: a queued write whose file changed meanwhile is never reported applied"
owners: []
links:
  - { type: blocked-by, to: ts-0136-rr }
---

## Goal

`settle` in `host/src/task_daemon.rs` decides what became of a queued write: an `-undone` Queue item means not
applied, else a changed task version means applied. A person's edit to the file while the write waits also changes the
version, and the Queue item leaves at take, before the daemon records the `-undone` one. So `settle` can see the new
version first and report the write applied when it was not. `a_queued_write_whose_file_changed_while_it_waited_...`
(`host/tests/task_daemon_cli.rs`, `ts-0120-wm`) failed once this way in a full parallel run. Found at the
`ts-0136-rr` build (its decision 8).

## Success criteria

- A write that the daemon did not apply is never reported applied, however the daemon's steps interleave.
- An applied write is still reported applied, with no wait longer than today's outcome wait.

## Tests

- Write first and see red at runtime: a unit test of `settle` with a stub session that answers the new version before
  the `-undone` item shows, which reports applied today.
- The ts-0120 test passes 20 runs in a row under load.

## Notes

- The builder's alternative: after a version change, look for the `-undone` item again before saying applied. The
  daemon could instead say in the queued write's own record whether it was applied, which `settle` reads; that is a
  wire change, so take it only if a re-check cannot be made sound.
- Owner decisions: none recorded yet. The recommendations stand until the owner answers at the split.
