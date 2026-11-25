---
id: ts-0184-hv
kind: task
status: done
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

### Built

- `app/src/drawer/`:
  - `Drawer.tsx`: the container. It reads the task (`useTaskGet`), keeps folds and width in the webview's storage,
    closes on Escape (`useEscape`; it ignores a key that something above it took, and an open `<dialog>`, which is
    the link window), and has the edge (`Edge`: pointer drag with capture, arrow keys, double-click). The width is a
    CSS variable on the wrapper (`--drawer-w`), which `style-src 'self'` allows. Below 900 px the wrapper is
    `fixed` at the right of the window and the edge is hidden.
  - `DrawerView.tsx`: the drawing, from what was read, with no state. The header (status chip, category in words,
    id, Close, title, the read-only line), then each part under a bar that folds it (`PartBody`, `LinkRows`,
    `PartOfChips`, `Fields`, `Copies`, `Details`). Text from a task is React text; the body goes through `Markdown`.
  - `model.ts`: `partsOf` (which parts, in 0.11's order), `blockingLinks`, `partOf`, `fieldRows`, `copyRows`,
    `detailsOf`, `readOnlyLine`, `categoryWords`.
  - `load.ts`: `useTaskGet`, `task.get` again on each change of the Field's `updatedAt`. A task is never shown with
    another's data, and a failed read after a good one keeps the good one and shows the alert.
  - `folds.ts` (`readShut`, `toggleShut`; Details for engineers starts folded), `width.ts` (`MIN_WIDTH`,
    `DEFAULT_WIDTH`, `maxWidth`, `clampWidth`, `readWidth`, `writeWidth`), `types.ts`.
- `app/src/field/FieldPage.tsx`: `DrawerPlace` is gone. `Board` holds the selected card id, finds its task on the
  board or else in the corpus (a link may name a task the view leaves out), opens a link's task, and hands the focus
  back to the card on close. `FieldPage` takes `owner` and `board`; `Board` has `key={boardId}`, so a selection does
  not outlive its board. `app/src/App.tsx` passes both.
- `app/src/field/colours.ts` (`DOT`, `TEXT`, moved out of `FieldPage.tsx`); `app/src/field/model.ts` exports
  `cardIdOf`, `sourceOf`, `tailOf`, `capital`, `categoryOf`.
- `app/src/frame/actions.ts`: scopes `drawer` and `drawer-edge`; actions `drawer-close` (Escape), `drawer-wider`
  (ArrowLeft) and `drawer-narrower` (ArrowRight).
- Test support: `app/test/support/loop-world.ts` (`getTask`, the old scratch answer), `field-world.ts` (`task.get`
  from the listing, with link titles, link types, views and copies; a collided key answers ambiguous with no task;
  the listing has no body, as the daemon's; `drawerWorld`, `TRACKER`), `worlds.ts` (`field-drawer`),
  `browser/support/fixtures.ts` (`openPage` takes a `watch` callback, as `openFake` does).
- Tests:
  - `app/test/drawer.test.tsx` (the parts per source, their order, empty words, labels, copies, collided ids, details,
    folds, the waiting and failed states, hostile text, `blockingLinks`).
  - `app/test/drawer-state.test.ts` (width limits, steps, storage that is missing, broken or holds junk; folds).
  - the drawer cases of `app/test/actions.test.ts`.
  - `app/browser/fake-drawer.spec.ts` (14 specs: parts, Close with focus back on the card, Escape and the link
    window, folds across tasks and a reload, a link opening its task, collided and differing copies, drag, limits,
    arrow keys and double-click, overlay below 900 px, a narrower window, a live event, the hostile body).
  - `app/browser/drawer.spec.ts` (bridge, real daemon on a temp root: a plan task, a scratch task, the hostile body
    under the CSP with no request out, no dialog and no policy error).
  - `app/browser/fake-field.spec.ts` and `fake-field-live.spec.ts`: a card is found by `data-card`, since a link in
    the drawer has a card's words; the stub text became the read-only line.
- Seen red at runtime. Unit: against stub modules (`DrawerView` an empty `aside`; width, folds, `blockingLinks` and
  `partsOf` returning neutral values) 61 of the 103 tests of the three files failed on values; the 42 that passed
  are the ones that say a thing is absent or does not throw. Browser: against the old drawer place (title, Close,
  "The drawer comes next.") all 14 specs of `fake-drawer.spec.ts` and all 3 of `drawer.spec.ts` failed on missing
  content, the bridge ones after the board opened and the card was clicked.
- Stage 2's detail panel and `placeholders-detail.ts` were already removed by `ts-0179-ke`; nothing was left to delete.
- No Rust changed, so `cargo test --workspace` was not run.

### Decisions for the owner

- `task.get` gives no task and no body for a collided key. The drawer then shows the task as the board lists it
  (title and status, no Description, no Waits on or Unblocks) and the copies of both. Recommendation: keep. Alternative:
  `task.get` takes a path, so the body of each copy can be read. One wire change, for a case the mock shows with a title
  only.
- Refetch on the Field's `updatedAt`, not on a second event subscription. The drawer and the Field then agree, and the
  daemon is asked once more for each change the board saw. Alternative: the drawer listens for events of its own key,
  which saves a read for other tasks' changes and doubles the event code. Recommendation: keep.
- Part of shows when the task has an epic, a collection or a view, and is left out otherwise. The mock always shows
  it. Recommendation: keep, as the item says each part is left out when the source has nothing for it.
- "differs from main" and "same as main" compare each copy's status and version with the shown copy's, which is
  the default root's. A copy in a run's workspace is named by the run ("Run r-121"), from the divergence list, else by
  its checkout's folder name, with the full path under it. Alternative: branch names, which `task.get` does not give.
- The raw fields are the task as `task.get` gave it (key, status, category, title, owners, labels, links, fields),
  as JSON, without the body. The owner took the recommendation. A plan task's frontmatter as written is the
  alternative the item named.
- The edge is a `separator` with a value, focusable, that holds the pointer while dragged. It is 12 px wide and
  half over the drawer. Below 900 px it is hidden: an overlay has one width. Recommendation: keep.
- Escape closes the drawer from anywhere in the window, unless the key was taken (the link window, a menu). Alternative:
  only with the focus inside the drawer, which would make Escape do nothing after a click on the board. Recommendation:
  keep.
- A link to a task the view leaves out still opens in the drawer, with no card lit. Alternative: switch the view to
  the whole board. Recommendation: keep; it changes nothing the person set.
- The checkout path of a copy shows under its label. It is the real path on the owner's machine. Alternative: leave
  it for Details for engineers. Recommendation: keep; a worktree has no other name.

### Not done, by design

- The source's operations as forms: no stage owns them yet (the split's "Where the plan is unclear").
- Changes and the diff window, and the id peek: `ts-0189-fg`.
- The questions part and "Discuss with the Desk": Stage 9a and Stage 9e.
- Claims, coverage, checklists and inverse link labels: `ts-0195-ju`.
- A look at the drawer in the bundled app's window: the owner's, with `ts-0185-ge` and `ts-0197-uj`. No test here opens
  the Tauri app.
