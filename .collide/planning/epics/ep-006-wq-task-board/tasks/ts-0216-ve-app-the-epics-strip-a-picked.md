---
id: ts-0216-ve
kind: task
status: ready
title: "App: the epics strip, a picked epic and its panel"
owners: []
links:
  - { type: blocked-by, to: ts-0190-tz }
---

## Goal

The Field gets a strip of epic cards between the filter bar and the view's own row, in every layout. Picking an epic
lights its cards, opens it in the drawer, and makes it what run order and graph draw. A funnel on each card shows only
that epic's tasks (notes 58 to 61 and 63; revision 30).

What exists.
- `task.list` (`core/src/owner/task.rs` `TaskListResult`): the tasks of a view, and on the first page the board's
  `collections` (`core/src/provider.rs` `Collection`: id, title). From `ts-0187-hb`, `collide/plan` lists each epic
  there, and each task names its epic in `collections` as well as by its `part-of` link (`app/src/field/model.ts`
  `stageOf`).
- From `ts-0181-ft`: the Field, its folds (`app/src/field/folds.ts`, webview storage) and its cards. From `ts-0184-hv`:
  the drawer and its folding parts. From `ts-0187-hb`: the layouts, and a collection as a closure's goal. From
  `ts-0190-tz`: the filter bar, its Epic box, and the view's tasks before the bar's filters, which its counts use.

What this adds.
- **The strip** (notes 58 and 61), under a header that folds it as the drawer's parts fold, kept per board and view.
  It shows the epics that have a task in the view before the filter bar's filters, so a filter never hides an epic.
  One row that scrolls sideways, open work first: an epic with a task under way, then one with a ready task, then the
  rest, and finished ones last. Over 8 epics a find box narrows the row by name or id. The strip keeps one height.
- **A card:** the epic's name, a bar of its tasks by category, "N of M done", how many are open and under way, its id,
  and the funnel. The counts are over the whole board, not the view.
- **Picking** (note 59): a click on a card picks the epic, and a second click or Clear ends the pick. Its cards on the
  board are lit. A line over the board says how many are lit, how many sit in folded columns, and how many are
  outside the view; a folded column says how many lit cards it holds. In run order and graph the line says how many
  open tasks are lit, and how many tasks of other epics they wait on. The epic stays picked while its tasks are
  opened in the drawer. The row keeps its scroll place; it moves only to keep the picked card in sight when the drawer
  opens.
- **The epic in the drawer** (note 60): Run it, with Add to Up next off and saying it comes in Stage 11, See the run
  order and See the graph; then its open tasks in run order, step by step, with any task of another epic that they
  wait on; the finished ones folded; then Details for engineers (its file and its tasks' folder, for the plan source).
- **Run order and graph follow the pick** (note 60): a picked epic is their goal, through `ts-0187-hb`'s collection
  goal, and its own tasks are lit. Choosing a goal in the goal bar picks that epic.
- **The funnel** (note 63): always shown. A click adds the epic to the filter bar's Epic box, and the funnel stays
  pressed while it is there; a second click takes it out. The strip keeps every epic, so another can be chosen.
- The order on the Field: banners, the filter bar, the strip, the view's own row (its name and layouts), the lit line,
  then the layout (round 10).

## Success criteria

- On a fixture board of 50 epics, the strip is one row in board, run order and graph, open work first, and the find
  box narrows it by name and by id. Folding it holds for that view across a reload.
- Picking an epic lights exactly its cards. The lit line's three counts match the board, a folded column shows its
  count, and the row's scroll place is the same before and after the pick.
- The drawer shows the picked epic's open tasks in the steps `task.list` gives for its closure, with the tasks of
  other epics they wait on. Add to Up next is off and says Stage 11.
- In run order and graph the picked epic is the goal, and choosing another goal there picks that epic.
- Its funnel shows only its tasks; the Epic box shows the same choice; a second epic's funnel adds it; the strip still
  lists all 50.
- A view with no task in any epic shows no strip.

## Tests

Write these first.

- `app/test/`: the strip's order (under way, ready, rest, finished); which epics show (before the bar's filters);
  the counts over the board; the lit counts, folded columns and outside the view; the funnel's change to the Epic
  box's choice.
- `app/browser/fake-epics.spec.ts`, over a fake-owner world of 50 epics: the one row, find, the fold held across a
  reload, a pick that keeps the row's scroll place, the lit line, the drawer's steps, the graph's goal, the funnel and
  the Epic box together.

## Notes

- IMPLEMENTATION_PLAN.md: Stage 6a: Field, Epics; Filter bar. Success criteria: Epics. Stage 11: Up next.
- Design: notes 58 to 61 and 63 (rounds 8 to 10, 2026-11-25). Mock functions: `epicsHtml`, `ecardHtml`,
  `elitHtml`, `epicDrawerHtml`, `epicRank`, `epicInSight`, `fieldHtml`; mock scenarios `epics-50`, `epics-50-pick`,
  `epics-only`, `epics-50-graph`.
- **Decision for the item, with a recommendation: where the board-wide counts come from.** Recommend a second
  `task.list` over the whole board (an inline view with no `where`), read with the Field's and refreshed on the same
  `watch` change, counted in the app. Alternative: core adds counts by category to each `Collection` in
  `TaskListResult`; one list instead of two, and a schema change.
- **Decision for the item, with a recommendation: a source with no epics.** The strip shows the board's collections
  whatever the source calls them, headed Epics. Recommend that wording until a second source with collections ships
  (Stage 5's GitHub milestones). Alternative: the source names its collections.
- Area: `app/src/` (an `epics/` folder), `app/src/field/`, `app/src/drawer/`, `app/test/`, `app/browser/`, the
  fake-owner worlds (`app/test/support/worlds.ts`).
- Dependencies: none.
- The test rule: no test touches the real home, real launchd, a real daemon, the keychain or the network. Each test
  that runs the `collide` binary clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to
  a temp root, as `host/tests/common/mod.rs` `TempRoot` does. A temp repository sets `GIT_CONFIG_GLOBAL` and
  `GIT_CONFIG_SYSTEM` to empty files. Browser tests run the installed Chrome with a temp profile
  (`app/browser/support/chrome.ts`).
- Stage 6a item (revision 30, 2026-11-25). Items of Stage 6a run one at a time, in id order, with this one between
  `ts-0190-tz` and `ts-0191-nv`, because they share `app/`, `core`, `host` and the generated files. No builder runs
  `tauri build`.
- Nothing here runs a real `claude`.
