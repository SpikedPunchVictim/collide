---
id: ts-0180-ad
kind: task
status: ready
title: "Wire: what the Field needs from each source and copy"
owners: []
links:
  - { type: blocked-by, to: ts-0179-ke }
---

## Goal

The owner protocol gives the board what it draws from. For each source: its statuses in order with their
categories, its fields with their labels, whether it has a blocking link type, and its id patterns. For a copy that
differs: its state, where it is, and its run. In `task.get`: the views that hold the task, and the linked tasks with
their titles. No UI here; `ts-0181-ft` and `ts-0184-hv` draw it.

What exists.
- A provider's `InitializeResult` (`core/src/provider.rs`) holds `statuses: Vec<StatusDecl>`, `category_status`,
  `fields: Vec<FieldDecl>` (`id`, `field_type`, `filterable`; no `label`), `links: Vec<LinkTypeDecl>`
  (`core/src/item.rs`: `blocking`, `inverse`), `id_pattern`, `short_id_pattern` and `scope`. No owner method returns
  them. `TaskGetResult.link_types` and `ops` give one task's.
- `TaskListResult` (`core/src/owner/task.rs`) has `divergent: Vec<String>` (keys only) and `collisions`. The corpus
  `Row` (`core/src/corpus.rs`) holds each `Divergence` (`checkout`, `state`, `root`, `root_state`), which the wire
  drops.
- `TaskGetResult` has `copies`, `collisions`, `runs`, `latest_run`, `links: Vec<HookLink>` (`core/src/hook.rs`: one
  hop in both directions, with `status`, `category`, `missing`; no title) and `link_types`. Nothing says which views
  hold the task.
- Revision 25 gives a field declaration an optional `label` (0.10). Nothing reads it yet.

What this adds.
- `FieldDecl.label: Option<String>` in core, the provider schema in `schemas/`, `@collide/pack-sdk`
  (`packages/pack-sdk/src/`) and the generated files (`pnpm gen`). `collide/plan` (`packs/plan/src/provider.ts`)
  declares a label where its field id is not a plain word.
- `TaskListResult.sources`: for each started source of the board, its name, pack and scope; its statuses in declared
  order, each with its category; `category_status`; its fields with labels; whether any of its link types is
  blocking; `id_pattern` and `short_id_pattern`. A source that did not start is left out, as `not_started` says.
- `TaskListResult.divergence`: for each divergent key, each other copy's status, category and checkout, and the run
  whose workspace that checkout is, when it is one. `divergent` stays as it is for the CLI.
- `TaskGetResult.views`: the names of the board's views that hold the task (built-in, repo, pack and personal),
  evaluated as `task.list` evaluates them, with the same budgets.
- Each `TaskGetResult.links` row gains the other task's title, so the drawer names a link with no second call. A
  missing target has none.
- All of it is additive. `collide tasks --json` and `collide task <id> --json` carry the new fields; the text output
  is unchanged.

## Success criteria

- A fixture source with eight statuses, two of them in `review`, is listed in `sources` with the eight in its order
  and each with its category.
- A field that declares `label` returns it; a field without returns none.
- A status write in a worktree gives a `divergence` entry naming the worktree's status and checkout. When the
  checkout is a run's workspace, the entry names the run.
- `task.get` on a plan task that two views hold lists both; on a scratch task it lists only the views that hold it.
- `task.get` links carry titles; a link to a missing item carries none and is still marked `missing`.
- The generated files are in sync, and every existing test passes.

## Tests

Write these first.

- `core`: serde of the new fields, absent fields read as empty, `deny_unknown_fields` still holding.
- `host/tests/`: a board with a fake provider (`host/tests/common/fake_provider.rs`) declaring eight statuses and two
  fields (one with `label`); a plan task changed in a worktree (`host/tests/common/plan_rig.rs`); a worktree that is
  a run's workspace; `task.get` views and link titles.
- `packages/pack-sdk`: a provider declaring `label` round-trips through `initialize`.
- The schema check for the provider declaration accepts `label` and refuses a non-string one.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 6a: Field (columns from the declared statuses; the copy that differs, "Review in Run R-121"); the drawer's
    Fields from the source and Part of. Success Criteria: Statuses as data; Drawer (notes 8 and 18).
  - 0.10: statuses as data (revision 23); the `label` of a field (revision 25); Ref-scoped sources (divergence).
  - 0.11: Task drawer (Part of: its epic, collections and views).
- Design: notes 5, 6, 8 and 18 (what the data is for). No mock function: no UI here.
- **Decision for the item, with a recommendation: where the declarations ride.** Recommend `task.list`, the call the
  Field makes anyway, so the columns always match the tasks they hold. A source restart that renames statuses shows
  at the next list. Alternative: a method of its own (`board.sources`), fetched once per board and again on a source
  event; one more call and one more way to be out of date.
- **Decision for the item, with a recommendation: the run of a checkout.** Recommend asking the workspace owner which
  run a checkout belongs to (it records each run's workspace), and leaving the run out when it does not know.
  Alternative: match the checkout's path against `run.list`. Slower, and wrong for the person's own checkout.
- Area: `core/src/provider.rs`, `core/src/owner/task.rs`, `core/src/hook.rs` (or a parallel field), `schemas/`,
  `packages/pack-sdk/src/`, `packs/plan/src/provider.ts`, `host/src/daemon.rs` (`task.list`, `task.get`),
  `host/src/corpus_service.rs`, `host/src/views.rs`, the CLI's JSON, `host/tests/`.
- Dependencies: none.
- The test rule: no test touches the real home, real launchd, a real daemon, the keychain or the network. Each test
  that runs the `collide` binary clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to
  a temp root, as `host/tests/common/mod.rs` `TempRoot` does. A temp repository sets `GIT_CONFIG_GLOBAL` and
  `GIT_CONFIG_SYSTEM` to empty files. Keychain writes go through `MemorySecretStore` (`host/src/secret_store.rs`),
  never the login keychain.
- Stage 6a item (revision 25 scope, split 2026-11-24). Items of Stage 6a run one at a time, in id order, because they
  share `app/`, `core`, `host` and the generated files. No builder runs `tauri build`.
- Nothing here runs a real `claude`: a run's workspace in the tests comes from the fake `claude` of `host/tests/`.
- Owner decisions (2026-11-24): the owner took the recommendations at the split.
