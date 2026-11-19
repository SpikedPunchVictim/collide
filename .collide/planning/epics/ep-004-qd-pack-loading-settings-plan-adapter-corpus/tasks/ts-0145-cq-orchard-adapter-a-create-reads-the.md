---
id: ts-0145-cq
kind: task
status: done
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

### Built

- `fixtures/orchard-arch/plan`, `commandNewTask`: `data.id` is now the string `T-<scope>-<num>-<slug>`, as the real
  CLI prints it (the slugs are the fake's own, from the title). Every other field of the envelope is as it was.
- `fixtures/packs/orchard/providers/plan/write.ts`, `create`: reads `data.id` as a string, checks it against
  `ID_PATTERN` (the full id with its slug), and takes the short id as its first eight characters (`T-NN-NNN`). Any
  other value is an error reply: `./plan gave data.id as <the value as JSON, or nothing>, not a task id with its slug`.
  The old build of `T-<scope>-<num>` from an object is gone.
- Readers of the fake's create output, all moved: the adapter (above); `fake-plan.test.ts` (three tests read the old
  object: `makes the next task of the proposal...`, `skips a number the id map names...`, `takes no number that the id
  map names...`). The other create tests (`write.test.ts` create block and previews, `provider-stdio.test.ts`) go
  through the adapter and read the key, so they needed no change. No Rust test creates through the fixture pack, and
  nothing in `core`, `host`, the app or the SDK was touched.
- Tests, `fixtures/arch-plan-tests/test/write.test.ts`, block `the id in the CLI envelope` (a one-line shell script
  stands in for `./plan`, run through `preview` so no file is needed):
  `the short id is the head of the full id the CLI printed`, and eight cases of `<shape> is an error reply that names
  data.id and what it held` (an object, a short id with no slug, a number, an empty string, a slug in capitals,
  another kind of id, nothing).
- Seen red at runtime before the code change (fake and adapter as they were): all three changed `fake-plan.test.ts`
  tests, and all eight new `write.test.ts` tests (the object case was accepted as `ok`, the others were refused
  with the old message that does not name `data.id`). All green after.

### Decisions for the owner

1. The adapter checks the whole id with `ID_PATTERN`, not just the head. Alternative: check only the head
   (`SHORT_ID_PATTERN` on the first eight characters), which would accept `T-03-009` with no slug. The item says the
   real CLI gives the full id and asks for a refusal of "any other shape", so the full check is the smaller reading.
   Recommend as built.
2. The message names the field as `data.id` and shows the value as JSON (`nothing` when absent). Alternative: quote
   the whole envelope. Too long. Recommend as built.
3. A create through the daemon is covered by the node-child test in `provider-stdio.test.ts` (key `plan:T-03-006`),
   not by a new Rust test: no daemon test uses the Orchard fixture pack, and the Rust side must not name Orchard (0.1).

### Not done, by design

- `ts-0149-kq` (short ids, `epic=` and `task create` on a board whose plan is not Collide's) and `ts-0148-cv` (an
  epic's collection from its tasks' `epic` field) are untouched. Neither is made trivial or changed by this item.

### At review

- Decisions 1 to 3 taken as recommended (owner delegation). The plan names no create envelope, so no plan text changed.
  Nothing outside the reference pack's fixture copy and the fake changed (0.1).
- `ID_PATTERN` fixes the short id at two and three digits, so taking the first 8 characters is the same as its head. The
  `as string` cast stays: `badCall` is a `never` arrow const, which TypeScript does not narrow through.
- Mutations, each red then restored: any string accepted (four refusal cases), the fake back to the object shape (three
  fake tests), a 7-character head (every create test), the message without the value (every refusal case).
- Drift between the fake and the real CLI is still found only by an owner check; a contract check of the fake against
  recorded envelopes was offered to the owner on 2026-11-19.
