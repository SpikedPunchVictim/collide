---
id: ts-0144-eh
kind: task
status: ready
title: "Ready view: a blocker named by its short id is the task it names"
owners: []
links: []
---

## Goal

On the Orchard board `collide tasks --ready` lists no task and prints 1,022 lines "blocked by T-NN-NNN, which is not an
item" for 380 targets, each a real task. `readiness` in `core/src/ready.rs` matches a `blocked-by` target against
`task.id`. An Orchard task's id is its file stem, the short id and its slug (`ts-0110-ra`), and its links name the short
id. The closure resolves the same links through the corpus names (`ts-0118-wp`), so it works. Found by the agent run of
`ts-0121-xp`'s checklist (2026-11-19).

## Success criteria

- The ready rule resolves a link's target as the closure does, with one shared resolver.
- On the fixture `basic` tree: a task whose blocker is done is ready, one with an open blocker is not, and only a target
  that names no task is reported unknown.
- Collide's own plan gives the same ready set as today.

## Tests

- Write first and see red at runtime: readiness over the fixture tree with short-id links lists the task whose blocker
  is done (today: none).
- Mutation at review: matching by `task.id` again turns the test red.

## Notes

- Recommendation: the corpus's name map, the one `follow` walks. Area: `core/src/ready.rs`, `core/src/corpus.rs`,
  `core/src/view_builtin.rs`.
- Owner decisions: none recorded yet. The recommendations stand until the owner answers.
