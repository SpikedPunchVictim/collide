---
id: sp-0009-bb
alias: S6
kind: spike
status: running
title: "S6: interface prototype"
owners: []
links: []
---
# S6: interface prototype

- **Date:** 2026-11-14
- **Spike:** S6. Plan revision 10 (frozen); this record only proposes changes and does not edit
  `IMPLEMENTATION_PLAN.md`.
- **Reproduction:** `.collide/planning/spikes/sp-0009-bb-interface-prototype/`. `README.md` has install, run and
  facilitator mode. `pnpm install && pnpm dev` starts it; `pnpm test` runs the pilots.
- **Status:** `running`. The prototype and the protocol are built and piloted by script. No human session has been run.
  The spike is not finished until sessions are done and the "Session results" section below is filled in.

Labels: **[M]** measured here, **[I]** inferred or a design choice not yet tested on people, **[U]** not measured.

## Versions and conditions

| Item | Version |
| --- | --- |
| macOS, Node, pnpm | Darwin 25.3.0 arm64; v24.18.0; 11.17.0 |
| React, Vite, Vitest | 19.3.1; 8.3.8; 5.1.0 |
| TypeScript, immer | 7.0.2; 11.2.10 |
| Test libraries | `@testing-library/react` 16.3.3, `jsdom` 31.0.1 |
| Browser used for the visual check | Chrome, one tab, dark theme |

Nothing was installed globally. The pnpm store is inside the spike (`.pnpm-store/`). No browser was downloaded. The
dev server I started for the visual check was stopped at the end. About 4,300 lines of source and 450 lines of tests.
Production build: 338 kB JavaScript (103 kB gzip), 20 kB CSS [M].

## Summary

- **[M]** The prototype runs from one command with fake data only: an invented task corpus, fake agent streams on a
  real-time clock with a speed control, every Queue kind with a context view, two finished runs to compare, and an
  A/B experiment from two branches.
- **[M]** Nine scenarios and three probes each start from a defined fake state. The app detects the end state and
  stops the timer, except probe P3 (the facilitator marks it).
- **[M]** Every scenario and probe is completable by a scripted walkthrough, and wrong paths do not end the
  scenario. 50 of 50 tests pass: 28 reducer pilots, 10 UI pilots, 12 protocol checks.
- **[M]** Facilitator mode (URL flag or a key) has the picker, card, timer, error counter, notes, variant selects and
  JSON export.
- **[I]** A scripted pilot proves a scenario can be done and the end state is detected. It says nothing about how
  long a person takes or where they go wrong. That is what the sessions are for.
- **[U]** No participant has used it. The owner set the thresholds below (2026-11-14) before any session.

## What was built and how each scenario is done in it

The app has four tabs (keys 1 to 4): Field, Tracks, Queue, Experiments. Compare opens from the finished-runs table
on Tracks. Run detail (Output, Time breakdown, Review) opens beside any surface.

- **Field** (home). A board of tasks by derived status (ready, blocked, running, review, done), a filter bar with
  chips, a CEL text box, saved views, and a launch panel (workflow, separate or one group, estimated cost). Running
  cards carry a lit border. A pinned Queue rail and a Tracks strip sit on the same page.
- **Tracks.** A real-time axis, one row per run. Segments are phases. Waits on the user are hatched. The projected
  remainder is dashed. A line marks now and another marks the usage-limit reset. Gate diamonds, finding triangles and
  decision circles sit on the rows. Zoom, pan and follow. Below: the finished runs, with two tick boxes to compare.
- **Queue.** Items grouped by kind (permission, command, edit, gate, budget, review), oldest first. The detail has
  the command or diff, and a context view (`i`) with six sections: why it is asked, what it would do, the task, what
  led here, history, and the agent's own words. A section the data lacks reads "Not available in this view".
  Keys `j k a d r o i u`; `Esc` backs out.
- **Run detail.** Output tab (live follow with a Jump to latest button, nested subagent calls, untrusted text shown
  as text). Time breakdown tab (filters on tool, phase, minimum duration and a time window; wall, summed and union
  time; per-tool timeline; click a bar to open the output at that call). Review tab (report, gates, reviewer A and
  B findings, diff, Accept, Re-prompt, Discard).
- **Compare.** Two runs side by side by phase and by tool, with a one-line note box.
- **Experiments.** Pick a task and two branches, see the preview (diffstat, protected-path hits, merge-base
  warning, metric, cost), run it, read the result table and verdict, write a note.

The data. 140 tasks in epics E-01 to E-08 with ids like `T-03-011`, `depends_on`, statuses, labels and a seeded
generator. Names and ids are invented, not taken from the mockups' data. Four branches (main,
`exp/ttl-cache`, `exp/lru-cache`, `exp/no-cache`). Two workflows (build and review; quick fix). Runs are scripts in
run-local seconds; a blocked run waits until its Queue item is answered, then shifts its later events by the wait.
Fake time starts at 09:00:00; every scenario starts at 10:00:00.

One reducer holds all state; every user action is a reducer action. That is why the pilots need no browser.

| Scenario | How it is done in the prototype | End state the app detects |
| --- | --- | --- |
| S1 fresh repo to a reviewed run | Empty Field shows a prompt box. Type the change, launch. A run goes through the short workflow at x30. Decide the review in the Queue or the Review tab. | Run from the new task is done and its review is decided |
| S2 view, save | Chips for Collection, Status and Label, or the CEL box. Save as view, give a name. | Saved view named "Slow spots" whose tasks equal the five targets |
| S3 group launch | Open the saved view Ready in E-03, tick three cards, choose "one group", Launch. | One group id whose runs are exactly the three tasks. Separate launch is an auto error |
| S4 five decisions | Queue: permission, edit, gate, budget, review, aged 14, 9, 6, 3 and 1 minutes. Answer with keys or buttons. | All five resolved |
| S5 review and accept | From the Field card or the Queue, open the Review tab of T-05-009's run, Accept. R-502 is a distractor. | That run is accepted |
| S6 compare | Tick two finished runs, Compare. The second run has two permission waits in the fix phase (1,080 s). Write a note. | Saved note names fix. Another place is an auto error |
| S7 experiment | Experiments tab, pick the two branches, run at x30 (480 s of sim time), read the verdict, write a note. | Experiment complete and note names lru. The other branch is an auto error |
| S8 live then slow calls | R-801 is in its fix phase, with build history. Open its output (live), then Time breakdown: phase build, minimum 2 s, window from the card, click the slowest bar. | Output open on the slowest call, filters and window match the card within 20 s |
| S9 decide from context | A command, an edit and a failed gate. Open the context view with `i`, decide with keys, type or choose what was missing. The gate item deliberately has no "would do" and no history. | All three decided, each with a "missing" answer. Leaving the context view is flagged |
| P1 | Change the line (top bar menu or filter chips). | Line is release-4 |
| P2 | Four runs are working; find task T-07-003's on Tracks. | Tracks open with R-701 selected |
| P3 | A permission request is waiting; say what came before it. | Facilitator marks correct or wrong |

The probes exist because the nine scenarios only touch the open questions. P1 isolates Q4, P2 isolates Q1 and P3
isolates Q3.

## The five open questions

Variants are switched in the facilitator panel. The protocol gives the rotation across participants.

### Q1. How is a run identified?

- **Variants built [M].** `id` (R-118), `lane` (a lane letter, B), `position` (#2 in the current order), and `colour`
  (a control that breaks the rule that colour means state; it is there to show what that costs).
- **What sessions must observe.** In S4, S5 and P2: time to the right run, wrong runs picked, whether the participant
  names runs aloud by the label or by something else (task title, "the third one"). For `position`, whether an
  answer changes the number so the participant loses the run. For `colour`, whether they confuse it with state.
- **What settles it.** Fewer wrong-run errors and lower time to the right track. A tie keeps `id`.

### Q2. What do `a` and `d` mean per Queue kind?

- **Variants built [M].** `literal` (a is yes, d is no or destroy: gate a proceeds, d stops, r retries; review a
  accepts, d discards, r re-prompts; budget a raises the cap by $2, d stops), `safe` (a is retry or accept, d is
  hold or send back; gate a retries, d holds, r stops; budget a continues by $1, d pauses), and `confirm` (literal
  keys, but a consequential action needs a second press, shown in an "armed" banner, with `u` to undo).
- **What sessions must observe.** In S4 and S9: the participant says what a key will do, then what it did. Count
  every mismatch. Count discards and stops they undo or regret. Count the time the confirm step costs, and whether
  they ever press Enter without reading the banner.
- **What settles it.** Fewest mismatches and regretted destructive actions, with an acceptable time cost.

### Q3. Where does `o` take you from a Queue item?

- **Variants built [M].** `moment` (the run's track, zoomed on the moment of the decision), `track` (the track at the
  usual zoom), `output` (the output pane at the last call before the ask).
- **What sessions must observe.** In P3 and in S4: time to a correct answer to "what was the agent doing before it
  asked", whether the answer is right, whether the participant reaches for the other tabs afterwards.
- **What settles it.** The variant with the most correct answers; time breaks a tie.

### Q4. Where does the line switcher live?

- **Variants built [M].** `top` (a menu in the top bar) and `filter` (chips in the Field filter bar).
- **What sessions must observe.** In P1: time to switch, whether they look in the wrong place first. Elsewhere:
  whether they notice which line they are on (ask in the follow-up of S2).
- **What settles it.** Time and wrong-place looks in P1. If both are equal, prefer the top bar, as it stays visible on
  every surface.

### Q5. What does "review ×2" mean?

- **Built [M].** Not a switch. In the data, a finished run has two reviewers, A and B, who run in parallel in the
  full workflow, and the Review tab shows both sets of findings. It is not two rounds.
- **What sessions must observe.** In S1 and S5: whether the participant reads both reviewers, and, when asked, what
  they say "two reviewers" means. If they say "two rounds" or "first and second pass", the label is wrong.
- **What settles it.** The participant's own words in the follow-up question after S5.

## Thresholds

**Set by the owner on 2026-11-14: the proposed time and error limits and the pass rule, unchanged. Do not change them
after the first session.** The numbers began as my estimates [I] from the number of steps in the scripted pilots and
from the x30 speeds. They are not measured on people.

Rule for each scenario: it passes if the median time over participants is at or under the limit, no more
than one participant in four gives up, and the median errors are at or under the error limit.

| Scenario | Time limit | Error limit (median) | Note |
| --- | --- | --- | --- |
| S1 | 150 s | 1 | includes about 20 s of waiting for the run at x30 |
| S2 | 60 s | 0 | |
| S3 | 60 s | 0 | auto error for a separate launch |
| S4 | 150 s | 1 | five items |
| S5 | 45 s | 0 | |
| S6 | 90 s | 1 | and a correct note in at least 3 of 4 |
| S7 | 120 s | 1 | includes about 16 s of waiting for the result at x30; correct note in at least 3 of 4 |
| S8 | 120 s | 1 | |
| S9 | 120 s | 1 | and "nothing missing" for at least 2 of the 3 items, at the median |
| P1 | 15 s | 0 | |
| P2 | 20 s | 0 | |
| P3 | 60 s | 0 | and a correct answer |

Still a proposal (the owner set the limits, not this rule): for a variant to win a question there is no significance
test; sessions are too few for that. Pick a variant if it has
at least two fewer errors in total than each other variant, or if every participant who tried both names it as the
preferred one. Otherwise keep the proposed default: `id`, `literal`, `moment`, `top`.

## Pilot results [M]

A pilot is a scripted walkthrough by the builder. It shows each scenario can be completed in the prototype and that
the end state is detected. It was run with `pnpm test`; all 50 tests pass. Wrong paths are checked too.

| Scenario | Reducer pilots | UI pilot | Result |
| --- | --- | --- | --- |
| S1 | 1 | 1 | pass |
| S2 | 2 (right and partial filter) | 1 | pass |
| S3 | 2 (group and separate launch) | 1 | pass |
| S4 | 4 (five kinds; keys; confirm variant; undo) | 1 | pass |
| S5 | 3 (Queue; Field and Review tab; other run) | 1 | pass |
| S6 | 3 (compare; gap sits in fix; wrong finding) | none | pass |
| S7 | 1 | none | pass |
| S8 | 5 (state; target unique with decoy; filters; wrong call; card times) | 1 | pass |
| S9 | 3 (facts first; decide and say; left-view flag) | 1 | pass |
| P1, P2, P3 | 3 | none | pass |
| `o` with nothing selected | 1 | none | pass |
| Facilitator: load, start, stop, export; variant switch | none | 2 | pass |
| Protocol: each card in `SESSION-PROTOCOL.md` word for word | 12 | none | pass |

One run of the suite on a heavily loaded machine (load average about 24) failed 5 tests on the default 5 s timeout; the
rerun passed, and the test timeout is now 30 s. Also [M]: `tsc --noEmit` is clean and `pnpm build` succeeds. I looked at the Field, Queue (with the context view),
Tracks (with the zoomed moment view after `o`) and the facilitator panel in Chrome. The layout holds at about
1,400 by 800. Problems found there and fixed: the facilitator panel had uneven gaps, the Tracks strip fell below the
fold on Field, and `o` did nothing when no item was selected. That check was by eye, by me, and counts for nothing
on usability.

## What changed from the mockups and why

- **Run identity does not use colour [I].** Colour is reserved for state (working, waiting, gate, done). Identity is
  a switchable label, so Q1 can be tested; the `colour` variant is kept only as a control.
- **Queue is a detail pane under a Field home [I].** The Field is the home page and the Queue rail is always visible
  on it; the full Queue is one key away. This follows the B mockup's detail pane.
- **A real-time axis on Tracks [I].** The mockups drew phases by width only. Here the axis is wall time, so waits,
  the projected remainder and the reset line have a place, and scenarios S6 and S8 are possible.
- **A context view with the gaps shown [I].** Missing sections say so, instead of being hidden. S9 depends on this:
  the gate item lacks two sections on purpose, to find what people want and cannot get.
- **Review ×2 as two reviewers [I].** Reviewers A and B run in parallel and their findings are shown separately.
- **Compare and Experiments surfaces added [I].** The mockups had no place for S6 and S7.
- **Time breakdown tab added [I].** S8 needs filters on tool, duration and window, and a click-through to the call.
- **Probes added.** The nine scenarios only touch Q1, Q3 and Q4; the three probes isolate them.
- **Facilitator mode added.** Not in the mockups; it is the test harness.

## Proposed plan changes (proposals only; the plan was not edited)

All of these are [I] until sessions run.

1. **Wait spans are data.** Record time waiting on the user as its own span kind linked to the Queue item, so a
   phase's time can be split into work and waiting. Scenario S6 is only answerable if this exists.
2. **Time totals need three numbers.** Wall time of a phase, the sum of its calls, and the union of its calls differ
   once calls run in parallel or nest (subagents, Bash by executable). The breakdown shows all three. The span model
   needs a parent id for this.
3. **The context view is a data contract.** Define the six sections as fields the provider fills, and require the UI
   to show "Not available" rather than hide a missing one. Keep the agent's own words last.
4. **Do not use colour for run identity.** Pick the label from Q1 once sessions are done. Until then show the id.
5. **Make the Queue answer keys per kind explicit,** with one table in the plan, and decide Q2 from sessions before
   freezing it. Whatever is chosen, keep an undo.
6. **An experiment result states its scope.** Show "on this task" with the verdict, the critical-path difference and
   whether outcomes are equal, so one run is not read as a general rule.
7. **Add a fake-data usability gate before product code.** The owner sets the thresholds first (done here), then a
   person who did not design it runs the nine scenarios.

## Left unmeasured

- **[U]** All human performance: time, errors, preference. No participant has used it.
- **[U]** Learning and carry-over. Probes repeat a task; later variants will be faster. The protocol rotates the order
  but cannot remove this.
- **[U]** Scale. 140 tasks and a handful of runs. The output list is not virtualised; a real run with thousands of
  calls was not tried.
- **[U]** Real agent text. The streams are scripted and short, and not like real output in tone or variety.
- **[U]** Other screen sizes, touch, keyboard layouts other than US, screen readers, and the light theme (the CSS has
  one; I looked only at the dark one).
- **[U]** Whether the fake clock (x30 for S1, S3, S7) changes how people judge waiting.
- **[U]** Facilitator effects: how much the way the cards are read changes the results.
- **[U]** Not built: command palette and Plan menu, the Planning view, a virtualised output list, a real New session
  flow and pack catalogue, and real agents.

## Session results

Not run. Fill this in from `sessions/s6-p<N>.json` once sessions are done: time and errors per scenario against the
thresholds, the winner or the tie for each of Q1 to Q4, and the words used for Q5.

## Reproduction

```sh
cd .collide/planning/spikes/sp-0009-bb-interface-prototype
pnpm install                   # local store in .pnpm-store/
pnpm test                      # 50 tests: pilots, UI pilots, protocol check
pnpm typecheck && pnpm build   # type check and production build (dist/ is git-ignored)
pnpm dev                       # http://127.0.0.1:5173/?facilitator=1
```

Run sessions as `SESSION-PROTOCOL.md` says. Logs go to `out/sessions/` (git-ignored); copy the ones to keep into
`sessions/`.
