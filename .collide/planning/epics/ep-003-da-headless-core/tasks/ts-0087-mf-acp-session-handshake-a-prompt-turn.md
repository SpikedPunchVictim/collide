---
id: ts-0087-mf
kind: task
status: active
title: "ACP session: handshake, a prompt turn and replay"
owners: []
links:
  - { type: blocked-by, to: ts-0086-tm }
---

## Goal

The ACP engine's `AgentSession`, in `host` (`host/src/adapters/acp/session.rs`): the second implementation of the
trait (0.3, Two core traits; Stage 3d). It starts from the scripted double in `core/src/testing/acp.rs`, which its own
doc calls the outline of this engine, and hardens it. Nothing ACP-shaped moves into `core`: the double stays there as
the test double of `core`'s own tests.

- `start` writes `initialize`: protocol version 1, and the client capabilities of 0.3 (Later adapters): **no** `fs`,
  `terminal` or `elicitation`, and the `session` flags `notices`, `compaction` and `configOptions`. S7's `driver.mjs`
  has the shape of the `session` flags.
- The `initialize` response makes it write `session/new` with the working folder and `mcpServers`. The servers come from
  the session's options and are `[]` by default. Stage 6b's ask box fills them with its endpoint; nothing here knows
  of it.
- The `session/new` response gives `SessionBound` with the agent's id, and makes the session write the first prompt
  (`LaunchRequest.prompt`, held as the session's own data, so a rebuilt session has it too).
- Request ids count from 0. A response is matched to its request by id. One that matches nothing changes nothing.
- `session/update`:
  - `agent_message_chunk` gives `Message`;
  - `tool_call` gives `ToolCall`, classified by `kind` and never by the title (S7, O12), with `input` `None` while the
    agent sends none or `{}` (S7, O5);
  - any other update (`tool_call_update`, `agent_thought_chunk`, `plan`, `available_commands_update`,
    `config_option_update`) gives `Other` named by its `sessionUpdate`.
- The `session/prompt` response ends the turn: `end_turn`, `max_tokens`, `max_turn_requests`, `refusal` and `cancelled`
  map to the `TurnEndReason` of the same meaning; any other word, or a JSON-RPC error, is `Error`. A `FinalReport`
  with source `Reconstructed` holds the message chunks after the last tool call.
- A request from the agent that the engine does not serve (`fs/*`, `terminal/*`, `elicitation/*`, any unknown method)
  is answered with the JSON-RPC error `-32601`, so the agent never waits on it, and is reported as `Other`.
- `send_turn`, `answer`, `cancel`, `finish` and `replay` are the double's, over the shared walk
  `collide_core::replay::replay_records`. Permission and cancel are hardened by `ts-0089-wv`; the model by
  `ts-0090-jd`; empty turns, lost children and usage by `ts-0091-pz`.

## Success criteria

- The session passes the whole shared contract (`collide_core::testing::contract`, the 19 scenarios) through
  `session_contract_tests!`, as Claude's session does in `host/tests/adapters/claude/contract.rs`.
- Over the fake ACP child under the real shim, one prompt turn gives, in order: `SessionBound` with the stored time of
  the `session/new` response, `Message`, `ToolCall` of kind `read`, `Message`, `FinalReport` holding only the words
  after
  the tool call, and `TurnEnd { EndTurn }`. `finish` says `Ended`.
- Replayed from the journal the real shim wrote, the session reaches the state of the live one, also for a journal cut
  at every byte (Stage 3d: the second `AgentSession` keeps the trait honest).
- `initialize` holds no `fs`, `terminal` or `elicitation`. `session/new` holds the working folder, and `mcpServers`
  as the options give them, `[]` when they give none.
- An agent request the engine does not serve gets `-32601` with its own id, and an `Other` with the method's name.
  Nothing of the request's params is kept in the event (0.3: a stream's content is untrusted and may be large; as
  `ts-0062-rj`, decision 1).
- An update of an unknown kind, and a stdout line that is not JSON, are each an `Other` with the kind or
  `unparsed_line` and no content. The journal holds the line whole.
- A JSON-RPC error in answer to `initialize` or `session/new` writes nothing more, is an `Other`, and makes `finish`
  say `Failed { cause: Api { status: None } }` (see the decision below).
- The `TurnEnd` of each stop reason in the list above, and `Error` for an unknown word and for an error response.
- No new crate is added.

## Tests

Write these first:

- the shared contract (a `ChildDialect` for the ACP session, as the Claude one: the handshake lines, a message, a
  permission request, a turn end);
- the fake ACP child under the real shim, with a live session fed from the journal (a `play` helper next to the child's,
  as `host/tests/adapters/claude/fake.rs` has): one prompt turn, its events in order and with their stored times;
- replay equal to live, a journal cut at every byte, a journal that is not this session's rejected with the record;
- `initialize` and `session/new` as written bytes, with and without servers;
- ids: the first requests are 0, 1 and 2; a response that matches no request changes nothing;
- each stop reason, an unknown one, an error response to the prompt;
- an unserved agent request, answered `-32601` and reported;
- an unknown update, a line that is not JSON;
- an error response to `initialize`, and to `session/new`;
- `ToolCall` classification by `kind` with a misleading title, and `input` `None` for `{}` (S7, O5, O12).

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 3d: ACP engine.
  - 0.3: Two core traits; Normalized events; A session starts as; Later adapters (an ACP adapter is one engine plus a
    profile; no `fs`, `terminal` or `elicitation`); The stream protocol (a phase's outcome is its prompt response).
  - 0.15: Dependency injection (the shared contract keeps the traits honest).
  - S7 record: P1, P2, P6, P8, P9 and O6, O12.
  - Stage 6b: The in-app agent (what it will later use: `mcpServers` in `session/new`, `ToolCall` kinds).
- **Decision for the item, with a recommendation: copy, do not share.** `core` may not hold the engine, and `host` may
  not use `core`'s `testing` module outside tests. Recommend copying the double's logic into `host` and hardening it,
  and keeping the double as is. The shared contract keeps the two in step. Update the double's module doc so it points
  at the engine in `host` and no longer calls itself the outline of a later stage.
- **Decision for the owner, with a recommendation: who writes the first prompt.** The double writes it as the
  handshake completes, from data given at construction; Claude's first turn comes through `send_turn`. Recommend
  keeping the double's way for ACP: it is replay-safe, because a rebuilt session has the prompt in its own data, where
  a turn queued by `send_turn` before the handshake ended would be lost by a restart in that window. The daemon then
  needs to know which of the two a session is. `ts-0092-tw` takes that up. Alternative: one rule for every adapter
  (`start`, then `send_turn` for the first turn, ACP holds it until ready), which needs a change to the contract and has
  the window above.
- **Decision for the item: the options type.** Recommend a small struct of the session's data (working folder, first
  prompt, `mcpServers`, and the model that `ts-0090-jd` adds), built from a `LaunchRequest` by the adapter of
  `ts-0088-pk`. Give each server a typed form for the `http` and `stdio` kinds ACP defines, not a raw `Value`, so a
  caller cannot send a shape the agent rejects. `sse` is left out until a use for it exists.
- **Decision for the item, with a recommendation: a failed handshake.** ACP has an `auth_required` error
  (`-32000`) for an agent whose login has lapsed. Recommend `Failed { cause: Api { status: None } }` here, with the
  error's text kept as evidence by `ts-0091-pz`. A new `PhaseFailure` variant for it is a larger change than this
  stage needs.
- `tool_call_update` stays an `Other`. Reading `rawOutput` for a `ToolResultSize` is a metrics item, and S7 measured
  that the update arrives late (the agent's own bookkeeping).
- The agent's `session/request_permission` may arrive before the `tool_call` it names (S7, O6). Hardened in
  `ts-0089-wv`; here the session must not fail on it.
- Run `pnpm gen` if any wire type changes. None is expected.
- Area: `host` (`adapters/acp/` new, `adapters/mod.rs`), `core/src/testing/acp.rs` (a doc line), the test folder of
  `ts-0086-tm`.
- The test rule: no test touches the real home, real launchd or a real daemon. Each test clears the environment
  (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as `host/tests/common/mod.rs` `TempRoot`
  does. Settings files are fixtures, never the owner's own `settings.json`.
- Stage 3d item (revision 23 scope, split 2026-11-15). Items of Stage 3d run one at a time, in id order, because
  they share `host` and the generated files.
- Owner decisions (2026-11-15, at the split): the decisions for the owner above, taken as recommended under the
  owner's delegation; the owner may change them before the item is built. The ACP protocol is written by hand: no ACP
  crate is added.

### Built

The ACP engine's session, its options and its tests. No wire type changed, so `pnpm gen` had nothing to regenerate. No
crate was added: the JSON of `session/new` is built by hand (`host` has `serde` without the `derive` feature).

- `host/src/adapters/acp/mod.rs` (new), `host/src/adapters/mod.rs`: the module `acp`, with a tag table as the Claude
  adapter has.
- `host/src/adapters/acp/options.rs` (new): `AcpOptions` (`cwd`, `first_prompt`, `mcp_servers`; `new`,
  `with_mcp_servers`) and `McpServer` (`Http` and `Stdio`, `to_json`). A stdio server has no `type`, as the schema
  has it.
- `host/src/adapters/acp/session.rs` (new): `AcpSession`, built from `AcpOptions`; `Debug`, `Clone` and `PartialEq`, so
  a test can compare a replayed session with the live one. It began as a copy of the double's logic, and differs from it
  where the item asks:
  - `start` writes `initialize` with protocol version 1 and `clientCapabilities.session` flags `notices`, `compaction`
    and `configOptions` only.
  - `read_response` writes `session/new` with `cwd` and `mcpServers` from the options, then, on its response,
    `SessionBound` and the first prompt, which is the options' own data.
  - `read` answers an agent request it does not serve (`fs/*`, `terminal/*`, `elicitation/*`, any unknown method) with
    `-32601` and the agent's own id (number or string kept as written), and reports `Other` named by the method.
  - Every `Other` this session makes has `raw: None`, and its name is cut to 64 characters. A line that is not a JSON
    object is `Other` `unparsed_line`.
  - An error, or an answer with no result, to `initialize` or `session/new` writes nothing more, is an `Other`
    (`initialize_error`, `session_new_error`, `malformed_response`) and sets `handshake_failed`; `finish` then says
    `Failed { Api { status: None } }`. A shim that left no exit record is still `Lost`.
  - `end_turn` maps the five stop reasons; an unknown word, a missing one and an error response are `Error`. The
    `FinalReport` (`Reconstructed`) holds the chunks after the last tool call.
  - `outcome_of` is `host`'s own copy of the rule in `core`'s `testing` module, which `host` may not use outside tests.
  - `send_turn`, `answer`, `cancel` and `replay` are the double's. `TurnEnd.empty` is `false`, no `Usage` event is made
    and `pending_permissions` is the trait's default: those are `ts-0091-pz` and `ts-0089-wv`.
- `core/src/testing/acp.rs`: the module doc now says the double is `core`'s own and points at the engine in `host`.
- `host/tests/adapters/acp/fake.rs`: `play` (the child under the real shim, a live `AcpSession` fed from the shim's line
  hook, the journal and the exit info back), `Played`. `main.rs` lists the new test modules.

**Tests**, all in the `acp_adapter` target, none touching the real home, launchd or a daemon (the child and the shim's
child have `env_clear()`, then `PATH`, `HOME` and `COLLIDE_DAEMON_ROOT` in a temp root).

- Seen red: the module was first written as a stub whose methods were `todo!()`, so the 67 new tests compiled and all
  67 failed (the 26 tests of `fake_acp.rs` still passed). They went green with the code, and were not changed after
  that except for formatting.
- `contract.rs`: the 19 scenarios of `session_contract_tests!` for an ACP dialect of the shared contract
  (`acp_session::*`).
- `session.rs` (20): `start_writes_initialize_with_protocol_version_1_and_the_session_flags`,
  `initialize_holds_no_fs_terminal_or_elicitation`,
  `the_initialize_response_makes_it_write_session_new_for_the_working_folder_with_no_servers`,
  `session_new_holds_the_servers_the_options_give_in_the_two_kinds_acp_defines`,
  `a_server_with_no_headers_args_or_env_sends_empty_lists`,
  `the_session_new_response_binds_the_agents_id_with_its_stored_time_and_writes_the_first_prompt`,
  `the_first_requests_are_0_1_and_2_and_the_next_turn_takes_3`, `a_response_that_matches_no_request_changes_nothing`,
  `a_response_is_matched_to_its_request_by_id_not_by_order`,
  `an_agent_request_the_engine_does_not_serve_is_answered_with_method_not_found_and_reported`,
  `an_unserved_request_keeps_the_agents_id_as_it_wrote_it_and_none_of_its_params`,
  `an_unserved_request_changes_no_request_the_session_waits_on`,
  `a_notification_the_engine_does_not_know_is_reported_and_answered_with_nothing`,
  `a_very_long_method_or_update_kind_is_cut_in_the_name_the_event_carries`,
  `a_stdout_line_that_is_not_json_is_an_other_with_no_content`,
  `an_error_in_answer_to_initialize_writes_nothing_more_and_is_an_other`,
  `an_error_in_answer_to_session_new_writes_nothing_more_and_is_an_other`,
  `a_failed_handshake_makes_finish_say_failed_with_an_api_cause_and_no_status`,
  `a_session_new_response_with_no_session_id_is_malformed_and_fails_the_handshake`,
  `a_child_that_went_away_before_any_prompt_response_is_lost`.
- `turn.rs` (18): the update mapping (`a_message_chunk_gives_a_message_with_the_stored_time`,
  `a_message_chunk_that_holds_no_text_is_an_other_with_no_content`,
  `every_other_update_is_an_other_named_by_its_session_update_with_no_content`,
  `an_update_with_no_kind_is_an_other_named_session_update`,
  `a_tool_call_is_classified_by_its_kind_and_never_by_its_title`,
  `a_tool_call_with_no_kind_is_of_kind_other_whatever_its_title_says`,
  `a_tool_calls_input_is_none_while_the_agent_sends_none_or_an_empty_object`,
  `a_permission_request_before_its_tool_call_is_not_an_error`); the turn's end
  (`each_stop_reason_ends_the_turn_with_the_reason_of_the_same_meaning`,
  `an_unknown_stop_reason_a_missing_one_and_an_error_response_are_each_an_error_turn_end`,
  `finish_says_what_the_turn_end_says`,
  `the_final_report_holds_the_words_after_the_last_tool_call_and_comes_before_the_turn_end`,
  `a_turn_with_no_words_after_its_last_tool_call_has_no_final_report`, `a_turn_with_no_tool_call_reports_all_its_words`,
  `an_error_response_gives_no_final_report_and_the_next_turn_starts_with_no_words`,
  `a_turn_the_host_cancelled_ends_cancelled_whatever_the_agent_says`); and the child under the real shim
  (`a_prompt_turn_under_the_real_shim_gives_its_events_in_order_with_their_stored_times`,
  `the_journal_of_a_prompt_turn_holds_the_three_requests_the_session_wrote`).
- `replay.rs` (10): `replayed_from_the_journal_the_prompt_turn_reaches_the_state_of_the_live_session`,
  `replayed_from_the_journal_the_permission_recording_reaches_the_state_of_the_live_session`,
  `a_replayed_session_finishes_as_the_live_one_does`, `a_replayed_session_numbers_its_next_write_as_the_live_one_does`,
  `the_prompt_turn_journal_cut_at_any_byte_replays_up_to_the_last_whole_record`,
  `the_permission_journal_cut_at_any_byte_replays_up_to_the_last_whole_record`,
  `a_journal_cut_after_the_handshake_reply_has_the_session_id_and_numbers_the_next_write_after_it`,
  `a_journal_cut_after_the_agents_request_and_before_the_answer_answers_as_the_live_session_did`,
  `a_journal_that_is_not_this_sessions_is_rejected_with_the_record`,
  `a_journal_with_a_method_this_session_never_writes_is_rejected_at_that_record`.
- No recording was added: the shim tests use `prompt-turn` and `permission-request` of `ts-0086-tm`; the other cases are
  fed by hand.
- Checks: `cargo build --workspace`, `cargo test --workspace --no-fail-fast` (2680 passed; 2613 before), `cargo clippy
  --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`, `pnpm test`, `pnpm typecheck`,
  `pnpm --filter @collide/app build`, `pnpm build:packs` and `pnpm check` all pass. In the first plain
  `cargo test --workspace` run `shim_child`'s `a_shim_that_cannot_write_exit_json_keeps_the_record_and_leaves_no_file`
  failed once (a `Permission denied` renaming `child.json` under load); it is code this item did not touch, it passed
  alone, and it passed in the full rerun.

### Decisions for the owner

1. **A `session/new` result with no `sessionId` fails the handshake.** The item names the error response only. The
   session cannot go on without an id, so it is `Other` `malformed_response` and `finish` says
   `Failed { Api { status: None } }`, as for an error. Recommend keeping. Alternative: leave `finish` as `Lost`.
2. **`tool` of a `ToolCall` stays the call's `title`**, as the double has it (ACP sends no programmatic name; S7, O12).
   A title can become a path while the call runs, so it may hold content. Recommend keeping, because the Queue and the
   timeline need a label and the kind is never read from it. Alternative: `tool: None` for ACP, which loses the label.
3. **A name is cut to 64 characters** in an `Other`, for the method of an unserved request and for an update kind. The
   item says "no content"; a name is the agent's text and may be large. Recommend keeping.
4. **A line that is valid JSON but not an object** (`42`, `[1]`) is `Other` `unparsed_line`. Alternative: drop it.
   Recommend keeping, so no stdout line is silent.
5. **`TurnEnd.empty` is `false` and no `Usage` is made here**, though the double computed `empty`. The double's
   check ignored usage, so it would call a turn with usage and no words empty. `ts-0091-pz` owns both. Recommend
   keeping the gap until then; until `ts-0091-pz` an empty `end_turn` is `Ended`, so no run may use this session.
6. **No `clientInfo` in `initialize`.** It is optional in ACP and the item lists none. Recommend adding name and version
   with the profile in `ts-0088-pk`, which knows the version.
7. **`McpServer` has no serde derive.** `to_json` writes the shapes by hand, so `host/Cargo.toml` is unchanged.
   Alternative: turn on `serde`'s `derive` feature for `host`. Recommend keeping.
8. **Plan text, proposed:** none needed. The module docs of the double and of `adapters/mod.rs` were updated;
   `core/src/replay.rs` still says the ACP session "will" use the walk, which is now true and can be reworded later.

- Owner decisions (2026-11-16, at the build): decisions 1 to 7 taken under the owner's delegation, as recommended.
  The 67 tests were seen red against a `todo!()` stub. `shim_child`'s
  `a_shim_that_cannot_write_exit_json_keeps_the_record_and_leaves_no_file` failed once under load (`Permission
  denied` renaming `child.json`) and passed alone and in the rerun; it is watched, and gets an item of its own if it
  recurs. Carried to `ts-0092-tw` (a note there): a journal cut between the `session/new` response and the first
  prompt leaves a rebuilt session that owes its prompt write.

### Not done, by design

- Permission and cancel hardening, `pending_permissions` (`ts-0089-wv`); the model and `session/set_config_option`
  (`ts-0090-jd`); empty turns, `Usage`, stderr evidence and lost children beyond the two the contract needs
  (`ts-0091-pz`); the adapter, profile and probe (`ts-0088-pk`); the daemon phase (`ts-0092-tw`).
- A journal cut between the `session/new` response and the first prompt leaves a bound session that never wrote its
  prompt, and `replay` writes nothing. The session has the prompt and the id, so `ts-0092-tw` can decide how a rebuilt
  session says that it still owes a write. A test shows the state (`a_journal_cut_after_the_handshake_reply_...`).
- `sse` servers, `session/load`, `tool_call_update` content and `rawOutput` (a metrics item).
- The daemon does not know whether a session writes its first prompt itself (ACP) or gets it from `send_turn`
  (Claude): `ts-0092-tw`.
