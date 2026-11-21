---
id: ts-0160-th
kind: task
status: ready
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
