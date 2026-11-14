---
id: sp-0008-bk
alias: S11
kind: spike
status: concluded
title: "S11: Derived-point coverage"
owners: []
links: []
---
# S11: Derived-point coverage

- **Date:** 2026-11-13. Plan revision 10 (`IMPLEMENTATION_PLAN.md`: 0.4 "Named points", "Derived points",
  "Lifecycle order", "Observers"; 0.5 "Input"; 0.10 "Protocol", "Writes are operations, not field patches",
  "Planning sources"; the Skeleton stage, "Planning contracts"; Deferred "Points for changes made outside Collide").
  `docs/research/planning.md` sections 2 and 7 are treated as a proposal under test. Neither file is edited here;
  the last sections propose changes.
- **Paper spike.** Nothing was installed or run except read-only `git log` in the Orchard repo. No beads, Obsidian or
  plan CLI code was executed.
- **Question.** Do the frozen point catalogue and hook payload cover every real touchpoint of planning backends
  and issue trackers? What is missing? Owner decision Q11 (no point for changes Collide only reads) is measured
  per backend.

**Labels.** **[read `file, symbol`]**, **[read URL, heading]**, **[inferred]**, **[unverified]**. A bracket that
starts with a source key below is a **read** label.

| Key | Source |
|---|---|
| `plan` | `IMPLEMENTATION_PLAN.md`, section and bold lead-in |
| `pn` | `docs/research/planning.md`, section |
| `pc` | the plan CLI (README, or a described source part) |
| `gz` | other Orchard repo files |
| `log` | read-only `git log` over the Orchard repo (method under Coverage) |
| `R` | beads `docs/` on GitHub, raw files of the main branch (the GitHub API now lists the repo as `gastownhall/beads`), page and heading |
| `D`, `C` | `https://docs.github.com/en/…`, `https://cli.github.com/manual/…` |
| `OH`, `OD` | `https://obsidian.md/help/…`, `https://docs.obsidian.md/Reference/TypeScript+API/…` |

`R`, `D`, `C`, `OH` and `OD` pages were read by research agents through WebFetch, which returns a model's
summary. The wording is second-hand.

**Mapping classes** (used in every touchpoint table and in Coverage):

| Class | Meaning |
|---|---|
| **P** | Collide makes the write. A core or derived point fires, and `change` carries what a `when:` needs. |
| **P~** | A point fires, but the payload or the firing rule falls short. The gap is named. |
| **O** | Outside Collide. Observers and the chip only, and that is right: a foreign writer, or nothing to gate. |
| **O-run** | An agent inside a Collide phase made the write. Q11 gives it observers only, but a gate or reaction is wanted (G1). |
| **X** | No defined mechanism, not even for observers. |
| **–** | No such path. |

## Summary

- **Verdict: the point model is right for Collide's own writes, but the payload is thin.** Gates belong on
  `before_*` points, and derived points with `when:` expressed every planning moment in `pn` 7.5 for all five
  backends. But of 38 Collide-made touchpoints, 22 are **P~**. The payload carries status only (G2). It has one item
  per op (G3) and no bulk rule (G4). Linked items' state is missing (G5). Create, delete and renames are undefined
  (G9). These must land before the skeleton freezes `change` [read plan, Skeleton stage, "Planning contracts"].
- **Q11 mixes two kinds of outside writes.** A teammate's push and an agent running `bd close` *inside a Collide
  phase* are both "changes Collide only reads". The first should fire nothing. The second is made on this machine,
  by a run Collide owns, so Q11's own rule ("a point fires once, on the machine that makes the write" [read pn
  7.1]) says it should fire. 13 of 69 touchpoints are of this second kind (**O-run**). This is gap **G1**, the
  largest.
- **Another mechanism serves better for foreign writes.** That mechanism is not more points. It is a normalized
  change event with a diff, a cause and the source's actor (G7). The chip should be a field that views and a Queue
  lane can filter. 11 touchpoints have no defined mechanism at all: renames, transfers, deletes, conflict markers,
  and changes made while Collide was closed (G7, G8).
- **Accept order defeats `done` gates.** Accept runs `integrate` before the source's operations [read plan 0.4,
  Lifecycle order]. So a `task.before_status: done` refusal comes after the code is merged, and a GitHub PR keyword
  has already closed the issue (G6).
- **A rule that must hold for every writer belongs in the backend.** The plan CLI already refuses an ADR `accepted`
  while its evidence spike is open, on every `plan set`. It does this for any writer, agent or human [read pc,
  the write gate and the rule registry]. The write gate runs every rule, the ADR-evidence rule (severity error)
  included, and refuses an error the edit introduces [inferred: the rule fires once the ADR is accepted, so the error
  is new]. This contradicts `pn` 7.7, "the plan CLI checks it only in `plan check`".
- **Q11 per backend** (share of activity left without points):

| Backend | Who writes | Share without points under Q11 | Holds? |
|---|---|---|---|
| plan CLI (Orchard) | the lead (terminal), builder agents in worktrees | **≥16 %** of status writes (agents' worktree commits; measured, heuristic). Up to 100 % of what the lead keeps doing in a terminal. | Holds with G1 (ref-scoped: fire at integrate). Without it, a review gate never sees the builder's own `in_review`. |
| beads | agents through `bd` by design, humans, cron (`gate check`, sync) | **majority** [inferred]: claim, close and discovered-work creates are the agent loop [read R cli-reference/update.md, close.md] | **No.** Needs G1 (global: attribute by `BEADS_ACTOR`, after points, phase-end checks). |
| GitHub | teammates (web), Actions, Projects automations, PR keywords | **majority** [inferred], mostly foreign | Yes for foreign writers. No for the local agent's `gh` (G1) or for Collide's own PR merge (G6). |
| Obsidian | the person in the app; plugins | **~all** [inferred] | Holds trivially. Gates are nearly useless: nothing can veto an in-app edit [read OD Vault, events]. |
| markdown (built-in) | Collide; rare hand edits | **small** [inferred] | Holds, with G1 for agents' edits. |

## The plan CLI (Orchard pack)

**What the backend does** [read pc README, Edit]:
- Every `plan new` and edit command is a gated write. It runs diagnostics on the before and after content in memory
  and refuses if a new error appears. The edit result lists other files whose generated regions were
  refreshed [read pc, the write gate].
- `--dry-run` works on every mutating command [read pc README, Edit].
- `plan set` writes scalar fields only and refuses array fields [read pc, the set command].
- Transitions are refused at write time [read pc, the transition table].
- No hooks or events are emitted. The Orchard repo has no installed git hooks and no CI job running `plan check`
  [read gz `.git/hooks`, `.github/workflows`].
- Worktree agents run `plan set <id> status=in_review` and commit. They must not run `plan new` or `plan
  supersede`, or set `done`, `reviewed_by` or `self_review` [read gz, the agent coordination protocol].

| # | Touchpoint | Writers | How Collide learns | Via Collide | Outside | Point or gap |
|---|---|---|---|---|---|---|
| 1 | `plan new task\|adr\|spike\|question\|obligation` | lead; Collide `create`; Share | watch: new file in a checkout | P~ (no key at `before_op`: G9) | O | `decision.after_propose`, `question.after_raise` |
| 2 | `plan set status=doing` | Collide `on_start`; lead | watch | P | O | `task.before_status: active` |
| 3 | `plan set status=in_review` | builder agent in its worktree; Collide `on_end` | the workspace checkout via watch; the line via `integrate` | P | **O-run** | `orchard/task.before_review` |
| 4 | `plan set status=done` + `reviewed_by`/`self_review` | lead; Collide accept | watch | P~ (fires after merge: G6; reviewer not in `change`: G2) | O | `task.before_status: done` |
| 5 | `plan close` (status, `closed_by`, History row) | lead | watch | P~ (fields beyond status: G2) | O | op `close` |
| 6 | `plan resolve --by` | lead; agents in spike runs [inferred] | watch | P | **O-run** | `question.before_resolve` |
| 7 | `plan supersede OLD --with NEW` (two items, gated together) | lead | watch: two files | P~ (NEW gets nothing: G3) | O | `decision.after_supersede` |
| 8 | `plan move --epic` | lead | watch: `collections` | P~ (G2; the op name collides with move points: G9) | O | `orchard/task.after_rehome` |
| 9 | `plan renumber OLD NEW` (renames the file, rewrites every citation, adds an ID-MAP row) | lead at merge; provider at Share | `task.renamed` from ID-MAP and git renames [read plan 0.10, Identity] | P~ (no id map at `after_move`: G9) | X (`task.renamed` shape undefined: G7) | — |
| 10 | claim (`owners`), `evidence`, `depends_on`, `blocks` after creation | no CLI writer (the set command refuses arrays) | watch | X (P1) | O | `spike.*_claim` has nothing to bind |
| 11 | hand edit of an item file | lead, editors | watch | – | O | chip |
| 12 | pull, merge, rebase | lead, teammates | watch: head moved | – | O | chip |
| 13 | conflict markers or a generated-region conflict after a merge | git | a counted parse error [read plan, plan-adapter stage ("Pack loading, settings, plan adapter, corpus and views") tests] | – | X (G8) | — |
| 14 | `plan generate`, `fix --slugs`, `migrate` | lead | watch | – | O | none needed |
| 15 | a write while Collide is closed | anyone | startup `list` against the cache [inferred] | – | X (no catch-up rule: G7) | chip |

**Stress cases:**
- **An agent runs `./plan resolve` or `./plan set status=done` in a phase.** The plan CLI's own write gate holds:
  illegal transitions and new diagnostics are refused for any writer [read pc, the write gate]. A Collide gate
  cannot stop it, for three reasons. The write lands in the worktree. It reaches the line through `integrate`,
  which fires no task point [read plan 0.4, Named points]. And `protectedPaths: { at: phase-end }` can only fail
  the phase for whole files [read plan 0.9], which would also block legitimate RESULTS.md writes. G1 fixes this at
  `integrate`.
- **Ids reallocated at Share.** `plan new` allocates new ids, and links are rewritten [read pn 7.8]. Today
  `after_move` carries `tasks[]` but no old-to-new pairs, and `latestRun` is keyed by the old key (G9).
- **Bulk.** The plan CLI has no bulk status op. 21 commits carried more than five non-create transitions, from several
  `plan set` calls committed together [read log; inferred]. Only `renumber` and `generate` touch many files in one
  call.

```yaml
# orchard pack.yaml: derived points (proposal, example only; binding syntax from pn 7.8)
config:
  roles: { decision: [adr], spike: [spike], question: [question], obligation: [obligation] }  # read by collide/plan
points:
  task.before_review:                     # orchard/task.before_review
    on: task.before_status
    match: [review]
    when: 'task.kind == "task"'
  task.after_review:
    on: task.after_status
    match: [review]
    when: 'task.kind == "task"'
  task.after_rehome:                      # `plan move --epic`, declared as op `rehome`, never `move` (G9)
    on: task.after_op
    match: [rehome]
  adr.after_supersede:                    # fires for OLD only until G3; NEW arrives in change.effects
    on: task.after_op
    match: [supersede]
  item.after_renumber:                    # needs change.renamed (G9)
    on: task.after_op
    match: [renumber]
at:
  - point: orchard/task.before_review
    gate: gates/build-green.ts            # the run's build and test verdict in the workspace
    timeout: 300s                         # today the builder writes in_review itself: fires only with G1
  - point: orchard/task.after_rehome
    run: hooks/regen-tables.ts            # ./plan generate; reports the files for record
  - point: collide/plan/decision.after_decide
    run: hooks/adr-index.ts               # no evidence gate needed: plan set refuses it itself
observers:
  registry: { script: observers/registry.ts, events: ["task.*", "run.*"] }
```

**Pack findings.** `pn` 2.6 maps "(claim) spike owners | plan set". `plan set` refuses that (it writes scalar
fields only) [read pc, the set command] (P1). The `adr-evidence` gate in `pn` 7.8 duplicates
`apply`'s own `refused`.

## beads

**What the backend does:**
- **The events journal.** It is opt-in and must be enabled on every writer. It is a gapless `seq`, and each
  record carries `op` (`create|update|close|delete|dep_add|dep_remove|comment`), `actor` and a full post-mutation
  snapshot. It is read with `bd events tail --since N --follow` or over SSE [read R reference/events-journal.md,
  Reading; R cli-reference/events.md, Record format].
- **What the journal misses.** Sync, `bd sql` and migrations are not journaled, and a synced clone must
  re-baseline [read R reference/events-journal.md, Journaled operations].
- **beads' own hook scripts.** `.beads/hooks/on_create|on_update|on_close` run after a CLI write. They are
  fire-and-forget, get the issue JSON on stdin and get no actor. These are from source, not the docs [unverified].
- **Actor resolution.** The order is `--actor`, `BEADS_ACTOR`, `BD_ACTOR`, git user, `$USER` [read R
  reference/configuration.md, actor resolution].
- **Compare-and-set.** `bd update --if-status/--if-assignee` exits 13 on a mismatch [read R
  cli-reference/update.md, Guards].

| # | Touchpoint | Writers | How Collide learns | Via Collide | Outside | Point or gap |
|---|---|---|---|---|---|---|
| 1 | `create`, `q`, `todo add`, incl. `--deps discovered-from:X` | agents filing found work; humans; Collide `create` | journal `create` with actor | P | **O-run** | `question.after_raise`, `beads/issue.after_discovered` |
| 2 | bulk create: `create -f`, `--graph`, `mol pour`, `cook --persist`, `mol wisp`, `swarm create` | agents, humans, formulas | N `create` records | P~ (G4) | **O-run** | — |
| 3 | claim: `update --claim`, `ready --claim`, `assign`, `close --claim-next` | agents (beads' loop); Collide `on_start` | journal `update` | P~ (status and assignee: G2) | **O-run** | `spike.before_claim` |
| 4 | `unclaim`; `reclaim` (bulk, stale leases) | agents; a supervisor | journal `update` | P | O (bulk: G4) | `spike.after_unclaim` |
| 5 | field edits: `update` (title, priority, labels, metadata), `tag`, `note`, `label propagate`, `set-state` | agents, humans | journal `update` with snapshot | P~ (G2) | **O-run** | — |
| 6 | close: `close [ids…] -r`, `done`, `todo done`, `human respond\|dismiss`, `gate resolve` | agents, humans; Collide accept | journal `close`; `auto_closed_molecule` in `--json` [read R reference/json-schema.md, Close] | P~ (multi-id: G4; molecule root auto-close: G3) | **O-run** | `task.before_status: done`, `question.after_resolve` |
| 7 | `reopen` | humans | journal `update` | P | O | `decision.after_return` |
| 8 | `defer [--until]`, `undefer`, `update --defer`; the wake on the next ready read | humans, agents; the wake: whoever reads | journal `update` [read R cli-reference/defer.md] | P~ (within-category: G9) | O (Collide's own read may cause the wake [inferred]) | `decision.after_defer` |
| 9 | `dep add\|remove`, `link`, `relate`, `update --parent` | agents, humans | journal `dep_add`, `dep_remove` | P (link in `args`) | **O-run** | `decision.after_recommend` |
| 10 | `duplicate --of`, `supersede --with` (close + reference) | humans, agents | journal `close` | P~ (status and link; target item: G3) | O | `decision.after_supersede` |
| 11 | comments | all; Collide `comment` | journal `comment` | P | O | — |
| 12 | `bd batch` (one transaction, one Dolt commit) [read R cli-reference/batch.md] | scripts, agents | per-op records [unverified] | P~ (G4) | **O-run** | — |
| 13 | hard deletes: `delete [--cascade]`, `prune`, `purge`, `mol burn\|squash`, `wisp gc`, HTTP sweep | humans, maintenance | `delete` with a null snapshot, often no actor, or an id-set diff | P~ (G9) | O | — |
| 14 | `rename`, `rename-prefix` | humans | no rename op: replayed as delete, create and deps [read R cli-reference/events.md] | P~ (G9) | X (G7) | — |
| 15 | sync: `dolt pull`, `sync`, `import`, `github\|jira\|… sync` | humans, cron | not journaled: re-baseline | – | O (actor lost) | chip |
| 16 | `bd sql`, `restore --apply`, migrations | humans | not journaled | – | O | chip |
| 17 | `gate check` (closes gates from CI and PR state); `bd serve` writes | cron, tools | journal | – | O | — |
| 18 | a write while Collide is closed | all | journal `--since seq` if every writer has it on; otherwise a list diff (deletes invisible) | – | X (G7) | — |

**Stress cases:**
- **An agent runs `bd close` in a phase.** The write is global and immediate. Three things could respond:
  - beads' own hooks are after-only.
  - A Collide gate cannot run first.
  - A pack's Claude `PreToolUse` hook (exit 2 [read plan 0.3, the stream protocol]) can deny it. That is
    Claude-only, can be bypassed through scripts, and would also have to match beads' MCP tools [unverified].
  - With G1, a role `env:` template sets `BEADS_ACTOR` from `run.slug` and `phase.label` [read plan 0.5, Env]. The journal then attributes the close to the
    run, so `after_*` points fire once, and the `before_status` gates run as a phase-end check. A refusal fails the
    phase, the write stands, and the failure rules decide (a `fix` phase can reopen).
- **Status and link at once.** `bd duplicate X --of Y` closes X and records Y. Only X's points fire. A gate that
  needs Y's state needs G5.
- **Ids at Share.** `bd create --id` sets explicit ids [read R cli-reference/create.md], so the provider can keep
  the markdown ids if their prefix is valid [inferred].

```yaml
# example beads pack.yaml (proposal, example only)
config:
  roles: { decision: [decision], spike: [spike], session: [epic] }   # kinds from pn 7.8
roles:                                    # role env templates (plan 0.5, Env: run.slug and phase.label allowed)
  builder:
    env: { BEADS_ACTOR: "collide-{{ run.slug }}-{{ phase.label }}", BD_EVENTS_JOURNAL: "1" }
points:
  issue.after_discovered:                 # beads/issue.after_discovered: fires only with G1 (via: run)
    on: task.after_op
    match: [create]
    when: 'change.args.deps.exists(d, d.type == "discovered-from")'
  issue.before_claim:
    on: task.before_op
    match: [claim]
  issue.after_close:
    on: task.after_status
    match: [done, dropped]
  issue.before_defer:                     # open → deferred may stay inside one category: G9
    on: task.before_status
    match: [backlog]
    when: 'change.to.status == "deferred"'
  molecule.after_autoclose:               # the root closes as a side effect of its last step: G3
    on: task.after_status
    match: [done]
    when: 'change.effectOf != null'
at:
  - point: collide/plan/spike.before_claim
    gate: gates/claim-check.ts            # bd dolt pull, then bd show --json (pn 7.8)
    timeout: 60s
  - point: beads/issue.after_discovered
    run: hooks/triage-note.ts             # links the new issue to the run's task
  - point: collide/plan/session.after_share
    run: hooks/publish.ts                 # bd dolt push
observers:
  outside: { script: observers/outside.ts, events: ["task.changed", "task.removed"] }
```

## GitHub issues

**What the backend does:**
- **Issue events.** Polling `/issues/events` and the timeline gives the actor and details for most changes: a
  title rename (from/to), `closed` with `commit_id`, labels, assignees, milestones, types, sub-issues, `blocked_by`,
  `transferred`, `marked_as_duplicate` [read D rest/using-the-rest-api/issue-event-types]. GraphQL's
  `ClosedEvent.closer` names the Commit, PullRequest or ProjectV2, with `stateReason` [read D
  graphql/reference/issues].
- **Projects status.** A Projects status change carries `previousStatus`, `status` and `wasAutomated` [read D
  graphql/reference/projects].
- **What polling cannot see.** A body edit shows as a new `updated_at` only. A delete shows as a 404 or 410 with no
  event [read D issues/…/deleting-an-issue].
- **Writes.** No conditional writes. A PATCH replaces label and assignee sets wholesale [read D rest/issues/issues].
- **PR keywords.** They close issues only for PRs into the default branch [read D
  issues/…/linking-a-pull-request-to-an-issue]. "Item closed" then sets Status to Done [read D
  issues/planning-and-tracking-with-projects/…/using-the-built-in-automations].

| # | Touchpoint | Writers | How Collide learns | Via Collide | Outside | Point or gap |
|---|---|---|---|---|---|---|
| 1 | create (web, `gh issue create`, API, forms, bots) | humans, bots, agents | `issues.opened` [read D webhooks/webhook-events-and-payloads]; events poll | – (v0 has no GitHub `create` [read plan 0.10, Writes are operations]) | O | — |
| 2 | close and reopen, with `state_reason` (`completed`, `not_planned`, `duplicate`, `reopened`) | humans; agents' `gh` (the user's login); Collide `setStatus` | `closed`/`reopened` with actor | P | **O-run** (the agent cannot be told from the user: `gh` acts as the user's login [inferred]) | `github/issue.before_close` |
| 3 | closed by a merged PR's keyword | GitHub at merge | `closed` with `commit_id`; `closer` | P~ (when Collide's `integrate` merged it: G6, and the chip is false) | O | `github/issue.after_pr_close` |
| 4 | Projects Status: drag, `gh project item-edit`, built-in automations | humans, Projects workflows | status-changed event with `wasAutomated` | P (if `board.yaml` maps it) | O (the automation after Collide's own close shows a chip: G7) | — |
| 5 | labels, assignees, milestone, type | humans, bots, agents | `labeled`, `assigned`, `milestoned`, `issue_type_*` | – (no v0 op) | O | a `when:` needs G2 |
| 6 | title, body | humans | `renamed`; body: `updated_at` only | – | O | — |
| 7 | comments | all; Collide `comment` | `issue_comment` | P | O | — |
| 8 | sub-issues, dependencies, relates-to | humans | `sub_issue_*`, `blocked_by_*` | – | O | gates need G5 |
| 9 | `gh issue close --duplicate-of`, mark as duplicate | humans, agents | `closed` + `marked_as_duplicate` | P~ (status and link: G3) | O | `github/issue.after_duplicate` |
| 10 | transfer | humans | 301; `transferred` | – | X (`task.removed` has no cause: G7) | — |
| 11 | delete | admins | 404/410, no event when polling | – | X (G7) | — |
| 12 | Actions and apps with their own tokens | bots | events with a bot actor | – | O | — |
| 13 | a change while Collide is closed | all | events since the last id (retention [unverified]); `since=` list; deletes invisible | – | X (G7) | — |

**Stress cases:**
- **A GitHub issue is closed by a PR that Collide's `github/pull-request` integrate merged.** GitHub closes the
  issue during `integrate`. The accept's `setStatus: done` and its `before_status` gate run afterwards [read plan
  0.4, Lifecycle order]. Then `apply` finds the issue already closed, and watch reports an outside change on
  Collide's own work. Fixes:
  - G6: fire the accept ops' `before_*` points before `integrate`.
  - G1: record the close as `via: integrate`, matching `closer` to the run's PR.
  - The pack option: open PRs without closing keywords (P4).
- **A web bulk edit** closes or labels many issues [unverified]. It arrives as N events and needs no point (O).
- **Rate.** Polling events uses REST at 5000/h, and a 304 does not count [read D
  rest/using-the-rest-api/rate-limits-for-the-rest-api].

```yaml
# example GitHub pack.yaml (proposal, example only)
points:
  issue.before_close:                     # github/issue.before_close (plan 0.4 names it)
    on: task.before_status
    match: [done, dropped]
  issue.after_duplicate:
    on: task.after_status
    match: [dropped]
    when: 'change.to.status == "duplicate"'          # link in change.diff.links: G2
  issue.after_pr_close:
    on: task.after_status
    match: [done]
    when: 'change.via == "integrate"'                # G1
at:
  - point: github/issue.before_close
    gate: gates/subissues-done.ts         # refuses while a sub-issue is open: needs G5
  - point: github/issue.after_pr_close
    run: hooks/run-summary-comment.ts     # comment body from a file, never argv (pn 7.6)
observers:
  outside: { script: observers/outside.ts, events: ["task.changed"] }   # actor and closer from G7
```

## Obsidian

**What the backend does:**
- **The plugin API.** It has after-events only: vault `create`, `modify`, `delete`, `rename(file, oldPath)`, and
  metadata `changed` (not on rename). There is no veto [read OD Vault, events].
- **Link rewriting.** `FileManager.renameFile` rewrites links when the setting is on. A rename outside the app breaks
  them [read OH links].
- **The CLI** (1.12+). It needs the running app and has no event stream [read OH cli]. Its `property:set` is
  reported to do nothing on freshly written files [read the forum thread
  "cli-property-set-silently-succeeds-but-makes-no-change-on-externally-written-files"].
- **Identity is the path.** Advanced URI uses a frontmatter `uid` [read the Advanced URI docs, Concepts/File].

| # | Touchpoint | Writers | How Collide learns | Via Collide | Outside | Point or gap |
|---|---|---|---|---|---|---|
| 1 | a Properties edit in the app | the person | fs modify | – | O | chip |
| 2 | Bases kanban drag: writes the grouped property or moves the file [read OH bases/views/kanban] | the person | fs | – | O | — |
| 3 | Tasks plugin: status symbol, done date; recurrence inserts a line [read the Tasks docs, Getting Started] | the person | fs: line diff | – | O (line items have no id: P3) | — |
| 4 | Kanban plugin drag: one file, whole-file rewrite [read the Kanban plugin's list parser] | the person | fs | – | O (one write, many items: G4) | — |
| 5 | rename or move in the app; links in other notes rewritten | the person | fs: delete + create | – | X (G7; path identity: P3) | — |
| 6 | a rename by Collide, git or an agent, outside the app | Collide, agents | `apply`, fs | P~ (links break unless the provider rewrites them: P3) | O | — |
| 7 | delete (trash per setting) | the person, Collide | fs | P~ (G9) | O | — |
| 8 | Templater rewrites a new file after its create event [read the Templater docs, hooks module] | the plugin | fs modify just after Collide's create | P~ (Collide's own new item shows the outside chip: P3) | O | — |
| 9 | an Obsidian Sync or Obsidian Git merge: a conflict file or markers [read OH sync/troubleshoot] | the plugins | fs | – | X (G8) | — |
| 10 | CLI `property:set`, `task … toggle`, `create`, `move` from an agent | agents (the app's vault, not the worktree [inferred]) | fs | – | **O-run** (G1, global) | — |
| 11 | a Collide provider op (frontmatter edit) | Collide | `apply` ok | P (an open editor can save over it [read the forum thread "obsidian-doesnt-reload-the-current-file…"]: P3) | – | `collide/plan` points |
| 12 | a change while Collide is closed | the person | startup list against the cache | – | O | — |

**Stress cases:**
- **Conflict on write.** Collide's version check runs before its write, but the app's open editor can later save
  stale content over it. That is a lost update no `conflict` result reports [inferred].
- **Renames that rewrite links.** One rename modifies N other notes. Each arrives as a `task.changed` with only link
  diffs, which an observer should be able to filter out (G7 `diff`).

```yaml
# example Obsidian pack.yaml (proposal, example only)
config: { idField: uid, statusField: status }      # P3: frontmatter id required, never the path
points:
  note.before_done:                       # fires for Collide's writes only; in-app edits never fire
    on: task.before_status
    match: [done]
at:
  - point: obsidian/note.before_done
    gate: gates/checklist.ts              # useful only for writes made in Collide
observers:
  vault: { script: observers/vault.ts, events: ["task.changed", "task.renamed", "task.removed"] }
```

## Markdown provider (built-in)

It is the reference backend [read plan 0.10, Planning sources]: random ids, two roots, Share copies to the
shared path, and the shared root is data.

| # | Touchpoint | Writers | How Collide learns | Via Collide | Outside | Point or gap |
|---|---|---|---|---|---|---|
| 1 | `create`, `setStatus`, `link`, `claim`, `setChecklist` from the UI or a workflow | Collide | `apply` | P (the whole `pn` 7.5 set) | – | all |
| 2 | Share, private to shared | Collide | `apply` (move) | P (random ids: nothing is reallocated) | – | `session.*_share` |
| 3 | a layout change, then Move (`git mv` with the config change) | Collide | `apply` | P~ (move points or not? G9) | – | — |
| 4 | two offline claims | Collide on two machines | merge on fetch | P on each | O ("claimed by two" [read plan, second-pack stage ("Second pack, composition, observers, grouped chains") tests]) | — |
| 5 | an agent edits a shared-root item in its worktree | agent | workspace watch; `integrate` | – | **O-run** (G1, ref-scoped) | — |
| 6 | an agent edits a private-root item (pack data directory, outside the repo) | agent with Bash | fs | – | **O-run** (G1, global) | — |
| 7 | hand edit | the person | watch | – | O | chip |
| 8 | pull, merge | teammates | watch | – | O | chip |
| 9 | conflict markers | git | a counted parse error | – | X (G8: `pn` 5.7 promises a Queue item with no mechanism) | — |
| 10 | a file deleted | the person, Collide | `task.removed` | P~ (G9) | O | — |
| 11 | a change while Collide is closed | anyone | startup; `git log` has the authors | – | O | — |

```yaml
# a team's pack.yaml on the markdown provider (proposal, example only)
at:
  - point: collide/plan/decision.before_decide
    gate: gates/evidence-concluded.ts     # needs each evidence spike's status in tasks[].links: G5
  - point: collide/plan/session.before_share
    gate: gates/secret-scan.ts            # needs change.files at before_move from the preview: G3
    timeout: 120s
  - point: collide/plan/checklist.after_mark
    run: hooks/notify-security.ts
    when: 'change.diff.checklist.exists(i, i.group == "security")'    # G2
```

## Coverage

**Counts by mapping class** (rows are the touchpoints above):

| Backend | Rows | Via: P | P~ | X | Outside: O | O-run | X |
|---|---|---|---|---|---|---|---|
| plan CLI | 15 | 3 | 6 | 1 | 10 | 2 | 3 |
| beads | 18 | 5 | 9 | 0 | 9 | 7 | 2 |
| GitHub | 13 | 3 | 2 | 0 | 9 | 1 | 3 |
| Obsidian | 12 | 1 | 3 | 0 | 8 | 1 | 2 |
| markdown | 11 | 3 | 2 | 0 | 5 | 2 | 1 |
| **Total** | **69** | **15** | **22** | **1** | **41** | **13** | **11** |

**P~ by cause:**

| Cause | Rows |
|---|---|
| G9 create, delete, rename, within-category, op names | 9 |
| G2 diff | 5 |
| G3 multi-item | 4 |
| G4 bulk | 3 |
| G6 accept order | 2 |
| P3 Obsidian (pack) | 2 |

Some rows have two causes.

**Activity share (plan CLI, measured).** The method: `git log --no-merges --since=2026-10-15` over the Orchard repo.
Lines added as `status: <x>` were counted in commits touching plan item files. A commit counts as "mixed" when it
also touched files outside the task directory, the heuristic for a builder agent's worktree commit [read log].

| | Status lines | specified | doing | in_review | done | dropped | ready |
|---|---|---|---|---|---|---|---|
| all | 2270 (1293 commits) | 690 | 340 | 493 | 518 | 130 | 99 |
| task-directory-only commits (lead) | 1914 (84 %) | 566 | 232 | 418 | 475 | 126 | 97 |
| mixed commits (agents) | 356 (**16 %**) | 124 | 108 | 75 | 43 | 4 | 2 |

- Planning kinds are rare: about 20 question resolutions, 5 spike conclusions and 2 ADR accepts (pathspec
  imperfect) [read log].
- Per commit: 1027 had one status line, 226 had 2–5, 36 had 6–20, and 4 had more than 20 [read log].
- The 16 % figure is a floor for Q11's uncovered share once the lead works through Collide. Today, with no Collide,
  the share is 100 %.

## Gaps

### G1. Writes made by a run's agent through the backend (core)

**What fails.** Q11 treats an agent's `bd close`, `gh issue close`, `./plan set` or edit of a planning file inside
a Collide phase like a teammate's push. No gate sees it. That includes the review gate in the plan's own
tests ("a lint/test gate on `task.before_status: review`" [read plan, second-pack stage tests]) whenever the
builder writes `in_review` itself, as Orchard's builders do. The `pn` 7.1 rationale (avoid duplicate reactions on
every teammate's machine) does not apply: only this machine ran the agent.

| Option | For | Against |
|---|---|---|
| (a) Q11 as is | Nothing to build | 13 of 69 touchpoints are left O-run. beads' main loop and Orchard's builders bypass every gate. |
| (b) **Ref-scoped sources: fire from the run's task diff at `integrate`.** Before `integrate`, core diffs the workspace's items against the line (the provider already reads every checkout [read plan 0.10, Ref-scoped sources]). It fires `task.before_status` for each category moved, plus `before_op: create`/`delete` for added and removed items, with `via: run`. A refusal stops `integrate`, and the accept goes to the Queue. The after points fire after `integrate`. | Exact, deterministic, prevents the write reaching the line, and needs no attribution | The op is unknown (`change.op: null`), so only status, create and delete points fire. Changes no agent made, such as a sync merge, also show in the diff [inferred]. |
| (c) **Global sources: attribute, then fire after points and check gates at phase end.** A phase `env:` marks the actor (`BEADS_ACTOR`). Otherwise the attribution comes from the stream's Bash `tool_use` input and the time window. `after_*` points fire once, here. `before_*` gates run as a phase-end check: a refusal fails the phase, the write stands, and the failure rules apply. | Same machine, once; uses existing failure rules | Cannot prevent the write. Attribution is heuristic for GitHub (same login) and Obsidian. |
| (d) A pack `PreToolUse` hook denies `bd close`, `gh issue close` or `./plan set` (exit 2) | Prevents it | Claude-only, regex on Bash, bypassed by scripts and MCP tools; same-user reality [read plan 0.1, Same-user reality] |

**Recommendation.** Before the skeleton:
- reserve `change.via` and `actor.kind: agent` with `run` and `phase`;
- state rule (b) in 0.4 and the lifecycle order.

Build (b) in the workflow engine stage. Defer (c) to the second-pack stage, with beads as its user. (d) stays a pack
choice. **core.**

### G2. `change` carries status only (core)

**What fails.** Claim (assignee), labels, `set-state`, `reviewed_by`, epic re-home (collections), checklist items,
a Projects field: a `when:` cannot see them. Observers get a snapshot with no diff.

| Option | For | Against |
|---|---|---|
| (a) **`change.diff`**: core computes it from the cache before and the `apply` result after. Sets are `{added, removed}`, scalars `{from, to}`. At `before_*` it is predicted (G3 `preview`, or else `args`). | One shape for points and events; providers do nothing | Predicted at `before_*` unless the provider previews |
| (b) Hooks read `tasks[]` and compare with `latestRun` | No new field | No "before" exists outside a run; every hook repeats it |

**Recommendation: (a)**, the same object in `task.changed` events (G7). **core.**

### G3. Ops that change several items, or status and links together (core)

**What fails.** `plan supersede` (two items), `bd duplicate` and `gh issue close --duplicate-of` (status and link),
a molecule root auto-closing [read R workflows/molecules.md, Completion and auto-close], `plan renumber` (many
files). Op effects are single-item and category-only [read plan 0.10, Protocol]. `change.files` at `before_*` is
unspecified, yet the `session.before_share` secret scan needs it [read pn 7.5].

| Option | For | Against |
|---|---|---|
| (a) **`apply` `ok` returns `items[]`** (key, version, from, to) **and `files`.** The primary item's `after_*` carries `change.effects[]`, and each other item gets `after_status` with `change.effectOf`. | Exact, after the fact | `before_*` still sees only the primary |
| (b) **Optional `preview` capability**: `apply`'s arguments, nothing written, returning `items[]` and `files`. `before_*` then carries predicted `effects`, `diff` and `files`. | Gates see the whole effect; Share and secret scans get files | Providers must dry-run. The plan CLI has `--dry-run` everywhere [read pc README, Edit], markdown is trivial, beads has it on some commands only [read R cli-reference/delete.md, batch.md], GitHub computes it [inferred]. |
| (c) Declare effects statically in `kinds` | No RPC | Cannot express "whichever item `--with` names" |

**Recommendation: (a) in the protocol now, (b) as an optional capability.** Without `preview`, `change.files` at
`before_*` is the declared target only. **core.**

### G4. Bulk semantics (core)

**What fails.** `bd close a b c`, `bd batch`, `mol pour`, `reclaim`, a Kanban whole-file rewrite, a multi-select in
Collide's own UI. Whether status and op points fire per item or per call is unstated. So is whether item 3's
refusal stops items 4–10.

**Recommendation.**
- Per item, in order, with `change.batch: {id, index, size}`. A refusal skips that item, and the batch reports
  each item's result.
- Move points stay per Share, all or nothing [read plan 0.4, Named points].
- A provider's bulk call is still N `apply` calls, unless a later `applyBatch` is declared.

**core** (text only).

### G5. Linked items' state is not in the payload (core)

**What fails.** "Accept only when every evidence spike is concluded" or "close only when sub-issues are closed".
The hook cannot read the corpus: no token reaches it [read plan 0.2, the socket token], and `tasks[]` carries
fields and paths only [read plan 0.5, Input]. Cross-source links (a markdown decision citing `github:acme/edge#212`)
cannot be resolved from the file.

| Option | For | Against |
|---|---|---|
| (a) **One hop of links in `tasks[].links[]`**, both directions: `{type, dir, key, kind, status, category, stale}`, resolved by core from the corpus cache across sources | Small; covers every rule found | No second hop |
| (b) A read-only corpus snapshot file named in the input | Complete | Large; a stale copy per call |
| (c) A read API for hooks | Live | Puts a token in hooks' environment, against 0.2 |

**Recommendation: (a).** **core** (additive, but it belongs to the frozen payload).

### G6. Accept runs `done` gates after the merge (core)

**What fails.** Accept is `sync`, `run.before_integrate`, `integrate`, `run.after_integrate`, then the source's
operations [read plan 0.4, Lifecycle order]. A `task.before_status: done` refusal leaves code merged and the task
open, which contradicts "a refusal at a `before_*` point stops the run there". GitHub PR keywords close the issue
during `integrate`.

| Option | For | Against |
|---|---|---|
| (a) **Fire the accept ops' `before_*` points (and G1(b)'s run-diff points) before `run.before_integrate`.** Apply the writes and fire the `after_*` points after `integrate`. | Refusal stops the merge; gates are checks, so early is safe | A write can still conflict after the merge (a Queue item, as today) |
| (b) Packs bind the same gate to `run.before_integrate` too | No core change | Every pack must remember; UI `done` writes and accept diverge |

**Recommendation: (a).** **core.**

### G7. Outside changes have no defined shape (core)

**What fails:**
- `task.changed`, `task.renamed` and `task.removed` are named, but their shapes are not [read plan 0.10, Protocol].
- Observers receive events only "from runs on lines that apply its pack" [read plan 0.4, Observers], and a pull is
  not a run.
- The chip shows on any outside change [read plan 0.4, Named points], but the second-pack stage test shows it "if its
  category moved".
- There is no catch-up rule for changes made while Collide was closed.
- Transfers and deletes look alike.
- beads renames arrive as a delete plus a create.

**Recommendation** (one shape, no new points):
- `task.changed {key, version, from, to, diff, cause, actor?, automated?, head?}`, where `cause` is
  `head | worktree | remote | journal | startup`. The `actor` is the source's own when it is known (GitHub events, the
  beads journal), and `automated` is GitHub's `wasAutomated`.
- `task.renamed {from, to, checkout}`, with history, runs and `latestRun` following the key.
- `task.removed {key, cause: deleted | transferred | unknown}`.
- Observers receive source events for every line that applies their pack.
- Startup reconciliation diffs `list` against the persisted cache and emits one coalesced change per item with
  `cause: startup`.
- The chip is a field (`outside: {at, cause, categoryMoved}`) that views, `when:` and a `queue: true` lane can read
  [read plan 0.11, Planning]. It shows for any change, and is emphasized when the category moved.

**core.**

### G8. Unparseable items have no notification (core)

**What fails.** Conflict markers, a generated-region conflict or a Sync conflict file leave an item unparseable.
The plan counts parse errors on the view [read plan, plan-adapter stage tests]. `pn` 5.7 and 7.9 say `collide/plan` "adds a
Queue item", but no pack mechanism can add one.

**Recommendation.** Add a `task.invalid {path, checkout, reason}` notification. Core turns it into a Queue item
and an event. **core** (small).

### G9. Undefined edge semantics (core, text only)

| Case | Proposed rule |
|---|---|
| A status write inside one category (Projects "In progress" → "Blocked", both `active` [inferred]; beads `open` → `deferred` where both map to `backlog`) | Status points fire on every status-name change. `match` tests the target category. `change.categoryMoved` says which. An op fires status points when it changes the status name, not only the category. |
| `before_op: create` | `task` holds the arguments, with `key: null`; `change.from: null` |
| delete | `change.to: null`; `tasks[]` holds the last known item |
| `conflict` from `apply` | `after_*` never fires. A retry re-runs the `before_*` points. An `unverified` write released from the Queue later re-runs them if the version moved. |
| ids reallocated at Share | `after_move` carries `change.renamed: [{from, to}]`, and runs follow the new key |
| op names | `move` is reserved for Share (move points). The plan CLI's `move --epic` is declared as `rehome`. The markdown layout Move is a path change in one source, not a move point. |

**core.**

### G10. Advisory checks on outside changes (core, can wait)

Gates are checks, so running a pack's `before_*` gates as **advisory** checks on observed changes is harmless on
every machine. That would turn "gates not run" into "gate X would refuse". It is the only gate value Obsidian and
GitHub's web users can get. It needs G1's `via` and G7's event shape, not a catalogue change. Deferred, with the
existing Deferred entry. **core**, not needed before the skeleton.

### Pack gaps

| # | Gap | Recommendation |
|---|---|---|
| P1 | The plan CLI has no CLI writer for `owners`, `evidence`, `depends_on`, `blocks` after creation [read pc, the set command] | The provider omits `claim` and `link` for the plan CLI, or edits frontmatter itself with the content-hash version check. Correct `pn` 2.6. |
| P2 | `pn` 7.7 and 7.8's `adr-evidence` gate duplicates the plan CLI's own write gate | Rely on `apply` → `refused`. Correct `pn` 7.7. |
| P3 | Obsidian: path identity, Templater rewrites after create, an open editor overwrites, CLI writes target the app's vault, renames outside the app break links | Require a frontmatter id (`uid`). Settle a create before reading the version. Write through the CLI's `move` when the app runs, otherwise rewrite links. |
| P4 | GitHub: closing keywords in PRs Collide opens; Projects automation as a second writer | Open PRs without keywords and close via the accept op (with G6). Suppress the chip when `automated` follows Collide's own close. |
| P5 | beads: the journal is off by default and per writer; sync is not journaled | The pack sets `events-journal: true` and `BEADS_ACTOR` in phase `env:`, and re-baselines after `bd dolt pull`. Its version check uses `--if-status`. |
| P6 | Cross-source reactions ("GitHub issue closed → close the beads issue") | Observers cannot act in v0 [read plan 0.4, Observers]. Use beads' own tracker sync, or wait for acting observers (Deferred). |

## Draft plan text (proposal, not applied)

**0.4 "Named points", the task-point bullets.** Replace "points fire only around writes Collide makes…" with:

> - points fire around writes Collide makes and writes its runs make. A run's write reaches its points one of two
>   ways.
>   - A ref-scoped source's change in the workspace fires from the run's task diff when the run is accepted, before
>     `integrate`, with `change.via: run` and `change.op: null` (status, `create` and `delete` points only). A
>     refusal stops `integrate`.
>   - A global source's change that core attributes to a phase (the source's actor, or the phase's tool calls and
>     time) fires its `after_*` points once, on this machine, and runs the `before_*` gates as a phase-end check.
>     A refusal fails the phase and the write stands. (Deferred to the second-pack stage; `via` is reserved now.)
> - a change Collide only reads from elsewhere (a pull, a teammate, a hand edit, a bot) fires no point. It reaches
>   observers as `task.changed`, and the item carries `outside` (owner, 2026-11-13; S11).
> - status points fire on every change of status name. `match` tests the target category, and
>   `change.categoryMoved` says whether it moved. An op fires them when it changes the status name.
> - a call on several items fires per item, in order, with `change.batch`. A refusal skips that item. Move points
>   stay once per Share.
> - `apply` → `conflict` fires no `after_*` point. A retry re-runs the `before_*` points.

**0.4 "Lifecycle order"**, the accept row:

> | later: accept | `sync` check; the `before_*` points of the source's operations and of the run's task diff;
> `run.before_integrate`, `integrate`, `run.after_integrate`; the source's operations (writes, then `after_*`
> points); `record` |

**0.4 "Observers"**: add:

> - An observer also receives source events (`task.changed`, `task.renamed`, `task.removed`, `task.invalid`) for
>   every line that applies its pack, whoever made the change.

**0.5 "Input"**: replace the `change` bullet with:

> - Task points add `change: { op, args, from, to, diff, categoryMoved, via, actor, batch, effects, effectOf,
>   renamed, checkout, files }`.
>   - `via` is `ui | workflow | share | integrate | run`.
>   - `actor` is `{ kind: person | workflow | agent, run?, phase? }`.
>   - `diff` maps each changed field to `{from, to}`, or `{added, removed}` for sets. It is predicted at `before_*`
>     (from `preview` when the provider has it) and actual at `after_*`.
>   - `from` is null at create and `to` is null at delete.
>   - Each `tasks[]` entry carries its links one hop out, in both directions, with each linked item's key, kind,
>     status, category and `stale`, across sources.
>   - Outside a run, `latestRun` as before.

**0.10 "Protocol"**:

> - `apply` → `ok` also returns `items[]`: every item the op changed, with its version, from and to.
> - Optional capability `preview`: `apply`'s arguments; writes nothing; returns `items[]` and `files`.
> - `watch` notification shapes:
>   - `task.changed {key, version, from, to, diff, cause, actor?, automated?}`;
>   - `task.renamed {from, to, checkout}`;
>   - `task.removed {key, cause}`;
>   - `task.invalid {path, checkout, reason}`, which core turns into a Queue item.
> - On start, core reconciles each source against its persisted cache and emits one change per item, with
>   `cause: startup`.
> - Op name `move` is reserved for Share.

**Skeleton stage, "Planning contracts"**: add:

> `change.diff`, `via`, `batch`, `effects`; `apply` `items[]`; `preview`; the `watch` notification shapes and
> `task.invalid`; the run-diff points at accept.

**Second-pack stage ("Second pack, composition, observers, grouped chains"), tests**: replace "shows the outside-change chip if its category moved" with:

> shows the outside-change chip, emphasized when the category moved; a builder's `in_review` in its worktree fires
> `task.before_status: review` at accept, and a refusal stops `integrate`.

**Deferred "Points for changes made outside Collide"**: narrow it to foreign writers:

> - **Points for foreign changes** (a teammate, a pull, a bot).
> - Advisory gate checks on observed changes come first. Run-made writes are covered in 0.4 (S11).

## Unverified

| Item | Would change |
|---|---|
| The heuristic of mixed commits as agent writes (16 %); task-directory pathspec counts | The plan CLI Q11 share, not the G1 recommendation |
| beads `.beads/hooks/on_*` behaviour (source only; current on main?), `bd batch` per-op journaling, `purge`/`prune` journaling, beads' MCP tool names, whether `bd list` (not only `bd ready`) wakes deferred issues | P5. Whether beads hooks could feed `watch`. The guard in G1(d). |
| GitHub issue-event retention; web bulk edit; the `issues.status_changed` preview | The catch-up quality in G7 |
| Whether the Obsidian CLI can target a vault folder other than the open one; whether Templater fires on externally created files | P3 |
| Whether the corpus cache persists across restarts (the plan does not say) | G7 startup reconciliation must add persistence if it does not |
| beads `json-schema` notes that `update --json` concatenates objects in one version (third-party issue, seen as a search snippet) | Provider parsing only |
