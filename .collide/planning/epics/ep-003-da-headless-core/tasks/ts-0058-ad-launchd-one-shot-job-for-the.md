---
id: ts-0058-ad
kind: task
status: ready
title: "launchd one-shot job for the daemon"
owners: []
links:
  - { type: blocked-by, to: ts-0056-ag }
---

## Goal

The one-shot launchd job that starts the daemon outside its launcher's coalition on macOS: a temporary plist with absolute, canonical paths that sets `USER`, `HOME` and `PATH`, bootstrapped with `launchctl bootstrap gui/<uid>` and removed once the daemon runs. When bootstrap fails (over SSH or with no GUI session), the start falls back to `setsid` and prints a warning that the daemon may end with its launcher. Replaces the stubs tagged `daemon: launchd job`.

## Success criteria

- The daemon started by the one-shot launchd job has `USER`, `HOME` and `PATH`, and a python3 child runs under it (Stage 3a criterion). The real-`claude` check is in Stage 3b.
- The temporary plist is gone once the daemon answers.
- A bootstrap failure, forced in a test, starts the daemon with `setsid` and prints the warning. The run is not refused.
- The plist text is pinned by a test: every path absolute and canonical, and the three variables present.

## Tests

Write first: the plist text; the forced bootstrap failure and its warning. One real launchd test, run on macOS with a GUI session and skipped with a printed reason elsewhere (CI has no GUI session).

## Notes

- IMPLEMENTATION_PLAN.md, Stage 3a; 0.2 (Starting the daemon: the macOS coalition, the launchd job, the S2 measurements, the SSH fallback). herdr-core's `macos_launchd.rs` was read for the approach.
- Area: `host` (`launch.rs`).
- Stage 3a item (revision 13 scope, split 2026-11-15). Items of Stage 3a run one at a time, in id order, because they share `host` and the generated files.
