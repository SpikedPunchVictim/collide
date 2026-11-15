---
id: ts-0065-kx
kind: task
status: done
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

### Built

- `core/src/owner/run.rs`, `core/src/owner.rs`, `core/src/schema.rs`: `RunStartFileParams` (repo, task file, optional alias, optional budget; unknown fields refused) and the method `run.start_file`, which answers with the existing `RunStartResult`. Generated: `schemas/RunStartFileParams.*.json` and `packages/pack-sdk/src/generated/RunStartFileParams.*.ts` (`pnpm gen` twice, no diff the second time). `app/src/owner/methods.ts` lists the method (its test keeps the table equal to the `Method` list).
- `core/src/owner/queue.rs`, `app/src/components/words.ts`: `QueueKind::DiskFull`, with its words ("The disk is full").
- `core/src/transition.rs`: the workspace table gains `ready -> removable` (engine), for a worktree that was made and never used.
- `core/src/journal.rs`: `read_journal_from(bytes, first_seq)`, so the daemon can read the journal from a byte offset and keep the sequence numbers checked. `read_journal` calls it.
- `core/src/adapter.rs` (with `core/src/testing/adapter.rs` and `host/src/adapters/claude/mod.rs`): `AgentAdapter::session` returns `Box<dyn AgentSession + Send>`, so a phase's watcher thread can own the session.
- `host/src/agent_feed.rs` (new): `AgentFeed` owns one session for one phase. It tails the phase's journal from its byte offset, feeds each stdout record to the session with its `wall_ns`, sends what the session writes (the journal has it before the child does), asks to close stdin after the first `TurnEnd` (fake Claude, like the real one, waits for stdin to close), keeps the last usage and the report, and at settle reads a 4 KiB stderr tail and gives the `PhaseOutcome`.
- `host/src/phases.rs`: an entry may hold a feed. `start_agent_phase`, `send_turn`, `append_event` (every event write takes the one events lock, so sequence numbers never collide), the feed pumped on each poll of the watcher, and a settle that goes through the session when there is a feed. `run_start_us` is public and reads the run's first `run.transition` event (the `spec.json` time is the fallback). A run with no phase yet reports its recorded state.
- Full volume at phase end (`phases.rs`, `PhasesConfig.disk`, the daemon passes its injected free-space reading and floor): a phase that ends with no `result` has `PhaseEnding::volume_full` set from `DiskGuard::volume_full` on the volume of the run directory (the journal's), so it fails with cause `disk` and its stored failure is `disk`, not `no_result`. This holds for an agent phase that ends `NoResult`, for a phase with no session that exits non-zero, and for a shim that is gone with no `exit.json` (that one is `failed`, cause `disk`, and not `lost`). An unreadable volume counts as not full and the phase keeps its other cause.
- `host/src/store.rs`: migration 2, table `phase_records` (usage, failure cause, report), `PhaseRecord`, `set_phase_record`, `phase_record`.
- `host/src/builtin_roles.rs` (new): the compiled-in `collide/builder` role and its default alias `fast`.
- `host/src/file_run.rs` (new): `FileRuns::start`, in this order: check the task file and the role (`refuse_unmet_capabilities` against no requirement) and the alias (`load_user_models`, `resolve_model_alias`); wait for the base-environment capture; record the run `Running`; the free-space checks; provision the worktree (ts-0055) with its workspace events; the phase row; the snapshot (ts-0061 `snapshot_files`); the launch spec; the layered environment; find `claude` on the base `PATH`; start the phase under the shim; send the task text as the first turn. `on_phase_end` moves the workspace `in_use -> idle` and records the run `Ended`, `Failed` or `Stopped`. `layered_env` is public so a test can check the order.
- `host/src/log_cap.rs` (new), `host/src/bin/collide-daemon.rs`: `daemon.log` is capped (10 MiB) with one rotated file, `daemon.log.1`; a descriptor that writes to the log (standard error under launchd) is `dup2`'d onto each new `daemon.log`.
- `host/src/daemon.rs`: options for home, workspaces dir, free-space reading, floor and git environment; the capture is shared (`Arc<Mutex<..>>`) so the first phase can wait for it; the phase-end hook runs the file-run hook first, then the user's; `run.start_file` (refused for `Caller::RunAgent`) and `queue.list` (from the in-memory queue) are served.
- `host/src/workspace.rs`: `remove` accepts a `ready` workspace (through `removable`).
- `host/tests/adapters/claude/fixtures/fake_claude.py`: steps `dump_env` and `grandchild_dump_env`, which write the environment each process has.
- How it ends, in either order: the shim writes `exit.json` only after it has drained stdout and stderr, so a reported exit means the journal is whole. The daemon reads the rest of the journal, feeds it, then calls `finish` with the exit. A result seen gives `Ended`, exit 0 with no result gives `Failed`, cause `no_result` (stored in `phase_records`), and a stop or a lost shim keep their own outcomes.
- Environment, in layers, later wins: the base environment less the adapter's strip (`BaseEnvironment::for_child`), then the role's `env:`, then the adapter's context variables, then `COLLIDE_RUN` and `COLLIDE_RUN_START` (microseconds). The three context variables therefore win over a role or phase `env:`.
- The disk floor: reads happen in this order: workspace volume, run-dir volume, `provision`'s own check, run-dir volume before the phase, `start_phase`'s own check. A short reading at the first two or the fourth records the run `Running -> Paused{Disk}`, puts a `DiskFull` item in the Queue (answers Resume and Stop), starts no phase, and leaves an existing worktree `ready`. A volume that cannot be read is a refusal, not a pause.
- Tests added, with counts: `core/tests/shim_protocol.rs` +2 (45), `core/tests/state_machines.rs` +1 (34), `host/src/store.rs` +2 (43), `host/src/agent_feed.rs` 6, `host/src/builtin_roles.rs` 2, `host/src/file_run.rs` 2, `host/src/log_cap.rs` 6 (host lib 243), `host/tests/workspace.rs` one test renamed and strengthened (27), `core/tests/owner_protocol.rs` a sample for the method (53), and in the `claude_adapter` target (now 285, from 267): `file_run.rs` (new, 17: the run ends with its report and last usage, the first turn is in the journal, exit 0 with no result is `no_result`, both settle orders, the strip in the child and the grandchild, the layer order, the capture wait, three disk-floor cases, a phase with no result under the floor fails with cause `disk` and one over it keeps `no_result`, alias refusal, missing task file refusal, run recorded before the phase with its start time) and 1 in `fake_claude.rs` for the new steps.
- Checks: `cargo test --workspace` twice (2056 passed, 0 failed), clippy with `-D warnings`, `cargo fmt --check`, `pnpm gen` twice (no diff), `pnpm typecheck`, `pnpm test`, `check:names`, `check:stubs`, `check:deps`, and `collide tasks --json | grep -c task.invalid` is 0. No stub tag belonged to this item.

### Decisions for the owner

1. The RPC shape. Chosen: a separate `run.start_file`, because `run.preview` shares `run.start`'s request schema, and a tagged union there would change the board form. Alternative: one `run.start` that takes either form. Recommend the separate method until the board form runs (ts-0069).
2. The builder's default alias. Chosen: `fast` (the plan's example pack gives its builder `fast` and its reviewer `strong`); `alias` in the params overrides it. Alternative: `strong`. Recommend `fast` for a first run and its cost cap.
3. Where the failure cause is kept. Chosen: a new `phase_records` table (migration 2), because `PhaseState::Failed` carries only a disk cause. Alternative: widen core's `FailCause` with `no_result` and the rest. Recommend the table now, and widening when ts-0067 stores events, since the cause may then be an event.
4. The run's start time. Chosen: the first `run.transition` event's time, which the stop sweep reads through `Phases::run_start_us`; the `spec.json` time is the fallback. Alternative: a column in the store.
5. `AgentAdapter::session` returns `Box<dyn AgentSession + Send>`. Alternative: a session built inside the watcher thread. Recommend keeping `Send`: it is the smaller change and the contract tests still apply.
6. `read_journal_from` in core, to tail a journal without a reread. Alternative: reread from the start each poll. Recommend keeping it; a long phase's journal is large.
7. The daemon tails the journal from each phase's watcher, on its 50 ms poll. Alternative: a notify from the shim on each append. Recommend the poll: the shim protocol stays as it is, and 50 ms is below any human-visible delay.
8. The log cap: 10 MiB, one rotated file. The daemon's standard error (which launchd points at `daemon.log`, so a panic message lands there) is followed: the binary passes descriptor 2 to `CappedLog::open_following` only when `fd_is_file` finds it is `daemon.log` (same device and inode), and after each rotation the new `daemon.log` is `dup2`'d onto it, so those lines are never written to the rotated file or to an unlinked one. Alternative: leave the descriptor alone (it is lost after the second rotation), or have the daemon own standard error through a pipe. Recommend the follow: it is small and keeps panics.
9. A `ready` workspace can go `removable`, so a disk pause before the first use can free the worktree. Alternative: leave it `ready` until the run is resumed. Recommend the move: it was a gap in the table.
10. The Queue is in memory and `queue.list` is served from it, until ts-0066 stores answers. A daemon restart loses a `DiskFull` item.
11. `QueueKind::DiskFull` is new. Alternative: reuse `Limit`. Recommend the new kind: the answers and the words differ.
12. The env strip is proven by each process writing its own environment (child and grandchild), not by reading another process's environment (`env_vars_of`, `ps -E`). Alternative: read it from outside. Recommend the dump: it needs no privilege and covers the grandchild.

Proposed plan text:

- 0.2, Durability, the free-space floor: add "A phase start reads the free space of the workspace volume and of the run directory's volume; a short reading before the phase pauses the run (`Paused{Disk}`) with a `disk_full` Queue item, and an unreadable volume refuses the start."
- 0.3, The stream protocol (settling): add "The shim drains the child's stdout before it reports the exit, so a phase settles when the exit is reported, whichever came first."
- Stage 3b, ts-0065: add "The run's role is the built-in `collide/builder`, alias `fast`; the three context variables are set after the strip and win over a role `env:`."

- Owner decisions (2026-11-15): the recommendations above are accepted as built, with two changes made in review: the log follows the daemon's standard error across rotations (decision 8), and a phase that ends with no `result` on a full volume fails with cause `disk` (0.2). Plan text applied to 0.2 (both volumes; an unreadable one refuses), 0.3 (the shim drains output before `exit.json`) and Stage 3b (the `collide/builder` role, alias `fast`, context variables after the strip). A shim gone with no `exit.json` on a full volume reads `disk` even when a `result` was seen; both outcomes are failures and the volume is full, so this stays. Follow-ups: `ts-0066-zb` stops and resumes a run paused for disk (the `DiskFull` item's answers), stores the Queue so a restart keeps its items, and rebuilds an agent phase's session from its journal after a daemon restart (`replay_journal_bytes`), which its pending-request-after-restart criterion needs. `ts-0067-kw` stores events, widens core's failure cause if the cause becomes an event, and re-runs ts-0063's "no hook output in the store" test against the store. `ts-0069-bn`: with no `models.yaml`, `collide run` says where the file goes and shows a mapping for `fast`. Removing the worktree after a run comes with integrate (Stage 3c).

### Not done, by design

- No agent events are stored: that is ts-0067-kw. The ts-0063 test "no hook stdout or stderr reaches the store" is therefore not re-run here, and was not faked. Re-run it when events are stored.
- No permission answers, interrupt or resume of a paused run: ts-0066-zb. A permission request from a real Claude waits until then. A run paused for disk has no record that `run.stop` can reach, and no resume.
- An agent phase is not rebuilt after a daemon restart (`replay_journal_bytes` could do it); a restart adopts the shim but not the session.
- No `volume_full` failure cause is wired.
- The worktree is not removed after the run; the workspace is left `idle`.
- The log cap is tested at unit level only, not with a real daemon process.
- No `collide run` CLI: ts-0069. No real-Claude steps: ts-0070-mk.
