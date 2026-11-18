---
id: ts-0110-ra
kind: task
status: ready
title: "Plan adapter, read side: one checkout"
owners: []
links:
  - { type: blocked-by, to: ts-0109-qj }
---

## Goal

The read side of the `plan` adapter (TypeScript): a provider that parses one checkout's `arch/` tree itself and answers
`initialize`, `list` and `get`. It is the Orchard pack's provider (`providers.plan` in the manifest), so it is built
and tested here, in the Collide repo, as the pack's fixture copy, and the owner copies it into the Orchard repo.

What exists. `core/src/provider.rs` has the protocol types, and `defineProvider` in `packages/pack-sdk/src/provider.ts`
serves them. `fixtures/packs/orchard/pack.yaml` declares `providers: plan: { script: providers/plan.ts }`, and the
script does not exist. `packs/plan` (the markdown provider) is a model to read, not to import: it has the capped
frontmatter reader (`src/read-file.ts`, 64 KiB cap, `yaml` 2.9.1 in core mode, alias limit), key and id rules, and
vitest tests with fixture trees. A repo pack imports only the SDK (0.6), so the reader is copied. There is no `arch/`
fixture tree.

What this adds.
- Fixture `arch/` trees for tests: epics with generated slice and stream tables, task files with frontmatter, the id
  map, and the hazards (a conflict-marker file, a file over the cap, bad YAML, a status the source did not declare).
- `fixtures/packs/orchard/providers/plan.ts` and its modules, importing `yaml` from `@collide/pack-sdk/yaml`
  (see Notes).
- A workspace package for the adapter's tests, since a repo pack may not hold a `package.json` (0.6).
- `initialize`: `scope: ref`; `idPattern` and `shortIdPattern`; the `labelId` template field (`^t\d{2}-\d{3}$`); the
  statuses with their categories (`specified` backlog, `ready`, `doing` active, `in_review` review, `done`,
  `dropped`) and `categoryStatus` (`active` writes `doing`, `review` writes `in_review`); fields; `collections` and
  the links it holds. No `ops` and no `create` yet (`ts-0111-vn`), no `watch` (`ts-0112-px`).
- `list` and `get`: each task as the protocol says, keyed `plan:<shortId>`, long form as `id`, the file stem as an
  alias, `labelId` as a template field, `depends_on` as links, `collections` (the epic as `plan:E-07`, and its slice
  and stream), a `version` that is a hash of the file's content, and `path`. The collections list holds each epic's
  ordered groups from its tables. An item that cannot be read goes to `invalid[]` as `{path, checkout, reason}`.

## Success criteria

- `list` returns every readable task of a fixture tree with the right key, `id`, alias, `labelId`, status, category,
  dependencies and collections. `get` finds one by its short id, its long id and its alias, and answers no task for a
  missing id (an answer, not an error).
- `"plan:E-07"` is in `collections` of exactly the tasks the fixture epic's tables list, and the collection's groups
  keep the tables' order (the fixture form of the stage's "same tasks as the prototype board's E-07 epic").
- The statuses of the fixture map as the plan says; `categoryStatus` has `active: doing` and `review: in_review`.
- A conflict-marker file, a file over the cap, bad YAML, a duplicate key, an anchor or merge key, a non-string `id` and
  a status the adapter did not declare each go to `invalid[]` with the path and a reason (the undeclared status is
  named), and the other tasks still list.
- `version` changes when a task's file changes and not when another file does.
- `list` pages with `nextCursor`, and walking every page gives each task once.
- The `initialize` result validates against the generated `InitializeResult` schema.
- A file's body is never sent in `list` (only `get` carries it), and a body over its cap is refused as invalid.

## Tests

Write these first.

- The adapter against the fixture tree in one checkout: list, get by the three forms, collections and groups,
  `labelId`, the statuses and `categoryStatus`.
- The hazard files: one test each of the criteria's invalid cases, with the reason checked.
- Versions: edit one file, list again, only that task's `version` differs.
- Paging with a page size of 2 over the fixture.
- One end-to-end test with a real `node` child on stdio (as `packs/plan/test/provider-stdio.test.ts` does): `initialize`
  with a `root`, then `list`.
- The `initialize` result against the generated schema.
- A check that the pack folder holds no `package.json` and no `node_modules`, so it would load as a repo pack.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 4b: the `plan` adapter bullets (parses frontmatter per checkout with `yaml` in core mode behind a size cap,
    keys by (shortId, path), declares `labelId`, declares statuses with categories and the status each category write
    produces, exposes collections).
  - 0.10: Sources (the `plan` row; "Why the plan adapter parses files itself"), Identity, Collections, Status categories
    (Orchard's statuses), Versions. 0.7: Pack sources (the Orchard pack is `repo:` in the Orchard repo, with a
    fixture copy here). 0.6: no `package.json` or `node_modules` in a pack; "a pack that needs one vendors it". 0.9:
    task data is untrusted.
- **Owner's choice, with a recommendation: the shape of the `arch/` fixture.** The Orchard repo is not in this repo, and
  nothing public describes its files exactly: the plan gives the ids (`T-01-042` and the long form
  `T-01-042-e-02-<slug>`), `depends_on` in the short form, epic files with slice and stream tables, an id-map file, and
  the statuses. The builder must invent the rest. Recommend that the owner puts a small sample, with stand-in names,
  in the item before it is built: one task file, one epic file with its generated tables, the head of the id map, and
  the output of the plan CLI's `--format json` and `--dry-run` for one `set`. The fixtures are then copies of real
  shapes.
  Alternative: the builder invents the shapes from the plan's text and `ts-0121-xp` (owner-run) finds what differs.
  That risks rework in the adapter, and nothing here can detect it.
- **Owner's choice, with a recommendation: `yaml` in a repo pack.** The plan says to parse with `yaml` in core mode, and
  a repo pack may not hold `node_modules` or a `package.json`. Options:
  1. Vendor `yaml` 2.9.1's compiled ESM (ISC, no dependencies) as plain files under the pack, with its licence and a
     note of the version. Recommended: the plan's own sentence, "a pack that needs one vendors it", and it is the
     same parser `packs/plan` uses and S5 measured.
  2. A hand-written parser for the frontmatter subset. No dependency, but we would own a YAML reader that parses
     untrusted files, which S5 advises against.
  3. Collide supplies `yaml` beside the SDK. Changes 0.6 ("v0 packs have no other npm dependencies"); not recommended
     now.
  A new npm dependency in effect (a vendored copy).
- **Decision for the item, with a recommendation: where the adapter's tests live.** A new private workspace package,
  `packs/orchard-fixture-tests`, with the same dev dependencies as `packs/plan` (`vitest`, `typescript`, `@types/node`,
  `@collide/pack-sdk`), importing `fixtures/packs/orchard/providers/plan.ts`. Nothing new from npm. Alternative: put
  them under `packages/pack-sdk/test`, which mixes a pack's tests into the SDK's.
- **Decision for the item: what an undeclared status is.** The adapter puts such a file in `invalid[]` with the status
  named, since it cannot give the file a category. Core also checks any task whose status its source did not declare
  (`ts-0115-ug`), for sources that do send one.
- **Decision for the item: the page size.** 500 tasks a page. The Orchard tree has about 1,600 and the plan CLI's JSON
  alone takes about 1 s for them (R2), so three pages.
- Out of scope: writes and `preview` (`ts-0111-vn`), checkouts, renames, collisions and `watch` (`ts-0112-px`).
- Area: `fixtures/packs/orchard/providers/`, `fixtures/orchard-arch/` (the trees), `packs/orchard-fixture-tests/`,
  `pnpm-workspace.yaml` only if the glob does not already take it (it takes `packs/*`).
- The test rule: no test touches the real home, real launchd or a real daemon. A test that runs the `collide` binary
  clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as
  `host/tests/common/mod.rs` `TempRoot` does. This item's tests are Node and run no daemon. Settings files are
  fixtures, never the owner's own `settings.json`. No test reads the Orchard repo.
- Stage 4b item (revision 23 scope, split 2026-11-17). Items of Stage 4b run one at a time, in id order, because
  they share `core`, `host` and the generated files.
- Nothing here runs a real `claude`.
- Owner decisions (2026-11-17, at the split): `yaml` comes from `@collide/pack-sdk/yaml` (built in `ts-0109-qj`; plan
  0.6 changed): option 3 above, not a vendored copy. The `arch/` shapes: the owner made a local copy of the Orchard repo
  for this. The build brief names its path and the ref that holds `arch/` and the plan CLI; no tracked file does. The
  builder reads them read-only (git objects, no checkout, fetch or push in the copy). It may run the plan CLI only in
  its own local clone in a temp folder, with no network and no package install; otherwise it reads the CLI's source.
  Fixtures keep the real shapes with stand-in names and invented content. Nothing is copied verbatim, and the denylist
  check covers every fixture. The rest: the recommendation above (`packs/orchard-fixture-tests`).

### Built

The fixture shapes follow the owner's copy of the Orchard repo (read 2026-11-16, read-only). Every value in the fixtures
is invented. Confirmed against the copy: the task path (`arch/proposals/NN-<slug>/tasks/T-NN-NNN-<slug>.md`, the id is
the file stem), the frontmatter keys (`title`, `status`, `epic`, `depends_on`, `blocked_by` and a few extras), the six
statuses (all 1,659 task files use one of them), `epic:` in the short (`E-07`) and the long (`E-07-<slug>`) form,
`depends_on` as `[]`, a flow list or a block list, the generated region markers, the slice headings
(`### Slice N: <name>`) and tables (`Lane | Tasks, in order | Waits on another lane`), the stream table
(`Stream | Owns | Gate | PoC`, size tags in the cells, under a heading that says "streams"), the id-map columns, and
that task-named files sit outside `tasks/` folders and non-`.md` files sit inside them (both are ignored).

Could not confirm: the value forms of the extra keys `awaiting`, `clears`, `migrated`, `backlog_id`, `self_review` and
`reviewed_by` (the adapter ignores them); what `blocked_by` holds (open-question ids in the fixture; the adapter ignores
it, so only `depends_on` makes links); the question-file shape (never read); the proposal file's exact keys.

What exists:
- `fixtures/packs/orchard/providers/plan.ts` (entry) and `providers/plan/`: `read-file.ts` (capped reader, `parse` from
  `@collide/pack-sdk/yaml`), `ids.ts`, `declaration.ts` (the `initialize` result), `task.ts`, `epic.ts` (tables),
  `collections.ts`, `files.ts`, `scan.ts`, `provider.ts` (paging, `get`, handlers). It imports only the SDK, node
  built-ins and its own files. `pack.yaml` `requires.sdk` is `>=0.2 <0.3`.
- Fixture trees: `fixtures/orchard-arch/basic` (2 proposals, 8 tasks, 2 epics with generated, slice and stream tables,
  an id map, a question and a proposal file) and `hazards` (15 task files that must go to `invalid[]`, one fine task,
  one non-task file, one epic with broken YAML). The cap cases are made at test time in a temp copy, so no large file is
  tracked.
- The test package `fixtures/arch-plan-tests` (54 tests, six files: initialize 7, list/get/collections/versions 16,
  hazards 21, paging 4, provider-stdio 2, pack-shape 4). Nothing touches the real home or a daemon; the child test runs
  with a cleared environment and a temp `HOME` and `COLLIDE_DAEMON_ROOT`, links a shim for the SDK in a temp pack copy,
  and ends the child by closing its stdin (it is killed if that fails).

Red at runtime: with `servePlan` a stub that threw `unimplemented`, 52 of the 54 tests failed when run, and every
initialize, list, get, collection, hazard, cap, paging and child-process test among them. The two that passed are two
pack-shape tests (no `package.json` or `node_modules`; the script the manifest names exists), which describe files that
already held. The SDK range and the imports-only test were red until `pack.yaml` and the modules existed.

Mutation checks (each reverted): dropping `onAnchor`, the merge-key check, the conflict-marker check, `uniqueKeys`, the
fence skip in the epic reader, a version over part of the file, and reading `### Gaps` as a slice each turned tests
red. Dropping `maxAliasCount: 0` did not: an anchor is refused first, so an alias has nothing to name; it stays as a
second guard.

Changes outside the item's area, both needed: `core/tests/pack_manifest.rs` asserts the fixture's `requires.sdk`
(now `>=0.2 <0.3`); `pnpm-workspace.yaml` has an entry for `fixtures/arch-plan-tests`, and `pnpm-lock.yaml` its importer
(offline, no new package: the same dev dependencies as `packs/plan`, plus `ajv`, which `claude-contract` already uses).

### Decisions for the owner

- **Where the tests live (recommend: keep).** `fixtures/arch-plan-tests`, not `packs/orchard-fixture-tests`.
  `host/tests/layout.rs` requires every folder of `packs/` to be a pack and counts five; the names check allows the
  reference pack's name only under `packs/` and `fixtures/`. `ts-0111-vn` and `ts-0112-px` name the old folder and
  should say this one. Alternative: change the layout test to allow a non-pack folder in `packs/`.
- **Id and alias (recommend: keep).** In the copy the long id is the file stem (`T-NN-NNN-<slug>`); there is no second
  long form (`T-01-042-e-02-<slug>`) and no separate stem alias, so `id` is the stem, `aliases` is empty, and `get`
  takes the short id, the stem and `plan:<short id>`. Plan text that says otherwise should be corrected.
- **Collections (recommend: keep).** A task's `collections` holds the epic ids only (`plan:E-07`); the slices and
  streams are the epic collection's `groups`, because a group has no id in the protocol. An epic holds a task when
  the task's `epic:` says so or any of the epic's tables (generated list, slices, streams) names it. A table under
  another heading, prose and code fences do not count. The item says a task carries "its slice and stream": read as
  the groups.
- **Body cap 1 MiB (recommend: keep).** The reader's 256 KiB would refuse real tasks (the largest file in the copy is
  about 330 KB). The frontmatter cap stays 64 KiB.
- **`checkout` (recommend: keep).** Tasks and `invalid[]` carry the root as `checkout`; `list` and `get` accept a
  `checkout` equal to the root and refuse another. `ts-0112-px` widens this.
- **Paging.** 500 tasks a page, as the item said; the collections and `invalid[]` come with the first page only, so
  walking the pages gives each once. The tree is read again for every page, so a change between pages can show.
- **Unknown YAML tags (recommend: accept for now).** `parse` hides warnings, so a file with an unknown tag is read as
  text and not refused. Exporting `parseDocument` from the SDK would let the adapter refuse it; that is an SDK change.
- **Plan text.** The plan's manifest example still says `sdk: ">=0.1 <0.2"` (0.7 Pack manifest); propose `>=0.2 <0.3`
  since 0.2 is the first SDK with the yaml export.

- Owner decisions (2026-11-17, at the build): the decisions above as recommended, on delegation. At review: the copy has
  a few task files whose stem carries the epic (`T-NN-NNN-e-NN-<slug>`, the form the plan cites as measured), so a test
  renames a fixture task to that form and checks it lists with the stem as `id`, is found by short id, stem and key, and
  keeps its epic (red with digits refused in the slug). Plan text applied: 0.10 Identity (the long form is `id`) and the
  0.7 manifest example (`>=0.2 <0.3`). `ts-0111-vn` and `ts-0112-px` now name `fixtures/arch-plan-tests`. Unknown YAML
  tags stay accepted until an SDK change exports `parseDocument`.

### Not done, by design

- No `apply`, ops, `create` or `preview` (`ts-0111-vn`): `apply` answers an error. No `watch`, checkouts other than the
  root, renames, id-map reading or collisions (`ts-0112-px`).
- Not run against the real Orchard tree (the item says no test reads it); `ts-0121-xp` (owner-run) checks that, and the
  scan time over about 1,660 files.
- No `labels`, `owners` or `checklist` for a task: the corpus has no such keys.
