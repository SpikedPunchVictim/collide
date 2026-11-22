---
id: ts-0167-jj
kind: task
status: ready
title: "Boards: git in a folder that is gone says to install git"
owners: []
links: []
---

## Goal

Seen in the real daemon's log during the owner's re-check of `ts-0138-pn` steps 6b and 6c (2026-11-21): for each board
whose folder was deleted, the corpus logged "`git` did not run (No such file or directory (os error 2)); install git
and try again". Git was installed; the folder was gone. The private `git` helper in `host/src/boards.rs` runs
`Command::new("git").current_dir(folder)`, and a `current_dir` that does not exist fails the spawn with `NotFound`,
which the helper reports as git missing. `ts-0163-su` already makes the corpus log the gone folder as its reason, but
every other caller of the helper (quick runs, accept, the board lines) still gives the wrong fix.

## Success criteria

- Git run in a folder that does not exist says the folder is gone and names it, not that git is missing.
- Git that is truly missing (not on `PATH`) still says to install git.

## Tests

- Write first and see red: the helper's caller in `boards.rs` (or the smallest public function that reaches it) on a
  folder that does not exist: the message names the folder and does not say "install git".
- The missing-git case, with a `PATH` that has no git, still says "install git" (a guard; it may already be covered).

## Notes

- Recommendation: the helper checks the folder before it spawns, and words the message the way `checkout_of` does.
  Alternative: map `NotFound` after the spawn by checking the folder then. Same result, one more branch.
- Low priority.
- Owner decisions: none recorded yet. The recommendations stand until the owner answers.

### Built

The private `git` helper in `host/src/boards.rs` checks the folder before it spawns. A folder that is not a directory
now fails with "`<folder>` is not a folder (it is gone or was moved), so `git` cannot run in it; check that the folder
is still there". The words follow `checkout_of` and `BoardCorpus` in `host/src/corpus_service.rs`. Git that is not on
`PATH` still fails the spawn and still says "`git` did not run (...); install git and try again". The error stays a
`String`; no caller needs to tell the two apart.

**The callers of the helper, and which messages change.**

- `git_dir_of` (public): changes. Callers are the board line (`host/src/board_lines.rs`, `LineError::Failed`) and the
  three pack-config reads in `host/src/config_get.rs` (`PackError::Failed`, and the refusal that names the board). A
  board whose folder is gone now says so, not "install git".
- `worktrees` (public): changes. The caller is the corpus's `discover` in `host/src/corpus_service.rs`. Its unread
  reason already has a fixed text for a folder that is not a directory (ts-0163-su), so the log shows no change; any
  other caller of `discover`'s error would.
- `checkout_of` (public): no change for a folder that is gone, because it checks `is_dir` first and words it ("is not a
  folder, so there is nothing to open"). That covers `board.open`'s `git_repo` check, quick runs, accept, and the board
  lines. The helper's check changes it only if the folder goes between that check and the spawn, or if the checkout's
  top level goes between `rev-parse` and the branch read (the `root` calls); both now say the folder is gone.
- `head_commit`, `branch_commit`, `line_commit`: no message. They drop the error and give `None`.
- `held_branch`: runs inside `checkout_of`; the same as above.

So the item's note that quick runs, accept and the board lines gave the wrong fix holds only through `git_dir_of`
(the board line) in this tree; `checkout_of` already worded it.

**Tests** (`host/tests/corpus_checkouts.rs`, where `worktrees` has its tests; `boards.rs` has no unit tests):

- `git_in_a_folder_that_is_gone_says_so_and_does_not_say_to_install_git` (`git_dir_of`): seen red at runtime, with
  "`git` did not run (No such file or directory (os error 2)); install git and try again".
- `the_worktree_list_of_a_folder_that_is_gone_says_so_and_does_not_say_to_install_git` (`worktrees`): seen red at
  runtime, with the same message.
- `git_that_is_not_on_the_path_still_says_to_install_git`: the guard. It passed before the change, as the item
  expected. `PATH` is set in the environment list the helper gives its own `Command`, to an empty temp folder, so the
  test process's environment is not touched.

The code was the old behavior when the two red tests ran, so no stub was needed.

### Decisions for the owner

- **The check is `!folder.is_dir()`, not "does not exist".** A path that is a file gets the same message; git cannot run
  there either, and `checkout_of` words it the same way. Alternative: `exists()`, with a separate message for a file.
  Recommendation: as built.
- **The message says "is not a folder (it is gone or was moved)".** It matches `BoardCorpus`'s reason text. Alternative:
  "is gone" alone, which is wrong for a file. Recommendation: as built.
- **Check before the spawn, not map `NotFound` after.** This is the item's recommendation. The alternative also covers
  a folder deleted between the check and the spawn, which now still says "install git" (a window of microseconds).
  It costs one more branch on the error kind. Recommendation: as built.
- **No new error type.** Alternative: an enum a caller could match on. No caller does. Recommendation: as built.
- **The tests are in `corpus_checkouts.rs`.** The callers are all public functions, so `host/tests/` reaches them with
  no new test hooks. Alternative: a new file. Recommendation: as built.

No plan text change is proposed.

### Not done, by design

- **No second wording for the corpus.** `BoardCorpus` keeps its own fixed text for a folder that is gone; it checks
  the folder before it calls the helper, so there is nothing to merge. Merging the two texts is not asked for.
