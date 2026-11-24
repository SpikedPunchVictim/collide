---
id: ts-0202-ab
kind: task
status: ready
title: "Window: the tools, the manifest and tools/call into the reducer"
owners: []
links:
  - { type: blocked-by, to: ts-0201-us }
---

## Goal

The window answers an agent's `tools/call`: the five tools of 0.11 are defined in TypeScript, the shell lists them
from a generated manifest, and each call becomes a typed reducer action on the Field's real state. S6's reducer and
tool tests are ported.

What exists.
- `app/src/ui/` (`ts-0042-vn`): `actions.ts`, `reducer.ts`, `state.ts`, `validate.ts`. Every tool is a reducer
  action with a snapshot Undo, its input validated against the live state, with the 0.9 limits. 27 tests in
  `app/test/ui-reducer.test.ts`. The reducer still takes Tracks, and `GROUP_BYS` lacks `category`.
- `core/src/ui_tools.rs` `ToolManifest` and its check; `fixtures/ui-tools/manifest.json`. `build_shell` passes an
  empty manifest. The shell answers `initialize`, `ping` and `tools/list` itself and forwards only `tools/call`.
- `app/src/owner/client.ts` `UiToolsRequestEvent`; `ui_tools.reply` in `app/src/owner/methods.ts`.
- From 6a: the Field (`ts-0181-ft`), the layouts (`ts-0187-hb`), the filter bar's chips and CEL (`ts-0190-tz`).
- S6's `src/agent/exec.ts`, `server/mcp.ts`, `src/agentDemo.ts`, `test/agent.test.ts`, `test/mcp.test.ts`, and its
  fixture repo (24 files) with a 12-task epic.

What this adds.
- The tool definitions in `app/src/ui/tools.ts`: name, description, input schema, and `readOnlyHint` for `query`.
  The Field's surfaces only: `set_view`, `show_groups` and `highlight` take `field`; `open` takes `field`, `queue`
  and `experiments`. A call for Tracks or for runs is refused with the supported values listed, until Stage 9. The
  reducer keeps its Tracks code behind a list of built surfaces.
- The manifest, generated from the definitions into a committed JSON file that `build_shell` embeds in place of the
  empty one. A check fails when the file is stale.
- The window's answer: on `ui_tools.request` it reads the `tools/call`, maps it to an `AgentAction` with its source
  from `caller` (agent and model, or `external`), applies it to the live state, and answers with `ui_tools.reply`:
  an MCP result whose text Collide writes, and `isError` on a refusal. No MCP SDK.
- The reducer's Field view becomes the Field's real state: `set_view`'s filter writes the filter bar's chips, and
  `group_by` and `sort` write the layout. `group_by` gains `category` (0.11).
- `query` over the tasks the window holds, with `filter` and `limit`, and no body. `fields`, the cursor and bodies
  are `ts-0207-en`'s.
- S6's 12-task epic as a TS fixture in `app/test/`, and its fixture repo and epic as a board in `fixtures/` (used by
  `ts-0205-qh` and `ts-0212-km`).

## Success criteria

- `tools/list` from the generated manifest names the five tools, and none launches, accepts, edits or deletes.
- Each tool's valid call applies; each refusal applies nothing: an unknown id, an id in two groups, a group with no
  reason, 13 groups, a bad sort key, an unknown surface or subject, and a call for Tracks or runs, with the
  supported values listed.
- Undo restores exactly the replaced state. A lens does not change when a task does.
- A `set_view` call shows in the filter bar as chips, and its CEL is the bar's.
- A `tools/call` through the shell (fake window replaced by the page under test) gets the reducer's answer.
- `pnpm test` and `cargo test --workspace` pass; the manifest check passes.

## Tests

Write these first.
- `app/test/`: the five definitions against `ToolManifest`'s schema; the stale-manifest check.
- The S6 ports: `agent.test.ts` "demo data", "agentSetView on the Field" (the apply and Undo case), "lens" (the
  stamp and Undo case; the Tracks lens as a refusal), "highlight and open", "tools run against state", "tool input
  errors"; the three Tracks cases as refusals. `mcp.test.ts` "tools/list names the five generic tools" and "returns
  the page error for a bad call, and changes nothing".
- The window's handler over a fake owner: a `ui_tools.request` in, a `ui_tools.reply` out, for a valid call and a
  refusal; an `external` caller's source.
- `set_view` to chips and CEL, over the filter bar of `ts-0190-tz`.

## Notes

- IMPLEMENTATION_PLAN.md:
  - 0.11: The five tools; A view and a lens are two things; Chips. 0.9: an agent's tool inputs.
  - 0.2: the manifest. 0.15: typed reducer actions.
  - Stage 6b: The Field's tools; Success Criteria (S6's 47 agent tests). Part L.
- Builds on `ts-0181-ft` (the Field), `ts-0187-hb` (layouts) and `ts-0190-tz` (the filter bar), and names them.
- **Decision for the item, with a recommendation: the MCP layer.** Recommend no SDK: the shell already answers
  everything but `tools/call`, so the window writes a `CallToolResult` by hand. Alternative:
  `@modelcontextprotocol/sdk` 1.32.2 with zod 4.6.7, as S6 used; a new npm dependency, so an Owner's choice if
  picked. The plan says the window answers "each JSON-RPC message"; the split proposes text that matches 0.2.
- **Owner's choice, recommendation taken (2026-11-24): filters the chips have no group for.** 0.11's task filters
  include `status` and `ids`; the bar's groups are stage, category, source, labels and search. Recommend a chip for
  each ("Status: ...", "Only these N tasks") that writes its CEL term. Alternative: refuse them on the Field.
- **Decision for the item, with a recommendation: the generator.** Recommend a script that writes the JSON from
  `tools.ts`, with a vitest check that compares them. Alternative: a hand-written JSON file and a check only.
- Area: `app/src/ui/`, `app/src/components/field/`, `app/src-tauri/src/lib.rs`, `app/test/`, `fixtures/`.
- Dependencies: none new.
- The test rule: no test touches the real home, real launchd, a real daemon, the keychain or the network. Each test
  that runs the `collide` binary clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to
  a temp root, as `host/tests/common/mod.rs` `TempRoot` does. A temp repository sets `GIT_CONFIG_GLOBAL` and
  `GIT_CONFIG_SYSTEM` to empty files. Keychain writes go through `MemorySecretStore` (`host/src/secret_store.rs`),
  never the login keychain. Browser tests run the installed Chrome with a temp profile (`app/browser/support/
  chrome.ts`), and the bridge's daemon on a temp root (`app/browser/support/world.ts`), with fake agents only. A test
  never stubs or bypasses a safety gate: the real-agent opt-in, the confinement probe, `env_clear`.
- Stage 6b item (revision 26 scope, split 2026-11-24). Items of Stage 6b run one at a time, in id order, because they
  share `app/`, `core`, `host` and the generated files. No builder runs `tauri build`.
- Nothing here runs a real `claude`, `opencode` or any model, or spends money.
- Owner decisions (2026-11-24): the owner took the recommendations at the split.
