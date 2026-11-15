---
id: ts-0017-qb
kind: task
status: active
title: "Shell rpc and event channel"
owners: []
links:
  - { type: blocked-by, to: ts-0032-pp }
  - { type: blocked-by, to: ts-0035-re }
---

## Goal

The shell's one generic `rpc` command and one event channel in `app/src-tauri`, as signatures with tagged stubs: the shell forwards each call to the daemon's socket and adds the token itself, so the token never reaches the webview; `ui_tools.*` methods go to the local handler of `ts-0035-re` and are never forwarded. In `app/`, a typed TypeScript client (`rpc(method, params)` and an event subscription) over the types of `ts-0032-pp`, with an in-memory fake owner behind the same interface, so UI tests run in a browser with no daemon.

## Success criteria

- `cargo build` of `app/src-tauri` and the app build pass.
- A test calls `rpc` against the fake owner and gets a typed result. An event pushed by the fake owner reaches a subscriber.
- The client's API has no place to pass a token, and a test over the types checks that.
- A `ui_tools.*` call is routed to the local handler, and a test shows the forward path is not called.
- Every stub carries a tag that belongs to a stage.

## Tests

Write first: the fake-owner round trip; the event subscription; the `ui_tools.*` routing test; a type test that no token parameter exists.

## Notes

- IMPLEMENTATION_PLAN.md, 0.2 (The UI speaks the owner's protocol; the diagram), 0.15 (Pass 1: the shell's `rpc` command and event channel), Stage 2: Skeleton (the shell's `rpc` command and event channel).
- Area: `app/src-tauri`, `app/` (the client and the fake owner).
- Done when the criteria hold.
- Built:
  - `app/src-tauri/src/shell.rs`: `Shell::rpc(method, params)` (the `rpc` command's logic, with no Tauri type), the traits `DaemonLink` and `EventChannel`, `ChannelWindow` (the endpoint's `Window` over the event channel, so a `tools/call` is a `ui_tools.request` event), `ChannelEvent {method, params}` (no envelope, no token), `ShellHandle` (the shell on its own thread) and the stubs `SocketLink` and `pump_daemon_events`. `ui_tools.*` goes to `UiToolsEndpoint::rpc` and is never forwarded; `initialize` is refused (the shell makes the handshake); `ask.start` and `ask.test` are read in the window's shape (a `token` key is refused) and `ask.cancel` goes on as it is; everything else is forwarded. `app/src-tauri/src/lib.rs`: the `rpc` command, `WebviewChannel` (emits on `collide:event`), `RPC_COMMAND`, `EVENT_CHANNEL`, `build_shell`. Depends on `collide-core` and `collide-host` (the dependency-direction check covers `core` only, so this is legal).
  - `app/src/owner/`: `methods.ts` (`OWNER_METHODS`, `ASK_METHODS`, `UI_TOOLS_METHODS`: one table of names and types, over the generated types), `client.ts` (`OwnerClient` with `rpc(method, ...params)` and `subscribe`, `ChannelEvent`, `OwnerError`), `fake.ts` (`FakeOwner`: `handle`, `push`, `calls`), `tauri.ts` (`createTauriOwner`: `invoke('rpc', {method, params})` and `listen('collide:event')`), `generated.ts` (type-only re-export of `packages/pack-sdk/src/generated`).
  - Tests, written first: `app/src-tauri/tests/rpc.rs` (16), `app/test/owner-client.test.ts` (7), `app/test/owner-types.test.ts` (4 plus the `@ts-expect-error` lines, checked by `pnpm typecheck`), `app/test/owner-methods.test.ts` (3), `app/test/tauri-owner.test.ts` (4). A checks test, `every_shell_tag_names_its_stage_and_counts_at_least_one_stub`, in `checks/tests/real_tree.rs`.
  - Stubs, in `stub-tags.json`: `shell: daemon connection` (3c), `shell: daemon events` (3c), `shell: ask token` (6b).
- Decisions for the owner:
  - (1) The connection to the daemon is a stub in Stage 3c, where the plan says the minimal shell is built; the alternative is Stage 6a, which finishes it.
  - (2) The shell runs on its own thread (`ShellHandle`), because `UiToolsEndpoint` holds boxed traits that are not `Send`; the alternative is to add `Send` to those traits in `host`, which breaks ts-0035-re's `Rc` fakes.
  - (3) The webview gets the generated types by a type-only relative import of `packages/pack-sdk/src/generated`; the alternative is a `workspace:*` dependency on `@collide/pack-sdk`, whose types live in `dist` and need a build before every typecheck. No npm dependency was added.
  - (4) The method table is hand-kept in `methods.ts`, and a test reads `core/src/owner.rs` to keep it equal; the alternative is for `pnpm gen` to write a method table.
  - (5) `initialize` from the window answers `method not found`, with no echo of its params; the alternative is `not_permitted`.
  - (6) The event payload is `{method, params}` with no JSON-RPC envelope, which is the shape of the generated `OwnerNotice` and `AskNotice`.
  - (7) `ui_tools.token` and `ui_tools.rotate` do hand the window the endpoint token, because Settings shows it (0.2). That is not the per-client token, which the window never holds; their params have no token.
  - (8) `run.preview` uses `RunStartParams` because `RunPreviewParams` is a type alias in `core` with no schema.
  - (9) The endpoint is built with an empty tool manifest, the unbound port 0, `KeychainTokens` and `SystemClocks` (both stubbed), until Stage 6.
- Not done by design: the socket, the token file and the handshake (Stage 3c); the daemon's notices onto the channel (Stage 3c); the ask's token, its endpoint and its registration (Stage 6b); the HTTP listener and the calls of `expire` and `window_gone` (Stage 6b); the webview's CSP and Tauri capability set (Stage 6a); a reconnect in the client (Stage 6a); the item's `status`, `links` and `owners`.
