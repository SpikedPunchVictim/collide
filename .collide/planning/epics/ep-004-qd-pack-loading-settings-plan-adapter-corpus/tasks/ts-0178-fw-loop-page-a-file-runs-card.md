---
id: ts-0178-fw
kind: task
status: ready
title: "Loop page: a file run's card says its task is unknown"
owners: []
links: []
---

## Goal

Seen in the owner's re-check of `ts-0138-pn` step 6b (2026-11-21): the run card showed a file run started with
`collide run follow-up-probe.md`, as it should, but its Details said Task "unknown" and left Workflow and "Branch the
work started from" blank. A file run has no task, no workflow and no line by design (`host/src/file_run.rs`: one
phase, no workflow, no board). The card reads as if something failed to load.

## Success criteria

- A file run's Details names what it ran from: the task file's path in place of "unknown", and no blank rows.
- A run with a task, a workflow and a line shows them as today.
- A run whose task is not found on any board still says so in words, not "unknown" alone.

## Tests

- Write first and see red: `app/test/loop-page.test.ts`, a file run on the card: Details shows the file and no empty
  row.
- The existing card tests for a quick run still pass.

## Notes

- Where the card gets its fields: `app/src/loop/RunCard.tsx` (Task from `followed.task`, Workflow from `run.workflow`,
  the branch from `run.line`). Whether the run summary carries the task file's path is for the builder to find; if it
  does not, adding it is a wire change (`pnpm gen`).
- Recommendation: hide a row whose value is empty, and show the file in the Task row as "none: a task file, <path>".
  Alternative: keep the rows and write "none". More noise for the same fact.
- Low priority. The test rule as in other items: no test touches the real home, real launchd or a real daemon.
- Owner decisions: none recorded yet. The recommendations stand until the owner answers.

### Built

The run summary did not carry the task file, so this is a wire change (`pnpm gen` ran). `RunSummary` (`run.list`,
`run.get`) has a new optional `task_file`, the file a run of a file (`collide run <file>`) was started from. It is
absent for every other run and for a file run an older daemon recorded.

- `core/src/owner/run.rs`: `RunSummary.task_file`.
- `host/src/store.rs`: migration 11 `run_task_file`, `ALTER TABLE runs ADD COLUMN task_file TEXT`. The path is kept in
  the run's row, not read from the run's context: `FileRuns::forget_context` deletes the context when the run ends,
  and `run.get` after a restart must equal `run.get` before it (the `event_stream.rs` test below).
- `host/src/run_store.rs`: `RunRow.task_file`, read with the row, and `Store::set_run_task_file`.
- `host/src/file_run.rs`: `FileRuns::begin` records the path once the run has its row.
- `host/src/run_summary.rs`: `summarize` copies it.
- `app/src/loop/RunCard.tsx`: a `Row` that is not drawn when its value is empty, and `taskWords`. A run with no tasks
  (a file run) says "none: a task file, <path>", or "none: a task file" when the path is not known. A run whose task
  is on no board says "<key>, not found on any board".
- `app/test/support/loop-world.ts`: `fileRun` now has no workflow and no line, as the daemon records a file run.

Tests, all red at runtime before the code:

- `app/test/loop-view.test.tsx` (where the card is drawn; `loop-page.test.ts` holds the page state only, which this
  change does not alter): "a file run names its task file in the Task row", "a file run shows no row for a workflow
  or a line it does not have", "a file run whose file the daemon did not record still says it ran from a file", "a run
  whose task is on no board says so in words". Green before and after, as a guard: "a run with a task, a workflow and
  a line shows each of them".
- `host/src/run_store.rs`: "a_runs_task_file_is_kept_through_its_end_and_a_run_without_one_has_none" and
  "a_task_file_for_a_run_that_has_no_row_writes_nothing_and_says_so" (red with `set_run_task_file` a no-op).
- `host/tests/adapters/claude/event_stream.rs`: the file-run test
  `run_get_and_run_list_give_a_file_runs_phase_and_end_and_the_same_after_a_restart` now checks `task_file` (red with
  the recording call in `begin` stubbed).
- `host/src/store.rs`: "a_store_at_version_ten_gains_the_task_file_column_and_keeps_its_runs" (a guard: the
  migration was in place before it ran).

### Decisions for the owner

- The path is kept in a new column of `runs` (migration 11). Alternative: read it from the live run's context, with
  no migration. It is lost when the run ends and at a restart, and the existing restart test would have had to be
  weakened. Recommendation: as built. Note: a store at version 11 is refused by a build from before this item.
- A row with an empty value is hidden for Workflow and Branch only; Run is never empty. Alternative: the same helper
  for the Work folder row too (it already hides when absent). Recommendation: as built, one rule for all rows.
- A run whose task is on no board says "<key>, not found on any board" in the Task row. Before, it showed the key
  alone. The item asks for "words, not unknown alone"; this is the smallest words. Alternative: leave the key alone.
- Proposed plan text: none.

### Not done, by design

- A file run recorded before this item has no path: its card says "none: a task file". No item is planned to fill it.
- The owner's re-check of `ts-0138-pn` step 6b with a real file run is not run here.
