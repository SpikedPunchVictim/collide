---
id: ts-0066-zb
kind: task
status: active
title: "Daemon: permission answers, interrupt, limit pause and resume"
owners: []
links:
  - { type: blocked-by, to: ts-0065-kx }
---

## Goal

The daemon's side of a phase's conversation (`host/src/daemon.rs`, `host/src/phases.rs`):

- **Permission requests.** Pending requests are held per phase and answered through an RPC with `Allow` or `Deny` (with a message). No answer is ever an "always" option.
- **Stop.** A stop sends `interrupt`, then closes stdin, then follows 0.5 (carried from `ts-0059-xs`).
- **Usage limits.** A `limited` event, or a 429 error result, pauses the phase as `paused: usage limit`. The reset time is wall-clock time, and the pause is measured on the continuous clock (0.2's table). The phase resumes by session id (`--resume`, with the budget left) at the reset time, or when the user resumes it (a `run.resume` RPC).
- **Other API errors.** An error result with any other status fails the phase, with the status.

Carried from 3a:

- **A resume with the daemon down.** A paused phase whose reset time passed while no daemon ran is resumed by the next daemon on its own, with no CLI connected. This is 3b's form of "the next phase starts with the app closed" (`ts-0057-ck`). A run here has one phase, so the engine that picks a next phase stays Stage 8's.
- **The login session.** The daemon records its login session, and ending at logout is recorded as host shutdown, not as a crash (`ts-0056-ag`; the daemon's start, step 2).

## Success criteria

- A recorded `result/success` with `is_error: true` is not taken as the phase's report: a 429 pauses the phase as a usage limit, and another status fails it. A recorded budget-error result's cost counts toward the phase (Stage 3b criterion).
- A pending permission request survives a daemon restart, rebuilt by replay from the journal, and can be answered after it.
- A stop sends `interrupt` first. A fake Claude that ends its turn on `interrupt` ends `Stopped` with no signal sent. One that ignores it gets stdin closed, and then 0.5's order.
- A limit pause resumes at the reset time read on the wall clock. A reset that passed during an injected sleep resumes on wake, without waiting out the rest of a countdown (0.3, The reset time is a wall-clock time).
- A paused phase whose reset passes while the daemon is down is resumed by the next daemon, with no CLI connected.
- Ending at logout is recorded as host shutdown, tested with an injected logout. The real one is part of the owner's restart check in `ts-0071-fd`.

## Tests

Write these first:

- limit pause and resume: a recorded limit message, a reset time from a fake wall clock, the resume by session id, and the budget left on `--max-budget-usd`;
- the 429, and another status;
- the permission answer over the RPC, and after a daemon restart;
- the stop order, with `interrupt` first.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 3b: permission requests answered from the CLI (the daemon's side); the `limited` event, pause and resume.
  - 0.3: Permissions; Usage and rate limits; The stream protocol (stdin must stay open; stopping a phase sends `interrupt`, then closes stdin).
  - 0.2: the clock table; Starting the daemon (the login session).
  - 0.5: the stop order.
  - S1 record: the stop order (interrupt, close stdin, SIGTERM gives exit 143).
- Timers come from `ts-0051-wh`'s clocks: uptime for work timeouts, continuous time for the pause's length, and wall time for the reset.
- 0.3 also pauses the account's other runs before their next turn. Stage 3b's text does not ask for it, so it is left out here.
- Area: `host` (`daemon.rs`, `phases.rs`, `stop.rs`), plus `core` for the RPC types.
- The test rule: no test touches the real home, real launchd or a real daemon. Each test clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as `host/tests/common/mod.rs` `TempRoot` does. Settings files are fixtures, never the owner's own `settings.json`.
- Stage 3b item (revision 18 scope, split 2026-11-15). Items of Stage 3b run one at a time, in id order, because they share `host` and the generated files.
