---
id: ts-0020-tt
kind: task
status: ready
title: "Planning slice: task CLI"
owners: []
links:
  - { type: blocked-by, to: ts-0014-yc }
---

## Goal

`collide tasks [--ready] [--kind <kind>] [--json]`, `collide task <id> [--json]`, `collide task <id> <op> …`. `<id>` may be the short form (`12` or `12-ua`) when it matches one item. The CLI starts the provider itself until the daemon exists.

## Success criteria

- `collide tasks --ready` equals the ready list checked by hand.
- With two tasks numbered 12, `collide task 12` lists both and asks for the suffix.

## Tests

- The ready rule: blocked, unblocked by `done` and by `dropped`, a `blocked-by` cycle.
- Short-form lookup: one match, several matches, none.
