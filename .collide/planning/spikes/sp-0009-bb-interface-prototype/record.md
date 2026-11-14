---
id: sp-0009-bb
alias: S6
kind: spike
status: results
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
- **[M]** An in-app agent changes the screen through five MCP tools (`query`, `set_view`, `show_groups`, `highlight`,
  `open`), launched from an ask box by `claude -p` (haiku) or an ACP agent, with a Settings page for which agent.
  Both requests from the owner worked live with Claude Code haiku (sort 7 to 8 s, lens 18 to 20 s, about $0.002 and
  $0.005 per request). See "In-app agent (UI tools)" below.
- **[U]** No participant has used it. The owner set the thresholds below, then skipped the sessions (2026-11-14): no
  one who did not design Collide is available. The owner iterates on the prototype directly instead.

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
from the x30 speeds. They are not measured on people. Not used so far: the sessions were skipped (see "Session
results"). They stand if sessions with first-time users ever run.

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
   person who did not design it runs the nine scenarios. Withdrawn: the owner has no such participants (2026-11-14).
   Revision 11 must instead replace the plan's "non-owner participant" criteria (see "Session results").

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

**Skipped by the owner on 2026-11-14.** No one who did not design Collide is available to take part. The owner and
the agent iterate on the prototype directly instead. What follows from that [I]:

- The thresholds are not tested. Times the owner takes are a designer's times, not a first-time user's, so they say
  nothing about the limits.
- Q1 to Q4 keep the defaults (`id`, `literal`, `moment`, `top`) unless the owner picks another variant while
  iterating; that pick is the owner's judgement, recorded as such, not a session result.
- Q5 (what "review ×2" means to a reader) stays unchecked; the owner already knows it means two reviewers.
- The plan names a non-owner participant in Stage 1's S6 item and criteria, and in later stages' criteria that
  reuse S6 scenarios and thresholds in the real app. Revision 11 (`ts-0005-st`) must replace those with checks the
  owner can run.

`SESSION-PROTOCOL.md` is kept in case participants become available. If sessions ever run, fill this section from
`sessions/s6-p<N>.json`: time and errors per scenario against the thresholds, the winner or the tie for each of Q1 to
Q4, and the words used for Q5.

## In-app agent (UI tools)

Added after the pilots, at the owner's request: can an agent change this interface through tool calls, and what does
it take to bring your own agent? Throwaway code, like the rest of the spike. The 50 earlier tests are unchanged and
the original scenarios use the plain `initialState`; the agent demo has its own state (`src/agentDemo.ts`).

### What was built [M]

- **One reducer still holds all state.** Every agent tool call becomes a typed reducer action (`src/agentReducer.ts`),
  so tests drive it without a DOM or a server. Each screen-changing call pushes an `AgentChange` (A-1, A-2, ...) onto
  a stack and stores a snapshot of the view state it replaced, so Undo restores exactly that.
- **Two kinds of result, kept apart.** A *view* is typed filter, sort and group-by state (Field and Tracks) that can be
  saved as a View. A *lens* is the agent's own judgement: named groups of ids, each with a reason, a snapshot stamped
  with the time it was made, ids exclusive across groups, the rest dimmed or hidden. A lens does not update when tasks
  change; it can be saved as a named snapshot, not as a view.
- **Field and Tracks.** The Field draws a lens as a board of groups with the reason under each name. Tracks sorts and
  filters runs (by total, start, end, spend, wait, or seconds in a phase such as build; missing values last), draws a
  lens as bands, and writes the sort value on each row.
- **Chips.** "Agent: <title>" with the expression (`true | sort phase_duration("build") desc`) or the group reasons,
  the source, Undo (last change only), and "Save as view" or "Save snapshot".
- **Ask box** in the top bar (every surface, also on `/`). It sends the request to the dev server, shows one line per
  tool call as it happens, then the chip. Failure is shown as text: agent error, timeout, budget, cancelled, or "the
  agent finished without changing the screen (N tool calls, none changed anything)". Cancel while working.
- **Agents page** (key 5, "5 Agents"): name, kind (Claude Code, ACP agent, External), command or path, arguments,
  model, readable folder, budget and timeout per request, environment lines, which agent the ask box uses. An External
  entry shows the endpoint and three text snippets (`claude mcp add --transport http collide <url>`, a JSON
  `mcpServers` block, an opencode `mcp` block). Collide writes to no agent's configuration. Saved through
  `PUT /__settings/agents` to `out/agents.json` (git-ignored).
- **Fixtures.** `fixtures/repo/` is a 24-file invented Node CLI (a pantry and expiry tracker: `src/cli`, `src/parse`,
  `src/store`, `src/notify`, `src/report`, `test/`). Epic E-09 has 12 ready tasks whose bodies describe behaviour and
  name no file. Intended grouping: quantities (T-09-001, 002), expiry (004, 005, 006), reports (008, 009, 010), data
  file (011, 012). T-09-003 (touches the quantity code and the error messages) and T-09-007 (touches expiry and the
  list command) are ambiguous: either area, or left out, counts as right. Nine finished runs have distinct build
  seconds and one is live, so "longest in build" has one right order.

### The tools [M]

MCP server `collide-ui`, in a Vite plugin (`server/plugin.ts`, `server/mcp.ts`). Streamable HTTP, stateless (a new
server and transport per request), JSON responses, `@modelcontextprotocol/sdk` 1.32.2 with zod 4.6.7, at
`http://127.0.0.1:5173/mcp`. `?ask=<id>` is added when Collide launched the agent, so the ask box can show each call.
Calls reach the open page over Vite's HMR websocket (`collide:tool-call`, `collide:tool-result`); the page runs the
call against its state with a pure `execTool` and dispatches the action. With no page open a call fails with a plain
error; if the page does not answer in 10 s the call fails with that.

| Tool | Input | Effect |
| --- | --- | --- |
| `query` | `subject` ("tasks" or "runs"), `filter?` {epic, status, label, source, text, ids, task, workflow}, `limit?` | none. Returns tasks (id, title, body, epic, status, labels, source, depends_on) or runs (id, task, status, workflow, started, total_seconds, wait_seconds, spend_usd, phases with seconds) |
| `set_view` | `surface`, `title`, `reason?`, `filter?` ({} clears), `sort?` {by, dir}, `group_by?` (field), `window?` {from, to} (tracks) | sets that part of the view; chip with expression |
| `show_groups` | `surface`, `title`, `groups` [{name, ids, reason}] (1 to 12), `rest?` ("dim" or "hide") | lens with reasons; chip with Save snapshot |
| `highlight` | `surface`, `ids`, `note?`, `title?` | marks ids; nothing is filtered |
| `open` | `surface`, `id?` | opens field, tracks, queue or experiments, and one item |

No tool launches, accepts, edits or deletes. Validation errors say what to do: `unknown surface "x" for set_view;
supported: field, tracks`, `unknown task id T-01-20; nothing was shown` (nothing is applied when any id is unknown),
`unknown subject "tracks"; known: tasks, runs`, `unknown run status "running"; known: working, waiting, ...`, an id in
two groups, a group without a reason, a bad sort key or window. A weak model recovered from these messages in 2 to 4
calls (opencode runs below) [M].

### Launching Claude Code [M]

`server/agents.ts`. The request goes to the child on stdin. Exact argv (the request is not in it), recorded in each
`out/asks/ask-*.jsonl` start event:

```
claude -p --model haiku --output-format stream-json --verbose
  --mcp-config '{"mcpServers":{"collide":{"type":"http","url":"http://127.0.0.1:5173/mcp?ask=<id>"}}}' --strict-mcp-config
  --tools Read,Grep,Glob
  --allowedTools Read,Grep,Glob,mcp__collide__query,mcp__collide__set_view,mcp__collide__show_groups,mcp__collide__highlight,mcp__collide__open
  --max-budget-usd 0.25 --no-session-persistence --disable-slash-commands
  --append-system-prompt "<SYSTEM_PROMPT in server/agents.ts>"
```

cwd is the readable folder (`fixtures/repo`). The child env is the parent's with every `CLAUDE*` and `OTEL_*` variable
removed (the parent is itself usually a Claude Code session). `--tools Read,Grep,Glob` removes Bash, Edit and Write
from the child altogether; `--allowedTools` alone would only skip prompts, not remove tools. The budget is enforced by
Claude Code (`error_max_budget_usd` is shown as "stopped at the $0.25 budget"); the timeout is Collide's (SIGTERM,
then SIGKILL).

### Live runs, Claude Code haiku [M]

Real `claude -p` runs through the ask endpoint, driven by a script that posts the request and a Chrome tab running the
page. "First tool" is the first MCP call; "first change" is the first call that is not `query`, which is when the chip
appears. Times in seconds from the request. Cost and tokens are from the stream's final result (input includes cache
reads and writes).

| # | Request | Prompt | First tool | First change | Finished | Calls | Correct | Input / output tokens | Cost |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | longest in build | v1 | 2.8 | 5.4 | 8.3 | query runs, set_view | yes: sort `phase:build` desc, order matches | 45.7k / 0.7k | $0.0013 |
| 2 | longest in build | v1 | 3.5 | 3.5 | 6.0 | set_view | yes, but did not read the numbers first | 27.6k / 0.3k | $0.0005 |
| 3 | longest in build | v2 | 2.4 | 5.2 | 8.3 | query runs, set_view | yes | 46.0k / 0.8k | $0.0023 |
| 4 | longest in build | v2 | 2.3 | 4.6 | 7.0 | query runs, set_view | yes | 45.9k / 0.6k | $0.0012 |
| 5 | same-file grouping | v1 | 2.3 | 22.4 | 25.4 | query tasks, show_groups | no: 11 groups of 107 open tasks across the whole corpus, from title wording; Read, Grep and Glob never used; it said so | 76.4k / 5.3k | $0.0082 |
| 6 | same-file grouping | v2 | 2.6 | 14.8 | 18.2 | query, Glob, Grep, show_groups | yes: 4 groups equal to the intended ones, T-09-003 with quantities, T-09-007 with expiry; named the other 61 ready tasks as left out | 104.5k / 2.6k | $0.0045 |
| 7 | same-file grouping | v2 | 2.5 | 17.2 | 20.4 | query, Glob, Grep, show_groups | yes: same four areas, T-09-007 with the reports and list group, T-09-003 with quantities; said which group was loosest | 107.3k / 3.1k | $0.0051 |
| 8 | "what should I work on next?" | v2 | 3.1 | 18.2 | 21.5 | 2 query, Glob, Grep, show_groups | plausible, no reference answer: put the crash-safe save first with a reason from the code (`store/file.js` writes in place), then the backup, CSV quoting, null expiry; grouped the 12 E-09 tasks by risk | 106.6k / 3.1k | $0.0059 |
| 9 | longest in build | v2, page reloading | 12.6 | none | 51.0 | 4 query, all failed ("The page did not answer within 10 s") | failure shown, correct: no change, the agent said it had changed nothing and guessed no ids | 72.0k / 1.0k | $0.0029 |

Prompt v1 said to read the code folder "when the request is about code". Run 5 showed a small model does not follow
a "when"; v2 says it must Glob and Read or Grep before grouping or ranking, to use only tasks that are about the code
in the folder, and to say which it left out. Nothing else changed between v1 and v2. Run 9 was an accident: editing
`server/agents.ts` restarted the dev server and the page had not yet reconnected. It is kept because it is the
timeout and failure path against a real agent. Across the table, the first tool call comes 2.2 to 3.5 s after the
request (mostly Claude Code starting), a lens takes 15 to 22 s to appear because the model writes every group with a
reason before the call, and a sort takes 4 to 5 s. Total Claude spend for the whole task: about $0.04 against the
$1 cap (the nine rows above sum to $0.032; the rest is a first run from before the per-ask log existed, one more sort
run done for the screenshot, and one probe call that returned no usage) [M].

Only the two requests in the brief plus one vague request were tried, each at most three times. Haiku is not shown to
be reliable at these; it was right in 6 of the 7 runs that have a reference answer, and the one wrong run was
repaired by the prompt, which is one data point for a prompt tuned to this demo [I].

### Bring your own agent, opencode 1.1.56 [M] for what ran, [U] for the rest

Setup as S7 ran it: project-local `opencode.json` in the spike folder, and `HOME` plus four `XDG_*` directories
under `work/opencode/` (git-ignored), `OPENCODE_DISABLE_CLAUDE_CODE=1`; the user's opencode config was not touched.
The Agents page entry is kind "ACP agent", command `opencode`, arguments `acp`, these as environment lines. Collide
speaks ACP itself (initialize, `session/new` with an HTTP `mcpServers` entry, optional `session/set_model`,
`session/prompt`; read permissions allowed, everything else refused).

- `opencode mcp list` with the project-local config: `collide ... connected` to the endpoint, so opencode's remote MCP
  client works against the streamable HTTP server with no sign-in [M].
- Default model: opencode's own hosted default (`opencode/big-pickle`). Driven through the ask box, the request
  timed out at 90 s with no event at all; `initialize` and `session/new` answered in under 4 s. I did not look further:
  it needs a hosted service I did not set up, and it falls outside "installed". Recorded as [U], not as a failure of
  the endpoint.
- With the S7 local model (`ollama/qwen2.5:7b`, config in the same `opencode.json`), the same two requests went through
  the real ask box and the same MCP endpoint [M]:

| Request | First tool | First change | Finished | Calls | Result |
| --- | --- | --- | --- | --- | --- |
| longest in build (run 1) | 46.6 s (model load) | 64.1 s | 69.9 s | query "tracks" (refused, valid subjects listed), query runs, show_groups | worse: three bands "Longest/Medium/Short" with R-101 and R-107 in the middle, the rest hidden; a sort was wanted |
| longest in build (run 2) | 12.3 s | 31.2 s | 33.5 s | three refused queries, query runs, highlight | partly right: R-102 and R-108 highlighted, the two longest; not sorted |
| same-file grouping | 4.1 s | none | 82.0 s | query tasks, show_groups (refused) | wrong: the model invented 8 ids ("T-01-20", ...), Collide refused all of it, nothing was shown; the model then listed a grouping in prose and said which ids it could not find. It never read the folder |

  The no-guessed-ids rule did its job: nothing wrong reached the screen. The model's prose still described a grouping
  that was not shown, so the chip and the status line (not the agent's text) must be what tells the user what
  changed. No cost or token figure came back (ACP has none) [M].
- Not measured: a second local model, any hosted opencode model, the ACP permission path under a real write attempt, a
  full set of three runs per request [U].

### Problems found [M] unless marked

- **The MCP endpoint has no authentication.** It is bound to 127.0.0.1 and refuses a foreign Host (DNS rebinding) and
  a foreign Origin, and requires a JSON content type (so a browser must preflight). Any local process can still call
  it, and with a page open can change the screen. No tool is destructive, so the exposure is the view, not the
  repository; a Collide that adds a launching tool must add a per-session token first [I].
- **Folder is not a sandbox for ACP.** For Claude Code the folder is the cwd and the tools are cut to Read, Grep and
  Glob. For an ACP agent the folder is only the cwd and what it is told; opencode's own tools decide what it can read.
  The child env is limited to PATH, HOME and the settings lines, and opencode's permission block denies edit and bash
  in its config, but that is the agent's promise, not Collide's guarantee [I].
- **ACP reports no cost, so the per-request budget cannot be enforced for it.** Only the timeout can [M].
- **Token load.** Each Claude Code run reads 28k to 107k input tokens, 46k for a one-tool sort. How much is the
  global user memory file and settings loading into the child, and how much is Claude Code's own prompt, was not
  separated (one attempt with an emptied environment returned no usage and told me nothing) [U].
- **A restarted dev server leaves the page unreachable for a few seconds**, and every call fails with the 10 s
  message (run 9). A real product would not have this, but "the page is not open" is a state an agent must be told
  plainly, and it is [M].
- **The lens costs 15 to 22 s**, nearly all of it the model writing reasons. Showing the first group before the rest
  was not tried [U].
- A tab in the background may throttle the websocket answer; not tested [U].
- Visual check: I looked at the Field with a lens, the Tracks sorted by build time with its chip, and the Agents page
  in Chrome, by eye. It is not usability evidence [M].

### What a real Settings page needs (plan section 0.7) [I]

The prototype has one flat list in one git-ignored file. A real one needs:

- **Layers.** Agents, folders and budgets exist in user, project and pack layers (0.7), with the same precedence
  and a visible "this value comes from X" per field; here there is only one file.
- **Packs and profiles.** A pack may bring an agent entry; a profile picks agent, model and budget per kind of work
  (an ask, a plan, a run). Here one agent serves the ask box and nothing else.
- **Secrets.** Environment lines are plain text here. API keys must go to the OS keychain and be referenced by name.
- **Permissions as data.** The readable folder should be a set of allowed paths and tool kinds per agent, shown in
  words ("can read these folders, cannot write"), checked by Collide for agents that cannot be cut down by flags.
- **Connection state and a test.** A "test this agent" action that runs the smallest request and reports first tool
  call and cost; MCP endpoint status (a page connected, the last call); per-agent last-used and spend this week.
- **Endpoint security.** A token in the MCP URL or header, shown in the snippets, rotated from the page.
- **Validation and undo of settings changes**, with a diff before save. Here a PUT validates; the page has no undo.
- **External agents.** Snippets for each common client, generated from the real endpoint and token; a record of
  which external agents have connected.

### Proposed plan changes (proposals only; the plan was not edited)

1. Treat "the agent changes the screen" as typed reducer actions with a snapshot Undo from the first stage that has an
   ask box. It worked here without special cases, and tests need no browser.
2. Keep views and lenses as two things. A lens is a timestamped judgement; saving it is a snapshot, not a View, so a
   saved View never silently holds an agent's guess.
3. Make the agent read the repository for code-shaped requests a tool-side requirement, not only a prompt line: a
   `show_groups` call that names code-based reasons without any file read in the same ask could be flagged on the chip
   ("no files were read"). Run 5 is the case.
4. Reject, not repair, ids an agent invents (done here), and make the chip, not the agent's text, the statement of
   what changed (opencode run 3).
5. Put the MCP endpoint behind a per-session token before any tool can launch or accept anything.
6. Decide whether the budget is a product promise. It is enforceable for Claude Code only; for ACP agents it is a
   timeout.
7. Add a settings test action and the layers above to the settings stage, as acceptance, not as later polish.

### Owner's decisions (2026-11-14)

The owner answered the open questions of the in-app agent, and the plan's revision 12 applies them. (1) The ask box's
default agent is Claude Code with haiku; ACP agents are selectable on the Agents settings page, and External MCP
clients are set up there too (they connect on their own, so the ask box cannot launch one). (2) The UI-tools MCP
endpoint requires a per-session token from day one, in the URL or a header, shown in the External snippets and
rotatable from Settings. (3) When Collide launches Claude Code for the ask box it uses a strict launch line
(Collide's own settings, not the user's CLAUDE.md, hooks or memory, which is stricter than a run's default;
`--strict-mcp-config`; `--tools Read,Grep,Glob`; `--no-session-persistence`), with a per-agent opt-in "use my Claude
config"; the prototype did none of this, and each ask loaded 28k to 107k input tokens [U whether that is the user
config]. (4) A per-ask budget is optional and off by default, enforceable only for Claude Code (`--max-budget-usd`);
every agent gets a timeout, and the Agents page says plainly that ACP and External agents get a timeout only.

## Reproduction

```sh
cd .collide/planning/spikes/sp-0009-bb-interface-prototype
pnpm install                   # local store in .pnpm-store/
pnpm test                      # 97 tests: 50 earlier (pilots, UI pilots, protocol) and 47 for the in-app agent
pnpm typecheck && pnpm build   # type check and production build (dist/ is git-ignored)
pnpm dev                       # http://127.0.0.1:5173/?facilitator=1
```

Run sessions as `SESSION-PROTOCOL.md` says. Logs go to `out/sessions/` (git-ignored); copy the ones to keep into
`sessions/`.
