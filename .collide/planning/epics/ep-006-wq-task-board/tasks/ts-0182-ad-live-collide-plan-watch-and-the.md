---
id: ts-0182-ad
kind: task
status: ready
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
- Owner decisions: none recorded yet. The recommendations stand until the owner answers at the split.
