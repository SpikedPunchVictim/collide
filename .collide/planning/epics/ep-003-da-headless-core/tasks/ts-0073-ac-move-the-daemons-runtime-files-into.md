---
id: ts-0073-ac
kind: task
status: ready
title: "Move the daemon's runtime files into Application Support"
owners: []
links:
  - { type: blocked-by, to: ts-0070-mk }
---

## Goal

A real daemon keeps its socket, token and lock file in `~/.collide/run` (`DaemonPaths::for_home`, ts-0056-ag). Move them to `~/Library/Application Support/Collide/run`, beside the store, the run folders and the log, so Collide makes no folder of its own in the home directory (owner, 2026-11-15; plan revision 20, 0.2 "Where Collide's files live on macOS"). `~/.collide` reads like a repo's `.collide/` folder.

Nothing else moves. `DaemonPaths::under` (the `COLLIDE_DAEMON_ROOT` layout of `run/` and `data/`) is unchanged.

## Success criteria

- `DaemonPaths::for_home` puts `socket`, `token_file`, `lock_file` and `socket_dir` under `~/Library/Application Support/Collide/run`; the store, runs and log paths are unchanged.
- The socket directory is still made with mode `0700` and checked at start (0.2, The socket), although its parent is the data folder.
- A test pins the new paths, and that the socket path for a home of a typical length fits in `MAX_SOCKET_PATH_BYTES`. A home long enough to pass the limit fails fast with the message that names the path and the limit (as before).
- No doc comment, help text or task Note that describes current behavior still names `~/.collide/run`.
- The launchd job's plist and the shim's daemon start take their paths from `DaemonPaths`, so they follow without a change of their own; a test or a read of the code confirms it.

## Tests

Write these first:

- `for_home` with a fixed home gives the new socket, token, lock and socket-directory paths, and the old data paths.
- The length check on a typical home, and on one long enough to fail.

## Notes

- IMPLEMENTATION_PLAN.md: 0.2 (Where Collide's files live on macOS; The socket); revision 20.
- No migration: no daemon has run from the old location outside tests. A stale `~/.collide/run` left by a manual run is not removed by Collide.
- Must land before `ts-0071-fd`, whose owner checks run a real daemon in the real locations.
- Area: `host` (`daemon.rs`, and any doc that names the old path).
- The test rule: no test touches the real home, real launchd or a real daemon. Each test clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as `host/tests/common/mod.rs` `TempRoot` does.
