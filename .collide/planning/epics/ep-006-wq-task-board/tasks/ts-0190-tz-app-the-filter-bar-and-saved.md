---
id: ts-0190-tz
kind: task
status: ready
title: "App: the filter bar and saved views"
owners: []
links:
  - { type: blocked-by, to: ts-0189-fg }
---

## Goal

The Field gets its filter bar: chips in plain words that write the CEL, a search, "Showing X of Y", Clear, and Save
as a view. The expression shows read-only under Advanced. The bar folds to one line per board, and `/` unfolds it
into the search (notes 9, 10 and 38).

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
  - Stage: the epics of the shown tasks, when there is more than one (`"<epic>" in collections`).
  - Where it stands: the six categories (`category in [...]`).
  - Comes from: the sources, when there is more than one (`source in [...]`).
  - Labels: the labels the shown tasks carry, when any do (`"<label>" in labels`). See the Owner's choice.
  - Search by name or id (`title` or `key` contains the text, case-insensitive).
  Each chip shows its count. Chips in one group are or'd; groups are and'ed.
- The chips' CEL joined with the view's own `where` is sent as an inline view; the result is the board. Advanced
  shows the expression the chips wrote, read-only.
- "Showing X of Y" when a filter is on; Clear filters; Save as a view…: a name, the expression, and "The view is
  kept for you." It saves a personal view (`target: personal`) and selects it.
- On a board whose `.collide/` exists, a personal view's row in the view picker offers Share with team, which saves
  it to `.collide/views/` (`target: repo`) for the person to commit. See the Owner's choice.
- Fold leaves one line: the filters in force, "showing X of Y" and Clear. It stays folded for that board (webview
  storage). `/` unfolds it and puts the cursor in the search, except while a text box has the focus.
- "Describe what to show" and its "From your words" chip are Stage 6b's.

## Success criteria

- A saved view round-trips: saved from chips, listed in the picker, and `collide tasks --view <name> --json` lists
  the same tasks as the app showed before saving (Stage 6a test "view save round-trip").
- Each chip writes the CEL shown under Advanced, and the board equals `task.list` over that expression.
- Fold leaves one line with the filters in force, "showing X of Y" and Clear; it holds for that board across a
  restart of the page; `/` unfolds it into the search.
- On a shared board, Share with team on a personal view writes `.collide/views/<name>.yaml`, and the daemon
  reads it once committed and trusted.

## Tests

Write these first.

- `app/test/`: the CEL each chip set writes, quoting in search text, counts, which groups show.
- `app/browser/fake-filters.spec.ts` (Stage 6a test "the filter fold in a browser"): fold, the one line, `/`, Clear,
  the fold held per board.
- `app/browser/views.spec.ts` (bridge): save as a view, the picker, the CLI's members against the app's; Share with
  team on a shared temp repo writes the file.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 6a: Filter bar. Tests: "view save round-trip"; "the filter fold in a browser".
  - 0.11: Filter bar → view; Saved views. 0.10: Views (CEL, the normalized fields).
- Design: notes 9, 10 and 38. Mock functions: `filtersHtml`, `foldedFiltersHtml`, `applyFilters`, `celOf`,
  `saveView`, `saveViewModal`, `resetFilters`, `viewbarHtml`.
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
