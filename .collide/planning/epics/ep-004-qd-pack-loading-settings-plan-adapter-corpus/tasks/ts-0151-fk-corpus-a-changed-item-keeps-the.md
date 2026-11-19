---
id: ts-0151-fk
kind: task
status: ready
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
