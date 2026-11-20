---
id: ts-0158-ow
kind: task
status: ready
title: "Orchard: fetching one changed task rescans the checkout"
owners: []
links:
  - { type: blocked-by, to: ts-0157-hd }
---

## Goal

Since `ts-0157-hd`, a `task.changed` whose `diff` names more than the status makes the daemon fetch that one task with
`get`, while it holds the board's gate. The Orchard `get` (`fixtures/packs/orchard/providers/plan/provider.ts`) scans
the whole checkout and, while a question is open, runs `./plan status` (30 s cap). A branch switch or pull that edits N
task files costs N scans and N plan CLI runs, one after another, and writes and other sources' notifications wait for
all of them.

## Success criteria

- A `get` for a checkout the watcher watches answers from the watcher's last read of it: no scan and no plan CLI run. A
  `get` for a checkout it does not watch reads as today.
- That answer is what a fresh `get` would give right after the read: the same task, body and collections.
- With a question open, a burst of N task edits in one watcher read runs the plan CLI at most once for the watcher and
  not once per fetch.

## Tests

- Write first and see red at runtime: with a stand-in plan CLI that counts its runs (as
  `fixtures/arch-plan-tests/test/watch-collections.test.ts` does), N edits then N `get`s run it once, not N + 1.
- A `get` for an unwatched checkout still reads the files.

## Notes

- Recommendation: as the criteria say, pack-local. Alternative: the daemon coalesces a burst of fetches into one relist
  of the checkout; general to every provider, more code in the daemon's event path.
- The gate held across a fetch predates this item (a task the corpus does not hold was fetched under it already).
- Owner decisions: none recorded yet. The recommendations stand until the owner answers.
