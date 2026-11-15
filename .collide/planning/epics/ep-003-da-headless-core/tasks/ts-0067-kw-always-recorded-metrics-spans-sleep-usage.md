---
id: ts-0067-kw
kind: task
status: active
title: "Always-recorded metrics: spans, sleep, usage and fingerprint"
owners: []
links:
  - { type: blocked-by, to: ts-0066-zb }
---

## Goal

The always-recorded metrics of 0.13 for a run, kept in the store by the daemon:

- **Spans** for runs, phases, turns and waits, with active time kept apart from waiting time. Waits are permission waits, usage-limit pauses and machine sleep.
- **The sampler's tick** (`host/src/sampler.rs`, `Sampler::tick`), carried from `ts-0051-wh`. It reads a `ClockPair` each second. A gap over `SLEEP_THRESHOLD` is recorded as a sleep interval and counted as waiting.
- **Tokens and cost** per turn and per phase, from `ts-0064-hw`'s usage events. Input, output, cache reads and cache writes are kept, plus compaction calls and other model calls.
- **Time per turn.** Wall time, and model time.
- **The token and context metrics** of 0.13: context size, compactions, cold returns and tool-result bytes.
- **The count of `Other` events** per run.
- **At phase end**, the shim's exit totals and the disk space of each workspace.
- **The always-on fingerprint.** Its user-config part comes from `ts-0061-bg`, and `ts-0068-wx` adds the `claude` version.

Replaces the stubs tagged `sampler: tick and sleep`.

## Success criteria

- A run's trace shows its phase and permission-wait spans, with the wait counted as waiting. Its tokens and cost match `result.usage` per turn plus its compaction calls, with no content block counted twice (Stage 3b criterion; S9).
- A recorded limit message pauses the phase with its reset time, and the phase resumes by session id. The pause counts as waiting (Stage 3b criterion).
- A sleep injected through fake clocks under a running phase is recorded as a wait, not as active time (0.2; carried from 3a).
- A value the agent did not report is stored as null, never 0. A total built from partial figures is marked as "at least" and never summed with exact ones (0.13).
- **Store appends.** The item measures the cost of one store connection per call at the fake-Claude event rate, then keeps or changes it, and records the numbers (follow-up from `ts-0053-gn` and `ts-0056-ag`).
- No stub tagged `sampler: tick and sleep` remains. The tag moves to `FILLED_TAGS`.

## Tests

Write these first:

- span-tree tests against fake-Claude streams: run, phase, turns, a permission wait, a limit pause and a sleep interval;
- missing values stored as null;
- the sampler tick over fake clocks: no sleep, a gap under 2 s, and a gap over 2 s;
- token sums against `result.usage` plus compactions.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 3b: Always-recorded metrics.
  - 0.13: spans; active and waiting time; Sleep is not work; tokens and cost; model time; the context metrics; the fingerprint; the detectors.
  - 0.2: the clock table.
  - S9 record: usage counting. S8 record: the sleep measurement.
- The owner's real sleep test is `ts-0071-fd`. This item gives it what it reads: the sleep spans, and active time with sleep taken out.
- Area: `host` (`sampler.rs`, the store's migrations for spans and usage, `daemon.rs`).
- The test rule: no test touches the real home, real launchd or a real daemon. Each test clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as `host/tests/common/mod.rs` `TempRoot` does. Settings files are fixtures, never the owner's own `settings.json`.
- Stage 3b item (revision 18 scope, split 2026-11-15). Items of Stage 3b run one at a time, in id order, because they share `host` and the generated files.

### Built

- `host/src/metrics_store.rs` (new): the SQL layer of the metrics tables, as methods on `Store` (one connection per call, like the rest of the store). Spans (`open_span`, `add_closed_span`, `close_span`, `spans`, `open_spans`), usage rows, tool calls, hook calls, findings, compactions, `Other` counts per run, phase ends, cold returns and fingerprint parts. `record_agent_events` writes what the whitelist keeps of a batch of agent events in one transaction: figures, names, ids and byte counts, never a message, a tool input, hook output, report text or an `Other` body. A figure an event lacks is stored as NULL.
- `host/src/store.rs`: migration 4, "metrics" (the tables above). `connect` and `sql_error` are `pub(crate)`.
- `host/src/metrics.rs` (new): `Metrics`, the recorder. It holds no state: an open span is looked up in the store, so a restarted daemon carries on. It opens and closes the run, phase, turn and wait spans, records sleep intervals, cold returns, phase exits and workspace size, and the fingerprint part. `span_times` gives a span's elapsed time and the union of the waits inside it, so active time is elapsed less waiting. `usage_totals` sums a phase or a run. A failure to record is logged and never fails a run.
- `host/src/sampler.rs`: `Sampler::tick` is implemented. A gap over `SLEEP_THRESHOLD` (2 s) between the uptime and the continuous clock is a sleep interval and is recorded as a wait. `host/src/clock.rs`: `ClockPair::continuous_nanos`.
- `host/src/daemon.rs`: `sampler_tick` (option), the sampler loop, `Metrics` wired into phases and file runs, and `Daemon::store()` for a reader.
- `host/src/agent_feed.rs`: `take_events` drains the events the feed took, once each. Events a restart replays through `rebuild` are not drained, so they are not stored twice.
- `host/src/phases.rs`: the permission request, answer and pause/resume hooks open and close wait spans; each poll passes the drained events to `Metrics`; a phase's exit and the workspace's size are stored at its end.
- `host/src/file_run.rs`: the run span, and `record_fingerprint` (the user-config part, written when the run starts).
- `host/src/adapters/claude/user_config.rs`: `user_config_fingerprint` (hook names, events and flags; flagged `env` names; the endpoint's host). `host/src/adapters/claude/session.rs`: `read_compact_boundary`.
- `core/src/agent.rs`: `AgentEvent::Compaction` and `CompactionTrigger`; `KNOWN_EVENT_TYPES` is 15. Schemas and the pack SDK's generated types were regenerated (`pnpm gen`; the second run changes nothing).
- `checks/tests/real_tree.rs`: `FILLED_TAGS` is 34 (17 executor tags).
- Tests added, 86 in all: `host/src/metrics.rs` 17 (unit); `host/tests/metrics_store.rs` 20; `host/tests/metrics.rs` 21; `host/tests/adapters/claude/metrics.rs` 11 (through a real daemon with fake Claude); `host/src/sampler.rs` 7; `host/src/agent_feed.rs` 2; `host/tests/adapters/claude/usage.rs` 4 (compaction); `host/tests/adapters/claude/user_config.rs` 3 (fingerprint); `core/tests/agent_events.rs` 1. `pause.rs` gained `advance` on the test clocks and a `sampler_tick` setting; its helpers are `pub(crate)` so `metrics.rs` reuses them.
- Checks run: `cargo test --workspace` twice (2180 passed each), clippy with `-D warnings`, `cargo fmt --all -- --check`, `pnpm gen` twice (no diff the second time), `pnpm typecheck`, `pnpm test`, `pnpm check:names`, `pnpm check:stubs`, `pnpm check:deps`. `collide tasks --json | grep -c task.invalid` is 0.

- A span open across a machine reboot (the continuous clock restarts, so it closes at a reading below its open): `span_times` gives `None` for its length, waiting time and active time ("not reported"), not zero. A wait of unknown length makes the waiting and active time of the spans it counts for `None` too. `time_totals` sums by span kind and counts such spans in `missing`, like the token sums. `Metrics::end_wait` records no cold return in that state. Tests use fake clocks: a phase opened, the continuous clock set lower, the phase closed: its stored length is null and a total reports one span lacking the value; a wait ended after a reboot records no cold return.

- `record_agent_events` runs its transaction as `Immediate`. A deferred one that read before it wrote (a finding reads its turn's cost) was refused at once with "database is locked" when another writer committed in between, so under load a whole batch of events was lost. `metrics_store.rs` test `batches_written_at_the_same_time_are_all_kept` fails without it. `set_workspace_bytes` returns the rows it updated and `Metrics` logs when it is 0. The workspace's size is written by the run's end hook just after the phase-end row, so a reader waits for `workspace_bytes` to be set; a failed measurement is logged by the run ("was not measured").

**The span tree.** run, then phase (child of run), then turn (child of phase); waits are spans of kind `wait` with a cause: `permission` (from the Queue item's push to its answer; carries the item id), `usage_limit`, `budget`, `disk`, `sleep`. Spans are measured on the continuous clock (ns); the wall clock (ms) is kept for display. Active time is a span's length less the union of its waits (the same phase's, or a wait with no phase), so a permission wait that the machine slept through counts once. A run's waiting time is the union of all its waits.

**Token sums.** `usage_totals` sums `Turn`, `Compaction` and `OtherCalls` rows and never a `Step` or a `RunningTotal` (the same tokens counted another way). `partial` rows are summed apart from `exact` ones, and a total with a partial part is "at least". A figure a row lacks is not a zero: the sum counts the rows that lacked it.

**Hooks and the detectors.** Stored: hook name, event, outcome, exit code and decision; refusals; `HookFailedOpen`; the detector findings (name, event, tool call id, cause) with the cost of the turn they came in (`cost_basis` is `turn`). Not stored: any hook stdout or stderr, tool result text or tool input. ts-0063's check is now a real test over the daemon's stored rows: the marker in `hook-deny-json` is absent from the raw bytes of `store.db`, `-wal` and `-shm` and from every table.

**Store-append measurement** (follow-up 6; this machine, debug build, 2000 appends into a fresh WAL store, three runs):

| path | per event |
|---|---|
| `Phases::append_event` as built: `last_seq` then `append_event`, a connection each | 1.26 to 1.32 ms |
| the same two statements on one kept connection | 0.06 to 0.08 ms |
| a span open and close (a connection each) | 1.55 to 1.78 ms per pair |

The fake Claude `report` run is about a dozen events in one burst, so about 16 ms of appends for the whole phase. The metrics rows are written once per feed poll in one transaction, so they do not scale per event. Kept: one connection per call.

### Decisions for the owner

1. Compaction is a typed core event (`AgentEvent::Compaction`: trigger, tokens before, tokens after, duration, mid-task), read from `compact_boundary`. Alternative: read the numbers from `Other.raw` in the store layer. Recommend the typed event: `Other.raw` is adapter text, and the store must not parse it. A value the line lacks is null. Tokens after is the next step's `context_used`. Mid-task means a tool call was open at the boundary. The token field is `context_before`, because core's `ask_protocol` test forbids "token" in a notice's field names.
2. `FailCause` is left as it is. Widening it changes the `phase.transition` wire format and the app's types, and nothing reads it yet. The cause stays as text in `phase_records.failure`, and the turn span's outcome carries the status. Alternative: widen it with the Stage 4 engine, which will read it. Recommend that.
3. Store-append: keep one connection per call (numbers above). Alternative: keep one connection for the event log and pass the next `seq` from the lock `Phases::append_event` already holds, which drops the `last_seq` connection (about half the cost, with no new shared state). Recommend keeping the current code until a real stream's event rate is measured by ts-0070-mk; if it passes about 200 events a second, make the change.
4. Cold return: a wait over the cache's 1 h life. The cause is `usage_limit` for a limit pause and `queue_wait` for a permission, budget or disk wait. A sleep alone is not a cold return. Alternative: count a sleep over an hour too. Recommend as built: the owner's own sleep shows in the sleep spans.
5. Spans are recorded when the daemon polls the feed (about 50 ms), not when Claude wrote the line. A turn's length is that coarse. Model time per turn is the stream's own `duration_api_ms`, kept apart. Alternative: stamp events in the shim. Recommend as built.
6. Events replayed after a restart are not stored again. A turn that a restart cut in half has its span closed by the next daemon at the time it saw the end.
7. Totals are in USD only: a row in another currency is counted as a row without a cost, not converted.
8. The missing-tool detector is stored with the other findings; its cost is that of its turn, and the finding says so (`cost_basis` `turn`).
9. Usage is a row per event (not per model). Per-model usage is not split.
10. `phase_ends` has a row when a phase pauses and when it reaches a final state, with the shim's exit totals when it has them (null otherwise) and the workspace's bytes.
11. The sampler tests' injected sleeps in the existing pause tests also show as sleep waits in the store. This is harmless: the tests do not read them.

Proposed plan text:

- 0.13, Spans: "A span is measured on the continuous clock. Active time is a span's length less the union of the waits inside it; a wait is a permission, a usage-limit, budget or disk pause, or a machine sleep. Spans are in the store and survive a daemon restart."
- 0.13, Tokens: "A total sums Turn, Compaction and OtherCalls usage, never a Step or a RunningTotal. A partial figure is 'at least' and is never summed with an exact one. A figure the stream lacks is null."
- 0.3, agent events: add `compaction` (trigger, tokens before and after, duration, mid-task; each null when the line lacks it).

- Owner decisions (2026-11-15): the recommendations above are accepted as built, with one change in review: a span closed across a machine reboot stores its length, active and waiting time as null, counted as missing in totals, and records no cold return (0.13, Missing is not zero). A flaky gate run then found a real gap: the metrics batch used a deferred transaction that read before it wrote, so a concurrent writer made SQLite refuse it at once and the batch was lost; it is now an immediate transaction, with a test of concurrent batches. Plan text applied to 0.3 (the `Compaction` event) and 0.13 (spans on the continuous clock, active time less the union of waits; totals sum turn, compaction and other calls). Follow-ups: `ts-0070-mk` measures a real stream's event rate, and if it passes about 200 events a second the append takes its `seq` from the lock it already holds (decision 3); `FailCause` widens with the Stage 4 engine, which reads it (decision 2); the cold-return causes owner restart and take-over come with the stages that add them.

### Not done, by design

- Cold-return causes "owner restart" and "take-over": there is no owner restart or take-over to see yet.
- The fingerprint's other fields (line ref, packs, profile, node version, tools): the user-config part only. `ts-0068-wx` adds the claude version.
- Per-tool-call timing and per-model usage.
- Resource sampling over time (only the shim's exit totals and the workspace's size are kept).
- A reboot as a cold-return cause: a span open across a reboot has no length (see Built), and a wait that ends after one records no cold return. "Owner restart" is a later cause.
- `FailCause` is not widened (decision 2).
