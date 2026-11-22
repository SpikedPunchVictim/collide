---
id: ts-0174-wf
kind: task
status: ready
title: "GitHub pack: watch, ETags, the full reconcile and removals"
owners: []
links:
  - { type: blocked-by, to: ts-0173-ku }
---

## Goal

An edit on GitHub reaches the corpus within a poll, a deleted or transferred issue leaves it within a reconcile, and a
token that expires mid-run makes the source "could not check" with nothing removed.

What exists. `ts-0172-xc` gives the client with ETag and `If-None-Match` support, the cause classes and the rate-limit
headers. `ts-0173-ku` gives `list`, `get` and the task mapping. `core/src/provider.rs` has `task.changed` (with `from`,
`to`, `diff`, `cause: remote`), `task.removed` (`Deleted`, `Transferred`, `Unknown`) and `source.stale`. The daemon
applies `task.changed` with a `get` when the diff names more than the status (`apply_changed` in
`core/src/corpus.rs`), relists a stale source with a backoff, and at start lists every source in full. Core's startup
list already reconciles the persisted cache against the pack's `list` (`reconcile_by_source`, cause `unknown`), so
start-time removal of what is gone is done by `list` being a full read (revision 22, plan section 0.2).

What this adds.
- `watch`: one poll loop per repo, every `pollSeconds`. It lists `state=all&sort=updated&direction=desc` with
  `since` set to the newest `updated_at` seen less one second, and sends
  `If-None-Match` with the ETag of the first page. A 304 (free against the rate limit) changes nothing. An issue
  whose `updated_at` differs from the pack's known version gives a `task.changed` with `cause: remote`,
  `from` and `to` states, and a `diff` over status, title, labels and assignees; a new issue has
  `from: null`. The pack holds the known versions in memory, seeded by `list`.
- A full reconcile every `reconcileSeconds`, and at `watch` start when the last one (a timestamp in a file in
  `COLLIDE_PACK_DATA`) is older than the period. It re-lists the window of `ts-0173-ku` for every repo and, for each
  known key that is absent, asks `GET /repos/<repo>/issues/<n>`: 404 or 410 gives `task.removed` `Deleted`; 301 gives
  `Transferred`; 200 (the issue left the window, closed long ago) gives `Unknown`. A key that is present and changed
  gives a `task.changed`. A reconcile that could not read every repo emits nothing and records no time.
- Pacing from the rate-limit headers: below 50 remaining, the interval doubles until the reset; at zero, the source is
  stale with cause `rate_limit` and polling resumes at the reset.
- Failure: a 401, a network error or a missing repo sends `source.stale` with the cause class and the fix, once, and
  pauses polling. The daemon's relist (with its backoff) is the retry; a successful `list` resumes polling. Nothing is
  removed while it is paused.
- `actor` is left off `task.changed`: the issues list does not say who edited, and the events API costs a request for
  each issue.

## Success criteria

- A fixture issue edited on the fake API (title, label, close) gives one `task.changed` within one poll, with cause
  `remote`, the right diff and the new version. No change gives no notification.
- After the first poll the next one sends `If-None-Match`, the fake answers 304, and nothing is emitted.
- A new issue arrives with `from: null`.
- A deleted issue, a transferred one (the fake answers 301) and one that left the window give `task.removed` with
  `Deleted`, `Transferred` and `Unknown`, within one `reconcileSeconds`. A repo that fails during the reconcile
  removes nothing.
- A reconcile is due at start when the stored time is older than the period, and runs before the first poll; when it
  is younger, none runs until it is due.
- An expired token mid-watch (the fake API answers 401) sends one `source.stale` with cause `auth`, emits no removal,
  stops polling, and resumes after a successful `list`.
- A rate limit near zero slows polling and at zero marks the source stale with cause `rate_limit`.

## Tests

Write these first.

- `packs/github/test/` poll and reconcile tests with `vitest` fake timers and the fake API: edit, no change, 304, new
  issue, each removal cause, a failing repo, the due-at-start rule with a stored time, the token expiry and the resume,
  the rate-limit pacing. The loop takes its clock and timers as parameters of the provider's factory, so a test needs no
  real wait.
- A stdio test of `watch` with `pollSeconds: 1` and the fake API (a real wait of a few seconds, at most).
- A daemon test in `host/tests/`: the real pack with `pollSeconds` and `reconcileSeconds` small, an edit shows in
  `task.list`, a delete leaves it, and an expired token marks the source stale and leaves the tasks.

## Notes

- Area: `packs/github/src/` (a poll and a reconcile module), `packs/github/test/`, `host/tests/`.
- The test rule: no test touches the real home, real launchd, a real daemon, the keychain or the network. Each test that
  runs the `collide` binary clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp
  root, as `host/tests/common/mod.rs` `TempRoot` does. A temp repository sets `GIT_CONFIG_GLOBAL` and
  `GIT_CONFIG_SYSTEM` to empty files. No test, build or builder runs the real `gh`, reads a real token or reaches
  GitHub: `gh` is a script in the test's own temp folder, first on the `PATH` the test builds, and the API is the fake
  server of `packs/github/test/support/` on a loopback port.
- `updated_at` has one-second granularity, so two edits in one second have one version and a write between the
  conflict check and the write is not seen (plan section 0.10 states the race). The one-second overlap of `since`
  and the known-version map make a repeat harmless. It is not removed here.
- Defaults are a recommendation: poll 30 s (a 60 s criterion has room for one slow poll), reconcile 3600 s. They are
  settings of the pack, per `ts-0172-xc`. The 60 s criterion itself is measured by the owner in `ts-0177-hd`.
- Stage 5 items run one at a time, in id order. Nothing here runs a real `claude`. No dependency is added.
- Owner decisions: none recorded yet. The recommendations stand until the owner answers at the split.
