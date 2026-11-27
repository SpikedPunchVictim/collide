---
id: ts-0222-gx
kind: task
status: ready
title: "Boards: remove a board from Collide"
owners: []
links:
  - { type: blocked-by, to: ts-0221-pp }
---

## Goal

A person can remove a board from Collide: it leaves the board menu and every window, and its folder is not touched.

What exists.
- `app/src/frame/MissingBoard.tsx`: "Remove from Collide" is there and off ("Arrives with board removal"). No stage
  builds it.
- The store (`host/src/store.rs`): `boards` holds each board; `scratch_tasks`, `runs` and the `corpus_*` tables carry a
  board. A person's files for a board sit in `~/.config/collide/boards/<board>/` (views and the like).
- `ts-0197-uj`, the owner's run: the owner's real Collide keeps five boards from earlier checks whose folders are
  gone, and none can be removed.

What this adds.
- A `board.remove` method. Recommended behaviour (see Notes): the board is marked removed in the store and leaves
  `board.list`. Its scratch tasks, runs and the person's files for it are kept. Opening its folder again (same root
  commits, `ts-0191-nv`) brings the board back with them. Its corpus rows, a cache, are dropped.
- "Remove from Collide" on the missing page, and "Remove from Collide…" in the board menu for the open board. Each
  asks first, and says the folder is not touched and that opening the folder again brings the board back.
- A window on a removed board moves to the board picked last (`ts-0218-sc`), or to the first-run page if none is left.
- `collide board remove <board>` for the CLI, so a person can clear boards without the app.
- Not served in Start with defaults (it writes), as other writes are refused there.

## Success criteria

- Remove a board whose folder is gone from its missing page: it leaves the menu in every open window.
- Remove the open board from the menu: the window opens on the board picked last; with none left, the first-run page.
- Open the removed board's folder again: the board is back with its scratch tasks, runs and saved views.
- A run in progress on the board: removal is refused, naming the run.
- The method refused on a defaults connection, with the mode named.

## Tests

- `host/tests/` (a temp root, `env_clear`): the migration; remove, list, open again; refused with a live run;
  refused in defaults mode; an unknown board refused; the CLI command.
- `app/test/` (vitest): the confirm's words; the window's next board after a remove.
- `app/browser/` (bridge): remove from the missing page and from the menu (extend the boards spec).
- The schema: the method in `schemas/`, with the checks that hold the method list (`collide-checks`).

## Notes

- IMPLEMENTATION_PLAN.md: Stage 6a (Line selector; the missing board's page, note 15); revision 33.
- **A decision for the owner, recommended here: remove hides, it does not delete.** The recommendation keeps
  everything but the cache, so a remove can be undone by opening the folder again, and nothing the person typed is
  lost. Alternative: delete the board's rows and move the person's files to `set-aside/` (as Reset… does); then
  opening the folder again starts a new, empty board. The owner chose to build removal (2026-11-27); the build
  follows the recommendation unless the owner says otherwise.
- **An item decision: a live run.** Recommend refusing, naming the run, over stopping it.
- The test rule: no test touches the real home, real launchd, a real daemon, the keychain or the network. Each test
  that runs the `collide` binary clears the environment and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root
  (`host/tests/common/mod.rs` `TempRoot`). A test never stubs or bypasses a safety gate.
- Nothing here runs a real `claude`. New dependencies: none expected.
- After the build, the owner clears the leftover check boards from their real Collide with the app or the CLI. That
  is the owner's to run; no agent runs `collide` against the real daemon root.
- Stage 6a item (revision 33, 2026-11-27), from `ts-0197-uj`'s owner run. It runs after `ts-0221-pp`, and Stage 6b
  (`ts-0198-tf`) waits for it.
