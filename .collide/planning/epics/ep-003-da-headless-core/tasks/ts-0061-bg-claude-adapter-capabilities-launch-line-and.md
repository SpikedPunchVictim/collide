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
