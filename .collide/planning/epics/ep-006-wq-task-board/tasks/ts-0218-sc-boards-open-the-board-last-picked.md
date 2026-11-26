---
id: ts-0218-sc
kind: task
status: ready
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
