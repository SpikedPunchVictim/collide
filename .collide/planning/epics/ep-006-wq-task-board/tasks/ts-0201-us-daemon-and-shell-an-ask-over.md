---
id: ts-0201-us
kind: task
status: ready
title: "Daemon and shell: an ask over fake Claude, end to end"
owners: []
links:
  - { type: blocked-by, to: ts-0200-gs }
---

## Goal

`ask.start` and `ask.cancel` work end to end over fake Claude: the shell adds the ask token, the daemon starts the
child under a shim, the child's calls reach the window through the endpoint, and the ask's events and outcome come
back to the window. A seeded secret reaches the child and nothing else.

What exists.
- `core/src/ask.rs` (`ts-0034-hh`): the params in both shapes, `AskOutcome`, `AskEnd`, `AskCost`, `AskNotice`
  with `scrub_token`. The daemon answers `ask.*` with -32601.
- `app/src-tauri/src/shell.rs`: `for_the_daemon` reads the window's shape; `with_ask_token` is a stub tagged
  `shell: ask token`. `SocketLink` holds one connection for calls; `pump_daemon_events` has its own.
- The endpoint's `register_ask`, `end_ask` and `window_gone`, and the listener of `ts-0199-xz`.
- Phases under the shim (`host/src/phases.rs`), stop and the sweep; `ClaudeSession`; `base_env.rs`; fake Claude
  (`host/tests/adapters/claude/fake.rs` and its script).
- `SecretStore` and `config set --secret` (`ts-0099-bw`, which left launching with secrets to this stage).
- `ask_launch_spec` (`ts-0200-gs`).

What this adds.
- The daemon's `ask.start` and `ask.cancel`. It resolves the entry, refuses an unknown id and an External entry
  with plain text, writes the ask folder under the daemon's data folder, and starts the child under a shim with the
  ask id in `COLLIDE_RUN`. Each stream event goes out as `ask.event`, scrubbed, to the connection that started the
  ask. Then `ask.outcome`: finished, cancelled, `timed out after N s` (Collide's timeout stops the child as a phase
  is stopped), `stopped at the $X budget`, or failed with its text. A dead shim ends the ask lost.
- The ask's record in its folder: argv without secrets, timings, each tool call, usage with its quality, and the
  outcome. It holds neither token. `mcp.json` is removed when the ask ends.
- The connection rule: when the connection that carried `ask.start` drops, the daemon cancels the ask.
- Secrets: an entry's `secrets` resolve from the store at launch, into the child's environment only.
- The shell's ask token (fills `shell: ask token`): 32 random bytes per ask, `register_ask`, and the endpoint and
  token added to `ask.start` and `ask.test`. `ask.event` and `ask.outcome` go to the window; the outcome calls
  `end_ask`. Closing or reloading the window calls `window_gone` and sends `ask.cancel` for each live ask, which
  ends as `cancelled: the window was closed or reloaded`.
- Fake Claude gains a step that calls the endpoint: it reads the URL and header from its `--mcp-config` file, posts
  a `tools/call` on loopback, and writes the call and its result into its stream.
- The tag moves to `FILLED_TAGS`; `SHELL_TAGS` and the stub's `should_panic` test in `app/src-tauri/tests/rpc.rs`
  go. A TS test of the ask types in `app/src/owner/generated.ts`, if `app/test/owner-types.test.ts` lacks one
  (carried from `ts-0034-hh`).

## Success criteria

- Over fake Claude, with a real daemon on a temp root: an ask finishes, and its events and outcome reach the window.
- Cancel, a timeout, a budget stop, a missing command and an agent error each end the ask with their text, and the
  child is gone.
- A dropped connection cancels its ask. A dead shim ends the ask lost.
- An unknown entry and an External entry are refused before launch.
- A token the agent echoes shows as `[removed]` in the events and the record.
- The record holds argv, timings, tool calls, usage and outcome, and neither token; the ask folder holds no
  `mcp.json` after the ask.
- The fake's call reaches the window as `ui_tools.request`, and the reply comes back into its stream.
- An ask token stops working when its ask ends. Closing the window cancels every live ask.
- The endpoint token appears in no log, no argv and no `ask` record.
- **The secrets launch test:** a seeded secret in `MemorySecretStore` reaches the child's environment, and is found
  in no file under the temp root, no argv and no log.
- `cargo test --workspace` and `pnpm test` pass; the stub tag is gone.

## Tests

Write these first.
- `host/tests/`: the daemon over fake Claude: finish, cancel, timeout on a short timeout, budget stop from the
  fake's `error_max_budget_usd` result, missing command, agent error, connection drop, a killed shim, the scrub, the
  record and the removed `mcp.json`, the two refusals.
- `app/src-tauri/tests/rpc.rs`: the token added and registered and never sent to the window; notices emitted;
  `end_ask` on the outcome; a window close sends `ask.cancel` for each live ask.
- End to end, in `app/src-tauri/tests/`: shell, listener, daemon and fake Claude; the call reaches a fake window.
- The secrets launch test: the fake reports that it got the value without printing it; the value is searched for
  in every file under the temp root, in each argv and in each log.
- Ported from S6's `mcp.test.ts`: "refuses an external agent and an empty request" (an empty request cannot pass
  `NonEmpty`), and the relaying half of "launches the configured agent, relays its tool calls and the end".

## Notes

- IMPLEMENTATION_PLAN.md:
  - 0.2: ask tokens; the connection rule; a closed window.
  - 0.3: The ask agent is a separate role ("It is not a run"; the record; `lost`); Budget and timeout.
  - 0.7: secrets. Stage 6b: The ask box; The launch tests that moved here from 4a; A closed window. Part L.
- **Decision for the item, with a recommendation: how the shell reads an ask's notices.** Recommend one daemon
  connection per live ask, opened by `ask.start` and read by a thread of its own that emits the notices. Closing it
  is the cancel the daemon's rule expects. Alternative: one shared call connection with a reader thread that splits
  answers from notices; one dropped socket would then cancel every ask.
- **Decision for the item, with a recommendation: lost.** Recommend a new `AskEnd::Lost {}`, with the text "the ask
  was lost; ask again". Alternative: `Failed` with that text; the record could not then tell a loss from an error.
- **Decision for the item, with a recommendation: the ask's stream.** Recommend `ClaudeSession`'s event parsing for
  the ask, with the request written once on stdin as text and stdin then closed. Alternative: a parser of its own.
- **Decision for the item, with a recommendation: where records live.** Recommend `asks/<ask id>/` under the
  daemon's data folder, kept like run records. Alternative: one JSON Lines file of all asks.
- Area: `host/src/daemon.rs` and a new `host/src/ask_runner.rs`, `core/src/ask.rs`, `app/src-tauri/src/shell.rs`
  and `lib.rs`, `host/tests/adapters/claude/fake.rs` and its script, `checks/tests/real_tree.rs`.
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
