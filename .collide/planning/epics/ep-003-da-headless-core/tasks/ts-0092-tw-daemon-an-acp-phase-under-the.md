---
id: ts-0092-tw
kind: task
status: active
title: "Daemon: an ACP phase under the real shim and phases"
owners: []
links:
  - { type: blocked-by, to: ts-0091-pz }
---

## Goal

The ACP engine runs inside the daemon's own phase machinery, with the real shim and a scripted fake ACP child, and
the daemon survives what Claude's path survives: a restart, a stop, a child that dies. This item closes Stage 3d. It
wires nothing into file runs or the app: which adapter a run uses is a pack and role question for a later stage.

What is missing today, found by reading `host`:

- `Phases` never calls `AgentSession::start`. Claude's `start` writes nothing, so nothing noticed. An ACP session writes
  `initialize` there. `AgentFeed::start` exists and has no caller outside tests.
- The first turn of an ACP phase is the session's own (`LaunchRequest.prompt`), where Claude's is sent by
  `Phases::send_turn` (`ts-0087-mf`, decision on the first prompt). The daemon must not send it twice.
- `FeedRebuilder` builds a session from the journal after a restart. The one in `host/src/file_run.rs` builds a Claude
  session. A test builds its own, as `host/tests/phases.rs` does; the ACP one must rebuild an `AcpSession`.
- Stage 6b's ask box and any later ACP phase need the `initialize` probe (`ts-0088-pk`) to run once, before a role's
  `require` list is checked. The probe's call site is this item's, not the session's.

The work:

- `AgentFeed` gets a builder, `start_on_launch()`, next to `close_after_turn()`. `Phases::start_agent_phase` writes
  `feed.start()`'s bytes to the child, after the shim is up and before any turn, for a feed that was made with it. A
  feed made without it is as it is today, so Claude's events and writes do not change.
- The daemon writes what `start` gave and then reads: the handshake, the model request, the prompt and the turn are
  the session's answers to the child's lines, written as `feed` returns them (this is how `pump` already works).
- A phase's first turn is not sent by `send_turn` for a feed that was made with `start_on_launch()`. A call to
  `send_turn` on such a feed after the turn ended is a second turn, as for Claude.
- A function that runs the probe: given a profile, it starts the agent through the shim, writes `initialize`, reads the
  response, stops the child, and returns the capabilities (`ts-0088-pk` `probe`). It is a daemon-side helper with no
  caller in file runs yet. Stage 6b's ask box calls it.
- A restart over a running ACP phase: the rebuilt feed replays the journal (`in` and `out`), writes nothing to the
  child,
  gives the pending requests (`ts-0089-wv`), and goes on reading.
- `interrupt_then_close` on an ACP feed: `session/cancel`, then the phase ends `Stopped` once the response comes, or
  `Lost` if the child goes first and does not answer.

## Success criteria

- Over the fake ACP child under the real shim and `Phases`: one prompt turn runs from `start_agent_phase` to the end,
  its events are in the store in order, with `SessionBound` first, and the phase ends `Ended` with the usage and the
  report that the recording gave.
- The first turn is written once. A recording that expects exactly one `session/prompt` and then waits for stdin to
  close passes.
- A permission request in the child's stream is put in the queue with the agent's own options; an `answer` with `Allow`
  reaches the child as the `allow_once` option's id and the child's turn goes on.
- A daemon restarted over a phase waiting for permission: the rebuilt feed has the request pending, the answer reaches
  the child, and the turn ends. Restarted mid-handshake, the child's `session/new` response is read after the restart
  and the first prompt is then written once.
- A stop (`interrupt_then_close`) mid-turn writes `session/cancel`, and the phase ends `Stopped` when the response
  comes. A child that exits first with no response ends `Lost`.
- A child that exits 0 with a request pending ends `Lost` and not `Ended` (S7, O7), as a phase in the store.
- An empty turn ends `Failed` with cause `empty_turn` in the store, with the evidence of `ts-0091-pz`.
- A Claude phase's events, writes and tests are unchanged: the whole suite passes with no change to a Claude test.
- The probe against the fake child returns the capabilities a recording implies, and leaves no child behind.
- No test starts the real `claude` or any real ACP agent, and no new crate is added.

## Tests

Write these first:

- `host/tests/acp_phases.rs`, a test target of its own, in the style of `host/tests/phases.rs` (a daemon inside the test
  process, real `collide shim` processes, `python3` children, a cleared environment, a guard that stops what a test
  started, by pid and start time):
  - one turn from start to end, the store's events and the phase's record;
  - the first turn written once;
  - a permission request answered `Allow`, `Deny` and `Cancel`, each reaching the child as the option or outcome the
    session chose;
  - a restart while the request is pending, and a restart in the handshake;
  - a stop mid-turn, a stop with a child that ignores the cancel, a child that exits first;
  - the exit 0 with a request pending;
  - an empty turn;
  - the probe, against each of the handshake recordings (`configOptions`, only `models`, neither);
- `AgentFeed::start_on_launch` as a unit test next to the feed's own, with a Claude session (no writes, no events) and
  with the double from `core::testing::acp` (the handshake bytes);
- every Claude test in `host` as it is.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 3d: ACP engine (permission request, cancel, `empty_turn`, `lost`).
  - 0.2: A phase and the shim (a phase is `lost` when the child went and left no record); The stream protocol.
  - 0.3: Two core traits (`start`, `replay`, `pending_permissions`); Later adapters; The stream protocol.
  - 0.15: Builders are Sonnet agents; every commit passes all tests.
  - Stage 6b: The in-app agent (what will call the probe and run an ACP phase).
  - Deferred: Other agent tools.
- **Decision for the owner, with a recommendation: who calls `start`.** Options:
  1. an opt-in builder on the feed, `start_on_launch()`, as above. Pro: Claude's path is untouched, and a reviewer
     can see
     which feeds start themselves. Con: a caller can forget it for an ACP session, so the ACP adapter's `session`
     should hand back the feed's settings (see the next decision).
  2. `Phases` always calls `start()`. Pro: one rule. Con: Claude's `start` emits `SessionBound` for an assigned session,
     and Claude's stream may emit it again, so Claude's events change and its tests with them.
  Recommend 1.
- **Decision for the item, with a recommendation: how the feed learns it must start.** Recommend a small method on
  `AgentAdapter`'s product, not on the trait: `AcpAdapter::feed(&self, request, dir) -> AgentFeed`, which builds the
  feed with `start_on_launch()` and `close_after_turn()`. The daemon's callers never choose by hand. The trait is
  unchanged, so the shared contract is too.
- **Decision for the owner, with a recommendation: ACP in file runs.** `file_run.rs` hard-wires `ClaudeAdapter` for the
  session, `launch` and `rebuild_feed`. Choosing an adapter by role (the `tool` a role names) is a change to the pack
  loader and the run engine. Recommend leaving it to Stage 6b (the ACP ask profile), where a real use exists. This
  stage proves the engine at the `Phases` level. Pro: a small item, no half-wired path. Con: until 6b, no `collide run`
  can use an ACP agent, which matches the plan (a real ACP agent is Deferred for runs).
- **Decision for the item: the probe.** It starts a child with the profile's argv, which means a real process under the
  shim. The probe's child is stopped with a bounded wait. It must not leave a phase directory behind, so it runs in a
  temp phase name and removes it.
- A restart in the handshake: the journal holds `initialize` out and its response in, and nothing more. The rebuilt
  session is waiting for `session/new`'s response. It must write nothing on replay and must not write `initialize`
  again. Give this test a name of its own.
- The first prompt is held by the session as data (`ts-0087-mf`), so a restart before it is written does not lose it.
- Run `pnpm gen` if a wire type changes. None is expected.
- Area: `host` (`agent_feed.rs`, `phases.rs`, `adapters/acp/mod.rs`, a probe helper), `host/tests/acp_phases.rs`
  (new), `host/Cargo.toml` (one test target if the harness needs it).
- The test rule: no test touches the real home, real launchd or a real daemon. Each test clears the environment
  (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as `host/tests/common/mod.rs` `TempRoot`
  does. Settings files are fixtures, never the owner's own `settings.json`.
- Stage 3d item (revision 23 scope, split 2026-11-15). Items of Stage 3d run one at a time, in id order, because
  they share `host` and the generated files.
- Owner decisions (2026-11-15, at the split): the decisions for the owner above, taken as recommended under the
  owner's delegation; the owner may change them before the item is built. The ACP protocol is written by hand: no ACP
  crate is added.
- Carried from `ts-0087-mf` (review): a journal cut between the `session/new` response and the first prompt leaves a
  rebuilt `AcpSession` that never wrote its prompt. Decide how the daemon, after a restart, makes that session send
  it exactly once, and test the cut there.
- Carried from `ts-0089-wv` (review): the same kind of owed write. A journal cut after a read that the session
  answers on its own (`-32601` for an unserved request, `cancelled` for a request after a cancel or at a turn end)
  and before that answer leaves the agent waiting, since replay writes nothing. Settle every owed write, the prompt's
  included, with one rule, and test each cut.

### Built

An ACP phase now runs under the real shim and the daemon's phases, proven at the `Phases` level. No wire type
changed, so `pnpm gen` had nothing to regenerate. No crate was added. `collide run` and `file_run.rs` still choose
Claude; that waits for Stage 6b.

- `core/src/adapter.rs`, `AgentSession::take_owed_writes` (new, defaulted to none): the writes a rebuilt session owes.
  See the first decision below.
- `host/src/adapters/acp/session.rs`, `AcpSession`: the owed-write rule (below). `initialize` is built in one place,
  used by `start` and by the owed `initialize` of an empty journal.
- `host/src/agent_feed.rs`, `AgentFeed`:
  - `start_on_launch()` (builder, next to `close_after_turn()`) and `starts_on_launch()`.
  - `take_owed()` and `owe_again()`. `rebuild` takes the session's owed writes once.
  - `Pumped.turn_ended`: a turn ended in the read.
- `host/src/adapters/acp/mod.rs`, `AcpAdapter::feed` (the constructor: `start_on_launch()` and `close_after_turn()`) and
  `rebuild_feed` (the same through `AgentFeed::rebuild`). `AgentAdapter` has no change.
- `host/src/phases.rs`, `Phases`:
  - `start_agent_phase` writes the feed's `start` after the shim is up and before any turn, for a feed made with
    `start_on_launch` only. A failed write closes stdin, is logged, and returns the new `PhaseError::NotStarted`.
  - `serve_link` sends the feed's owed writes before its first read, so they go before any `CloseStdin`.
  - A turn's end takes out of the Queue the phase's permission items the session no longer holds (a request open at the
    end is answered `cancelled` by the session), and ends their wait metric.
  - `phase_dir(key)`, and `send_unawaited` (the one place a write goes out without waiting; `pump_feed` uses it too).
- `host/src/acp_probe.rs`, `probe_through_shim` (new module): starts the agent as a phase of a run of its own with a
  unique name, writes `initialize`, reads the response with id 0 from the journal, closes stdin, waits 1.5 s, stops the
  run if the child is still there, and removes the run's folder. The result is `probe(&response)`. No file run calls it.
- `host/src/daemon.rs`: `DaemonOptions::feed_rebuilder` (default none), tried before the file run's own rebuild, so a
  test daemon restarts an ACP phase. `Daemon::queue_items`.
- Test fixtures: a `wait_file` step in `fake_acp.py`, and the recordings `unserved-request`, `handshake-held`,
  `permission-reply-cancel`, `cancel-ignored` and `prompt-turn-held`.

Tests, and which I saw red first:

- `host/tests/acp_phases.rs` (new, 23 tests). The first run was red for 21 of 22 (the stub probe passed the
  never-answers test by erroring); the 23rd, the Queue test, was added after. All pass now. Names:
  - one prompt turn, written once; a second `send_turn` after the turn is a second prompt;
  - Allow, Deny and Cancel each reach the child as the right option or outcome;
  - restart with a permission pending; `a_restart_in_the_handshake_reads_the_answer_that_comes_after_it_and_goes_on`;
    restart at each of five cuts that leave a write owed (`initialize`, `session/new`, the first prompt, the `-32601`
    answer, the turn-end `cancelled` answer), each proving one write of each kind in the journal; a restart with nothing
    owed adds no record;
  - stop mid-turn, stop of a child that ignores the cancel, exit 0 with a request pending (`Lost`), exit 1 in the
    handshake and in the turn (`Lost`); an empty turn (`Failed`, cause `empty_turn`, evidence `stand-in-model:1b`);
    a request open at the turn's end leaves the Queue;
  - the probe against `handshake-config-options`, `handshake-models` and `handshake-bare`, a probe that reads what the
    recording says, and a probe of a child that never answers (returns within its wait, stops the child).
- `host/tests/adapters/acp/replay.rs` and `permission.rs`: eight owed-write tests at every cut of a journal (seven red
  first), and two `permission.rs` tests changed to take the owed writes and compare them with the live journal.
- `host/src/agent_feed.rs`: six unit tests, with a Claude session and with the core double (red first: the methods did
  not exist). `fake_acp.rs`: two `wait_file` tests (red first).

### Decisions for the owner

1. **The owed-write rule.** A replay keeps every write a replayed line made as owed. A later `in` record removes it (a
   request by its method, a reply by the id it answers). A journal with no request at all owes `initialize`.
   `take_owed_writes` sends what is owed through the same path as a live write, so a second call returns nothing and
   the session equals one rebuilt from a journal that holds the writes. The rebuilt feed takes them once; the watcher
   sends them before it reads. A live session owes nothing. One rule covers the first prompt, `-32601`, `cancelled`, a
   model request and `initialize`. Claude's replay is unchanged (its session owes nothing). Alternative: carry the
   writes in the replay `Step`, which `replay_records` drops by design. Recommend the built rule. Plan text, if wanted
   (0.3, Two core traits): "A session rebuilt from the journal reports the writes the journal does not hold; the
   daemon writes them once before it reads."
2. **A defaulted method on `AgentSession`.** The item said no trait change; I took that as `AgentAdapter`. The method
   is additive and defaulted, so Claude and the core double need no change. Alternative: a trait of its own that only
   ACP implements, which the feed would have to find. Recommend the defaulted method.
3. **A stop wins over a child that goes first.** `settle_with_exit` makes a stopped phase `Stopped` whatever the child
   did. "`Lost` if the child goes first" is tested without a stop request.
4. **A failed start write.** The shim is up, so the phase would wait for an agent that waits for `initialize`. The
   daemon closes stdin, logs, and returns `PhaseError::NotStarted`; the phase then settles from the exit. Alternative:
   stop the run. Recommend closing stdin, the lighter step.
5. **The probe leaves its phase in the table and its events in the store.** Only the folder goes. The phase is final
   (`live()` is zero). A later item may want a `Phases::forget`. Recommend waiting until a file run calls the probe.
6. **A second turn with no response ends as the first did.** `AcpSession::finish` judges by the last turn it saw end,
   so a phase whose second prompt got no answer ends `Ended` when the child exits 0. `ts-0089-wv` and `ts-0090-jd`
   built that; I did not change it.

- Owner decisions (2026-11-16, at the build): decisions taken under the owner's delegation, as recommended. Decision
  1's `take_owed_writes` is a defaulted method on `AgentSession`; the split's "no trait change" was about starting a
  session (`start_on_launch` on the feed), and Claude's session is unchanged by the default. Dropping stale
  permission items at a turn's end applies to every adapter, and takes only items the session no longer holds. The
  open window (a write sent by the old daemon that the shim has not yet journaled when the new daemon replays; the
  shim serves each connection on its own thread) is carried to Stage 8, with the real-kill tests.

### Not done, by design

- No wiring into `collide run`, `file_run.rs` or an adapter choice: Stage 6b.
- No real ACP profile and no real agent.
- The daemon-level cut for a request read after a cancel, and for a model request, is covered at session level only
  (`replay.rs`); the daemon tests cut at the five writes above.
- One window is not closed and not tested: a write the old daemon sent to the shim that the shim had not yet journaled
  when the new daemon replayed would be written again. `Shim::write_to_child` journals under the input lock, so the
  window is the time between the socket send and that lock.
- `IMPLEMENTATION_PLAN.md` is untouched.
