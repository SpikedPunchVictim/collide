---
id: ts-0200-gs
kind: task
status: ready
title: "Claude adapter: the ask launch spec, with no process"
owners: []
links:
  - { type: blocked-by, to: ts-0199-xz }
---

## Goal

The Claude adapter builds the ask's launch spec from an ask profile, on Stage 3's traits and with no new trait. The
argv is 0.3's pinned line exactly. No process starts in this item.

What exists.
- `host/src/adapters/claude/launch.rs` `launch_spec`: the run line, built from a role and a phase. `env_strip`
  (the `CLAUDE*` variables) and `capabilities` (`mcp_servers.http: Yes`) in `host/src/adapters/claude/mod.rs`.
- `collide_core::adapter`: `LaunchSpec`, `LaunchError`, `refuse_unmet_capabilities`.
- `core/src/settings.rs` `AgentEntry` (`model`, `config`, `read`, `tools`, `timeout_s`, `budget_usd`, `secrets`)
  and `core/src/agent_layers.rs` (`ts-0099-bw`).
- `host/src/atomic_write.rs`. S6's launcher (`server/agents.ts` in the S6 spike), with its prompt v2.
- The results of `ts-0198-tf`.

What this adds.
- An ask profile (model, readable folders, `config`, optional budget, the ask's folder) and
  `ask_launch_spec(&AskProfile) -> Result<LaunchSpec, LaunchError>` in a new `host/src/adapters/claude/ask.rs`. The
  fixed words are constants, in the order of 0.3. `--restricted` is in the line only if `ts-0198-tf` says "adopt".
- The ask folder's files: `settings.json` (`{"autoMemoryEnabled":false}`) and `mcp.json` (the `collide` server: the
  endpoint URL and an `Authorization: Bearer` header with the ask token), mode 0600, written with `atomic_write`,
  and a function that removes the folder.
- Collide's fixed ask prompt, from S6's prompt v2: read the folder (Glob, then Read or Grep) before grouping or
  ranking by code; say which tasks were left out; never guess an id.
- The checks before launch: `mcp_servers: http` through `refuse_unmet_capabilities` ("this agent cannot use HTTP
  MCP servers"), and the command looked up on the base environment's `PATH` ("claude was not found on PATH").
- S6's three Claude launch tests and its environment test, ported.

## Success criteria

- The argv equals 0.3's pinned line exactly, word for word, for the default entry.
- The argv holds no token and no request text. The request goes on stdin.
- `--add-dir` appears once per readable folder after the first; the working directory is the first.
- `--permission-prompts none` is in the line; `--disable-slash-commands` and `--bare` are not.
- `config: user` changes only `--setting-sources` (`""` becomes `user`).
- `--max-budget-usd` appears only when the entry sets a budget.
- `mcp.json` is mode 0600, holds the endpoint and the ask token, and is gone after the folder is removed.
- The environment drops every `CLAUDE*` variable and keeps the rest of the base environment.
- An agent without `mcp_servers: http` and a missing command are each refused with their plain text.
- `cargo test --workspace` passes.

## Tests

Write these first.
- `host/tests/adapters/claude/ask.rs`: the pinned line as a literal list compared word for word; no token and no
  request text in argv; `--add-dir` for one, two and three folders; `config: user`; with and without a budget;
  `mcp.json`'s mode and content, and its removal; the environment strip; the two refusals.
- S6's `agents-server.test.ts` "Claude Code launch" (three tests) and "cleanEnv drops CLAUDE*", in Rust. S6 also
  dropped `OTEL_*`; 0.3 strips `CLAUDE*` only, so the port follows 0.3.

## Notes

- IMPLEMENTATION_PLAN.md:
  - 0.3: The ask agent is a separate role; Claude Code's launch line (the pinned line and its bullets).
  - 0.2: ask tokens (in an `mcp.json` of mode 0600, never in argv or the environment).
  - Stage 6b: The ask box ("the Claude adapter's `ask` launch spec"); Success Criteria (the launch specs). Part L.
- Out of scope: starting the process, the record and the timeout (`ts-0201-us`); the ACP profile (`ts-0208-dx`).
- **Decision for the item, with a recommendation: the shape.** Recommend a free function beside `launch_spec`,
  taking an ask profile, with `ClaudeAdapter` unchanged. Alternative: a second `LaunchRequest` kind in core; it
  changes the shared contract, which 0.3 says this stage does not.
- If `ts-0198-tf` says "adopt" for `--restricted`, the plan's pinned line changes with it; the split proposes the
  text.
- Area: `host/src/adapters/claude/` (a new `ask.rs`, `mod.rs`), `host/tests/adapters/claude/`.
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
