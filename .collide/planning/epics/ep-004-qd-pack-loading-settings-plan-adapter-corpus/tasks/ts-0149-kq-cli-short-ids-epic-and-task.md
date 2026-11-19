---
id: ts-0149-kq
kind: task
status: ready
title: "CLI: short ids, epic= and task create on a board whose plan is not Collide's"
owners: []
links:
  - { type: blocked-by, to: ts-0145-cq }
---

## Goal

On the Orchard board `collide task <goal>` is refused ("`<goal>` is not an item id; use the full id (ts-0012-ua)
..."): the short-id resolver (`resolve` in `host/src/task_daemon.rs`) knows Collide's id shape only. `task create ...
epic=E-73` is refused the same way. `create` takes only the Collide plan's arguments (`title`, `epic`, `kind`, `seed`,
`root`, in `host/src/task_cli.rs`), so an Orchard task, which needs a proposal, cannot be made from the CLI. Found by
the agent run of `ts-0121-xp`'s checklist (2026-11-19), steps 2 and 8.

## Success criteria

- A bare id that is exactly the short id of one item of any source names that item; two sources with the id are
  ambiguous, as today.
- `epic=<id>` resolves among a foreign source's epics the same way.
- `task create` passes the arguments the source's `create` op declares, and refuses any other with that list.

## Tests

- Write first and see red at runtime: the three on the fixture Orchard board (today: refused).

## Notes

- Recommendation: an exact match among every source's ids before Collide's short forms; create's arguments from the op's
  declaration.
- Owner decisions: none recorded yet. The recommendations stand until the owner answers.
