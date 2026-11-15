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

### Built

- `core/src/agent.rs`: three additions, all closed and in the generated schemas (`pnpm gen` twice, no diff the second time). `Usage.model_time_ms` (`Option<u64>`, left out when absent: the time of the call or calls, which a turn's is a difference of, decision 1). `UsageScope::OtherCalls` (a model call that is neither the turn's own nor a compaction). `AgentEvent::ToolResultSize { tool_call_id, bytes }`, the size of a tool result with no place for its content. `KNOWN_EVENT_TYPES` has 14 names. `core/src/testing/acp.rs` and the tests that build a `Usage` give `model_time_ms: None`.
- `host/src/adapters/claude/session.rs`: the parser, extended. It keeps, for the turn in flight, its responses (once per `message.id`, input and cache tokens, and the context of each), whether a `compact_boundary` came, and a held limit; for the session, the two anchors the running totals are differenced from and the model of its last response. All of it is rebuilt by replay.
  - `assistant`: a `Step` usage for a response not seen before (input, cache read and write, the write by tier, `context_used`; no output, which the stream's row counts only in part (S9); quality `Partial`), ahead of the line's own events. A row with no `message.id` or no `usage` is not counted, because it cannot be counted once.
  - `result`, every one, an error result's too: `Compaction` or `OtherCalls` usage, then `Turn`, then `RunningTotal`, then `Limited` if a limit is held, then `TurnEnd` (the usage events are replaced where ts-0063 gave a cost-only `RunningTotal` for an error result).
    - `Turn`: tokens, cache write tiers and `thought` from `result.usage`, never from the stream's rows. `cost` is `total_cost_usd` less the one before it. `model_time_ms` is `duration_api_ms` less the one of the last result with turns. `context_used` is the context of the turn's last response of the session itself; `context_size` is the `contextWindow` of that response's model in `model_usage_since_last_result`.
    - `Compaction` or `OtherCalls`: what `model_usage_since_last_result`, summed over its models, counts beyond `usage`, field by field, where both sides report the field. It is `Compaction` when a `compact_boundary` came in the turn or `local_command` is `compact`, else `OtherCalls`. Its cost is the spend of the result when the turn used no tokens of its own (a `/compact`), and the turn's `cost` is then 0; otherwise the spend stays in the turn's (decision 3).
    - `RunningTotal`: `total_cost_usd` as reported, nothing else, when the result has it.
  - Quality: `Exact` when input, output, cache read and cache write are all reported, `Unreported` when none is, else `Partial`. A result with no `usage` takes its responses' input and cache tokens (`Partial`), or is `Unreported`.
  - `finish`: a turn sent and never ended gives a `Turn` usage of its responses' input and cache tokens (`Partial`; `Unreported` with no response), no output, no cost, stamped with no time. A held limit no result followed is given there, once.
  - `rate_limit_event` `rejected`, an assistant `error: "rate_limit"` (its reset time, when it has one, is `quotaLimits.resetsAt`) and a 429 error result are signals of one limit. They are kept, and one `Limited` is given at the turn's `result`, with the first reset time any of them had (`resetsAt` in epoch seconds, as milliseconds). `allowed_warning` is an `Other` named `rate_limit_event/allowed_warning`, any other status `rate_limit_event`; neither pauses.
  - `user` with `tool_result`: a `ToolResultSize` for each result, before its refusal or failed-open events.
- `host/src/adapters/claude/mod.rs`: the declared granularity has `OtherCalls`.
- `host/tests/adapters/claude/fixtures/recordings/usage-session.jsonl`: built for the test, stand-in numbers and ids, none copied. Four turns on one process: two responses over three rows, a manual `/compact` (zero turns, `duration_api_ms` 0, the spend only in `model_usage_since_last_result`), a blocked prompt, and a turn after them. Recordings: 21.
- Tests, in the `claude_adapter` target (now 267, from 200): `usage.rs` (new, 41: once per response, steps, the turn from `result.usage`, quality, differences across a `num_turns: 0` result, a resumed process, a missing value, compaction and other calls, a phase cut before its result, an error result's tokens, and the recording), `limits.rs` (new, 16: one limit from each line and from all three, the held limit, the reset time filled in and first-wins, bad reset values, allowed and warning, finish), `tool_results.rs` (new, 8), `replay.rs` (2 more: the usage session replayed equals live, the limit journal cut at every byte). Existing tests that listed every event of a result or a tool result now filter usage and size events through `bare` (`session.rs`); `results.rs` has the 429's reset time and the error result's tokens. Core: `agent_events.rs` pins the new field, scope and event. Workspace: 2021 tests pass, twice.

### Decisions for the owner

1. The core contract is widened in three places, because nothing held these (a shared contract that does not fit is widened and reported, not skipped). Chosen: `Usage.model_time_ms`; `UsageScope::OtherCalls`; `AgentEvent::ToolResultSize`. Alternatives: model time on `TurnEnd` (a turn's usage and time then split across two events); fold other calls into `Compaction` (a label that would be wrong) or into the `Turn` (its tokens then are not `result.usage`'s); put the tool-result size on `ToolCall` (it comes later than the call, and a refused call has one too). Recommend as built. Proposed plan text is below.
2. What `quality` means. Chosen: it describes the token counts. `Exact` is all four main counts reported; the cost and model time are separate options that are `None` when not reported. A `Step` is `Partial` because it has no output. Alternative: let a missing cost make a turn `Partial`. Recommend as built: `ts-0067-kw` already stores cost and time as their own nullable columns.
3. Where a turn's cost goes. Chosen: cost is the difference from the result before it (any result, not only one with turns), because a `/compact` result reports the true running total, so a compaction's cost is in the result of the `/compact` and not again in the next turn's. A `/compact` gives the whole spend to its `Compaction` usage and its own `Turn` cost is 0, so the cost of `Turn`, `Compaction` and `OtherCalls` events adds up to the final running total (the recording's test checks it). For a compaction inside a turn, Claude reports one cost for both and nothing splits it, so the `Compaction` cost is `None` and the turn's cost holds it; its tokens are still apart. Alternative: take cost from the last result with turns too, which counts a compaction twice. Recommend as built. Model time follows the plan: from the last result with turns; a result with no turns has none.
4. A resumed process. Chosen: a session started with `Resume` has no total to difference its first result from, because S1 measured a resumed process continuing the earlier total. Its first turn's cost is the figure in `model_usage_since_last_result` (`None` if there is none), and its model time is `None`; the second result is differenced from the first. `duration_api_ms` across `--resume` is unmeasured, so the first turn's model time is left unreported rather than guessed. Alternative: difference from 0, which would charge the whole earlier session to the first turn. Recommend as built; a measurement would let the model time be read.
5. A missing value breaks the chain. Chosen: a result with no `total_cost_usd` or `duration_api_ms` leaves nothing for the next result to be differenced from, so the next turn's model time is `None` (its cost falls back to the turn's own figure when `model_usage_since_last_result` has one). A total that goes backwards is treated the same. Alternative: difference over the gap, which would give one turn the time of two. Recommend as built.
6. One limit, one `Limited`. Chosen: the signals of a turn are held and one event is given at the `result`, just before `TurnEnd`, with the first reset time any line had; a later line fills in a missing one and never replaces one. A rejected `rate_limit_event` or a limit message with a result that is not an error still gives the event. A limit with no result is given at `finish`. Alternative: give it on the first line, and let a later reset time be lost or sent as a second event, which the engine would read as a second pause. The cost of holding: the event comes a few lines late, at the result Claude writes straight after. Recommend as built.
7. `quotaLimits.resetsAt` on an assistant message is read as seconds, like `rate_limit_info.resetsAt`. S1 saw the key in transcripts only, never in the stream, and not its unit. If a reset time arrives from the event too, the first one wins. Recommend keeping it, and checking the unit when a real limit is recorded (`ts-0070-mk`).
8. `allowed_warning` is an `Other` named `rate_limit_event/allowed_warning`, so it is recorded and counted by name and can be found by `ts-0066-zb` when other runs pause early. Alternative: its own event, which the plan does not ask for. Recommend as built.
9. An error result's tokens. Chosen: any result gives `Turn` and `RunningTotal`, the same as a normal result, so an error result's tokens count once, from `result.usage`. The `Step` usages of its responses are for context and are never added in. The ts-0063 cost-only `RunningTotal` stays, now with the `Turn`.
10. Context. Chosen: a `Step`'s and the turn's `context_used` is input plus cache read plus cache write of a response of the session itself (a subagent's response is counted in tokens but is not the session's context), and `context_size` is the model's `contextWindow` from `model_usage_since_last_result`. Both are `None` when not reported. Alternative: the window from `get_context_usage`, which the effective `compact_at` changes (S9). Recommend as built until the context view needs the effective window.
11. The size of a tool result is the length in bytes of its string content, or, for content blocks, the text of each text block plus the JSON of any other block. A result with no content has no size event, and an empty one is 0. A refused call's result has one too.
12. Other calls are known per field only where both `model_usage_since_last_result` and `usage` report it; a field one side lacks is `None`, not 0, and no event is given when nothing is above 0. Over several models the entries are summed before `usage` is taken away, since `usage` is not split by model.

Proposed plan text:

- 0.3, Normalized events, Usage bullet: replace "an event with a scope (`Turn`, `Step`, `RunningTotal` or `Compaction`, the model call of a compaction, never added into a `Turn`)" with "an event with a scope (`Turn`, `Step`, `RunningTotal`, `Compaction`, the model call of a compaction, or `OtherCalls`, any other model call outside the turn's own; neither is ever added into a `Turn`, and a `Step` is for context per response and is never added to a `Turn` either)", and add "`model_time_ms`, the model time, a difference of `duration_api_ms` for Claude" after "a cost with its currency".
- 0.3, Normalized events: add "`ToolResultSize { tool_call_id, bytes }` (the size of a tool result, for each result, with no content)" to the list.
- 0.3, Usage and rate limits: after "No text parsing is needed." add "One limit is one `limited` event: the signals of a turn (the rejected `rate_limit_event`, the message, a 429 error result) are held until the turn's `result`, and the event carries the first reset time any of them has."
- `usage_report.granularity` (0.3, capabilities table): "(`turn`, `step`, `running_total`, `compaction`, `other_calls`)".

- Owner decisions (2026-11-15): the recommendations above are accepted as built. The proposed plan text is applied to 0.3 with this item. Follow-ups: `ts-0067-kw` sums `Turn`, `Compaction` and `OtherCalls` and never adds a `Step` (decision 9); `ts-0070-mk` measures `duration_api_ms` across `--resume` (decision 4) and the unit of `quotaLimits.resetsAt` when a real limit is recorded (decision 7).

### Not done, by design

- Storing any of these numbers, the per-phase totals, and the pause on `Limited`: `ts-0067-kw` and `ts-0066-zb`.
- The compaction's trigger, tokens before and after, duration, and whether it landed mid-task (0.13): the `compact_boundary` line is still an `Other`, whose only effect here is to label the turn's extra model call. `ts-0067-kw` reads the line when it stores compactions.
- Per-model usage: a `Usage` has no model, so `usage` and the other calls are summed across models.
- Splitting a mid-turn compaction's cost from the turn's (decision 3).
- Reading `duration_api_ms` across `--resume` (decision 4), and a real limit's stream shape (`ts-0070-mk`).
