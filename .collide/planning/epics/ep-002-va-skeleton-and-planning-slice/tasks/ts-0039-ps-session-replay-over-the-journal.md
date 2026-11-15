---
id: ts-0039-ps
kind: task
status: active
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
