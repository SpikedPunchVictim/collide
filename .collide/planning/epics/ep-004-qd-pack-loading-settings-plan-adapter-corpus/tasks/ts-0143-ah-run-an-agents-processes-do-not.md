---
id: ts-0143-ah
kind: task
status: ready
title: "Run: an agent's processes do not carry the runtime mark, so a stop signals them"
owners: []
links: []
---

## Goal

`collide stop` signals no process of a run's agent. The daemon carries `COLLIDE_RUNTIME=1` (`RUNTIME_MARKER`), set by
`launch.rs` both on the CLI's start and in the launchd job. Its base environment is a login shell's capture, which
inherits the mark (`host/src/base_env.rs`), and `BaseEnv::for_child` strips only `ALWAYS_STRIPPED` and the adapter's
list. So the agent child carries the mark too, and `find_tree` in `host/src/stop.rs` skips it as a runtime process.
Found by the agent run of `ts-0121-xp`'s checklist (2026-11-19): "0 process(es) signalled"; the stand-in `claude` ended
only when its standard input closed. A real `claude` that ends on the interrupt hides this; a hung agent, or a tool it
started, is never signalled.

## Success criteria

- No child the daemon starts for a run (the agent, and what it starts) has `COLLIDE_RUNTIME` in its environment,
  whatever the captured base environment holds.
- `collide stop` of a run whose agent ignores the interrupt and the closing of its standard input signals the agent's
  tree, and the run ends `stopped` within the grace.
- The daemon and the shims still carry the mark, and a stop or a sweep still never signals them.

## Tests

- Write first and see red at runtime: `for_child` of a capture that holds `COLLIDE_RUNTIME=1` gives an environment
  without it.
- A daemon test: the daemon is started with the mark (as the CLI starts it), the fake agent ignores the interrupt and
  its standard input, and `stop` signals it (red today: 0 signalled).

## Notes

- Recommendation: add `RUNTIME_MARKER` to `ALWAYS_STRIPPED`, so one list says what no child inherits. The shim sets the
  mark on its own command (`phases.rs`), which this does not touch; check that before choosing.
- The builder lists, in Built, any other variable the daemon's own environment hands to an agent through the capture
  (the CLI's start sets some), and says whether each is right.
- Build it first, before `ts-0137-pg`: a stop that does not stop is a safety gap for every real run.
- Owner decisions: none recorded yet. The recommendations stand until the owner answers.

### Built

- `host/src/base_env.rs`: `RUNTIME_MARKER` (from `host/src/stop.rs`) is in `ALWAYS_STRIPPED`, so `for_child` drops it
  from every child it makes, whatever the capture or the minimal fallback holds. The constant is now five names and its
  doc says why. Nothing else changed: the shim's own command still sets the mark in `spawn_shim`
  (`host/src/phases.rs`), and does not go through `for_child`. The shim runs the agent with `env_clear` and the
  phase spec's env, which is what `for_child` gave, so one change reaches the agent and what it starts.
- Tests. `host/src/base_env.rs`: `the_runtime_marker_is_always_stripped`. `host/tests/base_env.rs`:
  `a_child_never_gets_the_runtime_marker_the_capture_inherited_from_the_daemon` (also checks `COLLIDE_DAEMON_ROOT` and
  another variable are kept) and `a_child_never_gets_the_runtime_marker_the_minimal_environment_holds_either`.
  `host/tests/phases.rs`: `a_stop_signals_an_agent_started_with_the_environment_a_marked_daemon_captured`.
- The daemon test. The daemon runs in the test process, as in the other `phases.rs` tests, so it cannot carry the
  mark itself. Its login shell is a script that runs the capture under `env -i` with `COLLIDE_RUNTIME=1`, the daemon's
  root, `HOME` and `PATH`, which is what a login shell under the live daemon inherits. The capture is the real
  `BaseEnvironment::capture_with`, and the test asserts it holds the mark. The agent's env is `for_child(&env_strip(),
  ..)` of that capture plus `COLLIDE_RUN`, as `FileRuns` makes it. The agent ignores SIGINT, SIGHUP and SIGTERM and
  never reads its stdin. After `stop_run` the test checks the agent was signalled, is gone, the phase is `stopped`,
  and the agent saw no mark and did see `COLLIDE_RUN`, `COLLIDE_DAEMON_ROOT` and `HOME`. Not tested: a real
  `run.start_file` through a daemon process (it needs a git repo, `models.yaml` and a stand-in that speaks
  stream-json), so the test starts the phase itself and does not go through `file_run.rs`.
- Seen red at runtime, with the code compiling and the fix not yet made: all three new integration tests. The daemon
  test failed on `left: []`, `right: [<agent pid>]` (0 signalled). The unit test was seen red by replacing the new
  name in the array with another and restoring it.
- Every other variable the daemon's environment hands to an agent through the capture. The CLI's start forwards
  `COLLIDE_DAEMON_ROOT`, `HOME`, `SHELL` and `COLLIDE_LAUNCHCTL` when set (`FORWARDED_ENV`, `LAUNCHCTL_ENV`); the
  launchd job adds `USER` and `PATH`; both strip `COLLIDE_RUN`, `COLLIDE_RUN_START` and `COLLIDE_SPAN`. The login shell
  then adds its own.
  - `COLLIDE_DAEMON_ROOT`: right. An agent's `collide` must reach the same daemon (revision 24). Kept, tested.
  - `HOME`, `USER`, `SHELL`, `PATH` (the shell's): right. The base environment exists to give them.
  - `COLLIDE_LAUNCHCTL`: not right in principle. It names a stand-in `launchctl` for a test start, so it is set only
    under a test root, never in a real run. Left alone.
  - `PWD`, `OLDPWD`: the daemon starts in `/`, so a capture may hold `PWD=/`, which is not the agent's folder.
    Untested and left alone; `claude` and its tools use their own working directory.
  - The shell's own (`SHLVL`, `_`, `LOGNAME`, `TMPDIR`): harmless.
- A role's `env` is layered after the strip (`layered_env`); at review the mark is removed after it too (Decisions 3).

### Decisions for the owner

1. Take the recommendation: `RUNTIME_MARKER` in `ALWAYS_STRIPPED`. Reading the code did not change it. The only code
   that sets the mark on a child is `spawn_shim`, which does not use `for_child`; every path that makes an agent's
   env (`file_run.rs`, twice) or a provider's (`sources.rs`) goes through `for_child`. Alternative: strip it in
   `file_run.rs` only. That leaves providers and any later caller exposed. Recommend the list.
2. Providers get the strip too. A source provider is started with `for_child` (`sources.rs`), so it no longer carries
   the mark. No stop sweeps a provider (it has no `COLLIDE_RUN`), so nothing changes for it, and the daemon's own
   process is still marked.
3. Changed at review: the mark is also removed after the layers over the base, last in `for_child`, so a role's `env`
   (it reaches `for_child` as `extra`) cannot give it back and make its agent unstoppable. Test
   `a_layer_over_the_base_never_gives_a_child_the_runtime_marker` (`host/tests/base_env.rs`), red with that line
   removed. The builder had recommended waiting for a role that sets it.
4. Plan text, for `IMPLEMENTATION_PLAN.md` 0.2 (The base environment, last bullet): "... terminal session ids
   (`TERM_SESSION_ID`, `ITERM_SESSION_ID`), `XPC_SERVICE_NAME`/`XPC_FLAGS` and `COLLIDE_RUNTIME` (the daemon carries
   it, its login shell hands it on, and a child with it would never be stopped)." Not applied: the plan is the
   reviewer's.

### Not done, by design

- A test through `run.start_file` in a daemon process with a stand-in `claude`: no item names it. It would catch a
  future `file_run.rs` path that skips `for_child`. Worth adding with the `ts-0137-pg` run checks if the owner wants it.
- `COLLIDE_LAUNCHCTL` and `PWD` in the agent's env (above): left as they are.
