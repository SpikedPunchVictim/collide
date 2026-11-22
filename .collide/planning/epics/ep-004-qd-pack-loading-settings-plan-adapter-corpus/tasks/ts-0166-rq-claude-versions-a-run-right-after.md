---
id: ts-0166-rq
kind: task
status: done
title: "Claude versions: a run right after the daemon starts says it does not look up the published version"
owners: []
links: []
---

## Goal

Seen while reviewing `ts-0163-su` (2026-11-21): the test
`run_command::a_task_file_run_prints_both_versions_then_follows_the_phase_and_prints_its_record`
(`host/tests/adapters/claude/run_command.rs`) fails often, at HEAD as well, with
"claude published: not known (this daemon does not look it up)" where it expects the published versions. The daemon
sets its published-version source only once its start capture of the environment has ended (`Daemon::start`,
`claude_published.source(captured.vars)`), and `ClaudeVersions::status` answers "this daemon does not look it up"
(`NO_SOURCE`) whenever no source is set. A `collide run` that asks `daemon.agent_versions` before the capture ends
is told the daemon never looks it up, which is wrong: it will, once the capture ends. Slow process launches widen the
window, so the test fails most runs on a slow machine.

## Success criteria

- While the daemon's start capture is still running, `daemon.agent_versions` says the published version is not known
  yet, not that the daemon does not look it up. `collide run` prints "claude published: not known yet".
- A daemon with no source at all (`PublishedChoice::Off`) still says it does not look it up.
- The test above passes 10 runs in a row, run alone.

## Tests

- Write first and see red: a daemon whose start capture is held (a shell that waits) and a `PublishedChoice::Source`:
  `daemon.agent_versions` during the capture says "not known yet".
- The existing test: it waits for the published versions, or the CLI's version line is checked against both answers;
  the builder chooses, and says which.

## Notes

- Recommendation: the versions state knows a source is coming (set from the choice at start, before the capture), so
  `status` tells "not known yet" from "not looked up". Alternative: make the source before the capture with the
  daemon's own environment. That changes which proxy the registry fetch uses until the capture ends.
- Low priority: it shows only to a run in the first seconds of a daemon's life.
- Owner decisions: none recorded yet. The recommendations stand until the owner answers.

### Built

Taken on the owner's delegation: the versions state knows a source is coming, so `status` tells "not known yet" from
"not looked up". The setting `claude.check_published: false` still hides the chip.

- `host/src/claude_versions.rs`: `State` has `source_coming`. `ClaudeVersions::expect_source` sets it.
  `ClaudeVersions::status`, with no source set yet, answers `error: None` (the CLI prints "claude published: not known
  yet") when a source is coming, and `NO_SOURCE` when none is. With the setting off it answers no chip, as before.
- `host/src/daemon.rs`, `Daemon::start`: calls `claude.expect_source()` when `claude_published` is not
  `PublishedChoice::Off`, before the capture thread starts.
- `host/src/run_cli.rs` is unchanged: `print_versions` already waits up to `PUBLISHED_WAIT` while the chip is pending.

What a call can see, found while writing the test: `daemon.agent_versions` waits for the start capture itself
(`read_claude_version(capture.timeout + margin)`), so it cannot answer during a held capture. The window it sees is
after the capture ends and before the start thread sets the source: the start thread's first `claude --version`.
The test holds that read.

Tests:

- `daemon_socket::a_version_check_right_after_the_start_capture_says_the_published_version_is_not_known_yet`: a fake
  `claude` whose first `--version` waits for a release file; a `PublishedChoice::Source`; the call says the chip is
  shown, no versions, no error text; after the release the versions arrive. Seen red at runtime (old code answered
  `this daemon does not look it up`), and red again with `expect_source` stubbed to a no-op.
- `daemon_socket::a_daemon_with_no_published_source_says_it_does_not_look_it_up_right_after_the_start_capture`: the
  same window with `PublishedChoice::Off` still says `NO_SOURCE`. A guard: it passed before the change. The daemon
  with `Off` after the start is also covered by
  `agent_setup::a_daemon_given_no_source_makes_no_fetch_and_the_published_chip_says_so`.
- `published::with_a_source_coming_the_chip_says_it_is_not_known_yet_and_no_fetch_is_made`: seen red at runtime with
  `expect_source` stubbed.
- `published::with_a_source_coming_and_check_published_false_the_chip_is_hidden` and
  `published::a_source_given_after_one_was_expected_is_fetched_as_before`: guards, green with the stub too.
- `run_command::a_task_file_run_prints_both_versions_then_follows_the_phase_and_prints_its_record`: now waits, with a
  120 s deadline, until `daemon.agent_versions` has the published versions (`wait_for_published_versions`) before it
  runs the command. What it checks after that is unchanged. Run alone with
  `cargo test --offline -p collide-host --test claude_adapter a_task_file_run_prints_both_versions -- --test-threads=1`
  ten times in a row: 10 of 10 passed.

### Decisions for the owner

- The flag lives in `ClaudeVersions` and the daemon sets it (`expect_source`). Alternative: a field of
  `VersionsOptions` set at construction. A method keeps `VersionsOptions::new` and its callers as they are.
  Recommendation: the method.
- The existing test waits for the published versions before the run. Alternative: check the version line against both
  answers ("not known yet" or the versions). That accepts a wrong line when the fetch is merely slow, and it weakens
  what the test shows. Recommendation: wait, as done.
- A run that asks in the window still waits only `PUBLISHED_WAIT` (8 s) in `collide run`, then prints "not known yet"
  and goes on. That is now true, not wrong. A longer wait is not needed. Recommendation: leave it.
- Proposed plan text: none. The change keeps 0.3, Two version chips.

### Not done, by design

- No change to `collide run`'s wait (`PUBLISHED_WAIT`) or to when the source is made. The alternative in Notes,
  making it before the capture with the daemon's own environment, would change which proxy the first fetch uses.
