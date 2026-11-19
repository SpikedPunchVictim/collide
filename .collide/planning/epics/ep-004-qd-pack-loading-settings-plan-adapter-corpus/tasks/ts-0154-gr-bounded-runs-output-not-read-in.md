---
id: ts-0154-gr
kind: task
status: ready
title: "Bounded runs: output not read in time is never taken as empty output"
owners: []
links: []
---

## Goal

`run_bounded` (`host/src/bounded_run.rs`) gives each pipe's reader a grace of 2 s (`PIPE_GRACE`) after the child ends,
so a grandchild that keeps a pipe open (a hook, an ssh master) cannot hold the daemon. A reader that has not delivered
by then gives empty output (`unwrap_or_default`), with the child's real exit code. A caller that reads stdout then takes
missing output for the reply: in the gates of `ts-0148-cv` (2026-11-19, load about 28), `git rev-parse` exited 0 and was
read as "expected a commit id, got \"\"", failing a `task_daemon_cli` test that passed in the round before and alone.
Output that was not read to its end must never look like an empty reply.

## Success criteria

- `Captured` says which pipes were not read to their end within the grace.
- Each caller that reads stdout (`git_objects`, the board's tool runs in `board_load`, the claude version probe,
  `http_get`) fails on a cut stdout with a message that says the output was not read in time, never as the reply it
  would parse. A caller that reads no stdout keeps today's behaviour.
- The grace stays as it is: a grandchild that holds a pipe still does not hold the run
  (`a_grandchild_that_keeps_the_pipe_open_does_not_hold_back_the_exit_record` in `shim_child.rs` keeps passing).

## Tests

- Write first and see red at runtime: a child that ends while a grandchild it started holds stdout open past the grace
  (today: code 0, nothing says the output was cut). The grandchild is the test's own and the test stops it.
- A caller (`git_objects`' commit lookup, through a stand-in git on `PATH` if that is how its tests run git) fails with
  the cut message rather than "expected a commit id".

## Notes

- Recommendation: a per-pipe cut flag on `Captured`, read by each caller, rather than an error from `run_bounded`
  itself, so callers that ignore stdout keep working with a grandchild holding the pipe. The alternative, a longer
  grace, only makes the failure rarer.
- Not proven that the grace caused the failure (no trace was kept); the item makes the failure say what happened either
  way, and a test failing this way again will show it.
- Owner decisions: none recorded yet. The recommendations stand until the owner answers.
