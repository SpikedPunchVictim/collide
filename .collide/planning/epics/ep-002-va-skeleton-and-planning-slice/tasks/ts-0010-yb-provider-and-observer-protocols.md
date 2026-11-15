---
id: ts-0010-yb
kind: task
status: done
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
- Types added (core modules `provider` and `observer`; all are registry roots, so `collide schema` lists them): `InitializeParams`, `InitializeResult`, `ListParams`, `ListResult`, `GetParams`, `GetResult`, `WatchParams`, `WatchResult`, `ApplyParams` (also `preview`'s params), `ApplyResult` (tagged by `outcome`: `ok`, `conflict`, `refused`), `PreviewResult` (tagged by `outcome`), `Task`, `TaskCollision`, `SourceStale`, `TaskChanged`, `TaskRemoved`, `TaskInvalid`, `ObserverEvent`, `ObserverAck`. `Rename` (ts-0009's) is the body of `task.renamed`. `provider::check_initialize` returns every refusal serde cannot express: an op named `move` in `ops` or in a kind's `ops` (`ProviderError::ReservedOp`).
- Framing: newline-delimited JSON, one message per line. JSON escapes newlines inside strings, a provider in any language reads it with no length parser, and the observer protocol already uses it. Stdout carries messages only; stderr is for logs. No reason was found to use `Content-Length` framing.
- Envelope: the types cover each method's `params`, `result` and each notification's `params`, not the JSON-RPC envelope (`jsonrpc`, `id`, `method`, `error`), which is the same for every method and is the transport's. The module doc of `provider.rs` has the method-to-type tables. Every message of both protocols has a round-trip test.
- Observer envelope: `ObserverEvent {seq, event, payload}` on stdin and `ObserverAck {seq}` on stdout. `seq` is an unsigned integer; the key spelling is mine, the plan says only "sequence number". `payload` is an open JSON value until `ts-0018-dd` types each event. The `events:` filter is a key of an observer entry in `pack.yaml`, whose shape the plan does not give, so `PackManifest` is unchanged and the key is `ts-0012-ua`'s, which also owns the pattern matching (`run.*`).
- ts-0009 types extended: `OpDecl` gains `args` (the op's JSON Schema, optional) and `checkouts` (the checkouts it may target; empty means any), so the provider's `ops` and a kind's `ops` are one type; `Rename` gains an optional `checkout`, because `task.renamed` names it (inside `change.renamed` the change's own `checkout` applies); `Category` derives `Ord`, to key `categoryStatus`; `Effect`'s doc comment now says it is also `preview`'s entry. `PlanningFields` is not reused as a whole, because serde cannot flatten a struct that denies unknown keys: `Task` has `kind`, `links`, `owners` and `checklist` itself, using ts-0009's `Link` and `Checklist`.
- Where the item and the plan differ, the plan wins: the item lists `kinds` and `links` among the capabilities, but the plan makes them separate keys of the `initialize` result and `capabilities` is `watch`, `create`, `collections`, `preview`. Both are optional.
- Interpretations (the plan names no shape): `ApplyResult`'s tag key is `outcome`. `apply`'s `version` is optional, because a create has no task yet. `statuses` is a list of ts-0009's `StatusDecl`, and the status a category write produces is `categoryStatus` (category to status name). A template field is `{name, pattern}`. A field is `{id, type, filterable}`, with `type` the provider's own name. `Task` carries `key`, `id` (the long form that `get` and `apply` take), `aliases`, `title`, `body`, `state` (ts-0009's `ItemState`), `version`, `checkout`, `path`, `kind`, `links`, `owners`, `checklist`, `labels`, `collections`, `templateFields` and `fields`; deps are `blocked-by` links. A collection is `{id, title, groups: [{name, tasks}]}`. `task.collision` is `{shortId, locations: [{checkout, path}]}` and `source.stale` is `{reason}`; the plan gives neither a body. `task.changed.actor` is a string (the source's own name for the actor, not ts-0009's `Actor`). `watch` is answered with `{}`. `task.invalid.checkout` is written as null for a source that is not ref-scoped.
- Left for later: a Rust JSON Schema validator is not used: the fixture `initialize` result is checked through serde, as for ts-0009. The `TaskProvider` trait and `defineProvider` are not built here (`ts-0011-at` is the SDK outline).
- Two gaps were closed (owner, 2026-11-14): `task.changed` and `task.removed` carry an optional `checkout` (absent for a global source), and `preview` returns the same three outcomes as `apply`, tagged by `outcome` (`PreviewResult` is a tagged enum; its `conflict` and `refused` variants repeat `ApplyResult`'s, and a test pins the two in step).
- Tests: `core/tests/provider_protocol.rs` (44), a `collide schema` test in `host/tests/schema_command.rs`, and `packages/pack-sdk/test/provider-types.test.ts`. The fixture is `core/tests/fixtures/provider/initialize-one-kind.json`.
