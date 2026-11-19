---
id: ts-0148-cv
kind: task
status: done
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

### Built

- `readCollections` in `fixtures/packs/orchard/providers/plan/collections.ts`: a task is in an epic only when its own
  `epic:` names it (bare or with the slug; `epicRef` already reads both). The epic's tables no longer add members.
  Groups are unchanged.
- `readEpicTables` in `fixtures/packs/orchard/providers/plan/epic.ts`: the `members` field is gone, with the code that
  filled it (generated table rows, slice and stream ids). Nothing else read it. The generated table is skipped, since
  it gives no group.
- Tests in `fixtures/arch-plan-tests/test/list.test.ts`, `collections` block:
  - `plan:E-07 is on exactly the tasks whose epic: names it, not those another table names` (red at
    runtime: `plan:T-05-003` was a member of E-07, named in its slice table).
  - `a task carries the one epic its epic: names, and no slice or stream` (red at runtime: `T-05-003` carried E-07 and
    E-08).
  - `a task that a table of another epic names stays out of that epic, its group lists it` (red at
    runtime: a made-up `Streams` table added to the E-08 file names `T-03-001`, an E-07 task; it was a member of E-08).
    Built on a copy of the `basic` tree, with made-up ids.
  - `a task whose epic: is bare (E-08) or has the slug (E-08-signal-lamps) belongs to that epic` (green before and
    after; it pins the rule).
  - The existing "epic: names the epic even when no table lists it" test was renamed to say "no table" and kept.
  - The old tests "exactly the tasks the epic tables list" and "carries each epic that names it" asserted the old rule;
    they were rewritten as the first two above.
- Red was seen against the code before the change (the tests were written first, then run).

### Decisions for the owner

1. The field alone, as the item recommends (owner delegation; none recorded otherwise). Alternative: keep "any table"
   and change the criterion. Recommendation: the field alone.
2. Groups keep the tasks their tables name, so E-07's `Slice 2` group still lists `T-05-003` although that task is
   not a member of E-07. Alternative: filter each group to the members. Recommendation: leave it for now; a group is
   the epic file's order, and a view that wants only members can intersect with `"plan:E-07" in collections`. Filtering
   is a one-line change if the board shows it badly.
3. `members` removed from `EpicTables` rather than left unused. Alternative: keep it for the "named in an epic" relation
   the item says may come later. Recommendation: remove it; add it back when a view needs it.
4. Proposed plan text (0.10, Collections, the orchard bullet; not applied): "orchard epics, with the slices and streams
   from the epic file's tables (as the prototype board reads them); a task is in an epic when its own `epic:` names it,
   and a task a table names that belongs to another epic is not a member."

### Not done, by design

- The fixture text "Its tables name the tasks" in the `basic` epic file and "E-07's tables name it too" in
  `T-05-003` still read fine under the new rule and were left, so no tree hash moves.
- `ts-0151-fk` (a changed item keeps the collections of its last listing) is not touched. It is unchanged in scope:
  collections are still computed in `readCollections`, now from one source only.
- `ts-0153-kp` is not touched.

### At review

- Decisions 1 to 3 taken as recommended (owner delegation). Decision 2 (groups unfiltered) stands: groups are the file's
  tables as the prototype board reads them, and nothing in core or host reads a collection's groups yet. A review
  mutation pins it.
- Plan 0.10 Collections: the orchard bullet now says membership is the task's own `epic:`, and a table's group may list
  a task that is not a member; applied at review from the builder's proposed sentence.
- The slug case is real: `T-05-001`'s file has `epic: E-08-signal-lamps`, and its field reads `E-08`.
- Mutations, each red then restored: tables add members again (three tests), tables alone give membership (five), groups
  filtered to members (two).
