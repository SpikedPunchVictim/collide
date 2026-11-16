---
id: ts-0076-cw
kind: task
status: active
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

### Built

The daemon serves `daemon.subscribe`, `run.list` and `run.get`, sends `queue.changed`, and reads a run's latest state
from an index. Nothing in `core` changed, so `pnpm gen` had nothing to regenerate.

- `host/src/event_hub.rs` (new): `Hub`. Each subscriber has a bounded queue (`try_send`), so a publisher never waits.
  `register` cancels the connection's earlier subscription. A full queue ends its subscription, logged: the sender is
  dropped, and the entry stays until `remove`, so a later `register` still cancels it. Subscription flags: `cancelled`
  and `overflowed`.
- `host/src/daemon.rs`:
  - `Conn`: every write to a connection goes through one `Mutex<UnixStream>`.
  - `subscribe`: registers first, then reads the head, `oldest_seq` and the replay. The answer is written first, then
    `start_pump` starts a thread that writes the replay and then the queue. A live event at or below the head is
    dropped by its `seq`.
  - The pump reads `cancelled` with the writer lock held, so a replaced subscription writes nothing after the
    answer that replaced it.
  - A full queue ends the stream: the pump writes what the queue holds, logs, and shuts the socket down for writing. The
    client reads to the end and subscribes again from its last `seq`.
  - `run_list`, `run_get`, `summary_of`. `DaemonOptions.subscriber_queue` (`SUBSCRIBER_QUEUE`, 1024). The module doc
    lists the methods and describes the stream.
- `host/src/phases.rs`: `PhasesConfig.on_event` (`EventSink`), called in `append_event` under the lock that numbers
  events, after the store has the event. `recorded_run_state` is now `Store::run_state`. `run_phases(run)`.
- `host/src/queue.rs`: `on_change` (`QueueSink`). A new item is told as added, a taken item as removed, each with the
  size after it. A replaced item and a second `take` tell nothing.
- `host/src/store.rs`: migration 6 (`runs`, the `RUNS_SQL` const): `runs` and `run_tasks`, with a backfill from the
  `run.transition` events (`json_extract`) so a store that has runs gets their rows. `append_event` is one immediate
  transaction: the event, and the run's row when it is a `run.transition`. `oldest_seq`.
- `host/src/run_store.rs` (new): the SQL on `Store`: `run_state`, `run`, `runs` (board, task, scope, `before`,
  `limit`), `set_run_launch` (for `run.start`, ts-0078), and `note_transition`. `is_live` is the one place that says
  which states are final (`ended`, `failed`, `stopped`); `integrating` is live.
- `host/src/run_summary.rs` (new): `summarize`, a pure function from the row, the phases and the phase spans to a
  `RunSummary` and the rounds.
- `host/src/metrics_store.rs`: `phase_spans(run)`, so a list of runs does not read every turn span.
- `host/src/lib.rs`: the three modules.
- `host/tests/daemon_socket.rs`: `a_method_that_another_item_serves_answers_method_not_found` used `run.list`, which is
  served now. It uses `workspace.list`. Nothing else in that test changed.

No task status name is written in the host: the statuses of a run's tasks are carried as given and never read, and
liveness reads the run's state. The tests use opaque names (`status-a`).

**Tests.** `host/tests/common/subscriber.rs` is a raw socket client with a line reader that keeps notices that arrive
before an answer. No test touches the real home, launchd or a daemon: each uses a temp root, and the adapter tests the
`Rig` of `pause.rs` (`env_clear()`, `HOME` in the root).

- `host/tests/event_stream.rs` (9), seen red (all failed with `method not found`):
  `a_subscriber_that_gives_since_gets_each_later_event_once_in_order`, `the_answer_comes_before_the_replay`,
  `events_appended_while_the_replay_runs_come_once_and_in_order_with_none_missing`,
  `with_no_since_only_the_events_after_the_answer_come`, `an_empty_log_says_so_in_its_answer`,
  `subscribing_again_replaces_the_first_subscription`, `events_go_to_the_connection_that_subscribed_and_to_no_other`,
  `a_closed_connection_ends_its_subscription_and_the_daemon_goes_on`,
  `a_subscriber_that_reads_nothing_blocks_no_one_and_ends_and_can_subscribe_again_from_its_last_seq`.
- `host/tests/run_reads.rs` (11): ten seen red against a stub of the store functions. The index-against-scan test over
  300 runs passed against the stub, which scanned, so it checks that the real index gives what the scan gave. Names:
  `run_list_gives_the_runs_newest_first_with_the_file_runs_that_have_no_board`, `run_list_by_board_...`,
  `run_list_by_task_...`, `run_list_by_scope_...`, `run_list_pages_with_a_cursor_...`,
  `run_list_refuses_a_board_the_daemon_has_not_opened_and_a_cursor_it_did_not_give`,
  `a_summary_has_the_tasks_the_state_the_times_and_the_line_and_workflow_the_run_was_launched_with`,
  `run_get_gives_the_summary_of_one_run_and_not_found_for_a_run_it_has_not_heard_of`,
  `run_list_and_run_get_give_the_same_answers_after_the_daemon_restarts`,
  `the_indexed_read_gives_what_the_scan_gave_for_every_run_of_a_store_with_many`,
  `the_indexed_read_does_not_read_the_event_log` (a corrupt unrelated event: the scan fails on it, the index does not).
- `host/tests/adapters/claude/event_stream.rs` (4), with fake Claude, seen red: a permission request and its answer
  send `queue.changed`; each subscriber gets it and an idle connection does not; `run.get` and `run.list` for a file
  run, the same after a restart; a run waiting on a permission request is live, then finished.
- Unit tests, seen red before the code: `event_hub` (7) and `queue` (4 new). Written after the code: `store` (5: the
  backfill from a version-5 store, a refused event moves no row, `oldest_seq`), `run_store` (7) and `run_summary` (5).
  The red step for these was the stub in `run_store.rs` and the missing tables.

Checks: `cargo build --workspace`, `cargo test --workspace` (2437 passed, none failed), `cargo clippy --workspace
--all-targets -- -D warnings`, `cargo fmt --all --check`, `pnpm test`, `pnpm typecheck` and `pnpm check` all pass.

### Decisions for the owner

1. **The index is two tables, `runs` and `run_tasks`**, written in the `run.transition` event's transaction, with a
   backfill in migration 6. Alternative: a column and an index on `events` (still needs a sort for the latest
   state). Recommendation: keep.
2. **`run.start` links the board and tasks.** `Store::set_run_launch(run, RunLaunch)` writes the board, tasks (with the
   status each had), line and workflow after the run's first event. ts-0078 calls it. A file run is never linked, so
   its board is null and its line and workflow are empty text. Recommendation: keep.
3. **A file run's summary.** `line`, `workflow` and `snapshot_commit` are empty text, because the wire fields are
   required. Alternative: make them optional (a wire change, then `pnpm gen`). Recommendation: keep until the window
   draws file runs; then make them optional.
4. **A lagging subscriber's connection is closed for writing**, after what its queue held. The client sees the end of
   the stream and subscribes again from its last `seq`. Alternative: a notice that says "you fell behind", which is a
   new wire type. Recommendation: keep; the end of the stream is already the signal a window handles.
5. **Queue size 1024** notices for each subscriber (`DaemonOptions::subscriber_queue`). A burst of a run's events is
   far under it. Recommendation: keep.
6. **The replay is held in memory** before it is written. A client that subscribes with `since` far back on a large log
   holds it all. Alternative: read in pages of a few thousand. Recommendation: page it when a log is long enough to
   matter; the stored replay and the live queue are already separate, so it is a local change.
7. **A replaced Queue item sends no `queue.changed`.** The set of ids is the same. Recommendation: keep.
8. **`oldest_seq` of an empty log is the head plus one** (so 1). A `since` of 0 is then not a gap.
9. **Paging.** The default page is 50 and the most is 200. A limit of 0 is 1. The cursor is the `seq` of the last run's
   first event, as text, and the next page is the runs opened before it. A cursor that is not a number is
   `INVALID_PARAMS`. A board the daemon has not opened is `NOT_FOUND`. A `task` with no `board` matches any board.
   Recommendation: keep.
10. **A run's phases come from the daemon's table, their times from the phase spans.** Every phase is `agent` with one
    instance (named for the phase, no model or session id). The run's state is the row's, so it is the same after a
    restart. `run.get` has an empty `launch`, `implementations` and `fingerprint`: `run.start` pins them.
    Recommendation: keep; fill them in ts-0078.
11. **A client on macOS cannot set a socket option after the daemon shut the socket down.** The test client ignores that
    error. The shell (ts-0077) reads to the end and reconnects, and should not set options on a closed socket.
12. **Plan text (propose, not applied).** In 0.2, The wire, after the subscribe paragraph: "A subscriber has a bounded
    queue. A client that does not read until it is full has its stream ended after what was queued, and subscribes again
    from the last `seq` it has; a publisher never waits for a client."

- Owner decisions (2026-11-15): decisions 1 to 11 taken under the owner's delegation. Decision 12's plan text applied
  to 0.2, The UI speaks the owner's protocol. Review: the subscribe order (register, then read the head and the
  replay, then drop the queue's copies at or below the head) was read and holds. Some unit tests in `store`,
  `run_store` and `run_summary` were written after the code. Carried: making a file run's `line`, `workflow` and
  `snapshot_commit` optional goes to `ts-0081-wm` if its page draws file runs; the full-log reads in
  `Phases::run_start_us` and `Phases::new` go to Stage 8's engine; paging the replay waits for a log long enough to
  matter. The builder's scratch logs were left in its `mktemp -d` folder: a safety check blocked their removal.

### Not done, by design

- `run.start`, `run.preview` and the rest of `run.*`: ts-0078 onward. Nothing calls `set_run_launch` yet.
- `run.output` and the terminal notices: other items'.
- Linking a run to its tasks for `task.get` (`runs`, `latest_run`): `run.start`'s.
- Paging the replay (decision 6), and a cap on how far back `since` may go.
- `Phases::run_start_us` still reads the whole event log. Only a stop reads it (the survivor scan), not `collide run`'s
  poll; the row's `started_ms` could replace it.
- `Phases::new` still reads the whole log once at start to restore the last phase states.
- The shell's connection to the daemon (ts-0077).
- No change to the item's frontmatter or status, and nothing committed.
