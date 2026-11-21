---
id: ts-0165-ab
kind: task
status: ready
title: "Daemon stop: its environment capture shell outlives it"
owners: []
links: []
---

## Goal

Seen while reviewing `ts-0159-ra` (2026-11-21): the app's browser spec `app/browser/bridge.spec.ts` "the bridge
refuses a foreign Host, a foreign Origin and a call without the token" fails about half the time, at HEAD as well. The
fixture's check "nothing of the world is left running" finds two processes of the daemon's login-shell capture
(`base_env.rs`: `fake-shell -l -i -c printf ... env -0 ...`) still alive after the bridge and the daemon stopped. The
capture runs in a session of its own (`setsid`) and is killed after reading, so a capture still in flight when the
daemon stops outlives it. `pnpm test` is therefore red about half the time for any build.

## Success criteria

- When the daemon stops, every environment capture in flight is ended (its session killed) before the daemon exits.
- `bridge.spec.ts` passes 10 runs out of 10, run alone and inside `pnpm test`.
- A capture that finishes normally is unchanged: killed after reading, under its timeout.

## Tests

- Write first and see red: in `host/tests/base_env.rs` style, a capture with a shell that sleeps, then the daemon (or
  the owner of the capture) stops: no process of that shell is left.
- The browser spec above, 10 runs in a row, all pass.

## Notes

- Why two captures start in a test that runs nothing is for the builder to find (each connection, or each run's
  start); if they are needless, starting fewer is part of the fix, not a substitute for the kill on stop.
- Recommendation: track live capture sessions in the daemon and kill them on stop. Alternative: the bridge fixture
  waits for captures. That hides the leak from the test and leaves it in the real daemon.
- Owner decisions: none recorded yet. The recommendations stand until the owner answers.
