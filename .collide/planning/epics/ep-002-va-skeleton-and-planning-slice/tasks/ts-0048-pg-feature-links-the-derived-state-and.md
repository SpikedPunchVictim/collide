---
id: ts-0048-pg
kind: task
status: done
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
- Built: `core` has `LinkTarget` (`item` or `feature`, default `item`) as `LinkTypeDecl.target`, left out of the output when it is `item`, so a declaration with none validates and serializes as before. The new `core/src/feature.rs` holds `parse_feature_target` (`<item>#F<n>`, with `FeatureTargetError` for no fragment, no item, two fragments and a bad fragment, which is named) and `derived_state(proposal_status, task_categories)` returning `FeatureState` (`planned`, `deferred`, `dropped`, `accepted`, "n of m done", `done`; `Display` prints the words). In `packs/plan`: `item.ts` holds `LINK_TYPES`, with `implements` declared outward, not blocking, target `feature`; `feature-target.ts` is the TypeScript parse, with `namesItem`; `links.ts` decides whether a link is broken, and `scan.ts` counts it (a feature link is broken unless its proposal has the feature; the task is still listed); `write.ts` `link` takes `type=implements` to `pr-…#F<n>` and refuses a missing feature, a target that is no proposal, a missing feature part and a `blocked-by` with a feature, each naming what the proposal has or what the target is; `frontmatter-edit.ts` has `addLinkLine(text, type, to)` in place of `addBlockedByLine`; `reid.ts` and `replaceLinkTarget` keep what follows the `#` of every link they rewrite. `host/src/task_view.rs` prints a feature link as `implements pr-0003-km#F2 (ep-001)`, and each feature of a proposal with a `state` line and one line per implementing task; `task_cli.rs` help mentions both. Tests: `core/tests/planning_contracts.rs` (target serde and schema, the parse table, the state table over the six statuses), `provider_protocol.rs`, `packs/plan/test/feature-links.test.ts`, `host/tests/task_cli.rs`.
- Decisions for the owner: (1) the proposal print shows `state  <state>` under each feature, then `<task> (<epic>)  <status>  <title>` for every implementing task, a dropped one included (it is left out of the count only); the plan fixes the states, not the layout. (2) A feature link whose proposal exists but lacks the feature prints `(feature not found)`, and a missing proposal prints `(not found)`. (3) `link` refuses `implements` to a proposal's own feature, and to an item that is not a proposal with "<id> is a task, not a proposal"; it does not list proposals, since the plan's "names the features the proposal has" has no proposal to name there. It does not limit which kinds may carry `implements`, as `blocked-by` is not limited either. (4) A hand-written `implements` with no fragment, a malformed one, or a `blocked-by` with a fragment is a broken link, like a missing feature. (5) `reid` rewrites a link whose target is the old id followed by `#` and anything, so a malformed fragment is kept as it was, not judged. (6) `derived_state` takes the status word and returns an error for a word that is none of the six; a count of none done over a non-empty set prints "0 of n done". (7) The feature states are not in `collide task --json`; the print computes them. The view (Stage 6a) can take them from `core`.
- Not done by design: `collide sections` and the near-duplicate rule (`ts-0049-kn`); the view (Stage 6a); the feature ops (`addFeature`, `moveFeature`, `renameSection`; Stage 4c); the person-only rule and `by:`.
