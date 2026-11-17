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
