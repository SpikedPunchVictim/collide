---
id: ts-0191-nv
kind: task
status: ready
title: "Boards: stable ids, the main checkout, a missing board and locate"
owners: []
links:
  - { type: blocked-by, to: ts-0190-tz }
---

## Goal

A board keeps one id for its life, which a lost store never gives to another repository. A worktree opens as a line
of its repository's board, not as a board of its own. A board whose folder was moved shows as missing, deletes
nothing, and comes back as it was after "locate". The board menu gets the system folder picker.

What exists.
- `host/src/scratch_store.rs` names boards `board-<n>` by count (`BOARD_PREFIX`). `ts-0075-hj` carried two fixes
  here: an id a lost store never reuses (personal views and other files are keyed by it, `UserScope::board_dir`),
  and the board root becoming the main checkout, so a linked worktree is a line.
- `board.open` keys a board by the checkout's own top level. `host/src/checkout.rs` `main_checkout` finds the main
  checkout and is not used there.
- `host/src/corpus_service.rs` notices a board whose folder is gone and looks again every `GONE_PERIODS` rechecks.
  `BoardSummary` (`core/src/owner/board.rs`) has `id`, `root`, `lines` and `sources_unread`, and no missing flag.
  No method moves a board to a new folder.
- From `ts-0179-ke`: the board menu, with a typed folder path.

What this adds.
- New boards get a random id (`board-` and a short random suffix, checked unique in the store). Existing
  `board-<n>` ids are kept, so nothing on disk moves.
- `board.open` on a linked worktree opens the main checkout's board, with the worktree as a line of it. Opening the
  main checkout after a worktree finds the same board.
- `BoardSummary.missing`: the board's folder is gone. The board menu's row says "folder not found" (note 15).
  Selecting it shows the missing page (the mock's `missingHtml`): where Collide looked, "Nothing has been deleted",
  its tasks are kept, and "Locate the folder…". Remove from Collide is shown disabled ("Arrives with board removal").
- `board.locate { board, path }`: checks the folder is the same repository (below), then moves the board's root
  there and keeps its id, scratch tasks, trust entry, views and pack data. A different repository is refused with
  the reason.
- The folder picker: "Open a folder…" and "Locate the folder…" open the system folder picker in the app, through a
  shell-local method (`shell.pick_folder`), answered in `shell.rs` like `shell.open_external`. The bridge and the
  fake-owner page keep the typed path box.

## Success criteria

- **A missing board** (0.2; revision 22): a board whose repository folder was moved shows as missing with
  "locate", deletes nothing, and after "locate" shows its tasks as they were. Its Up next list is Stage 11's (see
  the split's text to apply).
- A lost store (the store file removed) never gives an existing personal-views folder's id to another repository.
- `board.open` on a worktree returns the main checkout's board, and its lines include the worktree.
- Locating a different repository is refused, and the board stays missing.

## Tests

Write these first.

- `host/tests/`: ids (new random, old kept, never reused after a lost store); a worktree opened first and then the
  main checkout; a moved folder becomes missing, its scratch tasks and personal views kept; locate to the moved
  folder; locate to another repository refused.
- `app/test/`: the menu row and the missing page's words.
- `app/browser/boards.spec.ts` (bridge): the test moves a temp repo's folder; the row says "folder not found";
  locate through the typed path; the tasks show again.
- `app/src-tauri/src/shell.rs` unit test: `shell.pick_folder` is answered in the shell through a fake picker and
  never forwarded.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 6a: Line selector ("A missing board's row says 'folder not found'"). Success Criteria: A missing board.
  - 0.2: After a restart, Boards and lines (revision 22). 0.11: Line selector. 0.12: lines.
- Design: notes 1 and 15. Mock functions: `missingHtml`, `locateModal`, `popHtml` (the menu row).
- Carried: `ts-0075-hj` (ids a lost store never reuses; the board root as the main checkout).
- **Owner's choice, recommendation taken (2026-11-24): the folder picker.** Recommend `tauri-plugin-dialog` 2.x, the
  release that matches tauri 2.12 (a new crate; the builder pins it in `Cargo.lock`), called from Rust in `shell.rs`, so
  it needs no capability in the webview. Alternatives: `osascript -e 'choose folder'` from the shell (no dependency; a
  script call, macOS only); or the typed path box in the app as in the browser pages.
- **Decision for the item, with a recommendation: the same repository.** Recommend comparing the root commits
  (`git rev-list --max-parents=0 HEAD`) of the old record and the new folder, recorded on first open. Alternative: the
  `origin` URL; it changes with a fork or a rename and is absent for a local repo.
- **Decision for the item, with a recommendation: the id scheme.** Recommend random ids for new boards only.
  Alternative: rename every `board-<n>` and move its files; more risk for no gain.
- Out of scope: the old path chip and Move (Stage 12); removing a board; the Up next list (Stage 11).
- Area: `host/src/scratch_store.rs`, `host/src/board*.rs`, `host/src/checkout.rs`, `host/src/corpus_service.rs`,
  `host/src/daemon.rs`, `core/src/owner/board.rs`, `schemas/` and the generated files, `app/src-tauri/src/shell.rs`,
  `app/src-tauri/Cargo.toml` (if the plugin is taken), `app/src/` (the menu and the missing page), `host/tests/`,
  `app/test/`, `app/browser/`.
- Dependencies: `tauri-plugin-dialog` 2.x (owner, 2026-11-24).
- The test rule: no test touches the real home, real launchd, a real daemon, the keychain or the network. Each test
  that runs the `collide` binary clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to
  a temp root, as `host/tests/common/mod.rs` `TempRoot` does. A temp repository sets `GIT_CONFIG_GLOBAL` and
  `GIT_CONFIG_SYSTEM` to empty files. Keychain writes go through `MemorySecretStore` (`host/src/secret_store.rs`),
  never the login keychain. Browser tests run the installed Chrome with a temp profile
  (`app/browser/support/chrome.ts`), and the bridge's daemon on a temp root (`app/browser/support/world.ts`). No test
  opens the system folder picker: the picker is a seam.
- Stage 6a item (revision 25 scope, split 2026-11-24). Items of Stage 6a run one at a time, in id order, because they
  share `app/`, `core`, `host` and the generated files. No builder runs `tauri build`; the real picker is checked by
  the owner in `ts-0197-uj`.
- Nothing here runs a real `claude`.
- Owner decisions (2026-11-24): the owner took the recommendations at the split.
