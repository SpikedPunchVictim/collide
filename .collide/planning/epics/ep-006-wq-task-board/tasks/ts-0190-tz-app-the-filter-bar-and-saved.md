---
id: ts-0190-tz
kind: task
status: done
title: "App: the filter bar and saved views"
owners: []
links:
  - { type: blocked-by, to: ts-0189-fg }
---

## Goal

The Field gets its filter bar: chips in plain words that write the CEL, a search, "Showing X of Y", Clear, and Save
as a view. The expression shows read-only under Advanced. The bar folds under its own header per board, and `/`
unfolds it into the search (notes 9, 10, 38 and 62; revision 30).

What exists.
- `ViewRef::Inline` (`core/src/owner/task.rs`): `task.list` evaluates a view the app made, with the same checks
  and budgets as a file view.
- `task.save_view` (`TaskSaveViewParams { board, view, target: ViewTarget { Repo, Personal }, overwrite }`)
  writes a view file, checked as a view load checks it. Personal views live in the person's board folder
  (`UserScope::board_dir`). Repo views are read from the line's committed snapshot, and only while the line is
  trusted (`host/src/views.rs` `repo_candidates`).
- From `ts-0181-ft`: the Field and the view picker. From `ts-0187-hb`: the layouts.

What this adds.
- The bar over the shown view's tasks. Groups, each shown only when it has a choice to make:
  - Epic (note 62; revision 30): one box, not a chip each, shown when the shown tasks are in more than one epic
    (`"<epic>" in collections`). It says Any, the epic chosen, or "N epics", and opens a list of the view's epics,
    each with a tick and its count of tasks, with a find box over 8 epics and Clear. `ts-0216-ve`'s funnels set the
    same choice.
  - Where it stands: the six categories (`category in [...]`).
  - Comes from: the sources, when there is more than one (`source in [...]`).
  - Labels: the labels the shown tasks carry, when any do (`"<label>" in labels`). See the Owner's choice.
  - Search by name or id (`title` or `key` contains the text, case-insensitive).
  Each chip, and each epic in the Epic box, shows its count. Choices in one group are or'd; groups are and'ed.
- The chips' CEL joined with the view's own `where` is sent as an inline view; the result is the board. Advanced
  shows the expression the chips wrote, read-only.
- "Showing X of Y" when a filter is on; Clear filters; Save as a view…: a name, the expression, and "The view is
  kept for you." It saves a personal view (`target: personal`) and selects it.
- On a board whose `.collide/` exists, a personal view's row in the view picker offers Share with team, which saves
  it to `.collide/views/` (`target: repo`) for the person to commit. See the Owner's choice.
- The bar sits under a header with a chevron, as the drawer's parts do (note 38; revision 30, in place of a Fold
  button). A click on it folds the bar to the header, which then shows the filters in force, "showing X of Y" and
  Clear. It stays folded for that board (webview storage). `/` unfolds it and puts the cursor in the search, except
  while a text box has the focus.
- "Describe what to show" and its "From your words" chip are Stage 6b's.

## Success criteria

- A saved view round-trips: saved from chips, listed in the picker, and `collide tasks --view <name> --json` lists
  the same tasks as the app showed before saving (Stage 6a test "view save round-trip").
- Each chip writes the CEL shown under Advanced, and the board equals `task.list` over that expression.
- The folded header shows the filters in force, "showing X of Y" and Clear; it holds for that board across a
  restart of the page; `/` unfolds it into the search.
- The Epic box: on a board of 50 epics, the list finds by name, a tick adds an epic, the box names the choice or
  counts it, and Clear empties it.
- On a shared board, Share with team on a personal view writes `.collide/views/<name>.yaml`, and the daemon
  reads it once committed and trusted.

## Tests

Write these first.

- `app/test/`: the CEL each chip set writes, quoting in search text, counts, which groups show.
- `app/browser/fake-filters.spec.ts` (Stage 6a test "the filter fold in a browser"): fold, the folded header, `/`,
  Clear, the fold held per board; the Epic box's list, find, ticks and Clear.
- `app/browser/views.spec.ts` (bridge): save as a view, the picker, the CLI's members against the app's; Share with
  team on a shared temp repo writes the file.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 6a: Filter bar. Tests: "view save round-trip"; "the filter fold in a browser".
  - 0.11: Filter bar → view; Saved views. 0.10: Views (CEL, the normalized fields).
- Design: notes 9, 10, 38 and 62. Mock functions: `filtersHtml`, `filterWords`, `epicBoxHtml`, `applyFilters`,
  `celOf`, `saveView`, `saveViewModal`, `resetFilters`, `viewbarHtml`.
- **Owner's choice, recommendation taken (2026-11-24): a Labels group.** The mock's bar has no labels, and 0.11 lists
  the chips "by stage, where a task stands, where it comes from". S6's scenario 2 asks for "the label perf"
  (`ts-0196-uw`). Recommend a Labels group, shown only when the shown tasks carry labels. Alternative: no group;
  scenario 2 is then done by typing CEL, which the plan says is never the only way in. No built-in source gives labels
  today (`collide/plan` and the Orchard fixture's provider set none), so the tests use a fixture pack whose tasks carry
  them.
- **Owner's choice, recommendation taken (2026-11-24): sharing a view on a board already shared.** The mock's Share with
  team is board-wide (it writes `board.yaml`, the saved views and the settings, `ts-0194-qp`) and shows only until the
  board is shared. Recommend a Share with team on each personal view's row once `.collide/` exists. Alternative: views
  saved after the first share stay personal, and the person copies the file by hand.
- **Decision for the item, with a recommendation: after sharing.** Recommend keeping the personal view until the repo
  view is read, then listing the repo view and hiding the personal one of the same name. Alternative: delete the
  personal file at once; the view vanishes until the commit and trust.
- Area: `app/src/` (a `filters/` folder), `app/src/owner/methods.ts`, `app/test/`, `app/browser/`.
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

- Wire (the gap, settled first). `core/view_eval.rs` and `view_builtin.rs` confirm it: a built-in view (`ready`,
  `bottlenecks`) has no `where` and takes its members from a rule, and `include` and `closure` add members the `where`
  never sees. So the chips' CEL joined to a `where` is wrong for those views. `task.list` takes a `filter`
  (`core/src/owner/task.rs` `TaskListParams.filter`): a CEL expression, checked, compiled and budgeted as a view's
  `where` is, applied to the members of the view asked for, in the view's order. With no view it lists the tasks the
  filter matches. A refusal is `INVALID_PARAMS` with `filter: ...` and the reason. The report is the view's, with
  `members` and `steps` cut to the filtered tasks and the filter's own per-task errors added; `cycle` stays the view's
  (`host/src/daemon.rs` `narrowed`, called from `task_list`). `task.views` says `shared` (`TaskViewsResult.shared`: the
  board's line has a Collide folder). `pnpm gen` was run; the generated files changed are `TaskListParams` and
  `TaskViewsResult`, in `schemas/` and `packages/pack-sdk/src/generated/`.
- The model, `app/src/filters/model.ts`: `celOf` (the chips as CEL, one term to a line, groups and-ed, values or-ed, a
  view's own `where` first when given; search text is a regex-escaped, JSON-quoted `matches "(?i)..."` on `title` and
  `key`), `groupsOf` (the six categories with counts, epics, sources, labels; each group only when there is a choice
  to make), `epicWords`, `summaryOf`, `toggle`, `cannotSave` and `saveOf` (Save as a view). `fold.ts` keeps the bar's
  fold per board in a `FoldStore`. `keys.ts` (`isSearchKey`) is `/` off while typing; `frame/actions.ts` has the new
  named action `filter-search`.
- The bar, `app/src/filters/FilterBar.tsx` and `EpicBox.tsx`: header with the drawer's chevron (`Chevron`, now exported
  from `drawer/DrawerView.tsx`), folded line "Filters · X · showing N of M" or "No filters · N tasks" with Clear
  filters; chips for Where it stands, Comes from and Labels with counts; the Epic box (a popover: tick list with
  counts, find box over 8, "N chosen", Clear; it says Any, the epic's title, or "N epics"); the search box; Showing X
  of Y; Clear filters; Save as a view… (a popover: name, "The view is kept for you.", the daemon's refusal as words);
  Advanced with the expression; the words of a refused filter.
- The read: `field/load.ts` `loadField(..., filter)` reads the board, the view without the filter (for the counts,
  `FieldData.inView`) and the view with it; the walk carries the filter. A refused filter is `FieldData.filterProblem`
  and no tasks. `field/live.ts` `FeedOptions.filter`. `field/useField.ts` holds `filters`/`setFilters` (the view's,
  forgotten with it; this is the one place `ts-0216-ve`'s strip sets `epics`), the 250 ms search wait, `shared`,
  `saveView`, `shareView`, `reloadViews`. `field/ViewPicker.tsx` `rowsOf` and Share with team on a personal view's row.
- Test support: `test/support/cel-lite.ts` (the terms the bar writes; an unknown term is an error), `FieldWorld`
  (`filter`, `shared`, `refuseFilter`, `task.save_view`), the worlds `field-filters` and `field-epics-50`;
  `browser/support/octo-board.ts` (the fixture repo of `field.spec.ts`, moved, with `labels` and `trust`).
- Tests, Rust (`host/tests/daemon_corpus.rs`):
  `a_filter_narrows_the_members_of_every_kind_of_view_and_keeps_the_views_order`,
  `a_filter_alone_lists_the_tasks_it_matches_and_pages_with_a_view`,
  `a_filter_is_checked_as_a_views_where_is_and_a_bad_one_is_refused_with_the_reason`,
  `task_views_says_whether_the_line_has_a_collide_folder_to_share_a_view_in`.
- Tests, vitest (`app/test/`): `filters-model.test.ts` (30: the CEL per chip set, quoting of `"` and `\`, patterns as
  text, counts, which groups show, Save as a view and why it is off, the picker rows, the fold),
  `field-filter.test.ts` (7: what `loadField` reads), `filters-page.test.tsx` (22: the bar, folded bar, Save, Epic box,
  Share with team), `filters-keys.test.ts` (6), and new cases in `actions.test.ts` and `field-live.test.ts`.
- Tests, browser: `fake-filters.spec.ts` (11: chips, groups, search and quoting, `/`, fold held across a reload, the
  choices going with the view, the Epic box over 50 epics, one epic by its title, Save as a view, Save off, Share with
  team) and the bridge `views.spec.ts` (3: labels from a fixture pack; the board equals `task.list` over the bar's
  expression under `ready`, an `include`/`exclude` view, an `include`-only view and the whole board; Save as a view
  equals `collide tasks --view checks --json`, then Share with team writes `.collide/views/checks.yaml`, and after the
  commit and trust the picker has one row, in the repo).
- Red at runtime. Rust: with `filter` accepted and ignored, the first three Rust tests failed by assertion (the whole
  view came back); with `shared` fixed at false the fourth failed at `assert!(after.shared)`. vitest: with the new
  code stubbed (`celOf` returning null, `readBarFolded` false, `loadField` and `FieldFeed` not passing the filter,
  `rowsOf` returning its input) 25 tests failed by assertion in `filters-model`, `field-filter`, `field-live` and
  `filters-page`; with `/` not handled (`isSearchKey` false) and the `filter-search` action absent, `filters-keys` and
  `actions` failed. Browser: with `isSearchKey` false, the fold not written, the find box never drawn and `rowsOf`
  not merging, four `fake-filters` tests failed at runtime (an expect that timed out); the edits were then put
  back. The bridge spec was seen red twice on the way, for a real reason each time (the repo copy of a shared view is
  read only when committed and trusted, and the Done column is folded).
- Checks, each run after the last edit to the code: `cargo build --workspace --offline`, `cargo clippy --workspace
  --all-targets --offline -- -D warnings` and `cargo fmt --all --check` pass; `cargo test --workspace --offline --
  --test-threads=2` passes (4964 passed, 0 failed, exit 0); `pnpm test` exits 0 (vitest: 806 in `app`, 99 in
  `pack-sdk`, 519 in `arch-plan-tests`, 954 in `packs/plan`, 2 in `claude-contract`; the Playwright suite of `app`,
  bridge and fake-owner specs, 110 passed); `pnpm typecheck`, `pnpm --filter @collide/app build`,
  `pnpm build:packs --out <temp>/packs` and `pnpm check` exit 0. The denylist check over every file written found no
  hit.

### Decisions for the owner

- **A `filter` on `task.list`, not the item's inline join.** The item sends the chips' CEL joined with the view's
  `where` as an inline view. That is wrong for built-in views (no `where`) and for `include` and `closure` (their
  members never meet the `where`). The `filter` narrows the members of the view asked for, whatever made them members,
  and keeps its order, so "the board equals `task.list` over the expression" holds for every view. Cost: one optional
  param on the wire. Alternative: the inline join, which is right only for views that are just a `where`. Recommend as
  built.
- **Save as a view saves the base view's `where` with the chips and-ed in, and copies `exclude` and `layout`. It is off
  for a built-in view and for a view with `include` or `closure`, with words that say why.** The brief's advice was to
  copy `include` and `closure` too. A file that did would list more than the board showed: the included and walked
  members never meet the `where` the chips were put in. A built-in rule no file can say. `workflow`, `profile` and
  `queue` are not copied: they say how runs launch and what feeds the Queue, not what the view lists. Alternative: a
  view-level `filter` in 0.10 that applies after `include` and `closure`, so Save works over every view. Recommend as
  built now, and that alternative as a later change to the plan.
  **Owner's choice (2026-11-25): as built now; the view-level `filter` is a later change to the plan, proposed at
  the next revision.**
- **After sharing, the picker merges a personal view and the repo view of the same name when they say the same.** The
  daemon reads a repo view only from the commit and only while the line is trusted, and it keeps using the personal
  view of that name (the repo copy is listed with an error that says so). A commit under `.collide/` also changes what
  the line's trust covers, so the bridge spec trusts again after it. So "hide the personal one" is done in the picker
  (`rowsOf`): one row, labelled "in this repo", with the counts of the view the daemon uses. When the two differ both
  rows stay. The personal file is not deleted: there is no wire method for that. The picker reads the views again each
  time it opens. Alternative: a "Remove my copy" action once the repo copy is read, which needs a delete method.
  Recommend as built.
- **`task.views` says `shared`.** The app needs to know whether `.collide/` exists to offer Share with team. The
  alternative is to try the save and show the refusal; that offers a button that fails on an unshared board. Recommend
  as built.
- **Counts come from three reads, not one.** With a filter on a view the Field reads the board, the view and the view
  with the filter, each paged. The groups and "Showing X of Y" are over the second, so a count never shrinks as the
  chips are clicked. With no filter the view is read once. Alternative: the daemon counts each chip in one call; that
  is a larger wire for a bar that has few chips. `ts-0216-ve` can read `FieldData.inView` for its strip. Recommend as
  built.
- **Search is case-insensitive through `matches` with `(?i)` and the text escaped, not `lowerAscii`.** The strings
  extension is not on in the CEL environment. The text is regex-escaped and then quoted as a CEL string, so a quote, a
  backslash or a bracket is text. Alternative: turn the strings extension on in core, a change to what every view's
  `where` may say. Recommend as built. A text longer than the CEL cap is refused by the daemon; the bar says so and
  shows no task.
- **The choices belong to the view and are not kept across a reload.** They are in `useField` (`filters`,
  `setFilters`), reset when the view or board changes, which is where `ts-0216-ve`'s strip sets `epics`. The bar's fold
  is kept for the board. Alternative: keep the choices per view in storage; the item asks only for the fold. Recommend
  as built.
- **A change to the choices restarts the feed, not the page.** The old board stays on screen until the new read lands,
  and nothing flashes for a filter change (the first read of a feed has nothing to compare). A search waits 250 ms after
  the last key; a chip reads at once. Run order and graph get the filtered steps from the same filter, so they show the
  tasks the board shows; the cycle note stays the view's.
- **Save and the Epic box are popovers.** `components/ui` has no dialog, and adding one is a new dependency. The diff
  window's `<dialog>` is modal, which a search-while-looking box is not. Recommend as built.
- **A chip's name says its count in words** ("Ready, 3 tasks"). A chip reading "Done 2" would share its name with the
  folded column's button, and a test, or a person using a screen reader, could not tell them apart.
- **Advanced shows the expression of the choices alone.** The view's own `where` is not in it, since the daemon joins
  the two. Alternative: show the view's `where` first, as the saved view would have it. Recommend as built, since the
  expression shown is exactly what is sent.
- **A refused filter shows no task and says why.** Chips cannot write a bad expression, so this is a text over the
  cap or a budget. Showing the view whole would not be what the bar says.

### Not done, by design

- "Describe what to show" and its "From your words" chip: Stage 6b.
- The epics strip and the funnels that set the Epic choice: `ts-0216-ve`. They read `FieldData.inView` and set
  `FieldState.filters.epics`.
- Deleting the personal copy of a shared view, and a view-level `filter` so Save works over every view: not planned
  yet; both are proposed above.
- The board-wide Share with team (`board.yaml`, saved views and settings together): `ts-0194-qp`.
