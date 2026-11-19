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

### Built

The provider reads, lists, creates, changes and renames a question, and holds `includes` and `supersedes`. All in
`packs/plan/src/`:

- `item.ts`: `QUESTION`, `QUESTION_STATUSES` (`open` ready, `resolved` done, `dropped` dropped), `NEEDS`, `INCLUDES`,
  `SUPERSEDES` and their entries in `LINK_TYPES` (out, not blocking), `QUESTION` in `KNOWN_KINDS`, `hasOwnStatuses` and
  `statusesOf`. `readItem` checks a question's own fields (`checkQuestion`): `needs` is `human`, `evidence` or `either`;
  `resolved_by` is required when `resolved`, refused otherwise, and is `human:<date>` or an item id. Each refusal names
  the field.
- `scan.ts`: `readEpicFiles` also reads the `.md` files directly in an epic folder's `questions/`, in both roots. A
  question anywhere else (the planning root, the epic folder itself, `questions/sub/`, inside a spike folder) is not
  opened.
- `ids.ts`: `KIND_IDS.question` (`qu`, four digits). `idsInPaths` and the working-tree read of `takenIds` count a
  file in `questions/` (`ITEM_FOLDERS`); refs are read by `idsInPaths`, so they count too. Spike rules were already
  there.
- `create.ts`: `create` of kind `question` writes `<epic folder>/questions/<id>-<slug>.md`, `open`, with `## Question`
  and `## Decision`. It needs an epic, as a task does, in either root. The refusal text names the kind.
- `write.ts`: `setStatus` knows a question's words. `link` takes `includes` and `supersedes` (`planningLinkRefusal`).
- `reid.ts`: it already rewrote any link type that names the id and kept a `#F2` fragment. New: reid of a shared epic
  also renames its private folder(s) in `<private root>/epics/` (`privateEpicFolders`), under the lock it already holds
  on the private root, so the private items keep their epic. `afterRename` follows more than one rename.
- `provider.ts`: `initialize` declares the kind with a `setStatus` that takes `open` and `dropped` (`ownStatusKind`
  gained `settable`), the two link types, `question` in `create`'s kinds and both types in `link`'s.

Tests: `packs/plan/test/questions.test.ts` (66 tests), and the `basic` fixture: its `questions/qu-0001-aa-why.md` decoy
(a file with broken frontmatter, which is now read) is a valid question, and the unread decoy is
`questions/sub/qu-0091-aa-nested.md`. Existing tests that list kinds, link types, create's kinds, the `basic` items or
the unknown-kind text were updated: `provider.test.ts`, `provider-stdio.test.ts`, `write.test.ts`,
`feature-links.test.ts`, `create.test.ts`, `spikes.test.ts`, `host/tests/task_cli.rs` and
`host/tests/builtin_packs.rs` (the `basic` fixture now lists `qu-0001-aa`).

Red at runtime: before any change to `src/`, 62 of the 66 new tests failed on an assertion or on the reply they read
(no question kind, no `qu-` ids, a refused `link`). Four passed, because they pin what already held: the two spike
collision tests (a spike's id rules came with `ts-0126-nf`), a stored `includes` or `supersedes` to an id no file has
(already counted as a broken link, for any type), and a `reid` that waits for the private root's lock. Then, with the
code in, each of these was broken in turn and the named tests went red, and each was put back: `reid` not renaming the
private folder (1), `ITEM_FOLDERS` without `questions` (4), no refusal of `setStatus resolved` (1), no refusal from a
resolved question (2), no `includes`/`supersedes` rule (16), no `resolved_by`/`needs` check (7), `create` writing a
question to `tasks/` (3), and the declared `setStatus` taking `resolved` (1).

### Decisions for the owner

1. The statuses of a question are `open`, `resolved` and `dropped`, as the plan CLI has them (taken as the item
   recommends, on the owner's delegation). The link types `includes` and `supersedes` are declared, none blocking, with
   `informs` (as the item recommends). A proposal "includes" what it links to, and nothing else.
2. `resolved_by` is checked for its form: `human:` and a `YYYY-MM-DD` date, or an item id. Alternative: only that it is
   there. Recommend as built: `resolve` (`ts-0129-uv`) writes one of the two, and a stray word would show up there as a
   wrong answer. The id is not looked up. `needs`, when present, must be `human`, `evidence` or `either`.
3. `resolved_by` is refused on a `dropped` question too, since the item says "refused otherwise". Alternative: allow
   it on `dropped`. Recommend as built.
4. `setStatus` on a question takes `open` and `dropped`. `resolved` is refused in words that point to the `resolve` op.
   From `resolved`, `setStatus` to `open` or `dropped` is refused, because the edit would leave `resolved_by` behind
   and the file would stop reading. Alternative: a reopen that also removes the line. Recommend: not now. If a person
   needs it, `ts-0129-uv` can add a `reopen` op beside `resolve`. A `category` argument is refused for a question, as
   for a proposal and a spike.
5. `includes` holds a task, a spike or a question: a link to an epic or a proposal is refused, with the rule.
   Alternative: any item. Recommend as built: the item says what a proposal holds. `supersedes` joins two proposals or
   two spikes, and nothing else. Neither moves the other item's status (`superseded` is set by hand or by the
   transition table of `ts-0129-uv`).
6. The `link` op refuses a target that no file has, for the two new types, as it does for `blocked-by` and `informs`.
   A link already in a file that names a missing id is a broken link in the count, as before. Alternative: store it
   and count it. Recommend as built: the op is where a typo is cheap to fix.
7. A new question's body is `## Question` and `## Decision`, the two parts that `resolve` will write into. Alternative:
   a bare `# <title>`, as a spike has. `create` writes no `needs`. Recommend as built.
8. Kind order in `initialize` is task, epic, proposal, spike, question: the new kind goes last, so the text of an
   unknown kind refusal ends `spike, question`.
9. A `reid` of a shared epic renames every private folder in `<private root>/epics/` that starts with the epic's id and
   either ends there or goes on with `-`, keeping what follows. Only a shared epic does this: a private epic's own
   folder already went with it. Private items that only moved are not in the reply's `items` effects, as the items of a
   shared epic's folder were not before. Recommend as built.
10. `reid` does not rewrite an item id in a question's `resolved_by`: it is a field, not a link. See Not done.
11. Not checked, as for epics and spikes: that a file's `kind` is the one its folder says (a `question` in `tasks/`
    reads as a question).
- Plan text to propose for 0.10, Planning sources, kinds: "A question has `open` (`ready`), `resolved` (`done`) and
  `dropped` (`dropped`). `needs` is `human`, `evidence` or `either`. `resolved_by` is required when it is `resolved`,
  refused otherwise, and is `human:<date>` or the id of the item that settled it. `includes` (a proposal to the tasks,
  spikes and questions it holds) and `supersedes` (a proposal to a proposal, a spike to a spike) are declared links
  that block nothing. A `reid` of a shared epic renames its folder in the private root too." Also
  `docs/tasks-and-docs.md` 2.1 still marks `proposals/` and `questions/` "planned, Stage 4c and 9d".

- Owner decisions (2026-11-18, at the build): a question's statuses `open`, `resolved`, `dropped` as the item
  recommends, on the owner's delegation; the rest as built. A `reid` that leaves an item id inside `resolved_by` is
  carried to `ts-0129-uv`, which adds `resolve`.

### Not done, by design

- The `resolve` op (`resolved_by` and the `## Decision` text), the person-only refusals, the transition table and the
  refusal of `accepted` while an included question is open: `ts-0129-uv`.
- A `reid` that also rewrites an item id in `resolved_by`. `ts-0129-uv` writes that form first, so it should add the
  line edit and its test with `resolve`.
- The corpus side of `includes` and `supersedes` (labels, `tasks_without_feature`, coverage): `ts-0133-kg`.
- `watch` over `questions/`: Stage 6a, when `watch` is built.
- `collide tasks` over the real tree: it starts the real daemon, which a build must not do. The provider's list over
  the real tree (`provider.test.ts`, "Collide's own items") still loads with no `task.invalid`, and the tree holds no
  question yet.
