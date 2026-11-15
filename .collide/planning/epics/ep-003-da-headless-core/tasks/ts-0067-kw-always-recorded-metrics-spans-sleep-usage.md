---
id: ts-0067-kw
kind: task
status: ready
title: "Always-recorded metrics: spans, sleep, usage and fingerprint"
owners: []
links:
  - { type: blocked-by, to: ts-0066-zb }
---

## Goal

The always-recorded metrics of 0.13 for a run, kept in the store by the daemon:

- **Spans** for runs, phases, turns and waits, with active time kept apart from waiting time. Waits are permission waits, usage-limit pauses and machine sleep.
- **The sampler's tick** (`host/src/sampler.rs`, `Sampler::tick`), carried from `ts-0051-wh`. It reads a `ClockPair` each second. A gap over `SLEEP_THRESHOLD` is recorded as a sleep interval and counted as waiting.
- **Tokens and cost** per turn and per phase, from `ts-0064-hw`'s usage events. Input, output, cache reads and cache writes are kept, plus compaction calls and other model calls.
- **Time per turn.** Wall time, and model time.
- **The token and context metrics** of 0.13: context size, compactions, cold returns and tool-result bytes.
- **The count of `Other` events** per run.
- **At phase end**, the shim's exit totals and the disk space of each workspace.
- **The always-on fingerprint.** Its user-config part comes from `ts-0061-bg`, and `ts-0068-wx` adds the `claude` version.

Replaces the stubs tagged `sampler: tick and sleep`.

## Success criteria

- A run's trace shows its phase and permission-wait spans, with the wait counted as waiting. Its tokens and cost match `result.usage` per turn plus its compaction calls, with no content block counted twice (Stage 3b criterion; S9).
- A recorded limit message pauses the phase with its reset time, and the phase resumes by session id. The pause counts as waiting (Stage 3b criterion).
- A sleep injected through fake clocks under a running phase is recorded as a wait, not as active time (0.2; carried from 3a).
- A value the agent did not report is stored as null, never 0. A total built from partial figures is marked as "at least" and never summed with exact ones (0.13).
- **Store appends.** The item measures the cost of one store connection per call at the fake-Claude event rate, then keeps or changes it, and records the numbers (follow-up from `ts-0053-gn` and `ts-0056-ag`).
- No stub tagged `sampler: tick and sleep` remains. The tag moves to `FILLED_TAGS`.

## Tests

Write these first:

- span-tree tests against fake-Claude streams: run, phase, turns, a permission wait, a limit pause and a sleep interval;
- missing values stored as null;
- the sampler tick over fake clocks: no sleep, a gap under 2 s, and a gap over 2 s;
- token sums against `result.usage` plus compactions.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 3b: Always-recorded metrics.
  - 0.13: spans; active and waiting time; Sleep is not work; tokens and cost; model time; the context metrics; the fingerprint; the detectors.
  - 0.2: the clock table.
  - S9 record: usage counting. S8 record: the sleep measurement.
- The owner's real sleep test is `ts-0071-fd`. This item gives it what it reads: the sleep spans, and active time with sleep taken out.
- Area: `host` (`sampler.rs`, the store's migrations for spans and usage, `daemon.rs`).
- The test rule: no test touches the real home, real launchd or a real daemon. Each test clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as `host/tests/common/mod.rs` `TempRoot` does. Settings files are fixtures, never the owner's own `settings.json`.
- Stage 3b item (revision 18 scope, split 2026-11-15). Items of Stage 3b run one at a time, in id order, because they share `host` and the generated files.
