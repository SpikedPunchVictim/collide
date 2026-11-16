---
id: ts-0077-ws
kind: task
status: active
title: "Shell: connect the app to the daemon"
owners: []
links:
  - { type: blocked-by, to: ts-0076-cw }
---

## Goal

The Tauri shell talks to the real daemon (`app/src-tauri/src/shell.rs`, `app/src-tauri/src/lib.rs`). It fills the two
Stage 3c stubs of the shell:

- `shell: daemon connection` (`SocketLink`):
  - the owner socket, the token file and the `initialize` handshake, then each call;
  - the daemon started on demand when the socket does not answer (0.2, Starting the daemon);
  - `daemon.recapture_environment` once the app has connected at start (0.2, The base environment).
- `shell: daemon events` (`pump_daemon_events`): `daemon.subscribe` on a connection of its own, and each
  `daemon.event` and `queue.changed` notice put on the `collide:event` channel.

Carried from 3a: `ts-0052-xt` left "the app telling the daemon where it is, and the app's own connection" to this
stub. The daemon knows a peer is the app only through `DaemonOptions` `app_executable`, and nothing sets it yet. So
the app's calls are taken as the CLI's, whose person-only operations wait for a confirmation.

## Success criteria

- A call through the shell's `rpc` command reaches a temp-root daemon and returns its answer. The token is read
  from the token file, and never reaches the webview or the event channel (0.2).
- With no daemon running, the first call starts one and then succeeds. After three failed starts in five minutes the
  shell stops trying and says so (0.2; `host/src/launch.rs` `RestartPolicy`).
- A connection that drops is made again on the next call. A call whose request was already sent is not sent again:
  it fails and says the connection dropped.
- The event pump forwards events in `seq` order. After the daemon restarts, it subscribes again from the last `seq`
  it forwarded, with none missing.
- The daemon takes a connection from the app as the app (`host/src/auth.rs` `Caller::App`), not as the CLI.
- Both shell tags count no stub. They move to `FILLED_TAGS` in `checks/tests/real_tree.rs`.

## Tests

Write these first:

- `rpc` to `daemon.status` through `SocketLink`, against a temp-root daemon;
- a token file with the wrong mode is refused, and says why;
- the start on demand, and the stop after three failed starts (with a starter that fails and a test clock);
- a dropped connection: the next call reconnects, and a call in flight fails without being sent twice;
- the pump: events in order, then a daemon restart, then the rest with none missing;
- the app's connection is admitted as `Caller::App`;
- nothing the webview receives holds the token (extend `app/src-tauri/tests/rpc.rs`).

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 3c: what it pulls from Stage 6 (the `rpc` command and event channel, with the token).
  - 0.2: The UI speaks the owner's protocol; Starting the daemon; The base environment; Who is asking.
  - Stage 6a: the Tauri shell (finished there: the CSP and the full capability set).
- **Decision for the item, with a recommendation: how the daemon learns the app's executable.** Recommend that the
  app writes its own executable path to a file in the daemon's run folder, mode `0600`, before it connects, and that
  the daemon reads it when it checks a peer. That covers a daemon the CLI started first. The alternative, a path in
  the `initialize` params, takes the caller's word for who it is.
- **Decision for the item: the client code.** Recommend that `SocketLink` wraps `host/src/owner_client.rs`
  `OwnerClient` and `launch::ensure_daemon`, as `launch::ensure_running_from_cli` does for the CLI, so the app and the
  CLI connect the same way.
- **Decision for the item: where the app finds `collide-daemon`.** Recommend beside its own executable, as the CLI
  does. Packing it into a bundle is later.
- "Per-client token" in Stage 3c is the CLI token file of 0.2. Named per-client tokens were not built (owner,
  2026-11-14).
- Tauri 2 may refuse the webview's `listen` on the event channel with no capability file. If it does, add one that
  allows only the event channel for the main window. The full set is Stage 6a's.
- 0.2 says the resident app restarts the daemon. The resident app (the menu bar) is Stage 9b. Here the window
  restarts it on demand, on a call.
- Area: `app/src-tauri`, `checks/tests/real_tree.rs`, and `host` only if the daemon's check of the app changes.
- The test rule: no test touches the real home, real launchd or a real daemon. Each test clears the environment
  (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as `host/tests/common/mod.rs` `TempRoot`
  does. Settings files are fixtures, never the owner's own `settings.json`.
- Stage 3c item (revision 22 scope, split 2026-11-15). Items of Stage 3c run one at a time, in id order, because
  they share `host`, the app and the generated files.

### Built

The app's shell reaches the daemon over the owner socket, starts it on demand, carries its notices to the window, and
the daemon takes the app's connection as `Caller::App`. No wire type changed, so `pnpm gen` had nothing to regenerate.
Nothing of ts-0078 was built.

- `host/src/auth.rs`: `write_app_executable` (the app's path in `app-executable` in the run folder, mode `0600`, written
  to a temporary file and renamed, the folder checked by `check_socket_dir`), `read_app_executable` (a plain file, not
  a link, mode `0600`, the daemon's owner, not empty), and `SystemProcessTree::with_app_file`. `is_app` uses an explicit
  path first, else the file, read at each check, so a daemon that the CLI started first still knows the app.
- `host/src/daemon.rs`: `DaemonPaths::app_executable_file`, `system_process_tree` (the daemon's default tree when its
  options give none), and the connection log line names the caller ("pid N connected as App"). Module doc, "Who is
  asking", says where the app is learned.
- `host/src/owner_client.rs`: `is_closed` (a non-blocking peek), `set_read_wait`, and `next_notice` returning `Incoming`
  (`Notice`, `Quiet`, `Closed`). A line cut in two by a wait is kept and read whole by the next call.
- `app/src-tauri/src/shell.rs`:
  - `DaemonStarter`, `LaunchStarter` (`launch::ensure_running_from_cli`), `DaemonAccess` (paths, starter, clocks, the
    app's executable, and the rule of 3 failed starts in 5 minutes on the continuous clock; it only connects after the
    rule trips, so a daemon someone else started is still used).
  - `SocketLink`: one connection, made again when the daemon closed it between calls. A call that fails under it drops
    the connection and says so; it is never sent again, because it may have taken effect. `CALL_WAIT` is 120 s.
  - `pump_daemon_events`: its own connection, `daemon.recapture_environment` once at the app's first connection,
    `daemon.subscribe` from the last `seq` it forwarded (an event at or below it is not sent twice), and a retry wait
    when the stream ends. It stops on its flag within 200 ms.
- `app/src-tauri/src/lib.rs`: `daemon_access` (paths from `COLLIDE_DAEMON_ROOT` or `HOME`, the canonical executable),
  one `DaemonAccess` shared by the link and a `collide-events` thread.
- `app/src-tauri/capabilities/main.json`: the main window may `core:event:allow-listen` and `allow-unlisten`, nothing
  else.
- `checks/tests/real_tree.rs`: both tags are in `FILLED_TAGS`; `SHELL_TAGS` keeps `shell: ask token`.

Tests, written first. Red means seen failing before the code:

- `host/tests/app_file.rs`, 11 tests; 9 red against stubs (two passed against them, as stubs that return nothing meet
  their "no app" cases).
- `host/tests/owner_client.rs`, 6 tests; 5 red (one passed against the stubs).
- `app/src-tauri/tests/daemon_link.rs`, 25 tests (calls, answers by id, a token file of the wrong mode, start on
  demand, the 3-in-5-minutes rule and its window, `Caller::App` from a daemon the app did not start, a dropped
  connection, a daemon that does not answer, the pump's order and resubscribe, the pump's stop, a real daemon restart
  with no event missing, the environment asked for once); 24 red against the `todo!` stubs, and the one that passed
  checks that a terminal is still the CLI.
- `app/src-tauri/tests/rpc.rs`, 3 added: the token in nothing the webview receives, a refused token not echoed, the
  capability file. Not seen red alone: they were added after the stubs were replaced. The real-tree check
  `every_filled_tag_stays_registered_under_its_stage_and_counts_no_stub` was seen red.
- `daemon_link` and `rpc` ran 8 times in a row, all green after one fix: a session of the daemon looks at its stop
  flag every 100 ms, so one test waits 500 ms after it ends the daemon.

Checks: `cargo build --workspace`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`,
`cargo fmt --all --check`, `pnpm test`, `pnpm typecheck`, `pnpm --filter @collide/app build` and `pnpm check` all pass.

### Decisions for the owner

- **The app's path goes in a file, as recommended in Notes.** Recommend keeping it. Plan text to add under 0.2, Who is
  asking: "The app writes its executable path to `app-executable` in the daemon's run folder, mode 0600, before each
  connection; the daemon reads it at each check."
- **A failed call is never sent again.** The window sees an error that says it may have taken effect. Recommend this:
  `run.stop` and the like are not safe to repeat. The alternative is a retry for read-only methods, with a list to keep.
- **The environment is recaptured once, at the app's first connection**, not at each reconnect. A restarted daemon
  captures its own. Recommend this.
- **The window is not told that the stream ended or came back.** If the daemon's log no longer holds the events since
  the last `seq`, the pump logs it and the window's state is behind. Recommend a later item: a `connection` notice so
  the window reloads with `run.list` and `run.get` after a gap.
- **One failed-start count is shared by the link and the pump.** Quitting the app resets it. Recommend this until the
  resident app (Stage 9b).
- **The capability file is minimal** (`main-window`). Stage 6a widens it. Recommend leaving it.

- Owner decisions (2026-11-15): the six decisions taken under the owner's delegation; the plan text applied to 0.2,
  Who is asking. Review: the app file is a guardrail like the rest of Who is asking (0.1, same user); `classify_peer`
  checks the run marker before the app, so a run's agent cannot become the app by writing the file. The three `rpc`
  tests were not seen red alone. Carried to `ts-0081-wm`: the shell tells the page when the event stream ends and
  comes back, and the page reloads with `run.list` and `run.get` (the builder's later-item recommendation).

### Not done, by design

- The real `LaunchStarter` is not run by any test: it can reach launchd and start a process. Only a fake and an
  in-process starter are used. Owner-run check: the app against a real install.
- No Tauri window was launched, so the webview's `listen` against the capability file is checked only by reading the
  file and by the build accepting it.
- A daemon that is stopping can answer a call for up to 100 ms; the call then succeeds or fails as a drop. Not
  changed.
- Failures of `daemon.recapture_environment` and a log gap are written to standard error only, not shown in the window.
- Packing `collide-daemon` into a bundle, the resident app's restart, and everything of ts-0078 (`run.start`,
  `collide/quick`, `run.diff`, Review, `pack.yaml`).
