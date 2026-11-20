---
id: ts-0157-hd
kind: task
status: done
title: "Corpus: a changed item keeps its old title, fields and links"
owners: []
links:
  - { type: blocked-by, to: ts-0151-fk }
---

## Goal

`Corpus::apply_changed` in `core/src/corpus.rs` updates a held copy from `task.changed` with the new state and version
only. The notification's `diff` names a changed title, link or field too, and the copy keeps the old one. So a title
edited on disk lists with its old title, and a `blocked-by` link added on disk leaves readiness (the `ready` view,
`collide tasks --ready`) wrong, until the source is listed again. Found at the `ts-0151-fk` build.

## Success criteria

- Through the daemon, over a fixture tree, with no restart and no relisting asked for: a task whose title changes on
  disk lists with the new title once the watcher has reported; a ready task that gains a `blocked-by` link to an open
  task leaves `--ready`; one whose blocking link goes joins it.
- The store's cache holds the new copy, so a restarted daemon serves it before its relist ends.
- A change of the status alone makes no more provider calls than today.
- Every provider of the repo that sends `task.changed` (Collide's plan, the Orchard pack's, the others under `fixtures/`
  and `packs/`) fills `diff` for what this needs; one that does not is named in Built.

## Tests

- Write first and see red at runtime: the title and the link criteria as daemon tests; today the old title and the old
  readiness stay.
- A corpus unit test: a change whose `diff` names anything but the status needs the task; one that names only the status
  does not.

## Notes

- Recommendation: `apply_changed` answers `NeedsTask` when the `diff` names any field but the status, and the daemon
  fetches the one task with `get`, as it does for a copy it does not hold (`ts-0115-ug`). No wire change. Alternative:
  `task.changed` carries the whole task; a wire change, and every provider sends more on every change.
- A `get` of an Orchard task runs the plan CLI while a question is open (`queue.ts`); say what that costs per change.
- Owner decisions: none recorded yet. The recommendations stand until the owner answers.

### Built

What each provider puts in `diff`, read first.

- The Orchard provider (`fixtures/packs/orchard/providers/plan/changes.ts`, `diffOf`) names the status, the title, each
  of the task's fields (`proposal`, `epic`), the path and `blocked-by` links (as `depends_on`). It did not name the
  other links of the kinds (an ADR's `evidence` and `supersedes`, a spike's `supersedes`, a question's `blocks`). Fixed
  here: `diffOf` now names each link type as `{added, removed}`, by its type, and `blocked-by` stays `depends_on`.
- Collide's own plan provider (`packs/plan`) declares `watch: false` and sends no `task.changed`. Nothing to fix.
- The test fakes: `fake_provider.py` sends one `task.changed` for a new item (`from` null), which has no held copy, so
  it is fetched as before. The daemon tests send `task.changed` through `notify`, and their `diff` is whatever the test
  writes. The scratch source's events (`changed_event` in `host/src/sources.rs`) are recorded, never applied, and name
  `status` and `category`.
- `FieldDiff` names the status as the key `status` (`core/src/change.rs`, and every provider above). The category moves
  with it, and daemon-made events name it as `category`; the notice's `to` carries both.

The change.

- `core/src/corpus.rs`: `Corpus::apply_changed` answers `Applied::NeedsTask` for a held copy when the `diff` names any
  key but `status` and `category` (`names_only_the_status`). A `diff` that names none is a change of the version alone
  (a body edit) and is applied as before, with no fetch. New `Corpus::apply_state`: the old held-copy update (state and
  version only), whatever the `diff` names. The shared body is the private `apply`.
- `host/src/corpus_service.rs`, `changed`: on `NeedsTask` it fetches the one task (`fetch`, as for a task it does not
  hold) and applies it. If the fetch gives no task, it logs and calls `apply_state`, so a held copy still takes the new
  status and version. `keeping_collections`: a copy that is held keeps the collections the corpus holds; see Decisions.
  `keep_copy` then puts the new copy in the cache, so a restarted daemon serves it.
- `fixtures/packs/orchard/providers/plan/changes.ts`: `diffOf` as above.
- `host/tests/fixtures/corpus_pack_provider.mjs`: `get` writes a `get <checkout> <id>` line to `calls.log`, and a
  `fail-get` file in the data folder makes `get` answer an error.

Cost of a fetch of an Orchard task. `get` runs `read`, which scans the whole checkout (every task, ADR, spike and epic
file under `arch/`) and, while a question is open, runs `./plan status` once (30 s cap). The SDK answers one request at
a time, so a burst of changes is fetched one after another, and the daemon's notification pump waits for each. A change
of the status alone, and a body edit, cost none of it. Per changed title, field or dependency: one scan of the checkout,
plus one plan CLI run while a question is open.

Tests, with what was red at runtime. For each red run I stubbed the new check in `apply` to the old behavior
(`if false && ...`), and put it back.

- `core/tests/corpus.rs`: `a_changed_notification_that_names_a_field_besides_the_status_asks_for_the_task` (red),
  `a_task_given_for_a_changed_notification_brings_its_title_and_links_so_readiness_follows` (red),
  `a_changed_notification_that_names_only_the_status_updates_the_copy_without_the_task`,
  `a_changed_notification_that_names_nothing_takes_the_version_and_asks_for_no_task`,
  `apply_state_takes_the_state_and_version_whatever_the_diff_names`,
  `apply_state_for_a_task_the_corpus_does_not_hold_asks_for_the_task` (green before and after, as they should be).
- `host/tests/daemon_corpus.rs`, all through the daemon with no restart and no relisting:
  `a_title_changed_on_disk_lists_with_the_new_title_and_the_cache_keeps_it` (red, timed out on the old title),
  `a_blocked_by_link_added_on_disk_leaves_ready_and_one_removed_brings_the_task_back` (red, the task stayed ready),
  `a_change_whose_task_cannot_be_fetched_still_moves_the_state_and_the_version` (red on the fetch count; red too, by
  mutation, when the fallback is removed), `a_changed_item_keeps_the_collections_that_collections_changed_gave_it`
  (red by mutation: with `keeping_collections` removed the collections come back empty),
  `a_change_of_the_status_alone_fetches_nothing` (green before and after: it holds the no-more-calls criterion).
- `fixtures/arch-plan-tests/test/watch.test.ts`: `a change of any link is named in the diff, by the type of the link...`
  (red: the diff was empty for an `evidence`, `supersedes` or `blocks` change).

### Decisions for the owner

1. A `diff` that names no field at all is applied from the notice, with no fetch. Alternative: fetch it too, in case
   the provider left something out. Recommendation: as built. The Orchard watcher sends an empty `diff` for a body
   edit, which the corpus does not hold (it drops bodies), and a fetch would rescan the checkout for nothing.
   The price: a provider that omits a field from `diff` leaves that field stale until a listing. Orchard now names
   everything its task holds.
2. `category` counts as part of the status. Alternative: only `status`. Recommendation: as built, since `to` carries
   both and the daemon's own events name both.
3. A held copy keeps the collections the corpus holds, when the task fetched for a change replaces it. A task's
   collections come from `collections.changed` (`ts-0151-fk`), and a `get` of some providers does not fill them.
   A task new to the corpus takes the ones the source gave. Alternative: take the fetched ones; the Orchard `get`
   fills them, but a provider whose `get` does not would wipe them until its next `collections.changed`.
   Recommendation: as built. `task.renamed` still takes the fetched task whole, since its comment says core reads it.
4. When the fetch fails or the source has no such task, the held copy takes the state and version from the notice
   (`apply_state`), and the title, fields and links wait for a listing. Alternative: leave the copy as it was.
   Recommendation: as built; the status drives readiness and must not go stale because a fetch failed.
5. Link changes are named by type in the Orchard `diff` (`evidence`, `supersedes`, `blocks`), not under one `links` key.
   Alternative: one `links` key. Recommendation: as built; it follows `depends_on`, which is the field name in the file.
6. Proposed plan text (0.10, Protocol, at `task.changed`; not applied): "`diff` names each field of the copy that
   changed: the status and category, the title, the fields, and each link type as `{added, removed}`. Core applies a
   notice that names no field but the state (or none) from the notice, and fetches the task with `get` for any other,
   and replaces the held copy with it, keeping the collections it holds. A provider that leaves a changed field out of
   `diff` leaves it stale until the source is listed."

### Not done, by design

- Core's startup reconcile (`diff_of` in `core/src/corpus.rs`) does not name link changes in the events it records.
  Those events are recorded, not applied, so no copy is stale. Not filed: nothing reads the links of that diff.
- A write's effect on another item (`note_write`) still updates that item's state and version only, as before.
- `ts-0152-jq`, `ts-0153-kp`, `ts-0155-wb` and `ts-0156-nt`: not changed and not made trivial. `ts-0153-kp` (the fake
  plan CLI held to the real envelope shapes) is not touched: the fetch runs the same `./plan status` as `list`.
- No test restarts the daemon to read the changed copy from the cache; the cache is read from the store while the
  daemon runs (`a_title_changed_on_disk_...`), which is the same row a restart loads.

### At review

- Decisions 1 to 5 taken as recommended (owner delegation). Decision 6's plan text applied, shortened, to the
  `task.changed` bullet of plan 0.10.
- Cost, filed as `ts-0158-ow`: the fetch runs under the board's gate, as the fetch of a task the corpus does not hold
  already did, and an Orchard `get` scans the checkout and runs `./plan status` while a question is open. A branch
  switch that edits many task files now costs one scan and one plan CLI run per file, one after another, while writes
  and other sources' notifications wait.
- Mutations, each red then restored: `category` treated as a field (one core test), an empty `diff` fetching (three core
  tests), the fetched task not applied (two daemon tests, incl. the cache one), the Orchard `diff` naming only
  `blocked-by` (one watcher test). The builder's own: the check stubbed to the old rule, `keeping_collections` removed,
  the fetch fallback removed.
