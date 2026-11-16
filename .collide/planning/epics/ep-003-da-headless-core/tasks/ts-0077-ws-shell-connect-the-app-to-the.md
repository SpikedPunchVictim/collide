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
