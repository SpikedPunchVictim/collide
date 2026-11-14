---
id: ts-0016-ka
kind: task
status: ready
title: "Shim protocol, exit.json and the UI/CLI JSON-RPC"
owners: []
links:
  - { type: blocked-by, to: ts-0008-mx }
---

## Goal

The shim protocol, versioned on its own, `exit.json`, and the JSON-RPC between UI or CLI and the owner. The shim logs the bytes written to the child's stdin beside its stdout (S7), so a session can be replayed.

## Success criteria

- Serde round-trips pass for every message.
- The log format has a place for stdin bytes and for stdout bytes, in order.

## Tests

- Round-trip tests; version handshake shape.

## Notes

IMPLEMENTATION_PLAN.md, 0.2 and 0.3.
