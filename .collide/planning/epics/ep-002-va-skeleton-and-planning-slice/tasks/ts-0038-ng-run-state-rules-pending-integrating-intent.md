---
id: ts-0038-ng
kind: task
status: active
title: "Run-state rules: pending, integrating, intent and outcome"
owners: []
links:
  - { type: blocked-by, to: ts-0027-ar }
  - { type: blocked-by, to: ts-0037-vg }
  - { type: blocked-by, to: ts-0018-dd }
---

## Goal

Three small pure functions in `core`, with fixtures, so the Stage 2 contract rules have tests before any engine exists (the real state machines are Stage 3a, the engine Stage 8). One: recording a hook result into run state, where a `pending` result replaces the previous entry for the same `pack/hook`, phase, round and instance, the next poll is the larger of `retryAfter` and the phase's `interval:` (never under 1 s, and the interval when there is no hint), and a `wait` that reaches its `timeout:` (an uptime reading passed in) while still pending ends as `error` with the last message. Two: handling an `integrate` answer, where `merged {commit}` goes on, and `pending {url, head}` puts the run in `integrating` and holds back the source's ops and `run.after_integrate` until `merged`. Three: recovery from the event log, where an intent with no outcome yields a "check the world" action for that step and never a retry, and an inconclusive check yields an "outcome unknown" Queue item.

## Success criteria

- A `wait` hook answering `pending` three times leaves one run-state entry. The last non-pending result is kept.
- An `integrate` of `pending` leaves the run `integrating` and holds the source's ops and `run.after_integrate` back. A later `merged` releases them.
- A `pending` from a hook that is neither `wait` nor a PR `integrate` is an `error`.
- An intent with no outcome yields a check, never a repeat. A merge that finds its change already at the target is a success, not a conflict.
- A `wait` at its timeout while pending is `error` with the last message.

## Tests

Write first: the three-pending fixture; the `integrating` hold and release; the `retryAfter` against `interval:` table; the intent-without-outcome log; the inconclusive-check log.

## Notes

- IMPLEMENTATION_PLAN.md, 0.5 (the `pending` result), 0.4 (`integrating` and `pending`; the lifecycle), 0.2 (intent and outcome; the clock table, for the uptime clock), 0.7 (the `wait` phase), Stage 2: Skeleton, Success Criteria (the new contract rules) and Tests (the `pending` poll, the `integrating` state, the intent and outcome events).
- Area: `core` (pure functions and fixtures).
- Done when the criteria hold. The SIGKILL-after-intent tests belong to Stages 7 and 8.
- Built: `core/src/run_rules.rs` holds the three pure functions, with no clock and no IO; the uptime reading is a `Duration` passed in (`Uptime`, the 0.2 clock table). One: `record_hook_result(entries, key, kind, wait, now, result)` returns `Recorded { entries, result, next }`. A `pending` replaces, in place, the pending entry of the same `EntryKey` (`pack/hook`, phase, round, instance), and the result that ends the polling replaces it too, so three pendings leave one entry and the last non-pending result is the one kept. `poll_delay(interval, retryAfter)` is the larger of the hint and the interval, never under 1 s, the interval with no hint. A `wait` at `started + timeout` while still pending becomes `error` carrying the last message. `pending` from any hook but `wait` and a PR `integrate` becomes `error`, with the pack/hook, phase and round in the message (the text of the rule is `hook::misplaced_pending_message`, shared with `effective_result`; `HookKind::may_answer_pending` is now `pub`). Two: `IntegrateAnswer` (`Merged {commit}`, `Pending {url, head}`, read from a result by `from_result`: `ok` with `outputs.commit`, `pending` with `outputs.url` and `outputs.head`) and `apply_integrate_answer(run, answer)` returning `IntegrateStep { run, release, held }`: `pending` gives `Some(Integrating {})` with `run.after_integrate` and the source's ops held, `merged` releases them in that order. The `integrating` run state already existed in `state.rs` (ts-0037-vg), so no type was added there. Three: `recover(&[Event])` lists the steps with an intent and no outcome as `CheckWorld { step, kind, action, intent_seq }` (there is no retry action), and `settle(check, Finding)` turns the check's finding into `Succeeded` (with the `StepOutcome` event to write), `RunStep` (the check was sure it did not happen) or `OutcomeUnknown` (the Queue item). Tests: `core/tests/run_rules.rs` (36: the three-pending fixture, the hold and release, the `retryAfter` against `interval:` table, the timeout, a misplaced `pending` for each hook kind, the intent-without-outcome log, the inconclusive-check log, a merge already at the target). No contract type was added or changed; the one regenerated change is the `HookResult` description (`retryAfter` now points at `poll_delay`), in `schemas/HookResult.*.json` and `packages/pack-sdk/src/generated/HookResult.*.ts`.
- Decisions for the owner: (1) A `pending` replaces only a pending entry of its key; one after a settled entry for the same key (a gate re-run) is added after it, so the last non-pending result is never lost. The plan says "replaces the previous entry", which read literally would drop it. (2) The next poll is `now + delay`, capped at the timeout, so the wait ends at its timeout with one last poll, not up to an interval after it; the cap can make the last poll sooner than 1 s after the one before. (3) The timeout error's message is "<hook, phase, round> was still pending at its <timeout> timeout: <last message>", so it carries the context the plan wants and the last message. (4) `Integrating {}` holds no outcome, so after `merged` the function reports `run: None` ("not integrating") and the state machine (Stage 3a) must put the run back where the accept found it; adding the outcome to `Integrating {}` would change a registered type. (5) An integrate answer is accepted only for an `ended` or an `integrating` run (accepting is an action on a finished run); a `failed`, `stopped`, `running` or `paused` run is refused with a message. (6) What a PR `integrate` answers is read from `outputs`: `commit`; `url` and `head`. The plan names the fields and not where they sit. (7) After `merged` only `run.after_integrate` and the source's ops are released, as the item says; the lifecycle's final `record` is not modelled. (8) A check that is sure the step did not happen yields `RunStep` (run it again with a new intent), as Stage 8's "one that shows nothing runs the step again" wants. (9) The new types are internal and not in the schema registry; `OutcomeUnknown` becomes a contract type when the Queue item's shape is written.
- Not done by design: the state machines (Stage 3a) and the engine, the runner and the real checks (Stage 8); the SIGKILL-after-intent tests (Stages 7 and 8); the Queue item's JSON and context record; the group-ready ancestor check (Stage 11); the `run_state` store itself and its schema declaration of `outputs`.
