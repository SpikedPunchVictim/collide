# How Collide manages its tasks and its documents

This guide is for a person or an AI agent working in a repo that uses Collide's planning layout. Collide's own repo is
the worked example, because Collide is built with Collide: its build tasks are markdown items under
`.collide/planning/`, and the plan that governs them is `IMPLEMENTATION_PLAN.md`.

The guide states the rules the plan states, and cites the plan by section or stage name (never by line). Where the
plan is silent, the guide says so. Read it once, then use it as a reference.

**What is built.** Nothing in the `collide task` CLI exists yet. It arrives with Stage 2's planning slice. Until then
you edit item files by hand, in the same format. Everything marked "planned" below is not built.

## 1. Overview

There are two kinds of written material.

| Kind | What it is | Where it lives | Who reads it |
|---|---|---|---|
| **Tasks** (items) | Units of work with a status and links. One markdown file each. | `.collide/planning/` (the shared root) | The `collide/plan` pack's markdown provider; agents and people |
| **Documents** | The plan, spike records, review ledgers, research notes | `IMPLEMENTATION_PLAN.md`, `.collide/planning/spikes/`, review items, `docs/research/` | People and agents, as text |

How they connect:

- The plan holds the design. Each stage's **Status** line names its epic and the plan revision its items came from.
- An **epic** folder holds the items for one stage. The items hold the work and its status.
- A **spike** answers one question with measurements. Its record is a document that is also an item of kind `spike`.
- A **review** is itself a task item. Its ledger records what happened to every finding.
- Research notes in `docs/research/` are background. They shape the plan, but nothing reads them as items.

The split follows a principle in the plan (0.1): the plan keeps the design, the items keep the status. A plan revision
does not leave a hundred items written against an old design, so **only the next stage is split into items**; later
stages stay outlines in the plan (0.10, "Collide's own build tasks").

## 2. Tasks

### 2.1 The shared root and its layout

The shared root is `.collide/planning/` by default, committed with the repo. (Planned, Stage 4c: the path is a repo
setting in `.collide/config.yaml`, and a private root in the pack's data directory holds uncommitted items.)

```
.collide/planning/
  epics/
    ep-002-va-skeleton-and-planning-slice/
      epic.md
      tasks/ts-0012-ua-<slug>.md
      proposals/pr-0003-km-<slug>.md      (planned, Stage 4c and 9d)
      questions/qu-0007-bx-<slug>.md      (planned, Stage 4c and 9d)
  spikes/
    sp-0005-fv-schema-expressions/
      record.md, scripts, out/, work/
```

The provider reads **only these paths** (0.10):

- `epic.md`;
- the `.md` files directly in `tasks/` (and in `proposals/` and `questions/`);
- each spike's `record.md`.

It never walks the rest of a spike folder, which can hold dependencies and gigabytes of run output, and its file watch
ignores that too. So a `README.md` at the shared root, or inside a spike folder, is not an item.

Two more rules:

- **The folder is the epic.** An item has no `part-of` field. The provider reports epic membership from the path.
  Moving a file into another epic's folder moves the item, and its links do not change. An item with no epic yet goes
  in `epics/ep-000-<suffix>-general/`.
- **Spikes sit at the top level**, because one spike often informs several epics. Its `links` name them.

In the first slice the provider reads `epics/` only. Spike records become items from Stage 4c (see 2.4).

### 2.2 Frontmatter

Every item starts with a YAML frontmatter block, then a markdown body.

| Field | Meaning |
|---|---|
| `id` | The item's identity (2.5). Matched against `^[a-z]{2}-[0-9]{3,}-[a-z]{2}$`. A string. |
| `alias` | Optional. A short label such as `S5` or `stage-2`. Shown, never linked to. |
| `kind` | `task`, `epic` or `spike`. |
| `status` | One of the kind's statuses (2.3). |
| `title` | A one-line title. |
| `owners` | The claim: a list of who has taken the item. Empty when unclaimed (2.7). |
| `links` | Typed links to other items. So far only `blocked-by` (2.6). |

Body sections for tasks: **Goal**, **Success criteria**, **Tests**, **Notes**.

Parsing rules that matter when you edit by hand (0.10):

- The block is parsed by the `yaml` package in its default (core) mode. No `gray-matter`, no YAML 1.1 mode.
- Ids are always strings. A field that holds a hash is written **quoted**, because an unquoted short sha can parse as
  a number (`3e10`, `1234567`).
- Duplicate keys are errors. Every field's type is checked, with no coercion.
- The frontmatter block has a size cap (S5 suggests 64 KiB), not the file. A long body is the point of an item, but it
  has its own cap, set when Stage 2 builds the provider.
- Two files with one id, an unknown kind or a broken link are reported as `task.invalid`, not silently dropped.

A real example from this repo (shape only):

```yaml
---
id: ts-0010-yb
kind: task
status: ready
title: "Provider and observer protocols"
owners: []
links:
  - { type: blocked-by, to: ts-0009-cu }
---
```

### 2.3 Kinds and statuses

**Task.** A task's statuses are the five status categories themselves (0.10):

| Status | Plain reading |
|---|---|
| `ready` | Can be started, once its blockers are cleared (2.6). |
| `active` | Someone is working on it. |
| `review` | The work is done and waits for a check. |
| `done` | Checked and accepted. Unblocks dependents. |
| `dropped` | Will not be done. Also unblocks dependents. |

"Open" means not `done` and not `dropped`. (The category `backlog` exists in the plan for other sources, but a task's
statuses do not include it.)

**Spike.** A folder with a `record.md`. Statuses, as the records use them (0.10, "A `spike`"):

| Status | Category | Meaning |
|---|---|---|
| `planned` | `ready` | Not started. |
| `running` | `active` | Its scripts are running. |
| `results` | `review` | Results are in; conclusions are not yet written. |
| `concluded` | `done` | The record's conclusions are written. |
| `dropped` | `dropped` | Abandoned. |

All ten records in this repo are `concluded`. `planned` and `running` are for the spike workflow of Stage 9.

**Epic.** The plan names the kind `epic` and its folder, but 0.10 does not list epic statuses. On disk, epics use
`ready` and `active`, and an epic's own criteria say "every item of this epic is `done`". Treat epic statuses as the
same category words and check the plan before relying on more.

**Proposal and question** (planned, Stage 4c and 9d). A proposal holds its decisions; a question holds its own
decision once resolved. Only a person accepts, defers, rejects or resolves one. They are not in the first slice.

### 2.4 When the provider reads what

| Stage | What the provider reads |
|---|---|
| Stage 2 (planning slice) | `epics/` only: kinds `task` and `epic`. |
| Stage 4c | Adds the `spike` kind: records in `spikes/` become items. The private root and layout setting arrive. |

The slice's criterion is narrowed on purpose: a spike record's statuses and shape are not a task's, so the slice does
not read `spikes/` (0.10; Stage 2 criteria).

### 2.5 Ids

An id is `<kind prefix>-<number>-<suffix>`, for example `ts-0012-ua` (0.10).

- **Prefixes:** `ep` (epic, three digits), `ts` (task), `sp` (spike), `pr` (proposal), `qu` (question). The other kinds
  use four digits.
- **Names on disk** are `<id>-<slug>`: the file `ts-0012-ua-<slug>.md`, or the folder `ep-002-va-<slug>/`.
- **The number** is the next free one for that kind **across every ref Collide can see** (local branches and fetched
  remotes). It keeps creation order and is the short handle. It can repeat after two branches merge.
- **The suffix** is two letters, without `i`, `l` or `o`, so it never reads as part of the number. It makes the id
  unique, so a merge never renumbers a file or rewrites a link. It is drawn from a seeded generator, and the draw is
  repeated until the id is free across every ref.
- **The short form:** `collide task 12` resolves when exactly one item has that number. Otherwise the CLI lists the
  matches and asks for `12-ua`.
- **Collisions.** Two files with the same whole id are a `task.invalid`. `collide task reid <id>` gives the second one
  a new suffix, rewrites the links that name it, and commits nothing.
- **Aliases** such as `S5` are display labels. They are not unique and never targets of a link. Note that a spike's
  alias is not its folder number: `S8` is `sp-0010-bn`.

By hand, until `create` exists: take the highest number for that kind across `git branch -a`, add one, and pick a
two-letter suffix not in use. This follows the same rule; it is advice, not a plan rule.

### 2.6 Links and the ready rule

Items link with `blocked-by`, the one link type so far, and it blocks (0.10):

```yaml
links:
  - { type: blocked-by, to: ts-0009-cu }
```

The `to` value is the whole id, without the slug.

**A task is ready to start** when its status is `ready` and every `blocked-by` target is `done` or `dropped`.
`dropped` unblocks, because the work will never arrive. A `blocked-by` cycle is reported, and none of its items is
ready.

Until the CLI exists, you pick a ready item by reading frontmatter: find `status: ready` items and check each
target's status.

### 2.7 Owners and claiming

`owners` is the claim. Planned (Stage 2): the `claim` op is **refused when the file already shows an owner**, unless
the claimant is that owner, and `unclaim` clears it. The plan does not fix the format of an owner string; on disk
today it is a plain name in a list, or `[]`.

### 2.8 Writing a good item

Collide's builders are Sonnet agents (0.15). Each Stage 2+ item is **sized and written so that one Sonnet agent can
finish it in one session from the item file alone**. An item should:

- name the plan sections and contracts it implements, by section, never by line;
- name the crate or package area it touches;
- list the tests to write first;
- say when it is done.

If a stage is too big for a handful of such items, split it into lettered sub-stages (3a, 3b, ...) in the plan, with
one epic and items that say which sub-stage they belong to.

An annotated example (illustrative; not a file in the repo). The `Area:` and `Done when` lines follow what this
repo's items do; the plan fixes the four things above, not these labels.

```markdown
---
id: ts-0099-xx                  # kind prefix, 4-digit number, two letters
kind: task
status: ready                   # one of the five categories
title: "Short, specific title"
owners: []                      # unclaimed
links:
  - { type: blocked-by, to: ts-0009-cu }   # whole id of the item this waits for
---

## Goal

One paragraph: the deliverable, with the contract it must meet. A fresh agent reads only this file.

## Success criteria

- Observable outcomes, each checkable by a command or a test.

## Tests

Write first: the tests that fail until the work is done, named.

## Notes

- IMPLEMENTATION_PLAN.md, Stage 2: Skeleton, Planning slice; 0.10 (Protocol).   # sections by name
- Area: `packages/pack-sdk` (types only).                                        # the code area
- Done when the criteria hold and CI is green.                                   # the finish line
```

### 2.9 Lifecycle

```
ready --> active --> review --> done
   \_________________________\--> dropped
```

What the plan says:

- A claim (`owners`) is refused when an owner is already shown (0.10).
- Moving a task's status is the op `setStatus`. The planning slice's criterion is that
  `collide task <id> setStatus active` changes only that item's `status` line.
- The main session spot-checks each builder's result (0.15), and design reviews stay with Fable and Opus.
- Only a person accepts, defers, rejects or resolves a proposal or a question (planned, Stage 4c and 9d). That rule is
  for those kinds, not for tasks.

What the plan does not say: it assigns no actor to `review -> done` for tasks. In practice in this repo, a builder
sets `review` when its criteria hold, and the person or main session that checked the result sets `done`. Treat that
as current practice, not a rule.

By hand, change only the `status` line, and commit the file on its own so a diff shows one change.

### 2.10 The CLI (arrives with the planning slice, Stage 2; until then, edit the files by hand)

None of these commands exists yet. They are the planned surface from Stage 2's "Planning slice":

| Command | What it does |
|---|---|
| `collide tasks [--ready] [--kind <kind>] [--json]` | List items. `--ready` applies the ready rule. |
| `collide task <id> [--json]` | Show one item. `<id>` may be the short form (`12` or `12-ua`) when it matches one item. |
| `collide task <id> <op> ...` | Apply an op: `create`, `setStatus`, `link`, `claim`, `unclaim`. |
| `collide task reid <id>` | Give an item a new suffix, rewrite links that name it, commit nothing. |

Writing behavior (0.10; Stage 2):

- A write goes to the **main checkout's shared root** by default. `--checkout` names another.
- A write leaves an **uncommitted** file. `--commit` commits that one file and nothing else, a stopgap until the
  `record` hook exists (Stage 7).
- Writes take a per-root `flock` and are written by temp file, `fsync` and rename, so a reader never sees half a file.
- `create` writes into the epic's `tasks/` folder and takes the next number for the kind across every ref.

The plan does not fix the argument syntax of each op. Check `collide task <id> <op> --help` once the CLI exists, and
update this section when the slice's CLI items land.

### 2.11 Later

| Feature | Stage |
|---|---|
| Views (named subsets of the corpus, with CEL filters) | 4b |
| Private root and layout setting; `spike` kind; `--view spikes`; the slice's CLI behind the provider supervisor | 4c |
| The task board (read-only desktop app) | 6a |
| The `record` hook, which `--commit` stands in for | 7 |
| Planning in the app: Plan... starts a New session on a private proposal; spike workflow; "Decisions needed" | 9d |
| Share: copies private items to the shared root | 12 |

## 3. Documents

### 3.1 `IMPLEMENTATION_PLAN.md`

The plan is one long document. How it is kept:

- **Numbered revisions.** The top of the file has a summary per revision, newest first: "Revision 13 (2026-11-14)
  applies R5 ...", with bullets that cite the sections changed. The **Status line** names the current revision, what
  has been applied, and what runs next.
- **Frozen until a review or spike lands.** Revision 9 froze the plan: spikes were to run before any further
  review round. An item can say the same: the item for revision 11 says "The plan stays frozen until this lands."
  Spike records state that they only propose plan changes and do not edit the plan.
- **Review log.** At the end. One entry per review or revision, with Changed, Rejected, Deferred and "For the next
  review to check" lists, naming the stages changed.
- **Deferred.** Each deferred item carries **the trigger that brings it back**, for example "when a user needs them
  in the UI".
- **Open questions (owner's call).** Choices only the owner makes. Each lists options, then a "Decided (owner, date)"
  line when settled.
- **Stages.** Each has a **Goal**, **Success Criteria**, **Tests** and **Status**. The Status names the plan revision
  the stage's items came from, and the epic that holds them.
- **Claims are labelled** measured, inferred or unmeasured (see 3.5).

### 3.2 Spike records

A spike answers a question with numbers (Stage 1).

- **Folder:** `.collide/planning/spikes/sp-NNNN-<xx>-<slug>/`, with `record.md`, its reproduction scripts, and
  `out/` and `work/` for raw output and scratch (both git-ignored).
- **`record.md`** has the item frontmatter (`kind: spike`, a status from 2.3) and the findings. A record in this repo
  has a date, a reproduction line, versions and conditions, a summary, results per question, recommendations,
  "Proposed plan changes", "Left unmeasured" and "Reproduction".
- **Reproduction:** one command re-runs every measurement. A record also says under what conditions it ran (for
  example, that the machine was loaded, so timings are upper bounds).
- **Labels:** each figure is **measured** (`[M]`), **inferred** (`[I]`) or **not measured** (`[U]`).
- A record proposes plan changes and does not edit the plan. The owner decides, and a revision applies them.

### 3.3 Reviews

An adversarial review is a task item. This repo's R5 is `ts-0006-ze`.

- **Reviewers are independent and read-only.** They get the plan and records, not the author's conclusions.
- **Ledger.** The item has a "Ledger" section with **one row per finding**: source, disposition, where it landed,
  and reason or trigger. Dispositions seen in this repo: applied, applied in part, rejected (with a reason), deferred
  (with a trigger), considered and not chosen, and noted with no plan change. A totals line sits above the table.
- **Owner decisions are dated** (for example "Owner, 2026-11-14: revision 13 accepted").
- **A Review log entry** in the plan names the stages changed.

### 3.4 Research notes

`docs/research/` holds background notes (planning, tokens and context, and notes on reference systems). They shape the
plan and are cited by it. They are not items, and nothing reads them as planning data. A note that a later spike
corrects says so, with a dated label on the corrected claim.

### 3.5 Writing rules

- **Never cite line numbers.** Cite a plan section or stage name, or a file and a symbol. Lines move.
- **Date owner decisions**: "owner, 2026-11-14".
- **Label claims** as measured (and name the observation), inferred or unmeasured.
- Write plain, short sentences. Match the terms the plan uses: item, epic, shared root, provider, op.
- Mark anything not built yet ("planned, Stage N").

## 4. How a change flows

```
question -> spike -> record -> owner's decisions -> plan revision (with ledger) -> review -> items -> build -> status changes
```

Collide's own history:

1. **Questions and spikes.** The plan lists the spikes it depends on (S1 to S11). Each leaves a record under
   `.collide/planning/spikes/`.
2. **Records.** S1 to S4 and S9 fed revision 10. S5 to S8 fed revision 11.
3. **Owner's decisions.** Each recommendation gets a dated decision.
4. **Plan revision with a ledger.** The task item `ts-0005-st` applied S5 to S8 as revision 11, with 77 rows, each
   with a disposition (applied, rejected, deferred, or noted with no plan change).
5. **Review.** R5 attacked revisions 10 to 12 with two independent reviewers. Both said "ready after the fixes".
6. **Revision with ledger.** The owner's decisions on R5 became revision 13. Ledger: `ts-0006-ze`, 55 rows. A Review
   log entry names the stages changed.
7. **Items.** Only the next stage is split. Stage 2's items (`ep-002-va`) were written against revision 12 and
   brought to revision 13 on 2026-11-14.
8. **Build.** A builder claims an item, works it, and moves it through its statuses.

## 5. Public repos

If a repo is public, keep private project names out of tracked files:

- Tracked files use neutral stand-ins for private project names. In Collide's docs the reference product is
  "Orchard" and the reference workflow "Ferry".
- The mapping and the list of names to refuse live in a git-ignored private folder, not in tracked files.
- A pre-commit check refuses a commit that contains a listed name.

Do not paste real names into items, records or commit messages. The plan itself does not describe this practice; it
is a repo convention.
