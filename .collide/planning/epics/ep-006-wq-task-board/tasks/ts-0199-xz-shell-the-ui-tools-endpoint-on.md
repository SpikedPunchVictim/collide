---
id: ts-0199-xz
kind: task
status: done
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

### Built

Listener and shell (`app/src-tauri`):
- `src/http.rs` (new, `pub mod http`): the bridge's reader moved here. `read_request(stream, max_body)` returns a
  `Request` (headers kept as a list, so a repeated `Host` is seen) or a `ReadError` (`Malformed`, `TooLarge`).
  `write_response` moved with it. The bridge's `http.rs` keeps its policy (`Policy`, `check_json`, `file_for`) and
  maps a read error to `BadRequest`.
- `src/endpoint.rs` (new): `EndpointServer::start(ports, shell, log)` binds 127.0.0.1, tells the endpoint its port
  (`ShellHandle::set_endpoint_port`), and accepts on a thread; one thread and one request per connection, 10 s read
  and write timeouts. A request is read with a limit of 1 MB + 1, so the handler's own 413 speaks for a body just over
  the limit; a larger `Content-Length` gets a 413 from the listener, and the rest of that body is drained (8 MB at
  most) so the reset cannot erase the answer. `PortStore` has `DaemonPortStore` (`config.get` and `config.set` of
  `ui_tools.port`, layer user, through the daemon) and `MemoryPortStore` (the bridge and tests). Dropping the server
  stops and joins it. No line it logs holds a request.
- `src/shell.rs`: `Job` is now `Rpc` or `Run(closure)`. The shell's thread wakes every `EXPIRE_TICK` (250 ms) and
  calls `Shell::tick` (`expire`). `Shell::http` answers at once or parks the client by `requestId` until the window's
  reply, `expire` or `window_gone`. `Shell::window_gone` answers the waiting calls and sends `ask.cancel` for each
  live ask. `ShellHandle::http`, `window_gone` and `set_endpoint_port` are the handle's calls.
- `src/lib.rs`: `build_shell` takes the secret store and a log; `run` uses `secret_store_for_root` and starts the
  listener on its own thread (`collide-endpoint-start`), so a slow daemon start never blocks setup. `window_gone` is
  called on the main window's `Destroyed` and on a page load `Started` (a reload).
- The bridge (`bin/collide-bridge/main.rs`): `MemoryToken` is gone. It uses `SecretStoreTokens` over a
  `MemorySecretStore` and hosts the same listener on a port of the system's choosing, with a `MemoryPortStore`. It
  prints a second line, `ui-tools <port>`, after `listening <port> <token>`.

Host (`host/src/ui_tools.rs`):
- `SecretStoreTokens::new(store, log)`: 32 bytes from `/dev/urandom` (`random_token`), hex, made at first use and
  kept under `ENDPOINT_TOKEN_ITEM` (`ui_tools.endpoint_token`). It caches the token. `rotate` writes the new token
  and then swaps the cache, so a failed write keeps the old one.
- `EndpointTokens::current` and `rotate` return `Result<String, TokenFailure>`. A failure is told once to the log as
  a plain line with no token. `handle` answers 503 for every request, `ui_tools.token` and `ui_tools.rotate` answer
  `INTERNAL_ERROR`, and `register_ask` returns `AskRegistration::TokenStoreFailed`.
- `UiToolsEndpoint::external_wait`: 20 screen-changing calls in a rolling 60 s for every `external` caller, on the
  uptime clock. The refused call is not counted. Queries, an ask's calls and a call that never reached a window are
  not counted. `set_port` lets the shell tell the port once it is bound.

Core:
- `core/src/ui_tools.rs`: `MAX_EXTERNAL_CALLS` and `EXTERNAL_CALL_WINDOW_SECS`.
- `core/src/settings.rs`: `UiToolsSettings { port }` in `ConfigFile` (range 1024 to 65535), `MIN_UI_TOOLS_PORT`,
  the key in `collide_keys_set` and `collide_value`, and the minimum in `check_config_file`.
- `core/src/settings_registry.rs`: `ui_tools.port`, integer, no default, `per_user`.
- `pnpm gen` was run: `schemas/ConfigFile.*.json` and `packages/pack-sdk/src/generated/ConfigFile.*.ts` changed.
- `checks/tests/real_tree.rs`: the tag moved to `FILLED_TAGS` (48), `UI_TOOLS_TAGS` and its test are gone.

Tests, all new unless noted:
- `app/src-tauri/tests/endpoint.rs` (real loopback socket, fake window, `TestClocks`, `MemorySecretStore`): each guard
  (wrong token, foreign Host, foreign Origin, content type, a body over 1 MB, a request that is not HTTP, a request
  that stops part way), `initialize`/`ping`/`tools/list`, 202, 405, the token as a path segment, the three window
  texts on a fake clock, a call answered by a reply, two calls answered out of order, the external limit over the
  socket, rotate over the socket, the token across starts, a failing store, the seeded token absent from every log
  line, the port (first start writes it to a temp user config through an in-process daemon, the next start binds it,
  a held port is named, a port that cannot be kept fails), loopback only, port freed on drop, and
  `a_window_that_goes_cancels_each_live_ask_at_the_daemon_once`.
- `app/src-tauri/src/http.rs` unit tests; the bridge's `http.rs` tests adapted.
- `host/tests/ui_tools.rs`: the external-limit group (8 tests) and the keychain-token group (13 tests); the stub's
  `should_panic` test and one 25-call test are replaced.
- `host/tests/config_set.rs`: the port is written to the user file, a value under 1024 or not a port is refused, a
  repo write is refused. Pinned key lists in `config_get.rs`, `config_command.rs`, `config_process.rs`,
  `schema_command.rs`, `settings_layers.rs`, `settings_registry.rs` and `board_config_settings.rs` now hold
  `ui_tools`.

Red at runtime. I wrote the core and host code before I ran the tests, so I made each behaviour old by hand, ran the
tests, then restored the file (compared with the copy I took):
- Registry entry removed: the eight `settings_registry` tests that pin the keys, and
  `the_entries_list_collides_keys_first_in_registry_order_then_each_pack_by_key`, failed.
- The 1024 minimum removed: `a_port_under_1024_fails_naming_the_key_and_the_minimum` failed.
- `external_wait` always allowing and the store never read: 15 `host/tests/ui_tools.rs` tests failed, among them the
  21st-call, wait, rolling, shared and ask tests and the store-made, rotate and failing-store tests.
- `bind` ignoring the kept port and not saving, `tick` not expiring, `window_gone` not answering and not cancelling:
  8 endpoint tests failed (the four port tests, the cancel test, the window-closes-mid-call test, the 10 s test, and
  the first-start test).
- `a_body_over_1_mb_is_refused_over_the_socket...` failed for a real fault, a reset that erased the 413 of a body the
  listener did not read. The drain fixed it.
- Not seen red: the guard tests over the socket (the guards are the handler's, which was already tested; the listener
  was missing, so they could not run), the tests that pass for a constant token (`the_token_is_the_same_each_time...`,
  `the_token_is_not_made_before_it_is_asked_for`), and the not-counted tests of the limit.

### Decisions for the owner

- Fallible token trait. `EndpointTokens::current` and `rotate` return a `Result`, so a failing keychain can refuse
  calls. Alternative: panic or an empty token. Recommend the `Result`.
- A store failure is a 503 for every request, with a plain log line. Alternative: 401. Recommend 503: the client's
  token is not the fault.
- The keychain item is `ui_tools.endpoint_token`. It is the only name the shell uses for it.
- `SecretStoreTokens` caches the token after the first read, so a call costs no keychain access. A change made in
  the keychain by hand needs an app restart. Alternative: read each call, which can prompt the user often.
- `ui_tools.port` is a `u16` from 1024 to 65535, checked in the file and in `config.set`. Alternative: any number.
- The port is read and written through the daemon (`config.get`, `config.set`), so a first start starts the daemon.
  Alternative: the shell reads the file itself. Recommend the daemon: one writer of that file.
- One start attempt, no retry. A failure (port in use, daemon not up) is one stderr line and the app goes on without
  the endpoint. Alternative: retry with a back-off. A later item should show the failure on the endpoint card.
- The reader takes a `max_body` argument. The shell passes 1 MB + 1 so the handler's 413 stays the one voice.
- The shell wakes every 250 ms to run `expire`, so a silent window is answered within 250 ms of its 10 s.
- `window_gone` is called on the main window's destroy and on a page load start. Alternative: only on destroy. A
  reload drops the page's pending replies, so the call would otherwise wait the full 10 s.
- The bridge hosts the listener on a port of the system's choosing and prints `ui-tools <port>`. A client
  reads its token with `ui_tools.token` through `/rpc`. No browser spec uses it yet.
- Proposed plan text for 0.2, the UI-tools endpoint: "The listener is a std `TcpListener`; no HTTP crate is added",
  in place of "a crate already in Tauri's dependency tree".
- Review (2026-11-27): the recommendations above are taken. The plan already says the HTTP server is std (its summary
  and Stage 6b's endpoint text), so the proposed 0.2 sentence is not needed. Noted, not changed: a store that fails
  is read again on the next request, so each failing call logs a line, and a keychain that asks (a debug build signed
  anew after each build) asks on the shell's thread, which the page's calls also wait on. A release build signed
  with one identity is not asked again; `ts-0223-mx` is where a dev build's prompts would show. Two doc comments
  rewrapped.

### Not done, by design

- The ask token and `ask.start` (`ts-0201-us`): `with_ask_token` is still a stub, and `ask.cancel` on a window that
  goes is sent for an ask only once `register_ask` has a caller.
- The real tool manifest (`ts-0202-ab`): the app's endpoint still has an empty manifest.
- The endpoint card and any display of a failed start or a port in use (`ts-0211-xg`).
- A name for `ui_tools.port` on the Settings page: it shows under its key until the mock names it.
- A retry when the endpoint fails to start, and a cap on open connections.
