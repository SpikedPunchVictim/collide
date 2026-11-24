---
id: ts-0195-ju
kind: task
status: ready
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
- Owner decisions: none recorded yet. The recommendations stand until the owner answers at the split.
