---
id: ts-0156-nt
kind: task
status: ready
title: "Orchard: its tasks are of kind task"
owners: []
links:
  - { type: blocked-by, to: ts-0151-fk }
---

## Goal

An Orchard task has no kind today (`ts-0137-pg`): the provider declares `adr`, `spike` and `question` only. So `collide
tasks --kind task` is refused on a board whose only source is Orchard's, and on a mixed board it lists Collide's tasks
and not Orchard's (`ts-0150-dj`, Decision 1). The owner wants Orchard's tasks to count as tasks.

## Success criteria

- The Orchard plan provider declares the kind `task`, with the statuses it declares for tasks today, and every task it
  lists or gets carries `kind: task`. ADRs, spikes and questions are unchanged.
- `collide tasks --kind task` on a board whose only source is Orchard's lists its tasks and exits 0; on a board with
  Collide's plan too, it lists the tasks of both.
- The listing and the text of `task <key>` show `task` for an Orchard task.
- What reads a task's kind still works: the `ready` view, `decisions-needed`, the roles check (`ts-0135-cx`), `create`,
  `setStatus` by status and by category, and the transitions of a task.

## Tests

- Write first and see red at runtime: the provider test that a listed task has kind `task`, and a daemon test that
  `--kind task` lists an Orchard task on an Orchard-only board.

## Notes

- Owner 2026-11-19: yes, Orchard's tasks count as tasks (answer to `ts-0150-dj` Decision 1).
- Recommendation: the provider declares the kind, since only the source knows what its items are. Alternative: Collide
  reads an item with no kind as a task; rejected, since it says that of every kind-less item of every source.
- The plan's 4c text says of the Orchard provider "(a task has no kind)"; it changes at review.
- Built after `ts-0151-fk`, which changes the same provider.
- Owner decisions: the owner's answer above. The recommendation on how stands until the owner answers.
