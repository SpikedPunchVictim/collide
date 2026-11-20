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
read as "expected a commit id, got "", failing a `task_daemon_cli` test that passed in the round before and alone.
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

### Built

- `host/src/bounded_run.rs`: `Captured` has `stdout_cut` and `stderr_cut`. A pipe not read to its end within
  `PIPE_GRACE` after the process ends is flagged, and its bytes are empty (the reader sends only at the end, so nothing
  partial is kept). `PIPE_GRACE` is unchanged. New `cut_message(pipe)` is the one sentence callers use (its words are
  under At review).
- Callers of `run_bounded` in `host/src`, each:
  - `git_objects.rs` (`run_ok`, `commit_of`): reads stdout. A cut stdout is `GitError::Run` with the cut message
    (`stdout_read`), never "expected a commit id". `git init` and `git add` read no stdout: new `run_ok_unread` keeps
    today's behaviour for them. `GitError::Run` text is now "could not be run to a result" (it also covers a cut).
  - `adapters/claude/version.rs`, `read_version_within`: reads stdout. Cut is `VersionError::NotRun` with the cut
    message, never `Unreadable`. `smoke_check_within`: reads stdout (the stream). Cut is `SmokeOutcome::Failed` with the
    cut message, not "no `result` line". It was not named in the item; it reads stdout, so it is covered.
  - `board_load.rs`, `run_tool`: reads stdout, and stderr as a fallback for the version. Cut stdout is an error. Cut
    stderr is an error only when no version was found, so a tool that prints its version on stdout and has a grandchild
    holding stderr works as before.
  - `http_get.rs`, `CurlGet::get`: reads stdout as the body. Cut stdout with exit 0 is an error with the cut message,
    not an empty body.
  - `http_get.rs`, `SystemProxy::proxy` (`scutil --proxy`): reads stdout, returns `Option`. Left as it is: a cut
    reading is "no proxy found", as an empty one was (see Decisions).
  - `git_fetch.rs` (`init --bare`, `fetch`): reads no stdout, only the stderr tail of a failure. Unchanged.
- Tests (all `Grandchild`, a guard in `host/tests/common/mod.rs` that kills the test's own sleeper on drop, also on a
  panic; each about 2 s):
  - `bounded_run.rs`: `a_stdout_a_grandchild_holds_open_is_flagged_cut`,
    `a_stderr_a_grandchild_holds_open_is_flagged_cut_and_stdout_is_not`,
    `a_run_whose_pipes_close_has_neither_pipe_flagged`.
  - `git_objects.rs`: `a_git_whose_stdout_a_grandchild_holds_open_fails_saying_the_output_was_not_read_in_time` (the
    caller test, through a stand-in `git` on `PATH`),
    `a_git_add_whose_stdout_a_grandchild_holds_open_still_hashes_the_folder`.
  - `adapters/claude/version.rs`: `a_version_whose_stdout_a_grandchild_holds_open_is_not_read_as_empty`,
    `a_smoke_check_whose_stdout_a_grandchild_holds_open_says_so`.
  - `adapters/claude/published.rs`: `a_curl_whose_stdout_a_grandchild_holds_open_is_an_error_not_an_empty_body`.
  - `board_load.rs`: `a_tool_whose_stdout_a_grandchild_holds_open_is_a_problem_not_a_missing_version`.
- Red at runtime (flags stubbed to `false`, the old behaviour): the two cut-flag tests, and each caller test failed with
  the old reply: git "expected a commit id, got \"\"", version `Unreadable { output: "" }`, smoke "the stream held no
  `result` line", curl `Ok([])`, tool "printed no version (it printed: )". The neither-flagged test passes before and
  after, as it should. The `git add` test was red when `git add` was moved to `run_ok` (mutation), green restored.

## Decisions for the owner

- A cut stdout is flagged on `Captured` and read by each caller, as the item recommends, not an error from
  `run_bounded`. Alternative: an error from `run_bounded`; it would break callers that ignore stdout with a
  grandchild holding the pipe (`git init`, `git add`, `git_fetch`). Recommend the flag.
- The cut stdout's bytes stay empty. Alternative: keep what was read so far; it needs a shared buffer in the reader and
  a partial reply is as easy to misread as an empty one. Recommend empty, with the flag.
- The cut is a `GitError::Run`, not a new variant. Alternative: a `GitError::Cut`; no caller tells them apart.
  Recommend `Run`.
- The scutil proxy lookup keeps `None` on a cut stdout. Alternative: `ProxySource::proxy` returns a `Result` so a cut
  fails the get; it changes a trait with three implementors for a call that runs a system tool with no hooks. Recommend
  as it is; a missed proxy ends in a visible curl error.
- `smoke_check_within` was added to the callers that fail on a cut (the item named four).

## Not done, by design

- `host/tests/adapters/claude/real.rs` and `repo_instructions_live.rs` call `run_bounded` in opt-in real-claude tests
  and do not read the cut flags. Not changed: they never run in the gates.
- A stderr tail in a failure message is empty when stderr was cut; the exit code still says the run failed. Not
  flagged in those messages.
- Plan text, if wanted (4b bullet on bounded runs, or 0.5): "A bounded run that ends while a process it started holds
  its output open gives the reply that was read, flagged cut; a caller that reads that pipe fails saying the output was
  not read in time, and never reads it as empty."

### At review

- Decisions taken as recommended (owner delegation): the flag on `Captured`, empty bytes for a cut pipe,
  `GitError::Run`, `scutil` left as no proxy, the smoke check added.
- Changed at review: `cut_message` no longer names one cause. The failure this item came from was most likely a reader
  not scheduled in time under load, with no grandchild at all, so the words are now "its stdout was not read in time: a
  process it started may still hold it open, or the machine was too busy", and the `Captured` doc names both causes. The
  tests check only "not read in time".
- Added at review: `a_git_write_tree_whose_stdout_a_grandchild_holds_open_fails_saying_so_not_with_a_bad_tree_id`. The
  cut check in `run_ok` (every git reply but `rev-parse`) had no test: removing it left the suite green.
- No plan text: the plan does not describe bounded runs; the rule is in `bounded_run.rs`.
- Mutations, each red then restored: stdout never flagged, stderr never flagged, `run_ok` not checking (red only with
  the test added at review), `rev-parse` not checking, `git add` through `run_ok`, the version read, the smoke check,
  curl and the tool version run each not checking.
