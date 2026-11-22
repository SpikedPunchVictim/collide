---
id: ts-0165-ab
kind: task
status: done
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

### Built

The daemon's stop now ends every environment capture in flight. All in `host/`.

**The cause.** Each capture runs `$SHELL -l -i -c ...` in a session of its own (`setsid`) and kills that session after
reading (`base_env::run_login_shell`). The daemon's stop did not know about it: `Daemon`'s drop joined the accept,
sampler and write threads and stopped the sources, the runs and the claude refresher, but the capture runs on a detached
thread (`collide-env-capture`) or on a connection's thread (`daemon.recapture_environment`), and neither was joined or
told. A process that ends takes no other session with it, so a capture still reading when the daemon went left its
shell, started by a slow launch, running in a session nobody owned.

**Why two start in a test that runs nothing.** The bridge's event pump (`app/src-tauri/src/shell.rs`,
`follow_daemon`) starts the daemon when none answers and then, on its first connection, sends
`daemon.recapture_environment` (plan 0.2, The base environment: the app captures again when it starts). The daemon had
already begun its own capture at start (`Daemon::start`, thread `collide-env-capture`). Both run at once, so a test
that stops the world right after it starts finds two. Reproduced with the old behavior: 3 of 10 runs of the browser
spec failed with exactly two `fake-shell -l -i -c printf ... env -0 ...` processes left.

**The change.**

- `host/src/base_env.rs`: `CaptureSessions` holds the pid (the group id) of each shell not yet waited for, and an
  `ended` flag, under one lock. `end_all` sets the flag and kills each session. `release` kills one session and
  forgets it under the same lock, before the shell is waited for, so `end_all` never signals a group whose leader was
  reaped. `BaseEnvironment::capture_in` is `capture_with` with a `CaptureSessions` given; `capture_with` passes a
  fresh one, so its callers are unchanged. A capture that comes after `end_all`, or that `end_all` cut short, returns
  the minimal environment with the reason "the daemon is stopping, so the capture was ended". A shell is not started
  after `end_all`; one started in the gap is killed at once. A capture that finished is returned as before.
- `host/src/daemon.rs`: `Shared.captures` is a `CaptureSessions`; `Shared::capture_environment` uses `capture_in`; the
  drop of `Daemon` calls `captures.end_all()` right after it sets the stop flag, ahead of the joins. The module doc's
  stop section says so.

**Tests.** `host/tests/base_env.rs`: `ending_the_sessions_kills_a_capture_that_is_still_running`,
`a_capture_that_comes_after_the_sessions_ended_starts_no_shell`,
`a_capture_that_finishes_before_the_sessions_end_is_unchanged`. `host/tests/daemon_socket.rs`:
`the_captures_in_flight_are_ended_when_the_daemon_stops` (the start capture and a `daemon.recapture_environment` in
flight, then `Daemon::stop`: both shells gone). Each test kills its own shells in a `Drop`, and waits on a condition
with a 15 s deadline.

**Seen red at runtime.** `the_captures_in_flight_are_ended_when_the_daemon_stops` against the daemon as it was: two
shells started, both still running after the stop. The first two tests of `base_env.rs` with `end_all` stubbed to do
nothing: the first failed with "the shell of the capture is still running", the second with a `LoginShell` where the
minimal environment was expected. The third passes both ways (it guards the unchanged path). The browser spec with
only the `end_all` call in `Daemon`'s drop removed: 3 of 10 runs failed with the two shells left. With the fix:
10 of 10 pass (`pnpm --filter @collide/app exec playwright test browser/bridge.spec.ts`, ten runs in a row).

### Decisions for the owner

- **Both captures still start.** The notes say that needless captures should be started fewer. These are not needless
  in general: the daemon's start capture serves a daemon that the CLI or launchd started long before the app, and the
  app's request is R5 of the plan. They are redundant only when the app has just started the daemon itself. I kept
  both and fixed the leak, which is the cause and also covers the real daemon. Alternative: the pump skips its
  `daemon.recapture_environment` when its own connect started the daemon. That needs `DaemonStarter::ensure_running`
  to say whether it started one, and changes the pump tests in `app/src-tauri/tests/daemon_link.rs`. Another:
  `Shared::capture_environment` joins a capture already in flight instead of starting a second, which makes a request
  that arrives mid-capture see the environment as of the capture's start. Recommendation: neither now; take the first
  as its own item if the doubled start cost matters. No plan text change.
- **The track-and-kill design the item recommended**, over the bridge fixture waiting for captures. The fixture would
  hide the leak and leave it in the real daemon.
- **Only a clean stop is covered.** A daemon killed with `SIGKILL` cannot end its captures. The capture's own timeout
  (`CAPTURE_TIMEOUT`, 10 s) and the shell's end of output limit what is left. Alternative: start the capture's shell in
  the daemon's session and rely on the session's end. Rejected: it gives up the group kill the capture depends on.

### Not done, by design

- Fewer starts (see the first decision): left for a later item, if the owner takes the alternative.
