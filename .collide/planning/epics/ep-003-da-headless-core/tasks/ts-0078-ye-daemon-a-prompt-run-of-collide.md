---
id: ts-0078-ye
kind: task
status: active
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

### Built

The daemon serves `run.start` for a prompt and `run.diff`. A scratch task runs through `collide/quick` in a fresh
worktree on `collide/<run>`, up to a `Review` Queue item. Nothing in `core` changed, so `pnpm gen` had nothing to
regenerate (`RunStartParams`, `RunDiffParams` and the diff types were already wire types).

- `host/src/file_run.rs` (generalised, not copied): `FileRuns::start` now builds a `Begin` and calls `begin`, which is
  the start both kinds of run go through (base environment, record, `linked` hook, worktree, phase). `check_agent` is
  the role, alias and budget check, shared. `RunContext.task_file` is optional and `RunContext.quick` holds the prompt
  run's record. `on_phase_end` and `stop_unstarted` call `finish_quick` before the final state is recorded, and keep a
  prompt run's context in the store (`forget_context`). `restored_workspace` uses the recorded commit as the base.
  The Accept, Reprompt and Discard answers say "not built yet" (they used to fall to the generic refusal).
- `host/src/file_run/quick_run.rs` (new, a child module so it reaches `FileRuns`' private state):
  - `QUICK`: the sequence's workflow, phase, role and the two categories. `QuickRun`: board, task, base branch and base
    commit, stored in the run's context.
  - `start_quick`: refuses another workflow ("not built yet (Stage 8)"), a line, profile, launch parameters or
    interactive mode (each "not built yet"), more or fewer than one task, a task that is not a scratch key, missing or
    not `ready`, a detached `HEAD` and a repository with no commit. All problems come together, before any run exists.
    Then `begin`; once the run is recorded `claim` (under `FileRuns.claims`) re-reads the task, links the run
    (`set_run_launch`: board, task with its status at start, line = base branch, workflow) and moves the task to
    `active` by category, recording the change as the run's. A start that fails after the claim puts the task back to
    `ready`.
  - `finish_quick`: commits the leftovers (`Commit what the agent left uncommitted`, author `Collide`, hooks run,
    signing off), maps the phase to `ended(done)`, `failed` or `stopped`, moves the task to `review`, and pushes the
    `Review` item (id `<run>-review`, the task key, Collide's own title) before the run shows as final.
  - `diff` and `is_prompt_run`.
- `host/src/run_diff.rs` (new): base commit against the branch head, from `git diff --raw/--numstat -z -M` with
  `--no-ext-diff --no-textconv`. Collapses `node_modules`, `.pnpm-store`, `target`, `dist`, `coverage` (outermost
  folder of the path, with file count and size from one `cat-file --batch-check`), always lists lock files, caps at
  1000 files and 100 folders with the rest in `more`, cuts a patch at 256 KiB and says so.
- `host/src/workspace.rs`: `Git` is `pub(crate)` with `with_env` and `run_with_input`, so the prompt run and the diff
  run git the way the workspace does.
- `host/src/daemon.rs`: `run.start`, `run.diff`; `run.get` gives `implementations: {workspace: builtin}` for a prompt
  run; `task.get` fills `runs` from the index. Module doc updated.
- `packs/collide/pack.yaml`: `quick` (one `build` phase with the builder, `on_start` active, `on_end` review when
  `outcome == "done"`, `integrate: collide/merge-local`).
- Tests' support: `fake_claude.py` has steps `write_file` and `git_commit`; the `Fixture` of `file_run.rs` also sets
  `HOME` for the daemon's git, and `no_result_recording` is shared.

**Tests.** None touches the real home, launchd or a daemon: each has a temp root, `env_clear()` for its own git, and
fake Claude as `claude`.

- Seen red before the code: all 21 of `host/tests/adapters/claude/quick_run.rs` (20 failed with `method not found` for
  `run.start`, one with `run.diff`'s `method not found`) and
  `core/tests/pack_manifest.rs` `the_collide_pack_declares_quick_as_one_builder_phase_that_moves_the_task_by_category`
  (no `quick`). `host/tests/quick_sequence.rs` (3) did not compile before `QUICK` existed.
- `quick_run.rs` names: `a_prompt_runs_in_a_fresh_worktree_on_its_own_branch_and_ends_in_a_review_item`,
  `the_prompt_is_the_first_turn`, `the_workflow_may_be_named`,
  `run_diff_lists_the_files_with_counts_collapses_folders_and_gives_one_patch`, `a_file_with_no_change_has_no_patch`,
  `a_run_the_daemon_never_heard_of_has_no_diff`, `a_file_task_run_has_no_review_diff`,
  `the_diff_is_against_the_commit_recorded_at_start_when_the_checkout_moves_on_during_the_run`,
  `what_the_agent_left_uncommitted_is_committed_by_collide_in_a_commit_that_says_so`,
  `a_phase_that_left_nothing_uncommitted_gets_no_commit_of_collides`,
  `a_permission_request_in_a_quick_run_is_a_queue_item_and_allow_lets_the_phase_go_on`,
  `a_deny_in_a_quick_run_is_told_to_the_agent_and_the_phase_goes_on`,
  `a_phase_that_fails_gets_a_review_item_with_only_reprompt_and_discard`,
  `a_run_that_is_stopped_gets_a_review_item_with_only_reprompt_and_discard`,
  `a_checkout_with_a_detached_head_is_refused_in_collides_words`, `another_workflow_is_refused_as_not_built_yet`,
  `a_task_that_is_not_there_or_not_ready_or_not_scratch_is_refused_with_its_key`,
  `a_task_with_a_run_going_cannot_be_started_again`, `a_run_that_will_not_start_leaves_its_task_ready`,
  `a_board_whose_root_is_a_linked_worktree_runs_from_the_branch_it_is_on`,
  `accept_reprompt_and_discard_are_not_built_yet`.
- Written with the code, not seen red: the five unit tests in `run_diff.rs` (collapse rules, lock files, cap, rename
  parsing). The cap and the rename path are not reached by a daemon test.
- Checks: `cargo build --workspace`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo fmt --all --check`, `pnpm test`, `pnpm typecheck`, `pnpm --filter @collide/app build`, `pnpm build:packs` and
  `pnpm check` all pass. The `quick_run` tests ran four times in a row with no failure.

### Decisions for the owner

1. **A failed or stopped run's task goes to `review` too.** Recommend yes: the run waits on the person (re-prompt or
   discard), and the Review item is how they find it. The alternative leaves it `active`, which reads as "a run is
   going". The `quick` workflow's `on_end` says `review` only when `outcome == "done"`; plan text proposed: "A run
   that does not end `done` also moves its task to review, because its Review item holds the decision."
2. **A commit of the leftovers that fails fails the run.** The repository's own hooks run on it (the user's rule: never
   bypass hooks), so a hook that refuses the commit makes an `ended` phase a `failed` run, with a Review item of
   re-prompt and discard. Recommend keeping it: the alternative is a diff that is not what accept merges. A failed or
   stopped run's commit failure is only logged.
3. **The recorded base is a commit, and `BuiltinWorkspace`'s `base` is that commit.** Removal checks "merged into
   base", and after accept the commits are in the base *branch* and not ahead of the start commit, so ts-0079 and
   ts-0080 must decide what removal checks: recommend they build the removal spec with `base_branch` (accept) or a
   forced removal (discard), both from `QuickRun`.
4. **Permission items carry no task key** (the pause path that makes them knows only the run). The run index links
   them. Recommend leaving it unless the window needs the key on the item.
5. **Refusals of what a prompt cannot choose yet:** `line`, `profile`, `launch` and `mode: interactive` are refused
   as "not built yet" and not ignored, so a client never believes a choice was honoured.

- Owner decisions (2026-11-15): decisions 1 to 5 taken under the owner's delegation. Review: decision 1 made the code
  move every ended quick run's task to review, but `packs/collide/pack.yaml` still said `when: outcome == "done"`;
  the declaration now matches (no `when`), with `core/tests/pack_manifest.rs` updated, so no plan text is needed. The
  leftovers commit is authored as Collide, with the repository's hooks run and signing off. The `run_diff` unit tests
  were not seen red. Carried to `ts-0079-pu` and `ts-0080-tq`: decision 3, the removal check after accept
  (`base_branch`) and discard (forced).

### Not done, by design

- Accept, re-prompt and discard (ts-0079-pu, ts-0080-tq): their answers are refused "not built yet" and the Review item
  stays. Nothing removes a prompt run's worktree or branch, and its context stays in the store for them.
- The engine, `then:` and the manifest's `on_start`/`on_end` operations: the sequence is hard-coded, and
  `quick_sequence.rs` holds it to `pack.yaml` until Stage 8.
- A test of a daemon restart after the review: the context is stored and `FileRuns::new` reads it back, but only the
  existing file-run restart tests exercise that path.
- `queue.list`'s `board` filter (still ignored, as before), `RunGetResult.integrate` and `latest_run` on `task.get`.
- Streaming a very large patch: the whole patch is read, then cut at 256 KiB.
