---
id: ts-0016-ka
kind: task
status: active
title: "Shim protocol, journal, frames and exit.json"
owners: []
links:
  - { type: blocked-by, to: ts-0008-mx }
---

## Goal

The shim protocol, versioned on its own, with its byte-level pieces in `core` (types and codecs, no I/O): the daemon-to-shim messages, **length-framed** so a half-written message is never taken as whole; the shim's **journal**, one ordered file of records `{seq, wall_ns, dir, bytes}` where `dir` is `in` (written to the child's stdin) or `out` (read from its stdout), with one counter for both directions; and `exit.json` (exit status, signal, CPU time, max RSS). The contract, written in the doc comments: the shim writes a journal record before the child sees the bytes, and the daemon accepts the previous shim version. The shim's file writer and its atomic `exit.json` write are Stage 3a.

## Success criteria

- Serde round-trips pass for every shim message and for `exit.json`. The journal and frame codecs round-trip.
- A journal whose last record is cut short, at any byte offset, reads back up to the last whole record.
- A framed message cut at any byte offset is never decoded as a whole message, and decodes once the rest arrives.
- The handshake shape: the shim announces its protocol version, the daemon accepts the current and the previous version, and an older one is refused with a message naming both.
- A damaged `exit.json` is a typed error, so the daemon can move it aside and report.

## Tests

Write first: round-trip tests; a cut-at-every-offset test for the journal and for the frame decoder; the version handshake cases.

## Notes

- IMPLEMENTATION_PLAN.md, 0.2 (the shim, the journal, `exit.json`, the shim protocol's version), 0.3 (`AgentSession`: `feed` and `replay`), Stage 2: Skeleton, Contracts (shim protocol, journal, framed messages) and Success Criteria (the journal replay lines). Spike S7's record for why replay needs both directions.
- Area: `core` (shim protocol types and codecs).
- Done when the criteria hold. Replaying a session over the journal is `ts-0039-ps`.
