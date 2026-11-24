---
id: ts-0184-hv
kind: task
status: ready
title: "App: the task drawer, read-only"
owners: []
links:
  - { type: blocked-by, to: ts-0183-ee }
---

## Goal

A selected task opens the drawer beside the board, read-only, with its parts in 0.11's order: the description, Waits
on and Unblocks, Part of, Fields from the source, each copy of the project, and Details for engineers. Each part
folds, and the fold holds for the next task. The person sizes the drawer. Changes is `ts-0189-fg`'s.

What exists.
- `task.get` (`core/src/owner/task.rs` `TaskGetResult`): the task, `copies` (`TaskCopy`: checkout, path, state,
  version), `collisions`, `runs`, `latest_run`, `links` (one hop both ways), `ops` and `link_types`.
- From `ts-0180-ad`: link titles, the views that hold the task, field labels, and each source's declarations.
- From `ts-0181-ft`: the Field, its selection, and the drawer's place.
- From `ts-0183-ee`: the `Markdown` component, the link window, and the CSP.
- Stage 2's `app/src/components/` detail panel over invented data (`placeholders-detail.ts`).

What this adds.
- The drawer over `task.get` for the selected task, refetched on the Field's live event for it.
- The header: the status chip, its category in words, the id, and Close. Then the title, and "Read only. Edit the
  file, or ask an agent; the board follows it." for a plan task, or "Change it in <source>." for another.
- The parts, in 0.11's order, each left out when the source has nothing for it:
  - Description: the body through `Markdown`.
  - Waits on and Unblocks: the direct links of the source's blocking link type, each with state and title. "Nothing.
    It can start." and "Nothing waits on it." when empty. A link opens that task in the drawer. A source with no
    blocking link type shows neither part (a scratch task).
  - Part of: its epic and other collections, and a chip for each view that holds it.
  - Fields from the source (note 18): each declared field by its `label`, or its id in words; "not set" when empty.
  - In each copy of the project: one row per copy with its state, "shown on the board", "differs from main" or "same
    as main"; a collision row reads "Same id, another task" with the other title and path (notes 6 and 7). Shown for
    a source whose scope is the ref.
  - Details for engineers, folded: source and pack, the file or what it reads, how copies work, board and line, and
    the task's raw fields.
- Folds (note 39): each part's bar folds it; the folds are the person's and hold across tasks (webview storage).
- Width: the person drags the drawer's edge, or moves it with the arrow keys on the edge, between 360 px and 70% of
  the window; a double-click resets it to 460 px. The width is kept across tasks. Below 900 px the drawer lays over
  the board.
- The questions part is Stage 9a's, and "Discuss with the Desk" is Stage 9e's: neither is shown.
- Stage 2's detail panel and its invented data go.

## Success criteria

- **Drawer** (notes 8 and 18): a plan task shows every part in 0.11's order; a scratch task shows no Waits on or
  Unblocks. A fixture source whose field declares `label` shows it, and one without shows the id in words.
- The hostile fixture task of `ts-0183-ee`, opened in the drawer against the real daemon, shows as text and fetches
  nothing.
- A fold made on one task holds on the next. A width set by dragging holds on the next task; a double-click
  resets it to 460 px; the drawer stays between its limits.
- A link in Waits on opens that task in the drawer.
- A collided id shows both copies, and no "differs from main".

## Tests

Write these first.

- `app/test/drawer.test.tsx`: the parts per source over `FakeOwner` worlds (plan task, scratch task, a fixture
  source with and without `label`, a ref-scoped source with a differing copy and a collision); their order; empty
  words.
- `app/browser/fake-drawer.spec.ts`: drag, arrow keys and double-click on the edge; the limits; folds holding across
  tasks; a link opening its task; Esc closing.
- `app/browser/drawer.spec.ts` (bridge): a plan task and a scratch task on a temp repo; the hostile fixture task's
  body against the CSP.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 6a: Task drawer, read-only (the parts, the folds, the width; Fields from the source). Success Criteria:
    Drawer; the hostile fixture task in the drawer. Tests: "the drawer's parts per source"; "the hostile-body
    fixture in a browser, against the CSP".
  - 0.11: Task drawer (its parts and their order).
- Design: notes 8, 18 and 39. Mock functions: `drawerHtml`, `fieldsHtml`, `foldSections`, `linkRow`, `mdHtml`.
- **Decision for the item, with a recommendation: the raw fields.** Recommend showing the task as the source gave
  it on `task.get` (key, status, category, labels, deps, fields), as JSON. Alternative: a `task.get` option that
  returns the file's frontmatter, as the mock shows for a plan task; one more provider path for an engineer's view.
- **Decision for the item, with a recommendation: the source's operations.** 0.11 says the source's operations show
  as forms in the drawer, and the mock's drawer is read-only. Recommend no forms in 6a (Stage 6a says read-only).
  No stage owns them yet: see the split's "Where the plan is unclear".
- Out of scope: Changes and the diff window, and the id peek (`ts-0189-fg`); the questions (9a); Discuss with the
  Desk (9e); claims, coverage, checklists and inverse link labels (`ts-0195-ju`).
- Area: `app/src/` (a `drawer/` folder), `app/src/owner/methods.ts`, `app/test/`, `app/browser/`.
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
