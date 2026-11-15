---
id: ts-0059-xs
kind: task
status: ready
title: "Stop: process tree, escalation and survivor report"
owners: []
links:
  - { type: blocked-by, to: ts-0057-ck }
---

## Goal

Stop, per 0.5 (`host/src/stop.rs`, `collide stop`). The ppid walk, the marker scan through libproc (pid plus `p_uniqueid`, skipping processes older than the run's start clock), the descendants of both, the signal order and escalation, the hooks' `detached` records, and the survivor report. The daemon and the shims carry `COLLIDE_RUNTIME=1` and are never swept. Replaces the stubs tagged `stop: find tree`, `stop: escalation` and `cli: stop`.

## Success criteria

- Stopping leaves zero processes, including a setsid'd grandchild, a double-forked orphan and a child that ignores SIGHUP and SIGTERM, checked with `ps`. The test children are python3, not platform binaries (Stage 3a criterion).
- A double-forked `/bin/sleep` orphan that its hook recorded as `detached` is stopped by the cleanup hook. An unrecorded one appears in the survivor report (Stage 3a criterion; 0.5).
- When a shim is killed, a child that ignores EOF and hangup is stopped as its phase turns `lost` (Stage 3a criterion; S2).
- A reused pid is never signalled: a process is matched by pid plus `p_uniqueid`.
- A daemon started by the CLI from inside a run has `COLLIDE_RUN`, `COLLIDE_RUN_START` and `COLLIDE_SPAN` stripped, so that run's stop cannot kill it (0.2).
- `collide stop <run>` stops a run through the daemon and prints the survivor report.
- No stub tagged `stop:` or `cli: stop` remains.

## Tests

Write first: the hostile python3 children (setsid grandchild, double fork, HUP and TERM ignored, EOF ignored), the recorded and the unrecorded `/bin/sleep` orphan, a pid-reuse case with a fake process table.

## Notes

- IMPLEMENTATION_PLAN.md, Stage 3a (Stop per 0.5); 0.5 (the stop order, `detached` records, the survivor report); 0.2 (how a stop finds descendants; the S3 `ps -E` measurement). Hostile-process tests are adapted from herdr-core (`sweep.rs`).
- Area: `host` (`stop.rs`, `commands.rs`). Last item of Stage 3a.
- Stage 3a item (revision 13 scope, split 2026-11-15). Items of Stage 3a run one at a time, in id order, because they share `host` and the generated files.
