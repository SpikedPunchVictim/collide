---
id: ts-0199-xz
kind: task
status: ready
title: "Shell: the UI-tools endpoint on loopback, its port and token"
owners: []
links:
  - { type: blocked-by, to: ts-0198-tf }
---

## Goal

The shell's UI-tools endpoint listens on 127.0.0.1, on a port picked once and kept in the user's settings, and its
token lives in the keychain. Every later item reaches the endpoint through this listener.

What exists.
- `host/src/ui_tools.rs` `UiToolsEndpoint` (`ts-0035-re`): the guards in order (Host, Origin, content type, 1 MB
  body, token), `initialize`, `ping` and `tools/list` from the manifest, 202 for a notification, 405 for GET and
  DELETE, `tools/call` forwarded as `ui_tools.request`, the 10 s deadline, the per-ask cap of 20, `register_ask`,
  `end_ask`, `window_gone`, Rotate. 49 tests in `host/tests/ui_tools.rs`, all at the handler, with no socket.
- `SecretStoreTokens` in the same file is a stub tagged `ui_tools: endpoint token store`.
- `app/src-tauri/src/lib.rs` `build_shell`: `UNBOUND_PORT` (0) and `SecretStoreTokens`. No listener.
- The test bridge's HTTP reader, std only (`app/src-tauri/src/bin/collide-bridge/http.rs`), and its `MemoryToken`.
- `SecretStore`, `MemorySecretStore` and `secret_store_for_root` (`host/src/secret_store.rs`).
- No settings key for the port (`core/src/settings_registry.rs`).

What this adds.
- A listener in the shell: a std `TcpListener` on 127.0.0.1, one thread per connection, one request per connection.
  It reads with the bridge's reader, moved into a module both use, and passes each request to `handle`. A pending
  call waits for the window's reply or for `expire`, which runs on a tick.
- The port: the first start binds port 0 and writes the port it got to `ui_tools.port`, a user-only key. Later starts
  bind that port. A port in use fails with a message that names it. The bridge hosts the same listener.
- `SecretStoreTokens` over an `Arc<dyn SecretStore>`: 32 random bytes from the OS, hex-encoded, made at first use,
  kept under one item name. `rotate` writes a new one, and the old one is refused at once. A store that fails leaves
  the endpoint refusing every call, with a plain error in the log. The app uses `secret_store_for_root`.
- The per-client call limit for `external` callers (0.7). See the Owner's choice.
- The tag `ui_tools: endpoint token store` is filled: it moves to `FILLED_TAGS` in `checks/tests/real_tree.rs`,
  `UI_TOOLS_TAGS` goes, and the stub's `should_panic` test is replaced.

## Success criteria

- Over a real loopback socket with a fake window: a wrong token, a foreign `Host`, a foreign `Origin`, a content
  type that is not JSON and a body over 1 MB are each refused, and nothing reaches the window.
- Over the socket: `initialize`, `ping` and `tools/list` answer with no window; a notification gets 202; GET and
  DELETE get 405; a closed window, a window closed when the client starts, and a window silent for 10 s each give
  the plain text of 0.2.
- The token is made at first use, is the same at the next start, and after Rotate the old token is refused at once.
- The port is kept in `ui_tools.port` and reused; a port in use fails with a message that names it.
- The external limit refuses the call over the limit with a text that names the limit; an ask's calls are not
  counted against it.
- The endpoint token appears in no log line the shell or the listener writes.
- `cargo test --workspace` and `pnpm test` pass; the stub tag is gone from `checks/tests/real_tree.rs`.

## Tests

Write these first.
- `app/src-tauri/tests/`: the listener over loopback with a fake window: each guard; the manifest answers; 202;
  405; the three window texts; a pending call answered by a reply, and by `expire` at 10 s on a fake clock.
- `host/tests/ui_tools.rs`: `SecretStoreTokens` over `MemorySecretStore`: made once, kept, rotated, old refused; a
  failing store refuses every call.
- The port: written on first start to a temp user config, reused, and a held port named in the failure.
- The external limit, on a fake clock.
- A seeded token is searched for in every log line the tests captured, and is found in none.

## Notes

- IMPLEMENTATION_PLAN.md:
  - 0.2: the UI-tools endpoint (transport, guards, port, endpoint token, ask tokens, closed window).
  - 0.7: Interactive launches get the same MCP rule (the per-client call limit, "its number set in Stage 6").
  - Stage 6b: The endpoint; A closed window; Success Criteria (the endpoint, tested with a fake window). Part L.
- Out of scope: the ask token and `ask.start` (`ts-0201-us`); the real manifest (`ts-0202-ab`); the endpoint card
  (`ts-0211-xg`).
- **Owner's choice, recommendation taken (2026-11-24): the external call limit.** Recommend one limit shared by every
  `external` caller: 20 screen-changing calls in a rolling 60 s, counted in the shell, with the text "external clients
  may change the screen 20 times a minute; try again in N s". There is one token, and the transport is stateless, so
  clients cannot be told apart. Alternative: a limit per `clientInfo.name` from `initialize`; that name is unverified.
- **Decision for the item, with a recommendation: the HTTP server.** Recommend std, with the bridge's reader moved
  into a module the shell and the bridge share: no new dependency, and the host already chose no HTTP crate
  (`host/src/http_get.rs`). The plan's text asks for "a crate already in Tauri's dependency tree", so the split
  proposes a plan change. Alternative: `hyper` 1.12.0 with `hyper-util` 0.1.21, in the tree through `reqwest`; a new
  direct dependency, so an Owner's choice if picked.
- **Decision for the item, with a recommendation: the randomness.** Recommend reading 32 bytes from `/dev/urandom`
  with std. Alternative: `getrandom` 0.3.4, in the tree through Tauri; a new direct dependency.
- **Decision for the item, with a recommendation: where the port is kept.** Recommend `ui_tools.port`, a user-only
  key in `core/src/settings_registry.rs`, written through `config.set`. Alternative: a file of the app's own.
- Area: `app/src-tauri/src/` (`lib.rs`, `shell.rs`, a new listener module, the bridge), `host/src/ui_tools.rs`,
  `core/src/settings_registry.rs`, `checks/tests/real_tree.rs`.
- Dependencies: none new.
- The test rule: no test touches the real home, real launchd, a real daemon, the keychain or the network. Each test
  that runs the `collide` binary clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to
  a temp root, as `host/tests/common/mod.rs` `TempRoot` does. A temp repository sets `GIT_CONFIG_GLOBAL` and
  `GIT_CONFIG_SYSTEM` to empty files. Keychain writes go through `MemorySecretStore` (`host/src/secret_store.rs`),
  never the login keychain. Browser tests run the installed Chrome with a temp profile (`app/browser/support/
  chrome.ts`), and the bridge's daemon on a temp root (`app/browser/support/world.ts`), with fake agents only. A test
  never stubs or bypasses a safety gate: the real-agent opt-in, the confinement probe, `env_clear`.
- Stage 6b item (revision 26 scope, split 2026-11-24). Items of Stage 6b run one at a time, in id order, because they
  share `app/`, `core`, `host` and the generated files. No builder runs `tauri build`.
- Nothing here runs a real `claude`, `opencode` or any model, or spends money.
- Owner decisions (2026-11-24): the owner took the recommendations at the split.
