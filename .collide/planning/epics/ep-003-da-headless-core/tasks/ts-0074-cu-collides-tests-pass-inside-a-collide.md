---
id: ts-0074-cu
kind: task
status: done
title: "Collide's tests pass inside a Collide run"
owners: []
links:
  - { type: blocked-by, to: ts-0073-ac }
---

## Goal

In the dogfood run of `ts-0070-mk`, the agent ran Collide's own `cargo test --workspace` inside its phase and got 183 failures. Two causes:

- About 100 tests start runs, and the daemon refused them with "may not start a run": the test process inherits the agent's `COLLIDE_RUN`, `COLLIDE_RUN_START` and `CLAUDE_CODE_AGENT`, so the test's client looks like a run's own agent (ts-0066-zb's refusal of `Caller::RunAgent`).
- About 85 tests could not find `node`: the phase's `PATH` had no `/opt/homebrew/bin`. That daemon was started by a build agent with a cleared environment, so this half may not happen under a real daemon; `ts-0071-fd` checks it.

Make the first cause go away so a Collide run can build Collide (part L). A test that starts a run, or talks to a daemon as a person, must not carry the run variables of the process that runs the tests.

## Success criteria

- `cargo test --workspace` passes with `COLLIDE_RUN`, `COLLIDE_RUN_START`, `COLLIDE_SPAN` and `CLAUDE_CODE_AGENT` set in its environment, as they are inside a phase. A test pins this: it runs a representative test binary as a child with those variables set and checks it passes.
- The fix is in the test helpers (`host/tests/common/mod.rs` and the fake-Claude helpers), not a weakening of the daemon's refusal: a real run's agent is still refused `run.start_file` and `queue.answer` (ts-0066's test stays).
- If a test needs `node` and cannot find it, it fails with a message that says so, not a confusing error.

## Tests

Write these first:

- the child-run check above, red before the fix.

## Notes

- `ts-0070-mk` decision 9.
- IMPLEMENTATION_PLAN.md: part L (Dogfood); 0.3 (the context variables after the strip); 0.2 (a run's agent may not start a run).
- Area: `host/tests/` helpers; the client's caller detection in `host/src/owner_client.rs` if it reads the process environment directly.
- The test rule: no test touches the real home, real launchd or a real daemon. Each test clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as `host/tests/common/mod.rs` `TempRoot` does.

### Built

**The real path.** The cause is not a leaked variable. The daemon decides the caller in `host/src/auth.rs` (`classify_peer`): it walks the peer's process and its parents and reads each one's `COLLIDE_RUN` with `KERN_PROCARGS2` (`sysproc::env_var_of`), which is the environment the process was *started with*. Nothing is sent in the handshake and `host/src/owner_client.rs` reads no variable. A test process started inside a phase is itself in that walk: a client in the test's process (in-process daemon), or a `collide` child of it (process daemon), has the test process as peer or parent, and it carries `COLLIDE_RUN`. So `env_clear()` on the child did not help, and `remove_var` would not either (the system's answer is the startup environment). The other path is `ProcessTree` in `real_peer.rs`: its own `CutAtThisProcess` stopped the walk above the test but still read the test's own marker.

The second cause is `common::build_packs`: `Command::new("node")` with the inherited `PATH`, `expect("node should run the build script")`, so a missing `node` showed as `Os { code: 2, NotFound }` in 85 tests.

**Changes.**
- `host/src/daemon.rs`: `DaemonOptions.process_tree: Option<Arc<dyn ProcessTree + Send + Sync>>` (default `None`, the system's tree as before); `Shared.tree` is that trait object. `auth.rs` is untouched: the refusal and `classify_peer` are as they were.
- `host/tests/common/mod.rs`: `StopsAtThisTest` (the system's tree, but the test process is not read and the walk ends there; a child's own marker is still read) and `test_process_tree()`; all four in-process daemons (`daemon_socket.rs`, `phases.rs`, `adapters/claude/file_run.rs`, `pause.rs`) set it.
- `host/tests/common/detached.rs`: `Detached`, a command started through a shell that is orphaned before the command starts, so the test is not among its parents (a person at a terminal). `run_process.rs` (`Proc::start`, `collide`) and `stop_command.rs` (`collide`) use it; the "killed command" test uses `lines()` and `kill()`.
- `host/tests/real_peer.rs`: `CutAtThisProcess` no longer reads the test's own marker.
- `common::build_packs` checks `PATH` first: `node_missing_message` names `node` and every folder searched.
- Tests: `host/tests/adapters/claude/run_env.rs` (two pinning tests, red before the fix with "the agent of run run-x may not start a run"), and two in `host/tests/builtin_packs.rs` (the message; a child run on an empty `PATH` fails and names `node` and that `PATH`).

**Found by the gates (review).** A gate run failed in `file_run` with "moving …/store.db-wal.keep aside: No such file or directory". `host/src/store.rs` `keep_side_files` checked that the WAL was there and then linked it, and the daemon's connection could close between the two, which makes SQLite delete the WAL; the test's own `Store::open` (`record()`) then failed. Only tests open the store beside a running daemon. `keep_side_files` and `release_side_files` now try the link and the removal and take "not there" (and, for the link, "already there") as nothing to do. Three unit tests pin those branches; the race itself is not reproduced in a test.

**Pinning tests.** Each re-runs this binary (`current_exe()`) with `--exact` on one other test, `env_clear()`, `PATH` and the four variables; one in-process daemon (`run_command::...only_the_version_in_use_is_printed`), one process daemon (`run_process::...exits_0`). No recursion guard: `--exact` on another test cannot reach the pinning tests; the check that exactly one test ran stands in for it.

### Decisions for the owner

1. **Seam for in-process daemons.** Chose `DaemonOptions.process_tree` (dependency injection of the existing `ProcessTree` trait). Alternative: a test-only switch or env variable in the daemon, or loosening the refusal (for example, ignoring markers of runs the daemon does not know). Recommendation: keep the injection; the daemon's refusal and the real tree are unchanged, and a run agent's refusal tests stay and pass.
2. **Seam for a daemon process.** Chose no product change: the `collide` command is started through `common::detached` so the test is not its parent. Alternative: a variable the daemon reads to cut its walk (a test switch in a security component), or a cargo `runner` that strips the variables (outside the test helpers, and the pinning test could not then run the binary directly). Recommendation: keep it.
3. **Whether to also run Collide's tests inside a real phase.** Chose the pinning tests (a phase's environment, simulated). Alternative: also run them in a real run. Recommendation: `ts-0071-fd` does that once, owner-run.
4. **Plan text (propose, not applied).** In IMPLEMENTATION_PLAN.md part L, add: "Tests run inside a run: the daemon reads the environment a process was started with, so tests must not rely on `env_clear()` of a child alone; a test that starts a run gives its in-process daemon a tree that stops at the test (`DaemonOptions::process_tree`) or starts the command through `common::detached`."

- Owner decisions (2026-11-15): decisions 1 to 3 taken. Decision 4's plan text applied, shortened, to Stage 3b's Dogfood bullet. The builder's own scratch logs (five files in its `mktemp -d` folder under `/var/folders`) were left there: a safety check blocked their removal, and the system cleans that folder. The store's side-file race, found by this item's gates, was fixed here (review).

### Not done, by design

- `launchd_run.rs` (opt-in, `COLLIDE_REAL_LAUNCHD=1`, skipped by default) still starts `collide` as a child with `Proc::command`; it is owner-run, with a real launchd, and belongs to `ts-0071-fd`.
- Tests that need `node` still fail without it (by design: not skipped); they now say so with the `PATH` searched. Nothing was done to make `node` appear on a phase's `PATH`; that is `ts-0071-fd`'s question.
- No change to `host/src/auth.rs`, the refusal of `Caller::RunAgent`, or any task status; nothing committed.
