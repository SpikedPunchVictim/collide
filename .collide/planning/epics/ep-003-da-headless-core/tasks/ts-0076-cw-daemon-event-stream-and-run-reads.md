---
id: ts-0076-cw
kind: task
status: ready
title: "Daemon event stream and run reads for the window"
owners: []
links:
  - { type: blocked-by, to: ts-0075-hj }
---

## Goal

The daemon serves what a window needs to follow runs live and to find them again after it reopens
(`host/src/daemon.rs`, `host/src/store.rs`, `host/src/phases.rs`):

- `daemon.subscribe` (`core/src/owner/daemon.rs` `DaemonSubscribeParams`): one `daemon.event` notice for each event
  after `since`, in order and with none missing, then the live ones, on that connection only;
- the `queue.changed` notice (`core/src/owner/queue.rs` `QueueChanged`) to each subscribed connection;
- `run.list` (by board, by task, and by scope `live`, `finished` or `all`) and `run.get`;
- an indexed read of a run's latest state, in place of the scan in `Phases::recorded_run_state`.

Carried from 3b: `ts-0069-bn` found that `recorded_run_state` reads every event in the store on each call, and
`collide run`'s poll reaches it near a run's end. It wants an indexed read (Stage 3c).

## Success criteria

- A subscriber that gives `since` gets every later event once, in `seq` order, across the switch from stored
  events to live ones, with none missing and none twice.
- With no `since`, it gets only the live events. Subscribing again replaces the earlier subscription. A closed
  connection ends it.
- A subscriber that stops reading never blocks the daemon or another connection. Its subscription ends, and it can
  subscribe again from the last `seq` it has.
- A Queue item added or answered sends `queue.changed` to each subscriber.
- `run.list` and `run.get` answer from the store, and give the same answers after a daemon restart.
- The run-state read no longer reads the whole event log. Its answers match the old scan for every recorded run.

## Tests

Write these first:

- replay from `since`, with events appended while the replay runs;
- no `since`; subscribe twice; close the connection;
- a subscriber that reads nothing while many events are appended;
- `queue.changed` on a permission request and on its answer, with fake Claude (in `host/tests/adapters/claude/`);
- `run.list` by board, by task and by scope, and `run.get`, before and after a daemon restart;
- the indexed read against the old scan, over a store with many runs.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 3c: closing the window and reopening it shows the same run (the daemon owns the run).
  - 0.2: The wire; The UI speaks the owner's protocol.
  - Stage 9a: Reattach after the app restarts (finished there).
- **Decision for the item, with a recommendation: the index.** Recommend a `runs` table that holds each run's board,
  task keys, latest state and times. It is written in the same transaction as the `run.transition` event, so the two
  never disagree. `run.list`, `run.get` and the run-state read all use it. The alternative, a run column and an index
  on `events`, still needs a sort to find the latest state.
- **Decision for the item: the switch from stored to live events.** Recommend that the subscription registers for
  live events first, then replays from the store, and drops any live event whose `seq` it has already sent.
- **Decision for the item: a slow subscriber.** Recommend a bounded queue for each connection. When it fills, the
  daemon ends that subscription and logs it. The client subscribes again from its last `seq`.
- Runs started by `run.start_file` have no board. `run.list` with no `board` lists them too.
- Today a connection answers one request at a time on one thread. Notices need writes from another thread, so writes
  to a connection go through one lock.
- The store's migrations are append-only. Run `pnpm gen` if any wire type changes.
- Area: `host` (`daemon.rs`, `store.rs`, `phases.rs`).
- The test rule: no test touches the real home, real launchd or a real daemon. Each test clears the environment
  (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as `host/tests/common/mod.rs` `TempRoot`
  does. Settings files are fixtures, never the owner's own `settings.json`.
- Stage 3c item (revision 22 scope, split 2026-11-15). Items of Stage 3c run one at a time, in id order, because
  they share `host`, the app and the generated files.
