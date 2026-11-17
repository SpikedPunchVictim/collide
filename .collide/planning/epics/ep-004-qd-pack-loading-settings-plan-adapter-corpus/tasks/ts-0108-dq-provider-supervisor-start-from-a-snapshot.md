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
