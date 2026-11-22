---
id: ts-0164-pa
kind: task
status: ready
title: "App and daemon: a HOME that is gone is made again"
owners: []
links: []
---

## Goal

In `ts-0138-pn` step 6 (2026-11-21) the owner started the app from a terminal that still held the check's `env.sh`,
whose `HOME` and `COLLIDE_DAEMON_ROOT` named a folder already deleted. The app started a daemon there, which made the
folder again, and ran with no word that its home was new. The app never saw the real daemon, so the page showed none of
the owner's runs, and it looked like a bug of the page.

## Success criteria

- The daemon refuses to start when `HOME` names no folder, and says so with the path; it never makes `HOME`.
- The app shows that refusal as its words, not as an empty page.
- `COLLIDE_DAEMON_ROOT` that names no folder is still made, as today (tests and checks make it fresh).

## Tests

- Write first and see red: the daemon binary with `HOME` set to a path that does not exist: it exits with the message,
  and the path is still absent.

## Notes

- Recommendation: as the criteria say. Alternative: start, and show the daemon's `HOME` on the page. Less safe: the
  wrong home still gets written.
- Low priority. The check's own text now says to look at `echo $HOME` before starting the app (`ts-0138-pn` step 6).
- Owner decisions: none recorded yet. The recommendations stand until the owner answers.

### Built

Found first. Nothing in the daemon makes `HOME` by name. Folders below the daemon's root are made, and the root can be
below `HOME`: `COLLIDE_DAEMON_ROOT` inside a deleted home. Three places made those folders before the daemon ran:

- The start, `launch::ensure_daemon`, in its `lock` step (the lock file's folder, with parents). The CLI, the app's
  `LaunchStarter` and the shim's daemon start all go through it (`ensure_running_from_cli`).
- The app's `DaemonAccess::connect`, which wrote the app's file below the run folder
  (`auth::write_app_executable`, then `check_socket_dir`) before it asked the starter to start anything.
- The daemon itself, in `collide-daemon` (log file, store, socket folder).

How the app learns of a failed start: `DaemonAccess::connect` maps the starter's `Err(String)` to an `UNAVAILABLE`
`RpcError` ("the daemon could not be started: ..."). The page shows an error of a call as
`Could not reach Collide: <message>` in its alert (`loop.ts`, `sync_failed`). The refusal uses that path. No new
channel, no new wire type, and the page's text is unchanged, so there is no page test.

The CLI and the app use one code path (`ensure_daemon`), so the CLI prints the same words: `collide stop` prints
`the daemon is not running and did not start: HOME names no folder: ...`. Other CLI commands do the same through
`ensure_running_from_cli`.

Changed:

- `host/src/daemon.rs`: `check_home(home)`. A `HOME` that is set, not empty, and not a folder is refused with a message
  that names the path. Unset and empty pass (today's behavior).
- `host/src/bin/collide-daemon.rs`: `main` calls `check_home` first, for `collide-daemon` and `collide-daemon start`,
  before any path is made. Exit code 1, as every other refusal of the binary.
- `host/src/launch.rs`: `StartError::HomeGone(message)`; `ensure_daemon` checks the home the daemon will have (the
  request's `HOME`, else this process's) before the lock.
- `app/src-tauri/src/shell.rs`: `DaemonAccess::with_home`; `connect` checks first and answers `UNAVAILABLE` with
  "the daemon could not be started: HOME names no folder: ...". `app/src-tauri/src/lib.rs` passes the app's `HOME`.
  The check is not counted by the restart policy: no start was tried.
- No existing test relied on a `HOME` that was not made (the ones with `root/home` make it first); the full run below
  shows none failed for it.

Tests, with red at runtime against the code before the change (run before the code was written, except where noted):

- `host/tests/daemon_process.rs`:
  - `a_daemon_whose_home_names_no_folder_refuses_and_makes_nothing`: red (the daemon ran and made the root's folders).
  - `a_start_below_a_home_that_names_no_folder_refuses_and_does_not_make_it`: red (it started, exit 0).
  - `the_cli_prints_the_refusal_when_its_home_names_no_folder`: red (`collide stop` started a daemon below the home).
  - `a_start_that_names_a_home_in_its_request_is_refused_when_it_is_gone`: red (it made the home and ran the stand-in).
  - `a_root_that_names_no_folder_is_still_made_by_a_start`: the guard. Green before and after, as expected; no other
    test starts a daemon under a root that does not exist yet.
- `app/src-tauri/tests/daemon_link.rs`:
  `a_home_that_names_no_folder_is_refused_in_words_and_nothing_is_made_or_started`: red with `with_home` stubbed to
  do nothing (the message was the starter's own, and the home was made).
- `host/src/daemon.rs` unit tests: `a_home_that_is_unset_empty_or_a_folder_passes_the_home_check`,
  `a_home_that_names_no_folder_is_refused_with_its_path` (written with the function, no red run).

Every test that starts a process clears the environment, and uses a temp `HOME` and `COLLIDE_DAEMON_ROOT`. The binary
tests that could serve forever run under a deadline (`output_within`), and the daemons they might start are stopped by
a guard.

### Decisions for the owner

- **Where the check lives: three places, one function.** `check_home` runs in the daemon, in `ensure_daemon` and in
  the app's `connect`. The app's check is needed because it writes below the home before any start. Alternative: put
  it in `DaemonPaths::from_env`. That would also refuse in every CLI command and make the app abort at setup instead
  of showing the page. Recommendation: as built.
- **Exit code and words.** Exit 1, as the binary's other refusals. The message: "HOME names no folder: <path>.
  Collide does not make it, so nothing was started. Make the folder, or set HOME to one that exists". Alternative: a
  new exit code. Recommendation: as built; nothing reads it.
- **Unset or empty `HOME` keeps today's behavior.** With a root set it is fine; with no root `from_env` already
  refuses. Alternative: refuse an unset `HOME` too. Recommendation: as built, since the item is about a path that
  names no folder.
- **A `HOME` that is a file is refused too** (it "names no folder"). Alternative: only a missing path. Recommendation:
  as built.
- **The app's refusal is not counted as a failed start.** The app shows it at each call and each pump retry, and
  never says "stopped trying", which would point at the log. Alternative: count it. Recommendation: as built.

Proposed plan text (the plan's Starting the daemon section): "A start refuses a `HOME` that names no folder, before it
makes anything, and the daemon does the same; the words name the path. `COLLIDE_DAEMON_ROOT` that names no folder is
still made."

### Not done, by design

- The app's event pump says the refusal in its stderr line and as a lost connection; the words reach the page through
  its calls only. A banner fed by the pump is not part of this item.
- A `HOME` that is gone while the daemon runs is not watched. The check is at the start only.
