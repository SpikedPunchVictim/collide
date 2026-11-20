---
id: ts-0157-hd
kind: task
status: ready
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
