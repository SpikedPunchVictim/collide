---
id: ts-0027-ar
kind: task
status: done
title: "Hook protocol and report line"
owners: []
links:
  - { type: blocked-by, to: ts-0009-cu }
---

## Goal

The hook protocol and the report line format, in full, with JSON Schema and generated types. Hook: the stdin input `{protocol, hook, pack, board, line, experiment, run, launch, config, tasks[], phase, workspace, history[], state}` (task points add `change`; each `tasks[]` entry carries its links one hop out), and the result file `{status: ok | fail | unverified | pending | refused | error, message, outputs, retryAfter?}`. `pending` is valid only from a `wait` hook and from a PR `integrate`; from any other hook it is `error`. Outputs are capped at 64 KB an object. Point bindings take `timeout:` (30 s for task points, 120 s for move points). Report line: `v`, `t`, and the kinds `span.start`, `span.end`, `metric`, `progress`, `usage` and `limited`.

## Success criteria

- Serde round-trips pass for every hook and report type.
- A result with `status: pending` and `retryAfter` parses. `pending` from a hook that is not a `wait` hook or a PR `integrate` is classified `error` by the result's validity function.
- A non-zero exit, or a missing or invalid result file, is `error`.
- The report line has the caps of 0.5: 4 KB a line, 100 lines a second with a burst of 1,000 (a token bucket that takes its time as an argument), 16 MiB a file. An excess line is dropped and "report truncated" is recorded.
- A report line with no `t` is rejected and counted. A `usage` line with a merged `cached` count is rejected and counted.

## Tests

Write first: round-trip tests; the `pending` classification table; the report caps with a fake clock; the rejected-line cases.

## Notes

- IMPLEMENTATION_PLAN.md, 0.5 (Input, Result, Reporting, Caps), 0.7 (the `wait` phase, for `pending` and `retryAfter`).
- Area: `core` (types, schemas, the pure line parser and token bucket).
- Done when the criteria hold. The rule that a pending result replaces its run-state entry is `ts-0038-ng`'s.
- Types added (core modules `hook` and `report`). Registry roots, so `collide schema` lists them: `HookInput` (with `HookTask`, `HookLink`, `HookExperiment`, `HookPhase`, `HookWorkspace`, `WorkspaceKind`, `HistoryEntry`, `HookState`), `HookResult` (with `HookStatus`) and `ReportLine` (tagged by `kind`: `span.start`, `span.end`, `metric`, `progress`, `usage`, `limited`). No ts-0009 or ts-0010 type was extended. A task point's input reuses ts-0009's `Change` and `LatestRun` as the optional keys `change` and `latestRun`. `TaskPointPayload` is not used inside `HookInput`, because serde cannot flatten a struct that denies unknown keys; it stays the documented shape of the two keys.
- `HookTask` is its own type and not the provider's `Task`: `Task` has a body, aliases, a version and owners, and a hook must get fields and paths only. A test refuses a `body` key. It holds `key`, `shortId`, `kind`, `state`, `path`, `templateFields`, `fields` and `links`; each link is `{type, dir, key, kind, status, category, stale}` and reuses `Category` and `LinkDir`. Nothing here builds a `HookTask` from a `Task` (the runner will), because `shortId` comes from the provider's `shortIdPattern`.
- Validity: `hook::effective_result(kind, exit_zero, file) -> HookResult`, with `HookKind` (`Workspace`, `Record`, `Integrate`, `IntegratePullRequest`, `Sync`, `Gate`, `Script`, `Wait`, `Decision`, `Point`). Whether an `integrate` is for a pull request is not visible in the result, so the caller says: `IntegratePullRequest` may answer `pending`, plain `Integrate` may not. How a pack declares that is `ts-0012-ua`'s. An `error` made here carries the reason as its message and no outputs.
- Where the plan goes beyond the item, the plan wins: `effective_result` also turns `unverified` into `error` from anything but a gate, a `script` or `wait` hook, or a handler at a named point (0.5, Result). `sync` and `Decision` (a workflow's decision script) are not named by the plan, so they are in the "any other hook" group for both `pending` and `unverified`.
- The 64 KB outputs cap is the size of the whole `outputs` object as compact JSON, at most 65,536 bytes. The plan says "each output object"; this takes the `outputs` object as the one object. It is checked inside `effective_result`, so oversized outputs make any status `error`, `pending` included.
- Timeouts: `hook::TASK_POINT_TIMEOUT_SECS` (30) and `MOVE_POINT_TIMEOUT_SECS` (120) are constants only. The binding struct is `ts-0012-ua`'s, which must default `timeout:` to the first for task points and to the second for `task.before_move` and `task.after_move`. The plan gives no default for the `run.*`, `phase.*`, `workspace.*` and `experiment.*` points. The `interval:` and `timeout:` of a `wait` phase are the workflow types' (`ts-0024-uv`).
- Result file: only `status` is required (`message` defaults to empty and `outputs` to `{}`), because the plan marks only `retryAfter` as optional but a hook author should not lose a verdict for a missing empty field. `retryAfter` is an unsigned whole number of seconds (`u32`); a fractional or negative one makes the file invalid. A `retryAfter` on a result that is not `pending` is kept and ignored. The clamp against `interval:` and the rule that a pending result replaces its run-state entry are `ts-0038-ng`'s.
- Input shapes the plan does not give, so they are my reading: `protocol` is the integer `hook::HOOK_PROTOCOL` (1); `workflow` is added next to `pack` because 0.4 says each handler receives both (null outside a run); `run` is the run id or null; `line` and `board` are strings; `experiment` is `{id, arm, dir}`; `phase` is `{name, kind, round, dir}`; `workspace` is `{kind, path, gitDir}` with kinds from 0.8; a `history[]` entry is `{phase, kind, round, startHead, endHead, artifacts, verdict}`; `state` is `{run, workspace, point}`, each a map of outputs; `launch` and `config` are open maps. The point that fired a handler is not a key: the plan's input list has none, so the hook's name is all it gets. These are fixed again where a later item (the run state `ts-0037-vg`, the event catalogue `ts-0018-dd`) needs more.
- Report line: `v` and `t` are repeated in each variant (serde cannot flatten them out of a tagged enum that denies unknown keys), so a line with an unknown key is malformed. `t` must be present and not null; it is not checked as a date, because the type is a string and the tailer does not need it parsed. `span.end` `status` is a string (the plan's example is `ok` and gives no list); a span's `attrs` is an open map. `usage` has `model`, and `tokensIn`, `tokensOut`, `cacheRead`, `cacheWrite`, `cacheWrite1h` and `cacheWrite5m`, each optional. `limited.resetsAt` is epoch seconds, as in 0.3. A `progress` line has `done` and an optional `total`.
- Parser and caps: `report::parse_line` and `report::ReportIntake` are pure and take time as a `Duration` (an uptime reading; only differences matter). `TokenBucket` counts in whole units of a billionth of a token, so no rounding moves a boundary at 100 lines a second. A reading that goes back gives no tokens. Each line is checked in this order: the file's bytes (every line counts, kept or not, with its newline; once past 16 MiB every later line is dropped), then the bucket (every line costs a token, junk included), then the 4 KB length, then the parse. Any dropped line sets `truncated()`, and `REPORT_TRUNCATED` is the label for the span; the plan says "at the size cap", the item says "an excess line", and this marks all three drops. A line is rejected, not dropped, for `NoTimestamp`, `MergedCache`, `UnsupportedVersion` (a `v` other than 1) or `Malformed`; each is counted apart in `ReportCounts`, and a `t`-less line is always `NoTimestamp`.
- Left for the tailer and the runner (later items): the rule that `usage` and `limited` count only from a `script` phase with `agentWork: true`; the span-id rules (a repeated open id is rejected and counted), nesting under `parent`, `unclosed` spans; the `inherited` mark for lines written by another pid; the 50 ms poll and holding a partial last line; redaction of `attrs`; `collide report`.
- Tests: `core/tests/hook_protocol.rs` (the input, the result, the table of kinds against `pending` and `unverified`, exit and file cases, the 64 KB boundary), `core/tests/report_line.rs` (the six kinds, the rejected lines, the bucket and the caps on a fake clock), a `collide schema` test in `host/tests/schema_command.rs`, and `packages/pack-sdk/test/hook-types.test.ts`.
