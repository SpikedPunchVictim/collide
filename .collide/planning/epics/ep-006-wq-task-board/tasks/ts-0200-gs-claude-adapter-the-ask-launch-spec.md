---
id: ts-0200-gs
kind: task
status: done
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

### Built

`host/src/adapters/claude/ask.rs` (new, `pub mod ask` in `mod.rs`; no new trait, no new crate, `ClaudeAdapter`
and core's `LaunchRequest` unchanged):
- `AskProfile { model, read, config, budget_usd, dir }` and `ask_launch_spec(&AskProfile) -> Result<LaunchSpec,
  LaunchError>`. The argv is 0.3's pinned line with `--restricted` on its own word after `--permission-prompts none`
  (`ts-0198-tf`: adopt). The fixed words are the constants `HEAD`, `CONFINEMENT`, `READ_TOOLS`, `UI_TOOLS`,
  `SERVER_NAME` and `ASK_PROMPT`. The env to add is empty, `env_strip` is the adapter's, and the working directory is
  the first readable folder. Refused with plain text: no folder, a folder or the ask folder that is not an absolute
  UTF-8 path, a model that is not a safe token, a budget that is not a positive amount.
- `check_before_launch(agent, &Capabilities, path) -> Result<PathBuf, LaunchError>`: `mcp_servers.http` through
  `refuse_unmet_capabilities` ("this agent cannot use HTTP MCP servers", naming the agent), then `claude` on the
  given `PATH` ("claude was not found on PATH"). It reads no environment of its own.
- `write_ask_folder(dir, endpoint, token)`: folder mode 0700, then `settings.json` and `mcp.json` (the `collide` server,
  `type: http`, the URL and an `Authorization: Bearer` header) at mode 0600 with `write_atomically`. An empty endpoint
  or token is refused (`AskFolderError`). `remove_ask_folder(dir)`: see Decisions.
- `host/src/adapters/claude/launch.rs`: `invalid` and `safe` are `pub(super)`, to be reused.

Tests, in `host/tests/adapters/claude/ask.rs` (28; no process starts; the fake `claude` is made with
`write_executable`):
- the line: `the_argv_is_the_pinned_line_word_for_word_for_the_default_entry`,
  `the_working_directory_is_the_first_readable_folder`, `the_argv_holds_neither_the_ask_token_nor_the_request_text`,
  `the_ask_gets_its_request_on_stdin_...`, `add_dir_is_passed_once_per_readable_folder_after_the_first` (1, 2, 3),
  `permission_prompts_are_off_and_neither_slash_commands_nor_bare_are_touched`,
  `restricted_follows_permission_prompts_on_its_own_word`, `config_user_changes_only_the_setting_sources`,
  `the_budget_flag_is_on_the_line_only_when_the_entry_sets_a_budget`,
  `a_budget_that_claude_would_not_enforce_is_refused`,
  `a_profile_with_no_folder_a_relative_folder_or_an_unsafe_model_is_refused`;
- S6's three launch tests and its environment test, ported: `it_uses_only_the_collide_mcp_server_and_the_cheap_model`,
  `it_gives_the_agent_reading_tools_and_the_ui_tools_and_no_way_to_run_or_write`, the request-off-argv test above, and
  `the_environment_drops_every_claude_variable_and_keeps_the_rest_of_the_base_environment` (`CLAUDE*` only, as 0.3
  says; `OTEL_*` is not stripped), plus
  `the_fixed_prompt_says_to_read_first_name_what_was_left_out_and_never_guess_an_id`;
- the folder: `the_ask_folder_holds_settings_and_mcp_files_of_mode_0600_and_is_closed_to_others`,
  `mcp_json_names_the_collide_server_with_the_endpoint_and_a_bearer_header`, `settings_json_turns_auto_memory_off_...`,
  `a_token_that_would_break_the_json_is_escaped_not_pasted`, `an_empty_endpoint_or_token_is_refused_and_writes_nothing`,
  `removing_the_folder_removes_it_with_its_token_and_nothing_beside_it`, `removing_a_folder_that_is_not_there_...`,
  `a_relative_path_is_refused_by_the_remove_...`, `a_symlink_is_not_followed_by_the_remove`;
- the checks: `the_command_is_found_on_the_path_it_is_given`, `a_missing_command_is_refused_with_the_plain_text`,
  `an_agent_without_http_mcp_is_refused_with_the_plain_text_and_the_agent_is_named`,
  `the_capability_is_checked_before_the_command_is_looked_up`.

Red at runtime: I wrote the types and signatures first, with bodies that return the old behavior (an empty argv, writes
that do nothing, `Ok` from the checks). 27 of 28 failed on the right assertion (for example an empty argv against the
pinned line). `removing_a_folder_that_is_not_there_is_not_an_error` passed against the no-op stub, as it must: nothing
to remove. `the_argv_holds_neither_...` first passed with an empty argv; I added a check that the argv is a real
line and saw it red. All 28 pass against the real code.

### Decisions for the owner

- **`--restricted`.** In the line, on its own word after `--permission-prompts none`. Proposed plan text for 0.3: add
  `--restricted` after the `--permission-prompts none` line of the block, with the comment "reads stay in the folders
  (measured ts-0198-tf)"; replace the `--restricted` bullet with "adopted: measured in `ts-0198-tf`, an outside read is
  refused with no prompt, and `--add-dir` takes effect only with it"; and in the `--add-dir` bullet drop "its effect
  under `-p` is unmeasured". Stage 6b's criterion for the launch specs needs no word change.
- **Profile shape.** `model` is a required string and `config` a plain `RoleConfig`, so the caller resolves the entry
  (`haiku` and `none` are the defaults) and the adapter has no second default. Alternative: `Option` for both and the
  adapter defaults them. Recommend as built: the run line's default for `config` is `user`, and two defaults in two
  places would drift.
- **The checks are a second function**, not part of `ask_launch_spec`, so the spec stays a pure function of the
  profile. Alternative: one call that takes capabilities and `PATH`. Recommend as built; `ts-0201-us` calls the check,
  writes the folder, then builds the spec.
- **The HTTP refusal is `LaunchError::Invalid` with the plan's text**, not `LaunchError::Capability`, whose message is
  the run's text ("a role that requires it cannot run on this tool"). Alternative: a new message for the variant.
  Recommend as built.
- **Absolute paths.** Folders and the ask folder must be absolute, so no folder reads as a flag and none depends on the
  child's start directory. `<board root>` is the caller's to resolve. A relative path is refused with its text.
- **Removing the folder** is `remove_dir_all` of exactly that path, and only after it is checked to be an absolute path
  to a real folder (not a link, a file or the root). A missing folder is `Ok`. Alternative: remove the two known files
  and then the folder with `remove_dir`, which can never delete anything else but fails on a folder that holds a record.
  Recommend as built for this item; see the first item in Not done.
- **The prompt** is S6's prompt v2 as written, with no change, because the item says "from S6's prompt v2". It is one
  constant, `ASK_PROMPT`.
- **The command lookup** reuses `file_run::find_on_path` (the run's), and `check_before_launch` returns the found path.
  The program is not in the spec's argv, as for a run's line.
- **No new stub tag** was added for `ask.rs`: its module doc has no tag table.
- Review (2026-11-27): the recommendations above are taken. The prompt was compared with S6's prompt v2: the same
  words. The plan now carries `ts-0198-tf`'s result, as Stage 6b said it would: 0.3's pinned line has `--restricted`
  after `--permission-prompts none`; its `--add-dir`, `--permission-prompts none`, `--restricted` and outside-read
  bullets say what was measured; the Agents page says "can read only" these folders for Claude under this line; and
  Stage 6b's launch-spec criterion names `--restricted`. For `ts-0201-us`: the record stays in the ask folder, so it
  removes only `mcp.json` when the ask ends, not the folder.

### Not done, by design

- Starting the process, the record, the timeout and the removal of `mcp.json` when the ask ends: `ts-0201-us`. That
  item says the ask's record lives in the ask folder and only `mcp.json` goes, so it either calls a small function that
  removes the token file, or keeps its record outside the ask folder and uses `remove_ask_folder`. This item adds
  neither.
- The ACP profile, and opencode's permission block (`webfetch` and `task` stay open under a block that names only
  `edit` and `bash`): `ts-0208-dx`.
- The words "can read only" on the Agents page and in the ask box: `ts-0210-mz`.
- The opt-in contract test that plants a file outside the folders and checks that its marker never reaches the answer:
  it needs a real `claude`, so it is not here.
- The plan edit for `--restricted` (above): at review.
