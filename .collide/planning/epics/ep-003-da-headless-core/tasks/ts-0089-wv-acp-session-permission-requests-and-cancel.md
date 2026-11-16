---
id: ts-0089-wv
kind: task
status: done
title: "ACP session: permission requests and cancel"
owners: []
links:
  - { type: blocked-by, to: ts-0088-pk }
---

## Goal

The ACP session's permission and cancel paths, made as 0.3 says and as S7 measured. `ts-0087-mf` left them as the
double had them.

**Permission request.** The agent sends `session/request_permission` with its own request id, a `toolCall` and a list of
options. The session:

- gives `PermissionRequest` with the id the agent used (as text), the `toolCallId`, the tool's `kind`, and the options
  as the agent named them (id, name, kind). It reads the kind from the request's `toolCall`, and when that has none,
  from
  a `tool_call` it saw earlier with the same id. When it has neither, the kind is `other`. S7 measured that the request
  comes before the `tool_call` (O6) and that `rawInput` can be `{}` for bash (O5): `input` is `None` then;
- holds the request as pending, with the JSON-RPC id exactly as the agent wrote it (a number stays a number), so the
  answer carries the same id;
- answers on `answer(id, reply)`:
  - `Allow` picks the option of kind `allow_once`. **Never `allow_always`**, not even when it is the only allow option;
  - `Deny` picks `reject_once`. The message of a deny is dropped: ACP has no place for it (S7, G5);
  - `Cancel` writes the outcome `cancelled`;
  - an allow with no `allow_once` option writes `cancelled`, gives `PermissionUnanswerable` and nothing else, so the
    host can tell the person why (0.3, Permissions);
  - an id the session does not hold writes nothing;
- keeps the request in `pending_permissions` until it is answered, also after a replay: a request the agent asked
  before a restart and nobody answered is returned, oldest first, and a request the journal shows answered is not.

**Cancel.** `cancel` writes `session/cancel` for the session id, a notification with no id, and marks the turn as
cancelled by the host. It answers each pending request with `cancelled` first, as ACP requires. The turn ends when the
`session/prompt` response comes, and the session reports `TurnEnd { Cancelled }` whatever stop reason the agent gave: S7
measured that opencode answers `end_turn` (O1). A cancel before the session has an id, or after the turn ended, writes
nothing.

## Success criteria

- `Allow` writes the `allow_once` option's id; with `allow_once` and `allow_always` both present, the first is chosen
  and the second never is. `Deny` writes `reject_once`.
- A request with only `allow_always` and `reject_always`, answered with `Allow`, writes `cancelled` and gives
  `PermissionUnanswerable`; the same with `Deny` is also `cancelled`.
- A deny message appears nowhere in the bytes written.
- The answer's id is the agent's id, a number stays a number and a string stays a string.
- An id the session does not hold, and an id answered twice, write nothing.
- The request's `kind` comes from the request; when absent, from the earlier `tool_call`; else `other`. A request that
  comes before its `tool_call` is not an error, and a `tool_call` after it changes nothing already given.
- After a replay of a journal that holds one asked and one answered request, `pending_permissions` is the asked one
  alone, with its options.
- `cancel` writes one `session/cancel` with no `id`, after one `cancelled` outcome for each pending request. A second
  `cancel` writes nothing.
- A turn cancelled by the host ends `Cancelled` when the agent says `end_turn` (S7, O1), `cancelled`, or `refusal`.
  A turn the host did not cancel ends as the agent says.
- Under the real shim, a permission request answered `Allow` lets the child go on to the end of its turn, and a cancel
  mid-turn ends it `Cancelled`.
- No new crate is added.

## Tests

Write these first:

- the shared contract's permission and cancel scenarios (already run for the session by `ts-0087-mf`) stay green;
- `Allow` with each combination of options (`allow_once` alone; both allows; `allow_always` alone; no allow);
- `Deny` with `reject_once`, with `reject_always` only, and with a message that must not be written;
- the id kept as number and as string;
- an unknown id and a repeated id;
- the kind from the request, from the earlier `tool_call`, and defaulted; the request before its `tool_call`;
- `pending_permissions` live, after an answer, and after a replay with and without the answer in the journal;
- `cancel` with two pending requests, with none, twice, and before the session id;
- a cancelled turn that the agent ends `end_turn`, `cancelled` and `refusal`; the same stop reasons without a cancel;
- the fake child under the real shim: a recording that asks for permission and waits for the answer (`$id` in the
  `expect`), and one that waits for `session/cancel` and then answers the prompt `end_turn`.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 3d: ACP engine (permission request, `session/cancel`).
  - 0.3: Permissions (who answers; `allow_once` only; `PermissionUnanswerable`; a deny message is dropped); Normalized
    events; Two core traits (`pending_permissions`, `cancel`); The stream protocol.
  - 0.15: Dependency injection.
  - S7 record: P4, P7 and G5, G6, O1, O5, O6.
- **Decision for the owner, with a recommendation: `pending_permissions` for ACP.** The trait's doc says an ACP session
  "answers inside its own turn" and so holds none. That is wrong across a restart: the agent is a child of the shim, it
  outlives the daemon, and its request stays open. Recommend that the ACP session returns its pending requests, rebuilt
  from the journal like Claude's, and that the trait's doc is corrected to say so (a doc line in `core/src/adapter.rs`;
  no change to the signature). Alternative: leave the default and let a restart cancel the turn. Pro: no code. Con: a
  person's pending approval is lost on every daemon restart, which is what the Claude path was built to avoid.
- **Decision for the item, with a recommendation: a request with no usable option.** Recommend `cancelled` plus
  `PermissionUnanswerable`, as 0.3 says; the host then shows the person that the agent offered no one-time allow.
  Never pick `allow_always`: it changes the agent's own standing rules, outside Collide's reach.
- `PermissionRequest` carries no paths (`core/src/agent.rs`). ACP's `toolCall.locations` has them. Stage 6b's ask box
  does not need them. Leave them out, and say so in the session's doc.
- A request from the agent for a tool the host's `permission_host` allow-list already allows is not seen here: that is
  the daemon's job above the session (0.3, Permissions).
- The same-id rule: ACP request ids are numbers or strings. Keep the agent's value as a `serde_json::Value` inside the
  pending list and give the event the text form.
- Run `pnpm gen` if a wire type changes. None is expected.
- Area: `host` (`adapters/acp/session.rs`, `permissions.rs`), `core/src/adapter.rs` (a doc line), the test folder.
- The test rule: no test touches the real home, real launchd or a real daemon. Each test clears the environment
  (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as `host/tests/common/mod.rs` `TempRoot`
  does. Settings files are fixtures, never the owner's own `settings.json`.
- Stage 3d item (revision 23 scope, split 2026-11-15). Items of Stage 3d run one at a time, in id order, because
  they share `host` and the generated files.
- Owner decisions (2026-11-15, at the split): the decisions for the owner above, taken as recommended under the
  owner's delegation; the owner may change them before the item is built. The ACP protocol is written by hand: no ACP
  crate is added.

### Built

The ACP session's permission and cancel paths, hardened, with their tests. No wire type changed, so `pnpm gen` had
nothing to regenerate. No crate was added. `ts-0087-mf` had already carried the double's `answer` (allow-once,
reject-once, `cancelled`, `PermissionUnanswerable`, the agent's raw id) and the `Cancelled` turn end; this item added
what was missing and pinned all of it with tests.

- `host/src/adapters/acp/session.rs`, `AcpSession`:
  - `HeldRequest` now holds the whole `PermissionRequest` beside the agent's raw id, so the request can be given back.
  - `pending_permissions` (new): the held requests, oldest first. A replay rebuilds them with no new code path: a
    request in an `out` record is held by `read`, and an answer in an `in` record removes it in `note_written`.
  - `tool_kinds` (new field) and `request_kind`: the kind of a request is its own `toolCall.kind`, else the kind of the
    `tool_call` announced earlier in the turn with the same id, else `other`. The list is cleared when a new
    `session/prompt` is written, so a replay clears it too. A `tool_call` after the request changes nothing given.
  - `cancel`: writes nothing before the session has an id, after the turn ended (no `session/prompt` is waiting), or
    when the turn is cancelled already. Otherwise it writes the outcome `cancelled` for each pending request, then
    `session/cancel`. Before, `session/cancel` came first and a second `cancel` wrote again.
  - `end_turn` (read of the `session/prompt` response): each request still held is answered `cancelled` and dropped,
    before the `TurnEnd` is made. `pending_permissions` is empty after it.
  - `read_request` (new): a `session/request_permission` read after `cancel`, while the turn is open, is answered
    `cancelled` at once and never held. Its event is an `Other` named `permission_request_after_cancel`, not a
    `PermissionRequest`, so the daemon puts nothing in front of the person.
  - `Reading` now carries a list of writes (it carried one), so a read can answer several requests.
  - The module doc: a section for answering, one for cancel, one for the turn end, one for the kind;
    `pending_permissions` is no longer left to a later item.
- `core/src/adapter.rs`: the doc of `AgentSession::pending_permissions` no longer says an ACP session holds none. It
  says why both engines do. No signature changed.
- `host/tests/adapters/acp/fixtures/recordings/` (new): `permission-deny`, `permission-unanswerable`,
  `permission-cancel` (a string request id; expects the `cancelled` answer, then `session/cancel`), `cancel-mid-turn`
  (waits for the notification, then answers `end_turn`), `permission-open-at-turn-end` (ends its turn with a request
  open, then expects the `cancelled` answer) and `permission-after-cancel` (asks after `session/cancel` and waits for
  the answer before it ends its turn). Each is cut to the handshake lines the session reads, and says
  in its first comment what it follows.
- `host/tests/adapters/acp/permission.rs` (new, 63 tests), `main.rs` (the module), `fake_acp.rs` (the count of
  recordings, 5 to 11), `turn.rs` (the test `a_turn_the_host_cancelled_ends_cancelled_whatever_the_agent_says` moved to
  `permission.rs`, where it covers three stop reasons).

**Tests**, in the `acp_adapter` target, none touching the real home, launchd or a daemon (the child and the shim's
child have `env_clear()`, then `PATH`, `HOME` and `COLLIDE_DAEMON_ROOT` in a temp root; no settings file).

- Seen red: the 48 tests were written before the code; 13 failed and 35 passed against the session as `ts-0087-mf`
  left it (the 35 are the `answer` paths it already had). The 13 that failed, and went green with the code:
  `a_request_is_pending_with_its_options_until_it_is_answered`,
  `pending_requests_are_returned_oldest_first_and_an_answer_takes_one_out`,
  `an_id_the_session_does_not_hold_writes_nothing` (its check that the other request is still held),
  `a_request_with_no_kind_takes_it_from_the_earlier_tool_call_with_the_same_id`,
  `a_tool_call_after_the_request_changes_nothing_already_given`,
  `after_a_replay_that_has_no_answer_in_the_journal_both_requests_are_pending`,
  `after_a_replay_a_request_the_journal_shows_answered_is_not_pending`,
  `cancel_answers_each_pending_request_cancelled_and_then_writes_session_cancel`,
  `a_second_cancel_writes_nothing`, `cancel_after_the_turn_ended_writes_nothing`,
  `a_cancel_with_a_request_pending_answers_it_first_and_the_turn_ends_cancelled` (the child exited 97 on the order),
  `a_journal_cut_while_the_request_waits_has_it_pending_and_the_whole_one_has_not`.
- Added after review (turn end clears what is held; a request after a cancel is answered at once), seen red against
  the session as first built, 13 of 16 failing; the other three held already
  (`the_request_is_dropped_before_the_turn_end_event_and_the_events_are_the_same`,
  `a_turn_end_after_a_cancel_writes_nothing_because_cancel_answered_the_requests`,
  `a_request_asked_in_the_turn_after_a_cancelled_one_is_held`), and are kept as guards. The 13:
  `a_turn_end_answers_the_request_still_held_cancelled_and_drops_it`,
  `a_turn_end_answers_two_held_requests_oldest_first_with_their_own_ids`,
  `a_turn_end_by_any_response_clears_what_is_held`,
  `a_request_cleared_by_a_turn_end_is_not_asked_again_by_the_next_turn`,
  `a_request_asked_after_a_cancel_is_answered_cancelled_at_once_and_never_held`,
  `a_request_after_a_cancel_keeps_the_agents_id_as_a_string`,
  `a_request_asked_before_a_cancel_and_one_after_it_are_each_answered_once`,
  `a_journal_cut_after_the_prompt_response_has_nothing_pending_and_the_session_is_the_live_one`,
  `a_journal_that_holds_the_answers_of_a_turn_end_replays_to_the_same_state_and_answers_nothing_more`,
  `a_request_read_after_a_cancel_is_answered_once_in_the_journal_and_a_replay_adds_no_second_answer`,
  `a_journal_cut_before_the_answer_to_a_request_after_a_cancel_leaves_nothing_pending`, and under the real shim
  `a_turn_that_ends_with_a_request_open_answers_it_cancelled_and_leaves_nothing_pending` and
  `a_request_read_after_a_cancel_is_answered_at_once_and_the_turn_ends_cancelled`.
- Allow: `allow_picks_the_allow_once_option_when_it_is_the_only_allow`,
  `allow_picks_allow_once_and_never_allow_always_when_both_are_offered`,
  `allow_with_only_allow_always_is_cancelled_and_unanswerable`,
  `allow_with_no_allow_option_at_all_is_cancelled_and_unanswerable`,
  `an_option_of_a_kind_this_version_does_not_know_is_not_offered_and_never_picked`.
- Deny and cancel replies: `deny_picks_reject_once`, `deny_with_only_reject_always_is_cancelled_and_not_unanswerable`,
  `a_deny_message_appears_nowhere_in_the_bytes_written`, `a_cancel_reply_writes_the_outcome_cancelled`.
- The id: `a_number_id_stays_a_number_and_gives_the_event_its_text`, `a_string_id_stays_a_string`,
  `a_string_id_that_looks_like_a_number_is_answered_as_the_string_the_agent_wrote`,
  `an_id_the_session_does_not_hold_writes_nothing`, `an_id_answered_twice_writes_nothing_the_second_time`.
- The kind: `the_kind_comes_from_the_request_when_it_has_one`,
  `the_kind_of_the_request_wins_over_the_earlier_tool_call`,
  `a_request_with_no_kind_and_no_earlier_tool_call_is_of_kind_other`,
  `a_request_with_a_kind_this_version_does_not_know_is_of_kind_other`,
  `the_tool_calls_of_an_earlier_turn_are_not_looked_in`, `a_request_with_no_input_sent_has_none`; the request before
  its `tool_call` is `turn.rs`' `a_permission_request_before_its_tool_call_is_not_an_error`, kept.
- `pending_permissions`: `a_session_that_was_asked_nothing_holds_no_request`,
  `an_unanswerable_request_is_no_longer_pending`,
  `a_request_the_journal_shows_cancelled_is_not_pending_after_a_replay`,
  `a_rebuilt_session_answers_a_request_as_the_live_one_does_with_the_agents_own_id`,
  `a_rebuilt_session_cancels_as_the_live_one_does`.
- Cancel: `session_cancel_is_a_notification_with_no_id_for_the_session_id`,
  `cancel_with_no_pending_request_writes_session_cancel_alone`, `cancel_before_the_session_has_an_id_writes_nothing`,
  `cancel_cancels_the_turn_a_later_prompt_opened_and_a_turn_after_it_is_not_cancelled`,
  `a_turn_the_host_cancelled_ends_cancelled_whatever_the_agent_says` (`end_turn`, `cancelled`, `refusal`),
  `a_turn_the_host_did_not_cancel_ends_as_the_agent_says`,
  `a_cancel_that_arrives_when_the_agent_has_already_answered_the_prompt_is_not_a_cancel`.
- Under the real shim: `a_request_answered_allow_lets_the_child_go_on_to_the_end_of_its_turn`,
  `a_request_answered_deny_writes_the_reject_option_and_no_message`,
  `a_request_with_no_one_time_allow_is_answered_cancelled_and_reported_unanswerable`,
  `a_cancel_mid_turn_writes_session_cancel_and_the_turn_ends_cancelled`.
- The shared contract (`acp_session::*`, 19 scenarios) stays green, and so do the replay tests of `replay.rs`.
- Checks: `cargo build --workspace`, `cargo test --workspace --no-fail-fast` (2772 passed, none failed; 2680 before),
  `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`, `pnpm test`, `pnpm typecheck`,
  `pnpm --filter @collide/app build`, `pnpm build:packs` and `pnpm check` all pass.

### Decisions for the owner

1. **A turn end answers each held request `cancelled` and drops it.** Built so, by the rule that an agent is never left
   waiting on Collide (as for the `-32601` answer). The answers are written when the `session/prompt` response is read,
   before the `TurnEnd` is made; `pending_permissions` is empty after, and a later `answer` is for an unknown id and
   writes nothing. A replay drops the same requests and writes nothing, so a journal cut after the response has none
   pending. Alternative (the first build): keep a request until it is answered, even after its turn ended; a stale ask
   would come back after a restart. Recommend keeping what is built. For `ts-0092-tw`: the queue item of a request
   that a turn end cleared is dropped by the daemon's reconcile with `pending_permissions`, since no event says it was
   answered.
2. **A request read after a cancel, in the open turn, is answered `cancelled` at once and never held.** The read writes,
   as `-32601` does. Its event is an `Other` named `permission_request_after_cancel`, not a `PermissionRequest`: the
   daemon asks the person on a `PermissionRequest` event, and this request is answered already. A second `cancel` still
   writes nothing. Alternative (the first build): hold it and leave it to the person; the agent may wait on it and the
   cancel would hang. Recommend keeping what is built. Open: a journal cut between that request and its answer leaves
   a rebuilt session that cannot know it owes the write, as for an unserved request today; the agent then waits on a
   cancelled turn. It is the window `ts-0092-tw` already has for the first prompt, and is carried there.
3. **`cancel` writes the answers first, then `session/cancel`.** The item's order, and the opposite of what 0087 had.
   ACP's text says the client answers pending requests after it sends the cancel; the item and the success criteria
   say first. Recommend keeping the item's order, which the real-shim test pins: the child sees the same outcome
   either way, and the order makes the notification the last write.
4. **The kind is looked up per turn.** `tool_kinds` is cleared when a new prompt is written. A request in one turn is
   about a call of that turn, and the list stays small. Alternative: keep every call of the session. Recommend keeping.
5. **An option of a kind this version does not know is not offered** (as `ts-0087-mf` did). An agent that offers only
   such options gets `cancelled` and `PermissionUnanswerable` on an `Allow`. Recommend keeping.
6. **The double in `core` has no `pending_permissions`.** It stays the test double of `core`, so the shared contract has
   no scenario for it (Claude's is checked by its own tests, ACP's by `permission.rs`). Recommend a contract scenario
   when the double gets the method, which is a change to `core` that this item does not need.
7. **The recordings' long lines.** A JSON Lines step is one line, so a request or an expectation is over 120 columns,
   as the earlier recordings are. The comments and the handshake lines are cut to fit where they could be.
8. **Plan text, proposed:** none needed. `ts-0092-tw` can say that `pending_permissions` of an ACP feed is what puts a
   request back in the Queue after a restart.

- The item's own decisions (`pending_permissions` rebuilt from the journal, the trait doc fixed, `cancelled` plus
  `PermissionUnanswerable` for no usable option) are built as written.

- Owner decisions (2026-11-16, at the build): decisions taken under the owner's delegation, as recommended. Decisions
  1 and 2 were flipped in review so that an agent is never left waiting on Collide (the rule of the `-32601` answer):
  a turn end answers each held request `cancelled` and drops it, and a request read after a cancel is answered
  `cancelled` at once. That request's event is an `Other` (`permission_request_after_cancel`), kept, so the Queue
  never asks about an answered request. The cut between such a request and its answer is carried to `ts-0092-tw` (a
  note there).

### Not done, by design

- No model, `session/set_config_option`, empty turn, `Usage`, stderr evidence or lost-child work: `ts-0090-jd` and
  `ts-0091-pz`.
- No daemon wiring: `AgentFeed::start`, the first prompt, the rebuilder, the queue and the probe are `ts-0092-tw`.
- No `locations` in `PermissionRequest` (the type has no paths); the session's doc says so.
- No recording that uses `$id` inside an `expect` for the agent's request id: the child remembers only the ids of the
  host's requests, and the agent's own id is a literal in a recording. The id is checked by the number and string
  recordings (`permission-request`, `permission-cancel`).
- `core/src/testing/acp.rs` is unchanged.
