---
id: ts-0055-cr
kind: task
status: done
title: "Built-in git worktree workspace and the disk floor"
owners: []
links:
  - { type: blocked-by, to: ts-0050-xc }
---

## Goal

The built-in workspace and the disk floor. `git worktree add` returning `{ path, gitDir }`; removal that refuses a dirty or unmerged worktree; the free-space check before a workspace is made and before each phase; and the disk space a workspace uses, recorded at phase end.

## Success criteria

- A workspace is made on a new branch in a scratch repository and its `{ path, gitDir }` are returned. Removing it while it holds an uncommitted change, or a commit not merged into its base, is refused with a message that says which.
- With free space under the floor, a workspace is not made and a phase does not start, and the run shows `paused: disk` with a Queue item (Stage 3a criterion; 0.2, Durability). The free-space reading is behind a trait, so tests set it.
- The disk space a workspace uses is recorded at phase end (0.13).
- The workspace moves through the `ts-0050-xc` workspace states.

## Tests

Write first: create and remove in a temporary git repository, both refusals, the floor with a fake free-space reader, the size recorded at phase end.

## Notes

- IMPLEMENTATION_PLAN.md, Stage 3a (Built-in workspace; Clocks and disk); 0.2 (Durability); 0.4 (workspace implementations; the built-in one); 0.13 (disk space per workspace).
- Area: `host`. Pack workspace hooks are Stage 7; this is the built-in one only.
- Stage 3a item (revision 13 scope, split 2026-11-15). Items of Stage 3a run one at a time, in id order, because they share `host` and the generated files.

### Built

- `host/src/disk.rs` (new): the floor. `Floor` and `DEFAULT_FLOOR_BYTES`; the `FreeSpace` trait with `SystemFreeSpace` (`libc::statvfs`, `f_bavail` times `f_frsize`, read on the nearest existing ancestor of the path); `DiskGuard` (`check` before a workspace or a phase, `volume_full` for `PhaseEnding::volume_full`); `DiskPause` (`cause`, `run_state`, `queue_item`) and `DiskError` (`Paused`, `Unreadable`). No new crate: `libc` was already a `cfg(unix)` dependency.
- `host/src/workspace.rs` (new): the built-in workspace. `WorkspaceSpec`, `BuiltinWorkspace` (`provision`, `start_phase`, `end_phase`, `remove`, `state`, `location`, `with_git_env`), `WorkspaceError`, and `disk_use`. `location` and `provision` return core's `HookWorkspace` (`kind`, `path`, `git_dir`), not a new type. Every state move goes through `collide_core::transition::workspace_transition`; a refused removal carries core's `RemoveRefusal` (`Dirty`, `Unmerged`, `InUse`). Git runs through `std::process::Command` with `LC_ALL=C`; answers come from `status --porcelain=v1 -z`, `rev-list --count` and `rev-parse --absolute-git-dir`.
- `host/src/lib.rs`: `pub mod disk;` and `pub mod workspace;`.
- There was no stub tag for this item, so `checks/tests/real_tree.rs` is unchanged.
- Tests added: 27 in `host/tests/workspace.rs` (create returns `{ path, gitDir }` on a new branch; the state moves, and a move core refuses names both states; clean removal takes the folder and the branch; refusals for a changed tracked file, an untracked file, an unmerged commit, in use, and dirty-before-unmerged; an ignored file does not block; a merged commit does not block; an option-like base is refused before git runs; a missing base fails the workspace with git's words; the floor with a fake reader: no workspace made, the same workspace made once space returns, a phase not started, the floor is a strict minimum, an unreadable volume is an error and not a pause, the Queue content, `phase_ending` with `volume_full` gives `failed (disk)`, the real reader on a path that does not exist yet, the default floor; disk use returned at phase end, hard link counted once, symbolic link not followed, the shared git directory not counted, an unreadable path named in the error). `cargo test -p collide-host`: 155 unit and 27 here (plus the other integration files, all passing).

### Decisions for the owner

1. **The floor is 5 GiB of free space, absolute (`DEFAULT_FLOOR_BYTES`). This changes product behaviour: a machine with less than 5 GiB free cannot start a run.** The plan gives no number. 5 GiB is above the 4.1 GB a provisioned reference-pack worktree measured (R3, 0.13), so one can be made and still leave room to write. Passed on: a fraction of the volume (it asks for 25 GB on a 500 GB disk and for too little on a small one, though what a phase needs to write does not grow with the disk); the larger of the two; and the repo's own size plus a margin (best fit, but it needs a measure of the repo before each workspace). Recommend the absolute floor now, and the margin later if people hit it. It is one constant and the `Floor` field, so a setting can carry it later.
2. **A free-space reading that fails is an error (`DiskError::Unreadable`), not a pause and not a pass.** With no reading nothing says space is short, and either guess hides the fault. Alternative: treat it as under the floor, which would stop every run on a volume the daemon cannot read. Recommend the error, which the daemon reports.
3. **The free space is the space an ordinary user can use (`f_bavail`), on the volume of the workspace folder (before a workspace) and of the workspace (before a phase).** Alternative: `f_bfree`, which counts space the system keeps back and overstates what a non-root process can write. Recommend `f_bavail`. A phase's logs and journal live in the run directory, which may be on another volume; the daemon should check that path too (ts-0056).
4. **There is no `QueueKind` for a disk pause, so `DiskPause::queue_item` returns the content (title, detail with free and floor in MiB or GiB, and the answers `resume` and `stop`) and the daemon picks the kind.** Alternative: add `QueueKind::DiskFull` to core now, which changes the generated schemas and the Queue's list in 0.11. Recommend adding it with the daemon item (ts-0056), when the Queue row is drawn. Also: a pause moves the run to `paused: disk` only; a phase that has not started stays `pending`, because core's phase table has no `pending → paused`.
5. **Untracked files block removal; ignored files do not.** Removing a worktree deletes an untracked file that git has no copy of, and an ignored file (build output, dependencies) can be made again. Alternative: block on ignored files too, which would refuse every worktree that has been built. Recommend as built.
6. **A branch that was squash merged or rebased is refused as `unmerged`.** Merged means `git rev-list --count <base>..HEAD` is zero. A squash merge leaves commits the base does not contain, so a person has to discard the work on purpose. Alternative: also accept a branch whose changes are in the base (`git cherry`, or comparing trees), which is more convenient and can wrongly pass. Recommend as built, and revisit when `integrate` (Stage 7) says how a branch is merged.
7. **Removal also deletes the branch Collide made (`git branch -D`), after the commits are known to be in the base.** Otherwise every removed workspace leaves a branch behind. Alternative: keep the branch. Recommend deleting.
8. **Disk use is allocated blocks of the worktree's folder, like `du`.** Ignored files count (they are most of the 4.1 GB), a hard link counts once, a symbolic link counts as itself and is not followed, and the shared git directory (`<repo>/.git/worktrees/<name>` and the object store) is not counted, because other worktrees share it. Alternative: file lengths, which hide sparse files and block rounding, or count the git directory too. Recommend as built. It is measured by the daemon after `exit.json` is written, not by the shim, and it is returned as a number: core has no field for it in the exit record, so the daemon stores it with the phase's totals (0.13).
9. **Core's workspace table does not let a `ready` workspace (made, never used) go to `removable`, so removing one is a transition error.** A run that fails before its first phase leaves such a workspace. Alternative: add `ready → removable` to core's table (ts-0050). Recommend adding it with the daemon item, or running the workspace through `in_use → idle` first, which is untrue.
10. **A failed removal (git failing after the checks pass) leaves the state `removable` and returns git's message**; it does not move the workspace to `failed`, which only the `resume` hook can leave, and the built-in workspace has none. A failed `provision` does go to `failed`, as core's table allows. Alternative: fail the workspace on a failed removal too.
11. **A branch or base that is empty or starts with `-` is refused before git runs** (`WorkspaceError::BadSpec`), and git gets `--` before the path, so task data cannot pass an option to git (0.9).

- Owner decisions (2026-11-15): the recommendations above are accepted as built; the 5 GiB floor is reported to the owner as the one owner-visible choice and stays a single constant until they answer. Follow-ups for Stage 3b (the first run, which drives phases): wire `DiskGuard::check` and `DiskPause` into the run, add `QueueKind::DiskFull` and the core move `ready -> removable` there, and check the run directory's volume too. When `integrate` lands, a branch whose change is already in the base (squash or rebase) should count as merged, per 0.2's rule for a merge that finds its change at the target.

### Not done, by design

- Wiring into a live run: nothing drives runs until the daemon (ts-0056-ag). It will call `DiskGuard::check` before each phase, turn a `DiskPause` into `RunState::Paused { cause: Disk }` and a Queue item, retry when space returns, store the number `end_phase` returns with the phase's totals, and call `volume_full` for `PhaseEnding` when a phase ends with no result. The criterion "the run shows `paused: disk` with a Queue item" is met here only as far as the decision, the state and the Queue content; showing it is that item's.
- The workspace layout (a repo's worktrees under one root in Collide's data directory, `worktreeRoot`, 0.4): the daemon builds `WorkspaceSpec::path`; this item takes the folder as given.
- The reaper that removes `task` workspaces once their tasks are done (0.8), and the discard of `sandbox` workspaces: they call `remove` and belong to the daemon and Stage 7.
- Pack workspace hooks (`create`, `resume`, `remove`, `discard`, `cleanup`) and their `{ path, gitDir }` results: Stage 7. `HookWorkspace` is reused so the two return the same shape.
- A `QueueKind` for a disk pause (decision 4) and `ready → removable` (decision 9): core changes the owner decides.
- The `FreeSpace` reader is Unix only (`statvfs`), like the shim; macOS is the target.
