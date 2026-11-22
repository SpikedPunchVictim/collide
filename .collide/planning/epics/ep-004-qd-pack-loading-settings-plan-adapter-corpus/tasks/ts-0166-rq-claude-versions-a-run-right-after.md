---
id: ts-0166-rq
kind: task
status: ready
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
