---
id: ts-0188-kb
kind: task
status: ready
title: "Host: phase heads and a task's changes"
owners: []
links:
  - { type: blocked-by, to: ts-0187-hb }
---

## Goal

Every run records the head of its workspace when each phase starts and ends (0.8). The daemon answers, for a task,
its changes: each run that worked on it, with where and how, and the files changed with their counts; and the
commits made by hand that name its id. A file's diff comes on its changes or as the whole file. No UI here;
`ts-0189-fg` draws it.

What exists.
- `PhaseRun.start_head` and `end_head` (`core/src/domain.rs`) and `HistoryEntry.start_head` and `end_head`
  (`core/src/hook.rs`) exist and are never set.
- `run.diff` (`core/src/owner/run.rs` `RunDiffParams`; `run_diff` in `host/src/daemon.rs`): a prompt run's branch
  against the commit it started from, as files with counts or one file's patch. A file run (`collide run <file>`,
  `host/src/file_run.rs`) or a run in the person's checkout is `NOT_FOUND`.
- `host/src/run_diff.rs`: the file list with `MAX_FILES`, `COLLAPSED` folders, the patch cap, and git run with
  `--no-ext-diff` and `--no-textconv`.
- `TaskGetResult.runs` and `latest_run`: the runs of a task.

What this adds.
- The heads, recorded in the store for each phase of every run (file runs and prompt runs): the workspace's `HEAD`
  when the phase starts and when it ends. `run.get`'s phases carry them. The store gets a migration; old runs have
  none.
- `task.changes { board, task }`: for each run of the task, its id, state, workspace and its words (a worktree, the
  person's checkout, a line), and its files with added and removed counts. A run's span is its first phase's start
  head to its last phase's end head. While a phase is active, the span ends at the working tree: tracked changes
  and new files that git does not ignore. Then the commits that name the task's id in their message, on the
  board's line, newest first, leaving out those inside a run's span. Each commit has its id, subject, date and files.
- `task.diff { board, task, run | commit, path, whole }`: one file's patch on its changes, or with `whole` the whole
  file after them, with each line numbered and marked. It reuses `run_diff.rs`'s limits and its git flags.
- A run with no heads (from before the migration) is listed with "no changes recorded".

## Success criteria

- **Changes** (note 17): a CLI run's phase heads list its files with their diffs, while the run is active and after,
  and a commit made by hand that names the task's id is listed too.
- A commit that names another id, or names the id as part of a longer word, is not listed.
- A file run in the person's checkout lists its changes; its words say they include the person's own edits.
- `whole` returns the whole file, its changed lines marked, and is cut at the patch cap with a note.
- An old run with no heads lists no files and says why.

## Tests

Write these first.

- `host/tests/`: a file run with the fake `claude` of `host/tests/` in a temp repo: heads recorded per phase, a
  phase held active by the fake and its edits listed from the working tree, then the run done and its span listed.
- `host/tests/`: a hand commit naming the id, one naming another id, one with the id inside a word; a commit inside
  a run's span is listed once, under the run.
- `host/tests/`: `task.diff` on changes and `whole`; a binary file; the caps; git's diff drivers never run.
- `host/tests/`: the store migration keeps old runs readable.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 6a: Task drawer, Changes ("a run's changes come from the heads it records per phase (0.8)"; "Work done
    outside a run is found by the commits that name the task's id"); a file opens its diff (note 29). Success
    Criteria: Changes. Tests: "Changes from phase heads and from commits".
  - 0.8: the run record and phase heads. 0.11: Task drawer, Changes.
- Design: notes 17 and 29 (what the data is for). No mock function: no UI here.
- **Decision for the item, with a recommendation: uncommitted edits.** Recommend listing a live phase's edits from
  the working tree, as the mock shows ("including edits not committed yet"). In the person's checkout the words say
  they include the person's own edits. Alternative: commits only, which shows nothing until the agent commits.
- **Decision for the item, with a recommendation: finding commits.** Recommend `git log --fixed-strings --grep
  <id>` on the line, then a word-bounded check in Rust, capped at the newest 200. Alternative: a regex grep in git;
  it differs across git builds.
- **Decision for the item, with a recommendation: one method or two.** Recommend `task.changes` and `task.diff` as
  new methods, with `run.diff` left as it is for the loop page. Alternative: widening `run.diff`; it is keyed by
  run, and commits have none.
- Area: `host/src/store.rs` (migration), `host/src/run_store.rs`, `host/src/file_run.rs`, the prompt run's phase
  code, `host/src/run_diff.rs`, `host/src/daemon.rs`, `core/src/owner/task.rs`, `core/src/owner/run.rs`, `schemas/`
  and the generated files, `host/tests/`.
- Dependencies: none.
- The test rule: no test touches the real home, real launchd, a real daemon, the keychain or the network. Each test
  that runs the `collide` binary clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to
  a temp root, as `host/tests/common/mod.rs` `TempRoot` does. A temp repository sets `GIT_CONFIG_GLOBAL` and
  `GIT_CONFIG_SYSTEM` to empty files. Keychain writes go through `MemorySecretStore` (`host/src/secret_store.rs`),
  never the login keychain.
- Stage 6a item (revision 25 scope, split 2026-11-24). Items of Stage 6a run one at a time, in id order, because they
  share `app/`, `core`, `host` and the generated files. No builder runs `tauri build`.
- Nothing here runs a real `claude`: runs in the tests use the fake `claude` of `host/tests/`.
- Owner decisions (2026-11-24): the owner took the recommendations at the split.
