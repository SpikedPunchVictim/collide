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
