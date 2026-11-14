---
id: ts-0010-yb
kind: task
status: ready
title: "Provider and observer protocols"
owners: []
links:
  - { type: blocked-by, to: ts-0009-cu }
---

## Goal

The provider protocol v0 and the observer protocol, in full, with JSON Schema and generated types. Provider (JSON-RPC over stdio; this item picks the framing and records it, newline-delimited JSON unless a reason is found): `initialize` (receives the pack's `config`; returns `scope`, `idPattern`, `shortIdPattern`, `templateFields`, `statuses` with categories, `fields`, `ops` with argument schemas, the capabilities `watch`, `create`, `collections`, `preview`, `kinds` and `links`); `list` and `get`; the `watch` notifications `task.collision`, `source.stale`, `task.changed`, `task.renamed`, `task.removed` and `task.invalid {path, checkout, reason}`; `apply`, answering `ok` (new task, version, the files touched, `items[]`), `conflict` or `refused`; and `preview`. The op name `move` is reserved for Share. Observer: events as JSONL on stdin, each with a sequence number, acknowledged on stdout, filtered by the pack's `events:`.

## Success criteria

- Serde round-trips pass for every message of both protocols.
- A provider that declares one kind and no `links` validates unchanged.
- `apply`'s `ok` carries the files touched and `items[]` (each with version, from and to). `conflict` carries the current task. `refused` carries the source's message.
- `task.invalid` carries `path`, `checkout` and `reason`.
- An observer acknowledgement names the sequence number it covers.

## Tests

Write first: round-trip tests per message; schema tests (unknown key refused in a notification and in an `apply` result); a fixture `initialize` result with one kind and no links.

## Notes

- IMPLEMENTATION_PLAN.md, 0.4 (Observers), 0.10 (Protocol, Versions, Writes are operations).
- Area: `core` (types and schemas).
- Done when the criteria hold and `collide schema` lists the protocol types. The hook protocol is `ts-0027-ar`.
