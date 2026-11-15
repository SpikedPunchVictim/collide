---
id: ts-0039-ps
kind: task
status: done
title: "Session replay over the journal"
owners: []
links:
  - { type: blocked-by, to: ts-0016-ka }
  - { type: blocked-by, to: ts-0019-vm }
  - { type: blocked-by, to: ts-0022-mc }
---

## Goal

`AgentSession::replay` over the shim's journal, in `core`. Records are fed in `seq` order: an `in` record tells the session what it wrote, so request ids and pending requests are rebuilt, and an `out` record goes through `feed` with its stored `wall_ns`. A replay writes nothing to the child, and the session never reads a clock. A cut-short last record is dropped by the journal reader of `ts-0016-ka`.

## Success criteria

- A fake `AgentSession` replayed over the journal reaches the same state as the live one, including a pending permission request and the events' timestamps, which come from the stored `wall_ns`.
- A journal whose last record is cut short replays up to the last whole record.
- A replay returns no bytes to write: the sink stays empty.
- Records out of `seq` order are an error, not silently reordered.

## Tests

Write first: a live run of the fake ACP child recorded into a journal, then a replay that must equal it; the cut-short journal; the empty sink; the out-of-order case.

## Notes

- IMPLEMENTATION_PLAN.md, 0.2 (the shim and its journal), 0.3 (`AgentSession`, `replay`), Stage 2: Skeleton, Success Criteria (the replay lines) and Tests (Session replay).
- Area: `core` (replay) with the doubles of `ts-0022-mc`.
- Done when the criteria hold.
- Built: `core/src/replay.rs` (registered in `core/src/lib.rs`), the one replay walk, promoted from the private helper of `testing` (ts-0022-mc decision 6, ts-0019-vm decision 2). `Replayable` (public trait: `note_written(&[u8]) -> Result<(), String>`, `read_line(&[u8], wall_ns) -> Step`) is what a session says about itself; `replay_records(&mut S, &[JournalRecord]) -> Result<Step, ReplayError>` is the walk (order check from seq 0, `in` through `note_written`, `out` through `read_line` with the stored `wall_ns`, events returned, `writes` empty, no clock, no I/O). `replay_journal_bytes(&mut S, &[u8]) -> Result<JournalReplay, ReplayError>` is the bytes-to-replay path: `read_journal`, then `AgentSession::replay` over the whole records, returning `JournalReplay { step, records, valid_len, end }`. A `Rejected` error from it carries the offset of its record in the message. `AcpSession` and `NormalizedSession` implement `Replayable` and their `replay` calls `replay_records`; the private `Replayable` and `replay_journal` are gone from `testing/mod.rs`, so there is one walk, and a test scans `core/src` to keep it so. Docs of `adapter.rs`, `journal.rs` and `testing/mod.rs` point at the module. Tests, written first (red: the module did not exist): `core/tests/session_replay.rs` (20): a live ACP run recorded through `encode_record` (every write an `in` record, every fed line an `out` record with its `wall_ns`) replayed into a fresh session whose whole `Debug` state equals the live one; the pending request 8 answered after the replay with the same id and bytes as live; the next turn's request id; events equal to the live events with the stored times; the empty sink; the empty journal; a cut at every byte of the last record, for a last `out` and a last `in` record; a damaged journal; records out of order, with a gap, repeated, or not starting at 0; a rejected record with its offset; a trait object. Nothing was registered in `core/src/schema.rs`.
- Decisions for the owner: (1) No change to `AgentSession`: `replay` stays a required method with the same signature, and a session implements it as `replay_records(self, records)`. `Replayable` is a new public trait and `ReplayError` is unchanged. Alternative if wanted: make `AgentSession` require `Replayable` and give `replay` a default body, which removes the one-line implementation in each session but ties the two traits together; not done, because ts-0019-vm decision 2 kept `replay` required. (2) A `JournalEnd::Damaged` journal is replayed up to `valid_len` and reported in `JournalReplay::end`, not refused. A cut is what a crash leaves; damage is not, but refusing would leave the daemon with no session at all, while the prefix is the same kind of partial state a cut gives. The caller must read `end` and decide (for example mark the run `lost`); Stage 3a's restart path owns that. The alternative is to refuse (`Err`) on damage. (3) An `Err` from a replay leaves the session half rebuilt, and the caller discards it. (4) The offset is added to the text of `ReplayError::Rejected` by `replay_journal_bytes`, not as a field, to keep the enum unchanged.
- Not done by design: the shim's file writer and the daemon's restart path (Stage 3a), what the daemon does with a `Damaged` end, the ACP engine and its use of `Replayable` (Stage 3d), a replay of a Claude session (its session comes with its adapter, 3b), and any change to the item's `status`, `links` or `owners`.
- Owner decisions (2026-11-15): the recommendations above are accepted as built. Follow-up for Stage 3a: a run whose journal ends `Damaged` is marked `lost` with its replayed prefix viewable, and is never resumed silently (decision 2).
