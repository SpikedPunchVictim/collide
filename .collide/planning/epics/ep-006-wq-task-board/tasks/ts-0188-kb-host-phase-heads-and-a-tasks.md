---
id: ts-0188-kb
kind: task
status: done
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

### Built

- Store: migration 12, `phase_heads` (`host/src/store.rs`): one row for each run and phase with `start_head` and a
  nullable `end_head`. `host/src/run_store.rs` has `PhaseHeads`, `Store::set_phase_start_head` (the first write stays,
  so a phase that resumes keeps its start), `Store::set_phase_end_head` and `Store::phase_heads` (in the order the
  phases started). A run from before has no rows.
- Heads, written by `FileRuns::record_head` (`host/src/file_run.rs`): the start head in `launch`, once the workspace's
  phase has started; the end head in `on_phase_end` for a file run and in `fail`. A prompt run keeps its end head in
  `finish_quick` (`host/src/file_run/quick_run.rs`) after its leftovers are committed and before its Review item, so
  the span holds the leftovers and a client that finds the item finds the span. A head that cannot be read is logged
  and the run goes on.
- Wire: `PhaseSummary.start_head` and `end_head` (`core/src/owner/run.rs`), left out when absent, filled by
  `summarize` (`host/src/run_summary.rs`) from the heads `summary_of` reads. New methods `task.changes` and
  `task.diff` (`core/src/owner.rs`, `core/src/schema.rs`, `app/src/owner/methods.ts`) with `TaskChangesParams`,
  `TaskChangesResult`, `RunChanges`, `CommitChanges`, `ChangeWords`, `ChangesPlace`, `TaskDiffParams`,
  `TaskDiffResult`, `WholeFile` and `NumberedLine` (`core/src/owner/task.rs`). `pnpm gen` wrote `schemas/` and
  `packages/pack-sdk/src/generated/`.
- Host, git: `host/src/task_changes.rs`. `Span` is `Between` (two commits: a run that ended), `WorkingTree` (a start
  head and the working tree: a run whose phase is active) or `Commit`. `list`, `patch` and `whole` read a span;
  `naming` finds commits (`git log --fixed-strings --grep=<id>`, up to 1000 from git, a word-bounded check by
  `names_id`, the newest 200); `commits_in` gives a run's own commits. The working-tree span is `git diff <start>`
  plus `git ls-files --others --exclude-standard`; nothing is staged or added, and the new files are read (and counted)
  by this code. `run_words` and `commit_words` make the words. Every diff, diff-tree and log passes `--no-ext-diff` and
  `--no-textconv`; working-tree reads pass `--no-optional-locks`. A client's path is refused when it is empty,
  absolute or has `..`, and a file is read only if it is inside the folder after links are resolved.
- Host, reuse: `host/src/run_diff.rs` is split so both users share `listing` (collapse and cap), `cut_patch`,
  `parse_raw`, `parse_numstat` and `diff_args`. `Entry.size` is for a file git has no blob of. `run.diff` is unchanged.
- Host, daemon: `task_changes` and `task_diff` in `host/src/daemon.rs`. A run's span is read in its own folder while
  that is there, else in the board's checkout. A commit inside any run's span (`rev-list start..end`, or `..HEAD` for a
  phase that is active) is left out of the list. A scratch task has no commits.
- Tests, `host/src/task_changes.rs` (unit, a real temp repo with git config out of reach): `span_of`, `names_id`,
  `changed_lines`, the words, `list` for the three spans, new and ignored and collapsed files, `patch` and `whole`
  (changed, new, binary, deleted, cap, no change, outside paths, a link out of the folder), `naming` (order, 200 cap,
  an id that looks like an option), `commits_in`, and the diff driver and textconv tests
  (`the_drivers_of_the_test_do_run_for_a_plain_git_diff` is the control,
  `a_diff_driver_and_a_textconv_the_repository_names_never_run`).
- Tests, store: `a_store_at_version_eleven_gains_phase_heads_and_keeps_its_runs`; `host/src/run_store.rs`:
  `a_phase_keeps_its_first_start_head_and_gains_an_end_head`,
  `an_end_head_for_a_phase_that_has_no_start_head_is_not_kept`,
  `phase_heads_come_in_the_order_the_phases_started_and_by_run`; `host/src/run_summary.rs`:
  `a_phase_carries_the_heads_it_started_and_ended_at_and_one_without_heads_carries_none`.
- Tests, `host/tests/adapters/claude/task_changes.rs` (the daemon, the fake `claude`): heads
  (`a_file_run_records_the_commit_its_phase_started_and_ended_at`,
  `a_prompt_run_ends_at_the_commit_collide_made_of_what_the_agent_left`), changes
  (`a_phase_that_is_active_lists_its_edits_from_the_working_tree_and_then_its_span`,
  `what_is_written_after_the_last_phase_ended_is_not_the_runs`,
  `a_run_from_before_heads_were_kept_lists_no_files_and_says_why`, `a_run_that_is_final_has_its_state_in_the_changes`),
  commits (`commits_that_name_the_task_are_listed_and_others_are_not`,
  `a_scratch_task_has_no_commits_because_its_id_is_a_short_number`,
  `a_commit_inside_a_runs_span_is_listed_once_under_the_run`), `task.diff`
  (`task_diff_gives_a_runs_patch_and_the_whole_file_with_its_changed_lines_marked`,
  `task_diff_of_a_phase_that_is_active_reads_the_working_tree`, `task_diff_gives_a_commits_patch_and_whole_file`,
  `task_diff_of_a_binary_file_says_it_is_not_text`, `a_whole_file_is_cut_at_the_patch_cap_with_a_note`,
  `task_diff_refuses_a_request_that_names_both_or_neither_and_a_commit_that_is_not_an_id`,
  `a_path_outside_the_repository_is_refused`) and
  `a_diff_driver_and_a_textconv_the_repository_names_never_run_for_the_changes` (a live run, a finished run and a hand
  commit, with a control that the same config runs the driver for a plain `git diff`). `core/tests/owner_protocol.rs`
  has samples for the two methods. `fake_claude.py` has a new step, `wait_file`, which holds a phase until the test
  makes a file (it ends the fake after two minutes), so a phase is held active with no sleep.
- Seen red at runtime, against stubs of the new code (each applied alone, then undone; the files were compared with
  the originals after):
  - no head kept: 8 of the 17 daemon tests;
  - the migration's table renamed: 4 store and run-store tests and 10 daemon tests;
  - a start head that replaces: the run-store test;
  - the heads left out of `summarize`: the run-summary test and the file-run test;
  - the word check dropped: the unit and the daemon test of commits; `names_id` made `contains`: 2 unit tests and the
    daemon test;
  - commits of a span not left out: the span test;
  - the flags removed: both driver tests;
  - the span always the working tree: 2 unit and 3 daemon tests;
  - new files not listed: 1 unit and 2 daemon tests;
  - no cap: 1 unit and 1 daemon test;
  - no "no changes recorded" note: the old-run test;
  - scratch commits not skipped, path check off, hex check off, binary check off, changed lines not marked: the daemon
    test for each (and the unit tests for binary and marks).
  Not seen red: the unit tests of the words of a worktree and of commits and of `changed_lines` (new pure functions),
  and the words of a run in the person's checkout at the daemon (no run is there yet).

### Decisions for the owner

- A span ends at the end head, so edits made in a worktree after the phase ended are not the run's. A re-prompt run
  takes over a worktree, so ending at the working tree would credit its edits to the earlier run. Alternative: end at
  the working tree for any run whose folder is still there. Recommendation: keep the heads. A run that is over and
  has no end head (the daemon did not see it end) says "no changes recorded".
- File runs (`run.start_file`) have no board and no task, so `task.changes` cannot list them. Their heads are on
  `run.get`. Recommendation: nothing more until a file run gets a task.
- No run works in the person's checkout yet: every run has a worktree. `ChangesPlace::Checkout` and its words ("...
  and your own edits") are in the wire and tested at the unit level, but the daemon always answers `worktree`.
  Recommendation: add the place test (the run's folder is not under the workspaces folder) in the item that first runs
  a task in the checkout, with a daemon test then.
- The heads are in their own table, not in the run context. The context is forgotten when a file run ends; the heads
  must outlive it. Alternative: a column on `phase_records`. Recommendation: keep the table; a phase record is written
  at the phase's end, and the start head is needed while the phase is active.
- A scratch task lists no commits: its id is a short number. Alternative: search the commit messages for `scratch:3`.
  Recommendation: keep it off.
- A commit is found when its message has the id with no letter, digit, `-` or `_` before or after it, so
  `ts-0160-ab` is not found in `ts-0160-abc` or `ts-0160-ab-more`. Alternative: only letters and digits are word
  characters, which finds `ts-0160-ab-more`. Recommendation: keep it; ids are made of those characters.
- `git log` is asked for up to 1000 commits before the word check, so the check cannot leave fewer than 200 when more
  exist. A task named in more than 1000 commits shows the newest 200 of the first 1000. The files of each listed
  commit are two git calls, so a list of 200 is 400 calls. Recommendation: keep it; the usual list is a few commits.
- `task.diff` with a `commit` does not check the commit names the task. Any commit id of the board's repository can be
  shown. The person can read the repository anyway. Alternative: refuse a commit `task.changes` did not list.
- A new file that is not tracked is listed with line counts up to 4 MiB; a larger one has none. Its patch is written
  here and cut at the patch cap.
- The ids of a task that was re-id'd are used for its runs (as `task.get` does) and not for its commits. Alternative:
  search for each old id too. Recommendation: wait until it is asked for.
- Plan text to add under Stage 6a, Task drawer, Changes: "`task.changes` lists, for each of the task's runs, the files
  from its first phase's start head to its last phase's end head (to the working tree while a phase is active), then
  the commits on the board's line that name the task's id, outside every run's span. `task.diff` gives one file's
  patch or the whole file after the changes. Heads are kept per phase in the store (`phase_heads`)."

### Not done, by design

- The Changes tab, the diff window and the "whole file" view: `ts-0189-fg`.
- A run in the person's checkout, end to end (see Decisions): Stage 8, "Where an agent starts and how work lands",
  which first starts a phase at the line root.
- The commits of a task's old ids (see Decisions): no item.
