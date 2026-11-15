---
id: ts-0074-cu
kind: task
status: ready
title: "Collide's tests pass inside a Collide run"
owners: []
links:
  - { type: blocked-by, to: ts-0073-ac }
---

## Goal

In the dogfood run of `ts-0070-mk`, the agent ran Collide's own `cargo test --workspace` inside its phase and got 183 failures. Two causes:

- About 100 tests start runs, and the daemon refused them with "may not start a run": the test process inherits the agent's `COLLIDE_RUN`, `COLLIDE_RUN_START` and `CLAUDE_CODE_AGENT`, so the test's client looks like a run's own agent (ts-0066-zb's refusal of `Caller::RunAgent`).
- About 85 tests could not find `node`: the phase's `PATH` had no `/opt/homebrew/bin`. That daemon was started by a build agent with a cleared environment, so this half may not happen under a real daemon; `ts-0071-fd` checks it.

Make the first cause go away so a Collide run can build Collide (part L). A test that starts a run, or talks to a daemon as a person, must not carry the run variables of the process that runs the tests.

## Success criteria

- `cargo test --workspace` passes with `COLLIDE_RUN`, `COLLIDE_RUN_START`, `COLLIDE_SPAN` and `CLAUDE_CODE_AGENT` set in its environment, as they are inside a phase. A test pins this: it runs a representative test binary as a child with those variables set and checks it passes.
- The fix is in the test helpers (`host/tests/common/mod.rs` and the fake-Claude helpers), not a weakening of the daemon's refusal: a real run's agent is still refused `run.start_file` and `queue.answer` (ts-0066's test stays).
- If a test needs `node` and cannot find it, it fails with a message that says so, not a confusing error.

## Tests

Write these first:

- the child-run check above, red before the fix.

## Notes

- `ts-0070-mk` decision 9.
- IMPLEMENTATION_PLAN.md: part L (Dogfood); 0.3 (the context variables after the strip); 0.2 (a run's agent may not start a run).
- Area: `host/tests/` helpers; the client's caller detection in `host/src/owner_client.rs` if it reads the process environment directly.
- The test rule: no test touches the real home, real launchd or a real daemon. Each test clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as `host/tests/common/mod.rs` `TempRoot` does.
