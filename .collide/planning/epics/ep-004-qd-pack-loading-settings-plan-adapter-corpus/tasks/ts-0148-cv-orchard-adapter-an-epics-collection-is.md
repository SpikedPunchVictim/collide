---
id: ts-0148-cv
kind: task
status: ready
title: "Orchard adapter: an epic's collection is its tasks' epic field"
owners: []
links: []
---

## Goal

`"plan:<epic>" in collections` gave 452 tasks on the Orchard board: the 428 whose `epic:` names it (the same 428 as its
generated table), and 24 named in other tables of the epic file, which belong to other epics. `readCollections` in
`fixtures/packs/orchard/providers/plan/collections.ts` puts a task in an epic when its own field says so, or when any
table of the epic's file names it. The criterion "`"plan:E-07" in collections` returns the same tasks as the prototype
board's E-07 epic" fails. The board page shows 645, adding ids named in prose sections; that is not a collection. Found
by the agent run of `ts-0121-xp`'s checklist (2026-11-19), step 3.5.

## Success criteria

- An epic's collection is the tasks whose `epic:` names it, bare or with its slug: the generated table's set.
- A task named only in another table of the epic's file is not a member.

## Tests

- Write first and see red at runtime: a fixture epic file with a second table that names a task of another epic (today:
  a member).

## Notes

- Recommendation: the field alone. The alternative is to keep "any table" and change the criterion. A second relation
  ("named in an epic") can come later if a view needs it.
- This changes what a collection holds; the owner may pick otherwise. Owner decisions: none recorded yet. The
  recommendations stand until the owner answers.
