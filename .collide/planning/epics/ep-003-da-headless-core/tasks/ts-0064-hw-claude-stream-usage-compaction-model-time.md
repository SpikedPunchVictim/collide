---
id: ts-0064-hw
kind: task
status: active
title: "Claude stream: usage, compaction, model time and limits"
owners: []
links:
  - { type: blocked-by, to: ts-0063-rw }
---

## Goal

The parser's usage and limit cases (0.3, Usage and rate limits; 0.13; S9):

- **Tokens.** Input and cache tokens are counted once per response (`message.id`), however many content blocks repeat them. Output tokens and the per-turn totals come from `result.usage`. Cache writes are split by tier where Claude reports it.
- **Model calls outside a turn.** A compaction call, and any other model call, are read from `model_usage_since_last_result`.
- **Running totals.** `duration_api_ms` and `total_cost_usd` are running totals, so a turn's model time and cost are differences. A `num_turns: 0` result (a `/compact`, or a blocked prompt) reports 0, so the difference is taken from the last result with turns.
- **Missing values.** A value Claude does not report is `None`, with the right `Usage.quality`. A phase that ends before its `result` keeps its responses' input and cache tokens, and its cost is unreported.
- **The limit signal.** It becomes a `limited` event whose reset time is wall-clock time. The signal is a `rate_limit_event` with `rate_limit_info.status: rejected` and `resetsAt` in epoch seconds, or an assistant message with `error: "rate_limit"`; a 429 error result does the same (`ts-0063-rw`). `allowed_warning` is recorded but does not pause.
- **Context.** The session reports the context size per turn, and the size in bytes of each tool result.

## Success criteria

- Over a recording with repeated content blocks in one response, input and cache tokens are counted once.
- Over a recorded multi-turn session with a `/compact` and a blocked prompt:
  - each turn's model time and cost are the differences from the last result with turns;
  - a result without `duration_api_ms` gives `None`;
  - the compaction call's tokens are their own `Usage`, and are not counted again in the turn's.

  `ts-0069-bn` owns the stage's criterion for this case and checks it end to end.
- A recorded limit message gives a `limited` event with its reset time, and no report.
- A session finished with no `result` gives a `Partial` usage: input and cache tokens, no cost.
- `Usage.quality` is `Exact`, `Partial` or `Unreported`, as 0.3 and 0.13 define them.

## Tests

Write these first:

- usage counting over repeated content blocks and over a compaction;
- per-turn differences of running totals, across a `num_turns: 0` result;
- other model calls;
- `Usage.quality` for a whole turn, a phase cut before its `result`, and a result with no usage;
- the limit message, and an assistant message with `error: "rate_limit"`.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 3b: Always-recorded metrics (tokens and cost; wall and model time per turn); the `limited` event.
  - 0.3: Usage; Usage and rate limits (the reset time is a wall-clock time); Context and cache.
  - 0.13: model time and cost as differences; missing values stay null; partial totals.
  - S9 record: usage per `message.id`; `model_usage_since_last_result`; the compaction's 5-minute write.
  - S1 record: Q8 (limits), and cost summed across `--resume`.
- The session reads no clock. The reset time comes from the line's own fields, and the time a line arrived comes from `wall_ns`.
- Area: `host` (`adapters/claude/session.rs`). Storing these numbers is `ts-0067-kw`.
- The test rule: no test touches the real home, real launchd or a real daemon. Each test clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as `host/tests/common/mod.rs` `TempRoot` does. Settings files are fixtures, never the owner's own `settings.json`.
- Stage 3b item (revision 18 scope, split 2026-11-15). Items of Stage 3b run one at a time, in id order, because they share `host` and the generated files.
