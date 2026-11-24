---
id: ts-0181-ft
kind: task
status: done
title: "App: the Field board, columns from declared statuses"
owners: []
links:
  - { type: blocked-by, to: ts-0180-ad }
---

## Goal

The Field shows the board's tasks over the corpus, in the board layout, with columns from the sources' declared
statuses. Cards carry the chips of note 5. The view picker lists the board's views, and a card opens the drawer's
place (`ts-0184-hv` fills it). This is the first time the owner sees real tasks in the app.

What exists.
- `app/src/components/field/FieldSurface.tsx`: Stage 2's columns, fixed to a `CATEGORIES` list, over invented data
  from `app/src/components/placeholders.ts`. Nothing reads `task.list`.
- `task.list` (`core/src/owner/task.rs` `TaskListParams`, `TaskListResult`) with a view by name, and the view report
  (`ViewReport`: `members`, `cycle`, `unknown_blockers`, `unmatched_include`, `errors` and the budgets). `task.views`
  lists the board's views; `core/src/view_builtin.rs` holds the built-in `ready`.
- A view's `layout` (`core/src/view.rs` `Layout`: `default`, `group_by`, `sort`).
- From `ts-0180-ad`: each source's statuses, categories, fields and id patterns, and each divergent copy's state,
  checkout and run, on `task.list`.
- From `ts-0179-ke`: the frame, the Field tab and the view picker's slot.

What this adds.
- The Field over `task.list` for the selected board and view. Grouping is by status when the shown tasks come from
  one source, and by category otherwise (0.10, revision 23); a view's `layout.groupBy` overrides it. Status columns
  come in declared order. Category columns are the six of 0.10, in its order, and a column with no task still shows.
- A column's header is the status name with "counts as <category>" beneath, or the category alone in category
  columns (note 3). Done and Dropped start folded, showing their count. The fold is kept per board and view, in the
  webview's storage (note 4).
- A card (note 5): its title, then its short id (from the source's `short_id_pattern`) and its stage (its epic
  collection). Chips show only when they say something: its status in category columns; Waits on N (open blockers);
  a copy that differs, named by its state and where it is ("Review in Run R-121", or the checkout's name; note 6);
  "Same id in N copies" for a collision, never shown as a difference (note 7). A card being worked on has the lit
  border. `Unblocks N` is `ts-0186-ch`'s.
- The view picker in the trail: the built-in views first, then the board's, each named. A view's report shows above
  the board as a short line when it has something to say: a cycle, unknown blockers, unmatched includes, a view
  error, a budget stop. The words come from the mock.
- A click on a card, or Enter on a focused card, selects it. Selection opens the drawer's place, which says
  "the drawer comes next" until `ts-0184-hv`. Arrow keys move between cards.
- `CATEGORIES` and Stage 2's Field data go.

## Success criteria

- **Statuses as data**: a fixture source with eight statuses, two of them mapped to `review`, shows eight columns
  in its order. Renaming its statuses changes the columns with no code change. A view mixing it with the plan source
  shows the six categories, each card with its own status name.
- The MVP view in the app lists the same tasks as `collide tasks --view edge-mvp` on the same board
  (`fixtures/views/edge-mvp.yaml`; its `github:` include is reported as unmatched by both, and no `github` pack is
  needed).
- Done and Dropped start folded with their counts; unfolding one holds after switching views and back.
- A divergent task shows its chip with the other copy's state and where it is. A collided id shows "Same id in 2
  copies" and no divergence chip.
- A card with no blockers, no differing copy and one source shows no chip.

## Tests

Write these first.

- `app/test/`: grouping by status and by category over `FakeOwner` worlds (one source; the eight-status source;
  mixed); header words; which chips show; the fold key per board and view.
- `app/browser/fake-field.spec.ts`: fold and unfold, a card selected by click and by Enter, arrow keys.
- `app/browser/field.spec.ts` (bridge, real daemon): a temp repo with a fixture pack of eight statuses and plan
  tasks; the columns; a rename in the fixture's declaration restarts the source and changes the columns; the MVP view
  against `collide tasks --view edge-mvp --json` run by the test with the world's environment.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 6a: Field (views; columns from declared statuses; header; fold; the card and its chips). Success Criteria:
    Statuses as data; the MVP view. Tests: "Columns from a fixture source's declared statuses, and from categories
    for a mixed view."
  - 0.10: statuses as data (revision 23), categories and their order. 0.11: the Field; views.
- Design: notes 3, 4, 5, 6 and 7. Mock functions: `fieldHtml`, `groupMode`, `columnsOf`, `isShut`, `boardHtml`,
  `cardHtml`, `divergence`, `catOf`, `viewbarHtml`.
- Out of scope: live updates (`ts-0182-ad`); the drawer (`ts-0184-hv`); `Unblocks N` and `bottlenecks`
  (`ts-0186-ch`); run order and graph (`ts-0187-hb`); the filter bar (`ts-0190-tz`).
- Carried: `ts-0135-cx` (the `ready` cycle and unknown blockers, "their display in the app: Stage 6"): the view's
  report line above.
- **Decision for the item, with a recommendation: where folds are kept.** Recommend the webview's `localStorage`
  under a key of board id and view name: it is the person's, per machine, and needs no daemon method. Alternative: a
  personal setting through `config.set`; it follows the person across windows but adds keys to Settings.
- **Decision for the item, with a recommendation: the stage on a card.** Recommend the task's epic collection, named
  as the source gives it, and nothing when it has none. Alternative: the first collection of any kind.
- Area: `app/src/components/field/` (or a new `app/src/field/`), `app/src/owner/methods.ts`, `app/test/`,
  `app/browser/`, a fixture pack under `fixtures/packs/` for the eight statuses if none fits.
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
- Carry (revision 29, Stage 6a's Named actions): make one list of the app's named actions, each with its key, and bind
  keys from it. Move `ts-0179-ke`'s keys 1 to 5 into it. Later items add their keys to the list. The palette (9d) and
  the in-app agent's tools (6b) use these names.
- Owner decisions (2026-11-24): the owner took the recommendations at the split.

### Built

- `app/src/field/`: `types.ts` (the wire types the Field uses), `load.ts` (`task.views` and `task.list`, every page;
  a named view is read next to the whole board, so open blockers can be counted), `model.ts` (grouping, headers, chips,
  stage, the view report's words), `folds.ts` (per board and view, in `localStorage`, tolerant of storage that is
  missing or broken), `navigation.ts` (arrow keys), `useField.ts`, `FieldPage.tsx` and `ViewPicker.tsx`. `App` holds
  one `useField`; the picker sits in the trail and the page fills the Field tab.
- Columns: the declared statuses of the shown tasks' source when there is one, the six categories otherwise, and a
  view's `layout.groupBy` over both. A status that is not declared is added after the declared ones. Header: the
  status name with "counts as <category>" under it, or the category alone. Done and Dropped start folded with their
  counts. A card: title, then short id and stage, with chips only for a status (category columns), "Waits on N", a
  differing copy, and a collided id. The lit border is on an active card. Click or Enter selects and opens the
  drawer's place ("The drawer comes next."); arrow keys move between cards.
- The view report line above the board, in the words of the CLI's own lines.
- Carry, named actions: `app/src/frame/actions.ts` is the one list. Keys 1 to 5 and the card keys are bound from it;
  `tabForKey` is gone from `tabs.ts`.
- Carry, category colours: `--c-backlog` to `--c-dropped` and the mock's `--working`, `--waiting`, `--blocked` and
  `--done` in `index.css` (both themes), registered for utilities. `TracksPreview.tsx` uses them.
- Stage 2's `CATEGORIES` and Field data were already gone (`ts-0179-ke`). The model has its own `CATEGORIES` with the
  mock's words.
- Tests. `app/test/field-model.test.ts` (grouping over the one-source, eight-status and mixed worlds; header words;
  chips; stage and short id; the view report; the fold key), `field-navigation.test.ts`, `actions.test.ts`,
  `field-page.test.tsx`, `category-colours.test.tsx`, and `frame.test.tsx` changed. `app/browser/fake-field.spec.ts`
  (folds, kept across a reload and its own for each view, click, Enter, arrow keys, eight columns) and
  `app/browser/field.spec.ts` (bridge, real daemon: the columns, a rename that restarts the source, and the
  `edge-mvp` view against `collide tasks --view edge-mvp --json`, run by the test with the world's environment).
  `fake-shell.spec.ts` changed: the Field is no longer a placeholder, and a board switch now causes one Field read.
  The red runs: the model, navigation and action tests were red against stubs (48 and 16 failures); the
  two specs were red with the Field page swapped for a placeholder. `field-page` and `category-colours` were written
  after the code and were not seen red.
- `worlds.ts`: `field-one-source`, `field-eight` and `field-mixed`; `two-boards` is now a `FieldWorld` with no tasks.
  `support/world.ts`: `Plan.path` puts folders (where `node` is) on the daemon shell's `PATH`.

### Decisions for the owner

- The plan has no `backlog` status: its statuses are `ready`, `active`, `review`, `done` and `dropped`, so a board of
  the plan alone has no Backlog column. A mixed board shows all six categories. Recommendation: keep; it is what the
  source declares.
- The plan lists each epic as a task, so an epic is a card on the Field. Recommendation: leave epics out of the board
  by kind in a later item (the view's `where` can do it today). Alternative: show them as a header over their tasks.
- A card's stage is the target of its `part-of` link, because the plan reports the epic as a link and not as a
  collection. Recommendation: keep, and show the collection when a source gives one.
- The Field reads on opening, on a board change and on a view change, and not on events. Recommendation: refresh on
  the corpus events in a later item; the rename test reloads the page for now.
- A status column is headed by the declared name, capitalised (`In_review`). Recommendation: a `label` on a declared
  status, as fields have one, when a pack wants nicer words.
- Open blockers need the whole board, so a view is read with two `task.list` calls. Alternative: the view report
  carries each member's open-blocker count, which is a wire change. Recommendation: keep until a board is large.
- A view is forgotten when the board changes, and the whole board is the default. Recommendation: keep.

### Not done, by design

- The drawer's content (`ts-0184-hv`); the selection only opens its place.
- `layout.sort` and `layout.default: run-order`: the board lays cards out in the order `task.list` gives them.
- Live refresh on events, drag to change a status, filters and search.
- No Rust change, so `cargo test --workspace` was not run.
