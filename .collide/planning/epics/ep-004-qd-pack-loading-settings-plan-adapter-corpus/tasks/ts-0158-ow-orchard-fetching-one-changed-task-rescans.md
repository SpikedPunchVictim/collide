---
id: ts-0158-ow
kind: task
status: done
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

### Built

A `get` for a checkout the watcher holds is answered from the watcher's last read of it. Pack-local, on the owner's
delegation. All in `fixtures/packs/orchard/providers/plan/`.

- `provider.ts`: `get` asks `watcher.held(checkout)` for the last read, finds the task in it (`fromRead`), and checks
  that the task's own file is still what the read saw: one `lstat` (it must be a plain file, as a scan needs) and one
  `readCorpusFile` of that file, whose `version` (sha256 of the whole content) must equal the read's. The answer is the
  read's task with the body of that read of the file. Any miss (no watcher, checkout not watched, task not in the read,
  file gone, link, other content, read stale, older than a write, or made with a question open and no queue) reads as
  before: a scan, and `./plan status` when a
  question is open. `apply` calls `watcher.wrote(root)` in a `finally`.
- `watcher.ts`: `Watcher.held(checkout)` and `Watcher.wrote(checkout)`. Each read (`set`, `refresh`) takes a number
  from a counter before it begins and keeps it as `Watched.readAt`; `wrote` takes a number when a write ends and marks
  the checkout dirty, so it is read again after the debounce. `held` gives the snapshot only when the checkout is
  watched, not `stale`, `readAt` is above the checkout's last write number, and the read is not one with a question
  open and no queue (`asked !== ''` and `queue === null`: the plan CLI could not say, and the watcher does not ask
  again until a question changes).
- `scan.ts`: `isNamed` (the rule of `findTask`, for one task), so the read and a scan name a task the same way.

How a burst costs one plan CLI run: the watcher already runs `./plan status` at most once per read, and only when the
open questions changed (`queueSignature`). The `get`s that follow no longer run it.

**Every caller of the provider's `get`, and whether it can now be answered from the read.**
1. The daemon's fetch after `task.changed` and after `task.renamed` (`BoardCorpus::fetch`, `corpus_service.rs`):
   yes, and it is the case this item is for. The watcher sets its snapshot before it sends the notice, so the read
   holds the task the notice names. The fetch runs under the board's gate, now with one file read.
2. `task.get` (`task_get`, `daemon.rs`, both of its `get`s): yes, when the checkout is watched. It wants the body, which
   comes from the file read at that moment, so the body is never from the read.
3. `run_task_of` (a run agent's `create`, `daemon.rs`): yes. It uses `id` and `epic`, both from the task's own file.
4. The person-only judgment (`judge_write`, `daemon.rs`), when the corpus does not hold the task: yes. It uses the
   kind and status, both from the task's own file, so they are exact.
5. The write paths in the provider (`setStatus`, `create`) do not call `get`: they scan with `findTask`, which is
   unchanged. A write is never planned from a read.
6. Tests: `list.test.ts`, `kinds.test.ts`, `hazards.test.ts`, `checkouts.test.ts`, `queue.test.ts`, `write.test.ts` and
   `provider-stdio.test.ts` call `get` on a provider that was never told to watch: unchanged, no read to answer from.
   `watch.test.ts` sends `get` of a task that is not there as a round trip: the read does not hold it, so it reads as
   before. The new `get-from-read.test.ts` is the one that answers from a read.

**What else can be stale in an answer from the read, and for how long.** The task's own file cannot be: it is checked
on every `get`. What is derived from other files can be, since the read was made some time ago:
- `collections`: the epics that hold the task (an epic file that comes, goes or is edited) and the places in the human
  queue. For an epic file, until the watcher reads again: `debounceMs` (300 ms) after an event in `arch/`, and at most
  `recheckMs` (10 s) after the change when the event was missed, plus the time of the read.
- The links of an ADR or a spike (`evidence`, `supersedes`) read each spike id with the set of spike folders
  (`spikeRef`), so a spike folder that comes or goes can change one: the same bound.
- A second file that takes the task's short id makes both invalid in a fresh read; the answer from the read still gives
  the task: the same bound.
- The queue is not in this list: a read in which a question was open and `./plan status` gave no queue
  (`asked !== ''` and `queue === null`) answers no `get`, so a `get` asks the CLI again, as it did before this item.
  A read with a queue is as old as the open questions it was made for: the watcher asks `./plan status` again only
  when an open question (its id or version) changed, so a change elsewhere that moves the queue shows in a `get` only
  when it does in the watcher's snapshot, which is also what `collections.changed` says. That is the same state as
  `list` has from the watcher, and is the one thing left with no time bound.

**How a write of this provider cannot leave a later `get` on an older read.** Three things, any of which is enough for
the case it covers.
- The SDK serves one request at a time, so a `get` never overlaps an `apply`.
- The task the write changes has a new version, so its file no longer matches the read: that `get` reads as before.
  A task the write makes (`create`) is not in the read, so it reads as before.
- A write can change what is not the task's file (the plan CLI refreshes epic tables and may touch other files).
  `apply` therefore calls `wrote` when it ends, success or error, and `held` is null for every read that began before
  that. The checkout is marked dirty, so the watcher's next read, which begins after the write, makes a new held read.
  A read already in flight when the write ended began before it, so it does not count either. `preview` writes nothing
  (it fails if a file changed) and does not call `wrote`. There is no `move` op in this provider.

Tests, in `fixtures/arch-plan-tests/test/get-from-read.test.ts` (14). Red at runtime before the code (the old `get`,
which always reads): `a burst of edits, then a get of each...` (5 plan CLI runs where 2 are expected), `a get that
names the watched checkout is answered from the read...` (2 where 1), `is answered from the read again once the watcher
has read the checkout after the write` (4 where 3). The others are guards that hold before and after, and were seen
red under a mutation of the new code, each restored: `wrote` made a no-op (the epic-removing write test), the version
check removed (`a task that is edited and not yet read...`), `held` ignoring `stale` (`a checkout that the watcher could
not read...`), the `lstat` removed (`a task whose file became a link...`), `held` always null (the three above).

Added at review: a read whose queue could not be read answers no `get`. Test first, red at runtime:
`a read whose queue could not be read does not answer: the get reads the queue again` (stand-in whose first `status`
fails, so the watcher's read has no queue: the `get` of `Q-03-03` had no `plan:queue`, and the stand-in ran once, not
twice). With the check in, it has `plan:queue` and the stand-in ran once more. The guard `a read with no open question
answers: a change of an epic file not yet read is not in it` shows such a read still answers (no run, and the epic
file removed since is not seen). Mutations, restored: the new check dropped (red: the first test); the check made
`queue === null` alone, without `asked !== ''` (red: the guard).
`pnpm --filter @collide/arch-plan-tests test` runs 516 tests, green, `contract.test.ts` among them; no shape file is
touched.

### Decisions for the owner

1. Pack-local, as the item recommends. Alternative: the daemon coalesces a burst of fetches into one relist, general to
   every provider but more code in its event path. Recommendation: as built; the daemon fetch still holds the gate, but
   for one file read each.
2. The read keeps no bodies, so the body is read from the task's file on each `get`, and that read is also the check.
   Alternative: keep every body in the snapshot (about 1,600 files, up to 330 KB each). Recommendation: as built.
3. "Own file unchanged" is the sha256 `version` of a read of that file. Alternative: `stat` size and mtime, cheaper,
   but an edit that keeps both would be missed. Recommendation: as built; one file is cheap.
4. A write invalidates the read by a counter (`wrote`), on top of the version check. Alternative: the version check
   alone, which leaves collections from before a write that touched an epic file. Recommendation: as built.
5. `list` is unchanged: it still reads the whole tree each time. The item is about `get`. Recommendation: leave it; a
   `list` is a request for the whole checkout and the daemon asks for it rarely.
6. No `get` is answered from a `stale` read (the last read failed), nor from one made with a question open and no
   queue (`asked !== ''`, `queue === null`), taken at review. Alternative: answer from it. Recommendation: as built;
   it reads as today. Cost: while the plan CLI cannot give a queue, every `get` reads the tree as it did before.
7. Proposed plan text, not applied (0.10, Planning sources, the Orchard source): "A `get` for a checkout the provider
   watches is answered from the watcher's last read when the task's own file is unchanged since that read, and reads
   the tree otherwise. A write by the provider makes every earlier read stale. What is derived from other files
   (collections, spike links) can be older than the file by the watcher's debounce, or its recheck when an event was
   missed; the queue by the time until an open question changes. No `get` is answered from a read made with a question
   open and no queue."

### Not done, by design

- `ts-0138-pn` (the owner's check): not changed in what it checks. If its Orchard run switches a branch that edits many
  task files, one `./plan status` run for the watcher's read, and none for the fetches, is now the expected count.
- A question's queue place in a read that has a queue can still differ from a fresh `get` until a question changes
  (above). Running `./plan status` on a timer is not done here: it would run the CLI again on every read, which is the
  cost this item removes. A failed queue is not retried by the watcher either (as before); only a `get` retries it.
  No item filed.
- The fetch still runs under the board's gate (as the item's Notes say it predates this item).

### At review

- Review found that a read made with a question open and no queue (the plan CLI's `status` failed) kept `queue: null`
  until the open questions changed, so a `get` of a queued question from that read lost `plan:queue` with no time bound,
  where it used to ask the CLI again. Fixed by the builder on resume: `held` gives no such read, and a test with a
  `status` that fails once.
- Reviewer mutations: `wrote` only after an `ok` write, and `held` ignoring the write count, went red. A `get` for one
  watched checkout answered from the main checkout's read survived: no test watched two. Added `of two watched
  checkouts, a get that names one is answered from that one read` (the other checkout lacks the epic file); red under
  that mutation. `wrote` without its re-read survived: speed only, since the write's own file events start a read, and a
  refused write changes nothing.
- Decisions 1 to 6 taken as recommended (owner delegation). Decision 7's text applied, shortened, as a 4c bullet after
  the fake plan CLI's.
