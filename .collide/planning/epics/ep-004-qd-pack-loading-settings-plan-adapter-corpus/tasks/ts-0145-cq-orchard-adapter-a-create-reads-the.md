---
id: ts-0145-cq
kind: task
status: ready
title: "Orchard adapter: a create reads the real plan CLI's id"
owners: []
links: []
---

## Goal

The real `./plan new task ... --format json` gives `data.id` as a string, the full id with its slug. The fake
(`fixtures/orchard-arch/plan`) gives an object (`kind`, `num`, `suffix`, `scope`, `slug`), and `create()` in
`fixtures/packs/orchard/providers/plan/write.ts` reads `id.scope` and `id.num`. So every create through the adapter on
the real CLI is a bad call (`T-undefined-undefined`). Found by the agent run of `ts-0121-xp`'s checklist (2026-11-19),
step 8.

## Success criteria

- The fake prints `data.id` as the real CLI does: a string.
- The adapter takes the short id from it (the `T-NN-NNN` head) and refuses an id of any other shape with a message that
  names the field and what it held.
- A create through the daemon on the fixture tree reports the new task's key, `plan:T-NN-NNN`.

## Tests

- Write first and see red at runtime: the fake changed to the string shape, and the adapter's create test against it
  (red today).

## Notes

- The other fields of the create envelope were not compared field by field at the check; keep the fake's. The real CLI
  wants the proposal as `P<NN>`, which the adapter already sends.
- Ids and shapes only: nothing from the owner's copy goes in the fixture.
- Owner decisions: none recorded yet. The recommendations stand until the owner answers.
