---
id: ts-0159-ra
kind: task
status: done
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

### Built

The cause was the one the item names, and a second one near it. Both were seen red at runtime before any code change.

- Cause 1, `app/src/loop/loop.ts` `sync`: with no run followed, `runId` was the run of the first Queue item of kind
  `permission` or `review`, and the newest run only when none waited. A file run makes no item, so the older run's item
  won. Seen red: "a file run that starts while an older run waits for review shows on the card" (card stayed on the
  older run) and "a run that is there when the page opens wins over the Review item of an older run".
- Cause 2, same function: once the page followed a run with a task, `listParams` listed only that task's runs. A
  newer run of another task was never listed. Seen red: "a run of another task that starts while the page follows
  the task of A shows on the card".
- Third, found while writing the tests: a followed run with no task listed the newest run of all, so "Open" on the
  item of a file run lasted until the next sync. Seen red: "opening the item of a run with no task shows it, and a
  later sync keeps it".

The fix, in `sync`: `run.list` is always `{ limit: 1 }`, the newest run of all. The controller keeps `newestSeen`, the
newest run the card has shown or passed. A newest run that is not `newestSeen` takes the card. Otherwise the run
followed stays (a person's `openItem`, or a run started from the page). With nothing followed, the newest run is
followed. Exception, per the Notes: a run whose own `permission` or `review` item waits holds the card, so a newer run
does not take it yet (the Review card keeps the person's re-prompt words and the diff). `newestSeen` advances only when
the card is on the newest run, so a held-back run is still new at the first sync after the item is gone. The Queue
items no longer choose the run. `FOLLOWED_KINDS` now names the kinds that hold it. `targetOf`, `findTask`, `openItem`,
`startRun` and the Queue list are unchanged. No daemon change, no wire change, no `pnpm gen`.

How the page learns of a new run: every `sync` calls `run.list`, and a `sync` runs on `queue.changed` and on the
daemon events in `RELOAD_EVENTS` (`run.transition` among them), and after the stream comes back. A file run started
by `collide run` reaches the page by `run.transition`: `file_run.rs` `record_run` appends it through `Phases`, which
tells the event hub, and `pump_daemon_events` forwards each `daemon.event`. So the page was told in time; it chose
the wrong run. A quick run from the page is followed by `startRun` itself, then the same `sync`.

Tests, in `app/test/loop-page.test.ts` (describe "the newest run (ts-0159-ra)"), with a `fileRun` helper and a task-less
`World.addRun` in `app/test/support/loop-world.ts`. First round, red at runtime on the old code (8 failures): the file
run, the run present at page open, the run of another task, the opened older run (with and without a task), a later run
after "Open", and two existing tests whose expectation was the old rule. Second round (the hold), red at runtime on
the first version of the fix (3 failures): "a file run that starts while the followed run has a Review item waiting does
not take the card", "the held run takes the card at the next sync after the Review item is answered" and "a permission
item of the followed run holds the card the same way, and lets go when answered". Green before and after, as guards:
"a run started from the page shows, and the older run stays in the Queue" (`startRun` follows its own run, so that
sub-case was never broken), "an opened older run with no item waiting keeps the card until a newer run starts" and
"the run followed keeps the card when its own item waits and no newer run started".

For `ts-0160-th` (Discard): the page answers the Review item of the run on the card (`LoopPage.tsx`). The card now
shows the newest run, so Discard shows only when that run has its own Review item. An older run's item is answered
after "Open".

### Decisions for the owner

- A newer run takes the card even from a run the person opened with "Open", once that run has no item waiting. The
  item says so ("until a newer run starts"). Alternative: never move off an opened run. Recommendation: as built.
- "Newer" is a run the page has not seen as the newest, not a comparison of start times. It needs no clock and no
  stored run state. Alternative: compare `started_ms` with the followed run. That moves a page that opened an older run
  back to the newest at the next sync. Recommendation: as built.
- A newer run does not take the card while the run followed has its own Review or permission item waiting, as the
  Notes recommend. It takes it at the first sync after the item is answered or gone. Alternative (my first choice): the
  newer run always wins. It moves the card off a Review the person is reading. Recommendation: as built.
- Proposed plan text: none.

### Not done, by design

- The card shows one run. A card that lists every live run is the item's alternative, left for Stage 6's Queue page.
- The owner's re-check of `ts-0138-pn` step 6b (a file run from the owner's own terminal shows on the card) is not
  run here.
