---
id: ts-0137-pg
kind: task
status: done
title: "Orchard plan provider: ADRs, spikes and questions"
owners: []
links:
  - { type: blocked-by, to: ts-0136-rr }
---

## Goal

The Orchard pack's plan provider reads ADRs, spikes and questions as items, so the Decisions needed view over the
Orchard repo lists the same question ids as the plan CLI's human queue.

What exists. The pack's plan provider is `fixtures/packs/orchard/providers/plan/` (the pack's fixture copy, which the
owner copies into the Orchard repo, `ts-0110-ra`): `scan.ts` reads only `arch/proposals/<proposal>/tasks/` and epics;
kinds `task` and `epic`; the ops `setStatus`, `create` and `reid` run the plan CLI (`run-plan.ts`, `write.ts`) and
`collections.ts` reads collections. `declaration.ts` declares six task statuses. `fixtures/packs/orchard/pack.yaml` has
no views and no roles. The fixtures are `fixtures/orchard-arch/` (trees `basic/` and `hazards/`) and a fake plan CLI
(`fixtures/orchard-arch/` support and `packs/orchard-fixture-tests`). They hold no ADR, spike or question, and no
`status --format json`. The Orchard repo's real shapes for those three are not in this repo.

What this adds.
- Kinds `adr`, `spike` and `question`, read from the folders the shapes name (see the Owner's choice), with their own
  statuses and categories as the plan CLI has them, `part-of` from the folder, and the id the file or the plan CLI
  gives. Frontmatter parsing as the other kinds' (the SDK's `yaml`; the same file cap; S5's hazards).
- Links: an ADR's `evidence` to the spikes and items it cites (the link type is `evidence`, inverse "evidence for"), a
  spike's `informs`, a question's `blocks`, as the real files name them. The pack declares them with their `inverse`
  labels (`ts-0133-kg`).
- The pack's `views:` in `fixtures/packs/orchard/pack.yaml`: `decisions-needed` with `queue: true`, listing the open
  questions that need a human (the plan CLI's rule, from its JSON), and `spikes`.
- The pack's `roles` default: `decision`: `adr`; `spike`: `spike`; `question`: `question`; `proposal`: `proposal`
  (`ts-0135-cx`). The roles check at load reports a mismatch.
- The fake plan CLI answers `status --format json` with the queue, from the fixture trees, so the view's ids are
  compared with a recorded answer. Writes on the new kinds are refused with a reason ("the plan CLI changes these");
  the daemon's answer is `refused`, as it is for any op a provider lacks.
- A fixture `arch/` tree with six ADRs, four spikes and five questions (three of them needing a human), made by hand in
  temp roots by the tests, never copied from the Orchard repo.

## Success criteria

- Over the fixture tree, `collide tasks --view decisions-needed --json` lists exactly the question ids of the fake's
  recorded `status --format json` human queue, in its order. Owner-run over the Orchard repo, in `ts-0138-pn`.
- An ADR's `evidence` link shows from the spike's side as "evidence for", in `collide task <spike> --json` and
  `task.get` (the stage's criterion), and a link to an id that does not exist is broken, not an error.
- ADRs, spikes and questions list with their statuses and categories; a file that is not valid is a `task.invalid` with
  the path and the reason, and the rest list.
- The roles check passes for the pack's own mapping; a layout where a role's kind is missing reports it.
- A write on an ADR, spike or question is refused with the reason, and no plan CLI call is made.
- The existing task and epic behavior, ids and the 500-per-page listing are unchanged.

## Tests

Write these first.

- Provider contract tests in `packs/orchard-fixture-tests` over the new fixture trees: each kind, each status, the
  links, an invalid file, the S5 hazards (an unquoted `no`, `1e10`, a duplicate key, an alias bomb, a file over the
  cap), a folder with 2,000 files.
- A test that the queue view equals the fake's JSON, and a test with the fake's output changed (a question added) that
  shows the view follows it.
- A daemon test that loads the pack as a `repo:` source from a fixture repository, with a trusted line, in a temp root:
  `task.views`, `task.list`, `task.get`.
- The existing orchard fixture tests unedited.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 4c: Planning ("The Orchard pack's plan provider reads ADRs, spikes and questions"); Success Criteria (the
    Decisions needed view; the "evidence for" link).
  - 0.10: Planning sources, Providers (the Orchard pack's provider is a script in the pack), Views. 0.7: Pack sources.
    Part L. `docs/research/planning.md` 1.2 (the plan CLI's kinds and statuses), 2.6 (mappings).
  - Carried from `ts-0110-ra`: the shapes for the three kinds, and the plan CLI's `status --format json`, which that
    item did not have.
- **Owner's choice, with a recommendation: the shapes of the three kinds.** The Orchard repo's folders, id forms,
  frontmatter and statuses for ADRs, spikes and questions, and the JSON of `plan status --format json`, are not in this
  repo. Recommend the owner give a sanitized sample (a few files per kind and one output, with names changed) before
  this item starts, as `ts-0110-ra` did for tasks, so the fixtures start from the real shape. Alternative: the item
  builds from the plan CLI's documented shapes in `docs/research/planning.md` 1.2 and the fixtures stay a guess until
  `ts-0138-pn`. The risk is rework after the owner's run. The owner has a local copy of the Orchard repo; no agent may
  read it.
- **Decision for the item, with a recommendation: the queue rule is the plan CLI's.** The view lists what the fake's
  `status --format json` says, read through a collection of the provider, and not a CEL guess at "needs a human".
  Alternative: a CEL view over `needs`. It diverges from the plan CLI the day the plan CLI's rule changes.
- **Decision for the item: ids.** The provider keeps the id the plan CLI gives; short-id lookup and `reid` are the
  plan CLI's for these kinds, so the provider declares no `reid` for them.
- **Decision for the item: ADR statuses** are mapped to the six categories by the rule that the plan CLI's status names
  decide (accepted: done; proposed: review; superseded: dropped; and so on), written in the declaration.
- Area: `fixtures/packs/orchard/providers/plan/` (`scan.ts`, `declaration.ts`, new modules per kind),
  `fixtures/packs/orchard/pack.yaml`, `fixtures/orchard-arch/`, the fake plan CLI, `packs/orchard-fixture-tests/`,
  `host/tests/`.
- The test rule: no test touches the real home, real launchd or a real daemon. Each test that runs the `collide` binary
  clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as
  `host/tests/common/mod.rs` `TempRoot` does. Settings files are fixtures, never the owner's own `settings.json`. A temp
  repository sets `GIT_CONFIG_GLOBAL` and `GIT_CONFIG_SYSTEM` to empty files. No test reads the Orchard repo.
- Stage 4c item (revision 23 scope, split 2026-11-18). Items of Stage 4c run one at a time, in id order, because they
  share `core`, `host`, `packs/plan` and the generated files.
- Nothing here runs a real `claude`. No dependency is added.
- Owner decisions: 2026-11-18, the shapes come from the owner's local copy of the Orchard repo, read only: stand-in
  names and made-up content, nothing verbatim, and no agent runs its plan CLI. The rest on delegation.

### Built

- Fixtures: `fixtures/orchard-arch/kinds/arch/` (new, all made up): six ADRs (every status), four spikes (every status;
  one with a hyphenated suffix, `S4d-dock`), five questions (open human, open either, open with evidence, resolved,
  and one in a second proposal), one task. The `basic` tree's one question file is removed: the new reader would list
  it as an item, and the old task tests count eight tasks.
- Fake plan CLI `fixtures/orchard-arch/plan`: `commandStatus` answers `status --root <arch> --format json` with the
  envelope (`schema_version`, `command`, `corpus`, `data.queue` and the other `data` keys, empty). A queue entry is
  `{ question, file, needs, blocks }`: open questions that need `human` or `either`, sorted by id.
- Provider `fixtures/packs/orchard/providers/plan/`: `kinds.ts` (new: the status tables, `KINDS`, `KIND_LINK_TYPES`,
  `toKindTask`), `queue.ts` (new: `readQueue`, `QUEUE_ID`), `ids.ts` (file and ref readers for the three kinds, with
  deterministic regexes), `scan.ts` (`candidates` finds ADRs, spike folders with a `PLAN.md` and questions;
  `scan(root, log, queue?)` adds the `plan:queue` collection and each member's `collections`), `declaration.ts`
  (`kinds`, the three link types with `inverse`, id patterns for every kind), `provider.ts` (`list` and `get` pass the
  queue options; the watcher and the write path do not), `write.ts` (`setStatus` on an item with a `kind` is refused:
  "the plan CLI changes these"; no `execute`, so no plan CLI call; applies to preview too).
- Statuses and categories: ADR proposed=review, accepted=done, rejected, deprecated and superseded=dropped. Spike
  open=ready, running=active, concluded=done, abandoned and superseded=dropped. Question open=ready, resolved=done,
  dropped=dropped.
- Links (none blocking): `evidence` (inverse "evidence for") ADR to spike; `supersedes` (inverse "superseded by") ADR to
  ADR and spike to spike; `blocks` (inverse "blocked by") question to an obligation. A malformed ref makes the file
  `task.invalid`; a well-formed ref to a missing id is a link to nothing.
- `fixtures/packs/orchard/pack.yaml`: `views:` `decisions-needed` (`where: "'plan:queue' in collections"`,
  `queue: true`) and `spikes` (`kind == 'spike'`), and `config.roles` (default: decision [adr], spike [spike],
  question [question], proposal []).
- Tests, `fixtures/arch-plan-tests/test/`: `kinds.test.ts` (initialize, each kind and status, links and the evidence
  table, invalid files, the S5 hazards for each kind (unquoted `no`, `1e10`, duplicate key, alias bomb, merge key, 64
  KiB frontmatter, 1 MiB body, conflict marker), 2,000 ADR files and 2,000 question files in pages of 500, refused
  writes with no plan CLI call), `queue.test.ts` (the queue equals the fake's JSON in its order, a scripted other
  answer, the view follows a changed fake, nine bad answers, a missing and a hanging plan CLI, no spawn when no
  question is open). `initialize.test.ts` is edited: the exact id patterns and the exact `links` list no longer hold.
  `host/tests/daemon_reference_kinds.rs` (new; the file name has no pack name, for the no-names check): the pack as a
  `repo:` source in a fixture repository, a trusted line and a temp root: `task.views`, the queue view equals the
  fake's JSON, the view over a canned answer, spikes view, "evidence for" from the spike side, a missing link, the
  roles check, a refused write with `plan-calls.log` unchanged.
- Red at runtime before the code (`pnpm exec vitest run test/kinds.test.ts test/queue.test.ts`): 109 of the 120 failed
  (empty lists, `undefined` results, the old refusal text), and the 11 that passed are the ones that hold of the old
  code (no spawn, basic tree has no kind, `get` of a closed question). Daemon tests: all nine failed against the old
  provider files and `pack.yaml` (from `git show HEAD:`), at the wait for sixteen items; that is the cause for each,
  so they are not red one by one.

### Decisions for the owner

- **Queue is read by running the plan CLI.** One `./plan status` per `list` or `get` while a question is open; none
  when no question is open; at most 30 s. Alternative: cache by the arch tree hash. Recommend keeping it until a
  checkout is slow.
- **The queue is a snapshot of the last full listing.** The watcher scans without the CLI, and the corpus keeps
  collections from its first listing (as for epics), so a question answered on disk leaves the view only at the next
  listing. Alternative: the provider sends collections with `task.changed`, or the daemon relists. Recommend a small
  item before the owner's run; a view over `status` would otherwise look stale.
- **A named view lists in corpus order (by key), not the collection's group order.** The plan CLI sorts by id, so they
  agree. The group carries the CLI's order for a reader of the collection.
- **No spike `informs` link.** The real spike has no such field, so none is emitted. The item's wording is dropped.
- **No `part-of` link for questions**, as there is no proposal item. A question's `proposal` field holds its folder's
  digits.
- **`config.roles.proposal` is `[]`.** The item says `proposal: proposal`, which would report a missing kind on every
  line. Alternative: declare a `proposal` kind later.
- **Required fields** are `title` and `status`. The ADR's `domain`, `docType` and the rest are not enforced; `needs`
  and `resolved_by` are not validated.
- **`blocks` is non-blocking**, and its target (an obligation) is not an item, so it always reads as missing.
- **Writes on the new kinds** are refused by the provider, because a kind cannot declare "no ops". The daemon's status
  check lets only task statuses through, so a person cannot even reach an ADR status.
- **Tasks stay kindless.** Only `adr`, `spike` and `question` are declared. Plan text to propose: "The Orchard plan
  provider declares the kinds adr, spike and question; a task has no kind."
- **Package path** is `fixtures/arch-plan-tests`, not `packs/orchard-fixture-tests` as the item says.

### Not done, by design

- A `task` kind and a `kind` on tasks (`ts-0150-dj`, first bullet: `--kind widget` now exits 2, but the blank `kind`
  line in the text of a task remains).
- A live queue (see the second decision). No `reid` for the new kinds, as decided.
- `ts-0144-eh`, `ts-0145-cq` and `ts-0148-cv` are not touched and not changed by this item.

### At review

- Shapes checked against the owner's copy, read only: ADR, question and spike `PLAN.md` frontmatter keys, the ADR
  (`ADR-NNN`) and spike (`S<num><suffix>`) id forms of the plan CLI's `ids.ts`, the status envelope (`schema_version` 1,
  `command`, `data.queue[].question`), the queue rule (open, `needs` `human` or `either`) and its sort by short id. No
  fixture line of 30 characters or more is in the copy. In the copy every question's `blocks` and every ADR's
  `supersedes` is empty, so the always-missing `blocks` link costs nothing there.
- Mutations, each red then restored: the kind refusal in `write.ts` (15 vitest, the daemon write test), the "evidence
  for" label (1 vitest, the daemon link test), the open-question gate on the queue (3), the queue's order (4), the ADR
  `proposed` category (3).
- Decision 2 confirmed in `apply_changed`: a held copy keeps its collections. Follow-up `ts-0151-fk`, and `ts-0138-pn`
  is blocked by it.
- Plan text added to Stage 4c, where the Orchard provider reads ADRs, spikes and questions.
