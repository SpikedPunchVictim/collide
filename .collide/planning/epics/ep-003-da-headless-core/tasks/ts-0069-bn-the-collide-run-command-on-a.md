---
id: ts-0069-bn
kind: task
status: ready
title: "The collide run command on a file task"
owners: []
links:
  - { type: blocked-by, to: ts-0068-wx }
---

## Goal

`collide run` (`host/src/commands.rs`, `run`): one Claude phase on a task given as a file, in a new git worktree, through the daemon. It starts the daemon with `launch::ensure_daemon` when needed (`ts-0056-ag`, decision 3).

What the command does:

- Before it starts, it prints the `claude` version in use and the published one.
- It follows the phase in the foreground.
- It asks each permission request on the terminal and sends the answer.
- At the end it prints the record: the outcome and the report; wall time, model time, tokens and cost per turn; and "not reported" for any value the agent did not give.

Inputs and flags:

- A task file is any markdown file, including an item from `.collide/planning/` (part L: dogfood).
- `--budget <usd>` sets the phase's budget.
- `--resume <run>` reattaches to a live run, or resumes a paused or lost phase by session id.

Replaces the stub tagged `cli: run`.

## Success criteria

- A permission request is answered from the CLI (Stage 3b criterion).
- A permission request with unknown input shows "input not available until approved", and no answer offers an "always" option (Stage 3b criterion).
- The multi-turn model-time case below (Stage 3b criterion), end to end through `collide run` and the store:
  - The recording is a multi-turn session with a `/compact` and a blocked prompt.
  - Each turn's model time equals the difference of `duration_api_ms` from the last result with turns.
  - A recorded result without the field stores null, and the CLI prints "not reported".
- A phase killed before its `result` keeps its responses' input and cache tokens and shows its cost as "not reported" (Stage 3b criterion).
- `collide run` prints both versions before it starts. With `claude.check_published: false`, it prints only the version in use.
- An item file from `.collide/planning/` runs as the task file, in a fresh worktree of the repo it is in.
- `--resume <run>` after the CLI was closed reattaches, and shows the phase's state.
- The exit codes are listed in the help and pinned by a test, as `collide stop`'s are. `host/tests/command_tree.rs` no longer lists `run` as not built.
- No stub tagged `cli: run` remains. The tag moves to `FILLED_TAGS`.

## Tests

Write these first:

- fake-Claude end to end: a file task, a worktree, one phase, a permission prompt answered on the terminal (stdin piped), and the record printed;
- the multi-turn recording with `/compact` and a blocked prompt: per-turn model times, and "not reported";
- a phase killed before its `result`;
- `--resume` after the CLI was closed.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 3b: `collide run` on a file task; permission requests answered from the CLI; Dogfood (part L); the version bullet (`collide run` prints both versions).
  - 0.3: Permissions (no "always"; "input not available until approved"); Two version chips.
  - 0.13: missing values are "not reported".
  - Part L.
- **Decision for the item: user resume.** It is a flag on `collide run`, not a new command, so the command tree gets no new entry. Recommend `--resume <run>`.
- **Decision for the item: a task by item id.** Accepting an item id (resolved as `collide task <id>` resolves it) as well as a path is optional. Recommend a path only, unless an id is no extra work.
- Area: `host` (`commands.rs`, `bin/collide.rs`, a module for the command such as `run_cli.rs` beside `stop_cli.rs`, and `owner_client.rs`).
- Tests that run fake Claude live under `host/tests/adapters/claude/` (the names check).
- The test rule: no test touches the real home, real launchd or a real daemon. Each test clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as `host/tests/common/mod.rs` `TempRoot` does. Settings files are fixtures, never the owner's own `settings.json`.
- Stage 3b item (revision 18 scope, split 2026-11-15). Items of Stage 3b run one at a time, in id order, because they share `host` and the generated files.
