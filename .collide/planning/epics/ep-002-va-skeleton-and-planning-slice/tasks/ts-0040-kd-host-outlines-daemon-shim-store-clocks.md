---
id: ts-0040-kd
kind: task
status: active
title: "host outlines: daemon, shim, store, clocks, OTLP"
owners: []
links:
  - { type: blocked-by, to: ts-0019-vm }
---

## Goal

Outlines in `host` as signatures with doc comments and tagged stubs: `collide-daemon` (on-demand start under a lock file, the one-shot launchd job with the `setsid` fallback, the base-environment capture, the socket server, adoption of existing shims), the `collide shim` mode (`setsid` spawn, `wait4`, the journal writer, the atomic `exit.json` write, the PTY ring), the store (SQLite and the event log, migrations, quarantine), a `clock` module that declares the three clocks of the 0.2 clock table (uptime, continuous, wall) as types a timer must name, and the OTLP receiver's declaration (`127.0.0.1`, a `Host` check, a per-run bearer token, a body cap) as a stubbed module.

## Success criteria

- Every stub carries a tag that belongs to a stage. The daemon, shim, store and clock stubs belong to Stage 3a, and the OTLP stub to Stage 9b.
- The clock module has exactly the three clocks, with a doc comment per use (work timeouts, the cache countdown and elapsed spans, wall display and limit reset), and a test that the doc table and the enum agree.
- `cargo build` and `cargo clippy` pass.

## Tests

Write first: the stub-count check over these tags; the clock enum and doc table agreement test.

## Notes

- IMPLEMENTATION_PLAN.md, 0.2 (the shim, starting the daemon, the base environment, Clocks, Durability, the OTLP receiver), 0.13, 0.15 (Tagged stubs), Stage 2: Skeleton (`host` outlines: `collide-daemon` with its launchd start, `collide shim`, the store).
- Area: `host`.
- Gap to report: the plan's Stage 2 text does not name the clock table or the OTLP receiver. They are placed here as outlines because 0.2 puts both in the daemon.
- Done when the criteria hold.
