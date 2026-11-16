---
id: ts-0079-pu
kind: task
status: done
title: "Daemon: accept a quick run with a local merge"
owners: []
links:
  - { type: blocked-by, to: ts-0078-ye }
---

## Goal

Accept on a quick run's `Review` item merges the run's branch into the board's checkout and removes the worktree.
This is `collide/merge-local` in its 3c form, written in Rust in the daemon. The pack's hook form waits for Stage 7.

`queue.answer` with `accept` on the item:

1. The check: `git merge-tree --write-tree` of the base branch and the run's branch. A conflict refuses the accept,
   names the files, and changes nothing.
2. The checkout must have the run's base branch checked out, with no rebase, merge, cherry-pick or bisect in
   progress. Otherwise the accept is refused and says why.
3. A `step.intent` event for the integrate, then the merge in the checkout, then a `step.outcome` event (0.2).
4. The integrate answer applied to the run through `core/src/run_rules.rs`.
5. The worktree removed and the branch deleted. The scratch task goes to `done`, the item leaves the Queue, and
   the answer is `merged` with the commit.

Carried from 3a and 3b:

- `ts-0055-cr`: a branch whose change is already in the base (squash or rebase) counts as merged, per 0.2's rule
  for a merge that finds its change at the target. Today `BuiltinWorkspace::remove` refuses such a branch.
- `ts-0065-kx`: removing the worktree after a run comes with integrate (Stage 3c).

## Success criteria

- Against a fixture repo, accept merges the branch into the checkout (the base branch has the change, per
  `git log`), removes the worktree and the branch, and sets the scratch task `done` (Stage 3c criterion).
- An accept that would conflict is refused, and `git status` of the checkout is unchanged. The item stays, so the
  person can re-prompt or discard.
- A checkout on another branch, or in the middle of a rebase or merge, is refused with the reason, and nothing
  changes.
- A local change in the checkout that the merge would overwrite: git refuses, nothing changes, and the answer says
  so. A local change the merge does not touch stays as it was.
- A branch whose change is already in the base (merged, squashed or rebased) is accepted as merged, and its
  worktree is removed.
- The intent and outcome events are written. After a daemon killed between them, the restarted daemon checks the
  checkout: a change at the base is a success and the removal finishes; an interrupted merge (`MERGE_HEAD`) is an
  `OutcomeUnknown` Queue item; anything else puts the review item back. The merge is never repeated blind.

## Tests

Write these first:

- accept with fake Claude's commits, then `git log`, the worktree folder, the branch and the scratch status;
- a conflict, then `git status`;
- a checkout on another branch; a rebase in progress;
- a local change the merge would overwrite, and one it would not touch;
- already merged, squashed and rebased branches;
- a kill between intent and outcome, then a restart, for each of the three outcomes;
- a board whose root is a linked worktree.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 3c: what it pulls from Stages 7 and 9a (accept with `merge-local`, the worktree removed).
  - 0.2: A step with a side effect writes an intent before it and an outcome after it.
  - 0.4: `collide/merge-local` (a local merge into the base ref); Root busy.
  - Stage 9a: Review and accept a single run (the full order, which replaces this).
- **Decision for the owner, with a recommendation: a checkout that is not on the base branch.** 0.4 says
  `merge-local` merges into the base ref; Stage 3c says into the checkout. They agree only when the checkout has the
  base branch checked out. Recommend refusing otherwise, with the branch to switch to. The alternative updates the
  base branch without touching the checkout, which a person may not notice.
- **Decision for the item, with a recommendation: the merge.** Recommend `git merge --no-edit` in the checkout,
  which fast-forwards when it can, with Collide's git environment (`DaemonOptions` `git_env`). The check in step 1
  runs first, so the merge never stops half-way on a conflict.
- **Decision for the item: what conflict does in 3c.** Stage 3c says nothing. Stage 9a sends a conflict to the Queue.
  Recommend a refusal that keeps the review item. It is the smallest form of 9a's rule: no checkout changes.
- **Decision for the item: removal of a branch already in the base.** Recommend a check, in the built-in workspace,
  that the base's tree already has the branch's change (a merge-tree of the two equals the base). It is used both
  by accept and by `remove`.
  This is accept, a person's action. The reaper's rule for an inferred landing (0.4, How a workflow's work lands:
  never remove on an inferred one) does not apply here.
- Accept is a person's answer. From the app it comes as `Caller::App` (`ts-0077-ws`), so it needs no confirmation.
- Tests that run fake Claude go in `host/tests/adapters/claude/`.
- Area: `host` (the quick-run module, `workspace.rs`, `daemon.rs`), `core` only to use `run_rules`.
- The test rule: no test touches the real home, real launchd or a real daemon. Each test clears the environment
  (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as `host/tests/common/mod.rs` `TempRoot`
  does. Settings files are fixtures, never the owner's own `settings.json`.
- Stage 3c item (revision 22 scope, split 2026-11-15). Items of Stage 3c run one at a time, in id order, because
  they share `host`, the app and the generated files.
- Owner decisions (2026-11-15, at the split): a checkout not on the base branch (refuse, and name the branch to
  switch to). The recommendation, taken under the owner's delegation; the owner may change it before the item is
  built.

### Built

`queue.answer` with `accept` on a prompt run's `Review` item merges the run's branch into the board's checkout, removes
the worktree and its branch, and moves the scratch task to done. No wire type changed, so `pnpm gen` had nothing to
regenerate (`QueueAnswerResult::Merged` and `QueueKind::OutcomeUnknown` already existed).

- `host/src/file_run/accept.rs` (new, a child of `file_run`, with the order in its module doc):
  - `FileRuns::accept` takes the item out of the Queue first, so two accepts never both merge, and puts it back when
    the accept is refused. `try_accept` holds the run's lock and `FileRuns.merging`, a lock of its own, so two runs of
    one checkout never meet in git's lock.
  - `check_merge`: `git merge-tree --write-tree` of `refs/heads/<base_branch>` and the run's branch. A conflict is a
    refusal that names the files. Then the checkout: no rebase, merge, cherry-pick, revert or bisect in progress
    (`operation_in_progress`, from `git rev-parse --git-path`, so a linked worktree's own files are read), and on the
    base branch (`boards::checkout_of`), or the refusal names the branch to switch to. A dirty run worktree
    (`BuiltinWorkspace::refuse_dirty`) is refused before anything is merged.
  - The step: `step.intent` (`collide/merge-local`, kind `integrate`), `merge`, `step.outcome` (`succeeded` with the
    commit, or `failed` with git's words). `merge` is `git merge --no-edit` in the checkout with the daemon's git
    environment: hooks run, nothing is bypassed. A merge git stopped with its state written (a `pre-merge-commit` hook
    that refused) is aborted, so the checkout is as it was.
  - `finish_merged`: `apply_integrate_answer` with `Merged { commit }`, then what it releases: the source's operations
    move the task to `QUICK.on_merged` (done, by category), then `remove_worktree`, which sets the removal base to
    `refs/heads/<base_branch>` (decision 3 of `ts-0078-ye`) and records the workspace's `removable` and `removed`
    moves. A failure after the merge is logged, never a refusal: the merge is done.
  - `recover_accepts` (called from `FileRuns::recover`, so at daemon start): for each ended prompt run whose worktree is
    not removed, the log's open `integrate` step (`run_rules::recover`) is checked in the checkout and settled
    (`run_rules::settle`). The base has the change: a `succeeded` outcome and the removal finishes. `MERGE_HEAD`: an
    `OutcomeUnknown` Queue item, nothing touched. Anything else: a `failed` outcome and the review item is back. With
    no open step, a last `succeeded` outcome whose removal never happened finishes the removal. `dismiss_unknown` is
    Dismiss on that item: it closes the step as failed and puts the review item back.
- `host/src/workspace.rs`: `Git::merge_check` and `MergeCheck` (`InBase`, `Clean`, `Conflict { files }`), from
  `merge-tree --write-tree --name-only -z`. `remove` now counts a branch whose change the base already has (merged,
  squash merged or rebased) as merged, through `change_is_in_base`. `refuse_dirty` is public and `refuse_work` is
  `refuse_dirty` then `refuse_unmerged`. `set_base` changes the removal base. A git failure that says nothing on
  standard error now carries standard output (a merge's conflicts).
- `host/src/file_run/quick_run.rs`: `QUICK.on_merged`; `push_review` (the Review item, shared with `finish_quick`);
  `move_task` is `pub(super)`.
- `host/src/file_run.rs`: the `Accept` arm of `answer`, Dismiss on an `OutcomeUnknown` item, `FileRuns.merging`, and
  the `recover` call. Reprompt and discard are still refused "not built yet".
- Tests' support: `Fixture` of `host/tests/adapters/claude/file_run.rs` has `stop_daemon` and `start_daemon` (the
  options are `daemon_options`, shared with `new`). The helpers of `quick_run.rs` are `pub(super)`, and its `quick`
  sets the repository's own `user.name` and `user.email`, which the merge commit needs (the daemon's git reads no
  global configuration).

**Tests.** None touches the real home, launchd or a daemon of the owner's: each has a temp root, `env_clear()` for its
own git, and fake Claude as `claude`.

- Seen red before the code: all 22 tests of `host/tests/adapters/claude/accept.rs` (each failed on "accept on a review
  is not built yet", the kill tests on the missing items), and two of `host/tests/workspace.rs`:
  `removal_goes_ahead_when_the_change_was_squash_merged_into_the_base` and
  `removal_goes_ahead_when_the_change_was_rebased_onto_the_base`.
- `accept.rs`: `accept_merges_the_branch_into_the_checkout_and_removes_the_worktree`,
  `accept_writes_the_intent_before_the_merge_and_the_outcome_after_it`,
  `a_base_that_moved_on_gets_a_merge_commit_with_both_sides`,
  `a_second_answer_to_an_accepted_item_is_refused_and_changes_nothing`,
  `a_conflict_refuses_the_accept_names_the_files_and_changes_nothing`,
  `a_checkout_on_another_branch_is_refused_and_names_the_branch_to_switch_to`, `a_rebase_in_progress_is_refused`,
  `a_merge_in_progress_is_refused`, `a_cherry_pick_in_progress_is_refused`, `a_bisect_in_progress_is_refused`,
  `a_local_change_the_merge_would_overwrite_is_refused_by_git_and_nothing_changes`,
  `a_local_change_the_merge_does_not_touch_stays_as_it_was`,
  `a_hook_of_the_repository_that_refuses_the_merge_is_a_refusal_and_the_checkout_is_as_it_was`,
  `a_worktree_with_uncommitted_changes_refuses_the_accept_before_anything_is_merged`,
  `a_branch_already_merged_into_the_base_is_accepted_as_merged`,
  `a_branch_squashed_into_the_base_is_accepted_as_merged`,
  `a_branch_rebased_onto_the_base_is_accepted_as_merged`,
  `a_board_whose_root_is_a_linked_worktree_merges_into_the_branch_it_is_on`,
  `after_a_kill_a_change_at_the_base_is_a_success_and_the_removal_finishes_with_no_second_merge`,
  `after_a_kill_an_interrupted_merge_is_an_outcome_unknown_item_and_nothing_is_touched`,
  `after_a_kill_before_the_merge_the_review_item_is_back_and_the_merge_is_made_when_accepted`,
  `after_a_kill_between_the_outcome_and_the_removal_the_removal_finishes`.
- `workspace.rs` (tests): the two above, and three that pass before and after as guards:
  `removal_refuses_a_change_the_base_does_not_have_even_when_the_base_moved_on`,
  `removal_refuses_a_change_that_was_squash_merged_and_then_taken_out_again`,
  `removal_checks_the_base_it_is_given_and_not_the_one_the_workspace_started_from` (it needed `set_base`, which was
  added first, so it was not red).
- `quick_run.rs`: `accept_reprompt_and_discard_are_not_built_yet` is now
  `reprompt_and_discard_are_not_built_yet`.
- The kill is simulated. The daemon runs in the test's process and cannot be killed between two statements, so a test
  stops it and writes what a kill leaves (the intent in the log, the item gone from the stored Queue) and, for the
  three outcomes, the checkout as it would be (merged by hand, `merge --no-commit`, untouched).
- Checks: `cargo build --workspace`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo fmt --all --check`, `pnpm test`, `pnpm typecheck`, `pnpm --filter @collide/app build`, `pnpm build:packs` and
  `pnpm check` all pass.

### Decisions for the owner

1. **Dismiss on an `OutcomeUnknown` item puts the review item back.** After a kill that left `MERGE_HEAD`, the item
   is gone, and without this the run would have a worktree and no way to remove it. Recommend keeping it: the person
   fixes the checkout, dismisses, and the next accept checks again. The alternative is an item that only clears,
   which leaves the run with no review until Stage 9a's "tell Collide what happened" answer. Plan text proposed
   (0.11, Queue, in the `OutcomeUnknown` row): "In Stage 3c the one answer is Dismiss, which closes the step as
   failed and returns the run's review item."
2. **A branch that changed nothing, or whose change the base already has, is accepted as merged** with the base's
   head as the commit. This is the item's rule applied to the empty change too. The alternative refuses a run with no
   change, which strands it until discard. Recommend as built.
3. **The run's worktree must be clean before the merge.** A change made in the worktree after the review would not
   be in the merge, and removal would refuse after the merge. Recommend refusing first, with the worktree's own
   words. The alternative merges and leaves the worktree behind.
4. **A revert in progress is refused with the other four.** The item lists four, and git has the same state for a
   revert. Recommend keeping it.
5. **The merge commit's identity is the repository's own configuration.** No `-c user.name`, no signing override,
   unlike the leftovers' commit: the person's merge commits are theirs. A repository with no identity, or a signing
   setup that cannot run in the daemon, is a refusal in git's words. The alternative commits as Collide. Recommend
   as built.
6. **Contexts of accepted runs stay in the store**, with the workspace `removed`, so `run.get` still reports the run
   as a prompt run (`implementations`) and a restart reads them. The alternative deletes the context at accept and
   loses that. They accumulate by one small row per run; discard (`ts-0080-tq`) should choose the same.
7. **The log is read once at start**, only when an ended prompt run still has its worktree
   (`Store::events_since(0)`, filtered by run). Recommend keeping it until the log is pruned or very large; a per-run
   read needs a store query that does not exist.
8. **Conflict words promise re-prompt.** The refusal says "re-prompt the agent to resolve it, or discard the run",
   which `ts-0080-tq` makes true. Until then the Review item offers both and refuses them "not built yet".

- Owner decisions (2026-11-15): decisions 1 to 8 taken under the owner's delegation. Decision 1's plan text applied,
  reworded, to 0.2's intent-and-outcome rule. The task's move to `done` after accept is the category
  `QUICK.on_merged`, written in `host/src/file_run/quick_run.rs` until a pack declares the operation that follows an
  integrate (0.4). A kill between intent and outcome is simulated by stopping the in-process daemon and writing what
  a kill leaves; the real SIGKILL test stays in Stage 8.

### Not done, by design

- Re-prompt and discard (`ts-0080-tq`): still refused "not built yet", so a run that cannot be accepted has no way out
  until it is built.
- The `run.before_integrate` and `run.after_integrate` points, the `sync` check of 0.4 beyond the merge check, gates,
  and the pack's `merge-local` hook (Stage 7, 9a). The task's move to done is hard-coded (`QUICK.on_merged`).
- Root busy as a wait (0.4): a busy checkout is a refusal that names the state, not a Queue item that waits.
- An answer to an `OutcomeUnknown` item that says whether the step happened (0.2); only Dismiss.
- A removal that fails after the merge is logged and not retried until the next start, which finishes it through the
  last `succeeded` outcome.
- A true SIGKILL between the intent and the outcome (Stage 8 tests that with the engine).
- A diff of an accepted run: its branch is deleted, so `run.diff` answers not found.
