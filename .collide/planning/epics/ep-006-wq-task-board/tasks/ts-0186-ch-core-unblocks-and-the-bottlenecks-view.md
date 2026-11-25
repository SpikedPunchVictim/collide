---
id: ts-0186-ch
kind: task
status: done
title: "Core: unblocks and the bottlenecks view"
owners: []
links:
  - { type: blocked-by, to: ts-0185-ge }
---

## Goal

Core derives each task's `unblocks`: how many open tasks wait on it, directly or through other open tasks, each
counted once, over the same links as `deps`. A view tests it in CEL. Core holds the built-in `bottlenecks` view,
listed after `ready`. A card shows "Unblocks N" from 3 up (note 41).

What exists.
- `core/src/view_expr.rs`: the CEL bindings (`key`, `source`, `title`, `status`, `category`, `kind`, `labels`,
  `collections`, `deps`, `link_types`, `author`, `fields`), and `dependencies`, which `deps` and a view's closure
  both read.
- `core/src/view_builtin.rs`: the built-in `ready` (`READY`, `ready_view`, `evaluate_ready`), first in `task.views`,
  so a file or pack view named `ready` is listed with an error.
- `core/src/corpus.rs` `Corpus::readiness`, with its cycle and unknown-blocker findings.
- From `ts-0181-ft`: the card and its chips.

What this adds.
- `unblocks` per task, computed with the corpus: the open tasks that reach the task through `dependencies`, each
  counted once. A task that is done or dropped stops the walk (it is not counted, and what waits on it through it is
  not reached through it). A cycle ends the walk. The counts are computed once per corpus version, not per view.
- `unblocks` bound in CEL as a number, so `where: 'unblocks >= 3'` works and the view budgets count it.
- The built-in `bottlenecks`: open tasks with `unblocks >= 3`, the most first, ties by key. It is listed after
  `ready`, with source `builtin`. A file or pack view named `bottlenecks` is listed with an error, as for `ready`.
- `unblocks` on each task of `task.list` and in `collide tasks --json`.
- The card's "Unblocks N" chip, from 3 up (note 41).

## Success criteria

- **Unblocks** (note 41): on a fixture where three open tasks wait on one task, and two more wait on one of those
  three, one of them by two paths, its `unblocks` is 5, and a done task in a chain stops the walk. `bottlenecks`
  lists it first, its card shows Unblocks 5, and a task with 2 shows no chip.
- A cycle of three tasks gives each a finite count and the walk ends.
- `collide tasks --view bottlenecks --json` lists the same tasks in the same order as the app.
- A view file named `bottlenecks` is listed with an error naming the built-in.

## Tests

Write these first.

- `core/tests/`: the hand counts above (the five, the two paths counted once, the done task, the two with no
  chip, the cycle), and `where: 'unblocks >= 3'` in a file view.
- `core/tests/`: `bottlenecks` order and ties; the name clash error.
- `host/tests/`: `collide tasks --view bottlenecks --json` on a plan board in a temp repo.
- `app/test/`: the chip shows at 3 and not at 2.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 6a: Field (the card's chips; "Core's `bottlenecks` view (0.10) is listed with the others"). Success
    Criteria: Unblocks. Tests: "Core's `unblocks` and `bottlenecks` against hand counts".
  - 0.10: Views (normalized fields: `unblocks`, revision 25); Built-in and pack views (`bottlenecks`).
- Design: note 41. Mock functions: `hubCount`, `cardHtml` (the chip), the `hubs` view in the mock's views list.
- **Decision for the item, with a recommendation: how `bottlenecks` is held.** Recommend a built-in view value with
  a `where` and a sort by `unblocks`, evaluated by the ordinary path, if `layout.sort` can sort by a derived number.
  Otherwise a rule in `view_builtin.rs` like `ready`. Either way it is listed as built-in.
- **Decision for the item, with a recommendation: open.** Recommend "open" as every category but done and dropped,
  the rule `ready` uses for a blocker.
- Later use, not built here: Up next fills by `unblocks` (Stage 11).
- Area: `core/src/corpus.rs`, `core/src/view_expr.rs`, `core/src/view_builtin.rs`, `core/src/owner/task.rs`,
  `schemas/` and the generated files, `host/src/views.rs`, the CLI's JSON, `app/src/` (the chip), `core/tests/`,
  `host/tests/`, `app/test/`.
- Dependencies: none.
- The test rule: no test touches the real home, real launchd, a real daemon, the keychain or the network. Each test
  that runs the `collide` binary clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to
  a temp root, as `host/tests/common/mod.rs` `TempRoot` does. A temp repository sets `GIT_CONFIG_GLOBAL` and
  `GIT_CONFIG_SYSTEM` to empty files. Keychain writes go through `MemorySecretStore` (`host/src/secret_store.rs`),
  never the login keychain.
- Stage 6a item (revision 25 scope, split 2026-11-24). Items of Stage 6a run one at a time, in id order, because they
  share `app/`, `core`, `host` and the generated files. No builder runs `tauri build`.
- Nothing here runs a real `claude`.
- Owner decisions (2026-11-24): the owner took the recommendations at the split.

### Built

- Core, the count: `Corpus::unblocks` (`core/src/corpus.rs`) gives each task's number of open tasks that wait on it,
  directly or through others, each counted once, a task never counting itself. It reads `dependencies`
  (`core/src/view_expr.rs`) and `resolve_target`, so a link counts as it does for `deps` and `ready`. It is computed
  once per corpus version, beside `Corpus::readiness`, and `touch()` drops it.
- Core, the wire: `Task.unblocks: Option<u32>` (`core/src/provider.rs`), left out when absent. Core sets it on each task
  of `task.list`, with or without a view, and on the task of `task.get` (`task_get` in `host/src/daemon.rs`). A source
  does not report it, and one it reports is replaced. The generated files came from `pnpm gen` (`schemas/`,
  `packages/pack-sdk/src/generated/`).
- Core, the order: `ListFilter.order` lists the tasks in the order of a view's members. The cursor then starts with the
  task's position, so paging follows the view. Without it, the listing is in key order, as before.
- Core, CEL: `unblocks` is bound as a number (`TASK_NAMES`, `TaskVars.unblocks`, `core/src/view_expr.rs`), set from
  the corpus in `core/src/view_eval.rs`, so a file or pack view can test `unblocks >= 3`.
- Core, the view: `BuiltinView` (`core/src/view_builtin.rs`) holds `ready` and `bottlenecks`. `bottlenecks` is the
  open tasks with `BOTTLENECK_MIN` (3) or more, the most first, ties in key order. It reports invalid items as `ready`
  does. `CompiledView::builtin` carries it through `evaluate_view`.
- Host: `host/src/views.rs` lists the built-ins from `BuiltinView::ALL`, so `bottlenecks` follows `ready`, and a
  file or pack view with that name is listed with the error. `host/src/daemon.rs` `task_list` passes the view's members
  as the order, and warms the counts before the corpus is cloned out of the lock.
- App: `app/src/field/model.ts` `unblocksOf` and the `unblocks` chip, "Unblocks N", pushed after "Waits on N". It shows
  from `UNBLOCKS_MIN` (3) up, for an open task of a source that has blocking links.
- Tests:
  - `core/tests/unblocks.rs` (16): the hand counts (five through two paths; a done task stops the walk; a task waiting
    on two that wait on one is counted once; a cycle of three; a cycle waited on; a link that does not block; a
    `depends_on` link), counts worked out once per version and again after a change, a file view testing `unblocks`,
    the `bottlenecks` members, order, report and name, `task.list` with and without a view, and an ordered listing
    paged by cursor.
  - `host/tests/views_find.rs`: `the_built_in_views_are_ready_then_bottlenecks_and_both_load_with_no_error`,
    `a_file_or_pack_view_named_bottlenecks_is_listed_with_an_error_naming_the_built_in`; the `ready` test now expects
    three views.
  - `host/tests/task_daemon_cli.rs` (in-process, no binary):
    `task_json_gives_the_count_of_open_tasks_waiting_on_it_as_task_list_does` (`collide task <id> --json`),
    `tasks_view_bottlenecks_json_lists_the_open_tasks_with_three_or_more_the_most_first_with_their_counts`,
    `tasks_view_bottlenecks_lists_in_the_order_the_app_reads_it_in_and_the_whole_board_carries_the_counts`,
    `a_view_file_named_bottlenecks_is_listed_with_an_error_naming_the_built_in`.
  - `app/test/field-model.test.ts`: the chip at 2, absent, 3 and 12, after "Waits on"; none for a done task or a
    source with no blocking links; a `bottlenecks` view's card shows "Unblocks 5" and a task with 2 shows none.
- Seen red at runtime, against stubs of the new code that kept the old behaviour (counts all 0, no `bottlenecks`
  built-in, no order): 14 of the 16 core tests, all 3 CLI tests, and both new `views_find` tests. The 2 core tests
  that passed say a thing is absent or a name is held. App: the 3 new tests failed with the chip push disabled.
- No test runs the `collide` binary. Existing tests that list views or tasks were changed only where `bottlenecks` or
  `unblocks: 0` is now in the answer.

### Decisions for the owner

- `bottlenecks` is a rule in `view_builtin.rs`, not a `where` with a sort. `layout.sort` is read by the surface only;
  core never sorts by it, so a view value could not give the order `task.list --view bottlenecks` and the CLI need.
  Alternative: a `where unblocks >= 3` view and the surface sorts, which leaves the CLI in key order. Recommendation:
  keep the rule.
- "Waits on" counts through others: a task is counted if it waits on the hub directly or through any open task.
  Alternative: direct waiters only, which hides a hub that sits above a chain. Recommendation: keep; the mock's
  `hubCount` walks the chain.
- Only open tasks count, and a done or dropped task in a chain stops the walk (it no longer waits on anything). A done
  task still has a count on the wire; the chip and `bottlenecks` leave it out. Recommendation: keep.
- Links: `blocked-by` and `depends_on` with `blocking` not false, the same set `deps` and `ready` read. Alternative:
  a second set for this view. Recommendation: keep one rule for what blocks.
- The threshold 3 is a constant in core (`BOTTLENECK_MIN`) and one in the app (`UNBLOCKS_MIN`), both from note 41.
  Alternative: send it on the wire. Recommendation: keep two constants until a setting asks for it.
- `unblocks` is a field of the task on the wire (optional, set by core), not a separate map. It is on `task.list` and
  `task.get`; the drawer does not show it (see Not done). Alternative: a map beside `tasks`. Recommendation: keep.
- The count is of the whole corpus, not of the shown view, so a card shows the same number in every view.
  Alternative: count within the view. Recommendation: keep; a hub is a hub on the whole board.
- `task.list` through a view lists in the view's order (new `ListFilter.order`, position-prefixed cursors), so the
  app's `bottlenecks` shows the biggest hub first with no sort of its own. Alternative: the app sorts. Recommendation:
  keep; `ready` and a file view keep key order, so nothing else changes.
- A file or pack view named `bottlenecks` is listed with an error and is not used, as `ready` is. Recommendation: keep.
- A source with no blocking links shows no chip, as it shows no "Waits on". Recommendation: keep.
- The chip has no colour of its own; it takes the muted default of the other chips. Alternative: a token for hubs.
  Recommendation: keep until the owner asks.

### Not done, by design

- `unblocks` in the drawer: `task.get` now gives it (core's count, replacing a source's), but the drawer does not show
  it. That is for a later drawer item; none names it yet.
- Up next filled by `unblocks`: Stage 11.
- A colour token for the chip: left for the owner's look in `ts-0197-uj`.
- A Playwright spec for the chip: the unit tests cover it, and the fake-owner worlds do not send `unblocks` yet.
