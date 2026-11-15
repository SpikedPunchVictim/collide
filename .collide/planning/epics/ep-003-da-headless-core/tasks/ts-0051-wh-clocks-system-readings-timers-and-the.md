---
id: ts-0051-wh
kind: task
status: active
title: "Clocks: system readings, timers and the sleep gap"
owners: []
links: []
---

## Goal

0.2's clock table behind the `Clocks` trait of `host/src/clock.rs`: real readings of the uptime clock (stops in sleep), the continuous clock (counts sleep) and wall time; timers that refuse a reading from another clock; and the sleep gap between two paired readings. Replaces the stubs tagged `clock: system readings`, `clock: timer` and `clock: sleep gap`.

## Success criteria

- A sleep gap injected through fake clocks counts as waiting, not active time, and a work timeout counts uptime only (Stage 3a criterion).
- A timer started on one clock and read with a reading of another is `ClockMismatch`. Work timeouts use uptime, the cache countdown uses continuous time, and the usage-limit reset uses wall time, as the table says.
- Real readings never go backwards over repeated reads, and uptime and continuous time advance together while the machine is awake (a short real test, no sleep needed).
- No stub tagged `clock:` remains.

## Tests

Write first: fake-clock tests for the gap and for each timer kind, the mismatch refusal, and one real-clock test of monotonic readings.

## Notes

- IMPLEMENTATION_PLAN.md, Stage 3a (Clocks and disk); 0.2 (Clocks, the table with the macOS calls for each clock); 0.13 (sleep intervals counted as waiting).
- Area: `host` (`clock.rs`). The owner's real sleep test is Stage 3b, not here.
- Stage 3a item (revision 13 scope, split 2026-11-15). Items of Stage 3a run one at a time, in id order, because they share `host` and the generated files.
