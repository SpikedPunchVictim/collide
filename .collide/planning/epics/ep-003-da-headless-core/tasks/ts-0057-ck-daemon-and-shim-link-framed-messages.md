---
id: ts-0057-ck
kind: task
status: active
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

### Built

- `host/src/shim_link.rs` (new): `FrameReader` (keeps partial bytes across reads; `read_next` gives `Next::{Message, Idle, Closed{dropped}}`; an oversize length is an error at the prefix), `send`, and `connect_shim` (uid check with `auth::admit_shim`, reads `Hello`, answers with `answer_hello`, caller-controlled wait for a silent shim). 4 unit tests (half a frame is `Idle` and the rest completes it; a cut frame is dropped and counted; two frames in one read; an oversize length).
- `host/src/shim.rs`: `ShimLink` (accept thread on `<run dir>/shim/s`, one thread per connection, socket and dir made by `auth::prepare_shim_socket`), `publish_exit` / resend of the record to each new connection until `AckExit`, `Limits.ack_wait`, `run_with`, `DaemonStart` and `ShimSpec.daemon`, `shim.json` identity, `shim.log`. The shim starts the daemon through `ensure_daemon` only when `spec.json` carries `daemon`. Existing unit test for the spec round trip gained a daemon case.
- `host/src/phases.rs` (new): `Phases` (`start_phase`, `write`, `close_stdin`, `state`, `health`, `exit_record`, `shim_pid`, `shim_exit`, `live`, `unresponsive`, `adopt`, `stop`), `PhaseKey` (name rules), `PhaseSpec`, `PhaseOptions`, `PhaseEnd`, `PhaseEndHook`, `Health`, `LinkError`, `PhaseError`, `read_exit_json`, `move_damaged_exit`, `stop_orphan`. One watcher thread per phase, one reaper thread per shim the daemon started. 5 unit tests.
- `host/src/daemon.rs`: `adopt_shims` is real (the `todo!("daemon: adopt shims")` is gone), `AdoptedShim{key, dir, identity, state}` and `AdoptedState::Damaged`; `Daemon::start` builds `Phases` from the event log and adopts before `bind_socket`; `DaemonOptions` gained `shim_exe`, `daemon_start`, `phases`, `on_phase_end`; `Daemon::phases()` and `Daemon::events_since`; `daemon.status` reports `live_runs` and a warning per unresponsive shim; dropping the daemon stops its watchers and leaves the shims.
- `host/src/sysproc.rs`: `is_stopped`, `is_zombie` (macOS reads the BSD status; other platforms answer `None`). 1 unit test.
- `host/src/bin/collide.rs`: hidden `collide shim <run-dir>`. `host/src/bin/collide-daemon.rs`: sets `shim_exe` (the `collide` next to it) and `daemon_start` (its own canonical path plus `COLLIDE_DAEMON_ROOT`, `HOME`, `SHELL` when set). `host/Cargo.toml`: `serde = "1"` (already in the lock through `core`; the framing is generic over serde types).
- `checks/tests/real_tree.rs`: `shim: daemon link` and `daemon: adopt shims` moved to `FILLED_TAGS` as `("3a", "ts-0057-ck")` (19 to 21), `HOST_OUTLINE_TAGS` 7 to 5.
- Tests: `host/tests/shim_link.rs` (5, real shim sockets: Hello first, uid and directory rules, path over 103 bytes, foreign message never taken for a Welcome), `host/tests/shim_child.rs` (38, now with `ack_wait`), `host/tests/phases.rs` (14, in-process daemon with python3 children under real shims), `host/tests/link_process.rs` (12, separate `collide`, `collide-daemon` and shim processes). Totals: `collide-host` 614, `collide-core` 957, `collide-checks` 42, all passing; `link_process`, `phases`, `shim_link` and `shim_child` run 3 times in a row and twice more in parallel with no flake.
- What the criteria were shown by:
  - (a) "ends while the daemon is down keeps exit code + totals": `link_process` (the child exits code 7 with CPU time after the first daemon was SIGKILLed; the shim starts the daemon through `ensure_daemon`; the new daemon's log, store events and `daemon.status` hold code 7 and the totals) and `phases` (`a_phase_that_ended_while_the_daemon_was_down_keeps_its_code_and_totals_and_the_next_phase_starts`). "The next phase starts with the app closed": there is no `run.start` RPC and no engine yet (Stage 3b), so this is shown two ways: in process, `DaemonOptions.on_phase_end` is the stand-in for the engine and starts the next phase from inside the adopting daemon (`phases` test); at process level, the chain test starts the first daemon with no app or CLI connected, kills it, and a daemon that the shim itself started adopts the shim and takes the record. It is not shown that an engine decides the next phase; that is Stage 3b.
  - (b) SIGKILL of a shim after the agent printed a `result` gives `lost`; SIGSTOP of one shim reports it unresponsive while a second run keeps taking writes without delay (`phases`; the SIGCONT and cleanup run in a drop guard).
  - (c) `a_restarted_daemon_adopts_the_running_shim_before_it_binds_and_reattaches_to_its_phase` (state settled before `Daemon::start` returns, no second `running` event) and `a_stopped_shim_is_unresponsive_to_a_daemon_that_adopts_it_and_the_daemon_still_binds`.
  - (d) `shim_link` half-frame tests over the real socket, plus the `FrameReader` unit tests. (e) `shim_link` socket rules tests. (f) `an_exit_record_is_served_again_to_the_next_connection_until_it_is_acknowledged`. (g) `pnpm check:stubs` and `real_tree`.
- How the daemon finds shims: `adopt_shims` scans `runs_dir` for `<run>/<phase>` directories that hold `spec.json` (names `PhaseKey` refuses are skipped). Per directory, in order: `exit.json` (a record is `Exited`, an unreadable one is `Damaged`); then `shim.json` (the shim's own pid and start time): alive when a process with that pid started at that time and is not a zombie; with no `shim.json`, alive when the socket `shim/s` accepts a connection or `spec.json` is younger than 5 s (a shim still starting); otherwise `Lost`. The child is found by `child.json` (pid and start time).
- Race found by the tests and fixed: a shim that has been started but has not yet written `shim.json` or bound its socket was classified `Lost` by a daemon that adopted at that moment; the `spec.json` age rule above is the fix.
- A write waits up to `PhaseOptions.reply_wait` for the watcher's connection when a phase has just started, so a caller may write right after `start_phase`. The journal sequence that `write` returns numbers the child's output too, so consecutive input writes are increasing, not consecutive.

### Decisions for the owner

1. Threads, not tokio. Choice: `std` threads (one watcher per phase, one reaper per shim the daemon started). Alternative: tokio tasks. Recommendation: keep threads; a daemon has a handful of phases, no async runtime was added, and the tokio line in the plan was for reaping, which one blocked thread per shim does. Revisit if the daemon must serve hundreds of shims.
2. "The agent succeeded" is read as "the child exited with code 0" until Stage 3b's stream parser exists (`phase_ending.result_seen`). Alternative: wait for the parser before adopting any exit as `ended`. Recommendation: keep; the tests that need a `result` before a SIGKILL print one, and the state machine already makes `lost` win over it. Product-behaviour note: a child that exits 0 without a `result` ends as `ended` until 3b.
3. Shim identity file `shim.json` (pid plus start time), written by the shim next to `child.json`. Alternative: take the pid from the socket's peer credentials only. Recommendation: keep; a quiet or stopped shim has no connection to ask, and the start time is what stops a reused pid from being adopted.
4. A shim starts the daemon only when its `spec.json` carries `daemon: {exe, env}`; the real daemon always writes it, tests that run a shim alone leave it out. Alternative: always start one from `$PATH` or a fixed path. Recommendation: keep; it keeps tests off the real home and lets the shim use the same root and home as its daemon.
5. Ack wait: a shim whose record is on disk waits 60 s for an `AckExit` and then exits (the record stays; the next daemon reads it), and waits without limit when the record could not be written. Named `ACK_WAIT`, injectable as `Limits.ack_wait` with `run_with`. Alternative: wait forever. Recommendation: keep 60 s; 60 s is a guess and no plan figure exists.
6. Unresponsive timings: `PhaseOptions.reply_wait` 5 s, `unresponsive_after` 5 s, `bind_wait` 5 s (all injectable). The plan names none. A shim is unresponsive when stopped (kernel status) or silent in the handshake for `unresponsive_after`, or a request has no answer for `reply_wait`. Unresponsive is not lost: the phase stays `running`, and a write that timed out may still be delivered after SIGCONT. Product-behaviour note: the UI should word it as "not answering" and never offer a second write as a retry without saying the first may still arrive.
7. Failed messages from a shim carry a `sink problem: ` prefix for unsolicited notices, so a request failure is never confused with one. Alternative: a new message type. Recommendation: keep for now; a new type means a protocol bump.
8. When a phase is `lost` and its child still runs, the daemon kills the child's process group with SIGKILL (checked by pid plus start time from `child.json`). Alternative: leave it and only report. Recommendation: keep; a child with no shim has no journal and no one to ack it. Product-behaviour change: a lost phase's work is stopped, not left running.
9. A damaged `exit.json` is moved aside to `exit.json.damaged-<ms>` and the phase is `lost`. Alternative: leave it and retry reading. Recommendation: keep; it is read once and never written over.
10. The shim runs in a session of its own (`setsid`) with a cleared environment, and `shim.log` holds its stderr. Signals to children go to the process group only, never a bare pid.
11. New dependency: `serde = "1"` in `host` (no new crate in `Cargo.lock`). Alternative: make the framing concrete over the two message types. Recommendation: keep.

- Owner decisions (2026-11-15): the recommendations above are accepted as built. Killing a lost phase's child follows the shim module's rule (the daemon stops that child when it marks the phase `lost`) and checks the recorded start time first, so a reused pid is never signalled. Follow-up for ts-0059-xs: decide whether that stop goes through the stop order (TERM, grace, KILL) and the descendant walk instead of an immediate SIGKILL.

### Not done, by design

- ts-0058 (launchd): the launchd job that keeps the daemon alive across logout and login. Here the daemon is started by the shim or the CLI through `ensure_daemon` only.
- ts-0059 (stop): stopping a phase on purpose (`Signal` from the daemon, `stopped` as an outcome, shutting shims down). `Phases::stop` only ends the daemon's own watchers, and the shims keep running. `Stopped` is never produced here.
- Stage 3b first run: the engine that starts the next phase on its own, the stream parser that supplies `result_seen`, and a `run.start` RPC. The end hook is the seam for it; the item shows that a phase can start from the hook inside an adopting daemon, not that an engine chooses it.
- The daemon's own login-session record (a step of the start sequence that `daemon.rs` has marked not built since ts-0056-ag) is still not built; nothing in this item needs it.
