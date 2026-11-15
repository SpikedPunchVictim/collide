---
id: ts-0048-pg
kind: task
status: ready
title: "Feature links, the derived state and reid"
owners: []
links:
  - { type: blocked-by, to: ts-0047-fb }
---

## Goal

Tasks link to the features they build (revision 14). A declared link type gains `target`, `item` or `feature`, which defaults to `item`, so existing declarations validate unchanged. `core` gains the parse of a feature target (`pr-0003-km#F2` into the item id and the feature number, refusing a malformed fragment) and the derived feature state as a pure function of the proposal's status and the categories of the tasks that implement the feature. The states are `planned`, `deferred`, `dropped`, `accepted`, "n of m done" and `done`, as 0.10 defines them. The markdown provider declares `implements` (outward, not blocking, target `feature`). The `link` op takes `implements` to a feature, an `implements` link to a missing feature is a broken link, and `reid` keeps the feature when it rewrites a link. The CLI prints a feature link with its epic, and `collide task <proposal>` shows each feature's derived state and its tasks.

## Success criteria

- `link implements` to `pr-…#F2` writes the link. The same op is refused, and the message names the features the proposal has, for each of these: a feature the proposal lacks (`#F9`), a target that is not a proposal, and an `implements` with no feature. A `blocked-by` with a feature is refused too.
- A hand-written `implements` link to a missing feature lists the task with a counted broken link. The task is not dropped.
- The derived state is right in a table test over the six proposal statuses, each with no tasks, some done, all done and all dropped: `draft` and `proposed` are `planned`; `deferred` is `deferred`; `rejected` and `superseded` are `dropped`; `accepted` with no tasks, or with all of them dropped, is `accepted`; one of three done, the others active, is "1 of 3 done"; and every task that is not dropped being done is `done`.
- `reid` of a proposal rewrites `pr-<old>#F2` to `pr-<new>#F2` in every item that links to it. `reid` of a task leaves the fragments on other targets alone.
- `collide task <proposal>` shows each feature's state and the tasks that implement it. `collide task <task>` prints its feature link as `implements pr-0003-km#F2 (ep-002)`.
- `target` is in the generated schema, and a provider whose link declarations have none validates unchanged.

## Tests

Write first: `core` tests for the target parse (good, no fragment, `#F0`, `#f2`, `#F2x`, two fragments) and the derived state table; provider tests for `link` taken and refused, the broken link counted, and `reid` keeping the fragment; a CLI test for both prints.

## Notes

- IMPLEMENTATION_PLAN.md, 0.8 (Task (cache): a link's `to` may name a feature), 0.10 (Protocol, the `links` capability's `target`; "A proposal's features": `implements`, the derived state, `reid`), Stage 2: Skeleton (Planning contracts, "From revision 14"; the slice's "features" bullet; the criteria's "features (revision 14)").
- Area: `core` (`LinkTypeDecl.target`, the target parse, the derived state), `packs/plan` (the declaration, `link`, the broken-link count, `reid`), `host` (the CLI prints).
- A feature target's fragment is `F` and a number from 1 with no leading zero, as `#F[1-9][0-9]*`.
- Not in this item: `collide sections` and the near-duplicate rule (`ts-0049-kn`); the view (Stage 6a).
- Generated files change: run `pnpm gen` and commit its output, byte-stable on a second run.
- Done when the criteria hold.
