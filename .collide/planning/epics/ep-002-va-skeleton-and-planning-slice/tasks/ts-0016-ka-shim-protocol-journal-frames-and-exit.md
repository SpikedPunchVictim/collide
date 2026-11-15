---
id: ts-0016-ka
kind: task
status: done
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
- Built: three `core` modules with pure codecs and no I/O or clock, and `core/tests/shim_protocol.rs` (43 tests). `core/src/shim.rs`: `SHIM_PROTOCOL` (1, its own constant), `ShimMessage` (`Hello {protocol}`, `Wrote {seq}`, `Exited {exit}`, `Failed {message}`), `DaemonMessage` (`Welcome {protocol}`, `Refused {message}`, `Write {data}`, `CloseStdin {}`, `Signal {signal}`, `AckExit {}`), `ExitRecord` (`protocol`, `end` as `Exited {code}` or `Signaled {signal}`, `cpu_user_us`, `cpu_system_us`, `max_rss_bytes`), and the handshake: `check_shim_version(current, announced)` accepts the current and the previous version and refuses an older or newer one with a message naming the shim's, the current and the previous version; `answer_hello` turns that into `Welcome` or `Refused`. `parse_exit_record` gives a typed `ExitRecordError` (`Empty`, `Cut`, `Invalid` with line, column and reason, `Protocol`), so the daemon can move the file aside. `core/src/frame.rs`: a frame is a u32 big-endian payload length, then the payload (compact JSON for a message), capped at 16 MiB; `decode_frame` returns `Whole {payload, consumed}` or `Incomplete {needed}` and never `Whole` for a cut frame; a length over the cap is refused when the prefix is read. `core/src/journal.rs`: a record is `seq` u64 BE, `wall_ns` u64 BE, `dir` u8 (0 in, 1 out), `len` u32 BE, then the bytes; no file header; `read_journal` returns the whole records, `valid_len` and an end of `Clean`, `Cut {offset}` or `Damaged {error}` (bad direction byte, length over 16 MiB, seq not the next one, each with its offset). The contract (journal record before the child sees the bytes; the daemon accepts the previous shim version) is in the module doc comments. Registered in `schema.rs`: `ShimMessage`, `DaemonMessage`, `ExitRecord`; `pnpm gen` output added under `schemas/` and `packages/pack-sdk/src/generated/` (second run changes nothing).
- Decisions for the owner: (1) The plan names the daemon-to-shim messages only as length-framed, so the message set is mine and minimal: it covers the handshake, stdin writes, close, a stop signal, the exit record and its acknowledgement. PTY attach, resize and the output replay buffer are not in it; Stage 3a or the take-over item adds them (a new variant is a new protocol version once a shim in the field cannot read it). (2) `Write.data` is a UTF-8 string, because JSON cannot carry raw bytes and the workspace has no base64 crate. That fits stream-json and ACP lines; raw PTY keystrokes that are not UTF-8 would need another encoding. The journal itself holds any bytes. (3) Both directions of the socket use the same framing, so `ShimMessage` is framed too. (4) A newer shim than the daemon (a daemon rolled back under a running shim) is refused, like an older one; the plan says only "accepts the previous". (5) At version 1 there is no previous version, so version 0 is refused and the message says there is none. (6) The journal has no file header, version byte or checksum, as the plan lists only the four fields; a cut is found by length, damage only by the direction byte, the length cap and the seq check. A header or CRC is cheap to add before Stage 3a writes the first journal. (7) `seq` must start at 0 and rise by one, so a gap stops the read as damage; the shim must not skip a number on a failed write. (8) `exit.json` records CPU as user and system microseconds and max RSS in bytes (the shim converts Linux KiB), and carries `protocol` so a daemon reads the previous version's file. (9) The 16 MiB caps on a frame and on a record are my choice.
- Not done by design: the shim's file writer and the atomic `exit.json` write (Stage 3a); the socket, the daemon and the shim themselves; replaying a session over the journal and `AgentSession::replay` (`ts-0039-ps`); a streaming or incremental journal reader (the whole file is read from byte 0); any change to the item's `status`, `links` or `owners`.
- Owner decisions (2026-11-15): the recommendations above are accepted as built. Follow-ups for Stage 3a: add a file header with a version byte and a CRC per record to the journal before the first journal is written (decision 6), and carry PTY bytes as base64 when attach and resize join the shim protocol (decision 2), matching the owner protocol's choice in ts-0032-pp.
