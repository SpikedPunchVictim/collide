---
id: ts-0201-us
kind: task
status: done
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

### Built

- Daemon (`host/src/daemon.rs`, new `host/src/ask_runner.rs`): `ask.start` and `ask.cancel`.
  - `ask.start` is refused before launch, as an owner error, for params that do not read, an id that cannot name a
    folder or is in use, an unknown agent id (-32010, naming the entries), a config with a problem, and an External
    entry (-32011, plain text). It answers `Ack`, and then the ask runs on a thread of its own, so the client reads
    the answer before the first notice.
  - The ask is one phase under the shim, with the ask id as the run id and in `COLLIDE_RUN`. Each stream event goes
    to the starting connection as `ask.event`, scrubbed of the ask's token. The end is `ask.outcome`: finished,
    cancelled, `timed_out`, `stopped_at_budget`, `failed` with text, or `lost`.
  - The connection rule: when the connection that carried `ask.start` closes, its ask is cancelled. `ask.cancel` may
    come on any connection and answers `Ack` for an ask that is not running. The daemon waits up to 30 s for its asks
    when it stops.
  - `asks/<ask id>/` under the data folder holds `settings.json`, `mcp.json` while the ask runs, and `record.json`:
    argv without secrets, program, working folder, timings, each tool call, usage with its quality, and the outcome.
    The record is scrubbed of the token. `mcp.json` is removed when the ask ends.
  - Secrets: an entry's `secrets` resolve from the store at launch into the child's environment. They go in the
    shim's own environment (`env_from_shim` in the shim's spec names them), so `spec.json` holds no value.
- `core/src/ask.rs`: `AskEnd::Lost {}` ("the ask was lost; ask again"), `AskNotice::EVENT_METHOD` and
  `OUTCOME_METHOD`. `pnpm gen` was run; the generated files are untouched by hand.
- Supporting: `ConfigService::agent_entry`, `Phases::start_phase_with_secrets`, `remove_mcp_file`,
  `UiToolsEndpoint::port`, `agent_checks::read_folder` made public, fake Claude steps (`init`, `assistant`, `result`,
  `write_pid`, `write_stdin`, `report_env`, `dump_launch`, `tools_call`, and `{{mcp_token}}`/`{{mcp_url}}`) and eight
  `ask-*` recordings.
- Shell (`app/src-tauri/src/shell.rs`, `lib.rs`, the bridge): `with_ask_token` is filled. Each ask gets 32 random
  bytes (hex), `register_ask`, and the endpoint URL and token in the daemon's shape. `ask.start` goes out on a
  connection of its own (`AskConnector`, `SocketAskConnector`), read by a thread that gives each notice to an
  `AskRelay`, which emits it to the window. The outcome ends the ask's token on the shell's next job or tick. A
  connection that ends with no outcome is told to the window as a lost ask. A window close sends `ask.cancel` per
  live ask (already in `window_gone`). `ask.test` forwards on the call connection and its token ends with the answer.
- `checks/tests/real_tree.rs`: the tag is in `FILLED_TAGS` (49); `SHELL_TAGS` and its test are gone.
  `stub-tags.json` keeps the tag. The `should_panic` test in `app/src-tauri/tests/rpc.rs` is gone.
- Tests, with the ones seen failing at runtime against pre-change behaviour:
  - `host/tests/adapters/claude/ask_run.rs`: 29 daemon tests over fake Claude. 26 of the first 27 failed with the old
    -32601 (the one that passed asserts a refusal). Added later, not run against the old behaviour: the two config
    problem tests and `stopping_the_daemon_stops_a_live_ask_and_its_child` (it also passes without an explicit
    cancel, because stopping the daemon closes the connection).
  - `app/src-tauri/tests/ask.rs`: 7 end-to-end tests (shell, listener, daemon, shim, fake Claude). All 7 failed with
    `ask.start` answering -32601.
  - `app/src-tauri/tests/rpc.rs`: 17 new shell tests. They failed on the `todo!("shell: ask token")` stub (a panic at
    runtime) and pass now.
  - Unit tests: `ask_runner` (6; the registry ones seen red with the registry stubbed, the three text ones were not),
    `shim` (`a_spec_takes_the_names_it_lists_from_the_shims_own_environment` and
    `a_spec_that_names_a_variable_the_shim_lacks_says_which`, both seen red), `remove_mcp_file` (seen red).
  - `app/test/owner-types.test.ts`: three type tests of `AskEnd` (with `lost`), `AskNotice` and `AskOutcome`. They can
    fail only at `pnpm typecheck` (vitest strips types); the `lost` one would not compile without the new variant.
- `host/tests/adapters/claude/fake_claude.rs`: `every_recording_is_valid_json_per_line_and_says_something` now counts 30
  recordings and accepts the new steps that write a line (it failed in the first full run, then passed).
- Ported from S6: the external agent and empty request refusals (`an_external_agent_and_an_empty_request_are_refused_
  and_start_nothing`, `an_ask_start_with_an_empty_request_is_refused_and_registers_nothing`), and the relaying half in
  `the_fakes_call_reaches_the_window_and_the_reply_comes_back_into_its_stream`.

### Decisions for the owner

- **Secrets go through the shim's environment, not `spec.json`.** The shim's spec holds the child's whole
  environment on disk, so a secret there would be in a file. Alternative: write it in the spec (mode 0600). Recommend
  keeping this: the secrets launch test finds the value in no file.
- **An ask id is used once.** An id that is running, has a phase, or has an `asks/<id>` folder is refused (-32011).
  Alternative: replace an old record. Recommend once: the window makes unique ids, and a record is evidence.
- **Where the phase lives.** The ask is a phase keyed by the ask id under the runs folder (so the shim, stop and sweep
  work as they are), and its record is under `asks/`. 0.3 says an ask is not a run; it has no task or workspace, and
  the id space is shared with runs. Alternative: a second phases root for asks. Recommend keeping this.
- **`<board root>` is the board opened last.** `ask.start` has no board id in the window's shape, so the daemon uses
  the most recently opened board; with none, the entry resolves without the repo's line. Alternative: add a board
  id to the window's shape. Recommend that when the ask box is built (ts-0210-mz), because two windows can differ.
- **Any config problem refuses the ask**, as `config.get` refuses the config, even if it is in another entry.
  Alternative: only the asked entry's own problems. Recommend keeping this until the Agents page shows problems.
- **`cancelled` has no text.** The item says a window close ends `cancelled: the window was closed or reloaded`.
  `AskEnd::Cancelled {}` has no field for a reason, and the daemon cannot tell who cancelled. A closed window cannot
  show text anyway; a reloaded one has lost the ask. Recommend the ask box write that text when its own close or
  reload sent the cancel, and keep `Cancelled {}` as it is.
- **A connection that ends with no outcome is a lost ask.** The shell tells the window `lost` in the shape of an
  outcome. Alternative: tell nothing and let the box time out. Recommend this.
- **`ask.test` on the call connection blocks the shell's thread** while the daemon answers, and the daemon still
  answers -32601 (ts-0209-kb). If the test's child calls the endpoint, the shell cannot serve the call. ts-0209-kb
  should run it on a connection of its own, as `ask.start` does, or the test must call no tool.
- **An ACP entry ends the ask `failed`** ("ACP profile is not built") until ts-0208-dx.
- **Defaults and limits I chose:** model `haiku` when the entry has none; the daemon waits 30 s for the child's exit
  record after a stop, and 30 s for its asks at shutdown; the journal is read every 50 ms.
- **The end-to-end tests need the `collide` binary** beside the test's `deps` folder (`cargo build --workspace`, or
  `cargo test --workspace`). `cargo test -p collide-app` alone fails with a message that says so.
- **Plan text to propose (0.2, the connection rule):** "An ask has a daemon connection of its own, opened by
  `ask.start`; its notices go only there, and closing it cancels the ask. The shell reads it on a thread of its own and
  ends the ask's token on the outcome."
- Review (2026-11-27): the recommendations above are taken. Checked in review: a client that stops reading cannot
  hold an ask, because every daemon connection has a 5 s write timeout and a failed write cancels the ask; the
  default model (`haiku`) and timeout (180 s) are the plan's. Carried: the window's `ask.start` gains a board id in
  `ts-0203-*` (the ask box), not `ts-0210-mz`, so `<board root>` is the window's board and not the one opened last;
  `ts-0209-kb` runs `ask.test` on a connection of its own, as `ask.start` does. The proposed 0.2 text goes in with
  the next plan revision.

### Not done, by design

- The daemon's `ask.test` and Test this agent (ts-0209-kb); `ask.test` still answers -32601 at the daemon.
- The ACP profile for an ask (ts-0208-dx) and the Agents page (ts-0210-mz). The ask box itself is not built.
- No real `claude`, `opencode` or model was run, and no `tauri build`. The Playwright specs do not start an ask.
- No change to `IMPLEMENTATION_PLAN.md`; the proposed text is above.
