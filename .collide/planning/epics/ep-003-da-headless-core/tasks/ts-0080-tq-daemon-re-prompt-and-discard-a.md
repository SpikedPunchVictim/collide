---
id: ts-0080-tq
kind: task
status: done
title: "Daemon: re-prompt and discard a quick run"
owners: []
links:
  - { type: blocked-by, to: ts-0079-pu }
---

## Goal

The other two answers to a quick run's `Review` item (Stage 3c's pull from Stage 9a).

- **Re-prompt** (`queue.answer` with `reprompt` and the person's words). The agent goes on in the same session, in
  the same worktree and on the same branch:
  - a new run of the same scratch task, which takes over the worktree;
  - its one phase resumes the earlier phase's Claude session by id, with the person's words as the turn;
  - the scratch task goes back to `active`, the old item leaves the Queue, and the new run ends with its own
    `Review` item, as in `ts-0078-ye`.
- **Discard** (`queue.answer` with `discard`):
  - the worktree is removed even with uncommitted changes, and the branch is deleted;
  - the scratch task goes to `dropped`, the item leaves the Queue, and the checkout is not touched.

## Success criteria

- Re-prompt continues the same session (Stage 3c criterion): fake Claude is resumed with the first phase's session
  id and gets the person's words as its turn.
- After a re-prompt, the branch has the commits of both phases, and `run.diff` of the new run shows them all against
  the base recorded at the first start.
- A re-prompt of a run that failed or was stopped works the same way.
- A session that cannot be resumed fails the new phase with that cause. The item comes back with re-prompt and
  discard, so the worktree is not left behind.
- Discard removes the worktree and the branch, and the checkout is unchanged (`git status`, `git log`; Stage 3c
  criterion). The event records the branch's last commit, so a person can restore it by hand until git collects it.
- An answer to an item that has already been answered is refused, and changes nothing.

## Tests

Write these first:

- re-prompt with fake Claude: the resume line, the turn, the second commit, the new item, the scratch statuses;
- `run.diff` after a re-prompt;
- a re-prompt after a failed phase;
- a session that cannot be resumed;
- discard with committed, uncommitted and untracked changes in the worktree, then `git status` and `git log` of the
  checkout;
- a second answer to the same item.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 3c: Re-prompt in the same session, and discard, are included.
  - 0.4: How a workflow's work lands, Work that never lands (a re-prompt is a new run of the task in that
    workspace).
  - Stage 11: Re-prompting a run reuses no other run's workspace name or iter label.
- **Decision for the item, with a recommendation: a re-prompt is a new run.** The quick run is `ended` before the
  review, and core lets nothing but an integrate answer out of `ended` (`core/src/transition.rs`). Recommend a new
  run that records the run it follows, and takes over that run's worktree, as 0.4 describes. The window follows the
  newest run of the task (`run.list` by task). The alternative keeps the run open through the review, which core's
  run machine does not allow.
- **Decision for the item: the resume.** Recommend the session resume that `ts-0066-zb` built for limit pauses
  (`Phases::resume_agent_phase`), in a new phase record.
- **Decision for the owner, with a recommendation: what discard deletes.** Recommend deleting the worktree and the
  branch, with the branch's last commit in the event. The alternative keeps the branch, which leaves branches to
  clean up. "Work that never lands" (0.4) is later work.
- **Decision for the item: the forced removal.** `BuiltinWorkspace::remove` refuses uncommitted changes and unmerged
  commits, which is right for accept. Discard needs a removal that says it discards, such as a `discard` flag or a
  method of its own.
- Fake Claude ignores the words of its launch line. If the launch line the phase recorded does not show the
  resume well enough, add a fake step that writes its own arguments to a file.
- Tests that run fake Claude go in `host/tests/adapters/claude/`.
- Area: `host` (the quick-run module, `workspace.rs`, `phases.rs`, `daemon.rs`).
- The test rule: no test touches the real home, real launchd or a real daemon. Each test clears the environment
  (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as `host/tests/common/mod.rs` `TempRoot`
  does. Settings files are fixtures, never the owner's own `settings.json`.
- Stage 3c item (revision 22 scope, split 2026-11-15). Items of Stage 3c run one at a time, in id order, because
  they share `host`, the app and the generated files.
- Owner decisions (2026-11-15, at the split): what discard deletes (the worktree and the branch; the branch's last
  commit is kept in the event). The recommendation, taken under the owner's delegation; the owner may change it
  before the item is built.

### Built

`queue.answer` with `reprompt` and with `discard` on a prompt run's `Review` item. One wire type changed: `StepKind`
has a fifth kind, `discard`, so `pnpm gen` regenerated the `Event`, `ObserverEvent` and `OwnerNotice` schemas and the
pack SDK types.

- `core/src/events.rs`: `StepKind::Discard`. `core/tests/catalogue.rs` lists it (the test of the step kinds was
  renamed, since it said four).
- `host/src/workspace.rs`:
  - `BuiltinWorkspace::discard`: the removal that says it deletes work. It refuses only an `in_use` workspace. It
    deletes the folder itself, runs `git worktree prune`, then `git branch -D`. The module notes say why it does not
    use `git worktree remove --force`: that command forgets the worktree before deleting the folder, so a folder it
    could not delete cannot be removed by the same command a second time. The folder is deleted only when the
    workspace's git directory is one of the repository's `worktrees/<name>`.
  - `BuiltinWorkspace::branch_head`: the commit of the branch, or `None`.
- `host/src/file_run/reprompt.rs` (new): `FileRuns::reprompt` and `try_reprompt`. The words must not be blank. The
  item is taken first and put back on a refusal. The run must have ended, not have been re-prompted, and have an
  `idle` worktree that is on disk. A new run is made through `begin`, with `Begin.follows` (the old context) and the
  task claimed through `claim`, which reads the task as it is (in review) for a re-prompt. If the start is refused
  after the claim, the task goes back to `review`. When it starts, the old run records `followed_by`.
- `host/src/file_run/discard.rs` (new): `FileRuns::discard` and `try_discard`. The step `collide/discard` is an
  intent that holds the branch's last commit, the deletion, and an outcome (`succeeded` with that commit, or
  `failed` with the cause). A failed deletion puts the item back and changes nothing else. The task goes to
  `QUICK.on_discarded` (dropped, by category). A run with no worktree (stopped for disk before it) is dropped with no
  step. The checkout is never read.
- `FileRuns::recover_discards` (`discard.rs`, called from `FileRuns::recover` after `recover_accepts`): settles a
  discard a kill cut short (0.2). It reads the log once, only when a prompt run with a worktree is waiting on its
  review. For each such run, the newest `collide/discard` event decides: an intent with no outcome, or an outcome that
  `succeeded` while the stored context does not say the worktree is removed. The open intent is closed with a
  `failed` outcome, and the run's review item is put back (`push_review`, with `review_title`). It deletes nothing
  and does not move the task; the person's discard finishes the deletion. `delete_work` now stores the context after
  the outcome, so a kill between the two leaves a context that says "not removed", and `try_discard` skips the
  deletion for a worktree already recorded removed.
- `host/src/file_run.rs`:
  - `RunContext.workspace_name` and `RunContext::continuing`, which copies the old context for the new run (same
    session, branch, base and worktree, `attempt` plus one, so the launch has `--resume <id>`).
  - `Begin.follows`.
  - `record_workspace` takes the workspace's name, so the events of a taken-over worktree keep the name of the run
    that made it.
  - `workspace_path` follows the name, so `run.get` of the new run shows the worktree.
  - `end_workspace_phase` takes the `Live`.
  - The `Reprompt` and `Discard` arms of `answer`.
- `host/src/file_run/quick_run.rs`: `QuickRun.follows` and `followed_by`; `task_now` (factored out of `ready_task`);
  `claim` is `pub(super)`; `QUICK.on_discarded`; the failed run's Review title carries the phase's recorded cause
  (`The run failed (no_result); ...`).
- `host/src/file_run/accept.rs`: `record_removal` (shared with discard), `write_event` is `pub(super)`, and
  `recover_accepts` skips a run that was followed, since its worktree is the new run's.

Tests, all in `host/tests/adapters/claude/` except where said. Seen red before the code: every test named here except
the last one listed. The adapter tests failed at run time (the answers were refused as not built); the workspace tests
and the catalogue test did not compile.

- `reprompt.rs` (15):
  - `a_reprompt_resumes_the_session_and_sends_the_words_as_the_turn_in_a_new_run`
  - `the_worktree_is_one_workspace_whose_events_carry_the_run_that_made_it`
  - `run_diff_of_the_new_run_shows_both_phases_against_the_base_recorded_at_the_first_start`
  - `a_reprompt_of_a_failed_run_works_the_same_way`
  - `a_reprompt_of_a_stopped_run_works_the_same_way`
  - `a_reprompt_of_a_reprompted_run_resumes_the_same_session_in_the_same_worktree`
  - `a_session_that_cannot_be_resumed_fails_the_new_phase_and_the_review_item_comes_back`
  - `a_reprompt_that_cannot_start_puts_the_item_back_and_the_task_stays_in_review`
  - `a_second_answer_to_the_same_item_is_refused_and_changes_nothing`
  - `a_run_whose_agent_never_started_has_no_session_to_resume_and_the_reprompt_is_refused`
  - `words_that_say_nothing_are_refused_and_the_item_stays`
  - `a_conflict_refusal_names_the_reprompt_and_the_reprompt_resolves_it` (the carry from ts-0079)
  - `accept_of_a_reprompted_run_merges_both_phases_and_removes_the_shared_worktree`
  - `after_a_restart_a_reprompted_run_still_owns_the_worktree_and_can_be_accepted`
  - `a_dismissed_outcome_unknown_gives_back_a_review_item_that_can_be_reprompted` (the carry from ts-0079)
- `discard.rs` (21): committed, uncommitted, staged, untracked and ignored work; commits made by hand; a worktree
  folder deleted by hand; failed, stopped and disk-stopped runs (with and without a worktree); the checkout
  unchanged (`git status`, `git log`, its branches, a local edit, a checkout on another branch); the task dropped and
  the item gone; the context and `run.get` kept, also across a restart; the event with the last commit, and a branch
  made from it; a second answer of each kind refused; a removal git cannot do (a locked folder) refused with the item
  back, then the same item discarded once the folder is unlocked; and the dismissed outcome unknown carry.
  Six tests of a kill in the middle of a discard (the kill is simulated as `accept.rs` does: stop the daemon, delete
  the item, write the intent with no outcome, delete the worktree's folder and keep the branch):
  - `a_discard_a_kill_cut_short_has_its_review_item_back_at_the_next_start_and_nothing_is_deleted`
  - `discard_on_the_item_a_kill_gave_back_finishes_the_deletion_and_writes_the_outcome`
  - `a_kill_after_the_outcome_but_before_the_run_was_recorded_removed_also_gives_the_item_back`
  - `an_item_given_back_still_discards_when_the_task_is_already_dropped`
  - `a_discard_that_finished_is_left_alone_at_the_next_start`
  - `a_discard_that_failed_is_left_alone_at_the_next_start`

  The first four were seen red. The last two passed before the code, as guards that the start touches nothing that
  was settled.
- `host/tests/workspace.rs`: seven tests of `discard` and `branch_head`. Six were seen red. The seventh,
  `discard_does_not_delete_a_folder_that_is_not_a_worktree_this_workspace_made`, was written after the code and was
  never red.
- Test support: fake Claude plays a second recording for a launch with `--resume` (`BinarySpec.resumed`,
  `Setup.resumed`) and has a `git` step (`fake_claude.py`), which the conflict test uses to resolve the conflict. The
  obsolete test `reprompt_and_discard_are_not_built_yet` was removed from `quick_run.rs`.

Checks, all run in the foreground and passing: `cargo build --workspace`, `cargo test --workspace`,
`cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`, `pnpm test` (the `packs/plan`
tests did not time out), `pnpm typecheck`, `pnpm --filter @collide/app build`, `pnpm build:packs`, `pnpm check`.

### Decisions for the owner

1. **A fifth step kind, `discard`.** 0.2's catalogue has four kinds, and a discard is not one of them. The event
   needs an intent and an outcome to hold the last commit, so I added the kind. Recommend keeping it. The
   alternative is a `source_op` kind, which misnames a local deletion. Plan text proposed (0.2, the step kinds):
   "`discard`: the deletion of a run's worktree and branch, with the branch's last commit in its events."
2. **A discard deletes the folder itself and not through `git worktree remove --force`.** See the module notes of
   `workspace.rs`. Recommend as built: it can be retried after a partial failure. The alternative uses git's own
   command and cannot be retried once git has forgotten the worktree.
3. **A re-prompt of a run whose agent never started is refused.** There is no session to resume, and discard is the
   answer for it. Recommend as built. The alternative starts a fresh session with the person's words, which is a new
   start and belongs to Stage 9a.
4. **A killed discard is settled at the next start (done).** The review item comes back, nothing is deleted at start,
   and the task is not moved; the person's discard finishes the work. See `recover_discards`. This is the 0.2 rule
   for a step with an intent and no outcome, applied the way `recover_accepts` does for accept. A discard is safe to
   repeat, so there is no "outcome unknown" item for it. Plan text proposed (0.2, after the rule on intents with no
   outcome): "A `discard` step with no outcome is closed as failed and the run's review item comes back; the
   deletion is repeated only when the person discards again."
5. **The new run keeps the old run's workspace name** in its events, and `run.get` of the new run shows the old
   run's folder. Plan 0.4 says a re-prompt is a new run "in that workspace", and Stage 11 says it reuses no other
   run's workspace name or iter label. Recommend reading Stage 11 as being about the labels of new workspaces, and
   keeping the one workspace. The alternative gives the worktree a new name, which breaks the folder's name.
6. **The re-prompt's words go as given**, not trimmed. Only a blank answer is refused.
7. **The failed run's Review title carries the phase's cause** (`no_result`), so the person sees why before they
   re-prompt. The words are the cause's own name. Recommend a plainer sentence per cause when the window (ts-0081-wm)
   shows it.

- Owner decisions (2026-11-15): decisions taken under the owner's delegation, as recommended. Decision 1's plan text
  applied, reworded, to 0.2's intent-and-outcome rule. Decision 4 was changed in review: a killed discard is settled
  at the next start in this item (`FileRuns::recover_discards`), so a run is never left without its Review item. A
  discard whose outcome was written but whose context was not yet stored also gets its item back; a second discard
  then finds nothing to delete. `discard_does_not_delete_a_folder_that_is_not_a_worktree_this_workspace_made` was
  written after the code and not seen red.

### Not done, by design

- Anything the window does with these answers: the words box, following the newest run of the task, and showing the
  discard's last commit (ts-0081-wm).
- A restart check that the old run of a re-prompt is still the one in the run's list: `run.list` by task shows both
  runs, newest first, as built.
- Deleting a context from the store on discard: it stays, like an accepted run's (ts-0079 decision 6).
- Releasing the task's `dropped` state back to `ready`; a dropped task cannot be started again.
- The Stage 11 "iter label" for a re-prompt (the engine's).
