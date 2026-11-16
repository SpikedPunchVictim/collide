---
id: ts-0088-pk
kind: task
status: done
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

### Built

The ACP adapter, the profile type, the `initialize` probe and the tests. No wire type changed, so `pnpm gen` had
nothing to regenerate. No crate was added.

- `host/src/adapters/acp/profile.rs` (new): `AcpProfile`, plain data: `agent` (the name a refusal uses), `program`,
  `args`, `env_strip` (`EnvStrip`), `config_paths` (`ConfigPath`), `resume_argv`, `usage_report`, `limit_signal`.
  `AcpProfile::new` has only the program. `AcpProfile::capabilities` is the table before any probe: the engine's fixed
  flags, the profile's claims, and `Unknown` for the rest.
- `host/src/adapters/acp/probe.rs` (new): `probe(&Value) -> Probed` and `Probed::merge_into`. `Probed` holds
  `session_resume` and `mcp_http`; `Probed::default()` is everything `Unknown`. It reads `result.agentCapabilities`
  `loadSession` and `mcpCapabilities.http`, and nothing else.
- `host/src/adapters/acp/mod.rs`: `AcpAdapter` (`AgentAdapter`), built by `new(profile)`, with the builders `probed`,
  `with_mcp_servers` and `requiring`. `launch` runs `refuse_unmet_capabilities` and gives the profile's argv, the
  profile's strip list, no env and the request's working folder. `session` gives an `AcpSession` from the request's
  working folder and prompt and the adapter's servers. `config_paths` and `interactive_command` are the profile's.
  The tag table and the module doc name the new modules.
- `host/src/adapters/acp/session.rs`: `start` now sends `clientInfo` (`name` `collide`, `version` the crate's), which
  `ts-0087-mf` decision 6 left for this item. The module doc says so. `host/src/adapters/mod.rs`: doc line.
- Table, as written by `AcpProfile::capabilities`: `headless_stream` Yes; `permission_host` Yes with `deny_message` No,
  `input_at_request` No, `request_reason` Unknown; `background`, `budget_native`, `tool_allowlist`, `named_agent` No;
  `usage_report` and `limit_signal` the profile's; `model_select` Session; `take_over` Yes if the profile has a
  `resume_argv`, else No; `mcp_servers` support Yes, `stdio` Yes, `http` Unknown, `sse` No. Everything else, and
  `session_resume`, is `Unknown`.

**Tests**, all in the `acp_adapter` target. No test touches the real home, launchd or a daemon; the fake child and the
shim's child have `env_clear()`, then `PATH`, `HOME` and `COLLIDE_DAEMON_ROOT` in a temp root. The profile is built in
the test and names `test-acp-agent`.

- Seen red: the module was first written as stubs whose bodies were `todo!()`, so the 28 new adapter and probe tests
  compiled and failed. The two `clientInfo` tests failed on the missing field before the session was changed. In all, 31
  tests were red (the 30 new ones, and the existing initialize test with its expected bytes changed). They went green
  with the code and were not changed after that except for formatting.
- `probe.rs` (8): `load_session_true_gives_replay_false_gives_none_and_absent_or_odd_gives_unknown`,
  `http_is_read_from_the_mcp_capabilities_and_is_unknown_when_the_response_leaves_it_out`,
  `what_the_probe_says_about_mcp_servers_in_the_merged_table`,
  `a_response_that_says_nothing_usable_gives_unknown_for_everything_it_covers`,
  `fields_the_probe_does_not_read_change_nothing`, `a_loud_response_with_no_load_session_does_not_say_none`,
  `the_merge_changes_only_session_resume_and_http`, `merging_a_probe_that_knew_nothing_changes_nothing`.
- `adapter.rs` (21): the table (`a_profile_with_no_usage_claim_gives_the_engines_fixed_flags_and_unknown_for_the_rest`,
  `what_the_profile_claims_about_usage_limits_and_take_over_is_in_the_table`,
  `the_engines_flags_are_the_same_for_every_profile`,
  `a_probe_that_has_run_is_merged_into_the_table_and_nothing_else_moves`); `launch`
  (`launch_gives_the_profiles_argv_the_requests_folder_and_the_profiles_strip_list`,
  `the_first_prompt_is_in_neither_argv_nor_env`, `a_required_capability_that_is_unknown_is_refused_with_its_name`,
  `a_required_capability_that_is_no_is_refused_with_its_name`,
  `a_required_sub_flag_the_engine_says_no_to_is_refused_by_its_dotted_name`,
  `the_refusal_names_the_agent_of_the_profile`, `a_required_capability_that_is_yes_launches`,
  `a_probe_that_says_yes_lets_a_role_that_requires_it_launch`,
  `a_session_id_cannot_be_assigned_or_resumed_because_the_session_cannot_carry_it_yet`, `launch_writes_no_file`);
  `session`
  (`the_first_two_lines_a_session_writes_are_initialize_and_session_new_in_the_requests_folder`,
  `a_session_against_the_fake_child_writes_what_the_child_expects_and_binds_its_id`,
  `the_servers_of_the_adapter_are_in_session_new_and_the_prompt_is_the_requests`); `config_paths` and
  `interactive_command` (`the_config_paths_are_the_profiles_and_empty_for_a_profile_that_has_none`,
  `the_interactive_command_is_the_profiles_with_the_session_id_last_in_the_sessions_folder`,
  `a_profile_with_no_resume_command_has_no_interactive_command`,
  `a_session_id_that_could_pass_for_a_flag_gets_no_interactive_command`).
- `session.rs` (1 new, 1 changed): `initialize_names_the_client_and_its_version_and_nothing_else_about_it`;
  `start_writes_initialize_with_protocol_version_1_the_client_and_the_session_flags` (renamed, now expects
  `clientInfo`).
- `host/tests/adapters/acp/main.rs` lists `adapter` and `probe`. No recording was added: the session test uses
  `handshake-bare` of `ts-0086-tm`.
- Checks: `cargo build --workspace`, `cargo test --workspace --no-fail-fast` (2710
  passed; 2680 before), `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`, `pnpm test`,
  `pnpm typecheck`, `pnpm --filter @collide/app build`, `pnpm build:packs` and `pnpm check` all pass. No test failed in
  code this item did not touch.

### Decisions for the owner

1. **`launch` refuses on a list the adapter holds (`requiring`).** The item says `launch` runs
   `refuse_unmet_capabilities`, "as Claude's launch does". Claude's `launch` does not: its caller does, and `pack::Role`
   has no `require` key yet. `LaunchRequest` has no list either. So the adapter takes the list when it is built.
   Alternative: a `required` field on `LaunchRequest` (a change to `core`). Recommend keeping, and moving the list to
   the request when the role key is decided.
2. **Only the first refusal reaches `LaunchError::Capability`**, which holds one `CapabilityRefusal`; the helper
   returns every one. Alternative: let `LaunchError::Capability` hold a list (a change to `core` and to Claude's
   readers). Recommend keeping: the caller that needs every name can call the helper itself.
3. **`sse` is `No` whatever the agent says, and the probe does not read it.** The engine has no `sse` form
   (`ts-0087-mf`), so Collide cannot use one, and `Yes` would let a role pass launch and fail later. `stdio` is `Yes`
   without being read. So `mcp_servers.sse` is `No` even when the response leaves `mcpCapabilities` out: this is an
   engine fact, not a response-derived field. Alternative: read `sse` and give `Yes`. Recommend keeping until a use
   exists.
4. **`budget_native` is fixed `No`, not a profile field.** The item lists it both as a fixed flag and as something the
   profile says. ACP has no budget carrier (S7, G2), so the profile has nothing to say. `limit_signal` and
   `usage_report` are the profile's. Recommend keeping.
5. **`take_over` follows `resume_argv`:** `Yes` with a command, `No` without. The item says "as the profile says", and a
   profile that has no command cannot take over. Alternative: a separate field, `Unknown` by default. Recommend
   keeping.
6. **The resume command takes the id as its last argument.** `resume_argv` is the program and its leading arguments.
   That covers opencode's `--session <id>` form (S7, not run). A command with the id in the middle needs a template;
   none exists. An id that is not a safe token (0.5) gets no command (`None`), so an agent-written id cannot pass for
   a flag in a terminal. Recommend keeping.
7. **`launch` refuses `Assign` and `Resume`.** An ACP agent names its own session (S7, G1), and the session has no
   `session/load`, so either would be dropped without a word. The item does not say. Recommend keeping, and relaxing
   `Resume` when a later item builds `session/load`. Alternative: ignore `request.session`.
8. **`clientInfo` is the engine's constant, not a profile field.** `name` `collide`, `version` the crate's. Decision 6
   of `ts-0087-mf` expected the profile to know the version; the version is Collide's own, so it is the same for every
   agent. Alternative: a field in the profile. Recommend keeping.
9. **Flags S7 measured but the item does not list stay `Unknown`:** `permission_host.request_reason`, `hook_events` and
   `refusal_source` (S7: none for ACP), and `compaction_events`, `strict_config`, `system_prompt`, `context_options`.
   A role that requires one is refused either way. Recommend setting `request_reason`, `hook_events` and
   `refusal_source` to `No` in a later pass: they are engine facts, and `No` gives a clearer refusal.
10. **The probe takes the whole JSON-RPC response** (with `result`), not the `result` alone, since that is the line the
    daemon has. An error response, or anything that is not an object, gives `Probed::default()`.
11. **`AcpAdapter::probed` stores the answer in the adapter**, so `capabilities()` is merged. The item says the merge is
    a function the caller holds; `Probed::merge_into` is that function and the builder is the convenience. Recommend
    keeping both.
12. **Plan text, proposed:** none needed.

- Owner decisions (2026-11-16, at the build): decisions 1 to 11 taken under the owner's delegation, as recommended.
  The later pass of decision 9 (`request_reason`, `hook_events` and `refusal_source` to `No`) is left to the item
  that next touches the capability table; until then a role that requires one is refused either way.

### Not done, by design

- No real profile (decision of the owner at the split): the first one comes with Stage 6b, from a real run.
- The probe does not read `promptCapabilities` (the item says "read and not used yet"; a field read and dropped cannot
  be tested) or `sessionCapabilities.resume`. So `loadSession: false` gives `None` even for an agent that has the silent
  `session/resume`.
- No call that sends `initialize` to a child and reads the response, no `AcpAdapter::feed`, no daemon use of the
  adapter (`ts-0092-tw`).
- The model is not passed to the session: `AcpOptions` has no model yet (`ts-0090-jd` adds it and reads
  `request.model` in `AcpAdapter::session`).
- What `ts-0089-wv` (permission, cancel) and `ts-0091-pz` (usage, empty turns) own. `usage_report.quality` is in the
  table; the session does not read it yet.
- The core double's `AcpDialect` holds no profile, as the item says.
- `launch` takes no budget: a role with a budget is refused by the capability helper once the caller requires
  `budget_native` or `usage_report.cost`, as the helper's doc says.
