---
id: ts-0187-hb
kind: task
status: done
title: "App: the run order and graph layouts"
owners: []
links:
  - { type: blocked-by, to: ts-0186-ch }
---

## Goal

The Field lays a view out three ways: board, run order and graph (note 11). Run order shows the steps of the view's
closure, numbered. The graph shows the same steps as columns, with one line for each blocking link among the shown
tasks, and lights a task's lines while the pointer is on it.

What exists.
- `core/src/view.rs`: `Closure { goal, follow: DependsOn }` (one goal task), `LayoutKind { Board, RunOrder }` (no
  graph), `Layout { default, group_by, sort }`.
- `ViewReport.steps` (`core/src/owner/task.rs`): the closure's tasks in steps, as `collide tasks --view` prints
  them, and `cycle`.
- `ViewRef::Inline` lets the app ask `task.list` for a view it made.
- From `ts-0181-ft`: the Field and the view picker. From `ts-0186-ch`: `unblocks`.

What this adds.
- `LayoutKind::Graph` in core, the schema and the generated files, so a view file may say `layout: { default:
  graph }`.
- `collide/plan` reports each epic as a collection (revision 30; Stage 4c, the built-in markdown provider): it
  declares `collections: true` (`packs/plan/src/provider.ts`), lists each epic folder as a `Collection` (its id and
  its `epic.md` title), and puts the epic's id in each of its items' `collections`. An epic item is in none. Without
  it the Collide board has no collections, since membership is the `part-of` link (`Task::epic`), and neither the
  goal picker nor `ts-0190-tz`'s Epic box (`"<epic>" in collections`) would find a `collide/plan` epic. `watch`
  reports a changed collection as it reports a changed task.
- A layout switch on the Field (Board, Run order, Graph), starting at the view's `layout.default`, kept per board
  and view in webview storage.
- Run order: the goal bar ("Toward <goal>", then "N open tasks in M steps. Only open tasks are walked; finished
  ones are left out."), then the steps from `ViewReport.steps`, numbered, each with its cards.
- Graph: the same steps as columns, drawn in hand-written SVG, one line for each blocking link between two shown
  tasks. Hovering or focusing a task lights its lines and dims the rest. A cycle is named above the graph from
  `ViewReport.cycle`, and its tasks are drawn with no lines between them.
- The goal: a view with a closure uses its goal. For a view with none, the goal bar's picker follows the Owner's
  choice below.
- A card in either layout selects its task for the drawer, as on the board.

## Success criteria

- **Run order and graph** (note 11): for a view with a closure on the fixture board, both show the tasks and steps
  of `collide tasks --view`, and the graph draws one line for each blocking link among them.
- Hovering a task in the graph lights exactly its lines.
- A view with `layout: { default: graph }` opens in the graph.
- A cycle in the closure is named, and the layout still draws.

## Tests

Write these first.

- `core/tests/`: `layout.default: graph` reads; an unknown layout is still refused.
- `app/test/`: steps and lines from a `FakeOwner` world, a cycle, a link to a task not shown draws no line.
- `app/browser/fake-graph.spec.ts`: hover and focus light the lines.
- `app/browser/run-order.spec.ts` (bridge): the edge-mvp view (`fixtures/views/edge-mvp.yaml`, goal
  `plan:T-01-042`) over a plan board built by the test; steps and line count against `collide tasks --view edge-mvp
  --json`.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 6a: Field ("board, run-order and graph layouts"; "The graph lights a task's lines"). Success Criteria: Run
    order and graph. Tests: "run order and graph against the closure".
  - 0.10: Views (`closure`, `layout`). 0.11: the Field's layouts.
- Design: note 11. Mock functions: `runOrderHtml`, `graphHtml`, `lightGraph`, `closureSteps`, `goalBar`,
  `pickGoal`, `goalsOf`.
- **Owner's choice, recommendation taken (2026-11-24): the goal of a view with no closure.** The mock's goal bar picks
  an epic among the shown tasks ("Toward <epic>") for any view; the plan's closure takes one goal task, set in the view
  file. Recommend the mock's picker: core accepts a collection key as a closure goal (all its open tasks are goals), and
  the app asks `task.list` for the shown view with that closure added inline. Alternatives: a goal-task picker instead
  of an epic picker; or run order and graph only for views whose file sets a closure, with the switch disabled
  elsewhere.
- **Decision for the item, with a recommendation: drawing.** Recommend hand-written SVG with the steps as columns
  and straight or one-bend lines, as the mock draws. No graph library. Alternative: a layout library (a new
  dependency, an Owner's choice).
- Area: `core/src/view.rs`, `core/src/view_eval.rs` (if the collection goal is taken), `schemas/`, the generated
  files, `packs/plan/src/` (epics as collections) and its tests, `app/src/` (the layouts), `app/test/`,
  `app/browser/`.
- Dependencies: none.
- The test rule: no test touches the real home, real launchd, a real daemon, the keychain or the network. Each test
  that runs the `collide` binary clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to
  a temp root, as `host/tests/common/mod.rs` `TempRoot` does. A temp repository sets `GIT_CONFIG_GLOBAL` and
  `GIT_CONFIG_SYSTEM` to empty files. Keychain writes go through `MemorySecretStore` (`host/src/secret_store.rs`),
  never the login keychain. Browser tests run the installed Chrome with a temp profile
  (`app/browser/support/chrome.ts`), and the bridge's daemon on a temp root (`app/browser/support/world.ts`).
- Stage 6a item (revision 25 scope, split 2026-11-24). Items of Stage 6a run one at a time, in id order, because they
  share `app/`, `core`, `host` and the generated files. No builder runs `tauri build`.
- Nothing here runs a real `claude`.
- Owner decisions (2026-11-24): the owner took the recommendations at the split.

### Built

- Core, the layout: `LayoutKind` (`core/src/view.rs`) has `Graph` beside `Board` and `RunOrder`, written `graph`. The
  schemas and `packages/pack-sdk/src/generated/` came from `pnpm gen`.
- Core, the goal: a closure's `goal` may be a collection key. `Corpus::has_collection` (`core/src/corpus.rs`) asks the
  collections the sources list. `goals_of` and `walk` (`core/src/view_eval.rs`) make every open task of the collection a
  goal; a collection with no open task is an empty closure and no error. A goal that is neither a collection nor a task
  is unmatched as before (`goal_unmatched`, or `INVALID_PARAMS` from `task.list` for a view with no `where` or
  `include`). A key that is both is read as the collection.
- Pack: `collide/plan` (`packs/plan/src/scan.ts`, `provider.ts`, `changes.ts`, `watch.ts`) reports each epic as a
  collection, `{ id: "plan:<epic id>", title, groups: [] }`, declares the `collections` capability, and puts the id in
  `collections` of each task of that epic's folder. The `part-of` link stays as it was. `collectionsChanged` sends one
  `collections.changed` when the list or a task's membership changes.
- App, the reads: `loadField` (`app/src/field/load.ts`) takes an optional inline view. It asks `task.list` with
  `ViewRef::Inline` and `limit: 1`, since the report is what is wanted, and keeps the report as `FieldData.walked`. A
  walk the daemon refuses is words in `FieldData.walkProblem`. `FieldData.collections` is the first page's list.
  `FieldFeed` (`live.ts`) takes `walk` and asks it at each read, so the steps follow the board. `useField` holds the
  goal per board and view (`goal`, `chooseGoal`) and builds the inline view with `inlineView` (`walk.ts`): the shown
  view with `closure: { goal, follow: depends_on }` added.
- App, the layout: `app/src/field/layout.ts` (`layoutOf`, `writeLayout`, `layoutKey`) keeps the layout per board and
  view in the `FoldStore` of `folds.ts`. A view starts in its `layout.default`, else the board; a kept choice wins.
- App, the pure parts: `app/src/field/walk.ts` has `goalsOf`, `goalTitle`, `stepsOf`, `walkWords`, `graphOf` (the
  geometry of the mock: 236 by 68 cells, 200 by 52 tasks), `lightOf` and `inlineView`. `model.ts` `cardOf` was cut
  out of `lanesOf` so the run order draws the same card. `CardButton.tsx` was moved out of `FieldPage.tsx` to be shared.
- App, the page: `app/src/field/Layouts.tsx` has `LayoutSwitch` (Board, Run order, Graph), the goal bar, `RunOrder` and
  `Graph`. The lines are svg `path` elements with `data-from`, `data-to` and `data-hot`, coloured by classes. A task's
  place is a React `style` prop. No `<style>` element or style string is made. Hover or focus on a task lights its lines
  and dims the tasks not at their ends. A cycle is named above the graph and no line is drawn among its tasks. A link to
  a task that is not shown draws no line. A card in either layout selects its task for the drawer.
- Tests, and which I saw fail at runtime before the code (the new modules were first stubs that return the old,
  empty behaviour; the browser specs were run with the layout switch left out of the page):
  - `core/tests/views_profiles_locks.rs` (2): `a_layout_may_default_to_a_graph` and the unknown-layout refusal. Red:
    "unknown variant `graph`".
  - `core/tests/view_eval.rs` (6, "a collection as the goal"). Red: five failed with "`closure.goal` ... matches no
    task" or a wrong member list; the cycle test passed only with the goal in place.
  - `packs/plan/test/collections.test.ts` (9), four tests in `watch.test.ts` (`collections.changed`), and the changed
    expectations in `provider.test.ts` and `watch.test.ts`. Red: capability false, no `collections` key, no event.
  - `app/test/field-walk.test.ts` (27), `field-layout-store.test.ts` (7), `field-layouts.test.tsx` (26) and four feed
    tests in `field-live.test.ts`: 43 of the first three failed against the stubs, and the three feed tests that read a
    walk failed against the feed without `walk`.
  - `app/browser/fake-graph.spec.ts` (8): the first seven failed (90 s timeouts) without the layout switch; I stopped
    that run there. `app/browser/run-order.spec.ts` (3, real daemon, compared with `collide tasks --view ... --json`
    run with the world's environment): not run red. The first two need the switch; the third (the inline closure over
    the wire) tests what the daemon already did for a task goal, so it was never red.
- The fake worlds: `walkWorld` (`app/test/support/field-world.ts`, `#world=field-walk`) has four epics as collections,
  scripted walks by goal, a cycle, a finished blocker and a link to a key no task has. `window.collideFake.views()`
  lists the view each `task.list` asked for.

### Decisions for the owner

- A key that is both a collection and a task: read as the collection. `collide/plan` lists an epic as both under one key
  (`plan:<id>`), and the picker offers the epic, so the epic's open tasks are the goals. Alternative: read it as the
  task. Recommend the collection.
- The collection id is `plan:<epic id>`, as the Orchard adapter of Stage 4b writes it, so a collection id and the
  epic's task key are one string. Alternative: the bare epic id. Recommend as built.
- A task is in the epic's collection only when the epic has an `epic.md` in the scan; a folder with no epic file lists
  no collection and keeps its `part-of` link. `groups` is `[]`. Recommend as built.
- A collection with no open task is an empty closure, not an error, and the picker leaves it out. Alternative: refuse
  it. Recommend as built, since an epic that is all done is a normal board.
- The inline view is the shown view plus the closure, so its own `where` and `include` still apply and join the
  closure's members. Only the steps and the cycle are drawn from it, and the cards are the board's own tasks. The whole
  board is walked as a view named "All tasks" with only the closure. Alternative: walk the closure alone for any view.
  Recommend as built: the picker then narrows with the view, as the mock's does.
- The picker starts on the first collection that has open tasks, and sets it as soon as a run order or graph is shown;
  a goal that is no longer in the list is replaced the same way. Alternative: start empty and ask the person to pick.
  Recommend as built, as the mock does.
- Lines are straight when the two tasks are in one row and have one bend when they are not, entering the task level for
  12 px. The mock's curve is not drawn. Both stay in the brief's "straight or one-bend". Recommend as built.
- The two layouts reuse the board's drawer, selection and the live reads. The arrow keys move between cards on the
  board only; the graph's tasks are buttons and take the Tab key. Alternative: arrow keys in the graph. Recommend later.
- Proposed plan text for Stage 6a: "`collide/plan` reports each epic as a collection `plan:<id>`; a closure's goal may
  be a collection, and the Field walks a picked collection through an inline view." The plan was not edited.

### Not done, by design

- The epics strip, the filter bar and a pick (a picked epic as the goal of the run order and graph): `ts-0190-tz` and
  `ts-0216-ve`. They build on `FieldData.collections`, `goalsOf` and the inline view here.
- The goal picker over the plan's real epics is not run against a real daemon in the browser. The bridge runs the staged
  built-in pack (`app/src-tauri/resources/builtin-packs`), which this item does not rebuild, so the bridge world lists
  no epics. The pack's lists are tested in `packs/plan/test/`, the collection goal in `core/tests/view_eval.rs`, and the
  picker in the fake-owner specs. A test of the daemon with a provider that lists collections is not written, since the
  shared corpus provider fixture lists none.
- Arrow keys in the graph and the run order; the "Up next" layout of the mock (a later item).
