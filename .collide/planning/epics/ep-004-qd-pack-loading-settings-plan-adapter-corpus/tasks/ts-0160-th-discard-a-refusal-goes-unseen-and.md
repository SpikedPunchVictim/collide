---
id: ts-0160-th
kind: task
status: done
title: "Discard: a refusal goes unseen, and a Review item whose repository is gone stays"
owners: []
links:
  - { type: blocked-by, to: ts-0159-ra }
---

## Goal

In `ts-0138-pn` step 6c (owner-run, 2026-11-21) the owner pressed Discard on a quick run's Review item and confirmed
"Delete this work? ... which hold 0 changed files ...". Nothing was deleted: the run's worktree and branch stayed (the
owner removed them by hand), the store still holds the Review item `run-1795253516592-2-review`, and the event log has
no `step.intent` of `collide/discard` for any run. The owner saw no refusal they noticed.

What most likely happened, from the store and the code:

- The page followed the older run `run-1795250812644-1` (`ts-0159-ra`), whose Review item is first in the queue, so the
  Review card and its Discard were that run's. "0 changed files" is also what the card says when it has no diff.
- That run was made by the previous daemon process and its clone was deleted. `FileRuns::discard`
  (`host/src/file_run/discard.rs`) takes the item, fails in `delete_work` before the intent (Git cannot read the branch
  of a repository that is gone), puts the item back and answers `refused`. The page shows that message only until the
  next reload.
- The old run's worktree folder is still in the real `workspaces/` folder, with nothing in Collide that can remove it.

The fix confirms each step first.

## Success criteria

- The Review card's Discard acts on the item the card shows, and the confirmation names its run.
- A refused discard shows its words on the page until the person dismisses them.
- A Review item whose repository is gone can be cleared: Discard deletes the run's worktree folder by the path the run
  recorded, drops the item, and says the branch went with the repository.
- A run made by an earlier daemon process can be discarded after a restart.

## Tests

- Write first and see red: a quick run's Review item, the repository deleted, then `queue.answer` `discard`: today
  `refused` with the item kept; after, `done`, the folder gone, the item gone.
- A discard after a daemon restart, with the repository in place: the worktree and branch are gone.
- `app/test/loop-page.test.ts`: a refused answer's message survives the reload that follows it.

## Notes

- Recommendation: as the criteria say. Alternative: a Dismiss answer on a Review item that leaves the folder. It leaves
  disk behind with nothing pointing at it.
- Blocked by `ts-0159-ra`, since the card's choice of run changes which item a Discard answers.
- Not a blocker of `ts-0138-pn`: Discard is a step of its clean-up, not one of its success criteria.
- The owner's real store still holds both stale Review items; the folder of `run-1795250812644-1` is left for the owner
  to remove, or for this fix to clear.
- Owner decisions: none recorded yet. The recommendations stand until the owner answers.

### Built

Which steps of the likely cause held, each checked with a test before any code changed:

- **Held:** the Review item of a run whose repository is gone is refused, and nothing is deleted. `FileRuns::discard`
  takes the item, `delete_work` fails at `branch_head`, the item is put back, the folder stays, and Collide has no
  other way to remove it. The words were "Git could not read the branch `collide/<run>`: could not run `git`: No such
  file or directory (os error 2)". No `step.intent` was written: the read comes before the intent.
- **Which refusal the owner's case hit:** that one, not "the daemon has no run". `FileRuns::new` reads every stored
  run context back at start, so `live_of` finds a run of an earlier daemon process. The same refusal came with and
  without a restart between the run and the discard. With the repository in place, a discard after a restart already
  worked (green before and after). "The daemon has no run" needs a Review item whose run context is missing from the
  store, which no test could make by any real path; it is left as it was.
- **Did not hold, as the item put it:** "the page shows that message only until the next reload". With `ts-0159-ra` in
  place the message survives the reload that follows the refusal, because the refused item is back and holds the
  card. It was lost when the card moved to another run: `afterRunChange` cleared `refusal`, so "Open" on another item,
  or a newer run once the item was gone, wiped it.
- **Held:** "0 changed files" is what the card says with no diff (`changed` was `0` for `diff === null`).
- **Not named in the item, found by the tests:** with the repository gone `run.diff` answers `NOT_FOUND` (the branch
  is gone), and `sync` threw on it. The page then said "Could not reach Collide" and stopped updating, so a Review
  card on such a run could not be reached to press Discard.
- **Not named in the item, found by the tests:** `discard` deleted the folder of the path the run recorded, after
  checking only that the recorded git directory looked like a worktree's. A stored path outside the `workspaces`
  folder was deleted (red: the discard went through).

What changed:

- `core/src/owner/queue.rs`: `QueueAnswerResult::Discarded { note }`, a discard that went through with a note for the
  person. `pnpm gen` was run: `schemas/QueueAnswerResult.*.json` and the SDK's `QueueAnswerResult.*.ts`.
- `host/src/workspace.rs`: `BuiltinWorkspace::repository_gone` (the repository's folder is missing). `discard` then
  deletes the folder and moves to `removed`, with no git call.
- `host/src/file_run/discard.rs`: `check_folder` (the recorded folder must be `workspaces/<name>` of the daemon's own
  `workspaces` folder, once links are resolved, and not a link itself; refused by path, before any event).
  `delete_work` calls it, reads no branch for a gone repository, and writes an intent that says so. `try_discard` and
  `discard` carry the note: `discarded` for a gone repository, `done` as before for the rest.
- `app/src/loop/loop.ts`: `diffOf` returns the reason when the diff cannot be read, and the sync goes on.
  `dismissRefusal`. `noticeFor` reads the `discarded` answer.
- `app/src/loop/state.ts`: `diffProblem`, the `refusal_dismissed` action, and `afterRunChange` keeps the refusal.
- `app/src/loop/Queue.tsx` (`ReviewItem`): the confirmation names the run, and counts files only when the diff was
  read. A review with no diff says "Collide could not read what changed" with the reason. `LoopPage.tsx`: the refusal
  has a Dismiss button. `types.ts`: `AnsweredResult`.

Tests, in `host/tests/adapters/claude/discard.rs` (new, after "a kill in the middle of a discard"). Ten of the eleven
were red at runtime on the code before my change (only the new enum variant existed), each for the right reason (the
branch-read refusal, or a discard that went through for a folder outside `workspaces`):

- `a_review_item_whose_repository_is_gone_is_discarded_and_its_folder_deleted` (red)
- `a_discard_with_the_repository_gone_writes_its_intent_and_outcome` (red)
- `a_run_of_an_earlier_daemon_whose_repository_is_gone_is_discarded_after_a_restart` (red)
- `a_worktree_folder_that_is_gone_with_the_repository_is_discarded_all_the_same` (red)
- `a_run_of_an_earlier_daemon_is_discarded_after_a_restart_with_the_repository_in_place` (green before: a guard)
- `a_recorded_folder_outside_the_workspaces_folder_is_refused_by_name_and_left_alone` (red)
- `a_recorded_folder_outside_the_workspaces_folder_is_refused_with_the_repository_in_place_too` (red)
- `the_workspaces_folder_itself_and_another_runs_folder_are_refused` (red)
- `a_run_folder_that_is_a_symlink_out_of_the_workspaces_folder_is_refused_and_not_followed` (red)
- `a_run_folder_that_is_a_symlink_out_of_the_workspaces_folder_is_refused_with_the_repository_in_place_too` (red)
- `a_symlink_inside_the_worktree_that_points_out_is_removed_and_not_followed` (red)

In `app/test/loop-page.test.ts`, describe "discard (ts-0160-th)". Red at runtime on the old code (5): "a refused discard
keeps its words when the card moves to another run", "... when a newer run takes the card", "the person dismisses the
words of a refusal, and they stay until then" (red against a `dismissRefusal` that did nothing), "a review whose
changes cannot be read still shows, and can be discarded" and "a discard that went through with a note says the
note". Green before and after, as guards (3): "answers the item of the run on the card, not the first item of the
Queue", "a question about discarding does not carry over to the item that is opened next" and "a refused discard
keeps its words through the reload that follows it". In `app/test/loop-view.test.tsx` (red, 3): "the question about
discarding names the run it is about, not another run in the Queue", "a review whose changes could not be read says
so, and does not count them as none" and "a refusal has a button that dismisses it".

### Decisions for the owner

- **A new answer, `discarded { note }`, for the note.** The criteria say the branch went with the repository, and
  `done` carries nothing. Alternatives: a `note` on `done` (touches every `done` in the code and its tests), or no
  wire change and the words only in the step's event. Recommendation: as built. `done` stays for every other discard.
- **The folder check guards every discard, not only the gone-repository one.** The old path deleted a recorded path
  too. Alternative: guard only the new path. Recommendation: as built, since the risk is the same.
- **A repository is "gone" when its folder is missing.** A folder that is there and that git cannot read is refused
  with git's words, as before, because a git failure that is not "gone" must never leave a branch behind. Alternative:
  also treat a folder that is not a repository as gone. Recommendation: as built.
- **A recorded folder that is a link is refused even when it points inside `workspaces`.** Alternative: follow a link
  that stays inside. Recommendation: as built.
- **A refusal stays through reloads and a change of run, until the person presses Dismiss or sends another answer.**
  Alternative: it ends with the run. That is what hid the refusal. Recommendation: as built.
- **A Review card whose changes cannot be read still shows, with the reason, and can be discarded.** Alternative: hide
  the item until the diff reads. That keeps the item that cannot be cleared. Recommendation: as built.
- The item's own alternative, a Dismiss answer on a Review item that leaves the folder, is not built.
- Proposed plan text: none.

### Not done, by design

- The owner's stale items are in the real store, and the folder of `run-1795250812644-1` is in the real `workspaces`
  folder. Nothing here read or changed them. After this ships, the owner presses Discard on each item, and the
  folder goes with it.
- A Review item whose run context is missing from the store ("the daemon has no run") is unchanged. Nothing makes
  one today.
- The owner's re-check of `ts-0138-pn` step 6c is not run here.
