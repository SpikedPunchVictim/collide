---
id: ts-0186-ch
kind: task
status: ready
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
