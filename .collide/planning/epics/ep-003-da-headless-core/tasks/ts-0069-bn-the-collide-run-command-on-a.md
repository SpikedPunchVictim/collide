---
id: ts-0069-bn
kind: task
status: done
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

Proposed plan text:

- Stage 3b, `collide run`: "`collide run <task-file> [--budget <usd>] [--alias <name>]` starts the daemon when none answers, prints the `claude` version in use and the published one (only the one in use when `claude.check_published` is false), runs one phase in a new worktree and follows it. A permission request is asked on the terminal (y allows once, n denies; never an always option) and answered with `queue.answer`. At the end it prints the record from `run.record`: state, report, and for each model call its wall time, model time, tokens and cost, with 'not reported' for any value the agent did not give. `collide run --resume <run>` reattaches to a live run, resumes a paused run with the Queue's Resume answer, and shows the record of a run that is over. Exit codes: 0 ended, 1 could not do its work, 2 wrong command line, 3 no such task file or run, 5 refused, 7 failed, 8 stopped, 9 waiting for a person."
- Owner protocol (0.13): add `run.record` (`run.record`: the end-of-run record of a file task: state, failure, report, worktree, the run's wall time, and one entry for each model call).
- Stage 3b, `--resume`: the goal text says "resumes a paused or lost phase by session id". The built command resumes a paused run through the Queue's Resume answer. A lost phase has no Queue item to answer, so it is not resumed here; see decision 7.

### Built

- `collide run` (`host/src/run_cli.rs`), wired in `host/src/bin/collide.rs`, marked built in `host/src/commands.rs`. The `cli: run` stub is gone and the tag is in `FILLED_TAGS` (`checks/tests/real_tree.rs`). `host/tests/command_tree.rs` no longer lists `run` as not built, and pins the help text and the exit codes (`task_cli::exit` gained `RUN_FAILED` 7, `RUN_STOPPED` 8 and `WAITING` 9; `run_cli::HELP` lists all of them).
- A new owner method `run.record` (`core/src/owner/run.rs`, `core/src/owner.rs`, `core/src/schema.rs`, `core/tests/owner_protocol.rs`, `app/src/owner/methods.ts`, generated `schemas/` and `packages/pack-sdk/src/generated/`). Its handler is in `host/src/daemon.rs`; the record is built in `host/src/run_record.rs` (with unit tests) from the metrics store's spans and usage rows and the phase record. Supporting pieces: `Phases::settled_run_state` (a run's derived end counts only once the recorded end is final, so the command never prints a record before the end hook ran) and `FileRuns::workspace_path`.
- `launch::ensure_running_from_cli` is the one place the CLI starts the daemon (finds `collide-daemon` beside `collide`, forwards the runtime variables, uses `launchctl_from_env`, so no launchd job loads under `COLLIDE_DAEMON_ROOT`). `stop_cli` now uses it too.
- Follow loop: polls `run.record` and `queue.list` for the run; prints state changes; asks each permission request (the tool, its kind, the reason, the input, or "input not available until approved" when the agent gave none); a usage limit is noted and followed; a full disk or an exceeded budget ends the command with 9 and a hint; an answer that is neither y nor n is asked again; the input ending leaves the request in the Queue and exits 9.
- The record prints a line for each model call (turn, compaction, other model calls) with wall time, model time, input, output, cache read, cache write and cost, "not reported" for every missing figure, a partial figure as "at least", and a total line.
- A refused start prints the daemon's problems on stderr and exits 5; when the alias is not mapped it adds where `models.yaml` goes (`~/.config/collide/models.yaml`) and an example mapping for the alias (`fast: { name: haiku }` by default).
- Tests (all under `host/tests/adapters/claude/`): `run_command.rs` (27 tests: the command's code against an in-process daemon, fake Claude as `claude`; stdin piped; a `Cli` helper with `Env` seams for the daemon start and the `git` call) and `run_process.rs` (7 tests: `collide` and `collide-daemon` as processes with `env_clear()`, `HOME` and `COLLIDE_DAEMON_ROOT` in a temp root: exit codes 0, 1, 2, 3, 5, 7, and `--resume` after the first command was killed). New recording `usage-one-send.jsonl` (the four-turn usage session without the later `expect` steps, because a file run sends one turn). Unit tests in `run_cli.rs` and `run_record.rs`.
- Dependencies: none added.

### Decisions for the owner

1. A new owner method, `run.record`. Chosen: add it, because no served method gave a file run's spans, usage rows and report in one answer. Alternative: print only what `queue.list` and the store's existing reads give (no per-turn wall or model time, so the Stage 3b record could not be printed). Recommendation: keep it; it is also what the UI's run summary will need.
2. Exit codes 7 (failed), 8 (stopped) and 9 (waiting for a person). Chosen: separate codes so a script can tell them apart. Alternative: everything that is not a success exits 1. Recommendation: keep.
3. A permission request when standard input ends. Chosen: leave the request in the Queue, say so, exit 9; `collide run --resume <run>` asks again. Alternative: deny it. Recommendation: keep; the command never answers for the person.
4. A full disk or an exceeded budget. Chosen: print the Queue item's title with the way forward and exit 9, leaving the run. Alternative: keep waiting in the foreground. Recommendation: keep; the person may be away, and `--resume` comes back. The budget item takes only Stop today, so there is no raise-the-budget answer to offer.
5. Wall time per call. Chosen: Collide's own span length goes to the first `Turn` row that names a turn span; rows the daemon tagged with the same span (a poll that found several turns' rows) show "not reported". Alternative: split the span's time over the rows. Recommendation: keep; a split would be an invented number.
6. A turn cut before its result. Chosen: the session's own cut `Turn` row (partial quality: input and cache tokens of the responses) is printed as the turn, as "at least", with output, cost and model time "not reported". When a turn has only `Step` rows and no `Turn` row (the daemon ended mid-turn), the command prints one line from the steps. Alternative: print nothing for a cut turn. Recommendation: keep. Note a cache write the agent reported as 0 prints as "at least 0".
7. A lost phase is not resumed by `--resume`. Chosen: `--resume` reattaches to a live run, answers the Resume item of a paused run, and shows the record of a run that is over (saying nothing resumes a failed or lost one). Alternative: a `run.resume` method that starts a lost phase by session id (decision 3 of the item asked for no such method). Recommendation: keep, and add the method in the item that gives a lost phase a Queue item.
8. A task by item id. Chosen: path only; it was extra work (the resolver lives in the task provider, not in the host). Alternative: resolve `ts-0069-bn` like `collide task`. Recommendation: path only; an item's file path tab-completes.
9. Published version at start. Chosen: wait up to 8 seconds while the daemon's first fetch is pending, then print "not known yet"; a failed fetch prints its error. Alternative: never wait. Recommendation: keep; the fetch normally finishes within a second.
10. The command's `git` call (the checkout that holds the task file, else the working directory's) goes through an `Env` seam so tests run it with a cleared environment. Alternative: no seam, tests inherit the environment. Recommendation: keep, for the test rule.

- Owner decisions (2026-11-15): all ten recommendations taken. Plan text applied to Stage 3b (the `collide run` bullet: flags, versions, prompts, the record, `--resume`, exit codes; a `lost` phase is not resumed by hand in 3b). `run.record` is documented by its wire type and schema. The multi-turn criterion is met with a recording whose four turns follow one send, which a real file run does not produce; real multi-turn model time across `--resume` stays a ts-0070-mk follow-up. Follow-ups: a hand resume of a `lost` phase comes with the item that gives it a Queue item (Stage 4 engine); `Phases::recorded_run_state` reads every event in the store on each call, which `collide run`'s poll now reaches near a run's end, so it wants an indexed read when the store grows (Stage 3c); a test left a `/tmp/cdt-*` root holding only `store.db-shm`/`-wal` after its folder was removed, so some thread outlives its test and reopens the store (removed by hand; find the thread when it recurs).

### Not done, by design

- A task by item id (decision 8).
- Resuming a lost phase by session id, and raising an exceeded budget (decisions 7 and 4).
- An always-allow answer: none exists, in the prompt or the wire type.
- Live agent text in the foreground: the command prints state changes, permission requests and the final record, not the agent's stream.
- More than one turn per file run: the feed closes the session after the first turn end, so a run has one sent turn; the multi-turn case is covered through a recording whose later turns are answered without being sent.
- `--json` output, colours and any terminal control: the command needs no TTY.
- Real `claude` steps: they belong to ts-0070. No test here runs the real `claude`, reaches a network or loads a launchd job.
- The published version under `COLLIDE_DAEMON_ROOT`: a daemon started by the CLI there fetches nothing, so the process tests print only the version in use; both versions are tested with an in-process daemon and a loopback source.
