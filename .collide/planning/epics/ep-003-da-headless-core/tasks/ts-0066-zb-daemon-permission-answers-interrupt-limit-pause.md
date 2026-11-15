---
id: ts-0066-zb
kind: task
status: active
title: "Daemon: permission answers, interrupt, limit pause and resume"
owners: []
links:
  - { type: blocked-by, to: ts-0065-kx }
---

## Goal

The daemon's side of a phase's conversation (`host/src/daemon.rs`, `host/src/phases.rs`):

- **Permission requests.** Pending requests are held per phase and answered through an RPC with `Allow` or `Deny` (with a message). No answer is ever an "always" option.
- **Stop.** A stop sends `interrupt`, then closes stdin, then follows 0.5 (carried from `ts-0059-xs`).
- **Usage limits.** A `limited` event, or a 429 error result, pauses the phase as `paused: usage limit`. The reset time is wall-clock time, and the pause is measured on the continuous clock (0.2's table). The phase resumes by session id (`--resume`, with the budget left) at the reset time, or when the user resumes it (a `run.resume` RPC).
- **Other API errors.** An error result with any other status fails the phase, with the status.

Carried from 3a:

- **A resume with the daemon down.** A paused phase whose reset time passed while no daemon ran is resumed by the next daemon on its own, with no CLI connected. This is 3b's form of "the next phase starts with the app closed" (`ts-0057-ck`). A run here has one phase, so the engine that picks a next phase stays Stage 8's.
- **The login session.** The daemon records its login session, and ending at logout is recorded as host shutdown, not as a crash (`ts-0056-ag`; the daemon's start, step 2).

## Success criteria

- A recorded `result/success` with `is_error: true` is not taken as the phase's report: a 429 pauses the phase as a usage limit, and another status fails it. A recorded budget-error result's cost counts toward the phase (Stage 3b criterion).
- A pending permission request survives a daemon restart, rebuilt by replay from the journal, and can be answered after it.
- A stop sends `interrupt` first. A fake Claude that ends its turn on `interrupt` ends `Stopped` with no signal sent. One that ignores it gets stdin closed, and then 0.5's order.
- A limit pause resumes at the reset time read on the wall clock. A reset that passed during an injected sleep resumes on wake, without waiting out the rest of a countdown (0.3, The reset time is a wall-clock time).
- A paused phase whose reset passes while the daemon is down is resumed by the next daemon, with no CLI connected.
- Ending at logout is recorded as host shutdown, tested with an injected logout. The real one is part of the owner's restart check in `ts-0071-fd`.

## Tests

Write these first:

- limit pause and resume: a recorded limit message, a reset time from a fake wall clock, the resume by session id, and the budget left on `--max-budget-usd`;
- the 429, and another status;
- the permission answer over the RPC, and after a daemon restart;
- the stop order, with `interrupt` first.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 3b: permission requests answered from the CLI (the daemon's side); the `limited` event, pause and resume.
  - 0.3: Permissions; Usage and rate limits; The stream protocol (stdin must stay open; stopping a phase sends `interrupt`, then closes stdin).
  - 0.2: the clock table; Starting the daemon (the login session).
  - 0.5: the stop order.
  - S1 record: the stop order (interrupt, close stdin, SIGTERM gives exit 143).
- Timers come from `ts-0051-wh`'s clocks: uptime for work timeouts, continuous time for the pause's length, and wall time for the reset.
- 0.3 also pauses the account's other runs before their next turn. Stage 3b's text does not ask for it, so it is left out here.
- Area: `host` (`daemon.rs`, `phases.rs`, `stop.rs`), plus `core` for the RPC types.
- The test rule: no test touches the real home, real launchd or a real daemon. Each test clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as `host/tests/common/mod.rs` `TempRoot` does. Settings files are fixtures, never the owner's own `settings.json`.
- Stage 3b item (revision 18 scope, split 2026-11-15). Items of Stage 3b run one at a time, in id order, because they share `host` and the generated files.

### Built

- `core/src/agent.rs`: `TurnEndReason::Budget {}` (the role's budget ran out). `core/src/adapter.rs`: `PhaseFailure::Budget`, and the trait method `AgentSession::pending_permissions` (default: none). `core/src/testing/mod.rs` maps `Budget` to `failed(Budget)`. Schemas and the pack SDK's generated types were regenerated (`pnpm gen`; the second run changes nothing).
- `host/src/adapters/claude/session.rs`: `error_max_budget_usd` is a `Budget` turn end, no longer an `Error`; the session keeps each pending request whole and reports it through `pending_permissions`.
- `host/src/store.rs`: migration 3 adds `run_contexts` and `queue_items`, with their read, write and delete methods.
- `host/src/queue.rs` (new): the stored Queue. An item is written to the store before it is listed and removed from the store before it is gone; `take` is the single-winner remove, so two answers to one item never both act.
- `host/src/login.rs` (new): the daemon's login session. `begin` reads the marker the last daemon left and `finish` writes it. The result is `End`: `First`, `Stopped`, `HostShutdown` or `Crash`. `SystemLoginSession` reads the macOS audit session id. `Daemon::previous_end()` returns the result.
- `host/src/agent_feed.rs`: `rebuild` (a feed made from a journal, for a restart), `answer`, `interrupt`, `pause`, `spent_usd`, `pending_permissions`; a limited turn, a budget turn and the cost of the session so far are tracked. The failure text keeps the status: `api_500`.
- `host/src/phases.rs`: a permission request is a Queue item and a `permission.requested` event (`answer_permission` answers it and records `permission.answered`); a phase can be `paused` (the pause hook runs, not the end hook); `resume_agent_phase` moves the old attempt's files into `attempt-<n>` and starts the same phase again; a restart rebuilds a running phase's feed from its journal; `stop_run` sends `interrupt`, waits `interrupt_wait`, closes stdin, waits `close_wait`, then follows 0.5; a paused phase is stopped without a signal; `run_state` is `Paused` when every live phase is.
- `host/src/file_run.rs`: a run's context (`session_id`, `attempt`, `spent_usd`, the workspace) is stored, so a restarted daemon can resume it. A limit pauses the run, pushes a `usage_limit` item (answers Resume and Stop) and arms a timer. The timer polls the wall clock through the injected `sleep`, and resumes by session id with the budget left. A budget error pauses the run for a person (`budget_exceeded` item, answer Stop, events `budget.exceeded` and `ask_human.asked`). `queue.answer` serves the Resume and Stop answers of a disk pause and of a limit, and `stop_unstarted` stops a disk-paused run that has no phase.
- `host/src/daemon.rs`: serves `queue.answer` (refused for a run's own agent, `NOT_PERMITTED`); `run.stop` reaches a run with no phase; the login session is recorded; the options `login`, `sleep`, `limit_poll`, `limit_min_pause` are injectable.
- `host/src/workspace.rs`: `BuiltinWorkspace::restored`. `host/tests/adapters/claude/fixtures/fake_claude.py`: steps `sleep` and `touch`.
- Tests added, with counts: `host/tests/adapters/claude/pause.rs` 25 (claude_adapter suite 310; one of them, `agent_client_child`, is the client the run-agent test starts in a child process, and does nothing on its own), `host/src/queue.rs` 6, `host/src/login.rs` 7, `host/src/phases.rs` 1 (host lib 257); the budget tests in `results.rs` and `session.rs` now expect `Budget`, and `core/tests/agent_events.rs` round-trips the new reason. `file_run.rs`'s first test now waits for the workspace's `idle` event: it read the run's state, which is derived from the phases and is `ended` a moment before the end hook records it (a flake that was there before this item).
- What the tests show: a run's own agent, connected with the run marker, is refused `queue.answer` and `run.start_file` and a person is not; an allow and a deny reach Claude over `queue.answer`; an answer of the wrong kind is refused and the request stays; a request pending at a restart is still there, rebuilt from the journal, and can be answered; one answered before the restart is not asked again. A fake Claude that ends its turn on `interrupt` ends `Stopped` with `signalled` 0; one that ignores it sees stdin close first, then is signalled. A 429 pauses with the reset at `resetsAt`, and with no `limited` event pauses with no reset time; a 500 fails with `api_500`; the budget error's cost (0.00353) is in the stored context. A limit resumes at the wall-clock reset with `--resume <the session id>` and `--max-budget-usd` equal to the budget less 0.0041; a reset that passed inside one injected sleep resumes on that wake (at most two sleeps); a stale reset still waits out the least pause; a Resume answer resumes at once and a Stop answer stops; a paused run survives a restart, and a reset that passed while no daemon ran is resumed by the next daemon. A disk-paused run answers Resume and Stop, `run.stop` reaches it, and it survives a restart. A logout injected before the daemon ends is `HostShutdown` at the next start; a plain stop is `Stopped`.
- Checks: `cargo test --workspace` twice (2098 passed, 0 failed), clippy with `-D warnings`, `cargo fmt --check`, `pnpm gen` twice (no diff the second time), `pnpm typecheck`, `pnpm test`, `check:names`, `check:stubs`, `check:deps`, and `collide tasks --json | grep -c task.invalid` is 0. No new `Method` (`app/src/owner/methods.ts` is unchanged). No stub tag belonged to this item. After the tests: no `dev.collide` launchd job, no `~/.collide`, no `~/Library/Application Support/Collide`, no process of mine.

### Decisions for the owner

1. What a budget error means in 3b. Chosen: a `Budget` turn end and failure; the phase pauses as `Paused{AskHuman}` with a `budget_exceeded` Queue item that takes only Stop, plus `budget.exceeded` and `ask_human.asked` events. Alternative: fail the phase. Recommend the pause: the work and the session are kept, and the answer to raise the cap (`raise_cap`) has no engine to resume into until Stage 4.
2. Resuming by hand. Chosen: no `run.resume` RPC; the Queue's `Resume` answer is the one path (a new `Method` would change the schemas, `methods.ts` and the board). Alternative: add `run.resume`. Recommend the Queue answer.
3. A pause floor. Chosen: a limit pause lasts at least `limit_min_pause` (30 s, continuous clock) before its reset resumes it, so a stale reset time cannot make a hot loop of resume and limit. A pause an earlier daemon began has no floor. There is no back-off: a resume that limits again pauses again. Alternative: back-off by attempt. Recommend this until a real limit is seen.
4. An attempt keeps its phase key and directory; the earlier attempt's files move to `attempt-<n>/`. Alternative: a new phase name per attempt (`build.2`). Recommend the folder: the run's state and the Queue items stay under one name.
5. The session is assigned on the first attempt (`--session-id`) and resumed after (`--resume`, the same id). The first turn of a resume is the fixed text "Continue where you left off." Alternative: send the task again. Recommend the fixed text: the session already holds the task.
6. `untrusted_input` is false on a permission item: nothing in 3b tracks where a request's arguments came from.
7. Stopping a disk-paused run leaves the workspace as it was: `provisioning` means no worktree exists, and a `ready` one can be removed.
8. The real login probe cannot see a logout while it is happening. Logout is read at the next start (a new session with no stop marker is a host shutdown) or marked by `logging_out`. How macOS behaves at a real logout is not measured; it needs the owner's own run.
9. `queue.answer` and `run.start_file` are refused for a run's own agent (`NOT_PERMITTED`): an agent must not answer its own permission request or start a run. Tested through a real connection: the test starts a child of its own binary whose environment has the run marker, so the daemon sees a run's agent at the socket; the item stays pending, nothing is written to the child, no run is recorded, and a person's client answers the same item. `run.stop` of another run was already refused. Alternative: a per-method table in `auth`. Recommend the check in each handler for now, since each has its own words.
10. A stop interrupts through the feed, then closes stdin after `interrupt_wait` (5 s), then waits `close_wait` (5 s), then SIGTERM, per 0.5. Alternative: a shorter wait. Recommend 5 s each until a real stop is timed.
11. The failure text keeps the status (`api_500`), because the phase record is the only place the status is kept until events are stored (ts-0067-kw). Alternative: widen core's `FailCause`.
12. The Queue is stored in the store (migration 3), so every item survives a restart; this replaces the in-memory Queue of ts-0065's decision 10.

Proposed plan text:

- 0.3, turn-end reasons: add `budget` ("the role's `--max-budget-usd` ran out; the phase pauses and the session is kept").
- 0.3 and Stage 3b: "A budget that runs out pauses the run as `Paused{AskHuman}` with a `budget_exceeded` Queue item that takes Stop; raising the cap is Stage 4's, which has an engine to resume into."
- 0.3, Usage and rate limits: "A limit pause lasts at least 30 s on the continuous clock before its reset time resumes it. A pause that an earlier daemon began is resumed by the next daemon at its reset time."
- 0.8, Phase: "A resumed phase keeps its key and directory; the earlier attempt's files are in `attempt-<n>/`."

- Owner decisions (2026-11-15): the recommendations above are accepted as built, with one change in review: the refusal of a run's own agent (`queue.answer`, and ts-0065's `run.start_file`) is tested through a real connection (decision 9). Plan text applied to 0.3 (the `Budget` turn end; a budget that runs out gives a `budget_exceeded` item until Stage 4; the 30 s floor and the next daemon's resume) and 0.8 (a resumed phase keeps its key and directory). Decision 2 replaces this item's `run.resume` RPC with the Queue's Resume answer. Follow-ups: `ts-0069-bn`'s CLI answers and resumes through `queue.answer`; `ts-0070-mk` checks a real resume's first turn ("Continue where you left off.") and, if a live limit is seen, whether the 30 s floor needs a back-off; `ts-0071-fd`'s restart check covers a real logout (decision 8).

### Not done, by design

- No pause of the account's other runs (0.3): Stage 3b's text does not ask for it.
- No `raise_cap` answer and no resume of a budget pause: there is no engine (Stage 4).
- No back-off between a resume and a new limit (decision 3).
- Logout is not measured on a real macOS logout (decision 8); it is tested with an injected one.
- The workspace is not removed when a run is stopped; the cleanup hook's execution is Stage 7's.
- No `collide run` CLI (ts-0069) and no real-Claude steps (ts-0070-mk): a real permission request and a real limit are not seen here.
