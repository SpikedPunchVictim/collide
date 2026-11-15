---
id: ts-0061-bg
kind: task
status: active
title: "Claude adapter: capabilities, launch line and phase snapshot"
owners: []
links:
  - { type: blocked-by, to: ts-0060-mm }
---

## Goal

The Claude adapter's declarations and launch (`host/src/adapters/claude/mod.rs`, `launch.rs`):

- The capability table of 0.3. Each capability is `Yes`, `No` or `Unknown`.
- The launch line of 0.3 (S1), built from a `LaunchRequest`.
- The phase's snapshot files: `settings.json` with auto memory off and the canary's no-op hook (`CANARY_MARKER`), `agents.json` with the `collide-<pack>-<role>` definition, and `mcp.json`. The adapter renders them; the daemon writes them.
- The context options (`compact_at`, `bash_output_max`, `mcp_output_max`, `config: user | none`), and the disallowed tools.
- The environment strip, applied before the context variables are set.
- The user's config a phase loads: its hooks and its settings `env`, read for the run preview's flags, and the model endpoint's host for the fingerprint (revision 15).

Replaces the stubs tagged `claude: capabilities` and `claude: launch line`.

## Success criteria

- A role with `compact_at` under 100,000 is refused with a message that names the floor (Stage 3b criterion).
- The fingerprint case below (Stage 3b criterion; revision 15). This item builds the fingerprint's user-config part, and `ts-0067-kw` stores it.
  - The fixture is a `settings.json` (never the user's own) with an `ANTHROPIC_BASE_URL` in its `env`, and a `SessionStart`, a `UserPromptSubmit`, a `PreToolUse` and a `PostToolUse` hook.
  - A phase's fingerprint records the endpoint's host, with no path, query or key, and each hook with its flag.
  - The same variable in the captured environment is recorded the same way.
- `capabilities()` returns Claude's row for each capability in 0.3's table. A capability that S1, S7 or S9 did not measure is `Unknown`.
- The argv is 0.3's line:
  - `--session-id` for `Assign`, and `--resume` for `Resume`;
  - `--model <model.name>`, and `--max-budget-usd` with the budget left;
  - `--agent collide-<pack>-<role>`;
  - `--setting-sources user` or `""`, with `--strict-mcp-config` kept in both cases;
  - the disallowed tools, unless the role asks for one.

  No prompt text appears in argv.
- The spec strips `CLAUDECODE` and every `CLAUDE_*`, and no `ANTHROPIC_*`. The context variables are set after the strip.
- The user's hooks and settings `env` are read from `$HOME/.claude/settings.json` under `config: user` only. Each hook is flagged by its event, and so is a settings `env` that sets a context variable or the endpoint (0.3, The user's own tooling).
- Two calls with the same request give the same spec and the same snapshot files, because the adapter has no state.
- No stub tagged `claude: capabilities` or `claude: launch line` remains. Both tags move from `HOST_ADAPTER_EXECUTOR_TAGS` to `FILLED_TAGS` in `checks/tests/real_tree.rs`.

## Tests

Write these first:

- context-option validation: 99,999 refused, naming the floor, and 100,000 accepted; `bash_output_max` and `mcp_output_max` set after the strip;
- env stripping at the spec level: `CLAUDECODE`, `CLAUDE_PID` and a `CLAUDE_CODE_*` value removed, `ANTHROPIC_BASE_URL` kept;
- the argv for `Assign` and for `Resume`;
- the snapshot files;
- the fixture `settings.json` for the fingerprint, with an endpoint URL that has a path, a query and a key in it.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 3b: the Claude adapter per S1; the adapter's declarations.
  - 0.3: the launch line and its bullets; What the adapter declares besides its launch line; Context and cache (Context options); Capabilities; The user's own tooling.
  - 0.13: the fingerprint. Revision 15: the endpoint host.
  - S1 and S9 records, for the measured flags.
- **Names check.** A line that names `CLAUDECODE`, `CLAUDE_` or `ANTHROPIC_` may appear only in `host/src/adapters/claude/` or `host/tests/adapters/claude/` (`checks/src/names.rs`, `ADAPTER_PATHS`). So this item's integration tests and fixtures go in `host/tests/adapters/claude/`, with a `[[test]]` target in `host/Cargo.toml`.
- **Decision for the item, with a recommendation: the first turn.** 0.3 says Claude's "launch line carries the first turn" and `start` writes nothing. S1 measured the turn as a stdin `user` message under `--input-format stream-json`, and argv shows in `ps`. Recommend that the session writes the first turn on stdin (`ts-0062-rj`) and that argv carries no task text. The owner corrects the 0.3 sentence afterwards.
- The canary hook is written here. `ts-0068-wx` checks for its `hook_started` event.
- Area: `host` (`adapters/claude/mod.rs`, `adapters/claude/launch.rs`).
- The test rule: no test touches the real home, real launchd or a real daemon. Each test clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as `host/tests/common/mod.rs` `TempRoot` does. Every `settings.json` the tests read is a fixture, never the owner's own.
- Stage 3b item (revision 18 scope, split 2026-11-15). Items of Stage 3b run one at a time, in id order, because they share `host` and the generated files.

### Built

- `host/src/adapters/claude/mod.rs`: `capabilities()` (also `ClaudeAdapter::capabilities`) is Claude's row of 0.3's table. `Yes`: `headless_stream`, `permission_host` with all three sub-flags, `budget_native`, `tool_allowlist`, `named_agent`, `usage_report` (scopes `Turn`, `Step`, `RunningTotal`, `Compaction`; cost, cache write tiers and context; quality `Exact`), `limit_signal`, `compaction_events`, `hook_events`, `refusal_source`, `strict_config`, `context_options` (all three), `take_over`, `mcp_servers` `stdio` and `http`. Modes: `session_resume` `Silent`, `model_select` `Launch`. `Unknown`: `background`, `system_prompt` and `mcp_servers.sse`, because none of S1, S7, S9 or S6 measured them. The stub test for `claude: capabilities` is gone.
- `host/src/adapters/claude/launch.rs`: `launch_spec` is filled. Argv is `FIXED_ARGV`, then `--session-id` (`Assign`) or `--resume` (`Resume`), `--model <model.name>`, `--max-budget-usd <left>`, `--agents`, `--agent collide-<pack>-<role>`, `--setting-sources user|""`, `--settings`, `--mcp-config`, and `--disallowed-tools` with the eight names as separate words, as S9's runs passed them. No prompt text. `env` holds only the context variables (after the strip, which `BaseEnvironment::for_child` applies first), and `env_strip` is the adapter's. Refused as `LaunchError::Invalid`: `compact_at` under 100,000 (the message names `100,000` and the value), a `bash_output_max` or `mcp_output_max` of 0, a spent or non-finite budget, `AgentAssigned`, and a model name, session id, pack or role that fails the safe token. New public: `agent_name`, `disallowed_tools_without`.
- `host/src/adapters/claude/snapshot.rs` (new): `snapshot_files(request, definition, mcp_servers)` renders `settings.json` (`autoMemoryEnabled: false` and a `SessionStart` hook whose command is `true collide-settings-canary`), `agents.json` (the definition's `description`, `prompt` and `tools`, under `collide-<pack>-<role>`, with its own `model` left out) and `mcp.json`. It opens no file; the daemon reads the definition and writes the text.
- `host/src/adapters/claude/user_config.rs` (new): `read_user_config(home, config)` reads `$HOME/.claude/settings.json` under `config: user` (or no `config` key) and returns each hook as a name and a flag (`PreToolUse`, `UserPromptSubmit`, `SessionStart`, `PostToolUse`, else none), the settings `env` entries that set a context variable or the endpoint (by name), and the endpoint. `endpoint_host` keeps the host only. `endpoint_in_environment` records the captured variable the same way. Hook commands and env values are never kept.
- `host/Cargo.toml`: a `[[test]]` target `claude_adapter`. `checks/tests/real_tree.rs`: `claude: capabilities` and `claude: launch line` moved to `FILLED_TAGS` (now 29; `HOST_ADAPTER_EXECUTOR_TAGS` now 22). No dependency added.
- Tests, all in `host/tests/adapters/claude/` (55, from `main.rs`): `capabilities.rs` 6, `launch.rs` 21, `snapshot.rs` 12, `user_config.rs` 16, with the fixture `fixtures/user-settings.json` (a gateway URL with a path, a query and a fake key, and the four hooks). One unit test in `launch.rs` for the thousands separators replaces the capability stub test. Every settings read uses a `TempRoot` as `HOME`; nothing spawns a process or reads the real home.

### Decisions for the owner

1. Which capabilities are `Unknown`. Chosen: `background`, `system_prompt`, `mcp_servers.sse`. `background` is `Unknown` although 0.3's table says "Claude has it": S7 recorded it as "not used", and the first run under a shim (`ts-0070-mk`) is the measurement. Alternative: `Yes` per the table, which would let a role require it before it is measured. Recommend `Unknown` until `ts-0070-mk`. Two are `Yes` on thinner evidence than the rest: `limit_signal` (a live `rate_limit_event` at `allowed`, the `rejected` shape from the binary and two transcripts) and `mcp_servers.http` (S6's prototype ran Claude over an HTTP endpoint; the ask role requires it). Recommend `Yes` for both, and the contract test records a live limit when one happens.
2. Role's required capabilities. `pack::Role` has no `require` key, so `launch_spec` cannot refuse a capability itself. Chosen: the caller (`ts-0065-kx`) runs `refuse_unmet_capabilities` over `ClaudeAdapter.capabilities()`. Alternative: add `require` to `Role` now. Recommend a later item, with the pack check that rejects a name that is not a capability.
3. Disallowed tools "unless the role asks for one". The role has no key for it either. Chosen: `disallowed_tools_without(asked)` is built and tested, and `launch_spec` passes none, so all eight are always disallowed. Alternative: a `Role.allow_tools` list. Recommend adding it with decision 2.
4. The endpoint host. Chosen: the host only, lowercased, with the port dropped, and `None` (variable set, nothing kept) when the value is not `scheme://plain-host...`. Alternative: keep `host:port`, which tells two local gateways apart. Recommend host only: a port can carry nothing secret, but the fingerprint compares endpoints and the host is the 0.13 wording.
5. A malformed `settings.json`. Chosen: `read_user_config` fails with the path and the place, and a file of the wrong shape names what is wrong. `claude` itself ignores such a file silently under `-p`. Alternative: return an empty config. Recommend an error, so the preview says so instead of promising a phase has no hooks; the caller may show it as a warning.
6. The definition's `model:`. Chosen: left out of `agents.json`, since `--model` always overrides it (S1). Alternative: pass it through. Recommend leaving it out, so one model reaches the phase by one route.
7. Canary hook. Chosen: a `SessionStart` hook running `true collide-settings-canary`, which prints nothing (a `SessionStart` hook's output becomes context). The marker is in the command, and the plan does not say whether `hook_started` carries the command: `ts-0068-wx` must read the real event and may need to match on the event name and ordering instead.
8. Context variables and role `env`. The spec's `env` holds only the three context variables. Role, phase and experiment `env:` templates (0.5, Env) are layered by the daemon; the order between them and these three is `ts-0065-kx`'s. Recommend the context variables win, as 0.3 says they are set after the strip.

- Owner decisions (2026-11-15): the recommendations above are accepted as built. Follow-ups: the pack schema has no role key for the capabilities 0.3 says a role may `require`, nor for asking for a disallowed tool; both keys land with pack loading in Stage 4a, and until then `ts-0065-kx` checks the built-in role against an empty requirement. `background` stays `Unknown` until `ts-0070-mk` measures it. The context variables win over a role's `env` (`ts-0065-kx`). `ts-0068-wx` confirms the real `hook_started` event carries the canary marker.

### Not done, by design

- `--plugin-dir` and `--permission-mode`: no role or request field carries a plugin or a mode yet.
- The first turn on stdin, the stream parser and the canary check (`ts-0062-rj` and later): `launch_spec` writes nothing but argv and env.
- The fingerprint's storage and the run preview's display (`ts-0067-kw`, `ts-0069-bn`): this item returns the values. The user's own `autoCompactWindow` and `bashOutputMaxChars` settings keys, which the fingerprint's context options should include (0.3, Context options), are not read; only a settings `env` that sets the variables is flagged.
- A role's opt-in to auto memory: no key.
- Plugins the user config loads are not listed (0.3 lists user hooks and plugins); only hooks and `env` are read.
- Validating `settings.json` against Claude's published schema (the contract test, `ts-0070-mk`).
