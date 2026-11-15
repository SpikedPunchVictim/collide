---
id: ts-0036-xt
kind: task
status: done
title: "Normalized agent events and capability table"
owners: []
links:
  - { type: blocked-by, to: ts-0008-mx }
---

## Goal

In `core`, as types with schemas: the normalized events (message, tool call, permission request, `SessionBound`, `Usage`, `TurnEnd`, `FinalReport`, `Other`); the session start `Assign(uuid)`, `AgentAssigned` or `Resume(id)`; the permission request (its tool call, `kind`, `input: Option`, why it was asked, the agent's own options) and the answer `Allow`, `Deny { message: Option }` or `Cancel`; `Usage` with a scope (`Turn`, `Step`, `RunningTotal`), every number an `Option`, a cost with its currency and a `quality` (`Exact`, `Partial`, `Unreported`); `TurnEnd { reason, cancel_requested, empty }` with the reasons of 0.3; `FinalReport.source` (`ResultField`, `Reconstructed`); and the capability table of 0.3, 18 capabilities, each `Yes`, `No` or `Unknown`, with its sub-flags.

## Success criteria

- Serde round-trips pass for every event and answer type.
- A `Usage` number that is absent is `None`, never 0, and the schema says so.
- A capability that is not set is `Unknown`, and the table lists all 18 capabilities of 0.3.
- A `PermissionUnanswerable` event exists, and a `Cancel` answer parses.
- An unknown key fails in every type, and an unrecognized stream event is representable as `Other` with its raw type name.

## Tests

Write first: a round-trip test per type; the all-`Unknown` default table; the `None`-not-zero usage case; the unknown-key cases.

## Notes

- IMPLEMENTATION_PLAN.md, 0.3 (Two core traits; Normalized events; Permissions; Usage; A turn ends with; Capabilities), Stage 2: Skeleton, Contracts (the normalized events and the capability table). Spike S7's record: `.collide/planning/spikes/sp-0006-kq-agentadapter-acp/record.md`.
- Area: `core` (types and schemas).
- The traits that use these types are `ts-0019-vm`.
- Done when the criteria hold.

### Built

- `core/src/agent.rs` (new, registered in `core/src/lib.rs`): `SessionStart`, `ToolKind`, `PermissionOption(Kind)`, `PermissionRequest`, `PermissionReply`, `UsageScope`, `UsageQuality`, `Money`, `Usage`, `TurnEndReason`, `FinalReportSource`, `AgentEvent`, `Support` and the 18-capability `Capabilities` table (with `CAPABILITY_NAMES`). Closed structs, `type` tag in `snake_case`, empty variants as `{}`.
- `core/src/schema.rs`: registers `AgentEvent`, `Capabilities`, `PermissionReply`, `PermissionRequest`, `SessionStart`, `Usage`. `pnpm gen` wrote the new `schemas/*` and `packages/pack-sdk/src/generated/*` files; a second run is byte-stable.
- `core/tests/agent_events.rs`: 30 tests (round-trips, `None` never 0 in code and schema, all-`unknown` table, unknown keys in every type, `Other`).

### Decisions

- `Other { raw_type, raw }`: serde rejects an unknown tag, so `AgentEvent` has a hand-written `Deserialize` (derive with `remote = "Self"`). A `type` outside `KNOWN_EVENT_TYPES` reads as `Other` with that name and the rest of the object in `raw`; a known `type` goes through the closed derive, so an unknown key still fails. A test pins the list to the schema. The schema lists known events only (a `not`-enum branch would break the generated TypeScript union).
- The answer is `PermissionReply`, not `PermissionAnswer`: `events::PermissionAnswer` (lowercase, no message) already exists for the `permission.answered` log event and was left alone.
- Capabilities: a flag is `Support` (`yes`/`no`/`unknown`, default `unknown`). A capability with a value (`session_resume`, `system_prompt`, `model_select`) is an enum with an `unknown` variant. `permission_host`, `context_options` and `mcp_servers` are structs with `support` plus sub-flags. `usage_report` has `support`, `granularity: Vec<UsageScope>` (empty = not declared), `cost`, `cache_write_tiers`, `context` and `quality: Option<UsageQuality>` (absent = not declared).
- Shapes the plan leaves open: `Message { text }`, `ToolCall { id, tool, kind, input }`, `ToolKind` (the ACP kinds), `PermissionRequest.id`/`tool_call_id`/`tool`, `PermissionOption`, `PermissionUnanswerable { request_id }`, `Usage.cache_write_1h`/`cache_write_5m` (from S7 P4 and the report line), `Error { status: Option<u16> }`. Session ids and uuids are strings (no new dependency).
- Absent numbers are left out when written (as `ReportLine::Usage` does) and are optional and nullable in the schema; a reported 0 is kept.

### Not done by design

No traits (`ts-0019-vm`), no adapter, no fixtures.
