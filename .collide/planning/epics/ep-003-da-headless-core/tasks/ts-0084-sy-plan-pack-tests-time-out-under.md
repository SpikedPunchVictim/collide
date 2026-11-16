---
id: ts-0084-sy
kind: task
status: done
title: "Plan pack tests time out under load"
owners: []
links: []
---

## Goal

Two `packs/plan` tests have failed the gates on vitest's 5 s default timeout while passing alone in about a second:

- `create` > "without a seed the suffix is random: two creates in two repositories seldom match, and every one is
  valid" (5020 ms, in the gates of `ts-0076-cw`; 1.3 s alone);
- the plan provider's "list returns every item under epics/ with no task.invalid, and does not read spikes/"
  (5089 ms, in the gates of `ts-0070-mk`).

Both make real git repositories and run the provider many times. Under the full gates the machine is loaded, so they
pass the default. Make the plan pack's tests pass reliably under load, without hiding a real slowdown.

## Success criteria

- The two tests, and any other plan-pack test that makes repositories in a loop, have a timeout that fits their work,
  or do less work for the same check.
- A slow test still fails: no timeout is removed or set without bound.
- `pnpm test` passes three times in a row while `cargo test --workspace` runs beside it.

## Tests

The run above is the test. No new test.

## Notes

- Area: `packs/plan/test/` and its vitest config. No product code.
- Prefer one timeout for the tests that make repositories (a per-file or per-describe setting) over a global raise.
- Small and cheap: a candidate for the first dogfood runs through the thin loop once `ts-0083-uf` is done (part L).
- The test rule: no test touches the real home, real launchd or a real daemon.

### Built

- By a Collide run, the first trial on a real build item (owner's choice, 2026-11-16: `collide run <file> --budget
  2.00 --alias smart`, Sonnet): one turn, 9 agent steps, 90 s wall, $0.10 against the $2.00 cap; one commit on the
  run's branch, applied to main in review.
- `packs/plan/vitest.config.ts` (new): `testTimeout` and `hookTimeout` of 30 s for the plan pack. Nine of its twelve
  test files make git repositories through `test/helpers.ts`, so one pack-level bound is the narrow form here. 30 s
  matches the per-test limits that `provider.test.ts` and `write-processes.test.ts` already set. A slow test still
  fails.
- The agent ran the pack's tests and typecheck on an idle machine (12 files, 283 tests). Its load check wrote a log to
  `/tmp` and left `cargo test` detached, so the host denied it, and the agent did not retry.
- Load check, run in review: `pnpm test` at the root passed three times in a row beside `cargo test --workspace`
  (which passed). The first two runs were wholly under it, the third about half.

### Not done, by design

- No per-test or per-file timeouts; no product code.
