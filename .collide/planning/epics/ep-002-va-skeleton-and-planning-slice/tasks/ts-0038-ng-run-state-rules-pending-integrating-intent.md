---
id: ts-0038-ng
kind: task
status: ready
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
