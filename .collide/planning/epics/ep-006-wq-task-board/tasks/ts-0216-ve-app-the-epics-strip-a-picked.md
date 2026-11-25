---
id: ts-0216-ve
kind: task
status: done
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

### Built

Wire and pack (the `path` choice, see Decisions):

- `core/src/provider.rs`: `Collection` gains `path: Option<String>` (`serde(default, skip_serializing_if)`), the file
  the source keeps the collection in. `core/tests/view_eval.rs` and `core/tests/corpus.rs` set `path: None`.
  `pnpm gen` was run: `schemas/{CollectionsChanged,ListResult,TaskListResult}.{input,output}.json` and the six
  matching `packages/pack-sdk/src/generated/*.ts` changed, and nothing else was hand-edited.
- `packs/plan/src/scan.ts` `collectionsOf`: each epic's collection carries `path: found.path` (its `epic.md`).
- Tests: `core/tests/provider_protocol.rs`
  `a_collection_may_name_its_file_and_one_that_names_none_is_written_as_before`;
  `packs/plan/test/collections.test.ts` (list expectations carry `path`; "names the epic.md of each epic as its
  path..."); `packs/plan/test/watch.test.ts` helper yields a `path`.
  Red at runtime first: the Rust test panicked "unknown field `path`" (field added as `serde(skip)` to get past the
  compile error, then made real); four `packs/plan` tests failed on the missing `path` before the scan change.

App:

- `app/src/epics/model.ts`: `epicsOf` (the collections with a task in `FieldData.inView`, counted over
  `FieldData.corpus`, so there is no new read), `rankOf` (category only, never a status name), `cardOf`,
  `findEpics`, `litIds`, `boardLit`, `outsideView`, `boardWords`, `walkWords`, `progressWords`, `BAR_ORDER`, `runOf`,
  `outsideSteps`, `epicTitleOf`.
- `app/src/epics/fold.ts` (`readStripFolded`, `setStripFolded`; one fold per board and view),
  `app/src/epics/EpicStrip.tsx` (`EpicStrip`, `keepInSight`; the header fold, the find box over 8 epics, one row that
  scrolls sideways, a card with a pick button and a funnel button), `app/src/epics/LitLine.tsx`.
- `app/src/drawer/EpicDrawer.tsx` (`EpicDrawer`, `EpicView`: Run it with Add to Up next off and saying Stage 11, See
  the run order, See the graph, open tasks by step with the tasks of other epics named by epic, finished tasks
  folded, Details for engineers with File and the task count), `app/src/drawer/epic-walk.ts` (`useEpicWalk`).
- `app/src/drawer/Drawer.tsx`: `DrawerShell` extracted (width, edge, Escape), shared with the epic's panel.
  `app/src/drawer/DrawerView.tsx`: `PartSection` extracted, unchanged markup. `app/src/field/load.ts`: `loadWalk`
  exported.
- `app/src/field/FieldPage.tsx` (`Board`): the pick, the epic drawer, the goal following the pick, the order banners,
  filter bar, strip, view row, lit line, layout; `ShutColumn` says its lit count. `CardButton.tsx`, `Layouts.tsx`
  (`RunOrder`, `Graph`): a lit card.
- `app/test/support/field-world.ts` `epicsStripWorld` (50 epics, 5 kinds, `loose` view) and `closureOf`, registered
  as `field-epics-strip` in `worlds.ts`.
- `app/test/filters-page.test.tsx`: one existing assertion changed. The Epic box test said the page has no "Find an
  epic" text; the strip's own find box now says it, so the test asks for the box's own markers instead.

Tests:

- `app/test/epics-model.test.ts` (22), `app/test/epics-page.test.tsx` (21), `app/browser/fake-epics.spec.ts` (19).
- Red at runtime, with the new behavior stubbed to the old (`epicsOf` returning none, `litIds` empty, the fold not
  written, the funnel doing nothing, Add to Up next enabled): 25 of the unit tests (all of the strip, order, card,
  fold, lit and Up next ones), then, with `epicsOf` restored, 10 browser specs: the fold across a reload; a pick
  lights its cards; Clear and Close; the lit line counts; the drawer's steps and disabled Add to Up next; a task
  opened from the epic; a pick as the goal; a goal choice as a pick; the funnel and Epic box; a pick and a funnel
  apart. Not seen red in the browser (they pass on stubs that leave them alone; the unit tests cover their rules):
  one row and height, open work first, find, no strip for `loose`, strip placement, a pick keeping `scrollLeft`,
  the row moving to the picked card, a task of another epic opening, and See the run order and the graph.
- Stubs were restored from copies; `cmp` shows the files equal to what they were.

Checks run after the last edit: `pnpm exec vitest run` in `app` (849 passed), `pnpm typecheck`,
`cargo build --workspace --offline`, `cargo clippy --workspace --all-targets --offline -- -D warnings`,
`cargo fmt --all --check`, `pnpm --filter @collide/app build`, `pnpm build:packs --out <temp>/packs`, `pnpm check`,
and `pnpm test` (all packages; `app` Playwright 129 passed, `packs/plan` 955 passed).

`cargo test --workspace --offline --no-fail-fast -- --test-threads=2` (after the last edit): 4962 passed, 3 failed,
all in `collide-host` tests that this item does not touch: `base_env`
`a_hung_shell_falls_back_to_the_minimal_environment_and_is_killed` (failed in two runs of the whole binary, passes
run alone), `daemon_process` `a_daemon_started_from_a_terminal_survives_the_terminal_closing` and `write_guard`
`a_queued_write_whose_version_is_old_becomes_a_conflict_item_and_overwrites_nothing` (each passes run alone). They
look like the known load effects; they are named here and left as they are.

### Decisions for the owner

- **Collection `path` on the wire (built).** `Collection` gains an optional `path`; `collide/plan` sets it to
  `epic.md`; the drawer's Details shows it as File beside the task count. Alternative: no File row, no wire change,
  no `pnpm gen`. Recommend keeping it: it is one optional field, absent files are written as before, and the pack
  protocol doc in `IMPLEMENTATION_PLAN.md` should name it. Proposed plan text: "A collection may name its file
  (`path`); the drawer shows it under Details for engineers."
- **A click on a card picks and opens the epic in the drawer; a goal chosen in the goal bar picks without opening
  it.** The lit line then has Open the epic. Alternative: the goal bar opens the drawer too. Recommend as built, so
  a goal change does not cover the board.
- **The epic's steps are read separately (`useEpicWalk`, a walk toward that collection over the whole board)**
  rather than taken from the layout's walk. The drawer then shows the same steps in every layout and under any
  filter. Alternative: reuse `FieldData.walked`; one read less, but the drawer would change with the layout.
  Recommend as built.
- **The find box shows over 8 epics, the same number (`FIND_OVER`) as the Epic box.** Alternative: its own number.
- **The funnel and the Epic box share `filters.epics`.** A funnel click toggles the epic in it; the funnel stays
  pressed while the epic is there. No new state.
- **Lit cards use the primary tint with a border; the strip's picked card uses a primary ring.** Colours are
  from the existing tokens; the look was not checked against the mock. Alternative: a dedicated lit token.
- **One existing test was changed** (`filters-page.test.tsx`, see Built). Recommend accepting.
- **The pick is kept per page, not per board or view.** It is gone after a reload. Alternative: keep it with the
  folds. Recommend leaving it, as a pick is a look and not a setting.

### Not done, by design

- Add to Up next does nothing: it is off and says it comes in Stage 11, with groups and Autopilot.
- No collection names from a source other than `collide/plan` (Stage 5's GitHub milestones); the strip heads them
  "Epics".
- No new read of the board: counts come from `FieldData.corpus`.
- The pick and the strip find text are not saved with a view.
