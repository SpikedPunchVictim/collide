# S6 session protocol

**Not run.** The owner skipped the sessions on 2026-11-14 (no participants who did not design Collide). Kept in case
participants become available; see `record.md`, "Session results".

For the facilitator. One session is one participant, about 60 to 75 minutes. The participant must not have designed
or built Collide. The pass thresholds are in `record.md` (section "Thresholds"); the owner set them on 2026-11-14,
before the first session. Do not change them after seeing data.

The test checks that every card below matches the app word for word (`pnpm test`, file `test/protocol.test.ts`).
If you edit a card, edit `src/scenarios.ts` too.

## Before the session

1. In the spike folder run `pnpm install` once, then `pnpm dev`. Open `http://127.0.0.1:5173/?facilitator=1`.
2. Give the participant the main window. Keep the facilitator panel (right side) on your own screen if you can.
   If you share one screen, hide the panel with its Hide button, and use the hotkeys below.
3. Type the participant id (`p1`, `p2`, ...) in the Participant box. Use an id only, never a name.
4. Set the four variant selects from the rotation table below. Do not tell the participant which variant is on.
5. Check the browser is at 100% zoom and in a window of at least 1400 by 800 pixels.

## Participant brief

Read this out. Do not add to it.

> Thanks for helping. We are testing a draft of a tool, not you. Nothing here is a real project and nothing you do
> can break anything. The tool helps one person run several coding agents and decide what they ask for.
>
> I will give you a short card for each task. I will start a timer when you begin and stop it when the task is done.
> Work as you normally would. Please say out loud what you are looking for and what you expect to happen, even if
> it feels obvious. If something is unclear, say so, but I will not be able to tell you where to click. If you want
> to give up on a task, say so; that is a fine result and I will write it down.
>
> Some of the data moves in real time. Some tasks have a clock that runs faster than real time; the app shows that
> in the top right. You do not need to do anything about it.

## Running a scenario

1. Pick the scenario in the facilitator panel. The app loads its fake start state and shows the card in the strip
   under the top bar.
2. Read the card out loud once. Answer only "What did the card say?" by reading it again.
3. Press Start (Alt+S). The timer runs. The app stops the timer itself when it sees the end state, except for
   probe P3, where you stop it with "Stop, correct" or "Stop, wrong".
4. Press Alt+E each time an error happens (see below). Type a short note in the Notes box when it helps.
5. If the participant gives up, or passes 5 minutes without progress, press Alt+S to stop and write "gave up" in
   the Notes box.
6. Ask the follow-up question for the scenario (below), write the answer in Notes, then pick the next scenario.

## Task cards

Read these exactly. They are worded without hints on purpose. Scenarios run in order 1 to 9. Probes P1 to P3 run
after scenario 9.

### Scenario 1. Fresh repo to a reviewed run (starts at x30 speed)

This repository is new: it has no tasks and nothing set up. You want a small change made: add a `--quiet` flag to the command line tool. Get an agent to make that change, then review what it produced.

End state the app detects: a run started from a task the participant made has finished and its review is decided.

### Scenario 2. Build a view and save it

Show only the tasks in epic E-07 that are ready and carry the label perf. Keep that selection under the name Slow spots, so you can come back to it later.

End state: a saved view named Slow spots whose tasks are exactly the five matching ones.

### Scenario 3. Launch a group of three from a view (starts at x30 speed)

The team keeps a saved selection called Ready in E-03. From it, launch tasks T-03-011, T-03-014 and T-03-019 together as one group, and nothing else.

End state: one launch group whose runs are exactly those three tasks. Launching them one at a time is flagged as an
automatic error.

### Scenario 4. Clear five pending decisions

Five things are waiting for an answer from you. Deal with all of them, as you would on your own project.

End state: all five items answered. The items are a permission, an edit, a failed gate, a budget cap and a review.

### Scenario 5. Review a finished run and accept it

The run for task T-05-009 has finished. Look at what it did, and accept it.

End state: the run for T-05-009 is accepted. Another finished run (R-502) is a distractor.

### Scenario 6. Compare two runs and say where the time went

Task T-04-003 was run twice and both runs finished. The second took much longer than the first. Compare the two runs and write down, in one line, where the extra time went.

End state: a saved one-line note on the pair. The note is auto-checked for the word "fix" (the phase where two long
permission waits sit). A note that names another place is flagged as an automatic error.

### Scenario 7. A/B experiment from two branches (starts at x30 speed)

On task T-04-012, set up a test of branch exp/ttl-cache against branch exp/lru-cache. Run it, read the result, and write down which branch won and by how much.

End state: the experiment is complete and a note is saved. The note is auto-checked for "lru". Correct answer: lru,
about 14 minutes shorter on the critical path (58 against 72). A note that names the other branch is an automatic
error.

### Scenario 8. Live output, then slow tool calls

A run is working on task T-07-003 right now. Watch what it is doing. Then, in its build phase, find the tool calls that took longer than 2 seconds between 09:42:00 and 09:49:00, and open the output at the slowest of them.

End state: the output pane is open on the slowest matching call, with the build phase, the 2 second minimum and the
window set within 20 seconds of the card. The clock times on the card are fixed by the data.

### Scenario 9. Decide three items from the context view

Three requests are waiting: a command, an edit and a failed gate. Decide each one using only the explanation the app gives for why it is being asked, without opening the run's own output. After each decision, say what you wanted to know and could not find, or that nothing was missing.

End state: all three decided, and a "what was missing" answer recorded for each (type it into the box the app shows,
or choose "Nothing missing"). If the participant opens the run output or the Tracks view, the app flags "left the
context view". Do not stop them; note it.

### Probe P1. Switch line

The team also works on a second line, release-4. Switch to it.

Auto-detected. Run it once for each line switcher placement (see "Probes").

### Probe P2. Find a task's run

Task T-07-003 is being worked on right now. Open the track of the run that is working on it.

Auto-detected. Run it once for each run identity variant (see "Probes").

### Probe P3. What led to the request

A permission request is waiting. Find out what the agent was doing in the two minutes before it asked, and tell me.

No detector. Stop the timer by hand when the participant has answered, then mark it "Stop, correct" or "Stop, wrong".
The item is `cargo add tiny-backoff` on run R-702. The "led here" section of its context view lists the last steps:
`cargo test -p queue` failed twice (exit 101), then the agent edited `tests/queue_retry.rs`. Mark "correct" if the
participant names those steps in that order. Run it once for each `o` variant (see "Probes").

## Follow-up questions

Ask these once the timer has stopped. Write the answers in Notes. Do not argue.

- After every scenario: "What was the hardest part?"
- After scenario 4: "How did you tell the five apart? Did you ever press a key and get a result you did not expect?"
- After scenario 5: "The screen showed findings from two reviewers. What does that mean to you?"
- After scenario 6: "How sure are you of that answer?"
- After scenario 9: "Which request would you have wanted to open the output for?"
- After the probes: "Which version of the same thing did you prefer, and why?"

## What counts as an error

Press Alt+E for each one. One mistake is one error, even if it lasts a while.

Count it when the participant:

- acts on the wrong run, task or item (a wrong accept, deny or launch, or opening the wrong run's output),
- does the opposite of what they said they meant to do (says "accept" and denies, or the key does something other
  than what they expected, for example `a` retries when they thought it approved),
- takes an action and then undoes it (`u`, or Discard then redo) because it was not what they intended,
- misreads the screen in a way that changes what they do (says a run is blocked when it is working),
- asks you where something is, or asks you to confirm they are right (count it, and tell them you cannot say).

Do not count it when the participant:

- looks around, opens panels, scrolls, or reads before acting,
- changes their mind before pressing anything,
- is slow,
- hits a prototype limit listed as not built in the README (note it instead).

The app also logs errors by itself: a separate launch in scenario 3, and a wrong finding in scenarios 6 and 7. They
are marked "auto" in the log and the participant's total counts them. Auto flags that are not errors (scenario 9
"left the context view") are listed in the log under `autoFlags`. Do not press Alt+E twice for the same thing.

## Order of variants

The four open questions each have variants (the full list is in `record.md`). Across participants rotate them with
this table. The pattern repeats from participant 5.

| Participant | Q1 identity | Q2 keys (a and d) | Q3 `o` opens | Q4 line switcher |
| ----------- | ----------- | ----------------- | ------------ | ---------------- |
| p1 | id | literal | track | top |
| p2 | lane | safe | output | filter |
| p3 | position | confirm | moment | top |
| p4 | colour | literal | track | filter |
| p5 | id | safe | output | top |
| p6 | lane | confirm | moment | filter |
| p7 | position | literal | track | top |
| p8 | colour | safe | output | filter |

Use the selects in the facilitator panel. Q5 (review by two reviewers) has no variant; it is in the data.

With fewer than eight participants some cells will be empty. That is expected. Do not reuse a participant to fill
them; the point is a fresh reader each time.

## Probes

Run after scenario 9, once per variant of the question under test. Use the same participant. Pick the variant in
the panel, load the probe, press Reload, then Start. Rotate the order of the variants by participant (start with the
variant in the table, then continue down the list in the order below). Note in Notes that the participant has seen
the task before; times after the first are lower bounds on the true difference.

| Probe | Question | Variants, in order | What to watch |
| ----- | -------- | ------------------ | ------------- |
| P2 | Q1 identity | id, lane, position, colour | time to the right track; any wrong run opened |
| P3 | Q3 `o` | track, output, moment | time to the answer; whether the answer is right |
| P1 | Q4 line | top, filter | time to switch; did they look in the other place first |

## Exporting the log

After the last scenario of the session:

1. In the facilitator panel press Save to out/sessions. The dev server writes a file under `out/sessions/`
   (git-ignored). The panel shows the path.
2. Also press Copy JSON and paste it into a scratch file, or press Download, as a second copy.
3. Copy the saved file to `sessions/` in the spike folder and rename it `s6-p<N>.json` (for example `s6-p3.json`).
   `sessions/` is not git-ignored. The participant is only an id in the file. Check it holds no name before you
   commit anything.
4. Do not reload the page before saving. The log lives in the page.

Hotkeys: Alt+S start or stop the timer, Alt+E add an error, Ctrl+Shift+F show or hide the facilitator panel.

## After the session

Write a few lines in the Notes box of the last attempt: what surprised you, and what the participant said unprompted.
Then fill the "Session results" section in `record.md` from the exported files: time and errors per scenario against
the thresholds, and for each open question what was seen.
