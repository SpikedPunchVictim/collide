---
id: ts-0050-xc
kind: task
status: done
title: "Run, phase and workspace state machines"
owners: []
links: []
---

## Goal

The transitions between the states of `core/src/state.rs`, as pure functions in `core`. Run: `running`, `paused` with its cause, `integrating`, `ended(<outcome>)`, `failed`, `stopped`. Phase: `pending`, `running`, `paused`, `ended`, `failed` (with cause `disk` when known), `stopped`, `lost`. Workspace: `provisioning → ready → in_use → idle → removable → removed`, plus `failed`, which leaves only through the `resume` hook. A refused transition is an error that names both states.

## Success criteria

- Each machine has one transition table. Every pair of states not in it is refused, and the error names both.
- Terminal states accept nothing: `ended`, `failed`, `stopped` and `lost` for a phase; `ended`, `failed` and `stopped` for a run.
- A phase whose shim died without `exit.json` goes to `lost` from `running` or `paused`, whatever the agent's stream reported (0.2).
- `paused` with cause `disk` is reachable from `running` and returns to `running`. A phase that ends with no `result` on a full volume goes to `failed` with cause `disk`, not `lost` (0.2, Durability).
- `ts-0038-ng`'s `apply_integrate_answer` moves a run into and out of `integrating` through this machine, not with its own checks.

## Tests

Write first: a table test per machine over every ordered pair of states (allowed or refused); terminal states refusing every move; the two disk cases; `integrating` reached and left only through the integrate answer.

## Notes

- IMPLEMENTATION_PLAN.md, Stage 3a; 0.7 (Outcomes and terminal states); 0.8 (Run, Phase, Workspace); 0.2 (Durability; a shim that dies without `exit.json`); 0.4 (`integrate` answering `pending`).
- Area: `core` only. No I/O and no clocks.
- Stage 3a item (revision 13 scope, split 2026-11-15). Items of Stage 3a run one at a time, in id order, because they share `host` and the generated files.
- Done when the criteria hold and `run_rules.rs` uses these transitions.

### Built

- `core/src/transition.rs` (new): the three machines as pure functions over the `core/src/state.rs` types. `run_transition(from, to, via)`, `phase_transition(from, to)` and `workspace_transition(from, to, via)` check one move against that machine's one table and return `TransitionError { machine, from, to, note }`. Its message names both states (`a run cannot go from ended (done) to running: an ended run is final`), with the pause cause, the outcome and the fail cause when there is one. A table compares the kind of each state, so `paused` with any cause is one row. `RunVia` (`Engine`, `IntegrateAnswer`) and `WorkspaceVia` (`Engine`, `ResumeHook`) say who asks. `phase_ending(&PhaseEnding { exit_recorded, result_seen, volume_full })` gives the state a phase goes to when its shim is gone: `failed(disk)` for no `result` on a full volume, else `lost` for no `exit.json`, else `ended` or `failed`.
- `core/src/run_rules.rs`: `apply_integrate_answer(run, answer, outcome)` builds the state the answer leads to and asks `run_transition(.., RunVia::IntegrateAnswer)`, so the run machine is the only place that says which states can be accepted. `IntegrateStep.run` is now the run's state after the answer (it was `Option<RunState>`), and `IntegrateError::RunNotFinished { state }` became `RunRefused(TransitionError)`. Its own state check and `run_state_name` are gone.
- `core/src/lib.rs` exports `transition`. `core/src/state.rs` has only its module comment changed (no schema or generated file moved: `pnpm gen` twice, no diff).
- Tests: `core/tests/state_machines.rs` (new, 33): a table test per machine over every ordered pair (run twice, once per way in, workspace twice), terminal states refusing every move, the errors naming both states, `lost` from `running` and `paused` only, both disk cases and the other `phase_ending` rows, and `integrating` entered and left only by the integrate answer. `core/tests/run_rules.rs` (changed, 33 as before): the integrate tests take the new signature and result type.

### Decisions for the owner

- **`ended` is final, with one door.** The item says a run's `ended` accepts nothing, and 0.4 says accepting an `ended` run moves it to `integrating`. The table keeps both: ordinary (`Engine`) moves out of `ended` are refused, and the rows `ended → integrating`, `integrating → ended`, `ended → ended` (a `merged` straight away) and `integrating → integrating` (a `pending` again) are open only to `RunVia::IntegrateAnswer`. `failed` and `stopped` refuse both. The alternative is two tables, or letting the engine enter `integrating` itself.
- **`apply_integrate_answer` takes the run's `outcome`.** `Integrating {}` holds no outcome, so a `merged` cannot put the run back to `ended(<outcome>)` from the state alone. The caller passes the outcome from the run record. The alternative is `Integrating { outcome }`, which keeps it in the state and survives a restart without the record, but changes the wire form and the generated types.
- **A paused run goes back to `running`, fails or is stopped.** It does not end from a pause (a human answer is a token the run follows once it runs again, 0.7), and it does not change its cause without going through `running`. The alternative is `paused → ended`, or `paused → paused`.
- **A pending phase can fail or be stopped, but not pause or be lost.** A free-space floor pauses the run and the phase stays `pending` (3a, `paused: disk`), and no shim owns the phase yet.
- **Workspace.** A `resume` takes `failed` back to `ready`, the only way out of `failed`; the alternative is to let the hook name the state. `idle → in_use` is allowed (a workspace outlives a run). Each state before `removed` can fail. No state skips ahead (`ready → removable` is refused); the alternative is a shortcut for a sandbox that is discarded unused.
- **`phase_ending`'s rules where 0.2 is silent.** No `result` on a full volume is `failed(disk)` even when no `exit.json` was written, because that is when `lost` would otherwise win. A `result` on a full volume with no `exit.json` is still `lost`. An exit record with no `result` on a free volume is `failed` with no cause. An exit record with a `result` is `ended`. The `result` of a failing agent (an error `result`) is Stage 3b's adapter to tell apart.
- **Moves are checked, not applied.** The functions return `Ok(())` and the caller keeps its new state, so one place (the store, later) owns the write. The alternative is a function that returns the new state.
- Owner decisions (2026-11-15): the recommendations above are accepted as built. Follow-up for the engine
  (Stage 8): a run in `integrating` keeps its outcome in the run record, so a `merged` can end it again.

### Not done, by design

- Saving a state, the event that records it, and the `from`/`to` of a state-change event: the store and the daemon items of Stage 3a, and the engine (Stage 8).
- Reading `exit.json`, the free-space floor and the `volume_full` measurement, the kqueue exit code, and the sweep of a lost phase's child: the shim, the disk-floor and stop items of Stage 3a. This item only decides the state from the three facts.
- The run's `paused: disk` Queue item and the usage-limit resume at the reset time: the disk-floor item of Stage 3a, and `3b`.
- The `resume` hook itself, the reaper that marks a workspace `removable`, and the built-in worktree workspace: the workspace item of Stage 3a and Stage 4.
- Fan-out instance states and the per-task state inside a group run: not in 0.8's Phase list, and the engine's.
