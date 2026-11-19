---
id: ts-0142-ua
kind: task
status: ready
title: "CLI: a queued write whose file changed meanwhile is never reported applied"
owners: []
links:
  - { type: blocked-by, to: ts-0136-rr }
---

## Goal

`settle` in `host/src/task_daemon.rs` decides what became of a queued write: an `-undone` Queue item means not
applied, else a changed task version means applied. A person's edit to the file while the write waits also changes the
version, and the Queue item leaves at take, before the daemon records the `-undone` one. So `settle` can see the new
version first and report the write applied when it was not. `a_queued_write_whose_file_changed_while_it_waited_...`
(`host/tests/task_daemon_cli.rs`, `ts-0120-wm`) failed once this way in a full parallel run. Found at the
`ts-0136-rr` build (its decision 8).

## Success criteria

- A write that the daemon did not apply is never reported applied, however the daemon's steps interleave.
- An applied write is still reported applied, with no wait longer than today's outcome wait.

## Tests

- Write first and see red at runtime: a unit test of `settle` with a stub session that answers the new version before
  the `-undone` item shows, which reports applied today.
- The ts-0120 test passes 20 runs in a row under load.

## Notes

- The builder's alternative: after a version change, look for the `-undone` item again before saying applied. The
  daemon could instead say in the queued write's own record whether it was applied, which `settle` reads; that is a
  wire change, so take it only if a re-check cannot be made sound.
- Owner decisions: none recorded yet. The recommendations stand until the owner answers at the split.

### Built

- `host/src/write_guard.rs`: `Fate` (`InFlight(checkout)`, `Applied`, `NotApplied(message)`), `FATES_KEPT` (128),
  and the guard's `fates` record. `WriteGuard::drain` sets `InFlight` before it takes the item, then `Applied` or
  `NotApplied` after the apply and the `-undone` item. A take that finds the item gone (a person's Dismiss) forgets
  the entry. `fate_of` reads it. `settled_fate` first holds the line lock of an in-flight write's checkout, which
  the drain holds until it is done, so its answer is final.
- `host/src/daemon.rs`, `queued_write_answer`: `queue.answer` reaches it for an item that is queued or has a fate. A
  Retry that finds the item gone answers `done` for `Applied`, `refused` with the daemon's words for `NotApplied`, and
  not found otherwise (dismissed, restarted, forgotten). No wire type changed, so `pnpm gen` had nothing to do.
- `host/src/task_daemon.rs`: `settle`, `undone_of`, `queued_at`, `OUTCOME_POLL` and `OUTCOME_WAIT` are gone. `Settled`
  now has `NotApplied(String)`. `retry_until` returns what the daemon said (`settled_by` maps the answer). The
  version of the task is no longer read, so `Waiting` is the label only; creates, reids and `renameSection` now get a
  true answer too. The words for a write that is gone and not applied no longer say "did not change".
- Tests, by name. Red at runtime, against a stub that returned no fate (before the guard recorded any), or against the
  old CLI: `a_write_a_drain_applied_has_the_fate_applied`,
  `a_write_a_drain_could_not_apply_has_the_fate_not_applied_with_the_words`,
  `a_write_is_in_flight_from_the_moment_it_is_taken_until_its_apply_returns`,
  `the_fate_of_a_write_in_flight_is_not_told_until_the_drain_has_ended`,
  `the_guard_forgets_the_oldest_fate_after_it_holds_as_many_as_it_keeps` (all `host/tests/write_guard.rs`),
  `retry_of_a_write_the_writer_applied_says_done`,
  `retry_of_a_write_the_writer_could_not_apply_refuses_with_the_reason` (same file, daemon), and
  `a_queued_write_a_person_dismisses_after_the_file_changed_is_not_reported_applied` (`host/tests/task_daemon_cli.rs`,
  red against the old CLI: exit 0, "applied"). Green before and after, kept as pins:
  `a_write_a_person_dismissed_has_no_fate`, `retry_of_a_write_a_person_dismissed_finds_no_item`, four `settled_by`
  unit tests. The ts-0120 test now also asserts the daemon's reason ("changed since"); the dismiss test's words
  changed with the message.
- There is no stub-session unit test of `settle` as the item asked: `Session` holds a concrete client, and the new
  rule lives in the daemon, so the tests drive the real guard and daemon.
- Mutations, each red then restored: `settled_fate` without the lock wait (the in-flight test), `NotApplied` answered
  `done` (the Retry test and the ts-0120 test).
- The ts-0120 test, 20 runs one after another, each beside one run of the `write_guard` test binary: 20 passed.

### Decisions for the owner

1. A re-check cannot be made sound, so the daemon says it (the item's second option, with no wire type change). Two
   orders defeat any re-check. (a) Take, then a person's edit changes the version, then the apply is still running:
   the CLI looks for `-undone`, finds none, and says applied; the item appears a moment later. Nothing the CLI can
   read says the apply has returned. (b) A person edits the file and dismisses the write: no apply, no `-undone` item,
   and the version changed. Under the new rule every order is told by the daemon: take then apply ok (`Applied`);
   take then conflict or refusal, with the `-undone` item made before the fate is set (`NotApplied`); take with the
   apply still running (`InFlight`, and the Retry waits for the line lock the drain holds, so it answers after the
   fate is set); Dismiss before the take (no fate, not found, never applied); Dismiss between the in-flight mark and
   the take (the entry is forgotten, not found); version change at any point (not read at all).
   Alternative: re-check, accepted as bounded. Recommend what was built.
2. The answer reuses `QueueAnswerResult` as it is: `done` means applied, `refused` carries the reason. The alternative
   is two new variants, which the app's `noticeFor` switch would have to know. The app's Retry of a queued write now
   shows the refusal, where it showed `done` before. Recommend as built.
3. The record is in memory and holds 128 writes. After a restart or an eviction a gone item is "not found", which the
   command reports as not applied ("dismissed, or the daemon has no record of it"), exit 5. That is safe, and wrong
   only for a write that did apply across a restart, which also drops the CLI's connection. Alternative: keep the
   fate in the store. Recommend memory until a case shows up.
4. The NotApplied line prints the daemon's reason alone, not the whole `-undone` item title. Alternative: keep
   `undone_of` for the title. Recommend the reason: it is the same words, and it drops a second `queue.list` call.
5. Plan text, for 0.5 (Root busy) or 0.2 (the CLI from a terminal): "A command that waited for a queued write says
   applied only when the daemon says it: a write that left the Queue and was not applied, or that the daemon has no
   record of, exits 5."
   Not applied: the plan is the reviewer's.

### Not done, by design

- The fate is not kept in the store, and the app does not show it (decision 3).
- `ts-0144-eh` to `ts-0152-jq` are untouched, and none of them is made trivial or changed by this item.

### At review

- The wire route taken (decision 1): the two orders the builder names defeat a re-check. Decisions 2 to 4 taken as
  recommended. Decision 5 applied to Stage 4c's CLI bullet, in place of the version-watch sentence.
- Traced: a dismiss that races the take (the in-flight mark is forgotten, so a Retry finds no record and the CLI says
  not applied), and a Retry while another drain applies the write (it waits on the checkout's line lock). A daemon
  restart ends the CLI's connection, so the no-record case is a dismissal in practice.
- Not built: the stub-session unit test of `settle`, which is gone; the daemon tests drive the real guard.
- Mutations, each red then restored: no in-flight mark (two guard tests), a problem recorded as applied (a guard test
  and a daemon Retry test), a gone write not routed to its fate (both daemon Retry tests), not found taken as applied
  (both dismiss tests in the CLI).
