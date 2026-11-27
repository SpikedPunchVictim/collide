---
id: ts-0218-sc
kind: task
status: done
title: "Boards: open the board last picked"
owners: []
links:
  - { type: blocked-by, to: ts-0217-dy }
---

## Goal

A window opens on the board the person last opened or picked, not on the first board in the list. That holds when
the app starts, when a window reloads, and when it leaves or enters Start with defaults (Start normally, Reset…).
So Reset… acts on the board the person was on.

What exists.
- `app/src/loop/state.ts` `synced`: the window keeps its board while `board.list` still lists it, and otherwise takes
  `boards[0]`. A new window, a reload, Start with defaults, Start normally and Reset… all start with no board, so all
  land on `boards[0]`.
- A pick in the board menu is the page's alone (`app/src/loop/loop.ts` `chooseBoard`, the action `board_chosen`): the
  daemon is not told. `board.open` (`host/src/daemon.rs`) records a board the first time its folder is opened, in the
  store's `boards` table (`host/src/scratch_store.rs`, migration 5 of `host/src/store.rs`). `boards()` lists them by
  `id`, the order they were first opened.
- Reset… (`host/src/set_aside.rs` `reset`) moves the person's files and the window's stored state; it does not touch
  the store. Start with defaults serves the methods of `defaults_serves` (`host/src/daemon.rs`) and refuses the rest.
- `ts-0197-uj`'s agent run found it: after a restart, Start normally or Reset the window opened the first board, and
  a Reset confirmed there would set aside that board's views, not the views of the board the person had been on.
- Also found there, small: the defaults banner shows before the board list has loaded, and a Reset… confirmed in that
  moment names "No board is open, so no board folder" (`app/src/App.tsx` `reset` reads `state.boardId` at the click).

What this adds.
- **The daemon keeps the last pick.** The store records, per board, when it was last opened or picked (a new
  migration). `board.open` records it. A new method records a pick from the board menu; the page calls it from
  `chooseBoard`. `board.list` carries the time with each board, or marks the latest; its order is unchanged.
- **The window opens on it.** Where `synced` now takes `boards[0]`, it takes the board picked last, and `boards[0]`
  only when none has a time (a store from before this item). A board whose folder is gone still opens, on its
  "its folder is gone" page.
- **Start with defaults.** The new method is not on `defaults_serves`: the window saves nothing in that mode, so a
  pick there is not recorded. Reading the last pick (`board.list`) is served, so the mode opens on the board the
  person was on, and Reset… acts on it.
- **Reset… waits for the board list.** The banner's Reset… is off until the first `board.list` has answered, so the
  window it opens names the board that is about to show.
- **The checklist.** `ts-0197-uj`'s steps 22 and 26 to 29 say a restart opens the first board and to pick
  `<s2 repo>` again; they say instead that the window opens on the board last picked. Its agent-run section stays as
  it was.

## Success criteria

- Open board A, then board B, pick A in the menu, restart the app (a new connection): the window opens on A.
- Pick B, Start with defaults: the window opens on B; Reset… names B's folder and moves B's views.
- In defaults mode pick A, Start normally: the window opens on B (the pick in defaults mode was not recorded).
- A store from before this item (no times) opens on `boards[0]` as now, and the first pick records a time.
- The board picked last, with its folder gone, opens on its "its folder is gone" page.
- Reset… in the banner cannot be clicked before the board list has loaded.
- The new method refused on a defaults connection, with the mode named, as other writes are.

## Tests

- `host/tests/` (a temp root, `env_clear`): the migration on a store from before it; `board.open` and the new method
  record the time; `board.list` carries it; the new method refused in defaults mode; an unknown board id refused.
- `app/test/` (vitest): `synced` with times takes the latest; with none takes `boards[0]`; a board still listed is
  kept as now.
- `app/browser/` (bridge): two boards, pick the second, reload: the second shows. Pick, Start with defaults, Reset…:
  the picked board's views are set aside (extend `defaults.spec.ts`). Reset… is off until the list loads.
- The schema: the new method's params and result in `schemas/`, with the checks that hold the method list
  (`collide-checks`).

## Notes

- IMPLEMENTATION_PLAN.md: Stage 6a (Line selector; Start with defaults); revision 32.
- **An item decision: a pick in defaults mode.** Recommend not recording it, as above: the mode saves nothing, and
  the plan says so. Alternative: record it, since it is daemon state and not the person's files; then Start normally
  after a pick in defaults mode opens the board picked there.
- **An item decision: one time per board or one "last" row.** Recommend a column on `boards`, so a board removed later
  takes its time with it. Name the method as the others are named (`board.*`).
- Other clients: the CLI picks no board. A second window opens on the board picked last in any window.
- The test rule: no test touches the real home, real launchd, a real daemon, the keychain or the network. Each test
  that runs the `collide` binary clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a
  temp root, as `host/tests/common/mod.rs` `TempRoot` does. A test never stubs or bypasses a safety gate (the trust
  check, `env_clear`, the defaults gate) to make a result appear. Never `fs::write` over an executable that has run
  (macOS 26.3 kills it on its next run): use `common::write_executable`.
- Nothing here runs a real `claude`. New dependencies: none expected.
- Stage 6a item (revision 32, 2026-11-26), from `ts-0197-uj`'s agent run. It runs after `ts-0217-dy` and before
  `ts-0197-uj`, whose steps 22 and 26 to 29 it shortens.
- Owner decision (2026-11-26): the window opens on the board last picked, built before the owner's check (chosen over
  building it after the check, and over keeping the first board).

### Built

The store keeps the time a board was last picked, the daemon records a pick, and a window opens on the board with
the latest time. A window sends a pick for what it picks in the menu and for a folder it opens. `board.open` itself
marks nothing, so the command line's opens do not move the window.

- `host/src/store.rs`: migration 14, `board_last_opened`, adds `boards.last_opened_ms` (nullable). A board never
  picked since has none.
- `host/src/scratch_store.rs`: `StoredBoard.last_opened_ms`, read by `boards()`; new `mark_board_opened(board, now_ms)`.
  The marked board gets `max(now, latest of the others + 1)`, so no two boards share a time and the latest is never in
  doubt, even if the clock stands still or goes back. It returns the time it set, or `None` for a board the store
  does not have.
- `core/src/owner/board.rs`, `core/src/owner.rs`, `core/src/schema.rs`: `BoardSummary.last_opened_ms` (optional, the
  stored time, given by `board.list` and `board.open`); new method `board.pick` (0.11), params
  `BoardPickParams { board }`, result `Ack`. `pnpm gen` wrote `schemas/BoardPickParams.*`, the changed
  `BoardList|BoardLocate|BoardOpenResult` schemas and the matching files in `packages/pack-sdk/src/generated/`.
- `host/src/daemon.rs`: `board_pick` (unknown board: `NOT_FOUND`), `mark_board_opened`; `board_open` marks nothing and
  answers with the board's stored time; `board_list` carries the time. `board.pick` is not in `defaults_serves`, so
  a defaults connection is refused with `NOT_PERMITTED` and the mode named (the doc of `defaults_serves` says so).
- `app/src/loop/state.ts`: `openingBoard`; `synced` takes it where it took `boards[0]`. `app/src/loop/loop.ts`:
  `chooseBoard` switches at once, then tells the daemon (`recordPick`, which ignores a failure: the pick stands in the
  window); `openChecked` does the same for the board a folder opened. `createLoop(owner, { defaults })`: a defaults
  window makes no pick call. `useLoop`, `App.tsx` pass it.
- `app/src/defaults/DefaultsBanner.tsx`: `canReset`; Reset… is off while `state.loading` (first sync not done).
- `app/src/owner/methods.ts`: `board.pick`. `app/test/support/loop-world.ts`: `board.pick`, `listGate`, times.
- `app/browser/fake/main.tsx`: the `hold-boards` fragment flag and `collideFake.releaseBoards()`.
  `app/browser/support/world.ts`: `repository`, `latestBoard`.
- `.collide/.../ts-0197-uj-owner-check-the-board-and-s6s.md`: steps 22 and 26 to 29 only.

Tests, by name. Red at runtime = failed against a stub with the old behavior (not a compile error), then green.
- `host/src/store.rs`: `a_store_at_version_thirteen_gains_the_last_opened_time_and_its_boards_have_none`,
  `a_marked_board_has_the_time_and_an_unmarked_one_has_none`,
  `the_board_marked_last_has_the_latest_time_when_the_clock_stands_still_or_goes_back` (all three red);
  `marking_a_board_the_store_does_not_have_changes_nothing` (passes with the stub, as it must).
- `host/tests/board_identity.rs`: `opening_a_board_marks_nothing_and_a_pick_marks_it_and_the_list_keeps_its_order`,
  `a_board_open_with_no_pick_leaves_the_latest_board_as_it_was` (the command line's case) and
  `a_store_from_before_lists_no_times_and_the_first_pick_marks_one` (all three red when `board.open` marked);
  `a_pick_marks_the_board_and_a_restarted_daemon_still_has_it`,
  `a_pick_of_a_board_the_daemon_does_not_have_is_refused_and_marks_nothing` and
  `a_board_whose_folder_is_gone_keeps_its_time_in_the_list` (red against a stub `board.pick` before it was written).
- `host/tests/daemon_defaults.rs`: `defaults_mode_refuses_a_pick_and_its_open_changes_no_time`. The refusal needed no
  stub: `board.pick` was already outside `defaults_serves`.
- `app/test/board-last-opened.test.ts` (14): eight red before the change (latest wins; a timed board beats untimed
  ones; a missing board can be the latest; a no-longer-listed board falls to the latest; a pick tells the daemon; a
  reload opens on the pick; a refused pick still switches; an opened folder is picked). Four hold either way (first
  listed when no times; no boards; a still-listed own board is kept; a defaults loop makes no pick call). The
  `opening a folder in the window` tests: the open picks the board it opens (red with the pick removed from
  `openChecked`); a defaults window's open picks nothing; a folder that did not open picks nothing.
- `app/test/defaults.test.tsx`: Reset… off while `canReset` is false (red); on while true.
- `app/browser/boards.spec.ts` (bridge): `a board picked in the menu is the one a reload opens on, not the first or the
  last opened` (red with `openingBoard` returning `boards[0]`); `a folder opened in the window is picked, so a reload
  opens on it` (red with the pick removed from `openChecked`); `a board.open with no pick, as the command line does,
  leaves the board a window opens on alone` (red when `board.open` marked).
- `app/browser/defaults.spec.ts` (bridge; both red with `openingBoard` returning `boards[0]`): `Start with defaults
  opens on the board last picked, and Reset moves that board’s files`; `a board picked in defaults mode is not the
  board Start normally opens on`.
- `app/browser/fake-defaults.spec.ts`: `Reset is off until the boards have been read, and then names the board that
  shows`. Not run red in the browser; its rule is the red vitest test above. It is a fake spec, not a bridge one: a
  bridge daemon answers `board.list` too fast to see the window before it.
- `app/browser/fake-shell.spec.ts`: the board-menu test now expects the `board.pick` call among the calls a switch
  sends (it still checks that no stop or cancel is sent).

### Decisions for the owner

- **A pick in defaults mode is not recorded.** `board.pick` stays off `defaults_serves` and the page makes no call
  there (an open in a defaults window sends none either). Alternative: serve it, so a pick in defaults mode is what
  Start normally opens on. Recommend: not recorded.
- **Only a window marks a board; `board.open` marks nothing.** The command line calls `board.open` for `config get`,
  `config set`, the pack commands and the board commands, so a marking `board.open` would let terminal use in a repo
  move the board the app opens on. A window sends `board.pick` after a menu pick and after a folder it opened (not in
  a defaults window). A board opened only by the command line is never the one a window opens on until a window
  opens or picks it. Alternative: a flag on `BoardOpenParams` that the command line sets so it does not mark (one
  more wire change). Not taken. The item's note "the CLI picks no board" is kept.
- **The time is a column on `boards`** (migration 14, `last_opened_ms`), with strictly increasing marks. Alternative:
  a `last_opened` row. Recommend: as built.
- **Wire shape.** A per-board `last_opened_ms` on `BoardSummary`, given by `board.list` and `board.open`, both from
  the store (so the two answer the same summary, as `board_open_answers_the_same_line_as_board_list` holds); a
  `board.locate` result carries none, and `board.locate` does not mark. Alternative: one `last_opened` id on the list
  result. Recommend: as built, it keeps the page's rule in one function over the list it already reads.
- **A failed `board.pick` is ignored by the page.** The pick stands in the window; the next window may open on the
  old board. Alternative: show a notice. Recommend: ignore, the failure cases are a stopping daemon.
- **Reset… is off while `state.loading`.** If the first sync fails it turns on with no board, as before. Alternative:
  keep it off until a board shows. Recommend: as built, a window that cannot reach the daemon should still offer it.
- **The Reset… test is a fake-owner spec, not a bridge one** (the item listed a bridge spec; see Built).
- **Plan text to propose** for Stage 6a, Line selector, "The board a window opens on": "A window opens on the board
  the person last picked or opened in a window. The daemon keeps the time per board (`board.pick` sets it; `board.list`
  carries it). A window sends `board.pick` for a board it picks or a folder it opens; `board.open` marks nothing, so
  the command line never moves it. A window that started with defaults sends nothing and is not served `board.pick`.
  With no time anywhere, the first board lists first, as before."

- Review (2026-11-26): the recommendations above are taken. Changed in review: the first build marked a board on
  every `board.open`, and the command line opens the board for `config get`, `config set`, the pack and board
  commands, so terminal use moved the board the app opens on. Only a window marks now (`board.pick` after a menu
  pick or a folder opened); `board.open` marks nothing (`board_identity.rs`
  `a_board_open_with_no_pick_leaves_the_latest_board_as_it_was`).

### Not done, by design

- A notice when `board.pick` fails. Left as ignored.
- The wire doc table in `core/src/owner.rs` has one row over 120 columns (the `board` row, which grew by `board.pick`).
  A markdown table row cannot wrap; the `config` row is already longer.
- `ts-0197-uj`'s agent-run section is untouched; its owner check is still to be run (steps 22 and 26 to 29 are updated).
