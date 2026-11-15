---
id: ts-0063-rw
kind: task
status: active
title: "Claude stream: error results, blocked prompts, refusals, hooks"
owners: []
links:
  - { type: blocked-by, to: ts-0062-rj }
---

## Goal

The parser's cases where `result/success` does not mean the work happened (0.3, The stream protocol; S9's addendum):

- **Error results.** A `result` with `is_error: true` is never the report. A 429 gives a `limited` event. Any other status fails the phase with that status. The error result's spend is still carried. This is how a failing agent's error `result` is told apart from a report, which `ts-0050-xc` left to this stage.
- **A prompt blocked by a hook.** Claude sends `system/informational` with `prompt_submit_outcome` (`blocked_by_hook` or `stopped_by_hook`, with a `reason`), then `result/success` with `num_turns: 0`. The turn ends blocked, with the reason.
- **Refused calls.** Each is classified from the `user` event's `tool_result_meta`, joined on `tool_use_id`, never from the result's text: hook (`hook`), host (`user_reject`), rule (`config`), missing tool (an error with no meta), or unknown.
- **Hook outcomes.** `hook_started` and `hook_response` give each hook call's name, event, outcome and exit code, and `hook_no_verdict` marks a timeout. A hook that exited 1, or gave no verdict, and whose tool then ran has failed open, and the failed-open detector of 0.13 records it.
- **Hook output.** A hook's `stdout` is read for a `decision`. Neither its `stdout` nor its `stderr` is kept.

## Success criteria

- A recorded hook-blocked prompt ends the turn as blocked, with the hook's reason, and its result text is not taken as the phase's report (Stage 3b criterion).
- Over S9's recorded hook cases, each refused call gets the right cause (hook, host, rule or missing tool) from `tool_result_meta`, a hook that exited 1 raises the failed-open detector, and no hook `stdout` or `stderr` reaches the store (Stage 3b criterion). Until `ts-0065-kx` stores events, "reaches the store" is checked on every event and journal record the session gives back. The test is kept, and runs again once the daemon stores them.
- At the session level, `is_error: true` gives no `FinalReport`: a 429 gives `limited`, another status gives `PhaseFailure::Api { status }`, and the error result's cost is carried in its `Usage`. `ts-0066-zb` makes the pause and the failure real for a phase.
- A hook `ask` arrives as a `can_use_tool` request with `decision_reason_type: "hook"`, and the request carries the hook's text as its reason (0.3).
- `result.permission_denials` is never shown as prompts the user refused (0.3).

## Tests

Write these first:

- the hook-blocked prompt (both `blocked_by_hook` and `stopped_by_hook`);
- refusal classification over each of S9's cases c0–c7b, plus an unknown shape;
- hook outcomes: pass, deny, exit 1, exit 2 on `PostToolUse`, and `hook_no_verdict`;
- error results' spend: a budget error with `total_cost_usd`, a 429, and a 500.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 3b: Fake Claude (hook denies, failures and blocks, a host deny, a disallowed-tool call, an error result).
  - 0.3: The stream protocol (refused tool calls and hook outcomes; `is_error`; error results' spend); The user's own tooling (the hook-blocked prompt).
  - 0.13: the failed-open and missing-tool detectors.
  - S9 record: the addendum's cases c0–c7b and the `--include-hook-events` table. S1 record: the 429.
- The fixtures for these cases are redacted or built from S9's recordings, under `host/tests/adapters/claude/fixtures/`. A recording is never copied raw.
- Area: `host` (`adapters/claude/session.rs`).
- The test rule: no test touches the real home, real launchd or a real daemon. Each test clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as `host/tests/common/mod.rs` `TempRoot` does. Settings files are fixtures, never the owner's own `settings.json`.
- Stage 3b item (revision 18 scope, split 2026-11-15). Items of Stage 3b run one at a time, in id order, because they share `host` and the generated files.
