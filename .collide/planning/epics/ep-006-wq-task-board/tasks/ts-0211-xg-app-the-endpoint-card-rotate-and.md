---
id: ts-0211-xg
kind: task
status: ready
title: "App: the endpoint card, Rotate and the snippets"
owners: []
links:
  - { type: blocked-by, to: ts-0210-mz }
---

## Goal

The Agents page gets the endpoint card: the URL, whether a window is connected, the last call, the masked token with
Reveal, Copy and Rotate, and snippets for external clients, generated from the real endpoint and token.

What exists.
- `ui_tools.token` and `ui_tools.rotate`, answered in the shell (`host/src/ui_tools.rs`); Rotate ends every client
  of the old token (`rotating_ends_every_client_of_the_old_token`). The listener, the port and its failure text
  (`ts-0199-xz`).
- The Agents page (`ts-0210-mz`). The mock: `endpointHtml`, `snipText`; note 57. S6's snippets (`src/ui/
  Settings.tsx`), which had no token.

What this adds.
- The card: the URL; whether a window is connected; the time and tool of the last call; each external client's
  name from its `initialize`, shown as unverified with the time last seen; a port in use, with its text.
- The token masked, with Reveal, Copy and Rotate. Rotate asks first and names the clients seen; after it, every
  snippet shows the new token.
- The snippets, generated: `claude mcp add --transport http collide <url> --header "Authorization: Bearer
  <token>"`, a JSON `mcpServers` block, an opencode `mcp` block, and the URL with the token as its last path segment.
  Beside Rotate: a pasted snippet puts the token in a file Collide does not control. Collide writes to no agent's
  configuration.
- The endpoint keeps the last call and the clients seen, read by the window through one new `ui_tools.*` method.

## Success criteria

- A snippet's URL and token, parsed out of each of the four snippets, are used against the endpoint over loopback,
  and each works: `initialize`, `tools/list`, and a `tools/call` that reaches a fake window.
- After Rotate, each snippet shows the new token, and the old token is refused.
- An `initialize` with a client name shows that name as unverified, with its time.
- Over a fake owner: the card, the mask, Reveal, Copy, the Rotate prompt naming the clients, and the note.
- `cargo test --workspace` and `pnpm test` pass.

## Tests

Write these first.
- `app/src-tauri/tests/`: the four snippets generated, parsed and used over loopback; Rotate; the client name.
- `app/test/`: the card over a fake owner.
- The S6 port: `agent-ui.test.tsx` "shows the endpoint and the copy-paste snippets for an external agent, as text".

## Notes

- IMPLEMENTATION_PLAN.md:
  - 0.2: the endpoint token (rotatable, Bearer or path segment). 0.7: Endpoint card; Snippets for external clients.
  - Stage 6b: The Agents page (the endpoint card with Rotate; the snippets); Success Criteria. Part L.
- **Decision for the item, with a recommendation: where snippets are made.** Recommend the window, from the URL and
  token `ui_tools.token` returns, in one TS module the tests import. Alternative: the shell; the token would then
  cross the wire inside four strings rather than one.
- **Decision for the item, with a recommendation: the status method.** Recommend `ui_tools.status` (window
  connected, last call, clients seen), answered in the shell like `ui_tools.token`. Alternative: events pushed on
  each call; noisier.
- Area: `host/src/ui_tools.rs`, `core/src/ui_tools.rs`, the shell, `app/src/components/settings/`, the generated
  files, `app/test/`, `app/src-tauri/tests/`.
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
