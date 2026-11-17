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
