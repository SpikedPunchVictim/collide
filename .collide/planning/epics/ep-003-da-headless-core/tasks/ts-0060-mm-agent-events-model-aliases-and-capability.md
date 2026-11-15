---
id: ts-0060-mm
kind: task
status: ready
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
