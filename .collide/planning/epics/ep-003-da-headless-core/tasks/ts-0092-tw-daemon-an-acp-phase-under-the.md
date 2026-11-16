---
id: ts-0092-tw
kind: task
status: ready
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
