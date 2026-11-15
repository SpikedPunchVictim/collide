---
id: ts-0042-vn
kind: task
status: done
title: "UI reducer and agent tool actions"
owners: []
links:
  - { type: blocked-by, to: ts-0032-pp }
  - { type: blocked-by, to: ts-0037-vg }
---

## Goal

In `app/`, the one reducer for UI state, in TypeScript with no React import. Every user action is a typed reducer action. So is every call of the in-app agent's five tools (`query`, `set_view`, `show_groups`, `highlight`, `open`), each with the state it replaced stored as a snapshot, so Undo restores exactly that and undoes the last change only. `query` changes no state. A payload the reducer cannot accept leaves the state unchanged and returns an error value. The tools' definitions and the validation of their inputs are Stage 6 and 6b. Here the action types, the snapshot and Undo, and the state shape for the surfaces are real.

## Success criteria

- A reducer test drives one user action and one agent tool-call action without a browser.
- After a `set_view`, `show_groups`, `highlight` or `open` action, Undo restores the previous state exactly, and a second Undo does nothing more.
- `query` leaves the state equal to before.
- Every action type in the union has a reducer branch (an exhaustiveness check that fails `tsc --noEmit` if one is missing).
- A rejected payload leaves the state unchanged.

## Tests

Write first: the two driving tests; the Undo-restores-exactly test per tool; the exhaustiveness check; the rejected payload.

## Notes

- IMPLEMENTATION_PLAN.md, 0.15 (Pass 1: one reducer, every user action a reducer action, and so every in-app agent tool call, with a snapshot for Undo), 0.11 (the five tools; Chips; Undo), 0.9 (an agent's tool inputs are untrusted), Stage 2: Skeleton (the React tree and the reducer).
- Area: `app/` (the reducer and its types).
- Done when the criteria hold.
- Built:
  - `app/src/ui/`: `state.ts` (`UiState`, `ScreenState`, `FieldView`, `TracksView`, `Lens`, `Highlight`, `Chip`, `UndoSnapshot`, `Corpus`, `initialState`), `actions.ts` (`UiAction` = `UserAction` | `AgentAction`, `ActionType`, `ActionResult`, `ActionError`), `validate.ts` (the parsers for an agent's untrusted input and the 0.9 limits), `reducer.ts` (`applyAction` and the `reducer` form for `useReducer`), `index.ts`. No React import, no new dependency.
  - Every user action and each of the five tools is a member of the union (`user/corpus_loaded`, `user/surface_shown`, `user/task_toggled`, `user/filter_set`, `user/lens_cleared`, `user/undo`; `agent/query`, `agent/set_view`, `agent/show_groups`, `agent/highlight`, `agent/open`). An agent action carries `{source, at, input: unknown}`: the input is untrusted and is validated against the state (keys, surface, enumerations, sort keys, phase names, clock window, 1 to 12 groups, at most 500 ids, an id in one group only, every id known to the window), and the agent's words are cut and cleaned. A refusal returns `{ok: false, state, error}` with the very same state object and never throws.
  - A screen-changing call stores the `ScreenState` it replaced in `undo` and adds a chip whose statement is built from the validated action. Undo restores that snapshot, marks the chip `undone` and drops the snapshot, so a second Undo returns the same state. A valid call that changes nothing leaves no chip and no snapshot, so it cannot hide the Undo of the call before it.
  - Exhaustiveness: the `default` branch of `applyAction` assigns `action` to `never`; removing the `agent/open` branch fails `tsc --noEmit` (shown: `Type '{ readonly type: "agent/open"; ... }' is not assignable to type 'never'`), and the test's `satisfies Record<ActionType, UiAction>` makes a new action type fail `pnpm typecheck` until it has a sample.
  - Tests, written first (red on the missing module), `app/test/ui-reducer.test.ts` (63 tests): the two driving tests (a user action and an agent call, no browser); Undo restores the screen exactly, and a second Undo does nothing more, for each of `set_view`, `show_groups`, `highlight` and `open`; Undo undoes the last change only; `query` leaves the state identical; a branch per action type, and an unknown type refused as a value; 26 rejected payloads each leaving the state identical (checked by identity) with the message the agent would see; the agent's words cut and the chip statement free of them. Each criterion's test was seen failing on its own assertion by breaking the reducer four ways (Undo keeps the snapshot, `query` changes state, a refusal returns a copy, Undo restores the current screen), then restoring it.
  - Gates: `pnpm test` 82 app tests (6 files) plus 66 and 283 in the other packages, `pnpm typecheck`, app build, `cargo build`, `cargo test` (1295 passed, 0 failed), clippy `-D warnings`, `cargo fmt --check`, `pnpm check` all pass; `collide tasks --json` shows 55 tasks, 0 invalid.
- Decisions for the owner:
  - (1) The reducer's corpus is only ids and phase names, loaded by `user/corpus_loaded`, because 0.9 validates ids against the window's live state; the alternative is for Stage 6 to pass the full rows in, which this item does not need. Before the corpus is loaded every id is unknown.
  - (2) `applyAction` returns `{ok, state, changed | error}` and `reducer` wraps it for React; the alternative is a reducer that returns only state, which would lose the error value the agent must be told.
  - (3) Undo restores the snapshot exactly even if a person edited the view after the agent's change (their edit is lost); the alternative is to drop the snapshot at the next user view edit, which the plan does not say.
  - (4) A call that is valid but changes nothing makes no chip and no snapshot; the alternative is a chip for every call, which would let a repeated call overwrite the Undo of a real change.
  - (5) Filter values are arrays of strings (`text` is one string), and `filter` on `set_view` replaces the whole filter of that surface, with `{}` clearing it; the alternative is merging key by key, which would leave no way to remove one key.
  - (6) `open` with an id on Queue or Experiments checks only its shape (a string of at most 200 characters), since those surfaces have no corpus here; with no id, the surface's open item is left as it is.
  - (7) `show_groups` and `highlight` replace the lens or highlight of that surface; the alternative is to stack several, which the plan does not describe. `rest` defaults to `dim`.
  - (8) A `set_view` that sets nothing (no filter, sort, group_by or window) is refused, and so are `group_by` on Tracks and `window` on the Field; the alternative is to ignore the part, which the plan's "invented values are rejected, never repaired" argues against.
  - (9) The tasks a person selected (`selectedTasks`) and the corpus are outside the snapshot, so Undo never changes them.
- Not done by design: the tools' definitions, the manifest and the mapping from an MCP `tools/call` to an action (Stage 6, 6b); `query`'s rows, paging and body rules, which need the corpus rows (Stage 6); the per-ask cap of 20 screen-changing calls, kept by the shell (Stage 6b); the "No files were read" flag and the ask box's failure texts (Stage 6b); Save as View and Save snapshot (Stage 6); the status vocabulary check on `status` filter values; rendering of chips (ts-0043-fr); the item's `status`, `links` and `owners`.
