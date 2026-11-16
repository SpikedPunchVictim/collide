---
id: ts-0080-tq
kind: task
status: active
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
