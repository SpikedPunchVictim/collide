---
id: ts-0042-vn
kind: task
status: ready
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
