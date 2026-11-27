---
id: ts-0220-oc
kind: task
status: done
title: "Saved views: replace a view, and show the filters on top of one"
owners: []
links:
  - { type: blocked-by, to: ts-0219-qf }
---

## Goal

A person can save over a view of their own from the page, and can tell when the board they see is a saved view with
filters on top of it rather than the view itself.

What exists.
- `app/src/filters/FilterBar.tsx` `SaveView`: a name and "Save view". `app/src/field/useField.ts` `saveView` calls
  `task.save_view` with `target: 'personal'` and never sends `overwrite`. The daemon refuses a name a view already
  has unless `overwrite` is set (`core/src/owner/task.rs` `TaskSaveViewParams`; `host/src/views.rs` `save_view`,
  `ts-0217-dy`). So the page cannot replace a view: the owner calls it a bug.
- `app/src/filters/model.ts` `celOf` joins the selected view's `where` and every chip with AND, and `saveOf` saves
  that. The heading shows the view's name only, so chips over a saved view look like the view.
- `ts-0197-uj`, the owner's run, step 12: "Slow spots" was saved with the epic only; the chips "Ready" and "perf" were
  added after. The board showed five cards under the heading "Slow spots" while the file held one term. The save box
  then refused "Slow spots".

What this adds.
- **Replace.** When the name typed is a view of the person's own, the save box says so and its button reads
  "Replace <name>". Clicking it saves with `overwrite: true`. A name a built-in or a pack's view holds is refused as
  now. Share to the repo is unchanged.
- **Filters on top.** While a saved view has chips on top, the page says so where the view's name shows, with the
  count of filters (for example "Slow spots + 2 filters"), and the filter bar's clear action takes the board back to
  the view alone.

## Success criteria

- Save "Slow spots" with the epic only, add two chips, type "Slow spots": the button reads "Replace Slow spots".
  Click it: the file holds the three terms, the chips are folded into the view, and the heading reads "Slow spots".
- A name that differs only in case from a view of the person's own is refused as `ts-0217-dy` refuses it.
- With chips over a saved view the heading shows the view's name and the filter count; with none it shows the name.

## Tests

- `app/test/` (vitest): the save box's state for a free name, a name of the person's own view, a built-in's name; the
  heading text with and without chips.
- `app/browser/` (bridge): save, add chips, replace; the file read back by the daemon holds the three terms. Extend
  `s6.spec.ts` or the filter spec.
- `host/tests/`: nothing new expected; `overwrite` is covered by `ts-0217-dy`'s tests.

## Notes

- IMPLEMENTATION_PLAN.md: Stage 6a (Filter bar, Save as View; views); revision 33.
- **An item decision: what "Replace" saves.** Recommend `saveOf` as now: the view's `where` and the chips, so the
  board after the replace is the board before it. The view's other parts (layout, group by) are kept.
- **An item decision: the words.** Recommend "Replace <name>" and "<name> + N filters"; the owner sees them at the
  next look. Keep the mock's terms (`docs/mockups/board.html`).
- The test rule: no test touches the real home, real launchd, a real daemon, the keychain or the network. Each test
  that runs the `collide` binary clears the environment and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root
  (`host/tests/common/mod.rs` `TempRoot`).
- Nothing here runs a real `claude`. New dependencies: none expected.
- Stage 6a item (revision 33, 2026-11-27), from `ts-0197-uj`'s owner run. It runs after `ts-0219-qf`, before
  `ts-0221-pp`.

### Built

No Rust change and no wire change: the daemon already takes `overwrite` (`ts-0217-dy`), and `pnpm gen` was not needed.

- `app/src/filters/model.ts`: `saveBoxOf(views, typed)` says what the save box does with a name: `free`, `replace`
  (the name is exactly a view whose `source` is `personal`), or `refused` (the name differs from such a view only in
  case, with plain words that name the view). `filterCount(filters)` counts the chips on, and one for a search with
  text in it. `headingOf(view, filters)` is the name, or "<name> + N filter(s)".
- `app/src/filters/FilterBar.tsx`: `FilterBarProps` takes `views`. `SaveView` reads `saveBoxOf`: on `replace` the
  button reads "Replace <name>", a line says the view is already the person's, and the click calls `onSave` with
  `overwrite` true; on `refused` the words show and the button is off. `onSave` takes `overwrite`.
- `app/src/field/useField.ts`: `saveView(view, overwrite)` sends `overwrite: true` only when asked. After a save it
  clears the filters of the saved view, since the view now holds them. Without this, a replace of the open view would
  leave its chips on top of a view that already says them.
- `app/src/field/FieldPage.tsx`: passes `views` to the bar; the heading is `headingOf(view, filters)` over a named
  view, and "All tasks" over the whole board. The bar's Clear filters already took the board back to the view alone.

Tests, with the ones seen red at runtime against the code before the change marked (R):
- `app/test/filters-model.test.ts`: "the save box" (free name; replace, with white space round it (R); refused for a
  case-only difference (R); a built-in's, a pack's and a repo view's name are the daemon's) and "the filters on top
  of a view" (count; "1 filter" and "N filters" (R); name alone with no chips).
- `app/test/filters-page.test.tsx`: "the heading" (name + count (R); name alone, and All tasks over the whole board).
- `app/browser/fake-filters.spec.ts` (fake owner, all three R against the old page): the heading counts the chips
  and Clear filters brings it back; a name the person's own view has is a replace; a case-only name is refused.
- `app/browser/s6.spec.ts` (bridge, real daemon), "S6 scenario 2, replaced" (R: the old page fails at the heading
  "Slow spots + 2 filters"): save with the epic only, add Ready and perf, a built-in's name is refused by the daemon,
  Replace; the daemon's view holds the epic, `category in ["ready"]` and `"perf" in labels`, is one view, and detects.

### Decisions for the owner

- **What Replace saves when the name is not the open view.** `saveOf` as now: the open view's `where`, `exclude` and
  `layout`, with the chips. If the person types the name of another view of their own, that view's layout is not kept.
  Alternative: keep the replaced view's layout. Recommend as built: the board after the replace is the board before it.
- **What counts as a filter.** Each chip on is one, and a search with text is one. Alternative: chips only. Recommend
  as built: a search narrows the board, so it should show.
- **Which headings say it.** Any named view, built-ins included. The whole board stays "All tasks". Alternative: only a
  view of the person's own. Recommend as built: a chip over "ready" misleads as much as one over "Slow spots".
- **Case-only clash.** Refused in the save box, with the button off, in plain words that name the view. The daemon
  still refuses it if asked. Alternative: leave it to the daemon, whose words give a file path.
- **Filters clear after any save**, as the saved view takes them. A new name already did so by changing the view.
- **Not checked here:** a name that makes the same file name without differing in case ("Slow-spots") is the daemon's to
  refuse, with its words as before.
- Review (2026-11-27): the recommendations above are taken. Replacing a different view of the person's own with
  what the board shows now (its layout included) is what "Replace" means here: the board after it is the board
  before it. A name that clashes by punctuation rather than case (`Slow-spots` beside `Slow spots`, one file) is free
  in the save box and refused by the daemon in its words.

### Not done, by design

- The picker row, the view menu's trigger and other places that show the view's name do not say the count; only the
  heading does. A later item if the owner wants it.
- A saved view that names tasks or walks to a goal still cannot be saved over (`cannotSave`), as before.
- Deleting a view of the person's own is not built; no item yet.
