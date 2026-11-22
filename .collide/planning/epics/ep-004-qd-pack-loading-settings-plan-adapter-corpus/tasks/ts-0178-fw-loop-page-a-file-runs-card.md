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
