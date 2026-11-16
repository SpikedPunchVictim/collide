---
id: ts-0088-pk
kind: task
status: active
title: "ACP adapter: profile, capabilities and the initialize probe"
owners: []
links:
  - { type: blocked-by, to: ts-0087-mf }
---

## Goal

`AcpAdapter`, the `AgentAdapter` for any ACP agent, built from a profile (0.3: an ACP adapter is one engine plus a
profile per agent). It lives in `host/src/adapters/acp/` beside the session of `ts-0087-mf`. It names no real agent: the
profiles in this stage are test profiles.

**The profile** is plain data, one value per agent:

- the program and the arguments that make it speak ACP (opencode's is `acp`; S7 measured it);
- the environment names and prefixes to strip (`EnvStrip`);
- the config paths the agent reads (`ConfigPath`), and the interactive command that resumes a session, if it has one;
- the capabilities the agent has beyond what `initialize` says: what the probe cannot learn (usage quality, whether
  it enforces a budget, whether it reports a limit).

**The probe** is a function from an `initialize` response to the capabilities it implies. S7 and 0.3 say which:

- `agentCapabilities.loadSession` gives `session_resume`: `Replay` when the agent has it, `None` when it does not;
- `agentCapabilities.mcpCapabilities` (`http`, `sse`) gives `mcp_servers`; `stdio` is always `Yes`, as ACP
  requires every agent to take it;
- `promptCapabilities` is read and not used yet;
- the session's flags the engine always has: `headless_stream`, `permission_host` with `deny_message: No` and
  `input_at_request: No` (ACP has neither: S7, G5, G6), `usage_report` as the profile says, `model_select: Session`,
  `background: No`, `budget_native: No`, `named_agent: No`, `tool_allowlist: No`, `take_over` as the profile says;
- a field the response lacks is `Unknown`, never `No`, so a role that requires it is refused as 0.3 says.

**The adapter**:

- `capabilities` is the profile's, until a probe has run; the probe's answer is merged in by a function the caller
  holds (the daemon does not call it in this stage: the probe is `ts-0092-tw` and Stage 6b);
- `launch` gives the profile's argv, `env_strip` and the request's working folder, after `refuse_unmet_capabilities`
  (the shared helper Claude's `launch` uses). It writes no config file;
- `session` builds the session of `ts-0087-mf` from the request (working folder, first prompt, model, servers);
- `config_paths` and `interactive_command` are the profile's.

## Success criteria

- A response with `loadSession: true` gives `session_resume: Replay`; without it, `None`; absent altogether,
  `Unknown`.
- A response with `mcpCapabilities: { http: true, sse: false }` gives `http: Yes`, `sse: No`, `stdio: Yes`. An absent
  `mcpCapabilities` gives `http: Unknown`.
- A probe never returns `No` for a field the response left out, and never reads a field it does not use.
- A role that requires a capability the adapter says `No` or `Unknown` for is refused by `launch`, with the name, as
  with Claude (`LaunchError::Capability`).
- `launch` gives the profile's argv, the request's working folder, and the profile's strip list; nothing from the
  request's prompt is in argv or env.
- `session` gives a session that, fed the fake child's handshake, writes `initialize` and then `session/new` for the
  request's working folder.
- The profile for a test is built in the test and holds no real program name. Nothing in the tracked tree names an
  agent's command, an env var of a real agent, or a path under a home folder; the names check passes.
- No new crate is added.

## Tests

Write these first:

- the probe, table-driven: `loadSession` true, false and absent; `mcpCapabilities` with each of the three flags, and
  absent; an empty response; a response with fields the probe does not read;
- the capabilities of a profile, with the engine's fixed flags (compare the whole value, as Claude's test does);
- `launch`: argv, cwd and strip list from the profile; a required capability that is `Unknown` is refused with its name;
  the prompt is in neither argv nor env;
- `session` against the fake child under a plain child: the first two lines the session writes are `initialize` and
  `session/new` in the working folder of the request;
- `config_paths` and `interactive_command` come from the profile, and are `None` or empty for a profile that has none.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 3d: ACP engine.
  - 0.3: Later adapters (an ACP adapter is one engine plus a profile; the ACP profile); What the adapter declares
    (capabilities and their `Unknown`); Capabilities (a sub-flag, `Unknown` is not `No`); Two core traits.
  - Stage 6b: The in-app agent (the ACP ask profile; `initialize` as the probe).
  - Deferred: Other agent tools (opencode and the rest are added with a profile and no core change).
  - S7 record: P1, P3, P5 and G5, G6, G11.
- **Decision for the owner, with a recommendation: no real profile yet.** Stage 3d's goal is the engine on a fake child.
  A profile for opencode or another agent needs its program name, its env names and its config files, and S7 measured
  only one agent in one version. Recommend that this stage ships the profile type and a test profile only, and that
  Stage 6b adds the first real profile with a real run. Alternative: ship opencode's profile now from S7's record.
  Pro: a real profile exists to review. Con: it is unmeasured on any other machine or version, and the names check
  would need to know its env names.
- **Decision for the item, with a recommendation: the probe's place.** Recommend a plain function in the adapter module
  (`probe(&Value) -> Probed`), not a trait method. 0.3 has no probe on `AgentAdapter` and a trait change reaches the
  shared contract and Claude. The daemon calling it is `ts-0092-tw`, so a later change to a trait method stays small.
- **Decision for the item: what the profile cannot learn.** Recommend that `usage_report` (`quality`, `cost`,
  `granularity`) comes from the profile, as S7 measured opencode's (`Partial`, no cost, last step only). A profile that
  says nothing gives `Unknown` and the host reports usage as not reported.
- `mcp_servers.sse` is `No` until a use exists; `ts-0087-mf` has no `sse` form.
- The core double's `AcpDialect` holds no profile. Leave it.
- Run `pnpm gen` if a wire type changes. None is expected.
- Area: `host` (`adapters/acp/` new: `profile.rs`, `probe.rs`, `mod.rs`; `adapters/mod.rs`).
- The test rule: no test touches the real home, real launchd or a real daemon. Each test clears the environment
  (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as `host/tests/common/mod.rs` `TempRoot`
  does. Settings files are fixtures, never the owner's own `settings.json`.
- Stage 3d item (revision 23 scope, split 2026-11-15). Items of Stage 3d run one at a time, in id order, because
  they share `host` and the generated files.
- Owner decisions (2026-11-15, at the split): the decisions for the owner above, taken as recommended under the
  owner's delegation; the owner may change them before the item is built. The ACP protocol is written by hand: no ACP
  crate is added.
