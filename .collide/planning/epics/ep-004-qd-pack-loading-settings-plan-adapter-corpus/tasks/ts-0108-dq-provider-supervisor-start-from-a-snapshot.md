---
id: ts-0108-dq
kind: task
status: ready
title: "Provider supervisor: start from a snapshot, calls, restart with backoff"
owners: []
links:
  - { type: blocked-by, to: ts-0107-rj }
---

## Goal

Make the provider supervisor real, so a provider is a long-lived child that the daemon starts, calls, hears from and
restarts, and so a provider that hangs or crashes never stalls the daemon.

What exists. `host/src/provider_client.rs` `ProviderClient` starts one child and does `initialize`, `list_all`, `get`
and `apply`, one request at a time, with no timeout (a hung provider hangs the caller), no `watch` or `preview` call,
and it skips notifications. `host/src/supervisor.rs` is an outline: `BackoffPolicy::delay` and every method of
`SupervisedProvider` are tagged `todo!`. `host/src/builtin_packs.rs` builds a launch for a built-in pack
(`BuiltinPacks::provider`), and `ProviderLaunch::with_pack_data` sets `COLLIDE_PACK_DATA` (`ts-0102-vu`).
`host/src/board_load.rs` `load_line` returns each pack as a `LoadedPack` (`dir`, `entry`, `manifest`, `dev`), and
`host/src/pack_cache.rs` makes the materialized copy that holds the SDK link. Nothing starts a provider from a loaded
pack.

What this adds.
- A launch for a loaded pack's provider: `node <the pack's folder in the snapshot>/<script>`, the `.ts` script run as it
  is (types stripped, 0.6), refused when the script is not a plain path inside the pack. A `dev:` pack runs from its
  folder. Built-in packs keep `BuiltinPacks::provider`.
- `ProviderClient` gains `watch` and `preview`, a timeout on every call, and the notifications as typed events on a
  channel, read by one thread per child, so none is lost while the caller is busy and none blocks a reply.
- `SupervisedProvider`: `initialize` starts the child, sends the pack's settings and the board's `root`, and checks the
  answer with `core::provider::check_initialize`; the other methods send one request and, when the child is gone,
  start a new one after `BackoffPolicy::delay` and ask again; `Gone` after too many crashes; a stop that closes the
  child's input, waits, then kills.
- `BackoffPolicy::delay`: the pure rule.

## Success criteria

- `initialize` receives the pack's settings (`config`) and `root`, and a result that fails `check_initialize` is
  refused, naming the provider and the reason.
- `list`, `get`, `apply` and `preview` reach the running child. A provider with no `preview` answers
  `ProviderError::Unsupported`. A JSON-RPC error is `ProviderError::Failed` with the provider's message.
- After `watch`, the six notifications (`task.changed`, `task.renamed`, `task.removed`, `task.invalid`,
  `task.collision`, `source.stale`) arrive as typed events, in order, even while a request is waiting for its reply.
- A child that exits between calls is started again after the backoff and the next call succeeds. The wait doubles from
  1 s to 60 s, a successful call resets it, and after 5 crashes in a row the supervisor stops and answers `Gone`.
  A person's explicit retry starts it once more and resets the count.
- A child that does not answer fails the call at the timeout, naming the provider and the method. The child is killed
  and the next call starts a new one. The caller's thread is never left waiting.
- The child's environment is cleared except for the captured base environment and `COLLIDE_PACK_DATA`. Its folder is
  the pack's own, never a workspace.
- A repo pack's provider runs from the snapshot: edit its script in the working copy and the committed one still runs.
  A `dev:` pack runs the folder.
- The tags `supervisor: provider start`, `supervisor: restart with backoff` and `supervisor: provider calls` count zero
  stubs and move to `FILLED_TAGS` in `checks/tests/real_tree.rs`. `supervisor: observers` stays (Stage 12).

## Tests

Write these first.

- `BackoffPolicy::delay`: the table for crashes 1 to 5 and the cap; `None` at `max_crashes`.
- Protocol contract tests (the stage's first test) against a fake provider, a real child process: a script with modes
  (normal, exit after N requests, hang on a method, malformed line, bad `initialize` result, notifications after
  `watch`, noise on stderr). The sequence `initialize`, `list`, `get`, `apply`, `preview`, `watch`, each outcome of
  `apply` (`ok`, `conflict`, `refused`), and `Unsupported` for a method the provider lacks.
- Restart: a child that exits after one request, with a fake clock so no test sleeps: delays 1 s and 2 s are asked of
  the clock, the call after each succeeds, the fifth crash gives `Gone`, an explicit retry works.
- Timeout: a hang on `list` fails at the test's short timeout and the next call starts a new child.
- Notifications: an event sent while a `list` is in flight is delivered and the `list` still answers.
- Launch: a fixture repo whose committed `providers/` script differs from its working copy runs the committed one; a
  `dev:` pack runs the folder; `../x.ts` and an absolute script are refused.
- The environment of the child is the cleared one: a fake provider that echoes its environment names.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 4b: first bullet (protocol v0 with settings passed to `initialize`; a supervisor that restarts providers with
    backoff, isolated from the process owner); Tests: "Protocol contract tests with a fake provider".
  - 0.4: Observers (the supervisor that restarts providers restarts observers too; Stage 12). 0.10: Protocol.
  - 0.6: Built-in packs are not materialized; the cache entry and the SDK link. 0.2: the captured environment.
  - Stub tags tagged 4b: `supervisor: provider start`, `supervisor: restart with backoff`, `supervisor: provider calls`.
- Carried from `ts-0020-tt`: no provider timeout until the supervisor exists. Carried from `ts-0102-vu`:
  `COLLIDE_PACK_DATA` on a provider's launch comes from `pack_data_dir`, built-in packs first.
- **Decision for the item, with a recommendation: the numbers.** The plan fixes none ("Stage 4b confirms them").
  Recommend `initialize` 30 s, `list` 60 s (the plan CLI's JSON alone takes about 1 s for 1,567 tasks, and a provider
  parses files itself), `get` 10 s, `apply` and `preview` 120 s (a provider lock waits up to 15 s, 0.10), `watch` 10 s.
  They sit in one `SupervisorPolicy` next to `BackoffPolicy`. Alternative: one timeout for every call. The backoff
  values stay as the outline has them (1 s, 60 s, 5 crashes).
- **Decision for the item, with a recommendation: what a crash is.** A child that exits or closes its output, or a
  timeout, before a call has its reply. A child that exits while idle counts when the next call finds it. Recommend
  counting in a row since the last successful call. Alternative: a window of time.
- **Decision for the item, with a recommendation: threads.** One reader thread for each child and the call on the
  caller's thread with `recv_timeout`; no async runtime. Alternative: a supervisor thread that owns the child and
  takes calls on a channel. Recommend the first: it is the smaller change from `ProviderClient`.
- **Decision for the item: the fake provider.** A small script under `host/tests/fixtures/`, the way `fake_acp.py` is,
  because it must crash and hang on demand and needs no Node build. The Node side of the protocol is tested by the SDK
  and by `ts-0110-ra`.
- Out of scope: `supervise_observer` (Stage 12), the corpus, the daemon's methods (`ts-0113-ck`), the plan adapter.
- Area: `host/src/supervisor.rs`, `host/src/provider_client.rs`, `host/src/builtin_packs.rs` (or a new launch module
  beside it), `host/tests/`, `checks/tests/real_tree.rs`.
- The test rule: no test touches the real home, real launchd or a real daemon. Each test clears the environment
  (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as `host/tests/common/mod.rs` `TempRoot`
  does. Settings files are fixtures, never the owner's own `settings.json`.
- Stage 4b item (revision 23 scope, split 2026-11-17). Items of Stage 4b run one at a time, in id order, because
  they share `core`, `host` and the generated files.
- Nothing here runs a real `claude`.
- Owner decisions (2026-11-17, at the split): the recommendations above, on delegation (timeouts and backoff).

### Built

- `core/src/provider.rs`: `ProviderEvent` and `ProviderEvent::from_notification`, the six notifications as typed events.
  A method it does not know, or params that do not read, become `Unreadable { method, detail }` and never an error.
  No schema derive, so no `pnpm gen`.
- `host/src/provider_client.rs` rewritten: one reader thread for each child, replies to the caller's thread with
  `recv_timeout`, notifications to an events channel. `CallTimeouts`, `ClientOptions`, `ClientError` (with
  `ends_child`, `may_have_run`, `is_method_not_found`). A hung child is killed at the timeout. `start`, `list_all`,
  `get` and `initialize` keep their signatures, so `task_cli.rs` is unchanged.
- `host/src/supervisor.rs`: `SupervisedProvider` (a `TaskProvider`), `SupervisorPolicy`, `BackoffPolicy::delay`,
  `Pause` (injected, so tests make no real wait), `retry`, `stop`, `take_events`, `crashes_in_a_row`, `last_failure`.
  `supervise_observer` stays a Stage 12 stub.
- `host/src/provider_launch.rs`: `provider_launch(context, pack, provider)`: the script in the line's snapshot copy
  (the folder itself for a `dev:` pack), a plain path inside the pack only, `node` from the captured PATH; a built-in
  pack goes to `BuiltinPacks::provider`. `ProviderLaunch` gained `env` (`with_env`, cleared environment) in
  `builtin_packs.rs`; the `node` lookup and its message are shared with the built-in loader, text unchanged.
- Fixtures: `host/tests/fixtures/fake_provider.py` (modes in its header), `host/tests/common/fake_provider.rs`.
- Tests: `core/tests/provider_protocol.rs` (3), `host/tests/provider_supervisor.rs` (32),
  `host/tests/provider_launch.rs` (8), `supervisor::tests` unit tests (7 on backoff and the policy defaults).
  Tags `supervisor: provider start`, `restart with backoff` and `provider calls` moved to `FILLED_TAGS`.
- Runtime red: the three core tests failed against the `todo!` stub, and so did all 32 supervisor tests and the 8 launch
  tests against a skeleton that compiled (panic at the `todo!`, not a compile error); the 5 backoff unit tests likewise.
- Mutation checks, each seen red then restored: an `apply` asked again (`an_apply_that_was_in_flight...`); no crash
  reset on success (2 tests); a timed-out call asked again (2 tests); no re-`watch` on the new child (1 test);
  no kill at the timeout (found surviving, so `a_child_that_does_not_answer...` got a 60 s grace and now fails
  without the kill).
- Checks: build, test (4038 passed, 0 failed), clippy `-D warnings`, fmt, `pnpm test`, `pnpm typecheck`, app build,
  `build:packs` and `pnpm check` all pass.

### Decisions for the owner

1. An `apply` that was in flight when the child ended is not asked again: it answers `Failed` saying the write may have
   happened. The item said "ask again". A second `apply` of a `create` could write twice. Recommend keeping this.
   An `apply` that never reached the child (it was already gone) is asked again; reads (`list`, `get`, `watch`,
   `preview`) are asked again either way. Alternative: ask again and rely on the provider's own idempotency.
2. A timeout fails the call and kills the child; the next call starts a new one (after the backoff). It is not asked
   again, since a call that took 60 s would take 60 s again. Recommend keeping this.
3. `Gone` after 5 crashes in a row stays until `retry()` (or `initialize`), which resets the count. Nothing retries by
   itself. Recommend keeping this until the daemon method (ts-0109) says how a person retries.
4. `preview` and `watch` answer `Unsupported` without asking the child when `initialize` did not declare them, and a
   "method not found" answer is `Unsupported` too.
5. A failed start counts as a crash, so a pack whose `initialize` is refused backs off like a crash and ends in `Gone`.
6. `ProviderLaunch.env` is `Option`: `None` keeps today's inherited environment (the CLI), `Some` clears it. The
   supervisor's caller must give the captured base environment; ts-0109 owns that call.
7. Plan text to propose (not applied): in 0.10, "a provider that does not answer within the method's timeout is
   killed and the call fails; the timeouts are 30, 60, 10, 120, 120 and 10 s for initialize, list, get, apply, preview
   and watch; restarts wait 1 s doubling to 60 s and stop after 5 crashes in a row since the last answered call".

- Owner decisions (2026-11-17, at the build): decisions 1 to 5 as recommended, on delegation. Decision 6: the call that
  gives the base environment is `ts-0113-ck`'s, not `ts-0109-qj`'s. At review, a supervised launch with no environment
  is refused before any child starts (`SupervisedProvider` `start`; test
  `a_launch_with_no_environment_is_refused_and_starts_no_child`, seen red), so a forgotten `with_env` fails instead of
  inheriting the daemon's environment. Decision 7: the numbers are in the plan's Stage 4b first bullet.

### Not done, by design

- The daemon methods, the source registry, settings and secrets through the daemon (ts-0109, ts-0113).
- `supervise_observer` (Stage 12); the corpus and the plan adapter.
- `task_cli.rs` still starts a `ProviderClient` directly with the default timeouts; Stage 4c moves it behind the
  supervisor.
- No test starts the `collide` binary, a daemon or a real `claude`.
- A child that exits while idle is noticed at the next call, not at once: nothing polls it.
