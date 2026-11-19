---
id: ts-0129-uv
kind: task
status: ready
title: "Plan pack: proposal and question transitions, by: and resolve"
owners: []
links:
  - { type: blocked-by, to: ts-0128-bb }
---

## Goal

A proposal moves only along the moves 0.10 allows. Accepting writes `by: human:<date>`, a question is resolved by an op
that records who and what, and the provider refuses what an agent may not do.

What exists. A proposal has six statuses (`PROPOSAL_STATUSES` in `packs/plan/src/item.ts`), and `setStatus` moves it to
any of them, refusing only `proposed` while the Features part has a problem (`proposedRefusal` in `write.ts`). There
is no transition table, no `by:`, no `defer` trigger and no `resolve`. From `ts-0127-js` a question exists with the
`resolved_by` rule on read, and `includes` links. From `ts-0128-bb` the provider is told the actor.

What this adds.
- A transition table for a proposal, in `packs/plan/src/transitions.ts`. Recommended: `draft` to `proposed` or
  `rejected`; `proposed` to `accepted`, `deferred`, `rejected` or back to `draft`; `deferred` to `proposed` or
  `rejected`; `accepted` to `superseded`; `rejected` to `draft` (reopened); `superseded` is final. A refused move
  names the status and the moves it has.
- Ops `accept`, `defer { trigger }`, `reject { reason? }` and `resolve { by?, text }`, declared for their kinds, beside
  `setStatus`, which stays and applies the same table and the same person-only checks (as `person_only_request` reads
  them).
- `accept` writes `status: accepted` and `by: human:<date>`, the date from `actor.at`. It is refused while a question
  the proposal `includes` is `open`, naming each. `defer` writes the trigger as `trigger:` and refuses an empty one.
- `resolve` on a question writes `status: resolved` and `resolved_by`: `human:<date>`, or the id of the item named in
  `by`. `text` goes in the question's `## Decision` section, which is made at the end of the body when it is missing.
- The provider refuses `accept`, `defer`, `reject` and `resolve` (and `setStatus` to those words) when `actor.kind` is
  `agent`, in its own words. This is the second wall; the daemon's is `ts-0128-bb`.
- A file that is `accepted` with no `by:` reads as before: it lists, and nothing is invalid.

## Success criteria

- Each legal move works and each other pair is refused naming the moves it has.
- `accept` writes `by: human:<date>` with the date of `actor.at`, and changes no other line of the frontmatter.
- `accept` while an included question is open is refused naming it, and works once the question is `resolved` or
  `dropped`.
- `defer` with a trigger writes it; with none it is refused. `deferred` back to `proposed` works.
- `resolve` writes `resolved_by` and the `## Decision` text, and the question then reads without a `task.invalid`.
- With `actor.kind: agent`, every person-only op is refused by the provider and the file is untouched.
- `setStatus accepted` and `accept` give the same file.

## Tests

Write these first.

- Provider tests over fixture proposals and questions: the full table (every pair), `by:` with a fixed `at`, the open
  question refusal, `defer`, `resolve` with and without `by`, an agent actor on each op, a file hand-edited to
  `accepted` with no `by:`.
- The ops' declarations in `initialize`, and their argument schemas.
- A daemon test that drives `accept` from the app's connection and from the CLI (the `ts-0128-bb` shapes) over the real
  built-in provider, in a temp repository.

## Notes

- Carried from `ts-0127-js`: a question's `resolved_by` may name an item id. `reid` does not rewrite it, so a question
  resolved by an item that is then re-ided names an id that no longer exists. `resolve` writes the field, so this item
  makes `reid` rewrite it too.
- IMPLEMENTATION_PLAN.md:
  - Stage 4c: Feature ops bullet ("The `proposal` kind gets its transitions, the person-only rule and `by:` (0.10),
    with the actor"). Stage 2's slice text: "The transitions, the person-only rule and `by:` wait for Stage 4c".
  - 0.10: Planning sources (the proposal's statuses; "Only a person accepts, defers, rejects or resolves ...
    `collide/plan` refuses those ops from a run ... and refuses `accepted` while a question the proposal includes is
    open"; "A question holds its own decision once it is resolved").
- **Decision for the item, with a recommendation: the transition table.** 0.10 gives the statuses and a few rules, not
  the edges. Recommend the table above. Alternative: any move among the six, as now, with only the rules above. Simpler,
  and a typo could take an `accepted` proposal back to `draft`. The edges are in one table, so a change is one line.
- **Decision for the item: the phase-end check is not here.** 0.10 says an agent phase that writes a `status:` or `by:`
  line itself fails at the phase end, by hashing the planning files that hold those lines before and after. That is
  `protectedPaths` at `phase-end`, which Stage 8 builds. Carried there; see the split proposal. Until then an agent that
  edits the file directly is not caught, which 0.10 already says is a mistake guard and not a wall.
- **Decision for the item: `reject` takes an optional reason**, written as a `reason:` line. Alternative: none.
- **Decision for the item: `resolve`'s `by`.** An item id is accepted only when it exists, as a link target does.
- Area: `packs/plan/src/transitions.ts` (new), `write.ts`, `item.ts`, `frontmatter-edit.ts`, `provider.ts`,
  `packs/plan/test/`, `host/tests/`.
- The test rule: no test touches the real home, real launchd or a real daemon. Each test that runs the `collide` binary
  clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as
  `host/tests/common/mod.rs` `TempRoot` does. Settings files are fixtures, never the owner's own `settings.json`. A temp
  repository sets `GIT_CONFIG_GLOBAL` and `GIT_CONFIG_SYSTEM` to empty files.
- Stage 4c item (revision 23 scope, split 2026-11-18). Items of Stage 4c run one at a time, in id order, because they
  share `core`, `host`, `packs/plan` and the generated files.
- Nothing here runs a real `claude`. No dependency is added.
- Owner decisions: none recorded yet. The recommendations stand until the owner answers at the split.
