---
id: ts-0075-hj
kind: task
status: ready
title: "Scratch tasks and a minimal board in the daemon"
owners: []
links:
  - { type: blocked-by, to: ts-0074-cu }
---

## Goal

The daemon keeps boards and scratch tasks in its store (`host/src/daemon.rs`, `host/src/store.rs`). This is what
Stage 3c pulls from Stage 4b: `create` and `setStatus`, and nothing else. No corpus, no views.

- `board.open` in its smallest form. A folder in a git checkout becomes a board:
  - a stable id for the checkout's top level, so opening it twice gives the same board;
  - its root;
  - one line, the branch the checkout has now.
- `board.list`: the boards opened before, read from the store.
- `task.apply` on source `scratch`:
  - `create` takes the prompt, and its first line becomes the title;
  - `setStatus` takes one of the five categories (`ready`, `active`, `review`, `done`, `dropped`).
- `task.get` of a scratch task, with the whole prompt.
- Each write records a `task.changed` event, with the connection's actor.

## Success criteria

- A scratch task created over the socket has the key `scratch:<id>`, its first line as the title, and status
  `ready` (0.10).
- `setStatus` moves it to each category. An unknown category is refused, and the message names the five.
- Scratch tasks belong to one board. A key from one board is not found on another (0.10: one list per user per
  board).
- Boards and scratch tasks survive a daemon restart.
- `board.open` on a folder that is not a git checkout returns no board and a failed `git_repo` check, in Collide's
  words, with the fix. `init_git: true` is refused as not built yet (Stage 6a).
- A scratch task is never written into the repository (0.1: scratch tasks are never committed).

## Tests

Write these first:

- scratch-task create and status (the Stage 3c test), over the socket against a temp-root daemon;
- the first line as the title, for a one-line prompt and a long one;
- an unknown category;
- two boards, and a key that belongs to the other one;
- a daemon restart keeps boards and tasks;
- `board.open` on a plain folder, on a checkout's top level and on a folder inside it;
- `git status` of the checkout is unchanged after create and every `setStatus`.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 3c: what it pulls from Stage 4b (scratch tasks with `create` and `setStatus`).
  - 0.1: Scratch tasks are the one exception.
  - 0.10: Task sources (the scratch source: statuses are the categories, a prompt's first line is the title).
  - Stage 4b: Scratch source (what is finished there).
- **Decision for the item, with a recommendation: where the scratch source lives.** Recommend a small type in
  `host` that implements `core/src/ports.rs` `TaskProvider` over the store: `get` and `apply` real, `list` for one
  board, and `watch` and `preview` answering `Unsupported`. Stage 4b then finishes the same type behind the
  supervisor. The alternative, plain daemon methods with no provider, would be rewritten in 4b.
- **Decision for the item: the short id.** Recommend a number per board (`scratch:1`, `scratch:2`). It is short to
  type and needs no collision rule.
- **Decision for the item: `board.open` checks.** Recommend only `git_repo` in 3c. The `claude` checks come with
  Stage 6a's no-setup open. The window shows the `claude` versions from `daemon.agent_versions` (`ts-0081-wm`).
- `run.start` and `task.*` already take a `board` (`core/src/owner/run.rs` `RunStartParams`, `core/src/owner/task.rs`).
  3c's pull list does not name a board, but scratch tasks are per board, so a minimal one is needed here.
- The store's migrations are append-only. This item adds one for boards and scratch tasks.
- Run `pnpm gen` if any wire type changes.
- Area: `host` (`daemon.rs`, `store.rs`, a new scratch source module), plus `core` only if a wire type changes.
- The test rule: no test touches the real home, real launchd or a real daemon. Each test clears the environment
  (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as `host/tests/common/mod.rs` `TempRoot`
  does. Settings files are fixtures, never the owner's own `settings.json`.
- Stage 3c item (revision 22 scope, split 2026-11-15). Items of Stage 3c run one at a time, in id order, because
  they share `host`, the app and the generated files.
