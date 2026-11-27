---
id: ts-0222-gx
kind: task
status: done
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

### Built

- The wire. `core/src/owner/board.rs`: `BoardRemoveParams { board }` and `BoardChanged { removed }`.
  `core/src/owner.rs`: the method `board.remove` (answers `Ack`) and the notice `board.changed`. `pnpm gen` wrote
  `schemas/BoardRemoveParams.*.json`, `schemas/OwnerNotice.*.json` and the matching
  `packages/pack-sdk/src/generated/` files; nothing generated was edited by hand.
- The store. `host/src/store.rs`: migration 15 `board_removed` adds `boards.removed_ms`. `host/src/scratch_store.rs`:
  `Removal` (`Removed`, `AlreadyRemoved`, `Unknown`), `remove_board`, `removed_board_at`, `is_board_removed`.
  `boards()` and `board_root` list live boards only, so a removed board is unknown to every method that names a
  board. `open_board_with` clears the mark when the root matches, and returns the same id. `mark_board_opened`
  (`board.pick`) gives nothing for a removed board. `host/src/corpus_store.rs` `corpus_drop_board` and
  `host/src/corpus_service.rs` `Corpora::remove` (close the service, then drop the cache rows).
- The daemon. `host/src/daemon.rs` `board_remove`: refused for a run agent, for a defaults connection (it is not in
  the `defaults_serves` list, so the existing refusal names the mode), for an unknown board, for a board already
  removed (says so), and for a live run (names the run, stops nothing). Then it marks the board, stops its providers
  (`sources.stop_board`), drops the corpus and publishes `board.changed` through `Hub::publish_board`.
  `board_locate` onto the folder of a removed board is refused in words that name the removed board.
- The CLI. `host/src/board_cli.rs`, `host/src/bin/collide.rs`, `host/src/commands.rs`: `collide board remove <board>`
  takes a board id, or a folder (as given, or from the current folder, or the root text). It prints that the folder is
  not touched and that opening it again brings the board back. An unknown board exits with the id code and lists the
  boards.
- The app. `app/src/loop/loop.ts` `removeBoard` (returns the daemon's words on a refusal, else reloads the list) and
  a `board.changed` handler that reloads it. The window moves by the existing rule (ts-0218-sc): the board with the
  greatest `last_opened_ms` still listed, else the first-run page. `app/src/frame/RemoveBoardWindow.tsx` (new, a
  `Modal`: "Remove <board> from Collide?", the folder is not touched, opening it again brings the board back, what is
  kept, a refusal shown in the window). `MissingBoard.tsx` has the button on; `BoardMenu.tsx` has "Remove from
  Collide…" for the open board; `Frame.tsx` and `App.tsx` wire them. `app/src/owner/methods.ts` has `board.remove`.
- Tests.
  - `host/tests/board_remove.rs` (new, 16): the daemon's refusals and results, the notice, a restart, the same id back
    with tasks, views, files and trust, the corpus rows of the other board kept. Also in `host/src/store.rs` (4),
    `host/tests/corpus_store.rs` (1), `host/tests/daemon_corpus.rs`
    (`a_run_agent_may_not_remove_a_board_and_the_board_stays_listed`), `host/tests/daemon_defaults.rs` (the write list
    gains `board.remove`), `host/tests/board_cli.rs` (6), `host/tests/board_process.rs` (one real-binary test, in a
    temp root with a cleared environment, and the usage cases), `host/tests/command_tree.rs`,
    `core/tests/owner_protocol.rs` (the method and the notice).
  - `app/test/board-remove.test.ts` (new, 10), `app/test/missing-board.test.tsx` and `app/test/frame.test.tsx`
    (updated), `app/browser/fake-remove.spec.ts` (new, 6), `app/browser/fake-missing.spec.ts` (updated),
    `app/browser/boards.spec.ts` (bridge, 2 new: remove from the missing page, then open the folder again for the same
    board and its task; remove the only board from the menu and see the first-run page).
- Red at runtime. The store and daemon tests failed against a stub that did nothing (the board stayed listed), and
  two daemon tests were then made stronger because they passed on it. `board.pick` accepting a removed board was
  found by a test and fixed. The corpus test failed with `corpus_drop_board` emptied. The vitest component tests for
  the two buttons failed when the old disabled button and the hidden menu entry were put back; the fake-owner specs
  `fake-remove` failed (timed out) with the same change. Not seen red: the CLI, process, tree and protocol-sample
  tests were written together with the code they check; the new loop tests failed only for a missing method.

### Decisions for the owner

- A new notice, `board.changed { removed }`. The brief assumed a board-list notice existed; none did. The app reloads
  the whole list on it, which gives every window the same rule for the next board. Recommend: keep it.
- Matching by root only. `board.open` on the folder of a removed board brings it back by root; a folder that moved
  is not offered Locate, since a removed board is not listed. Matching by commits is not built. Recommend: keep it
  until a person asks.
- A removed board is unknown to every method that names a board (`board.pick`, `board.locate`, tasks, runs). Its name
  stays taken, so a new board never gets the name of a removed one. Recommend: keep it.
- `board.locate` onto the folder of a removed board is refused naming that board and saying to open the folder.
  Recommend: keep it, as two boards cannot have one root.
- The corpus service is closed, then its rows are dropped, as on a move.
- A run agent may not remove a board (as for `board.locate`). A defaults connection is refused by the existing
  whitelist.
- The CLI takes an id or a folder. Recommend: keep both, as a person who clears boards whose folders are gone has only
  the root text to go by.
- The confirm is a `Modal` in the page, like Locate and Reset, never a browser dialog. The menu entry reads "Remove
  from Collide…" and the page button "Remove from Collide", as the mock does.
- Review (2026-11-27): the recommendations above are taken; remove hides, as the owner left it. The builder's one
  red test in its full runs, `a_daemon_started_from_a_terminal_survives_the_terminal_closing` ("hangup 0"), came from
  launching the runs under `nohup`: an ignored SIGHUP is inherited, so the test's session leader never ends by it.
  It passes in the review's gates. The `board` row of `core/src/owner.rs`'s method table was already over the line
  width and grows by `board.remove`; a table row does not wrap. The plan's Stage 6a follow-ups now say what was built.
  Clearing the leftover check boards from the owner's real Collide is the owner's (`collide board remove <folder>`, or
  the missing page).

### Not done, by design

- Matching a removed board by commits.
- A `collide board list` command; `remove` lists the known boards when it finds no match.
- A hard delete or purge of removed boards, and a "removed boards" list in the app.
- No change to `IMPLEMENTATION_PLAN.md`.
