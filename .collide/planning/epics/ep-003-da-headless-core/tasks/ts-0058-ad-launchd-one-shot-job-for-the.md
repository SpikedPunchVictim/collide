---
id: ts-0058-ad
kind: task
status: done
title: "launchd one-shot job for the daemon"
owners: []
links:
  - { type: blocked-by, to: ts-0056-ag }
---

## Goal

The one-shot launchd job that starts the daemon outside its launcher's coalition on macOS: a temporary plist with absolute, canonical paths that sets `USER`, `HOME` and `PATH`, bootstrapped with `launchctl bootstrap gui/<uid>` and removed once the daemon runs. When bootstrap fails (over SSH or with no GUI session), the start falls back to `setsid` and prints a warning that the daemon may end with its launcher. Replaces the stubs tagged `daemon: launchd job`.

## Success criteria

- The daemon started by the one-shot launchd job has `USER`, `HOME` and `PATH`, and a python3 child runs under it (Stage 3a criterion). The real-`claude` check is in Stage 3b.
- The temporary plist is gone once the daemon answers.
- A bootstrap failure, forced in a test, starts the daemon with `setsid` and prints the warning. The run is not refused.
- The plist text is pinned by a test: every path absolute and canonical, and the three variables present.

## Tests

Write first: the plist text; the forced bootstrap failure and its warning. One real launchd test, run on macOS with a GUI session and skipped with a printed reason elsewhere (CI has no GUI session).

## Notes

- IMPLEMENTATION_PLAN.md, Stage 3a; 0.2 (Starting the daemon: the macOS coalition, the launchd job, the S2 measurements, the SSH fallback). herdr-core's `macos_launchd.rs` was read for the approach.
- Area: `host` (`launch.rs`).
- Stage 3a item (revision 13 scope, split 2026-11-15). Items of Stage 3a run one at a time, in id order, because they share `host` and the generated files.

### Built

- `host/src/launch.rs`: `ensure_daemon` now tries a one-shot launchd job before `setsid` when `StartRequest::launchctl` is set (`start_launchd_job`), and falls back to `start_with_setsid` when launchd refuses; the result is `StartKind::Launchd`, or `SetsidFallback` with `SETSID_WARNING` plus what `launchctl` said. The wait for the answer is shared (`wait_for_answer`); for a launchd job it asks launchd for the job's process (`JOB_SETTLE`, `JOB_LOOK`) and ends early as `StartError::Exited` when the job has none. The plist text is `launchd_plist(&LaunchdJob)`; its inputs are made canonical by `launchd_job` (daemon, log file, `HOME`, `COLLIDE_DAEMON_ROOT`, `SHELL`, `COLLIDE_LAUNCHCTL`; `USER` required, `HOME` required and absolute, `PATH` defaulting to the system's). The `launchctl` seam is the `Launchctl` trait (`bootstrap`, `bootout`, `process_of`) with `SystemLaunchctl` (a real or stand-in program); `launchctl_from_env(root, launchctl)` is the rule for when a start uses launchd. `StartRequest` gained `launchctl` and `label_prefix` and lost `PartialEq`/`Eq` (it holds a trait object). `PlistFile` removes the plist when dropped, so it is gone after a success, a refusal and an error. Labels are `<prefix>.<pid>-<nanos>` with the prefix `DEFAULT_LABEL_PREFIX` (`dev.collide.daemon`). The stubs tagged `daemon: launchd job` (`launchd_plist`, `bootstrap_launchd_job`) are replaced; the tag moved to `FILLED_TAGS` in `checks/tests/real_tree.rs`.
- `host/src/bin/collide-daemon.rs` (`start`) and `host/src/shim.rs` (`start_daemon`) set `request.launchctl` from `launchctl_from_env`; the binary passes `COLLIDE_DAEMON_ROOT`, `HOME`, `SHELL` and `COLLIDE_LAUNCHCTL` to the job (`FORWARDED_ENV`), since launchd's environment is bare, and forwards the same names to shims.
- Plist keys: `Label`, `ProgramArguments` (the daemon, no arguments), `WorkingDirectory` `/`, `EnvironmentVariables`, `StandardOutPath` and `StandardErrorPath` (the daemon's log), `RunAtLoad` true, `AbandonProcessGroup` true, `ProcessType` `Interactive`. No `KeepAlive`.
- Tests added: 16 unit tests in `host/src/launch.rs` (plist text pinned, three variables and absolute paths, one-shot keys, XML escaping, canonical paths through a symlink, request variables before the starter's, `USER`/`HOME` refused when missing, a missing daemon named, bootstrap arguments and plist location, plist removed on drop, failed bootstrap leaves nothing, stale label booted out first, unique labels, the `pid =` line, `launchctl_from_env` rule, `new` means `setsid` only); 4 integration tests in the new `host/tests/launchd_start.rs` (refused bootstrap starts the daemon with `setsid` and warns, the refused bootstrap's arguments and plist content, an overridden root never tries launchd, and the opt-in real launchd test). `host/tests/common/mod.rs` gained `daemon_status`.
- Real launchd test (`COLLIDE_REAL_LAUNCHD=1`, run by the builder on macOS with a GUI session; it skips with a printed reason otherwise): the daemon's environment as `ps -E` shows it had `USER=collide-test`, `HOME`, `PATH=/usr/bin:/bin`, `SHELL` and `COLLIDE_DAEMON_ROOT` from the plist (plus `XPC_SERVICE_NAME` set to the job's label, and `LOGNAME`, `TMPDIR`, `SSH_AUTH_SOCK`, `XPC_FLAGS`, `OSLogRateLimit` from launchd); `launchctl print` showed the job's process as the daemon that answered; a `/usr/bin/python3` child run by the daemon's environment capture (a recording login shell) saw the same `USER`, `HOME` and `PATH` and its parent shell's parent was the daemon; the plist was gone once the daemon answered; after the guard's `bootout` of that exact label the job was no longer loaded. Afterwards `launchctl list | grep dev.collide` was empty, `~/Library/LaunchAgents` was unchanged, `~/.collide` and `~/Library/Application Support/Collide` did not exist and no `collide` process was left.

### Decisions for the owner

1. A start uses launchd only when its caller asks: `StartRequest::new` is `setsid` only, and `launchctl_from_env` turns launchd on for the binary and the shim as follows: `COLLIDE_LAUNCHCTL` set means that program; else `COLLIDE_DAEMON_ROOT` set means no launchd; else `/bin/launchctl`. Alternative: always try launchd on macOS and make every test set an off switch. Recommendation: keep; a test that forgets the switch can never load a job into the owner's launchd, and an overridden root is a test's or a developer's. Product-behaviour note: a developer running with `COLLIDE_DAEMON_ROOT` gets `setsid` unless they also set `COLLIDE_LAUNCHCTL=/bin/launchctl`.
2. Job keys: `RunAtLoad` true with no `KeepAlive`, so one `launchctl bootstrap` runs the daemon once (no `kickstart` step, as herdr-core does with `RunAtLoad` false). Alternative: `RunAtLoad` false, then `bootstrap`, `kickstart`, `bootout`. Passed on because the job's process here is the daemon itself, and `bootout` would end it. Recommendation: keep.
3. `AbandonProcessGroup` true and `ProcessType` `Interactive` (from herdr-core's approach). The first so that shims and children are not ended with the daemon's process group when the daemon ends; the second so macOS does not treat the daemon as a background job. Alternative: leave both out. Recommendation: keep; the survival of shims through a daemon crash is a Stage 3a criterion and ts-0059 and Stage 3b should check it with a real kill.
4. The job stays loaded in launchd while the daemon runs, and after the daemon ends it stays loaded as a finished job until the next start. The start records its label in `launchd.label` in the run directory; the next start, which holds the lock and finds the socket silent, runs `launchctl bootout gui/<uid>/<that label>` first and ignores a failure. Alternative: enumerate `launchctl list` for our prefix. Passed on: it would touch labels the start did not make. Recommendation: keep. The label is `dev.collide.daemon.<pid>-<nanos>`, so a stale job never collides. Consequence: a stuck daemon of ours that does not answer is ended by the next start.
5. A job that launchd loaded but that has no process for 3 s (`JOB_SETTLE`) is reported as `Exited` with the log's tail, using `launchctl print`'s `pid =` line (not a stable interface; an unreadable answer is treated as "cannot tell"). Alternative: wait the full 15 s for `NoAnswer`. Recommendation: keep, and re-check the parse on each macOS release.
6. When bootstrap is refused the warning is `SETSID_WARNING` followed by `(launchd said: <launchctl's words>)`, so the person sees why (for example no GUI session). The warning text itself is unchanged and still pinned. Recommendation: keep.
7. The job's `USER` comes from the request, then the starter's `USER`, then `LOGNAME`; with none, launchd is skipped and `setsid` starts the daemon (it passes the starter's environment on whole). `PATH` is the starter's or `/usr/bin:/bin:/usr/sbin:/sbin`. Alternative: look the user up with `getpwuid`. Recommendation: keep; `getpwuid` is a later change if a launcher is ever found with no `USER`.
8. The plist lives in the daemon's run directory (next to the lock file, mode `0600`) and never in `~/Library/LaunchAgents`, so nothing loads it at the next login. Recommendation: keep.

- Owner decisions (2026-11-15): the recommendations above are accepted as built. The opt-in real test was checked clean afterwards from the main session (no `dev.collide` job loaded, `~/Library/LaunchAgents` unchanged, no process left). Follow-up for ts-0059-xs: a daemon that stops leaves its job loaded (no process) until the next start boots out the recorded label or the user logs out; stopping the daemon should boot out `launchd.label` itself.

### Not done, by design

- ts-0059-xs (stop): stopping a phase on purpose, and whether it bootouts the job. Nothing here stops a daemon or a job except a start's own stale-label bootout.
- Stage 3b: a real `claude` run under the launchd-started daemon (`USER` is set, but `claude`'s `api_error` check is Stage 3b's). Coalition id of the launchd-started daemon is not compared in this item (measured in S2).
- The app's restart policy (`RestartPolicy`, `MAX_FAILED_STARTS`, `FAILED_START_WINDOW`) is still only the constants: the resident app that applies it is a later stage.
- A `KeepAlive` LaunchAgent and a login item stay deferred (plan, 0.2).
- A real SSH session (bootstrap failing for real) was not run; the refusal is forced with a stand-in `launchctl`.
