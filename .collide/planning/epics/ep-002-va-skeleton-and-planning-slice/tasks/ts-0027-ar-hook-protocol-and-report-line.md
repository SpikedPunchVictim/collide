---
id: ts-0027-ar
kind: task
status: ready
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
