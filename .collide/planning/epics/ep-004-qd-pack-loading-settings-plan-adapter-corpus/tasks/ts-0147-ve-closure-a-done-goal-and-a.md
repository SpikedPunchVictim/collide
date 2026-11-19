---
id: ts-0147-ve
kind: task
status: ready
title: "Closure: a done goal and a missing goal as the prototype generator has them"
owners: []
links: []
---

## Goal

The agent run of `ts-0121-xp` settled `ts-0118-wp`'s two closure decisions against the prototype board's generator, by
the checklist's own rule. A done goal: the generator walks nothing (no open run; the former dependencies are listed as
stranded), while Collide gives the goal and its 13 open dependencies. A goal that names no task: the generator stops
with an error (exit 2, no task file carries that id), while Collide gives an empty closure and `goal_unmatched`.

## Success criteria

- A done goal: the closure holds the goal alone; nothing is walked from it, and it is no step.
- A goal that names no task, in a view whose members come from the closure alone: a view error (exit 10, the message
  names the goal), no members.
- A view with a `where` beside the closure keeps today's rule for a missing goal: its other members, and
  `goal_unmatched`.
- Plan 0.10 Views says the two rules.

## Tests

- Write first and see red at runtime: one test per rule, on the fixture tree.

## Notes

- Recommendation: the three rules above. The alternative for a done goal is an empty set, which hides the goal itself;
  the goal alone keeps it on the board.
- This changes what a view shows; the owner may pick otherwise. Owner decisions: none recorded yet. The recommendations
  stand until the owner answers.
