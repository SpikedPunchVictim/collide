---
id: ts-0195-ju
kind: task
status: done
title: "App: the Planning tab, read-only"
owners: []
links:
  - { type: blocked-by, to: ts-0194-qp }
---

## Goal

The Planning tab shows what to build next, read-only: the epics with their progress, the proposals with the
branch-only chip, the features by section across proposals with their derived state and tasks and the
near-duplicates flagged, and the shown proposal's readiness checklist. The drawer shows planning links both ways,
checklists, claims and coverage, read-only.

What exists.
- `task.sections` (`core/src/section.rs` `Sections { sections, near_duplicates }`; `collide sections --json`): the
  sections and features across proposals, with derived state and tasks, and near-duplicate groups (`ts-0133-kg`,
  `ts-0136-rr`).
- The corpus marks a copy found only off the default line's root as `branch_only` (`core/src/corpus.rs` `Row`). The
  wire does not carry it.
- `TaskGetResult.links` (one hop both ways) and `link_types` with `inverse` (`core/src/item.rs` `LinkTypeDecl`).
  The plan pack's claim blocks, checklists and coverage are in the task's fields and parts (`ts-0131-jh` to
  `ts-0134-ch`).
- From `ts-0179-ke`: the Planning tab's placeholder. From `ts-0184-hv`: the drawer.

What this adds.
- Wire: `TaskListResult.branch_only`: the keys whose only copy is off the default line's root, with the branch.
  `collide tasks --json` carries it.
- The tab (the mock's `planningHtml`): "Planning", a "read only" chip, and its line about where changes are made.
  - Epics: a tile per epic with done of total and the next ready task.
  - Proposals: each with its state, section and feature counts; "only on branch <branch>" for a branch-only one.
  - Features by section: Section, Feature, From (its proposal), State (derived), Task (short id); a near-duplicate
    shows "looks like <other>".
  - Readiness: the selected proposal's checklist, checkboxes disabled.
  - The Plan… button says planning sessions arrive in Stage 9d. Sort and merge are Stage 9d's.
- The drawer, for planning items: links of every type both ways, named by the link type's `inverse` from the far
  side ("evidence for"); checklists read-only; claim blocks and coverage, read-only, under their own parts.

## Success criteria

- A proposal shared on a branch and checked out as a worktree shows its items with the branch-only chip (Stage 6a).
- The planning view's counts match the CLI's, and its sections and their feature counts match `collide sections
  --json` (Stage 6a).
- A link shows from both sides, the far side by its `inverse` label.
- No control on the tab or in the drawer writes anything.

## Tests

Write these first.

- `host/tests/`: `branch_only` on `task.list` for a proposal only on a branch's worktree.
- `app/test/`: tiles, proposals, features and near-duplicates from a `FakeOwner` world; inverse labels in the drawer.
- `app/browser/planning.spec.ts` (bridge; Stage 6a tests "planning-view counts against the CLI; the branch-only
  chip"): a temp repo with plan items, a proposal on a branch checked out as a worktree; the chip; counts against
  `collide tasks --json` and sections against `collide sections --json`, run by the test with the world's
  environment.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 6a: Planning (owner, 2026-11-13). Success Criteria: the branch-only chip; the planning view's counts and
    sections. Tests: "Planning: planning-view counts against the CLI; the branch-only chip."
  - 0.11: the Planning surface; revision 14's features (Features by section).
- Design: note 2 (the tab). Mock functions: `planningHtml`, `drawerHtml` (links), `linkRow`.
- Carried: `ts-0130-mv` (the read-only Features view, Stage 6a); `ts-0133-kg` (showing `inverse` and `coverage` in
  the app); `ts-0134-ch` (showing claims in the app).
- **Decision for the item, with a recommendation: which proposal's readiness.** Recommend the proposal picked in
  the Proposals list, the first open one by default. Alternative: every open proposal's checklist, stacked.
- Out of scope: Plan… and planning sessions, Sort and merge (9d); accepting a proposal (the Queue, 9a).
- Area: `core/src/owner/task.rs`, `host/src/daemon.rs` (`task.list`), `schemas/` and the generated files,
  `app/src/` (a `planning/` folder, the drawer's planning parts), `host/tests/`, `app/test/`, `app/browser/`.
- Dependencies: none.
- The test rule: no test touches the real home, real launchd, a real daemon, the keychain or the network. Each test
  that runs the `collide` binary clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to
  a temp root, as `host/tests/common/mod.rs` `TempRoot` does. A temp repository sets `GIT_CONFIG_GLOBAL` and
  `GIT_CONFIG_SYSTEM` to empty files. Keychain writes go through `MemorySecretStore` (`host/src/secret_store.rs`),
  never the login keychain. Browser tests run the installed Chrome with a temp profile
  (`app/browser/support/chrome.ts`), and the bridge's daemon on a temp root (`app/browser/support/world.ts`).
- Stage 6a item (revision 25 scope, split 2026-11-24). Items of Stage 6a run one at a time, in id order, because they
  share `app/`, `core`, `host` and the generated files. No builder runs `tauri build`.
- Nothing here runs a real `claude`.
- Owner decisions (2026-11-24): the owner took the recommendations at the split.

### Built

- Wire: `core/src/owner/task.rs` has `TaskBranchOnly { key, checkout, branch }` and `TaskListResult.branch_only`, set
  on the first page with the other board-wide lists. `core/src/corpus.rs` keeps each checkout's branch
  (`CheckoutInfo.branch`, `Corpus.branches`) and `Corpus::branch_only_entries` lists a key once, from the rows that are
  `branch_only`. `host/src/corpus_service.rs` `discover` fills the branch from `boards::worktrees`. `pnpm gen` ran;
  `schemas/TaskListResult.*.json` and `packages/pack-sdk/src/generated/TaskListResult.*.ts` are its output.
  `collide tasks --json` carries it, as it prints the same result.
- Tab: `app/src/planning/` is `model.ts` (`tilesOf`, `tileWords`, `proposalsOf`, `proposalWords`, `defaultProposal`,
  `featureRowsOf`, `readinessOf`), `PlanningView.tsx` (the drawing), `PlanningPage.tsx` (the picks and the drawer) and
  `useSections.ts` (`task.sections`, read again with the Field's stamp). `Frame.tsx` takes a `planning` page in place
  of the placeholder and `App.tsx` gives it. `FieldData.branchOnly` is read in `app/src/field/load.ts`. `boardOf` and
  `distinct` (`epics/model.ts`) and `indexOf` and `openBlockers` (`field/model.ts`) are exported for the tiles, so the
  tiles count as the strip does.
- Drawer: `app/src/drawer/model.ts` has `linkRows`, `checklistRows`, `claimRows` and `coverageLines`, and four new
  parts in `partsOf`: `links`, `checklist`, `claims` and `coverage`. `DrawerView.tsx` draws them (`TypedLinks`,
  `Checklist`, `Claims`, `Coverage`). The checklist boxes are `disabled`.
- Tests, written first:
  - `core/tests/corpus.rs`: five tests for `branch_only` and the branch (red at runtime on a stub that returned an
    empty list: four of them).
  - `host/tests/daemon_plan_branch_only.rs`: `a_proposal_only_on_a_branch_worktree_is_branch_only_with_its_branch`
    (red at runtime on the same stub; real plan pack, a worktree in a temp root).
  - `app/test/planning-model.test.ts` (20 tests) and `app/test/planning-page.test.tsx` (14), over
    `app/test/support/planning-world.ts` (registered as the `planning` world). Red at runtime on stubs that returned
    empty lists: 19 of 20 and 13 of 14. The ones that passed on the stub were negative checks ("skip the epics...",
    "no Sort or merge").
  - `app/test/drawer-planning.test.tsx` (13): red at runtime on stubs, 12 of 13.
  - `app/browser/fake-planning.spec.ts` (6, fake owner): all six red when the tab was the old placeholder. "nothing on
    the tab or in the drawer writes" is also red when a checklist box is left on: it names the four boxes. It lists the
    methods called and allows only reads (`task.list`, `task.get`, `task.views`, `task.sections`, `task.changes`, and
    the window's own start reads).
  - `app/browser/planning.spec.ts` (bridge): a temp repo, a proposal committed on a branch and checked out as a
    worktree, the chip, the proposal, feature and section counts against `collide tasks --json`, the rows and each
    section's count against `collide sections --json`, the near-duplicates, and the tiles against the listing's
    collections. The command runs with the environment cleared, `HOME` and `COLLIDE_DAEMON_ROOT` the world's, and
    `GIT_CONFIG_GLOBAL` and `GIT_CONFIG_SYSTEM` empty files. Red when the chip was left out.
  - `FieldWorld` (`app/test/support/field-world.ts`) answers `task.sections`, `branch_only` and feature links
    (`pr-0001-aa#F1`), so the other worlds and tests are unchanged.

### Decisions for the owner

- Readiness shows the proposal picked in the Proposals list, the first open one by default, as the item recommended.
  A click on a proposal picks it and opens it in the drawer. Alternative: a pick that does not open the drawer.
  Recommend as built: one click, one place.
- Readiness holds the five lines the proposal's own parts can answer: no problem in the Features part, no unsorted
  feature, coverage, assumptions verified or an accepted risk, and each decision naming an alternative it passed on.
  The lines that need a spike, a question or a red-team pass wait for the planning sessions. Alternative: the full
  mock list with some lines always off. Recommend as built; a line that cannot be answered should not be drawn.
- The tiles follow the strip: an epic's count includes its proposals (the plan pack names the epic's collection on each
  item), and "next" is the first task of the epic that is ready and waits on nothing open. The mock took any open
  task. Alternative: count only tasks. Recommend as built, so the two never disagree.
- The Links part holds every link that is not Waits on or Unblocks, and leaves out an outgoing `part-of`, which Part of
  already names. The part is drawn only when there is a link to show, so the parts of a task with none are as before.
  Alternative: show `part-of` in both. Recommend as built.
- A feature's state on the tab is derived in `planning/model.ts` `stateOf`, from the proposal's status and the
  categories of the tasks that implement it, as `core/src/feature.rs` `derived_state` does. Alternative: the daemon
  reports it on the wire. Recommend a field on `task.sections` or the corpus if a second reader needs it, so the rule
  has one home.
- `branch_only` is a list of `{ key, checkout, branch }` and not a map, as the generated types keep a list simple. A
  checkout on no branch has no `branch`, and the tab says "only in <folder>". Alternative: a map by key.
- The bridge spec's tile check follows the collections the command line lists. A plan pack staged before epics became
  collections lists none, and the check then holds for zero tiles. Run `pnpm build:packs` before the suite.

- Review (2026-11-26): "What exists" said `task.sections` carries each feature's derived state and tasks; it carries
  each section's counts and proposals only, and `core/src/feature.rs` `derived_state` serves the CLI's text view. The
  app's `stateOf` follows the same rules. Proposed for a later revision: the derived state on the wire. The bridge
  spec now fails when the command line lists no epic, instead of checking no tile.

### Not done, by design

- Plan… and planning sessions, Sort and merge: Stage 9d. The button is off and says so.
- Accepting a proposal: the Queue, Stage 9a.
- A plan-pack checklist: the pack emits none yet; the drawer draws `task.checklist` when a source gives one.
