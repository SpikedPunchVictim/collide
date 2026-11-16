---
id: ts-0078-ye
kind: task
status: ready
title: "Daemon: a prompt run of collide/quick, up to review"
owners: []
links:
  - { type: blocked-by, to: ts-0077-ws }
---

## Goal

The daemon runs `collide/quick` on a scratch task, as a hard-coded sequence, up to the review (Stage 3c's pull from
Stage 8):

- `run.start` with a board, one scratch key and workflow `collide/quick` (or none, which means it);
- a new worktree from the built-in workspace (`host/src/workspace.rs` `BuiltinWorkspace`), on branch `collide/<run>`,
  from the branch the board's checkout has now. The base branch and its commit are recorded at start;
- one agent phase with the built-in `collide/builder` role, with the prompt as its first turn;
- the scratch task `active` at start and `review` when the phase ends;
- the run ends `ended(done)`, and a `Review` Queue item is added, with accept, re-prompt and discard as its answers;
- `run.diff` for the run: the recorded base commit against the branch's head;
- `quick` added to `packs/collide/pack.yaml` as the declaration of what the sequence does.

## Success criteria

- With fake Claude, a prompt becomes a scratch task and a `collide/quick` run in a fresh worktree. The phase ends,
  the run is `ended(done)`, the scratch task is `review`, and the Queue holds one `Review` item for the run.
- `run.diff` lists the files the agent changed, with their counts, and gives one file's patch when asked. Folders
  that `.gitignore` does not hide but 0.11 collapses are collapsed.
- The diff is against the commit recorded at start, even when the checkout has moved on since.
- A permission request during the phase is a Queue item. Allowing it lets the phase go on; denying it is told to
  the agent and the phase goes on (Stage 3c criterion).
- A run that fails or is stopped gets the item too, with only re-prompt and discard, so its worktree is never left
  with no way to remove it.
- A checkout with a detached `HEAD` is refused at start, in Collide's words. Any other workflow is refused as not
  built yet (Stage 8).
- The sequence's phase, role and status moves match the `quick` workflow in `packs/collide/pack.yaml`.

## Tests

Write these first:

- the run from prompt to `Review` item, with fake Claude;
- `run.diff`: two changed files, one patch, a collapsed folder;
- a checkout that moves on during the run;
- a permission request, allowed and denied;
- a phase that fails, and a run that is stopped;
- what the agent left uncommitted (per the decision below);
- a detached `HEAD`; another workflow;
- a board whose root is a linked worktree of the repository (the owner check in `ts-0083-uf` runs on one);
- the sequence against `packs/collide/pack.yaml`.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 3c: what it pulls from Stages 7, 8 and 9a; `collide/quick` compiled into the bundle.
  - 0.4: The built-in `collide` pack (`collide/quick`); the built-in workspace.
  - 0.11: Queue (the review row); the diff's collapsed folders.
  - Stage 9a: Launch from a prompt (what replaces this sequence).
- **Decision for the item, with a recommendation: where the code lives.** Recommend a module of its own, such as
  `host/src/quick_run.rs`, whose doc says Stage 9a replaces it. It shares the worktree and phase code of
  `host/src/file_run.rs` rather than copying it.
- **Decision for the item: the recorded base.** `file_run.rs` `restored_workspace` uses the symbolic `HEAD`. Recommend
  recording the branch name and its commit at start, in the run's record, and using both from then on.
- **Decision for the owner, with a recommendation: what the agent left uncommitted.** The builder role says to
  commit (`host/src/builtin_roles.rs` `BUILDER_DEFINITION`), but an agent may not. Recommend that Collide commits
  what is left at the end of the phase, in a commit of its own whose message says so. Then the diff the person
  reviews is exactly what accept merges. The alternative shows the changes and refuses accept until a re-prompt
  commits them.
- **Decision for the item: the Queue item's words.** A Queue item's title is in Collide's own words and leaves out
  the task title (`core/src/owner/queue.rs` `QueueItem`). The window shows the prompt from `task.get`.
- The run is `ended` before the review. Core allows only an integrate answer out of `ended` (`core/src/transition.rs`,
  `core/src/run_rules.rs`). Accept in `ts-0079-pu` goes through that door.
- The worktrees live under the daemon's workspaces folder (0.4: one root in Collide's data directory).
- Fake Claude (`host/tests/adapters/claude/fixtures/fake_claude.py`) has no step that changes files. Add one that
  writes a file in its working folder and one that commits, so the diff, accept and discard tests have work.
- Tests that run fake Claude go in `host/tests/adapters/claude/`. Code outside the adapter must not spell the Claude
  variables (the names check).
- Run `pnpm gen` if any wire type changes.
- Area: `host` (a new module, `file_run.rs`, `daemon.rs`, `workspace.rs`), `packs/collide/pack.yaml`.
- The test rule: no test touches the real home, real launchd or a real daemon. Each test clears the environment
  (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as `host/tests/common/mod.rs` `TempRoot`
  does. Settings files are fixtures, never the owner's own `settings.json`.
- Stage 3c item (revision 22 scope, split 2026-11-15). Items of Stage 3c run one at a time, in id order, because
  they share `host`, the app and the generated files.
- Owner decisions (2026-11-15, at the split): what the agent left uncommitted (Collide commits it at the end of the
  phase, in a commit of its own). The recommendation, taken under the owner's delegation; the owner may change it
  before the item is built.
