---
id: ts-0187-hb
kind: task
status: ready
title: "App: the run order and graph layouts"
owners: []
links:
  - { type: blocked-by, to: ts-0186-ch }
---

## Goal

The Field lays a view out three ways: board, run order and graph (note 11). Run order shows the steps of the view's
closure, numbered. The graph shows the same steps as columns, with one line for each blocking link among the shown
tasks, and lights a task's lines while the pointer is on it.

What exists.
- `core/src/view.rs`: `Closure { goal, follow: DependsOn }` (one goal task), `LayoutKind { Board, RunOrder }` (no
  graph), `Layout { default, group_by, sort }`.
- `ViewReport.steps` (`core/src/owner/task.rs`): the closure's tasks in steps, as `collide tasks --view` prints
  them, and `cycle`.
- `ViewRef::Inline` lets the app ask `task.list` for a view it made.
- From `ts-0181-ft`: the Field and the view picker. From `ts-0186-ch`: `unblocks`.

What this adds.
- `LayoutKind::Graph` in core, the schema and the generated files, so a view file may say `layout: { default:
  graph }`.
- A layout switch on the Field (Board, Run order, Graph), starting at the view's `layout.default`, kept per board
  and view in webview storage.
- Run order: the goal bar ("Toward <goal>", then "N open tasks in M steps. Only open tasks are walked; finished
  ones are left out."), then the steps from `ViewReport.steps`, numbered, each with its cards.
- Graph: the same steps as columns, drawn in hand-written SVG, one line for each blocking link between two shown
  tasks. Hovering or focusing a task lights its lines and dims the rest. A cycle is named above the graph from
  `ViewReport.cycle`, and its tasks are drawn with no lines between them.
- The goal: a view with a closure uses its goal. For a view with none, the goal bar's picker follows the Owner's
  choice below.
- A card in either layout selects its task for the drawer, as on the board.

## Success criteria

- **Run order and graph** (note 11): for a view with a closure on the fixture board, both show the tasks and steps
  of `collide tasks --view`, and the graph draws one line for each blocking link among them.
- Hovering a task in the graph lights exactly its lines.
- A view with `layout: { default: graph }` opens in the graph.
- A cycle in the closure is named, and the layout still draws.

## Tests

Write these first.

- `core/tests/`: `layout.default: graph` reads; an unknown layout is still refused.
- `app/test/`: steps and lines from a `FakeOwner` world, a cycle, a link to a task not shown draws no line.
- `app/browser/fake-graph.spec.ts`: hover and focus light the lines.
- `app/browser/run-order.spec.ts` (bridge): the edge-mvp view (`fixtures/views/edge-mvp.yaml`, goal
  `plan:T-01-042`) over a plan board built by the test; steps and line count against `collide tasks --view edge-mvp
  --json`.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 6a: Field ("board, run-order and graph layouts"; "The graph lights a task's lines"). Success Criteria: Run
    order and graph. Tests: "run order and graph against the closure".
  - 0.10: Views (`closure`, `layout`). 0.11: the Field's layouts.
- Design: note 11. Mock functions: `runOrderHtml`, `graphHtml`, `lightGraph`, `closureSteps`, `goalBar`,
  `pickGoal`, `goalsOf`.
- **Owner's choice, recommendation taken (2026-11-24): the goal of a view with no closure.** The mock's goal bar picks
  an epic among the shown tasks ("Toward <epic>") for any view; the plan's closure takes one goal task, set in the view
  file. Recommend the mock's picker: core accepts a collection key as a closure goal (all its open tasks are goals), and
  the app asks `task.list` for the shown view with that closure added inline. Alternatives: a goal-task picker instead
  of an epic picker; or run order and graph only for views whose file sets a closure, with the switch disabled
  elsewhere.
- **Decision for the item, with a recommendation: drawing.** Recommend hand-written SVG with the steps as columns
  and straight or one-bend lines, as the mock draws. No graph library. Alternative: a layout library (a new
  dependency, an Owner's choice).
- Area: `core/src/view.rs`, `core/src/view_eval.rs` (if the collection goal is taken), `schemas/`, the generated
  files, `app/src/` (the layouts), `app/test/`, `app/browser/`.
- Dependencies: none.
- The test rule: no test touches the real home, real launchd, a real daemon, the keychain or the network. Each test
  that runs the `collide` binary clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to
  a temp root, as `host/tests/common/mod.rs` `TempRoot` does. A temp repository sets `GIT_CONFIG_GLOBAL` and
  `GIT_CONFIG_SYSTEM` to empty files. Keychain writes go through `MemorySecretStore` (`host/src/secret_store.rs`),
  never the login keychain. Browser tests run the installed Chrome with a temp profile
  (`app/browser/support/chrome.ts`), and the bridge's daemon on a temp root (`app/browser/support/world.ts`).
- Stage 6a item (revision 25 scope, split 2026-11-24). Items of Stage 6a run one at a time, in id order, because they
  share `app/`, `core`, `host` and the generated files. No builder runs `tauri build`.
- Nothing here runs a real `claude`.
- Owner decisions: none recorded yet. The recommendations stand until the owner answers at the split.
