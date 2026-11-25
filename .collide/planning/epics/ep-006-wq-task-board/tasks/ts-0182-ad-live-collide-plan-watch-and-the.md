---
id: ts-0182-ad
kind: task
status: done
title: "Live: collide/plan watch and the Field's quiet updates"
owners: []
links:
  - { type: blocked-by, to: ts-0181-ft }
---

## Goal

The built-in `collide/plan` source watches its files, so a change in any checkout reaches the board within the
debounce. The Field follows the daemon's events quietly: a changed card flashes once and the Live stamp updates,
with no toast (note 14).

What exists.
- `packs/plan/src/provider.ts` declares `capabilities: { watch: false, … }`. The daemon lists a source with no
  `watch` again every `recheck` (10 s; `host/src/corpus_service.rs`), and reconciles each list
  (`reconcile_by_source`), recording `daemon.event` `TaskChanged`, `Renamed`, `Removed` and `Invalid`.
- The Orchard fixture's provider has a watcher (`fixtures/packs/orchard/providers/plan/watcher.ts`, `ts-0112-px`):
  one watcher per checkout and its git folder, debounced (`DEBOUNCE_MS` 300), a full read every `RECHECK_MS`
  (10 s), and the notifications computed by comparing snapshots (`changes.ts` `compare`). It takes its clock and
  file watcher from the caller, so its tests wait for no real time.
- `collections.changed` records no event (`collections_changed` in `corpus_service.rs`), and `collide/plan`
  declares `collections: false`.
- From `ts-0181-ft`: the Field over `task.list`.

What this adds.
- `watch` for `collide/plan`, modeled on the fixture's watcher: each checkout's planning root and private planning
  root, and the git folder's `HEAD`, debounced, with its own full read every 10 s as the net under missed events.
  It notifies `task.changed`, `task.removed` and renames as the scan finds them. It ignores the files of a spike
  folder other than the spike's own (`ts-0126-nf`), and covers `questions/` (`ts-0127-js`). A checkout that goes
  away has its watchers closed. `capabilities.watch` becomes true.
- The Field's live reducer: on a `daemon.event` for the shown board, it fetches the view again, debounced, and keeps
  the selection, scroll and folds. A card whose status, chips or column changed flashes once. The Live stamp at the
  top of the Field says when the board last changed. No toast. An event for another board is ignored.
- A change only in what a task is part of (its epic) moves its stage on the card: the reconcile records it as a
  `TaskChanged` (decision below).

## Success criteria

- A `plan set` of a task's status in a worktree shows as a divergence chip within the watch debounce plus the 10 s
  recheck.
- A status change in the main checkout moves the card to its new column, and the card flashes once. No toast shows.
- A task moved to another epic shows its new stage.
- A spike's own files change its task; a change to another file in the spike folder sends nothing.
- A new question file under `questions/` shows without waiting for the recheck.
- The fixture's watcher and `collide/plan`'s give the same notifications for the same edits.

## Tests

Write these first.

- `packs/plan/test/watch.test.ts`: fake clock and fake file watcher, as the fixture's tests do: debounce, the full
  read, a renamed task, a removed checkout, the spike-folder rule, `questions/`.
- `packs/plan/test/`: one end-to-end run of the provider over stdio, with real files in a temp folder and a short
  debounce, showing `task.changed` after an edit.
- `app/test/`: the Field's reducers over provider notifications (Stage 6a test "Reducers over provider
  notifications"): an event for the board refetches once for a burst, an event for another board does nothing,
  the flash marks only cards that changed, selection and folds hold.
- `host/tests/`: a relist that changes only a task's collections records a `TaskChanged`.
- `app/browser/field-live.spec.ts` (bridge): a status change in a worktree, written by the test's own git and file
  helpers, shows as a divergence chip within 11 s.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 6a: Field ("live updates from `watch`"; "Live updates are quiet"). Success Criteria: "A `plan set` in a
    worktree shows as a divergence chip within the watch debounce plus the 10 s recheck." Tests: "Reducers over
    provider notifications."
  - 0.10: Ref-scoped sources; the provider's `watch`. 0.11: the Field.
- Design: note 14. Mock: the Live stamp and the card flash in `fieldHtml` and `cardHtml`.
- Carried: `ts-0126-nf` ("`watch` ignoring the rest of a spike folder: Stage 6a, when `watch` is built");
  `ts-0127-js` ("`watch` over `questions/`: Stage 6a, when `watch` is built"); `ts-0151-fk` (when the Field lists
  views it "needs a signal for a collections change too"): the decision below.
- **Decision for the item, with a recommendation: refetch, not patch.** Recommend the Field fetching the view again
  on an event, debounced to one fetch per burst. The view's members, closure and counts are the daemon's to compute.
  Alternative: patching cards from the event's task; cheaper, but a view's membership can change with it.
- **Decision for the item, with a recommendation: a collections change.** Recommend the reconcile recording
  `TaskChanged` when only a task's collections changed, as a list shows them. No new event kind. Alternative: a
  `CollectionsChanged` event in the catalogue; more to keep in step with Stage 9's consumers.
- **Decision for the item, with a recommendation: sharing the watcher.** Recommend copying the fixture's watcher
  into `packs/plan/src/` and adapting it, since the fixture stays a fixture. Alternative: a shared module both
  import; it couples a test fixture to a shipped pack.
- Out of scope: the adopted-worktree reading and its minute poll (0.4, "Worktrees an agent makes for itself"),
  which `ts-0116-wh` named for 6a. It attributes worktrees to runs, and Stage 8's bullet "Worktrees an agent makes
  for itself" builds it.
- Area: `packs/plan/src/` (a new `watch.ts`, `provider.ts`), `packs/plan/test/`, `host/src/corpus_service.rs`,
  `app/src/` (the Field's live state), `app/test/`, `app/browser/`.
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

- Finding on the reconcile. `reconcile_by_source` (`core/src/corpus.rs`) compares the whole `Task` and records
  `TaskChanged` with the cause `startup`, so a relist that changes only a task's links, or only its collections,
  already records one. The epic move needs no new code there. What recorded nothing was the provider's
  `collections.changed` notice: `collections_changed` in `host/src/corpus_service.rs` applied it and said nothing.
  It now records one `TaskChanged` for each task the notice moved (new `record_moved`), with the diff
  `collections` and the cause `worktree`. No new event kind, no wire change.
- `packs/plan/src/`: `watch.ts` (the watcher: `createWatcher`, `isPlanChange`, `DEBOUNCE_MS` 300, `RECHECK_MS`
  10 s; the clock and the file watcher are given by the caller), `changes.ts` (`compare` of two snapshots, and
  `diffOf`, which names every top-level field that differs), `git.ts` (`gitHead`, `gitDir`, `gitRenames`),
  `provider.ts` (`watch` handler; `capabilities.watch` is true; `planHandlers` became `planProvider`, which also
  returns `stop`; `servePlan` stops the watcher when its input ends), `scan.ts` (`SPIKE_RECORD`). It watches each
  checkout's planning roots, the private root of the main checkout only, and the folder of `HEAD`. It sends
  `task.changed`, `task.removed`, `task.renamed`, `task.invalid` and `source.stale`. Only a spike's own `record.md`
  counts inside a spike folder (`ts-0126-nf`); `questions/` is under the epics folder and is covered
  (`ts-0127-js`).
- `app/src/field/live.ts` (new): `isBoardEvent`, `changedCards`, `agoWords`, `Burst` (one job for a burst of pokes,
  and one more after it if poked while it runs), `FieldFeed` (subscribes, reads with `loadField`, reads again on an
  event, keeps the board while it reads, publishes nothing when a read finds the same board). `useField.ts` runs one
  feed for each board and view and returns `flashed` and `updatedAt`. `FieldPage.tsx` has the Live stamp
  ("Live · updated just now") and marks a changed card with `data-flash` and `field-flash`; `index.css` has the
  keyframes and `--primary-soft`. `FieldData` has `settling` (`types.ts`, `load.ts`). Selection, folds and scroll are
  not touched: `Board` stays mounted while its data is replaced. No toast.
- Tests, host, in `host/tests/daemon_corpus.rs`:
  `a_collections_changed_notice_records_a_task_changed_for_each_task_it_moved`,
  `a_relist_that_changes_only_a_tasks_collections_records_a_task_changed` and
  `a_relist_that_changes_only_a_tasks_links_records_a_task_changed`.
- Tests, pack: `packs/plan/test/watch.test.ts` (39 tests: a change between `list` and `watch`, the debounce, the
  full read, a status write, a moved epic, a new task, a question, the private root, a worktree, the spike rule, a
  removed file, an invalid file, a git rename, a checkout that cannot be read, the end of the input, a real
  in-process watcher, and the provider as a process over stdio), `test/fakes.ts` (fake clock and file watcher),
  `test/helpers.ts` (`connect` hears notifications), `test/provider.test.ts` (`watch: true`).
- Tests, app: `app/test/field-live.test.ts` (events of the board and of another board, the cards that flash, the
  stamp's words, one read for a burst, the feed over a fake owner and a fake clock), `field-page.test.tsx` (stamp,
  flash mark, no alert), `support/fake-clock.ts`, and `FieldWorld.edit`, `announce`, `lists`. Browser:
  `app/browser/fake-field-live.spec.ts` (the page's own clock is frozen and stepped: the card moves and flashes once
  with no toast, a burst is one read, another board's event is ignored, selection and a fold and the scroll stay,
  the connection coming back reads) and `app/browser/field-live.spec.ts` (bridge, real daemon: a workspace's copy
  shows as a divergence chip, a change in the main checkout moves the card and it flashes, the page is not reloaded).
- Seen red at runtime. Host: the `collections_changed` test failed against the old function; the two relist tests
  pass on the old code, which is the finding above. Pack: the watch tests were written after the code and seen red
  by a `watch` handler that answers and watches nothing, and by breaking the spike rule and the diff. The
  tests of a change between `list` and `watch` were written before the code and failed (three through the fakes, one
  over stdio) until the provider remembered its list. App:
  `field-live.test.ts` was written after `live.ts` and seen red by four breaks (every task event taken as the
  board's, a burst not coalesced, every card flashing, a read of an unchanged board published); the stamp and flash
  tests in `field-page.test.tsx` were red before `FieldPage.tsx` changed, and so were the `settling` tests and the
  no-flash-while-settling test before their code; `fake-field-live.spec.ts` was red with the feed deaf to events.
  The bridge spec found two real faults, both fixed: a read made while the daemon was still listing ignored every
  event, and every card flashed as the first listing arrived.
- Existing tests changed: `FieldData` has `settling`, so `field-model.test.ts` has it in its data and
  `field-page.test.tsx` builds its state from one `IDLE`.

### Decisions for the owner

- Which board an event is for. A `daemon.event` has a source name and no board. The Field takes a task event as its
  own when the board it read has that source, and takes every task event while the daemon is still listing or a
  source has not started (`settling`). Two boards that both have a `plan` source cannot be told apart: the other
  board's event costs one read that finds nothing and shows nothing. Alternative: a `board` on the task events, a
  wire change and a change for `host`. Recommendation: keep, and add the board id when a second consumer needs it.
- The plan's diff names every changed top-level field, because core fetches the task when a diff names a field other
  than status or category. Alternative: send the whole task. Recommendation: keep.
- A rename is only what git says it is: an id that went while another came is a rename only when git renamed the
  file between the two commits. Otherwise it is a removal and a new task. The private root is not in git, so it has
  no renames. Alternative: match by content. Recommendation: keep, since a wrong rename keeps a stale link alive.
- A change between the daemon's `list` and its `watch` is sent at once. The provider keeps, for each checkout, the
  snapshot (HEAD, tasks as `list` gave them, invalid files) its last `list` of that checkout was computed from, and
  only when HEAD did not move while it read. `watch` takes it as the baseline, once, and reads the checkout again
  right after its handles are open, so the change is a `task.changed` without an event or the recheck. With no
  remembered snapshot the watcher reads one, as before. No wire or host change. The Orchard fixture's watcher still
  has the gap. Alternative: have the daemon send its list's version with `watch`. Recommendation: keep.
- The watcher uses recursive `fs.watch` on the planning folders, so it also sees the folders of spikes and their
  `node_modules`, and then ignores them. Cheap on macOS; heavier on Linux inotify. Alternative: one watcher for each
  folder outside a spike. Recommendation: keep until a Linux build.
- A card flashes when its column, title, stage, status or chips changed, and not for a change it does not show. A
  board that is still being listed does not flash. A card that moves column is a new element, so a keyboard focus on
  it is lost. Recommendation: keep; restore the focus when the drawer item needs it.
- The Live stamp says "just now" from the first read, then how long ago a read last changed the board. It looks at
  the clock every 5 s.
- A divergence chip needs a workspace: the corpus names a copy divergent only for a worktree in the daemon's
  workspaces folder (`divergences_of` in `core/src/corpus.rs`). A worktree made elsewhere is a copy, not a
  divergence, so the bridge spec uses a workspace. Proposed plan text for Stage 6a's success criterion: "A `plan set`
  in a workspace of the board shows as a divergence chip..." Recommendation: say "workspace".
- The bridge spec waits 8 s, under the daemon's 10 s recheck, so a pass proves the `watch` path. The bridge world's
  daemon takes the bundle staged by `pnpm build:packs`, so the spec fails on a bundle staged before `watch` existed.
  Recommendation: run `pnpm build:packs` before the suite.
- `collections.changed` records `TaskChanged` with the cause `worktree` and the same state in `from` and `to`; there
  is no cause for a collections change. Alternative: a new cause. Recommendation: keep.

### Not done, by design

- A board id on the daemon's task events (above): no item yet.
- The adopted-worktree reading and its minute poll: Stage 8, "Worktrees an agent makes for itself".
- The drawer's content, filters, search and drag: `ts-0184-hv` and later.
