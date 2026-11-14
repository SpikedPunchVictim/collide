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

The shim protocol, versioned on its own, `exit.json`, and the JSON-RPC between UI or CLI and the owner.

## Success criteria

- Serde round-trips pass for every message.

## Tests

- Round-trip tests; version handshake shape.

## Notes

IMPLEMENTATION_PLAN.md, 0.2.
