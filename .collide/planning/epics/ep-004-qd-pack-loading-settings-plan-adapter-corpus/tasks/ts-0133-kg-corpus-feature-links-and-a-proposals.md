---
id: ts-0133-kg
kind: task
status: ready
title: "Corpus: feature links, inverse link labels and a proposal's coverage"
owners: []
links:
  - { type: blocked-by, to: ts-0132-pm }
---

## Goal

A link that names a feature resolves in the corpus, a link type can say what it is called from the other end, and a
proposal reports its coverage.

What exists. `core/src/corpus.rs` `links_of` resolves a link's target to a key and gives each task its links one hop out
in both directions as `HookLink` (`core/src/hook.rs`); a link whose type targets a feature (`LinkTarget::Feature`) is
left out, because `HookLink` cannot say it (`ts-0115-ug`). `core/src/feature.rs` parses a feature target and derives a
feature's state. `LinkTypeDecl` (`core/src/item.rs`) has a name, a direction, `blocking` and `target`, and no label for
the other end. `TaskGetResult` (`core/src/owner/task.rs`) has `links` and `ops` and nothing about link types. The
provider's `implements` link is declared with `target: feature`, and `isBrokenLink` (`packs/plan/src/links.ts`) checks a
fragment against the proposal's features. Nothing derives coverage.

What this adds.
- `HookLink.feature` (additive: `F2`, left out for a link to an item). For a link to `pr-0003-km#F2`, `key` is the
  proposal's key, `kind`, `status` and `category` are the proposal's, and `missing` is true when the proposal has no
  `F2`. The proposal's own `task.get` shows the incoming link, with the same `feature`.
- `LinkTypeDecl.inverse` (additive, optional text: "evidence for", "blocks", "included in"), and
  `TaskGetResult.link_types` (the declarations of the task's source, left out when empty), so a reader can name an
  incoming link. The SDK's `LinkTypeDeclInput` takes it; the markdown provider declares its own (`blocked-by`:
  "blocks", `part-of`: "has part", `implements`: "implemented by", `informs`: "informed by", `includes`: "included in",
  `supersedes`: "superseded by").
- `Task.coverage` (additive, proposals only): `features_without_task` (the `F<n>` no task implements) and
  `tasks_without_feature` (the ids of the tasks the proposal `includes`, not dropped, that implement no feature).
  Derived by the provider from its scan, never written. A task implements a feature when it has an `implements` link to
  `<proposal>#F<n>`, whether or not the proposal includes it.

## Success criteria

- With three features and two included tasks, one implementing a feature and one implementing none, `coverage` names the
  two features with no task and the second task (the stage's criterion).
- A dropped included task is not named, and a task that implements a feature without being included still counts for it.
- A task's link to `pr-0003-km#F2` appears in `task.get` with `feature: "F2"` and the proposal's state; the
  proposal's own `task.get` shows it as an incoming link; a fragment the proposal lacks is `missing`.
- `link_types` in `task.get` carries each declared type with its `inverse`; a hook's `tasks[].links` has `feature` for
  such a link and the hook payload's schema is regenerated.
- A proposal with no features and no includes has no `coverage` key.
- The corpus cache (`ts-0116-wh`) restores feature links after a restart, and a feature added to a proposal turns a
  `missing` link to resolved on the next change event.

## Tests

Write these first.

- `core/tests/corpus*.rs` extended: feature links out and in, `missing`, a removed feature, a proposal in another
  checkout, the cache round trip.
- Provider tests for `coverage`: the stage's fixture, dropped tasks, a task that implements without being included, a
  proposal with no tasks, `includes` of a question and a spike (not tasks, never named).
- Serde and generated-schema checks for `HookLink.feature`, `LinkTypeDecl.inverse`, `TaskGetResult.link_types`,
  `Task.coverage`; a daemon `task.get` test through the real built-in provider in a temp repository.
- The existing corpus, hook and provider tests unedited.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 4c: Quality parts ("derives a proposal's `coverage`"); Success Criteria ("an ADR's `evidence` link shows from
    the spike's side as 'evidence for'"; the coverage case).
  - 0.8: Task (coverage; "A link's `to` may name a feature"). 0.10: "A proposal's features" (`implements`; derived
    state), "A proposal's quality parts" (coverage). 0.5: `tasks[].links`.
  - Carried from `ts-0115-ug`: "feature links: 4c".
- **Owner's choice, with a recommendation: an additive `feature` on the hook's link.** The hook payload is frozen
  (0.5), and `ts-0115-ug` already added `missing` to it with the owner's approval. A feature link has to be said
  somehow. Recommend `feature` on the existing entry, left out for an item link. Alternative: leave feature links out of
  hooks, so a gate such as "every feature has a done task" cannot be written. This changes what a hook can see.
- **Decision for the item, with a recommendation: the label for the other end is on the type declaration** (`inverse`),
  and the daemon sends the declarations with `task.get`. Alternative: a `label` on each `HookLink`. It repeats the text
  on every link and changes the hook payload again. The CLI and the app print the label (`ts-0136-rr`, Stage 6a).
- **Decision for the item: a dropped task is not "included".** It implements nothing on purpose. Alternative: name it.
  That would flag work a person has already abandoned.
- Area: `core/src/corpus.rs`, `core/src/hook.rs`, `core/src/item.rs`, `core/src/provider.rs`, `core/src/owner/task.rs`,
  `host/src/daemon.rs` (`task_get`), `host/src/corpus_service.rs`, the generated schemas and TypeScript,
  `packages/pack-sdk`, `packs/plan/src/` (`coverage.ts` new, `item.ts`, `scan.ts`), `core/tests/`, `packs/plan/test/`.
- The test rule: no test touches the real home, real launchd or a real daemon. Each test that runs the `collide` binary
  clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as
  `host/tests/common/mod.rs` `TempRoot` does. Settings files are fixtures, never the owner's own `settings.json`. A temp
  repository sets `GIT_CONFIG_GLOBAL` and `GIT_CONFIG_SYSTEM` to empty files.
- Stage 4c item (revision 23 scope, split 2026-11-18). Items of Stage 4c run one at a time, in id order, because they
  share `core`, `host`, `packs/plan` and the generated files.
- Nothing here runs a real `claude`. No dependency is added.
- Owner decisions: none recorded yet. The recommendations stand until the owner answers at the split.
