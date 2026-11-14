---
id: ts-0034-hh
kind: task
status: ready
title: "Ask methods and events"
owners: []
links:
  - { type: blocked-by, to: ts-0032-pp }
  - { type: blocked-by, to: ts-0036-xt }
---

## Goal

The ask contract, in `core`: the methods `ask.start`, `ask.cancel` and `ask.test`, and the `ask.*` events. `ask.start` has two shapes: the one the window sends (agent id and the request, no token) and the one the shell forwards to the daemon, which adds the endpoint URL and the ask's token, so the window never holds one. `ask.*` events carry the normalized stream events of `ts-0036-xt` and the ask's outcome (finished, cancelled, timed out, stopped at budget, failed, with its cost and `Usage.quality`). `ask.test` is "Test this agent": its result holds whether the endpoint was reached with the ask's token, the time to the first tool call, the finish time, the cost or "not reported", the readable folders and tool kinds in effect, a plain failure text, and the confinement probe (`red`, `green` or `not tested`). An ask is tied to the connection that started it, so the daemon cancels it when that connection drops (the rule is stated in the type's docs; the daemon builds it in Stage 6b).

## Success criteria

- Serde round-trips pass for every `ask.*` method, result and event.
- The window's `ask.start` has no token or endpoint field, and the daemon's has both. An unknown key fails in each.
- No `ask.*` event type has a field that can hold a token.
- An `ask.*` event can embed any normalized event of `ts-0036-xt`.
- The `ask.test` result's probe field takes `red`, `green` or `not tested`.

## Tests

Write first: round-trip tests; the two `ask.start` shapes; a type-level test that no event carries a token field; the probe values.

## Notes

- IMPLEMENTATION_PLAN.md, 0.2 (the UI-tools endpoint: ask tokens, the ask agent child), 0.3 (the ask agent), 0.7 (Test this agent, the probe), 0.11 (the ask box), Stage 2: Skeleton, Contracts (the ask methods and their events).
- Area: `core` (types and schemas).
- Done when the criteria hold.
