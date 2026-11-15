---
id: ts-0060-mm
kind: task
status: done
title: "Agent events, model aliases and capability refusal"
owners: []
links: []
---

## Goal

The `core` pieces the Claude adapter needs before any parsing. First, the normalized events that 0.3 describes but `AgentEvent` (`core/src/agent.rs`) does not have yet: a `limited` event with its reset time, a turn blocked by a hook (with the hook's reason), a hook call (name, event, outcome, exit code), a refused call's cause (hook, host, rule, missing tool or unknown) and a compaction call. Second, `models.yaml` (`~/.config/collide/models.yaml`, 0.3 Who picks what): loaded in `host`, and resolved in `core` by alias for one tool, including `model.name`, with `model.id` kept opaque. Third, the capability refusal. Replaces the stubs tagged `resolve: model alias` and `launch: capability refusal`.

## Success criteria

- A role whose alias is unmapped is refused with a message that names the alias (Stage 3b criterion).
- A mapping whose `model.name` fails the safe token, or a pack's `requires.modelName` pattern, is refused, and the message names the pattern. `model.id` never reaches an env value, a path or argv (0.3, Who picks what).
- `refuse_unmet_capabilities` (`core/src/adapter.rs`) returns every required capability that the adapter says `No` or `Unknown` for, naming the agent and the capability as the role wrote it (0.3, Capabilities).
- The two traits in `core/src/adapter.rs` are checked against S7's P1–P9, and `feed` takes `wall_ns`. Any change this item makes to them is listed in the item.
- Each new event and turn-end reason round-trips through JSON and is in the generated schemas (`pnpm gen`). `KNOWN_EVENT_TYPES` and its schema test agree. An event type outside the list still reads as `Other`.
- `models.yaml` is read from `$HOME/.config/collide/models.yaml`. A missing file means no mappings, so every alias is unmapped. A malformed file fails with its path and the place of the error.
- No stub tagged `resolve: model alias` or `launch: capability refusal` remains. Both tags move from `CORE_OUTLINE_TAGS` to `FILLED_TAGS` in `checks/tests/real_tree.rs`.

## Tests

Write these first:

- alias resolution: mapped, unmapped, a name that fails the safe token, a pack's pattern, and `model.id` kept out of everything but the adapter;
- capability refusal with `Yes`, `No` and `Unknown`;
- a JSON round trip of each new event and turn-end reason;
- `models.yaml` under a temp `HOME`: missing, valid and malformed.

The `should_panic` outline tests in `core/tests/outlines.rs` for the two stubs become real tests.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 3b: the two traits; `models.yaml` loading and alias resolution; the `limited` event.
  - 0.3: Who picks what; Normalized events; A turn ends with; Capabilities; Usage and rate limits; The stream protocol (refused tool calls and hook outcomes); The user's own tooling (the hook-blocked prompt).
  - S7 record: P1–P9.
- **Decision for the item, with a recommendation.** 0.3's list of normalized events leaves out five things that 0.3's other sections and Stage 3b's criteria need: `limited`, a hook-blocked turn, hook calls, refusal causes and a compaction call. Recommended shapes:
  - `AgentEvent::Limited`, with the reset time as wall-clock milliseconds, or none;
  - `TurnEndReason::Blocked { reason }`;
  - `AgentEvent::HookCall`, which never carries the hook's `stdout` or `stderr`;
  - `AgentEvent::ToolRefused { id, cause }`;
  - the compaction call as a `Usage` with its own scope or marker.

  The owner updates the plan's 0.3 list afterwards; the build does not edit the plan.
- Area: `core` (`agent.rs`, `adapter.rs`, `resolve.rs`), plus a small loader in `host` for `models.yaml`. The generated schemas change: run `pnpm gen`.
- The test rule: no test touches the real home, real launchd or a real daemon. A test that reads a file clears the environment (`env_clear()`) and sets `HOME` (and `COLLIDE_DAEMON_ROOT` if a daemon is involved) to a temp root, as `host/tests/common/mod.rs` `TempRoot` does. Files the tests read are fixtures, never the owner's own `models.yaml` or `settings.json`.
- Stage 3b item (revision 18 scope, split 2026-11-15). Items of Stage 3b run one at a time, in id order, because they share `host` and the generated files.

### Built

- `core/src/agent.rs`: the new normalized events, each a closed object: `AgentEvent::Limited { reset_at_ms: Option<u64> }`, `AgentEvent::HookCall { name, event, outcome: HookOutcome, exit_code: Option<i32> }` (no field for `stdout` or `stderr`), `AgentEvent::ToolRefused { id, cause: RefusalCause }`. `TurnEndReason::Blocked { reason: Option<String> }`. `UsageScope::Compaction` for the model call of a compaction. `HookOutcome` is `success`, `error`, `cancelled`; `RefusalCause` is `hook`, `host`, `rule`, `missing_tool`, `unknown`. `KNOWN_EVENT_TYPES` has 12 names. `UserModels` is the shape of `models.yaml` (`tools: { <tool>: { <alias>: { name, id? } } }`) with `mapping_for(tool)`. `Capabilities::support_of(name)` reads one capability by the name a role writes, with its sub-flags.
- `core/src/adapter.rs`: `refuse_unmet_capabilities` is filled. It returns every required capability that is not `Yes`, once each, in the role's order, with the name as the role wrote it. A name the table does not have reads as `Unknown`. `PhaseFailure::Blocked { reason }` is new.
- `core/src/resolve.rs`: `is_safe_token` (0.5) and `resolve_model_alias` are filled. Order: unmapped alias, then the safe token on `name`, then the pack's `requires.modelName`. `ResolveError` gains `UnsafeModelName` and `InvalidModelNamePattern`. `id` is neither checked nor repeated in any error.
- `core/src/testing/mod.rs`: `outcome_of_turn` maps `Blocked` to `PhaseFailure::Blocked`. `core/src/schema.rs`: `UserModels` is in the registry. `core/Cargo.toml`: `regex = "1"` (see decision 6).
- `host/src/models.rs` (new): `models_path(home)` and `load_user_models(home)`. The home value is passed in (the CLI passes `HOME`, as `stop_cli.rs` does), never read here. A missing file is no mappings; a malformed file or a directory in its place fails with `path:line:column`.
- `checks/tests/real_tree.rs`: `launch: capability refusal` and `resolve: model alias` moved from `CORE_OUTLINE_TAGS` (now 8) to `FILLED_TAGS` (now 27). `stub-tags.json` is unchanged, as in earlier items.
- Generated: `pnpm gen` rewrote the `AgentEvent`, `Usage`, `Capabilities`, `AskNotice`, `OwnerNotice` and `RunOutputResult` schemas and types, and added `UserModels`. A second `pnpm gen` changes nothing.
- Tests added: `core/tests/capability_refusal.rs` (11), `core/tests/model_aliases.rs` (16), `host/tests/models_file.rs` (7, every one under a `TempRoot` home), and in `core/tests/agent_events.rs` 7 new tests plus the event lists extended for the three new events and the count of 12. The two `should_panic` stub tests in `core/tests/outlines.rs` became two real tests. `core/tests/fake_adapter.rs` gains a `Blocked` case in `finish_reads_the_outcome_from_how_the_turn_ended`.
- **The two traits against S7's P1 to P9.** P1 (adapter and session, `Step`, `ExitInfo`, `replay`) and P2 (`SessionStart`, `SessionBound`) hold as built. P3 (permission request and reply) and P4 (`Usage` with `quality`) hold. P5 (capabilities with `Yes`, `No`, `Unknown`, and the refusal) holds, and the refusal is now real. P6 (`TurnEnd`, outcome, `FinalReport.source`) holds. P7 (`ModelRef`) holds, and the alias path is now built. P8 and P9 are `host`'s and add nothing to `core`. `feed(&mut self, line: &[u8], wall_ns: u64)` already takes `wall_ns`, and `StampedEvent.wall_ns` carries it. **Changes this item makes to the traits' data:** `PhaseFailure::Blocked` only. Neither trait's method list changed.

### Decisions for the owner

1. `limited`. Chosen: `Limited { reset_at_ms: Option<u64> }`, wall-clock milliseconds, absent when the tool gives none. Alternative: also carry a `status` (`rejected` or `allowed_warning`) so a warning can pause other runs early (0.3, Usage and rate limits). Recommend adding a separate `LimitWarning` event when that behaviour is built, so a warning cannot be mistaken for a pause.
2. A blocked turn. Chosen: `TurnEndReason::Blocked { reason: Option<String> }`, one reason for both `blocked_by_hook` and `stopped_by_hook`, and `PhaseFailure::Blocked { reason }` so the Queue item has the hook's reason. Alternative: keep the kind (`prompt_kept`) apart. Recommend as built; the adapter (ts-0063) can add the kind if the Queue needs it.
3. Hook calls and refusal causes. Chosen: `HookCall` with a closed `HookOutcome` (`success`, `error`, `cancelled`) and an optional `exit_code`; `ToolRefused { id, cause }` with a unit-variant `RefusalCause`. Alternative: `outcome` as free text, which would carry any value a later `claude` adds. Recommend the closed form; an outcome the adapter does not know goes out as an `Other` event, so nothing is relabelled.
4. The compaction call. Chosen: `UsageScope::Compaction`, a separate `Usage` event. Alternative: a marker field on `Usage`. Recommend the scope, because it keeps `Usage`'s fields unchanged and a consumer that sums `Turn` scopes cannot add it by mistake.
5. Shape of `models.yaml`. Chosen: `tools: { claude: { fast: { name: haiku } } }`, closed, using the existing `ModelRef`. Alternative: a flat alias map per tool at the top level. Recommend the `tools:` key, so the file can gain other top-level keys without breaking old files. 0.3 mentions a "model and source" per mapping; the source stays the user's own and has no key (the `ModelRef` is closed).
6. Pattern engine. Chosen: add `regex = "1"` to `core`. It is already in `core`'s build through `cel`, so `Cargo.lock` gains one dependency edge and no new package, and `pnpm check:deps` is clean. Alternative: evaluate the pattern with `cel`'s `matches`, which couples a pack's pattern to the expression engine. Recommend `regex`. A pattern is unanchored unless it writes `^` and `$`, as in JSON Schema.
7. Capability names. Chosen: a name with a mode (`session_resume`, `system_prompt`, `model_select`) is met by any mode but `none` and `unknown`; one with a sub-flag but no mode (`permission_host`, `usage_report`, `context_options`, `mcp_servers`) reads its own `support`. Alternative: refuse a role that names a bare group. Recommend as built. The budget rule (`budget_native` or `usage_report.cost`) is the caller's, because the function takes names that must all be met; its doc says so.
8. A name that is not a capability. Chosen: refused as `Unknown`. Alternative: a separate error. Recommend as built; the pack check should also reject the name at load, when `pack::Role` gets its `require` key.

- Owner decisions (2026-11-15): the recommendations above are accepted as built. The proposed 0.3 text is applied to IMPLEMENTATION_PLAN.md with this item (Normalized events, Usage scopes, `Blocked` turn end), together with the S1 correction that every Claude turn, the first included, goes on stdin. Follow-ups: a `LimitWarning` event for `allowed_warning` when a later item needs it; the budget either-or rule is the caller's, in the launch item.

### Not done, by design

- No `require` key on `pack::Role`, and no caller of `refuse_unmet_capabilities` or `resolve_model_alias` yet: the Claude adapter and the launch path are ts-0061 and later.
- No check of `models.yaml` names at load: a bad `name` is refused when its alias is resolved, so an unused alias cannot block a launch.
- No check that a fan-out's aliases resolve to different models (0.3), which needs the workflow; no `model.id` checks (an ACP id is checked against the agent's list after `session/new`).
- No `PhaseFailure` for a hook-blocked prompt beyond `Blocked`; the Queue item that shows it is later.
- The Claude parser does not emit any of the new events yet (ts-0061 to ts-0064).
- No change to IMPLEMENTATION_PLAN.md. Proposed 0.3 text, replacing the "Normalized events" bullet: "**Normalized events:** message, tool call, permission request, `SessionBound`, `Usage`, `TurnEnd`, `FinalReport`, `Limited { reset_at_ms }`, `HookCall { name, event, outcome, exit_code }` (never the hook's `stdout` or `stderr`), `ToolRefused { id, cause }` with `cause` one of `hook`, `host`, `rule`, `missing_tool`, `unknown`, and `Other`. A compaction call is a `Usage` with scope `Compaction`." And in "A turn ends with": add "`Blocked { reason }` (a prompt a hook refused)" to the list of reasons, and in `Usage` add `Compaction` to the scopes.
