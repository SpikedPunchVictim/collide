---
id: ts-0090-jd
kind: task
status: active
title: "ACP session: choosing the model"
owners: []
links:
  - { type: blocked-by, to: ts-0089-wv }
---

## Goal

The ACP session sets the model a role asked for, checks it against what the agent offers, and never reaches a model the
role did not name (0.3, Who picks what; S7 P6, O8, O9, O11). Capability `model_select` is `Session`.

**The id.** The session takes `ModelRef::adapter_id` (the opaque `model.id`, else `model.name`) from the request. The
id is only data: it is written in a JSON-RPC body and never in argv, env or a path, so it needs no safe token (the
`name` has one and is what the host puts in env and paths).

**After `session/new`.** The response may carry the model in two forms, and the session reads both:

- stable: `configOptions`, a list; the option with `category: "model"` holds `id`, `currentValue` and `options` (a flat
  list of `{ value, name }`, or groups of them);
- draft: `models`, with `currentModelId` and `availableModels` (`modelId`, `name`).

The session then:

1. finds the advertised ids, from `configOptions` if it has a model option, else from `models`, else none;
2. when the request has no model: writes nothing and keeps the agent's own default;
3. when the id is the current one: writes nothing;
4. when the id is advertised: writes `session/set_config_option` with the option's `configId` and the `value`, if the
   model came from `configOptions`; else `session/set_model` with the `modelId`;
5. waits for that response before it writes the first prompt (the prompt is held until the model is set), and checks the
   answer: for `set_config_option`, the response's `configOptions` hold the value as current; for `set_model`, an empty
   success is enough, and a later `current_model_update` or the next `session/new` must not contradict it;
6. when the id is **not** advertised, or the agent has no way to set a model: writes no prompt, writes nothing more,
   gives an `Other` named `model_unavailable` (the id is not in the event, only the count of models offered), and makes
   `finish` say the phase failed (see the decision below).

**An agent that sets the model itself.** S7 measured one that ignored the model in its config and used its own default
(O8). The session never trusts the config: it only trusts `currentValue` or `currentModelId` after its own write.

## Success criteria

- With `configOptions` that list the id, the bytes after `session/new` are `session/set_config_option` for the model
  option, with the id, and no `session/set_model`.
- With only the draft `models`, they are `session/set_model` for the id, and no `set_config_option`.
- With both forms in one response, `configOptions` wins.
- With a request that has no model, or whose id is already current, no model request is written, and the prompt follows
  at once.
- The first prompt is written only after the model request was answered. A model response that is an error writes no
  prompt and fails the phase.
- An id that is not in the list: no model request, no prompt, one `Other` named `model_unavailable`, a failed phase.
  This holds for an id that is another provider's hosted model (S7, O11) that the agent lists but the request did not
  ask for: the session never picks a model the request did not name.
- An agent that offers no model list and no way to set one, asked for a model, fails the phase the same way. Asked for
  none, it runs.
- A model id with `/` and `:` (`ollama/qwen2.5:7b`) is written as given in the JSON body. It is in no argv, env or file
  name that this adapter makes (`launch` is checked for this).
- Replayed from a journal, the session is in the same state, also when cut between the model request and its answer.
- The shared contract still passes.
- No new crate is added.

## Tests

Write these first:

- model in `configOptions`: flat list, grouped list, id current, id offered, id not offered, response with an error;
- model in the draft `models`: the same cases with `session/set_model`;
- both forms together;
- no model in the request; no list in the response; no list and a model asked for;
- the prompt held until the answer, and the order of the three writes after `session/new` (model, then prompt);
- the id with `/` and `:` in the body and nowhere in `launch`;
- replay cut between the model request and its answer;
- the fake child under the real shim, for each form: recordings that expect `session/set_config_option` or
  `session/set_model` with the id and answer it, then serve one turn; and one that offers other ids only (the phase
  fails and the child is stopped by the host).

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 3d: ACP engine (model through `configOptions` and `session/set_config_option`, `session/set_model` as the
    fallback).
  - 0.3: Who picks what (`model.id` beside `model.name`, checked against the advertised list); Capabilities
    (`model_select`); Later adapters; The ACP profile.
  - 0.5: safe token.
  - S7 record: P6 and G10, O8, O9, O11.
- **Decision for the owner, with a recommendation: how a refused model fails.** There is no `PhaseFailure` for it.
  Recommend a new variant, `ModelUnavailable`, with `failure_text` in `host/src/agent_feed.rs` saying that the agent
  does
  not offer the model the role names. A small change to `core/src/adapter.rs`, one arm in the host's failure text, and
  `pnpm gen`. Alternative: `Failed { cause: Api { status: None } }` with no new variant. Pro: no wire change. Con: the
  person sees "API failure" for a role that names a model the agent does not have, which is a pack or user-models
  mistake, not an outage.
- **Decision for the item, with a recommendation: where the id comes from.** Recommend that the check is the session's,
  not `launch`'s: the list exists only after `session/new`, so `launch` cannot do it. `launch` still refuses a model
  with no `ModelRef` for this tool (the role's alias is unmapped), as 0.3 says.
- **Decision for the item: the draft form.** The draft `models` shape is not in the stable schema. Keep it behind one
  small function with a comment that says when it can go (when the agents the profiles name all offer `configOptions`).
- The set-model request must not name a model the session did not read from the agent's own list, so a hosted model the
  agent lists (O11) is never chosen by a default.
- A `config_option_update` or a `current_model_update` arriving later stays an `Other`. The session does not follow an
  agent that changes its own model mid-session. A model change by the agent is the agent's, and the usage report names
  the model the turn used when the agent says one.
- Run `pnpm gen` if the variant is added.
- Area: `host` (`adapters/acp/session.rs`, `model.rs`), `core/src/adapter.rs` (the variant, if taken), the test folder.
- The test rule: no test touches the real home, real launchd or a real daemon. Each test clears the environment
  (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as `host/tests/common/mod.rs` `TempRoot`
  does. Settings files are fixtures, never the owner's own `settings.json`.
- Stage 3d item (revision 23 scope, split 2026-11-15). Items of Stage 3d run one at a time, in id order, because
  they share `host` and the generated files.
- Owner decisions (2026-11-15, at the split): the decisions for the owner above, taken as recommended under the
  owner's delegation; the owner may change them before the item is built. The ACP protocol is written by hand: no ACP
  crate is added.

### Built

The ACP session now sets the model the request names, from either list form, and holds the first prompt until the
agent has answered. A model the agent does not offer fails the phase with the new `PhaseFailure::ModelUnavailable`.
No crate was added.

**No wire type changed.** `PhaseFailure` is a Rust enum of `core` with no serde or schema derive, and the store keeps
the failure as free text (`RunRecordResult.failure` is a string in the schema). So the new arm has no generated file
and no TS that matches on it. `pnpm gen` was run and left every generated file as it was.

- `core/src/adapter.rs`, `PhaseFailure::ModelUnavailable` (new arm).
- `host/src/agent_feed.rs`, `failure_text`: `model_unavailable`; the test `a_failure_is_worded_for_the_store` has it.
  `host/src/store.rs`, the doc of `PhaseRecord::failure`, names it.
- `host/src/adapters/acp/options.rs`, `AcpOptions::model` and `with_model`: the id, as data only.
- `host/src/adapters/acp/model.rs` (new, private module): `plan` reads the `session/new` result and says `Keep`,
  `Set(ModelRequest)` or `Unavailable { offered }`; `config_option_holds` checks the answer to
  `session/set_config_option`. `configOptions` (the option with `category: "model"` and an `id`; a flat or grouped
  list) wins; else the draft `models`, which only `draft_listing` knows, with a comment that says when it can go.
- `host/src/adapters/acp/session.rs`, `AcpSession`:
  - `Awaiting::Model(ModelForm)`, a request kept by `note_written` for `session/set_config_option` and
    `session/set_model`, so a replay holds the same awaiting request;
  - `model_refused` (new field) makes `finish` say `Failed { ModelUnavailable }` and makes `send_turn` write nothing;
    `send_turn` also writes nothing while the model request is out (`model_pending`);
  - `read_response` asks `model::plan` on the `session/new` result: `Keep` writes the prompt (as before), `Set` writes
    the request and no prompt, `Unavailable` writes nothing and gives `Other` `model_unavailable` with
    `raw: { "offered": N }` (the count, never the id);
  - `model_answered` (new): an answer that is an error gives `Other` `model_set_error`; a `set_config_option` answer
    that does not hold the id as the option's `currentValue` gives `Other` `model_not_set`; both write no prompt and set
    `model_refused`. Otherwise it writes the first prompt (`first_prompt`, new, shared with the `Keep` path);
  - the module doc: the table rows and a section on the model.
- `host/src/adapters/acp/mod.rs`, `AcpAdapter::session`: passes `request.model.adapter_id()` into the options.
  `launch` is unchanged and writes no part of the model.
- Recordings (new), in `host/tests/adapters/acp/fixtures/recordings/`: `model-config-options` (expects
  `session/set_config_option` with `configId` `model`, answers with the options as they stand, then expects the
  prompt), `model-set-model` (the draft form, an id with `/` and `:`, an empty answer), `model-not-offered` (stops
  after `session/new`).
- Test helpers: `fake.rs` `play` also stops the run when the session says `model_unavailable` (no turn will end);
  `fake_acp.rs` has the recording count (11 to 14) and a test for the shapes of the three new recordings.
- `host/tests/adapters/acp/model.rs` (new, 43 tests) and `main.rs` (the module).

**Tests**, in the `acp_adapter` target. None touches the real home, launchd or a daemon: the fake child and the shim's
child have `env_clear()`, then `PATH`, `HOME` and `COLLIDE_DAEMON_ROOT` in a temp root. No settings file is read.

- Seen red: all 43 were written before the code, against a session that ignores `AcpOptions::model`. 34 failed, 9
  passed. The 9 that passed are guards for what the old session already did (no request, no model, prompt at once):
  `a_draft_id_that_is_already_current_writes_no_model_request`,
  `a_later_config_option_update_stays_an_other_and_writes_nothing`,
  `a_request_with_no_model_gives_a_session_that_writes_no_model_request`,
  `a_request_with_no_model_keeps_the_agents_own_even_when_it_lists_a_hosted_one`,
  `a_request_with_no_model_writes_no_model_request_in_any_form`,
  `an_agent_with_no_list_runs_when_no_model_is_asked_for`,
  `an_id_that_is_already_current_writes_no_model_request_and_the_prompt_follows_at_once`,
  `launch_puts_no_part_of_the_model_in_argv_env_or_a_path`,
  `the_id_is_not_in_the_event_that_says_the_model_is_unavailable`.
  The 34 went green with the code. The five that use the real shim failed first on the child (it exited 97 on the
  order, or the run timed out).
- `configOptions`: `an_id_the_config_option_offers_is_set_with_set_config_option_and_no_prompt_is_written_yet`,
  `the_request_names_the_option_by_its_own_id_not_by_the_word_model`, `an_id_in_a_group_of_a_grouped_list_is_offered`,
  `a_grouped_list_counts_every_option_of_every_group`,
  `an_id_that_is_not_offered_writes_nothing_and_is_model_unavailable_with_the_count_offered`,
  `an_answer_that_holds_the_model_as_current_lets_the_prompt_follow`,
  `an_error_in_answer_to_set_config_option_writes_no_prompt_and_fails_the_phase`,
  `an_answer_that_leaves_another_model_current_writes_no_prompt_and_fails_the_phase`,
  `an_answer_with_no_options_is_not_believed`, and the guard for the id that is current (above).
- Draft `models`: `an_id_the_draft_list_offers_is_set_with_set_model_and_no_prompt_is_written_yet`,
  `a_draft_id_that_is_not_offered_writes_nothing_and_is_model_unavailable`,
  `a_hosted_draft_model_is_never_chosen_when_the_request_names_another`,
  `an_empty_success_to_set_model_lets_the_prompt_follow`, `the_meta_a_real_agent_answers_set_model_with_is_a_success`,
  `an_error_in_answer_to_set_model_writes_no_prompt_and_fails_the_phase`,
  `a_current_model_update_after_set_model_is_an_other_and_changes_nothing`.
- Both forms, one, neither: `with_both_forms_in_one_response_config_options_wins`,
  `when_both_forms_are_there_an_id_only_the_draft_list_has_is_not_offered`,
  `config_options_with_no_model_option_fall_back_to_the_draft_list`,
  `an_agent_with_no_list_and_no_way_to_set_one_fails_the_phase_when_a_model_is_asked_for`,
  `a_model_list_that_is_not_a_list_offers_nothing`,
  `a_hosted_model_the_agent_lists_is_never_chosen_when_the_request_names_another`.
- Order and the held prompt:
  `the_prompt_is_held_until_the_answer_and_the_writes_after_session_new_are_model_then_prompt`,
  `a_turn_sent_while_the_model_is_not_set_is_not_written`, `a_turn_sent_after_the_model_was_refused_is_not_written`,
  `a_turn_sent_after_the_model_was_set_and_the_first_turn_ended_is_written`,
  `a_model_that_was_set_gives_an_ended_phase_when_the_turn_ends_well`.
- The id as data: `an_id_with_a_slash_and_a_colon_is_written_as_given_in_either_form`,
  `the_adapter_hands_the_requests_model_id_to_its_session_and_the_name_when_there_is_no_id`.
- Replay: `replayed_from_the_journal_each_form_reaches_the_state_of_the_live_session` (the sessions compare equal, also
  at `finish`), `a_journal_cut_between_the_model_request_and_its_answer_writes_the_prompt_when_the_answer_comes` (the
  prompt written is the byte string the live journal holds).
- Under the real shim, one for each form:
  `under_the_shim_a_child_with_config_options_is_set_through_set_config_option_then_serves_a_turn`,
  `under_the_shim_a_child_with_only_models_is_set_through_set_model_with_a_slash_and_a_colon_in_the_id`,
  `under_the_shim_a_child_that_offers_other_ids_gets_no_request_and_is_stopped_by_the_host` (the journal holds only
  `initialize` and `session/new`; the child exits 0 when the host closes stdin; `finish` says `ModelUnavailable`).
- The shared contract (`acp_session`) passes unchanged. `core/src/testing/acp.rs` is unchanged.
- Checks: `cargo build --workspace`, `cargo test --workspace --no-fail-fast` (2816 passed, none failed),
  `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`, `pnpm test`, `pnpm typecheck`,
  `pnpm --filter @collide/app build`, `pnpm build:packs` and `pnpm check` all pass.

### Decisions for the owner

1. **`PhaseFailure::ModelUnavailable` is not a wire change.** The item took it as one. It is a Rust-only enum, and the
   store keeps a string, so nothing was generated and no TS changed. Recommend keeping the variant as built. If the app
   later wants to word it, the string `model_unavailable` is what it will match.
2. **Three event names, one failure.** A model not offered is `model_unavailable` (the item's name). An answer that is
   an error is `model_set_error`, and an answer that does not show the model set is `model_not_set`; the item named
   none of these two. All three make `finish` say `ModelUnavailable`, as `initialize_error` and `session_new_error`
   share one failure. Alternative: name all three `model_unavailable`. Recommend keeping three, so a person reading the
   events can tell a missing model from an agent that refused or ignored the request.
3. **`model_not_set` also covers an answer with no `configOptions`.** The schema says the answer holds the full
   options; an answer without them cannot show the model current, so it is not believed (S7, O8). Alternative: trust
   an answer with no options. Recommend keeping.
4. **`set_model` is trusted when its answer is not an error.** The item also says a later `current_model_update` or
   the next `session/new` must not contradict it. The notes say those updates stay an `Other`, so nothing reads them,
   and a `session/new` is one per child. Recommend keeping; no check is built for it.
5. **The count of models offered is in `Other.raw`** as `{ "offered": N }`, a number and not agent content. Alternative:
   `raw: None`, as the other `Other`s here have. Recommend keeping, since the item asks for it.
6. **A current id wins over the list.** The order is the item's: no model, then current, then offered. An agent whose
   current model is the one asked for but that lists nothing runs with no request. Recommend keeping.
7. **`send_turn` writes nothing while the model request is out or after it was refused**, and says `turn_not_sent`, as
   it does before the session has an id. Alternative: leave `send_turn` as it was. Recommend keeping: a turn must not
   reach an agent before its model, and a refused phase writes nothing more.
8. **The first model option only.** If `configOptions` holds two options with `category: "model"`, the first with an
   `id` is used. Recommend keeping.
9. **`play` stops on `model_unavailable`.** A test helper change: a refused phase has no turn end. Alternative: a second
   `play`. Recommend keeping.
10. **Plan text, proposed:** none needed. `ts-0092-tw` can say that a refused model arrives as an `Other` followed by a
    `Failed` outcome, and that the daemon stops the child (closes its stdin) on that outcome.

- Owner decisions (2026-11-16, at the build): decisions 1 to 10 taken under the owner's delegation, as recommended.
  Review confirmed decision 1: `PhaseFailure` is not on the wire, the store and `run.record` carry the failure as
  text, and `pnpm gen` gives no diff. The cut after the model answer and before the first prompt is the first-prompt
  window already carried to `ts-0092-tw`.

### Not done, by design

- No empty turn, `Usage`, stderr evidence or lost-child work: `ts-0091-pz`.
- No daemon wiring, and nothing that closes the child after a refused model: `ts-0092-tw`. The session writes nothing
  more and `finish` says why; the host stops the child.
- A journal cut after the model answer was read and before the first prompt was written leaves a rebuilt session with
  the model set and no prompt open. It is the window for the first prompt that `ts-0089-wv` carried to `ts-0092-tw`;
  the model changes nothing about it, and no test pins it here.
- The session does not follow `current_model_update`, `config_option_update` or any later change of model by the
  agent; each is an `Other` (the item's note). Nothing reads a model the agent chose by itself.
- No usage record names the model a turn used: `ts-0091-pz`.
- No real ACP profile or real agent: Stage 6b.
