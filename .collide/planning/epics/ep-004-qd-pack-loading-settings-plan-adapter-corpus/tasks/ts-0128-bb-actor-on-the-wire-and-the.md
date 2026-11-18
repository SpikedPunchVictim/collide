---
id: ts-0128-bb
kind: task
status: ready
title: "Actor on the wire and the person-only rule in the daemon"
owners: []
links:
  - { type: blocked-by, to: ts-0127-js }
---

## Goal

A write to a proposal or a question reaches the provider with the actor the daemon read from the connection, and the
daemon applies the person-only rule before the provider is asked.

What exists. `host/src/auth.rs` has `Caller` (`App`, `Cli`, `RunAgent`), `classify_peer`, `Session::actor` and
`Session::judge`, `PersonOnlyOp` (`accept`, `defer`, `reject`, `resolve`), `person_only_request(op, item_kind, item,
to_status)` and `judge`, which gives `Allow`, `Confirm` (a `QueueConfirmation`) or `Refuse` (`NOT_PERMITTED`). All of it
is unit-tested and none of it is called outside `auth.rs`. `host/src/daemon.rs` `task_apply` passes `session.actor()` to
`apply_and_record`, which only records it on the event. `ApplyParams` in `core/src/provider.rs` has no actor.
`QueueKind` (`core/src/owner/queue.rs`) has no confirmation kind; `AnswerKind::Accept` and `Dismiss` exist. A
queued write (`host/src/write_guard.rs`, `ts-0114-hr`) is the model for a stored write answered from the Queue.

What this adds.
- `ApplyParams.actor` (additive, set by the daemon, never read from `TaskApplyParams`): `{ kind: person | agent,
  at: <date>, run?: <run id> }`. `at` is the daemon's local date as `YYYY-MM-DD`, from the injectable clock, so a test
  fixes it. The SDK's generated types and the schemas are regenerated.
- `task_apply` finds the item's kind (corpus, by key) and calls `person_only_request`. `Allow` writes with the actor of
  the connection. `Refuse` answers `NOT_PERMITTED` and the provider is never called. `Confirm` writes nothing, stores
  the write, and adds a Queue item.
- `QueueKind::Confirmation`: a title in Collide's own words (no task title; 0.9), the stored write, answers `Accept`
  and `Dismiss`. `Accept` applies the stored write as a person (as the app); `Dismiss` drops it. The item and the
  write are one store row, as a queued write's are, so both survive a restart. `app/src/loop/Queue.tsx` renders the new
  kind with its title and its two answers.
- `collide task <id> accept` and its relatives: the CLI prints that a confirmation is waiting in the app and exits with
  a code of its own (12). Scratch tasks and every kind that is not `proposal` or `question` are unaffected.

## Success criteria

- From the app's connection, `accept` on a proposal is written, and the event's actor is a person.
- From a run's agent (a child of a run, as `pause.rs` builds one), the same call is refused with `NOT_PERMITTED` naming
  the run and the op, and nothing changes on disk.
- From the CLI, the same call writes nothing, adds one Queue item with no task text in its title, and the CLI exits 12.
  Answering `Accept` applies it once, with a person's actor; `Dismiss` leaves the file as it was; answering twice
  applies once; a restart keeps it.
- `setStatus` to `accepted`, `deferred` or `rejected` on a proposal is judged as `accept`, `defer` and `reject`; any
  other `setStatus`, and every op on a task, is not.
- The provider sees `actor` on every `apply` the daemon makes, and a call with no actor field (an older client) is read
  as `person`.

## Tests

Write these first.

- `host/tests/auth.rs` extended: each verdict through the daemon's `task_apply`, with the caller faked at the session.
- A socket test with a run agent child (the `pause.rs` shape of `ts-0119-tj`'s `save_view` test): refused, and the
  provider's fake saw no call.
- The confirmation through `queue.answer`: `Accept`, `Dismiss`, twice, and after a restart. A component test of
  `Queue.tsx` for the new kind.
- Schema and generated-type checks for `ApplyParams.actor`; the CLI's exit code and text.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 4c: Feature ops bullet ("the person-only rule and `by:` (0.10), with the actor").
  - 0.10: Planning sources ("Only a person accepts, defers, rejects or resolves ... The actor is read from the
    connection (R5), never from an argument"). 0.2: Who is asking. 0.1: same-user rule. 0.9: Task text is data.
- **Owner's choice, with a recommendation: what the CLI's person-only op does.** 0.10 says it becomes a Queue
  confirmation that the person answers in the app. Recommend that, as the plan says. Without it a person cannot accept a
  proposal from a terminal at all, and the app's Queue page (3c's loop page lists Queue items; the full page is Stage 6)
  answers it. Alternative: the CLI refuses with "do it in the app". Smaller, and no new Queue kind, but the owner cannot
  accept a proposal by CLI before the app has the drawer (Stage 9d). This sets what the owner can do from a terminal.
- **Decision for the item, with a recommendation: judged by kind name.** `person_only_request` names `proposal` and
  `question`. Recommend keeping that for the built-in `collide/plan`'s kinds and for any source that declares them.
  Alternative: judge by the kind's role tags, which is the `roles` mapping (`ts-0135-cx`) and Stage 12's "provider role
  tags as defaults". A team provider's `adr` is then not covered until Stage 12.
- **Decision for the item: returning `deferred` to `proposed`.** 0.10 says "until a person returns it". Recommend adding
  the move to the person-only set (one more `PersonOnlyOp`, `reopen`). Alternative: leave it to any actor.
- **Decision for the item: the date.** `at` comes from the daemon, not the provider's clock, so `by: human:<date>` is
  testable and one clock answers for all providers.
- Not here: the phase-end check that fails a phase whose agent edited a `status:` or `by:` line. See `ts-0129-uv`.
- Area: `core/src/provider.rs` (`ApplyParams`), `core/src/owner/queue.rs`, `host/src/auth.rs`, `host/src/daemon.rs`
  (`task_apply`, `write`), `host/src/write_guard.rs` (the stored write), `host/src/task_cli.rs`, the generated schemas
  and TypeScript, `app/src/loop/Queue.tsx`, `app/test/support/` (the doubles), `host/tests/`.
- The test rule: no test touches the real home, real launchd or a real daemon. Each test that runs the `collide` binary
  clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as
  `host/tests/common/mod.rs` `TempRoot` does. Settings files are fixtures, never the owner's own `settings.json`. A temp
  repository sets `GIT_CONFIG_GLOBAL` and `GIT_CONFIG_SYSTEM` to empty files.
- Stage 4c item (revision 23 scope, split 2026-11-18). Items of Stage 4c run one at a time, in id order, because they
  share `core`, `host`, `packs/plan` and the generated files.
- Nothing here runs a real `claude`. No dependency is added.
- Owner decisions: none recorded yet. The recommendations stand until the owner answers at the split.
