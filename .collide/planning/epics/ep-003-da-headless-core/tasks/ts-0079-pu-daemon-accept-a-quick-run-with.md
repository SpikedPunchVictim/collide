---
id: ts-0079-pu
kind: task
status: ready
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
