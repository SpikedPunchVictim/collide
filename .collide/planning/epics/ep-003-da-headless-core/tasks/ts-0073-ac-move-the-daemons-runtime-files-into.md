---
id: ts-0073-ac
kind: task
status: done
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

### Built

- `host/src/daemon.rs`, `DaemonPaths::for_home`: the socket directory, `d.sock`, `token` and `lock` are now under `<home>/Library/Application Support/Collide/run`. `runs`, `store` and `daemon.log` stay in `<home>/Library/Application Support/Collide`. The doc comment of `for_home` says so. `DaemonPaths::under` is unchanged.
- Tests, written first and seen failing:
  - `host/src/daemon.rs`: `a_home_gives_the_run_files_in_application_support_and_the_data_where_it_was` (every path, for a made-up home); `a_home_makes_no_folder_of_its_own`; `the_default_socket_fits_for_an_ordinary_home` (now also runs `auth::check_socket_path`); `a_home_too_long_for_the_socket_fails_with_the_path_and_the_limit` (the message names the socket path and the limit); the `from_env` test now expects the new location.
  - `host/tests/auth.rs`: `the_home_socket_directory_is_made_0700_below_folders_that_were_not_there` makes the socket directory of `for_home(<temp root>)` with `check_socket_dir` and `StdFs`, with the data folder missing, and checks mode `0700` and that a second check accepts it.
- Read of the code: `launch.rs` (`StartRequest::new`, `ensure_running_from_cli`), `shim.rs` (`start_daemon`), `run_cli.rs`, `stop_cli.rs` and `bin/collide-daemon.rs` all take their paths from `DaemonPaths` (`from_env`, `under`, `for_home`), so the lock, the launchd job and the shim's start follow without a change of their own. The plist's log path is `DaemonPaths::log_file`, which did not move.
- Search of tracked files (excluding `private/`): the only current-behavior mention of `~/.collide/run` was the `for_home` doc comment. The others are records and are left as they are: ts-0056-ag Notes (decision 2 and the owner decisions line), this item's own Goal and Notes, and IMPLEMENTATION_PLAN.md revision 20's description of the move.

### Decisions for the owner

1. **Mode of the Collide folder.** I chose to change nothing: `check_socket_dir` makes the socket directory with `DirBuilder::recursive` and mode `0700`, so when the daemon's first start makes `run` before anything else made the data folder, `~/Library/Application Support/Collide` is also `0700`. The alternative is to make the data folder first with the default mode. Recommendation: keep it; a folder only the user can open is what Collide wants, and nothing needs it open.

- Owner decisions (2026-11-15): decision 1 taken (the Collide folder may be made `0700` by the daemon's first start). No plan change: revision 20 already describes the move.

### Not done, by design

- No migration, and no removal of a stale `~/.collide/run`.
- No change to `DaemonPaths::under`, the store, the runs or the log.
- No edit to IMPLEMENTATION_PLAN.md, no status change, no commit.
- No check against a real daemon or the real home; that is ts-0071-fd.
