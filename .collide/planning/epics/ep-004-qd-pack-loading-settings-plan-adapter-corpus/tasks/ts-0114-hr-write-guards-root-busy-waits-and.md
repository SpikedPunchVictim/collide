---
id: ts-0114-hr
kind: task
status: ready
title: "Write guards: root busy waits and writes queue while a phase runs"
owners: []
links:
  - { type: blocked-by, to: ts-0113-ck }
---

## Goal

Guard every write to a checkout the way 0.4 and 0.10 say: one write at a time to a root (the line lock), a write waits
while a rebase, merge, cherry-pick, revert or bisect is in progress there and says why, and a write into a checkout
where a phase is running is queued until the phase ends.

What exists. `host/src/file_run/accept.rs` has a private `IN_PROGRESS` list (rebase-merge, rebase-apply, MERGE_HEAD,
CHERRY_PICK_HEAD, REVERT_HEAD, BISECT_LOG) and refuses an accept on a busy checkout; it does not wait (`ts-0079-pu` left
"root busy as a wait" undone). `host/src/task_cli.rs` runs ops directly on the `collide/plan` provider with no guard.
After `ts-0113-ck`, the daemon writes any source through one function. The line lock is named in `host/src/hooks.rs` as
later work, and the planning slice's `--checkout` and `--commit` (`ts-0031-xu`) left "refuse a write while a phase runs
in the checkout" for a later stage. `core/src/owner/queue.rs` has `QueueKind::Conflict` ("a queued write, a busy line
root") and the store keeps Queue items (`queue_items`).

What this adds.
- `root_busy(checkout)` in `host/src/checkout.rs`, shared with accept (accept keeps refusing): which operation is in
  progress, in words.
- A line lock: one holder at a time for each root, held for a write and its follow-up. In the daemon's process; `record`
  and `integrate` (Stage 7) take the same one.
- A registry of checkouts where a phase is live, fed by `host/src/file_run.rs` when a phase starts and ends, and rebuilt
  at daemon start from the live runs the store and the adopted shims hold.
- The daemon's write path: a write to a busy or phase-running checkout is not applied. It is stored, with a `Conflict`
  Queue item that says why, answered as queued, and applied in order once the checkout is free. A write to a root where
  a phase starts at the root is the exception (revision 22): it does not queue.
- The CLI's direct path: a write waits while the checkout is busy, prints why, and applies when it ends.

## Success criteria

- A write into a checkout with a running phase is queued, not applied: the answer says queued and names the Queue item,
  and the file is unchanged. When the phase ends the write applies, and several queued writes apply in the order they
  were made.
- A queued write survives a daemon restart.
- A queued write whose `version` is old when it comes to apply does not overwrite: it becomes a conflict item with the
  current task.
- A write to the board root during a rebase, merge, cherry-pick, revert or bisect does not apply and says why ("a rebase
  is in progress in <root>"), in the Queue item and in the answer. It applies once the operation ends.
- `collide task <id> <op>` (direct path) during a rebase prints the reason once, waits, and applies after the rebase
  ends. `--no-wait` refuses at once with the same reason. The default wait ends after 10 minutes with a refusal that
  names the cause; `--wait <seconds>` sets it.
- Writes to one root run one at a time: a second write waits for the first. Writes to two different roots do not wait
  for each other.
- A write to a checkout where a phase starts at the root is not queued by the phase.
- A scratch write never queues and takes no lock (it touches no checkout).
- Accept still refuses on a busy checkout, unchanged.

## Tests

Write these first.

- Root busy, against a fixture repo stopped mid-rebase (the stage's test): two branches that edit one line, a rebase
  that stops on the conflict. Check the reason text for each of the six states with a hand-made git-dir file.
- The daemon: a write to that root is queued with the reason; `git rebase --abort`, then the write applies and the item
  clears. Use the built-in `collide/plan` provider on a fixture board with `.collide/planning/`; no Orchard files.
- The CLI: the process run with `env_clear()` waits, prints the reason, applies after the rebase ends (the test ends the
  rebase from another thread); `--no-wait` exits at once with the reason; `--wait 1` refuses after 1 s.
- Phase queue: a fake live phase registered for a checkout, writes queued and applied in order when it ends, a restart
  in between, and a stale version becoming a conflict.
- The line lock: two writers on one root in turn, two roots in parallel.
- The registry is rebuilt at start from a live run left in the store.
- The root-start exception, with a fake registration marked as started at the root.
- Accept's refusal still passes its existing test with the shared `root_busy`.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 4b: "Root busy (0.4)"; the adapter's "queue while a phase runs in that checkout"; Success Criteria: "A write
    into a checkout with a running phase is queued, not applied" and "A write to the board root during a rebase waits,
    and the CLI says why. It applies once the rebase ends."; Tests: "Root busy, against a fixture repo stopped
    mid-rebase".
  - 0.4: Line lock; Root busy; Where an agent starts ("Collide's writes at a root with a live phase do not wait"). 0.10:
    Ref-scoped sources (no writes into a checkout while a phase is running). 0.12: Lines.
- Carried from `ts-0031-xu` (refuse a write while a phase runs) and `ts-0079-pu` (root busy as a wait).
- **Owner's choice, with a recommendation: what a daemon write does while the root is busy.** The plan says the write
  waits and the Queue says why. A UI action that blocks for the length of a rebase is a hung screen, so recommend that
  the daemon stores the write, answers queued with the reason, and applies it when the root is free, the same as a phase
  queue; the CLI, which has a person watching a terminal, waits in place. Alternative: the daemon call also waits, with
  notices to the caller. It ties up a connection for minutes. This decides what the owner sees in the app during a
  rebase.
- **Decision for the item, with a recommendation: the CLI's wait.** Up to 10 minutes by default, with `--wait <seconds>`
  and `--no-wait`, because an agent or a script that runs `collide task` during a rebase should not hang for ever.
  Alternative: wait with no limit. Recommend the limit.
- **Decision for the item, with a recommendation: six states, not four.** The plan names rebase, merge, cherry-pick and
  bisect; accept's list also has revert, and a revert stopped on a conflict is as busy. Recommend the six.
- **Decision for the item: the wire.** `TaskApplyResult` gains a `queued` field (the Queue item and the reason), and its
  `result` is absent when the write is queued. A wire change, so the schemas and generated TypeScript are regenerated.
- **Decision for the item: Queue item answers.** `Retry` (try now) and `Dismiss` (drop the write), both existing kinds
  of answer. Check them in `core/src/owner/queue.rs` before adding any.
- **Decision for the item: the lock.** In the daemon's process, keyed by the root's canonical path. The CLI's direct
  path keeps the provider's own lock file (the markdown provider's, 0.10 "Provider writes are safe against each other").
  Two processes writing one root, the CLI direct and the daemon, are not serialized by this item. Say so in a comment.
- Out of scope: `record` and `integrate` taking the lock (Stage 7); a phase that starts at the root (Stage 8; only the
  exception's rule is here); the index lock retry; making accept wait.
- Area: `host/src/checkout.rs`, `host/src/file_run/accept.rs` (use the shared check), `host/src/file_run.rs` (the
  registry), a new `host/src/write_guard.rs`, `host/src/daemon.rs`, `host/src/task_cli.rs`, `host/src/store.rs` only if
  the queued write needs a table (prefer `queue_items`' JSON), `core/src/owner/task.rs`, `schemas/`, `host/tests/`.
- The test rule: no test touches the real home, real launchd or a real daemon. Each test clears the environment
  (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as `host/tests/common/mod.rs` `TempRoot`
  does. Settings files are fixtures, never the owner's own `settings.json`. Every git repository is a temp one with
  `GIT_CONFIG_GLOBAL` and `GIT_CONFIG_SYSTEM` set to empty files.
- Stage 4b item (revision 23 scope, split 2026-11-17). Items of Stage 4b run one at a time, in id order, because they
  share `core`, `host` and the generated files.
- Nothing here runs a real `claude`.
- Owner decisions (2026-11-17, at the split): a daemon write to a busy root is stored and answered `queued`, a Queue
  item says why, and it applies when the root is free; the CLI waits in place (`--no-wait` to skip). The rest: the
  recommendations above.

### Built

- `core/src/owner/task.rs`: `WriteQueued { item, reason }`; `TaskApplyResult.result` is now `Option<ApplyResult>` and
  there is a new `queued: Option<WriteQueued>`. A queued write has no `result`. `pnpm gen` ran; `schemas/` and
  `packages/pack-sdk/src/generated/` (`TaskApplyResult`) are in step.
- `host/src/checkout.rs`: `root_busy(env, checkout) -> Result<Option<&'static str>, WorkspaceError>`, with the
  `IN_PROGRESS` list moved here from accept (six states: rebase, merge, cherry-pick, revert, bisect; the rebase has two
  files). It reads the checkout it is given (`git rev-parse --git-path`), so a linked worktree has its own answer.
  `host/src/file_run/accept.rs` calls it; its refusal text is unchanged.
- `host/src/write_guard.rs` (new): `LineLocks::hold` (one lock per canonical checkout path, in the daemon's process),
  `LivePhases` (`started(run, checkout, at_root)`, `ended`, `blocking`, `take_released`), `QueuedWrite`, `Applied`, and
  `WriteGuard` (`blocked`, `enqueue`, `drain`, `dismiss`, `checkouts_waiting`, `checkout_of`, `is_queued_write`).
  `drain` stops at the first write that still has to wait and refreshes that item's title.
- `host/src/store.rs`: migration 9, `queue_item_payload` (`ALTER TABLE queue_items ADD COLUMN payload TEXT`), with
  `set_queue_item_with_payload`, `queue_payload` and `queue_payloads`. `host/src/queue.rs`: `push_with_payload`,
  `payload`, `payloads`. `Queue::push` leaves a payload that is there.
- `host/src/file_run.rs`: `FileRunConfig.live`. A phase is registered after `start_phase` succeeds, a restored context
  whose workspace is `in_use` is registered in `FileRuns::new`, and `end_workspace_phase` takes it out last. A paused
  run stays registered.
- `host/src/sources.rs`: `Source::writes_checkout` (true for a pack source, false for scratch).
- `host/src/daemon.rs`: `write` now takes the board and root. A scratch write is applied with no lock and no queue.
  Any other write holds the checkout's line lock, drains the writes queued before it, then applies, or is queued when
  `blocked` says why (`Written::{Applied, Queued}`). `apply_and_record` is the old body of `write`. A thread
  (`collide-write-queue`, `writer_loop`) drains at start, when a phase ends, and every `DaemonOptions::write_poll`
  (`WRITE_POLL`, 1 s). `queue_answer` hands a queued write's item to `queued_write_answer` (Retry drains that checkout
  now; Dismiss drops the write). `Daemon::live_phases()` lets a test register a phase.
- `host/src/task_cli.rs`: `Wait { Default, Seconds, Never }`, `DEFAULT_WAIT` (10 minutes), `--wait <seconds>`,
  `--wait=<n>`, `--no-wait`, `wait_until_free` (polls every 250 ms, says the reason once on stderr, then exit 5 on
  `--no-wait` or when the wait ends). It runs after `open` and before the item is read, for an op, `create` and `reid`.
  `USAGE` and `HELP` name it.
- `app/src/loop/loop.ts` (`taskFor`): reads the new optional `result` of `task.apply` (a scratch write is never queued).
- Tests. `host/tests/write_guard.rs` (30): the six states by hand-made git-dir file, a real stopped rebase, a linked
  checkout, the lock, the registry, and the daemon over the built-in plan provider (queued with the reason, abort then
  applied, the other five states, order, behind a queued write, restart, no id reuse after restart, Retry, Dismiss, live
  phase, order behind a phase, another checkout not held, stale version becomes a conflict item, root-start exception,
  rebuild from a stored context, scratch never queues, the wire has no `result`).
  `host/tests/daemon_sources.rs`:
  `writes_to_one_root_run_one_at_a_time_and_writes_to_two_roots_do_not_wait_for_each_other` (new `gate` op in
  `fake_pack_provider.mjs`). `host/tests/task_cli_wait.rs` (11): the options, and the binary with a
  cleared environment waiting, applying after the rebase ends, `--no-wait`, `--wait 1`, create and reid, a read, and a
  free checkout. `host/src/queue.rs` (3), `core/tests/owner_protocol.rs` (1). Updated for `Option`:
  `daemon_sources.rs`, `scratch_tasks.rs`, `host/tests/adapters/claude/quick_run.rs`.
- Seen red at runtime. Against a stub of `WriteGuard::blocked` (always free), `root_busy` (always none),
  `LineLocks::hold` (no lock) and `LivePhases::blocking` (none): 21 of the 30 `write_guard.rs` tests failed (every
  queued, restart, Retry, Dismiss, order, stale, registry, rebuild and wire test, the six-state and rebase `root_busy`
  tests, the lock test, the live-phase test); the 9 that passed are the ones that expect no blocking. With the lock
  alone stubbed, the daemon lock test failed (after the provider's 60 s gate). With `wait_until_free` stubbed, the four
  binary tests that wait or refuse failed. With the payload not stored, the three `queue.rs` tests failed. Not seen red
  at runtime (they failed only to compile before the code): the option-parsing tests, the help test, the core wire test,
  and the accept refusal test, which is an existing test that passes unchanged.

### Decisions for the owner

- **Where the queued write is kept: a `payload` column on `queue_items`** (migration 9). The alternative is a table of
  its own. `QueueItem` is a wire type with `deny_unknown_fields` and the write holds task text, so the payload stays out
  of it; one row means `take` removes both. Recommend the column. `ts-0116-wh`'s migration comes after 9.
- **Take, then apply.** A queued write is taken out of the Queue before it is applied, so only one caller applies it. A
  crash between the take and the write loses the write (at most once). The alternative is to apply and then take, which
  can apply twice. Recommend at most once, as accept's intent and outcome records do.
- **A queued write that conflicts, is refused or errors** becomes a new `Conflict` item that can only be dismissed, and
  the write is gone. Its `task` is the key, so `task.get` shows the current task. The alternative keeps the write with
  Retry. Recommend dropping: a stale write must not be tried again unseen.
- **A paused run stays live**, so writes to its worktree queue until it ends. The alternative frees them during a
  pause. Recommend queue: the agent comes back to the same files.
- **What counts as a checkout write** is `Source::writes_checkout`: any pack source, whatever scope it declares.
  Scratch is the only exception. Recommend this until a provider declares a global scope that is not a checkout.
- **The CLI waits before it reads**, so the version is the one the finished rebase left. A wrong op name or argument
  therefore shows after the wait, not before. The alternative validates first and reads again after the wait.
  Recommend the simple order.
- **Exit code 5** for `--no-wait` and for the wait running out, the same as a provider refusal (nothing was written).
  The alternative is a new code. Recommend 5.
- **The CLI cannot see a live phase** (only the daemon knows), and its lock is the provider's lock file. Two processes
  writing one root are not serialized (comment on `wait_until_free`). The CLI's move behind the daemon is
  `ts-0120-wm`.
- **The line lock is in memory and its entries are never freed.** One entry per checkout path used; small. Recommend
  leaving it.
- **A drain runs every second** (`WRITE_POLL`) and at once when a phase ends. A rebase ending is seen within the
  poll. Recommend 1 s.
- **Item ids** are `write-<ms>-<n>`; the number goes on after the highest the store holds, so a restart under a clock
  that did not move makes no id twice.
- **Plan text.** Stage 4b "Root busy (0.4)" could add: "A daemon write to a busy root answers `queued` and names its
  Queue item; the item takes Retry and Dismiss. The CLI waits in place, 10 minutes by default."

- Owner decisions (2026-11-17, at the build): every recommendation above, on delegation. The plan's Root busy bullet in
  Stage 4b now names revert and the CLI's 10-minute default; Retry and Dismiss are there too. Two were weighed at review
  and kept: the CLI waits before it reads, because a checkout mid-rebase can hold conflict markers, so a read before the
  wait could fail on what the finished rebase would fix; and take-then-apply loses a write only when the daemon dies
  inside one provider call, and the task not moving shows it. At review, the stale-version path was mutated (a conflict
  counted as applied) and `a_queued_write_whose_version_is_old_becomes_a_conflict_item_and_overwrites_nothing` failed.

### Not done, by design

- A phase starting at the root of a line: the `at_root` registration and its exception are tested with a fake
  registration; nothing registers one until Stage 8.
- `record` and `integrate` under the line lock: Stage 7.
- A shim adopted with no run context is not registered as a live phase; none exists until Stage 8.
- Writes through the daemon are not committed (`commit` is refused): `ts-0120-wm`.
- One event for a watched write, watching sources, and a `config.set` provider restart: `ts-0116-wh`.
- The index lock retry, and making accept wait: out of scope in the item.
- The app's Queue view of a queued write (the item says why in its title; no new UI).
