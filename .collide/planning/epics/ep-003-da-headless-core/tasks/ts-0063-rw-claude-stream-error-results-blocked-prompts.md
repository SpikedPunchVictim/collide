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

### Built

- `core/src/agent.rs`: three additions, all closed (each is in the generated schemas). `HookDecision` (`allow`, `deny`, `ask`, `block`) and a `decision` on `AgentEvent::HookCall`, absent when the hook's output gave none. `HookFailure` (`exit_error`, `no_verdict`) and `AgentEvent::HookFailedOpen { name, event, tool_call_id, cause }`, the failed-open observation (decision 1). `KNOWN_EVENT_TYPES` has 13 names. Neither event has a place for a hook's `stdout`, `stderr` or `output`.
- `host/src/adapters/claude/session.rs`: the parser, extended. The session now also keeps, for the turn in flight, its calls with no result, its failed tool hooks and a blocked prompt (all cleared when a turn ends or a new one is sent, and all rebuilt by replay).
  - `system/hook_response` gives a `HookCall` (name, event, outcome, exit code, and the `decision` read from `stdout`). `hook_started` gives no event. `hook_no_verdict` marks the timed-out hook's call, or, with no `hook_response` before it, gives the `HookCall` itself.
  - `user` with `tool_result` gives a `ToolRefused` for a call that did not run, from the `tool_result_meta` entry with the call's id: `source` `hook` is `hook`, `user_reject` is `host`, `config` is `rule` (with or without a `reason_type`), any other source or a call that did not run with no decision is `unknown`; an error with no metadata at all is `missing_tool`. The result's text is never read. A `user` line with no `tool_result` is still `Other`.
  - A failed tool hook (outcome `cancelled`, or `error` with any exit code but 2) of a `PreToolUse` or `PostToolUse` event waits for the result of its call. If the call ran (it was not refused), the session gives `HookFailedOpen`. A hook is joined to a call by the call's id when the stream names it (`hook_no_verdict`), else by the tool in its name (`PreToolUse:Bash` is for `Bash`).
  - `system/informational` with `prompt_submit_outcome` `blocked_by_hook` or `stopped_by_hook` gives no event; the turn's `result` then ends `Blocked { reason }` with the outcome's `reason`, and its text is not the report. A cancel still wins (`Cancelled`).
  - An error result gives no report. Its `total_cost_usd` is carried as a `Usage` with scope `RunningTotal` and only a cost (USD, quality `Exact`); a result with no cost gives none. A 429 also gives `Limited { reset_at_ms: None }`. Any other status gives only the `TurnEnd` `Error { status }`, and `finish` gives `PhaseFailure::Api { status }`.
  - `result.permission_denials` is never read.
- Fixtures, built for the tests (no recording copied): `hook-no-verdict.jsonl` (S9 case 4c with the flag), `hook-deny-json.jsonl` (case 1, JSON on stdout, with `SECRET-...-MARKER` text in `stdout` and `stderr`), `stopped-prompt.jsonl` (`stopped_by_hook`). The ts-0062 recordings `blocked-prompt`, `hook-deny`, `hook-failure`, `hook-block`, `host-deny`, `disallowed-tool`, `limit` and `error-result` are now read in full. Recordings: 20.
- Tests, in the `claude_adapter` target (now 200): `hooks.rs` (new, 31: hook outcomes for pass, deny, exit 1, exit 2 on `PostToolUse` and a timeout; the decision table; no `stdout` or `stderr` in any event or write; S9's cases c0 to c7b plus unknown shapes and a text trap; the failed-open cases; each hook recording through fake Claude under the real shim), `results.rs` (new, 15: blocked and stopped prompts, a 429, a 500, a budget error, errors with no cost, `permission_denials`), `replay.rs` (3 more, 14: replay equals live for seven of the new recordings, a cut at every byte of the failed-open journal, a cut between the failed hook and its result). `session.rs` still has 55: one test now uses a 500 where it used a 429, because a 429 now also gives `Limited` and a cost. `fake_claude.rs`: the recording count is 20. In `core/tests/agent_events.rs`, 3 new tests (the decision, `HookFailedOpen` and its closed shape), the schema count is 13 and the unknown-key list has the new event. The session-helper functions of `session.rs` tests are `pub(crate)` so the new files share them.
- Generated: `pnpm gen` changed the `AgentEvent`, `AskNotice`, `OwnerNotice` and `RunOutputResult` schemas and types; a second `pnpm gen` changes nothing. No stub tag belongs to this item, so `FILLED_TAGS` is unchanged.

### Decisions for the owner

1. The failed-open detector (0.13). No detector type existed. Chosen: a new `AgentEvent::HookFailedOpen { name, event, tool_call_id, cause }`, which the session gives when a failed tool hook's call ran. It is a fact about one call, so `ts-0067-kw` can count it per hook and attach the cost without joining a hook to a call itself (a `HookCall` has no call id in the stream). Alternative: a `tool_call_id` on `HookCall` plus a join in `ts-0067-kw`; that cannot work, because `hook_response` does not name the call. Recommend as built. The detector's finding (what it cost, what a change would save) stays `ts-0067-kw`'s.
2. What counts as failed open. Chosen: a `cancelled` hook (a timeout), or an `error` hook with any exit code but 2, on a `PreToolUse` or `PostToolUse` event, whose call then ran. Exit 2 is the deliberate block, so a `PostToolUse` exit 2 is never a failure. 0.13 says "exit 1": claude documents every non-zero code but 2 as non-blocking, so exit 3 lets the call run too. Alternative: exit 1 only. Recommend as built. A `PostToolUse` hook that fails also counts, since its `event` is in the observation and the detector may drop it. Invalid JSON on exit 0 is not counted: it reads as a pass (see Not done).
3. The hook's `decision` is kept, as a word. Chosen: `decision` on `HookCall`, read from `hookSpecificOutput.permissionDecision` or the top-level `decision`. It is the only way a `PostToolUse` JSON block shows (outcome `success`, exit 0), and 0.13 counts blocks per hook. Alternative: read `stdout` and keep nothing, as the goal's wording could mean; then a JSON block and a JSON deny would be invisible. Recommend as built: one of four fixed words cannot hold a secret.
4. A missing tool is any error with no metadata. Chosen as 0.3 says. The risk: S9 measured only `Bash` on haiku, and `tool_result_meta` may be absent for a built-in tool that needs no approval (Read) when it fails (a missing file), which would read as `missing_tool`. Alternative: also require the tool's name to be absent from `system/init.tools`; the session would then keep that list, and S9 7a shows the model called a tool `init` did not list. Recommend keeping as built until `ts-0070-mk` records a failing `Read` or `Edit`; add the `init.tools` check then if the metadata is absent.
5. `hook_started` gives no event. Chosen: like `system/init`, since `hook_response` carries all of it, and an `Other` for every hook call would swamp the count of unknown events that 0.13 treats as a finding. Alternative: keep it as `Other`. `ts-0068-wx` can read the canary from the `HookCall` for the `SessionStart` event; if it needs a hook that never answered, it needs `hook_started` back.
6. The error result's spend. Chosen: a `Usage` of scope `RunningTotal` with only the cost, because `total_cost_usd` is the session's running total (S1) and a `Turn` scope would be added into a turn by mistake. The result's tokens are left to `ts-0064-hw`, which reads them for every result. Alternative: tokens and cost together in a `Turn` usage. Recommend as built.
7. A 429 gives `Limited { reset_at_ms: None }`, the status being all the result has (its text is not parsed). The reset time is on the `rate_limit_event` that came before it, which `ts-0064-hw` reads. When it does, one limit must give one `Limited`: `ts-0064-hw` should remember the `rate_limit_event`'s reset time and put it on the one from the result, or drop this one.
8. A budget error is `TurnEnd Error { status: None }` and `PhaseFailure::Api { status: None }`. `PhaseFailure` has no budget cause, and 0.3 says the phase goes to `ask_human` with cause budget. Recommend `ts-0066-zb` adds it with the pause, since it needs the subtype, which the session reads today and the event drops.
9. A blocked prompt's `reason` is the hook's own, and for an exit-2 hook that is its `stderr` text (S9, Q5: `prompt_submit_outcome.reason`). That is the intended message to the person, and the Stage 3b criterion asks for it; it is not hook output of the `hook_response` kind, which is never kept. A hook that prints a secret on stderr and exits 2 on a prompt would put it there. Alternative: drop it. Recommend as built, with the pack SDK's docs saying a blocking hook's message is shown.
10. A hook joined to a call by tool name. Two parallel calls of the same tool, one hook failure, and both results ran: the failure is given to the first result. Recommend accepting it; the hook's call id is not in the stream, and the event says which hook and which event.

- Proposed plan text (IMPLEMENTATION_PLAN.md, 0.3, Normalized events), replacing the `HookCall` clause: "`HookCall { name, event, outcome, exit_code, decision }` (never the hook's `stdout` or `stderr`; `decision` is the word the hook's output gave, when it gave one), `HookFailedOpen { name, event, tool_call_id, cause }` (a `PreToolUse` or `PostToolUse` hook that timed out or exited with a code other than 0 or 2, and whose tool then ran: the failed-open detector's observation, 0.13)". And in 0.13, the failed-open sentence: "A hook that failed open is a `HookFailedOpen` event: a `hook_response` that is `cancelled`, or `error` with an exit code other than 2, or a `hook_no_verdict`, where the tool then ran."

- Owner decisions (2026-11-15): the recommendations above are accepted as built. The proposed plan text is applied to 0.3 (Normalized events) and 0.13 (the failed-open detector) with this item. Follow-ups: `ts-0064-hw` makes one limit give one `Limited` and carries an error result's tokens; `ts-0066-zb` adds a budget cause; `ts-0070-mk` checks against the real binary whether a failing Read or Edit reads as `missing_tool`, and whether hook events name their call, so two parallel calls of one tool cannot mis-join a hook failure.

### Not done, by design

- Invalid JSON from a hook that exits 0 is not a failure: the session can see it (the `stdout` is read and has no `decision`), but 0.3 says a hook that gives invalid JSON "looks like a pass", and a hook that prints text and nothing else is normal. Raising it would need a closed shape for "output not JSON".
- The reset time of a limit, `rate_limit_event`, `allowed_warning`, and an assistant message with `error: "rate_limit"`: `ts-0064-hw`. The pause and the resume of a phase on `Limited`, and the budget cause: `ts-0066-zb`.
- The tokens of an error result, and any other usage: `ts-0064-hw`.
- Storing hook calls, refusals and the failed-open event, and the two detectors' findings with their cost: `ts-0067-kw`. "No hook `stdout` or `stderr` reaches the store" is checked on every event and every message the session writes (the journal's `in` records); the test is kept and should run again over the daemon's rows once `ts-0065-kx` stores them.
- The "tool not in `init.tools`" signal, a second source for the missing-tool detector (decision 4): the session does not keep the tool list.
- Hooks other than `PreToolUse` and `PostToolUse` (`PermissionRequest`, `Stop`, `SessionStart`) never give `HookFailedOpen`: S9 measured only the first two.
- Tools other than Bash, MCP tools and subagent hooks: S9 measured Bash on haiku, so their metadata shapes are unmeasured; `ts-0070-mk` is where the real `claude` is run.
- No change to IMPLEMENTATION_PLAN.md (proposed text above).

