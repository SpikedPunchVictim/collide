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
