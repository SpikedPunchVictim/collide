---
id: ts-0220-oc
kind: task
status: ready
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
