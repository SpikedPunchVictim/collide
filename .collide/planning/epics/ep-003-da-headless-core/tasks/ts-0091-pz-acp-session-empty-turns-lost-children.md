---
id: ts-0091-pz
kind: task
status: active
title: "ACP session: empty turns, lost children and usage quality"
owners: []
links:
  - { type: blocked-by, to: ts-0090-jd }
---

## Goal

The ACP session's outcomes and its usage, as 0.3 and S7 say. A protocol with no failure flag must not let a silent
failure look like success.

**Outcome.** The phase's outcome is the `session/prompt` response (0.3, The stream protocol). `finish` says:

- `Ended`: a response `end_turn` with output or usage, and the child's exit after it;
- `Failed { EmptyTurn }`: `end_turn` with no `Message` text and no usage (S7: a model Ollama lacked gave `end_turn`,
  zero
  usage and no message, and the error text was only on stderr, O4). The session keeps the end of the child's stderr
  (`ExitInfo::stderr_tail`) as evidence, bounded in size;
- `Failed { Refusal }` for `refusal`, `Failed { MaxTokens }` for `max_tokens`, `Failed { MaxTurns }` for
  `max_turn_requests`;
- `Stopped`: the host cancelled and the turn ended `Cancelled` (`ts-0089-wv`);
- `Lost`: the child's stdout ended with no prompt response, whatever the exit code. S7 measured an exit 0 with a
  permission request pending (O7) and that is `Lost`, not `Ended`. The same holds when the child dies in the
  handshake;
- a JSON-RPC error in answer to the prompt: `Failed { Api { status: None } }`, and the error's `code` is kept as
  evidence.

An empty turn needs both "no output" and "no usage": a turn with tool calls and no text, or with usage and no text, is
not empty. The check reads the whole turn, not the last chunk.

**Usage.** A `session/prompt` response may carry `usage` (unstable in ACP; S7 O2). The session gives `Usage` with:

- scope `Turn`, quality `Partial` (the figure is the last model step only: O2) when the profile says so, `Exact` when
  the profile says so, and `Unreported` when the response has no `usage`;
- every figure an `Option`: input, output, cache read, cache write, thought tokens, as the agent reports them. A figure
  the agent does not report is absent, **never 0**;
- no cost (S7: opencode reports none). The host never invents one;
- a turn that ended with no `usage` still gives one `Usage` event with quality `Unreported`, so the phase's record says
  "not reported" and never shows a total that looks like zero.

The quality comes from the profile's `usage_report.quality` (`ts-0088-pk`). A profile that says nothing gives
`Unreported` for all.

The host's run summary already shows `Partial` as "at least" and `Unreported` as "not reported" (`host/src/run_cli.rs`,
`call_text` and the totals); no change is expected there. A test shows it for an ACP phase.

## Success criteria

- An `end_turn` with no message, no tool call and no usage gives `Failed { EmptyTurn }`, and the stderr text the shim
  captured is in the record's evidence (decision below).
- An `end_turn` with usage and no message, one with a tool call and no message, and one with a message and no usage, are
  each `Ended`.
- `refusal`, `max_tokens` and `max_turn_requests` give their causes.
- A child that exits 0 before any prompt response is `Lost`; so is one that exits non-zero, one that is killed and one
  that exits in the handshake.
- A cancelled turn is `Stopped` even when the agent said `end_turn` (S7, O1).
- A response with `usage` gives a `Usage` event with quality `Partial` and only the figures it carried. No figure is
  ever 0 unless the agent said 0.
- A response with no `usage` gives quality `Unreported`, every figure absent and no cost.
- Over a daemon run, `collide run` shows an ACP phase's usage as "at least" or "not reported" and never as `0`.
- The evidence kept is at most 2 KiB, from the end of stderr, on a character boundary.
- Replay of a journal for each outcome gives the same outcome as the live session.
- The shared contract still passes.
- No new crate is added.

## Tests

Write these first:

- `finish` for each outcome above, from the bytes of a turn: empty; usage only; tool call only; message only; refusal;
  max tokens; max turn requests; error response; cancelled with `end_turn`; no response with exit 0, exit 1 and a
  signal;
  an exit in the handshake;
- usage: `partial` with each figure present or absent; `unreported`; a figure of 0 sent by the agent stays 0 and an
  absent one stays absent; no cost field;
- the evidence: shorter than the bound, longer, cut inside a multi-byte character;
- replay of each of the journals above;
- the fake child under the real shim: a recording that answers `end_turn` and writes its error on stderr (the record has
  the text and `empty_turn`), one that exits after the permission request with the answer pending (`lost`), one with
  usage and one without;
- the run summary text for an ACP phase with `Partial` and with `Unreported` usage (`host/src/run_cli.rs`).

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 3d: ACP engine (`empty_turn`, `lost`, `Usage.quality`, `TurnEnd` reasons).
  - 0.3: The stream protocol (an adapter whose protocol has no failure flag fails an empty turn; a child that exits
    before
    its prompt response is `lost`); Usage (quality, `Option`, never 0); Normalized events (`TurnEnd`); Two core traits
    (`ExitInfo`, `Finished`).
  - 0.13: Usage totals ("at least", never summed with `exact`).
  - S7 record: P2, P9 and G1, G2, O1, O2, O3, O4, O7.
- **Decision for the owner, with a recommendation: where the evidence goes.** 0.3 says the record keeps the agent's
  stderr
  tail. `PhaseRecord` (`host/src/store.rs`) has `usage`, `failure` and `report`, and no place for it. Options:
  1. a new optional `failure_evidence` text on the record, with a store migration. Pro: the evidence is in the run's
     own record and shown with the failure. Con: a migration, and stderr may hold text the person did not expect to be
     kept in the store.
  2. the evidence stays in the phase's stderr log file, which the shim already keeps, and the record holds the failure
     name only. Pro: no migration, and the text stays where the person already finds logs. Con: the plan's wording
     ("the record keeps") needs a change.
  3. an event on the stream. Con: events hold no content by rule.
  Recommend 1, capped at 2 KiB, as 0.3 says; it follows the plan as written. Option 2 needs a plan change.
- **Decision for the item, with a recommendation: an empty turn with a pending request.** A turn cancelled by the host
  with no output is `Stopped`, not `EmptyTurn`. Cancel wins, as in `ts-0089-wv`.
- **Decision for the item: thought tokens.** ACP usage has `thoughtTokens`. `Usage` has no field for it. Recommend that
  the session adds it to output only if the profile says output includes it, and otherwise drops it, since an
  unmeasured sum is worse than an absent one. Say so in the session's doc.
- `Usage.quality` for an ACP agent is the profile's claim, not the session's measurement. S7 measured one agent only;
  other profiles later set their own.
- Run `pnpm gen` if the record or an event type changes.
- Area: `host` (`adapters/acp/session.rs`, `usage.rs`, `store.rs` if the evidence field is taken, `agent_feed.rs`),
  the test folder.
- The test rule: no test touches the real home, real launchd or a real daemon. Each test clears the environment
  (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as `host/tests/common/mod.rs` `TempRoot`
  does. Settings files are fixtures, never the owner's own `settings.json`.
- Stage 3d item (revision 23 scope, split 2026-11-15). Items of Stage 3d run one at a time, in id order, because
  they share `host` and the generated files.
- Owner decisions (2026-11-15, at the split): the decisions for the owner above, taken as recommended under the
  owner's delegation; the owner may change them before the item is built. The ACP protocol is written by hand: no ACP
  crate is added.

### Built

The ACP session now judges how a phase ended, and reports what a turn spent. No crate or dependency was added. A wire
type changed (`RunRecordResult` gained `failure_evidence`); `pnpm gen` was run and its output kept.

**Outcome.** `AcpSession::finish` follows the table in the session's module doc:

- `Lost`: stdout ended with no response to the open prompt, whatever the exit (code 0 with a request pending, code 1,
  a signal, a death in the handshake, a shim with no exit record). A prompt written starts a new turn, so a second turn
  with no response is `Lost` and not the first turn's outcome (`last_turn` is cleared in `note_written`).
- `Stopped`: the host cancelled the turn, even when the agent said `end_turn` with nothing.
- `Failed { EmptyTurn }`: `end_turn` with no message text, no tool call, no permission request and no usage figure
  above 0 over the whole turn. The evidence is the end of the child's stderr.
- `Refusal`, `MaxTokens`, `MaxTurns` keep their causes even when the turn is empty. An error response and an unknown
  stop reason are `Api { status: None }`; the error response keeps its code as evidence.
- Evidence: `core/src/adapter.rs`, `Finished::evidence`, `EVIDENCE_BYTES` (2048) and `evidence_tail` (trim, keep the
  end, start on a char boundary, `None` when empty).

**Usage.** `read_usage` in `host/src/adapters/acp/session.rs`. One `Usage` event per prompt response, scope `Turn`,
before the final report and the turn end. Quality is the profile's claim (`AcpOptions::usage_quality`, from
`profile.usage_report.quality`). Every figure is an `Option`: only a whole non-negative number the agent sent is kept.
No cost, model time or context figure. A response with no `usage`, one with no figure the session reads, and a profile
that claims nothing give `Unreported` with every figure absent. `AcpProfile::output_includes_thoughts` (new, default
false) decides whether `thoughtTokens` goes to `Usage.thought`.

**The record.**
- `host/src/store.rs`: migration 7 `phase_record_failure_evidence` (`ALTER TABLE phase_records ADD COLUMN
  failure_evidence TEXT`), `PhaseRecord::failure_evidence`, and the read and write SQL.
- `host/src/agent_feed.rs`, `AgentFeed::settle`: copies `Finished::evidence` into the record.
- `core/src/owner/run.rs`, `RunRecordResult::failure_evidence` (optional, skipped when none); `host/src/daemon.rs`,
  `run_record` fills it. Regenerated: `schemas/RunRecordResult.{input,output}.json` and
  `packages/pack-sdk/src/generated/RunRecordResult.{input,output}.ts`. No TS code reads the field.
- `host/src/run_cli.rs`, `record_text`: prints `evidence:` after `failure:`, each line indented, control characters
  dropped (the text is the agent's). The "at least" and "not reported" figures needed no change; tests show them for
  an ACP phase.

**Harness.** `fake_acp.py` has a `{"signal": N}` step; `play_until_exit` (new, in `host/tests/adapters/acp/fake.rs`)
stops after N lines; six new recordings: `empty-turn`, `prompt-turn-no-usage`, `lost-request-pending`, `lost-exit-1`,
`lost-signaled`, `lost-in-handshake`. `session.rs` (test helper) has `ready_with(options)`.

**Tests, written first.** Red before the code, seen by running them:

- `host/tests/adapters/acp/outcome.rs` (new; run as `cargo test -p collide-host --test acp_adapter`). Red, among
  them:
  `an_end_turn_with_no_words_no_tool_call_and_no_usage_is_an_empty_turn`,
  `an_empty_turn_keeps_the_end_of_the_childs_stderr_as_its_evidence`, `an_empty_turn_with_no_stderr_has_no_evidence`,
  `a_chunk_with_no_text_is_not_output`, `a_usage_of_zeros_with_no_output_is_still_an_empty_turn`,
  `the_next_turn_starts_with_no_output`,
  `an_error_response_is_an_api_failure_that_keeps_the_errors_code_and_nothing_else`,
  `a_second_turn_with_no_response_is_lost_and_not_the_first_turns_outcome`, the three `evidence_*` bound tests, the two
  replay tests, the shim test for an empty turn with its stderr, the journal-replay test of an empty turn, and the
  three `record::` tests (stderr text in the record, long stderr bounded, usage never 0). Green from the start, as the
  old code already did it: the `Lost` cases (exit 0, 1, signal, handshake, request pending), `Stopped`, and the
  refusal, max-tokens and max-turn-requests causes. They pin the behavior now.
- `host/tests/adapters/acp/usage.rs` (new). Red: every test that reads a figure or an event (19 of its 22), including
  `the_adapters_session_has_the_profiles_usage_quality`, `..._thought_rule`,
  `thought_tokens_are_dropped_unless_the_profile_says_the_output_includes_them`,
  `a_response_with_no_usage_gives_an_unreported_event_with_every_figure_absent`,
  `a_profile_that_says_nothing_gives_unreported_for_all_and_drops_the_figures`,
  `a_figure_that_is_not_a_whole_non_negative_number_is_absent`, `each_turn_gives_its_own_usage_event`,
  `the_usage_comes_before_the_final_report_and_the_turn_end_and_has_the_lines_time`, and the two recordings under the
  shim.
- `core/tests/evidence_tail.rs` (new, 5 tests): 4 red, `nothing_but_white_space_is_no_evidence` green on the stub.
- `host/src/store.rs`: `a_store_at_version_six_gains_the_failure_evidence_column_and_keeps_its_records` and the
  round-trip test red; `a_phase_record_without_evidence_reads_back_without_it` guards the old shape.
- `host/src/run_cli.rs`: `an_acp_phase_with_partial_usage_prints_at_least_and_never_a_zero_for_what_it_lacks`,
  `an_acp_phase_with_no_usage_prints_not_reported_in_every_figure_and_never_a_zero`,
  `a_failure_with_evidence_prints_the_evidence_after_it_and_a_failure_without_prints_none` and
  `evidence_is_untrusted_so_its_control_characters_are_not_printed`: red on the missing field and the missing session
  usage.
- `fake_acp.rs`: `a_signal_step_ends_it_by_that_signal_with_no_exit_code`; the recording count is 20.

**Existing tests changed** because the semantics changed (a `Usage` event now comes at every turn end, and a turn with
nothing is empty): `turn.rs` (stop reasons and error turns give `empty: true`; the events list holds `Usage`),
`permission.rs` (the event lists; the "events are the same" test compares with `empty` set, since an ask is output),
`model.rs` (two turns that ended with no words now feed a message first, or they are `EmptyTurn`).

**Checks** (each run in the foreground, after the last edit): `cargo build --workspace`; `cargo test --workspace
--no-fail-fast` (2890 passed, 0 failed); `cargo clippy --workspace --all-targets -- -D warnings`;
`cargo fmt --all --check`; `pnpm test`; `pnpm typecheck`; `pnpm --filter @collide/app build`; `pnpm build:packs`;
`pnpm check`. All pass. Code and comment lines in the files written are 120 columns or fewer.

### Decisions for the owner

1. **The evidence column (the item's decision).** Taken as recommended: option 1, an optional `failure_evidence` on the
   record, with migration 7, capped at 2 KiB. Alternative: keep it in the stderr log only (needs a plan change).
   Recommend keeping. The text is the child's stderr and may hold what the person did not expect to be stored.
2. **Thought tokens.** The item's note says `Usage` has no field for them; it has `thought`. So with
   `output_includes_thoughts` true the session sets `thought`, and `output` stays as the agent reported it (nothing is
   added). With it false the figure is dropped. Alternative: add it to `output` as the note says. Recommend keeping:
   a sum the agent did not report is worse than a separate figure.
3. **An `Unreported` claim drops all figures.** A profile claim of `Unreported` (or none) gives an event with no
   figure, even if the agent sent some. A figure under `Unreported` would print as exact in `count_text`. Alternative:
   keep the figures under `Unreported`. Recommend keeping.
4. **Zeros count as no usage for "empty".** A usage whose figures are all 0 with no output is an empty turn, as S7 O4
   measured. A usage with any figure above 0 (thought tokens too, whatever the profile does with them) is not.
   Alternative: any `usage` object at all means not empty; that would hide the opencode missing-model case. Recommend
   keeping.
5. **A permission request counts as a tool call.** The agent may ask before announcing the call (S7, O6). Alternative:
   count only `tool_call` updates. Recommend keeping; a turn that asked for a tool did something.
6. **An error response keeps its code only.** The evidence is `JSON-RPC error code <n>`; the message is the agent's
   free text and is not kept. A handshake error keeps no evidence at all. Alternative: keep the message, bounded.
   Recommend keeping, as events hold no content by rule.
7. **A second turn with no response is `Lost`.** A prompt written clears the last turn's end, so the first turn's
   outcome never stands for a second one. Alternative: keep the last response. Recommend keeping.
8. **A cancelled turn with no output has `empty: true` on its `TurnEnd`** and is `Stopped` (cancel wins). The flag
   says only what the turn gave. Alternative: force `false` on cancel. Recommend keeping.
9. **The usage event does not name the model** (carried from `ts-0090-jd`). No `Usage` field holds one. Alternative: a
   new field. Recommend leaving it until a record needs it.
10. **Plan text, proposed (0.3, Usage):** "an ACP turn's usage is `Unreported` with every figure absent when the agent
    sends none or the profile claims none; a figure never shows as 0 unless the agent said 0." And (The stream
    protocol): "the evidence kept with a failed phase is at most 2 KiB, from the end of stderr, or the code of a
    JSON-RPC error."

- Owner decisions (2026-11-16, at the build): decisions 1 to 9 taken under the owner's delegation, as recommended.
  Decision 10's plan text applied to 0.3 (the Usage bullet and the empty-turn bullet). The app shows no evidence yet;
  `collide run` prints it, with control characters dropped.

### Not done, by design

- No daemon wiring, no owed writes after a journal cut, and nothing that closes the child's stdin after a turn:
  `ts-0092-tw`.
- No real ACP profile and no real agent; no profile sets `usage_report.quality` or `output_includes_thoughts` yet:
  Stage 6b.
- No display of `failure_evidence` in the app: no TS reads `RunRecordResult.failure` yet either. The generated types
  carry the field.
- No cost, model time, context or per-model usage figure for an ACP turn.
- A handshake that fails keeps no evidence, though stderr may say why.
