---
id: ts-0034-hh
kind: task
status: done
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

- **Built**
  - `core/src/ask.rs`: `AskMethod` (`ask.start`, `ask.cancel`, `ask.test`, with `window_params_schema`, `params_schema`, `result_schema`); `AskStartParams` (window: ask, agent, request) and `AskStartDaemonParams` (adds `endpoint` and `token`; `Debug` hides the token); `AskCancelParams`; `AskTestParams` and `AskTestDaemonParams`; `AskOutcome { end, cost, quality }` with `AskEnd` (`finished`, `cancelled`, `timed_out`, `stopped_at_budget`, `failed`) and `AskCost` (`reported {cost}` or `not_reported`); `AskNotice` (`ask.event` with `AskEventNotice`, which embeds `AgentEvent` whole, and `ask.outcome` with `AskOutcomeNotice`), `to_notification`, `from_notification`, `scrub_token`; `AskTestResult` and `ConfinementProbe` (`red`, `green`, `not tested`). The connection-drop rule is in the module docs and on `AskStartDaemonParams`.
  - `core/src/lib.rs` (`pub mod ask`) and `core/src/schema.rs` (eight roots registered: `AskStartParams`, `AskStartDaemonParams`, `AskCancelParams`, `AskTestParams`, `AskTestDaemonParams`, `AskTestResult`, `AskOutcome`, `AskNotice`); `pnpm gen` wrote their schemas and TypeScript, and a second run changed nothing.
  - `core/tests/ask_protocol.rs`: round trips, the two `ask.start` shapes, the type-level test (a walk of the generated schemas for property names containing `token` or `endpoint`, and for free-form places), the probe values, the scrub.
  - `host/src/ui_tools.rs`: `WindowGone::cancel` is now `Vec<AskCancelParams>` (the core type); `AskCancel` is gone. `host/tests/ui_tools.rs` changed one assertion to `.ask.as_str()`.
  - No stubs added; `stub-tags.json` and `real_tree.rs` are unchanged.
- **Decisions for the owner**
  - The ask's id is chosen by the window and sent in `ask.start` and `ask.test`, so it can show lines and Cancel before the daemon answers, and the shell can register the token under it. Alternative: the shell or the daemon makes the id and returns it, which leaves a window with no id until the answer and events that can arrive first.
  - `ask.test` has two shapes like `ask.start` (window: ask, agent; daemon: adds endpoint and token), because the test is an ask and the daemon needs both to start the child. The item named two shapes for `ask.start` only. Alternative: the daemon makes its own test token, which breaks "the shell makes every ask token".
  - `ask.*` stays out of `owner::Method` (owner tests require it to answer `METHOD_NOT_FOUND` until Stage 6b), in its own `AskMethod`, as `ui_tools.*` is. Alternative: a `Family::Ask` in `owner::Method` once the daemon takes the methods, with the window shape handled by the shell.
  - Refusals before launch (`claude was not found on PATH`, `this agent is not confined; acknowledge it in Agents first`, `this agent cannot use HTTP MCP servers`) arrive as `AskEnd::Failed {text}`, and `ask.start` answers `Ack` once the daemon accepts the ask. Alternative: an owner error from `ask.start`, which gives the ask box two places that write a failure.
  - The agent's own tool input and the raw body of an unknown event are free-form values in `AgentEvent` (ts-0036-xt), so "no field can hold a token" cannot hold literally for an embedded event. The type-level test allows exactly three places (`AgentEvent.input`, `AgentEvent.raw`, `PermissionRequest.input`) and fails if any other appears or an allowance goes stale. `AskNotice::scrub_token` removes the ask's token from them (an added pure function; the daemon must call it before sending or storing an event). Alternative: a narrower event type that drops tool input, which loses the one-line-per-call text.
  - `AskEnd::Cancelled` carries no cause (requested, window closed, connection dropped), because nobody reads the cause: a new window starts with no asks and a dropped connection has no reader. Alternative: a `cause` field.
  - Money is used for cost and budget (a currency with the amount), as `Usage.cost` does, and not a bare USD number. Times in the test result are milliseconds (`first_tool_call_ms`, `finish_ms`) and absent, not 0, when they did not happen.
  - `AskTestResult.endpoint_reached` is set by the daemon from the stream; the shell, which sees the endpoint's requests, may replace it. Alternative: leave it to the shell only.
  - `ask.test` sends no `ask.event` or `ask.outcome`; its answer is the result.
- **Not done by design**
  - The daemon's handling of the methods, the cancel when the connection drops, the shell's addition of the endpoint and token, and the refusals and probe run: Stage 6b.
  - The Claude launch line, the ACP ask profile and the ask box that reads these events: Stage 6b.
  - A TypeScript test of the generated types in the app: Stage 6b, with the ask box.
- Owner decisions (2026-11-15): the recommendations above are accepted as built.
