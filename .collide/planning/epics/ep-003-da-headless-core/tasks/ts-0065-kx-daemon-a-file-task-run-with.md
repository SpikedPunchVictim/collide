---
id: ts-0065-kx
kind: task
status: ready
title: "Daemon: a file-task run with one Claude phase"
owners: []
links:
  - { type: blocked-by, to: ts-0061-bg }
  - { type: blocked-by, to: ts-0064-hw }
---

## Goal

The daemon runs one Claude phase for a task given as a file (`host/src/daemon.rs`, `host/src/phases.rs`):

- a `run.start` RPC for a file task;
- a new worktree from `ts-0055-cr`'s built-in workspace;
- the phase's snapshot, written from `ts-0061-bg`;
- the launch spec, with the adapter's environment strip applied to the base environment;
- the phase started under the shim;
- every stdout line fed to the Claude session with its `wall_ns`, and every write journalled before the child sees it;
- the phase settled once the child has exited and its stdout has ended, in either order;
- the run, the phase and the workspace recorded in the store, with the phase's last reported usage.

Carried from 3a:

- the free-space check at phase start: `DiskGuard::check` and `DiskPause` wired into the run, `QueueKind::DiskFull`, the core move `ready -> removable`, and a check of the run directory's volume (`ts-0055-cr`);
- `result_seen` from the stream parser, so a child that exits 0 with no `result` no longer ends `ended` (`ts-0057-ck`, decision 2);
- the first phase waits for the first capture of the base environment (`ts-0056-ag`, decision 5);
- a size cap on `daemon.log`, with one rotated file (`ts-0056-ag`, decision 8).

## Success criteria

- Started from a shell with `CLAUDECODE=1` and other `CLAUDE_*` values set, neither the phase nor its children see them (Stage 3b criterion). Checked with `ps -E`, or `host/src/sysproc.rs` `env_vars_of`, on fake Claude and a grandchild it starts.
- A fake-Claude phase on a file task runs in a new worktree and ends `ended` with its report. A child that exits 0 with no `result` ends `failed`, with cause no result.
- Settling: exit before the end of output, and the end of output before exit, both keep the last lines, including the `result` (0.3).
- With free space under the floor, the phase does not start, and the run shows `paused: disk` with a `DiskFull` Queue item. The run directory's volume is checked too (R5; carried from 3a).
- A run started before the base environment is captured waits for the capture, and never starts a phase with no environment.
- The phase record keeps the last usage its session reported (0.3: a phase's cost is the last total its session reported).
- `daemon.log` stays under its cap.

## Tests

Write these first:

- settling on exit and end of output, in either order;
- env stripping end to end: `CLAUDECODE=1`, `CLAUDE_PID` and a `CLAUDE_CODE_*` value in the environment that starts the daemon, and a fake-Claude child that starts a grandchild;
- the file-task run with fake Claude;
- exit 0 with no `result`;
- the disk floor at phase start, with a fake free-space reading;
- the capture wait;
- the log cap.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 3b: `collide run` on a file task (a new worktree, one Claude phase, the record); the adapter's declarations (the environment strip).
  - 0.2: The shim; The base environment; Durability; the free-space floor.
  - 0.3: The stream protocol (settling); What the adapter declares (environment to strip).
  - S1 record: Q9, what children see.
- **Decision for the item, with a recommendation: the run's role.** 3b has no pack loading (Stage 4a), and `collide/quick` is 3c. Recommend that a file-task run uses the built-in `collide` pack's `builder` role, which `packs/collide/pack.yaml` already names. Its definition and prompt are compiled in, and its model alias comes from `models.yaml`. The task file's text becomes the first turn.
- **Decision for the item: the RPC shape.** `RunStartParams` (`core/src/owner/run.rs`) takes a board and task keys. A file task needs its own form, such as a task-file variant with the repo, the alias and the budget. The generated schemas change: run `pnpm gen`.
- How the daemon finds `claude`: on the base environment's `PATH` (0.2). Tests put fake Claude first on that `PATH`, or use a test-only option in `DaemonOptions`.
- Code outside the adapter must not spell `CLAUDECODE`, `CLAUDE_` or `ANTHROPIC_`. The daemon uses the adapter's `env_strip()`. Tests that run fake Claude or set those variables go in `host/tests/adapters/claude/`.
- The run's start time can be recorded with the run, which `ts-0059-xs` decision 5 wanted for the stop sweep. If it is, the stop reads it from there.
- Area: `host` (`daemon.rs`, `phases.rs`, `workspace`), plus `core` for the RPC types and the Queue kind.
- The test rule: no test touches the real home, real launchd or a real daemon. Each test clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as `host/tests/common/mod.rs` `TempRoot` does. Settings files are fixtures, never the owner's own `settings.json`.
- Stage 3b item (revision 18 scope, split 2026-11-15). Items of Stage 3b run one at a time, in id order, because they share `host` and the generated files.
