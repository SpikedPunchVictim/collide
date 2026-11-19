---
id: ts-0153-kp
kind: task
status: ready
title: "Orchard check: the fake CLI is held to the real one's envelopes"
owners: []
links:
  - { type: blocked-by, to: ts-0145-cq }
  - { type: blocked-by, to: ts-0148-cv }
  - { type: blocked-by, to: ts-0150-dj }
---

## Goal

The fake plan CLI (`fixtures/orchard-arch/plan`) drifted from the real one and every test still passed: the create
envelope's `data.id` was an object where the real CLI prints a string (`ts-0145-cq`), and `ts-0150-dj` lists more. Drift
is found only when an owner check runs the adapter on a real clone. Record the shape of the real CLI's envelopes once,
and test the fake against those shapes, so drift fails the gates instead.

## Success criteria

- A shape reader in the fixtures turns one run of the CLI into its shape: exit code, whether stdout and stderr are
  empty, stderr's line count, and the JSON envelope as its key tree with each leaf's type. A string leaf is named by the
  reference pack's id pattern it fits (`ids.ts`), or `string`. Values are kept only where they are the CLI's own words:
  `command`, `schema_version`, a status the pack declares.
- A recorder script runs, against a plan CLI and a root given as arguments, each command the adapter calls: `status`,
  `set` (a legal move and an illegal one), and `new task`. It writes one shape file per case under
  `fixtures/orchard-arch/contract/`. It names no path of its own.
- The shape files come from the real CLI, recorded once (Notes). They hold no title, id, slug or other text from the
  real repo, and pass the denylist.
- A test runs the fake through the same cases on the fixture tree and fails, naming the case and the path in the
  envelope, when its shape differs from the recorded one.
- Every difference the recording shows is fixed in the fake (and in the adapter if it read the wrong shape), or filed as
  an item, before this item is done.

## Tests

- Write first and see red at runtime: the shape test over a shape file edited by hand to differ from the fake (a leaf's
  type, a missing key), red; restored, green.
- The shape reader: unit tests for a leaf per type, an id-pattern string, a kept constant, an array of objects.

## Notes

- Owner 2026-11-19: file this item (yes), on the reviewer's offer to hold the fake to recorded real envelopes.
- Order of work. The builder writes the shape reader, the recorder and the test, and records the fake's own shapes as a
  first set. The builder does not run the real CLI. The reviewer then runs the recorder against the real CLI and puts
  its shape files in place; the diff against the fake's set is the drift. The builder is resumed to fix it.
- The recording run follows the rules of the agent run of `ts-0121-xp`: a temp clone with `--no-hardlinks` of the
  owner's copy, a temp `HOME`, the plan CLI's packages installed inside the temp clone, write commands only on a made-up
  task in the temp clone, the owner's copy never written, the temp clone removed by exact path after. The copy's path
  and branch names go in no tracked file, commit or item.
- Recording again is by hand when the real CLI changes; the shape files carry the date they were recorded.
- Owner decisions: none recorded yet. The recommendations stand until the owner answers.
