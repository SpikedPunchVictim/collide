---
id: ts-0032-pp
kind: task
status: ready
title: "Owner JSON-RPC: methods and events"
owners: []
links:
  - { type: blocked-by, to: ts-0008-mx }
---

## Goal

The JSON-RPC 2.0 contract between the UI or CLI and `collide-daemon` over the local socket: the envelope, the version handshake (`initialize` with the protocol version and the per-client token), the error codes, the event notification channel (each event carries a sequence number and the `actor` the daemon recorded), and the request and result types of the methods Milestones A and B need, grouped by family (`daemon.*`, `run.*`, `task.*`, `queue.*`, `workspace.*`, `config.*`; the `ask.*` family is `ts-0034-hh` and `ui_tools.*` is `ts-0035-re`). The plan does not list the methods, so this item derives the list from the CLI commands and the surfaces of 0.11 and records it in its Notes. The actor is never a parameter: it comes from the connection (0.2).

## Success criteria

- Serde round-trips pass for every request, result, error and event type.
- A request type with an `actor` field fails validation (unknown key), so no client can name its own actor.
- The handshake refuses a protocol-version mismatch with a message naming both versions, and a missing or wrong token with a distinct error.
- An unknown method answers the JSON-RPC "method not found" error.
- Every method in the recorded list has a request and a result schema, and `collide schema` lists them.

## Tests

Write first: round-trip tests; the `actor` refusal; handshake cases; one schema per recorded method.

## Notes

- IMPLEMENTATION_PLAN.md, 0.2 (the UI speaks the owner's protocol; Who is asking; the socket), 0.11, Stage 2: Skeleton, Contracts (the JSON-RPC between UI or CLI and the owner).
- Area: `core` (types and schemas).
- Done when the criteria hold. Gap to report: the plan names no method list.
