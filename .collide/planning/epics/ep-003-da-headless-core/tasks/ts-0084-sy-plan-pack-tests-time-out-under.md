---
id: ts-0084-sy
kind: task
status: ready
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
