---
id: ts-0180-ad
kind: task
status: done
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

### Built

All additive; no UI, and the CLI's text output is unchanged (`--json` prints the daemon's answer, so it carries the
new keys).

- `FieldDecl.label` (`core/src/provider.rs`): an optional string, left out of the wire when it is not set.
- `TaskListResult.sources` (`BoardSource`: name, pack, scope, statuses in order with categories, `category_status`,
  fields with labels, `blocking_links`, `id_pattern`, `short_id_pattern`). The corpus fills it from what each source
  declared; the host fills `pack` from the board's line and leaves out a source that did not start (`not_started`
  already says why). Scratch is listed like any other source, with no pack. It rides the first page only.
- `TaskListResult.divergence` (`TaskDivergence`: key, and for each other copy its status, category, checkout and
  `run`). `divergent` is as it was. The host fills `run` from `FileRuns::run_of_workspace`, which asks the workspace
  owner for the run whose worktree the checkout is, and leaves it out when no run's is. When a re-prompt took over a
  worktree and several runs share it, the newest is named.
- `TaskGetResult.views`: the names of the board's views that hold the task, in the order `task.views` lists them,
  each evaluated through the same cache and budgets as `task.list` (`views_holding` in `host/src/daemon.rs`). A view
  that failed to load or run holds nothing, and a line that does not load gives an empty list; neither fails the get.
- `HookLink.title`, filled only by `task.get` for a link that is not missing. Hooks never get one, so task text still
  does not reach a hook.
- Generated files re-made with `pnpm gen`, never edited by hand.

Tests (red at runtime before the change, unless noted):

- core `corpus.rs`: the eight-status source, labels, blocking links, redeclare and later page, workspace divergence,
  no divergence, link titles (a missing link has none), a hook's links never have a title.
- core `owner_protocol.rs`, `provider_protocol.rs`: serde of the new keys, absent keys read as empty,
  `deny_unknown_fields` still holding, a non-string `label` refused, the keys in the schemas and not required on input.
- pack-sdk: `define-provider.test.ts` (label through `initialize`) and `provider-types.test.ts`. The SDK passes the
  declaration through, so the only red here is the type check; the runtime test is green from the start.
- host `daemon_corpus.rs`: the eight-status source with pack, order, categories, labels (red: pack was missing); the
  scratch source and a source that did not start; a status write in a worktree (green at once: no run made it);
  `task.get` on a scratch task lists only the views that hold it.
- host `daemon_plan_task_get.rs`: a plan task held by two views lists both (red); a task no view holds; link titles.
- host `adapters/claude/divergence_run.rs`: a task file changed in a run's own worktree names the run (red). It uses a
  repo pack over files and the fake `claude`.
- host `file_run.rs` unit test: run ids order by time, then count.

- Review fix (2026-11-24): `FileRuns::run_of_workspace` now lets go of the run table's lock before it takes each run's
  lock, as `recover` does. A start holds its run's lock and then takes the table's, so the first form could deadlock a
  `task.list` against a start that fails.

### Decisions for the owner

- The run of a checkout, as recommended: the workspace owner is asked. Recommendation: keep it. Open point: when
  several runs share one worktree (a re-prompt), I name the newest. Alternative: name all of them as a list. A
  list costs a wire change now for a case the Field shows as one badge.
- `sources` rides the first page only. Recommendation: keep; the Field's columns do not change page to page. A client
  that pages from a cursor reads it from its first call.
- A source that did not start is left out of `sources`, so the Field has no columns for it. Alternative: list it with
  its statuses from the last start. Recommendation: leave it out until a source can say what it declared last.
- `task.get` `views` names a view by its full name (`collide/plan/spikes`). Recommendation: keep; it is what
  `task.list --view` echoes.
- `pack` on a source is the pack name from the line, so it is `collide/plan` for the plan source.

### Not done, by design

- No UI, no `board.sources` method, and no change to the CLI's text output.
- The pack SDK does not check `label` beyond its type; the daemon's decoder (`core`) refuses a non-string one.
- `packs/plan` declares no fields yet, so no labels. Giving it some is for the item that shows the drawer's Fields.
- Hooks get no link titles, on purpose.
