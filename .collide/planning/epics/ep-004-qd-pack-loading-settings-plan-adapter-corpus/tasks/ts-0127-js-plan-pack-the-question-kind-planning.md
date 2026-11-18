---
id: ts-0127-js
kind: task
status: ready
title: "Plan pack: the question kind, planning links and ids over every kind"
owners: []
links:
  - { type: blocked-by, to: ts-0126-nf }
---

## Goal

The provider reads questions, holds the planning links (`includes`, `supersedes`), and keeps ids unique and `reid`
correct over every kind it now has.

What exists. `packs/plan/src/scan.ts` opens `tasks/` and `proposals/` of each epic folder, never `questions/`. The link
types are `blocked-by`, `part-of`, `implements` and, from `ts-0126-nf`, `informs` (`packs/plan/src/item.ts`
`LINK_TYPES`); the `link` op takes only the blocking and feature types. Ids, `create` and `reid` cover `task`, `epic`
and `proposal` (`ids.ts` `KIND_IDS`), and tests pin the seeded draw, a forced collision redrawn, a fetched remote
branch and two branches merging (`packs/plan/test/create.test.ts`, `reid.test.ts`). 0.10 gives a question no statuses.

What this adds.
- Kind `question`: files directly in an epic folder's `questions/` (`qu-<4 digits>-<two letters>-<slug>.md`), `part-of`
  from the folder. Statuses `open` (`ready`), `resolved` (`done`) and `dropped` (`dropped`), as the plan CLI's question
  has them (`docs/research/planning.md` 1.2). Fields `needs` (`human`, `evidence` or `either`; optional) and
  `resolved_by` (`human:<date>` or an item id), required when `resolved` and refused otherwise, are read into `fields`.
- `setStatus` on a question takes `open` and `dropped`. `resolved` comes only from the `resolve` op of `ts-0129-uv`.
- `create` of kind `question` (`qu-`, four digits) and the `idsInPaths` and `takenIds` rules for `questions/` and
  `spikes/`.
- Link types `includes` (a proposal to the tasks, spikes and questions it holds) and `supersedes` (a proposal to a
  proposal, a spike to a spike). The `link` op takes `blocked-by`, `implements`, `informs`, `includes` and `supersedes`;
  `includes` and `supersedes` are refused from an item of another kind, with the rule.
- `reid` rewrites `informs`, `includes` and `supersedes` and keeps a feature fragment, as it does `implements`.

## Success criteria

- A question under `epics/<epic>/questions/` lists as kind `question` with `part-of`, status and category, `needs` and
  `resolved_by` in `fields`.
- `resolved` with no `resolved_by`, or `open` with one, is a `task.invalid` that names the field.
- `link includes` from a proposal to a question, a task and a spike works; from a task it is refused; to an id that
  does not exist it is a broken link, counted as `implements` is.
- Two branches that each create ten items across the five kinds (task, proposal, question, spike, epic) merge with no
  two files sharing an id and no `task.invalid`.
- A forced collision on a question and on a spike is redrawn until the id is free among every ref; a number whose every
  suffix is taken stops with an error.
- `reid` of a spike or question rewrites each `informs`, `includes` and `supersedes` that names it, including
  `pr-0003-km#F2` fragments, and commits nothing.

## Tests

Write these first.

- Provider contract tests over fixture questions: the three statuses, `needs` values, the `resolved_by` rule, an S5
  hazard in a question file, a question outside an epic folder (not read).
- `create`, `setStatus` and `link` for each new case; the refusals by source kind.
- The ten-items-on-two-branches merge over a real temp repository (`git merge`), seeded so it is the same on every run,
  and the forced-collision cases, extended from `create.test.ts`.
- `reid` over every link type, with the plan's `reid` rules unchanged for the old kinds.

## Notes

- Carried from `ts-0125-uh`: a private epic folder is named by the epic's id alone (`<pack data>/planning/epics/<epic
  id>/`). A `reid` of a shared epic does not rename that folder, so the private items in it keep the old epic id as
  `part-of`. `reid` over every kind renames it too, under the private root's lock.
- IMPLEMENTATION_PLAN.md:
  - Stage 4c: Planning ("kinds, op effects and links"); Success Criteria ("two branches that each create ten items in
    the shared root merge with no id collision"; "`collide task reid` on a colliding item rewrites the links that name
    it, and a repeated suffix draw is redrawn").
  - 0.10: Planning sources (kinds; "A question holds its own decision once it is resolved"; ids; `includes` in the
    Plan… text under Stage 9d's design); Ref-scoped sources. R5.
  - `docs/research/planning.md` 1.2 (the plan CLI's kinds), 7.2 (lifecycles).
- **Owner's choice, with a recommendation: the statuses of a question.** 0.10 names none. Recommend the plan CLI's:
  `open` (`ready`), `resolved` (`done`), `dropped` (`dropped`), so an Orchard question and a markdown one look the same
  in a view. Alternative: `open` and `resolved` only, with no `dropped`. Smaller, but a question nobody will answer
  then stays open for ever. This sets what a person sees in "Decisions needed".
- **Decision for the item, with a recommendation: the link types.** The plan uses `includes` (a proposal holds tasks,
  spikes and questions) and `informs` (a spike or a task-started proposal informs an item) without declaring them, and
  `supersedes` is how a `superseded` status says by what. Recommend declaring all three, none blocking. Alternative:
  frontmatter fields instead of links. Links show from the other side, which is the point.
- **Decision for the item: what a proposal "includes".** The coverage rule and "refuses `accepted` while a question the
  proposal includes is open" both need it. The link is the one definition. A question in the same epic folder but not
  linked is not included.
- Area: `packs/plan/src/scan.ts`, `item.ts`, `ids.ts`, `create.ts`, `write.ts`, `reid.ts`, `links.ts`, `provider.ts`,
  `packs/plan/test/`.
- The test rule: no test touches the real home, real launchd or a real daemon. Each test that runs the `collide` binary
  clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as
  `host/tests/common/mod.rs` `TempRoot` does. Settings files are fixtures, never the owner's own `settings.json`. A temp
  repository sets `GIT_CONFIG_GLOBAL` and `GIT_CONFIG_SYSTEM` to empty files.
- Stage 4c item (revision 23 scope, split 2026-11-18). Items of Stage 4c run one at a time, in id order, because they
  share `core`, `host`, `packs/plan` and the generated files.
- Nothing here runs a real `claude`. No dependency is added.
- Owner decisions: none recorded yet. The recommendations stand until the owner answers at the split.
