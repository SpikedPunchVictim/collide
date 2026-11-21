---
id: ts-0159-ra
kind: task
status: ready
title: "Loop page: the run card follows a stale Review item, not the newest run"
owners: []
links: []
---

## Goal

In `ts-0138-pn` step 6b (owner-run, 2026-11-21) a file run started from the owner's own Claude Code session never
showed on the loop page's run card, with the app connected to the real daemon before the run started. The run itself
passed: it ran, filed its follow-up and ended with exit 0.

The likely cause is in `app/src/loop/loop.ts` `sync`. With no run followed, the page follows the run of the first Queue
item of kind `permission` or `review`, and only falls back to the newest run (`run.list` with `limit: 1`) when there is
none. The real queue held a Review item of an older quick run (run `run-1795250812644-1`, from a mistaken first try of
step 6c, whose clone was later deleted), first in order. A file run makes no Review item. So the page followed the old
run. Once it follows a run with a task, `listParams` lists only that task's runs, so a newer run of another task is
never picked up. The fix confirms this first, with a test that sees it red.

## Success criteria

- A run that starts while the page is open shows on the card within one reload, whether it was started from the page,
  from `collide run` (a file run, with no task) or from a run's agent, and with an older run's Review or permission item
  still waiting.
- An older run's waiting item stays in "Needs you", and opening it (its "Open" button) shows that run, as today.
- The card keeps following a run the person picked or started from the page until a newer run starts.
- The owner re-checks one look of `ts-0138-pn` step 6b: a file run from the owner's own terminal shows on the card.

## Tests

- Write first and see red: `app/test/loop-page.test.ts`, a queue with a Review item of run A and `run.list` giving a
  newer run B with no task: the card shows B.
- The same with B started from the page while the page follows A's task.
- Opening A's item shows A, and a later sync keeps it until a newer run starts.

## Notes

- Recommendation: the newest run wins over a waiting item of an older run; a waiting item of the run already followed
  keeps it. Alternative: the card lists every live run. More UI; Stage 6's Queue page covers several runs.
- How the page learns of a new run (each sync's `run.list`, or a subscription) is the builder's call; the criteria are
  what the person sees.
- Blocks `ts-0138-pn`: its success criterion "a run started from the owner's own Claude Code session ... shows on the
  loop page" failed. After the fix, the owner re-checks only that look.
- Owner decisions: none recorded yet. The recommendations stand until the owner answers.
