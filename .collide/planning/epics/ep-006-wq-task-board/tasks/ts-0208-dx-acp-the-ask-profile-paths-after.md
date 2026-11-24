---
id: ts-0208-dx
kind: task
status: ready
title: "ACP: the ask profile, paths after realpath, the listener"
owners: []
links:
  - { type: blocked-by, to: ts-0207-en }
---

## Goal

An ACP agent can be the ask agent: the ACP engine of Stage 3 with an ask profile in `host`, and no new trait. Collide
allows a read or a search only inside the readable folders, after `realpath`, refuses everything else, and checks
that the agent opened no network listener it should not.

What exists.
- The ACP engine (`host/src/adapters/acp/`): `AcpSession` (no `fs`, `terminal` or `elicitation` capability; picks
  `allow_once`, never an "always" option; `Cancel`), `AcpOptions` with `McpServer::Http` (empty by default,
  `ts-0087-mf`), `AcpProfile` with no real profile (`ts-0088-pk`, `ts-0090-jd`), the model through `configOptions`
  or `session/set_model`, and `AcpAdapter::feed` with `start_on_launch` (`ts-0092-tw`).
- `host/src/acp_probe.rs` `probe_through_shim` (the `initialize` probe, no caller yet).
- `core/src/agent.rs` `PermissionRequest`: kind and input, but no paths (`ts-0089-wv`).
- The fake ACP child (`host/tests/adapters/acp/fixtures/fake_acp.py`) and its recordings.
- ACP entries in settings (`command`, `args`, `env`), and a budget on one as a load error (`ts-0105-jv`).
- The daemon's ask path for Claude (`ts-0201-us`). The results of `ts-0198-tf`.

What this adds.
- The ask profile for an ACP entry: the child's environment is `PATH`, `HOME`, the entry's `env` lines and its
  secrets; the working directory is the first readable folder; `session/new` carries the endpoint as an HTTP MCP
  server with the ask token in an `Authorization` header; the model from the entry; Collide's timeout only.
- `initialize` is the probe for `mcp_servers: http`: no HTTP support gives "this agent cannot use HTTP MCP servers"
  before `session/new`.
- The permission rule: `read` and `search` get `allow_once` when every reported path, after `realpath`, is inside a
  readable folder, also after `realpath`. A request with no path is refused, and so is every other kind. With no
  `allow_once` option the answer is `Cancel`.
- `PermissionRequest` gains the paths from ACP's `toolCall.locations`.
- The profile refuses `--mdns`, a `--hostname` that is not loopback, and `--cors` in `args`, with a message that
  names the flag.
- The listener check: after start, `lsof` by the child's pid for listening TCP ports, behind a seam. A port found is
  kept in the ask's record for the Agents page.
- A permission configuration for opencode, supplied by the profile, only if `ts-0198-tf` found that one confines it.
- The daemon picks the adapter by the entry's kind for an ask. The fake ACP child gains the endpoint call step of
  `ts-0201-us`'s fake Claude, with the URL and header from `session/new`.

## Success criteria

- Over the fake ACP child: a read inside the readable folders is allowed; a read outside them, a read with no path,
  an edit and a command are each refused; a symlink inside a readable folder that points out of it is refused after
  `realpath`; `Cancel` when no `allow_once` exists.
- An agent with no HTTP MCP support is refused with the plain text. A timeout stops the child and says so. Cancel
  ends the ask.
- `session/new` holds the endpoint and the ask token; the token is in no argv, log or record.
- `--mdns`, `--hostname 0.0.0.0` and `--cors` are each refused, naming the flag.
- The listener check reports a port the child opened, and none when it opened none.
- The child's environment holds `PATH`, `HOME`, the `env` lines and the secrets, and nothing else.
- An ACP `read` or `search` tool call clears the "no files were read" flag end to end.
- `cargo test --workspace` passes.

## Tests

Write these first.
- `host/tests/adapters/acp/`: each permission case over the fake ACP child, with a temp folder holding a symlink
  that points outside; the HTTP refusal; the three flag refusals; `session/new`'s server entry; the environment.
- `host/tests/`: the daemon's ACP ask over the fake child: timeout, Cancel, the endpoint call reaching a fake
  window, the flag cleared.
- The listener seam with a fake `lsof`, and one real `lsof` against a listener the test opens on 127.0.0.1.
- The S6 port: `agents-server.test.ts` "an ACP agent starts with PATH, HOME and what the settings add".

## Notes

- IMPLEMENTATION_PLAN.md:
  - 0.3: ACP profile; An ACP agent's listener; Budget and timeout; The ask role requires `mcp_servers: http`.
  - 0.7: agent entries. Stage 6b: The ask box ("the ACP engine's ask profile for opencode"); Success Criteria (the
    launch specs; the probe and the listener). Part L.
- Out of scope: ACP agents in file runs. `ts-0092-tw` and `ts-0140-gk` left that choice to this stage; the plan
  defers a real ACP agent for runs (Deferred), and this item does not wire it.
- **Decision for the item, with a recommendation: where paths come from.** Recommend `toolCall.locations[].path`
  only, as a new `paths` field on `PermissionRequest`; a request with no locations has no path and is refused.
  Alternative: also read paths out of `rawInput`; its shape is the agent's own. `ts-0089-wv` said the ask box did
  not need paths; the realpath rule does.
- **Decision for the item, with a recommendation: the listener check.** Recommend `/usr/sbin/lsof -a -p <pid>
  -iTCP -sTCP:LISTEN -nP`, run once a few seconds after start and again at the end. Alternative: on each event; it
  runs a process per event.
- Area: `host/src/adapters/acp/`, `core/src/agent.rs`, `host/src/daemon.rs` and the ask runner, the fake ACP child,
  `host/tests/`.
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
