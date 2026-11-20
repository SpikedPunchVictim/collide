---
id: ts-0151-fk
kind: task
status: done
title: "Corpus: a changed item keeps the collections of its last listing"
owners: []
links:
  - { type: blocked-by, to: ts-0137-pg }
---

## Goal

`apply_changed` in `core/src/corpus.rs` updates a held copy from `task.changed` with the new state and version and keeps
the rest, `collections` included. A collection can change without its member's file changing: the Orchard pack's human
queue (`plan:queue`, from `./plan status --format json`, `ts-0137-pg`) and an epic's set (`ts-0148-cv`). So a question
resolved on disk stays in `decisions-needed` until the source is listed again, and a question that comes to need a
person shows only then. Found at the `ts-0137-pg` build (its second decision).

## Success criteria

- Over the fixture `kinds` tree, through the daemon: a queued question set to `resolved` on disk leaves
  `decisions-needed` once the watcher has reported, with no restart and no relisting asked for.
- A question set to `needs: human` on disk joins the view the same way.
- An epic's collection follows a change of its epic file the same way.
- A source with no collections makes no more provider calls per change than today.

## Tests

- Write first and see red at runtime: the first two criteria as daemon tests next to
  `host/tests/daemon_reference_kinds.rs`; today the resolved question stays in the view.
- A corpus unit test: a change of collections reaches the members that left and the ones that joined.

## Notes

- Recommendation: the provider's watcher reads the collections when it rescans (the Orchard one with the queue when a
  question or epic file changed) and says what changed, in a notification the corpus applies to members. It is a wire
  change (`pnpm gen`). Alternatives: the daemon fetches the task with `get` on each `task.changed` of a source with the
  `collections` capability (one call per change, and each Orchard `get` runs the plan CLI while a question is open), or
  relists the checkout after a change.
- The watcher scans without the queue today (`fixtures/packs/orchard/providers/plan/watcher.ts`), and `list` and `get`
  run `./plan status` while a question is open (`queue.ts`, 30 s cap).
- Owner decisions: none recorded yet. The recommendations stand until the owner answers at the split.

### Built

What was found first: today's `task.changed` does not carry an epic change. Its diff has an `epic` entry (the provider
diffs every field), but `Corpus::apply_changed` takes only the state and the version, so a held copy keeps its
collections, fields and links. The epic daemon test timed out on that, after the probe had shown the watcher reports.
The same holds for a held copy's title, fields and links: they stay stale on `task.changed` until a listing. That is
older than this item and is not fixed here.

The wire. A new provider notification, `collections.changed`, for a source with the `collections` capability:
`{ collections, members: [{ key, collections }], checkout? }`.

- `core/src/provider.rs`: `CollectionMember`, `CollectionsChanged`, `ProviderEvent::Collections`, the method in
  `from_notification` and in the header table. `core/src/schema.rs` registers `CollectionsChanged`. `pnpm gen` wrote
  `schemas/CollectionsChanged.*.json`, `packages/pack-sdk/src/generated/CollectionsChanged.*.ts` and the index.
- `packages/pack-sdk/src/provider.ts`: `ProviderNotifications['collections.changed']`.
- `core/src/corpus.rs`: `Corpus::apply_collections`. It replaces the checkout's collections and sets `collections` on
  each named copy that the corpus holds, and returns the keys that moved. A key it does not hold, a checkout it was
  never listed for, and a task of another source change nothing.
- `host/src/corpus_service.rs`: the pump applies it under the gate, keeps the cache copy of each key that moved
  (`keep_copy`) and records no event: no item changed, and a task's own change has its own event.
- Orchard pack, `fixtures/packs/orchard/providers/plan`: `scan.ts` exports `withQueue`, `queueSignature` and
  `readQueueOf`, and `scan` now uses `withQueue` (same result for `list` and `get`). `changes.ts`: a `Snapshot` also
  holds the collections, the queue and what it was read for, and `collectionsChanged` diffs two snapshots. `watcher.ts`:
  `takeSnapshot` reads the queue at `watch` and again only when the open questions (key and version) are not the ones
  it was last read for, and `refresh` sends `collections.changed` after `compare`'s notifications. `provider.ts` gives
  the watcher the provider's `timeoutMs`.

A source without the `collections` capability sends nothing new and costs no new call: only the Orchard provider
sends it. In Orchard, a checkout with no open question never runs the plan CLI in the watcher; one with open questions
runs it once at `watch` and then once per change of the open questions, never per task change.

Tests, by name.

- `core/tests/corpus.rs`: `a_collections_notification_moves_the_members_that_left_and_the_ones_that_joined`,
  `..._changes_no_state_and_no_version`, `..._changes_the_copy_of_its_checkout_and_no_other`,
  `..._leaves_a_task_it_does_not_hold_and_a_checkout_never_listed`, `..._changes_no_task_of_another_source`,
  `..._for_a_source_never_declared_is_refused_by_name`, `applying_a_collections_notification_twice_is_the_same_as_once`.
  Three were seen red at runtime against a no-op `apply_collections`.
- `core/tests/provider_protocol.rs`: `collections.changed` and `collections.changed with checkout` in the notification
  cases (round trip, extra key refused, event type) and
  `collections_changed_without_its_collections_or_with_a_member_lacking_a_key_is_refused`.
- `host/tests/daemon_reference_kinds.rs` (the real Orchard pack, the fake plan CLI, the `kinds` tree):
  `a_question_resolved_on_disk_leaves_decisions_needed_with_no_relisting_asked_for`,
  `a_question_set_to_need_a_person_on_disk_joins_decisions_needed_with_no_relisting_asked_for`,
  `a_task_whose_epic_line_changes_leaves_the_old_epic_and_joins_the_new_one`,
  `an_epic_file_that_changes_is_the_epic_as_listed_and_one_that_goes_takes_its_tasks_out`. All four seen red at runtime
  before any change. They flip a status until the watcher reports (a `watching` probe), then wait on the condition with
  a deadline.
- `fixtures/arch-plan-tests/test/watch-collections.test.ts`, ten tests on a fake clock and fake file watcher: epic line,
  epic file gone, epic title, no collection touched, new task (not named), question resolved, question joins in the
  CLI's
  order, last question resolved, CLI asked at the start and only when the open questions change, CLI cannot answer.
  Against the watcher without `collectionsChanged`, eight failed at runtime (the two negative ones pass, as they
  should); with the queue re-read on every scan, the CLI-count test failed.

Three existing tests in `fixtures/arch-plan-tests/test/watch.test.ts` said "nothing else" after a change that moves
a task out of, or renames it within, an epic's groups: the invalid-file-says-it-once test, the two-files-one-short-id
test and the renumber test. They failed with the new notification, which is the epic's groups changing, and now take
the `collections.changed` that follows (the renumber one also checks its checkout and that no member is named).

### Decisions for the owner

1. One notification for collections and members together, one per checkout per read. Alternative: one per changed
   collection. Recommendation: as built; `collections` replaces the checkout's list, so a lost notification is healed by
   the next, and a list is the only other writer.
2. `collections` is always the whole list of the checkout. Alternative: only the ones that changed. Recommendation: as
   built, it is small and cannot drift.
3. A new or renamed task is not named in `members`: core already reads it from its `task.changed` (`get`) or keeps its
   copy through `task.renamed`. Alternative: always name every task whose places differ. Recommendation: as built.
4. The queue is read again only when an open question's key or version changes, so a failed read is not retried until
   one does (the 10 s recheck does not ask). Alternative: retry a failed read on each recheck. Recommendation: as built
   for now; a CLI that is down costs nothing, and the next question edit asks again. Change it if a down CLI at start
   shows badly.
5. A failed read means no queue in the notification (the collection is absent and its members lose it), with the reason
   logged once, as `list` does today. Alternative: keep the last queue. Recommendation: as built, it matches `list`.
6. A task that becomes invalid, goes, or is renumbered also changes the epics' groups (they name keys), so the watcher
   sends `collections.changed` for it with no member. Alternative: compare only memberships and the queue, not the
   groups. Recommendation: as built; core's copy of the groups would otherwise name a task that is gone.
7. Core accepts the notification from any source, not only one with `collections`. Alternative: refuse it at the
   corpus. Recommendation: as built; a source that never sends it is unchanged, and a refusal would be a second place
   that knows the capability.
8. Proposed plan text (0.10, Protocol, where the notifications are named; not applied): "`collections.changed`
   (sources with `collections`): the checkout's collections as `list` gives them now, and for each held task whose
   collections differ, its new ids. It changes no item's state or version and records no event. A provider sends it
   when a collection changes with no change to the member's file: an epic file, or a queue that follows another item."

### Not done, by design

- `Corpus::apply_changed` still keeps a held copy's title, fields and links from its last listing. Only collections
  have their own notification. Not filed: nothing asks for it yet.
- No test shows that the cache copy of a moved task is kept (`keep_copy` in the new arm): no restart test reads it.
- A change that lands between the daemon's first listing and the watcher's first read is not reported by any
  notification. This is older than this item; the daemon tests use a probe that flips again until one is reported.
- `ts-0152-jq`, `ts-0153-kp` and `ts-0155-wb`: not changed and not made trivial. `ts-0153-kp` (the fake plan CLI held to
  the real envelope shapes) is touched only in that the watcher now runs `./plan status` at `watch` and on a change of
  open questions, so a recorded shape of `status` matters to the watcher as well as to `list`.

### At review

- Decisions taken as recommended (owner delegation), including the one put to the owner: a task that becomes invalid,
  goes or is renumbered changes the epics' groups, so `collections.changed` is sent for it too, and the three
  `watch.test.ts` tests that said "nothing else" now expect it.
- Accepted: a failed queue read is not tried again until the open questions change, so one timed-out read keeps the
  queue empty until a question changes or the source is listed again. Trying on every recheck would run a failing plan
  CLI every 10 s.
- Added at review: `a_collections_notification_alone_moves_the_stamp_that_keys_a_cached_view` in `core/tests/corpus.rs`.
  Removing `touch()` from `apply_collections` left every test green, because in the daemon tests a `task.changed` always
  comes first and moves the stamp; a collections change after a view was cached would have left that view stale.
- Plan 0.10's watch notifications gained `collections.changed`, worded to cover a task's own epic change, which the
  watcher sends it for too (`task.changed` carries no collections).
- The finding that `apply_changed` keeps a held copy's title, fields and links is filed as its own item: a `depends_on`
  edit leaves readiness wrong until a relist.
- The app has no view page yet and reloads on `task.changed` only; when the Field lists views (Stage 6a) it needs a
  signal for a collections change too, since this notification records no event.
- Mutations, each red then restored: copies' collections not changed (three core tests), the checkout's collections not
  replaced (one), no `touch()` (red only with the test added at review), the daemon ignoring the notification (four
  daemon tests), the watcher not sending it, the queue never read again.
