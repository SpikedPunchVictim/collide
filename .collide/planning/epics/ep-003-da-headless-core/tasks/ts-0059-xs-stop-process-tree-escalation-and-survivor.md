---
id: ts-0059-xs
kind: task
status: done
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

### Built

- `host/src/sysproc.rs`: process identity and environment through libproc (`ProcRecord`, `record_of`, `all_pids`, `env_vars_of`). Identity is pid plus `p_uniqueid`; `EnvRead::Hidden` marks platform binaries whose environment the kernel will not show.
- `host/src/stop.rs`: the process table sits behind a trait (real libproc table, fake in tests). The order is TERM plus CONT, grace, rescan and KILL, the cleanup hook with `detached` records, a final rescan and the survivor report (Found, Recorded, Suspect). A target is found by the recorded roots and their descendants, and (when `by_marker`) by an exact `COLLIDE_RUN=<id>` match that skips processes older than the run start, plus the ppid walk. Processes with `COLLIDE_RUNTIME=1` (daemon, shims) are never swept. Every signal re-checks pid plus `p_uniqueid`. An empty run id is refused.
- `host/src/phases.rs`: `stop_run(run, grace)` marks non-final phases `stop_requested` (so they settle `Stopped`), stops from each phase's `child.json`, reads `detached.json`, and waits up to `STOP_SETTLE_WAIT` for the phases to settle. The lost-phase stop (ts-0057 follow-up) goes through the same order on a background thread, joined in `Phases::stop`, with `PhaseOptions::lost_grace`. `stop_orphan` and its SIGKILL are gone. The shim starts with `COLLIDE_RUNTIME=1`.
- `host/src/daemon.rs`: `run.stop` RPC (`RunStopResult` with the survivor report), `DaemonOptions` gains `processes`, `cleanup` and `launchctl`. The daemon boots out its recorded `launchd.label` as the last act of `Daemon::drop` (ts-0058 follow-up).
- `host/src/launch.rs`: `STRIPPED_ENV` (`COLLIDE_RUN`, `COLLIDE_RUN_START`, `COLLIDE_SPAN`) is stripped on both daemon start paths, which also set `COLLIDE_RUNTIME=1`. New `boot_out_recorded_job` over a `Launchctl` trait; it tolerates a bootout error only when the job is no longer loaded.
- `host/src/owner_client.rs` (new): a small blocking client that reads the token file, connects and initializes. `host/src/stop_cli.rs` (new): `collide stop <run> [--grace <secs>]`, which starts the daemon when needed and prints the survivor report. `host/src/commands.rs` and `host/src/bin/collide.rs` wire it; the `stop` stub is gone.
- Tests: `host/tests/stop.rs` (9), `host/tests/phases.rs` (20, 6 new), `host/tests/stop_command.rs` (5, new), `host/tests/daemon_socket.rs` (38, 2 new), unit tests in `stop.rs`, `launch.rs`, `stop_cli.rs`. Shared helpers in `host/tests/common/procs.rs`: unique run ids, `Guarded` tables that refuse to signal a pid the test did not start, and a pre-check that the kill set is the test's own. `checks/tests/real_tree.rs`: the three stub tags moved to `FILLED_TAGS`.

**Gate fix.** Two gate failures in `host/tests/phases.rs`.

- Spotlight noise in the report (`stopping_a_run_leaves_zero_processes_and_the_phase_stopped`). Cause: a suspect was any user process with parent 1, started after the run, and a hidden environment, and Spotlight workers (`mdworker_shared`) are exactly that. Measurement for the `p_puniqueid` idea: macOS does not separate them. A reparented, exec'd `/bin/sleep` orphan has `parent_unique_id` 1, the same as the `mdworker_shared` processes (a python orphan that does not exec keeps its dead parent's id, so only the exec'd case, which is the criterion's, is hidden). launchd's own record cannot be read by a user process, so `Snapshot` never has pid 1 and its child rule (`parent.unique_id == record.parent_unique_id`) is untouched. Fix, the fallback: `stop.rs` drops from the suspects any executable under `/System/`, `/usr/libexec/` or `/usr/sbin/` (`is_system_service`); `/bin` and `/usr/bin` stay, so the unrecorded `/bin/sleep` orphan is still reported, and an unreadable path is not excluded. The test now asserts no `Found` or `Recorded` survivor and none of its own pids, and ignores other suspects; the separate unrecorded-sleep-orphan test is unchanged. A unit test covers the rule.
- Refusal in `a_killed_shim_has_its_hostile_child_stopped_as_its_phase_turns_lost`. Not reproduced: 12 runs of `phases` beside two other test binaries, 25 runs under ten busy loops, 6 runs with a scan of every process carrying the run's marker taken just before the shim kill (it listed only the three recorded pids), and 8 full `cargo test -p collide-host` passes, all green. So the cause is not proven. The one mechanism found that fits (a refused pid that is gone afterwards, and the stop signals its targets as KILL then CONT back to back) is a test-guard race, not a product bug: the stop reads a target's record, the target ends, and the guard's own read of it fails, so a process of the test's looked like a stranger. The guard (`host/tests/common/procs.rs`) is no looser for a process it can read: that is still refused and panics before anything is sent. For a pid that can no longer be read it now sends nothing and returns ESRCH, as `kill` would, and `assert_nothing_refused` then checks, from what the stop's own scan read of that pid (kept in the guard), that it was the test's by pid and start time or a child of one; anything else still fails. A refusal now also prints that scan record, the command and the parent chain, so a recurrence names the process. A regression test in `host/tests/stop.rs` proves a readable stranger is refused and an ended process of the test's is not.
- The killed-shim test now runs a child that reads its stdin to the end (`hostile_reading_stdin`), ignoring hangup and terminate, and asserts the pidfile shows `eof` after the shim is killed, before the lost-phase stop ends it.

### Decisions for the owner

1. Exit code of `collide stop` with survivors. Chosen: 0 when the stop ran, even if survivors are listed (the report is the result); 3 for an unknown run, 2 for usage, 1 otherwise. Alternative: a non-zero code when any `Found` or `Recorded` survivor remains. Recommend keeping 0 now and revisiting when a script needs to branch on it (product behaviour: affects scripts).
2. Lost-phase grace. Chosen: 10 s between TERM and KILL (`LOST_GRACE`), shorter than the 60 s default of a requested stop, because the shim is already gone and nothing can still be flushing through it. Alternative: the same grace as a requested stop. Recommend 10 s.
3. Marker scan with a live sibling. Chosen: when a lost phase's run still has another live phase, the scan by `COLLIDE_RUN` is off and only the lost phase's recorded roots and descendants are stopped, so a double-forked orphan of the lost phase is not found until the whole run stops. Alternative: always scan by marker, which would kill the live sibling's processes. Recommend keeping this; the later full stop sweeps it.
4. Run state after a stop. The daemon has no run state machine yet (Stage 4), so `run.stop` derives it from the phases: Running, Stopped, Failed, or Ended with the outcome text "every phase ended". Alternative: leave the state out of the result until Stage 4. Recommend keeping it; Stage 4 replaces the derivation.
5. Run start. The sweep skips processes older than the run start, and the start is the earliest `spec.json` mtime in the run's phases. Alternative: record the start explicitly in a run file. Recommend the explicit record once Stage 4 owns runs.
6. Launchd boot-out. The daemon boots out its own label last when it stops, and only when a `Launchctl` is configured (never under `COLLIDE_DAEMON_ROOT` unless `COLLIDE_LAUNCHCTL` names a program). Alternative: leave the job loaded, since it is one-shot. Recommend booting out so `launchctl list` stays clean.
7. Unrecorded orphans. Chosen: reported as `suspect` and never signalled, because without the marker or a recorded root the match is a guess (a machine-wide scan). Alternative: kill suspects on request with a flag. Recommend no kill; revisit with a `--force-suspects` only if owners ask (product behaviour).
8. Who may stop. A RunAgent may stop only its own run (`NOT_PERMITTED` otherwise); an owner may stop any run. Alternative: let agents stop sub-runs. Recommend as built.
9. Daemon stop and lost-stop threads. They are joined when the daemon stops, bounded by grace plus settle, so a daemon stop can take up to about 20 s while a lost phase is being stopped. Alternative: detach them and risk orphans. Recommend joining.
10. Ambient `COLLIDE_RUN`. A `collide` command run with `COLLIDE_RUN` set is a RunAgent, so `collide stop other-run` from inside a run is refused. This follows from the auth rules; noting it because it surprises a developer who exports the variable in a shell. Recommend as built.

- Owner decisions (2026-11-15): the recommendations above are accepted as built. Follow-ups: interrupt and stdin-close before TERM land with the Stage 3b agent adapters; the cleanup-hook executor lands in Stage 7; suspects stay report-only (a `--force-suspects` flag is revisited only if a real orphan escapes); the run start moves to an explicit run record when Stage 4 owns runs.

### Not done, by design

- The cleanup hook executor is not wired in the daemon (Stage 7). Recorded `detached` survivors are therefore reported as `recorded`; the tests drive the hook through an injected executor.
- No `interrupt` or stdin-close step through the shim before TERM (Stage 3b).
- No real run state machine in the daemon (Stage 4); see decisions 4 and 5.
- Suspects are never killed (decision 7).
- The `run.stop` RPC blocks its connection thread for the whole stop; other connections are served by their own threads.
- A known limit of the test guard: the stop_command tests start a real daemon, so its kill set gets a read-only pre-check instead of a pre-kill guard.
