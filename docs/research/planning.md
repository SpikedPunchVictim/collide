# Planning in Collide: providers, multi-agent spikes, shared sessions, checklists and lifecycle hooks

Design note for the planning experience, 2026-11-13. The owner chose option (b): a planning pack and source on the
existing task model, with no new core object kinds. This note tests the lead's first proposal rather than
confirming it.

**Inputs, all read only.**
- `IMPLEMENTATION_PLAN.md`: revision 10 with parts E, F and G. Part G was being applied while this note was
  written.
- The five spike records in `docs/decisions/`.
- `docs/research/herdr.md` and `docs/research/ferry.md`.
- The lead's first proposal to the owner.
- The plan CLI's source and README in the Orchard repo.
- The Orchard repo's corpus of plan CLI files, counted with `ls` and `grep`.
- beads' public documentation, fetched.

Nothing was built, installed or run. No other file was edited.

**Corrections** (S11, 2026-11-13). `.collide/planning/spikes/sp-0008-bk-derived-point-coverage/record.md` found three claims here to be wrong:
the writers the plan CLI has (2.6), the plan CLI's write gate (1.2, 7.7 and the example in 7.8), and the "no Collide
gate runs on an agent's `bd` writes" line (2.6). Each is marked "(corrected, S11)". Two short notes (Obsidian in 2.6,
cross-source reactions in 7.7) are marked "(added, S11)".

**Owner decisions recorded here** (owner, 2026-11-13). The first three were open choices in the first proposal.
1. **Sharing is git-backed.** A live sync server is deferred.
2. **Checklists: agents suggest items, and only people confirm them.**
3. **beads is an example solution**, like Orchard and Ferry. It tests the provider seam. Collide does not ship a
   beads provider.
4. **Where a session lives:**
   - **Before Share**, a session stays outside the repo, in Collide's own data directory.
   - **On Share, with the built-in markdown provider**, it goes into the repo next to the code, under
     `.collide/planning/`, committed as `.collide/views/` is.
   - **On Share, with a team system** (the plan CLI, beads), the provider writes through that system's own CLI into
     that system's own paths in the repo.
5. **The built-in provider's path is configurable per repo.** The default is `.collide/planning/`. A repo that
   already keeps decision records elsewhere can point it there. This project keeps them in `docs/decisions/` and
   `spikes/`.

The owner also asked which hooks the planning lifecycle needs. Section 7 answers that.

**Labels.**
- **[read `loc`]**: seen at that file and symbol (or plan section).
- **[fetched `name`]**: read in beads' public docs (URLs below).
- **[inferred]**: reasoning the source does not state.
- **[read … (counted)]**: a count I took with `ls` or `grep`.
- **[proposal]**: something this note proposes, which no source states.

**Location prefixes.**
- `plan` `IMPLEMENTATION_PLAN.md`, cited by section and bold lead-in, or by stage name.
- `rec:` `docs/decisions/`. `res:` `docs/research/`.
- `prop` the lead's first proposal, as relayed to the owner, cited by section and bold lead-in.
- `bd:readme` https://github.com/steveyegge/beads (README).
- `bd:deps` https://raw.githubusercontent.com/steveyegge/beads/main/docs/core-concepts/dependencies.md.
- `bd:sync` https://raw.githubusercontent.com/steveyegge/beads/main/docs/core-concepts/sync-concepts.md.
- `bd:faq` https://raw.githubusercontent.com/steveyegge/beads/main/docs/reference/faq.md.
- `bd:cli` https://raw.githubusercontent.com/steveyegge/beads/main/docs/CLI_REFERENCE.md (first 200,000 of
  312,095 characters read).

**Where each mechanism lives.**
- **core**: generic, and useful to a non-planning pack too. This is the R4 rule: a finding changes core only if a
  pack that is not Orchard, such as issue → PR → CI, would hit it as well [read plan: Review log,
  a finding changes core only if a pack that is not Orchard (issue → PR → CI) would hit it too].
- **`collide/plan`**: a new built-in pack, trusted with the app like `collide` [read plan 0.4,
  The built-in `collide` pack].
- **provider**: a source adapter (0.10).
- **example**: Orchard, Ferry and beads. Core never names them [read plan 0.1, Orchard is the reference pack,
  not a special case].

## Summary

> **Superseded in part (owner, 2026-11-13, after this note).** A planning session is now a **proposal** (`pr-`),
> and there are no decision or deferred files. A decision is the Decision section of an accepted proposal or a
> resolved question, and `deferred` is a proposal status. The markdown provider's layout is fixed: `epics/<epic>/`
> holds `epic.md` and the `proposals/`, `questions/` and `tasks/` folders, and `spikes/` sits at the top level. The
> folder gives epic membership. Ids are a kind prefix, a number and a letter suffix (`ts-0012-ua`), not random
> ids. Section 2.4's per-kind `paths:` and folder per session, and section 4.2's `session:` field, no longer apply.
> Collide's own spike records and their scripts moved from `docs/decisions/` and `spikes/` into
> `.collide/planning/spikes/`; file citations here were updated, and bare mentions of the old folders are
> historical. A proposal's statuses (plan 0.10, owner, 2026-11-14) replace section 7.2's decision and session
> tables, and "Plan…" (plan 0.11) is how a person plans with an agent. Where this note and plan 0.10 differ, 0.10
> wins.

| # | Question | Verdict | Where | Stage |
|---|---|---|---|---|
| 1 | Working with the team's system | A provider declares its **kinds** (each with its own statuses), its **typed links** and its ops. `collide/plan` maps kinds to planning roles. One planning source per board, with no silent fallback to repo markdown. | core (kind, links) + `collide/plan` + provider | 2, 4 |
| 2 | A repo with no planning system | A markdown provider in `collide/plan`, with random ids and an alias. Private root in Collide's data directory; shared root `.collide/planning/` by default, **configurable per repo (owner, 2026-11-13)**. Collide's own records are its first user. | `collide/plan` | 4 |
| 3 | Lifecycle hooks | Generic task points in core: after a status write, around other ops, around Share. `collide/plan` names the planning moments as **derived points**: a core point plus a filter. Provider ops write the register, hooks react and may block, observers notify. | core (points) + `collide/plan` (derived points) | 2, 8, 9, 12 |
| 4 | Several agents on one spike | Adversarial rounds and a merge phase already fit the engine. Panels and specialists need **fan-out instances** (a role, prompt, model or profile per instance) and a run-level budget. | core + `collide/plan` | 2, 8, 11 |
| 5 | Verify gate | A `script` gate re-runs each claim's `repro:` and compares the result within a declared tolerance. It samples under a verify budget, and asks before running a teammate's scripts. | `collide/plan` | 11 |
| 6 | Record format | The source keeps its own frontmatter (the plan CLI's is strict). Collide's structure goes in the body: a pre-registration, a summary table and fenced `claim` blocks. | `collide/plan` | 4 |
| 7 | Sharing | **Git-backed, and private in Collide's data directory until Share (owner, 2026-11-13).** Share moves each item into the planning source: the configured root, or the team system's own CLI and paths. `record` commits it. | `collide/plan` + core (move op, branch-only items) | 12 |
| 8 | A teammate continuing | The handoff note is built at Share on the sharer's machine, reviewed, then committed with the session. The teammate adopts the run in their own checkout. | core (reuse) | 12 |
| 9 | Checklists | **Agents suggest, people confirm (owner, 2026-11-13).** A `checklist` field type and drawer UI in core. Templates and the gate in packs. | core + `collide/plan` | 2, 9, 12 |
| 10 | beads | **Example solution (owner, 2026-11-13).** It is Dolt-backed and not branch-scoped, so it tests the seam where the plan CLI cannot. | example | 12 |
| 11 | Several decisions in one step | Stays deferred. Disagreements become question items in the source, and a `wait` phase holds the run until they are resolved. | `collide/plan` | 11 |

## 1. How planning works today

### 1.1 This project

Collide is planned in markdown, in conversation and in git. No planning item has a machine-readable id or status.

| Kind | Where it lives | Its states | Evidence |
|---|---|---|---|
| **Decision** | Inline in plan text as "(owner, date)": 50 lines cite one. Also "Decided (owner, …)" in Open questions. | made, or not | [read `plan:` (counted: lines containing "owner, 20")], [read plan: Open questions, When the desktop app appears] |
| **Spike** | A Stage 1 entry with its questions. Code in `spikes/<id>/` with a README and `run-all.sh`. A record in `docs/decisions/<id>.md`. | planned → done (prose: "done: `docs/decisions/…`") | [read plan: the spikes stage], [read `rec:s9-tokens-context.md`, the intro under the title] |
| **Question** | "Open questions (owner's call)", numbered, with options, pros and cons, then "Decided (owner, date)". | open → decided | [read plan: Open questions] |
| **Deferred item** | "Deferred", each with the trigger that brings it back. | deferred → back in a stage | [read plan: Deferred] |
| **Review round** | R1–R4 in the Review log. Two reviewers (Fable and Opus). Each finding is accepted, modified or rejected, with where it landed. | per finding | [read plan: Review log] |
| **Revision part** | A named edit to the plan (revision 10 parts C, D, D2, E, F, G). "Not taken" lists what was declined. | drafted → applied | [read plan: Review log, Ferry entry, "Not taken"], [read plan: Review log, No publishing needed] |
| **Research note** | `docs/research/*.md`, an input to a revision. | written | [read `res:herdr.md`, the intro] |

**A spike record has a fixed shape** [read `rec:s2-interactive.md`, "Proposed plan changes", "Not measured", "Spend",
"Left behind"]:
- header bullets: Date, Versions, Reproduction, Method, Labels;
- `## Answers`, with Q1…Qn, each ending in **Answer:**;
- `## Proposed plan changes` ("None applied", cited by plan section);
- `## Not measured`, `## Spend` and `## Left behind`.

S9 adds a summary table [read `rec:s9-tokens-context.md`]. S3 uses `## Decision`, with a recommended owner [read
`rec:s3-process-ownership.md`]. Claims are labelled measured, inferred or unmeasured.

**The steps between them** [inferred from the plan, the records and the review log]:
1. A question is raised: in conversation, by a review finding, or in a record's "Not measured".
2. Either the owner decides it directly, and the decision is cited inline, or it becomes a spike: its questions
   are written into Stage 1 with a spend cap.
3. An agent runs the spike and writes the record. The lead spot-checks load-bearing claims by re-running them.
4. The lead relays the proposed plan changes to the owner, with options and a recommendation.
5. The owner decides, often taking the recommendations.
6. The changes become a revision part: drafted, dry-run, then applied.
7. A review round tests the revision, and its findings are triaged by the R4 rule.
8. A deferred item comes back when its trigger fires.

**What is missing** [inferred]:
- **Decisions have no id and no list.** "Which decisions rest on S9?" is a grep.
- **State is prose.** A tool cannot read whether a spike is done or a question is open.
- **The human queue is the lead's memory.** Nothing lists the decisions waiting on the owner.
- So a planning view over Collide's own repo needs either a format or the markdown provider (2.4).

### 1.2 The plan CLI (Orchard's system)

The plan CLI works over a corpus of markdown files with YAML frontmatter. A file's path is its identity. It keeps one
fact in one place and never stores an inverse relation [read the plan CLI's README].

| Kind | Statuses and legal edges | Notable fields |
|---|---|---|
| ADR | proposed → accepted \| rejected; accepted → superseded \| deprecated | `deciders`, `supersedes`, `evidence` → spike, `summary` |
| Spike | PLAN.md: open → running → concluded; open or running → abandoned; concluded → superseded. RESULTS.md has no status. | PLAN: `owners`, `question`, `supersedes`, `tags`. RESULTS: `concluded`, `recommendation`, `classification` (measured \| inferred \| unmeasured) |
| Question | open → resolved \| dropped | `needs` (human \| evidence \| either), `raised`, `blocks` → obligation, `resolved_by` (required iff resolved) |
| Obligation | open → closed \| retracted \| superseded | `classification`, `raised_by`, `closed_by`, `arises_from` |
| Proposal | draft \| reviewed \| stable \| superseded | `depends_on`, `adrs_consumed` |
| Epic, Task | Epic: planned … done. Task: specified → ready → doing → in_review → done \| dropped | Task: `depends_on`, `blocked_by`, `reviewed_by`, `awaiting` |

Sources: [read the plan CLI's source: its frontmatter schemas and its table of legal status edges].

What matters for Collide:
- **Source refs.** `raised_by`, `closed_by` and `resolved_by` name a spike, ADR or proposal, or
  `human:YYYY-MM-DD` for a decision a person made without a document behind it [read the plan CLI's README].
  That is Collide's "(owner, date)" in a form a tool can read.
- **A human queue already exists.** `plan status` lists every open question with `needs: human` or `either` [read
  the plan CLI's source].
- **Five provenance markers:** MEASURED, INFERRED, UNMEASURED, CORRECTED and RETRACTED [read
  the plan CLI's README].
- **Rules link the kinds.** An accepted ADR needs every evidence spike concluded. A concluded spike needs a
  RESULTS.md [read the plan CLI's source]. These are `plan check` rules, so `plan set` does not refuse them
  [inferred from the rule sources]. (Corrected, S11:
  the write gate runs every rule, so `plan set` does refuse them. See 7.7.)
- **Writes are commands.**
  - `plan new adr|spike|obligation|question|task`, `set`, `close`, `resolve`, `move`, `supersede` and `renumber`,
    with `--dry-run` [read the plan CLI's README].
  - A `--format json` envelope with `schema_version: 1` [read the plan CLI's README].
  - Exit codes 0, 1, 2, 130 and 143 [read the plan CLI's README].
- **Gaps a provider must fill.**
  - `plan new` cannot create proposals or epics.
  - Nothing writes RESULTS.md. It is left to a later close-out step that the package does not implement [read the plan
    CLI's source].
- **Frontmatter is strict.** Every schema rejects undeclared keys [read the plan CLI's
  source]. Collide cannot add its own keys to plan CLI frontmatter (section 4).
- **Ids per checkout.** Task ids are allocated per checkout and reconciled at merge by `plan renumber`
  [read plan 0.10, Identity]. ADR numbers use a lock.
- **Bugs that would hit a provider** [read the Orchard repo's bug-hunt notes on the plan CLI]:
  - #2: `plan new` overwrote an existing file. The current source fixes it with a create-only write.
  - #3: concurrent `new adr` runs produced duplicate numbers. The current source fixes it with an atomic lock file.
  - #4: piped `--format json` output was cut at 65,536 bytes. The CLI's process helper now sets the exit code instead
    of exiting at once, but the command base class still exits at once on two paths. Whether every path is fixed is
    unverified.
- **Scale in the Orchard repo** [read its plan corpus (counted)]:
  - 110 spike folders, with 108 PLAN.md and 104 RESULTS.md. Statuses: 95 concluded, 7 running, 3 open,
    2 abandoned, 1 superseded.
  - About 175 questions. 54 are open and closable by a person (26 `human`, 28 `either`). 29 are open and waiting
    on evidence.
  - About 35 ADRs, 55 obligations and 1599 tasks.
- **A rich spike shape: one Orchard spike** [read that spike's PLAN.md and RESULTS.md, section headings].
  - Its PLAN.md pre-registers:
    - questions and assumptions;
    - arms, with predictions and "what makes each red";
    - controls, and pass, fail and kill rules;
    - what each outcome decides, and stop rules.
  - Round amendments are added later, with the owner's answers written before any run.
  - Its RESULTS.md quotes the predictions unedited. Then come Deviations, per-question results, a Verdict, "What
    this spike did not check", "Predictions against outcomes" per round, and "Owner decision 2026-11-07".

### 1.3 beads

beads is "a distributed, git-backed graph issue tracker for AI agents", with a `bd` CLI [fetched `bd:readme`]. The
lead's proposal called it git-backed [read prop 1, A provider maps a team's system onto those roles; prop 3,
(a) Git-backed]. The current docs differ in one way that matters.
- **Storage is Dolt.** Embedded Dolt is the default, with a single writer, in `.beads/embeddeddolt/`. Server mode
  allows concurrent writers [fetched `bd:readme`].
- **Sync is not per branch.**
  - Issue history lives under `refs/dolt/data` on the git remote, "separate from source branches such as
    `refs/heads/main`". It moves with `bd dolt push` and `bd dolt pull` [fetched `bd:sync`].
  - `.beads/issues.jsonl` is an export, "not the canonical cross-machine sync channel" [fetched `bd:sync`].
  - Older third-party descriptions say JSONL in git. I saw those only in search results and did not read them.
- **Types**: bug, feature, task, epic, chore, **decision** (alias `adr`), **spike**, story and milestone. The system
  types are message, molecule, gate and event. Custom types come through config [fetched `bd:cli`].
- **Statuses**: open, in_progress, blocked, deferred and closed, plus custom ones [fetched `bd:cli`].
- **Dependencies** [fetched `bd:deps`]:
  - blocking, which decide `bd ready`: `blocks`, `parent-child`, `conditional-blocks`, `waits-for`;
  - annotations: `related`, `tracks`, `discovered-from`, `caused-by`, `validates`, `supersedes`.
- **Ready** means every blocking dependency is closed [fetched `bd:deps`].
- **Claiming**: `bd update <id> --claim` sets the assignee and `in_progress` atomically [fetched `bd:faq`].
- **People**: a bead with the `human` label needs a person [fetched `bd:cli`].
  - `bd human list`, `respond` (comments and closes) and `dismiss`.
  - Gates block work until a condition clears: human approval, a timer, a PR or CI [fetched `bd:deps`],
    [fetched `bd:faq`].
- **Workflows**: formulas, which are TOML or JSON templates, are cooked into molecules (persistent) or wisps
  (ephemeral) [fetched `bd:faq`]. I did not read these in detail.
- **Interfaces**: `--json` on every command, an MCP server, hash ids such as `bd-a1b2`, and Dolt's cell-level merge
  [fetched `bd:readme`], [fetched `bd:faq`].
- **It writes agent config.** `bd init` installs Claude Code, Codex and Cursor integrations unless given
  `--skip-agents` or `--stealth`. `bd setup claude` installs hooks and settings [fetched `bd:readme`].
  - So in a beads repo, agents write task state with `bd` themselves [inferred].
  - Collide sees those writes through `watch`, so the source stays the register. No Collide gate runs on them
    (7.1).

### 1.4 Side by side

| Role | Collide today | plan CLI | beads |
|---|---|---|---|
| decision | inline "(owner, date)" | ADR | type `decision` |
| decision with no document | "(owner, date)" | `resolved_by: human:YYYY-MM-DD` | close reason |
| spike | Stage 1 entry + record | PLAN.md + RESULTS.md | type `spike` |
| question needing a person | Open questions | Question, `needs: human\|either` | `human` label |
| deferred item, with trigger | Deferred list | none native | status `deferred` (trigger as text) |
| human queue | the lead relays | `plan status` queue | `bd human list` |
| claim | none | spike `owners` | `--claim` |
| evidence label | measured / inferred / unmeasured | `classification`, 5 markers | none native |
| typed links | prose | forward-only: `evidence`, `supersedes`, `blocks`, `arises_from` | 10 dependency types |
| ids | S9, R4, part E | from the path; per-checkout allocation; renumber at merge | hash ids |
| scope | git, per branch | ref-scoped (per checkout) | global (`refs/dolt/data`) |
| review round, revision part | review log, revision | Proposal (draft → stable) | epic or molecule |

**The finding that shapes sections 2, 5 and 7** [inferred]:
- The plan CLI is ref-scoped and beads is global.
- So "share" means a commit for one and `bd dolt push` for the other.
- Sharing is therefore the provider's act, not a git action in core.
- Owner decision 4 says the same: each system writes through its own CLI into its own paths.

## 2. Roles and providers

### 2.1 Roles

A role is the planning meaning that a source item plays. Roles belong to `collide/plan`, not to core, and core
stores no role names [inferred; required by "mechanism in core" plan 0.1, Mechanism in core, policy in packs].

| Role | Meaning | Category when open / closed |
|---|---|---|
| `decision` | A choice made or proposed, with its reasons | `ready` (proposed), `review` (recommended) / `done` (accepted), `dropped` (rejected, superseded) |
| `spike` | A question answered by running something, with a record | `backlog` → `ready` → `active` → `review` / `done`, `dropped` |
| `question` | A choice waiting on a person or on evidence | `ready` / `done` (resolved), `dropped` |
| `deferred` | Work parked until a trigger | `backlog` |
| `session` | One planning effort: a goal, its items, its checklist, its runs | `backlog` → `active` → `review` / `done`, `dropped` |

Section 7.2 gives each role's states and transitions.

Review rounds and revision parts are not roles [inferred]:
- **A review round is a run of a review workflow.** Its record is a phase artifact (`adjudicate/round-record.md`),
  and Collide already keeps phase artifacts as process state [read plan 0.7, Example: the Orchard pack,
  `workflows.loop`], [read plan 0.7, `show:`].
- **A revision part is a session whose output is a document edit.** It is integrated like any run.

### 2.2 What a provider declares

The 0.10 protocol already covers most of this: `fields`, `ops` with JSON Schemas, `statuses` with categories, and
`capabilities` [read plan 0.10, Protocol]. Writes are operations, so `resolve`, `supersede`, `writeResults` and
`link` are provider ops and need no core change [read plan 0.10, Writes are operations,
not field patches]. Six additions are needed.

| Addition | Why planning needs it | Non-planning pack that benefits | Where |
|---|---|---|---|
| **`kind`**: a normalized field, plus `kinds` in `initialize`, each with its own statuses, transitions and ops | The plan CLI's statuses differ by kind: an ADR is `accepted`, a question `resolved`. The UI offers "the status transitions it lists" [read plan 0.10, Writes are operations, not field patches], which only works per kind. | GitHub issues and PRs (a PR can be `merged`). beads types. `accepts:` and views for "bugs only". | core |
| **Op effects**: each op declares the category it moves an item to, if any | The plan CLI's `resolve` closes a question. `task.before_status` must fire before it, so core must know the target category before the op runs (7.3). | GitHub "close as not planned" | core |
| **`links`**: typed links `{ type, to, blocking }`, declared per provider. Collide derives the inverses, as it already shows `deps` both ways. | evidence for, supersedes, resolves, informs, recommends | GitHub sub-issues, duplicates and "closes #n". beads' ten types. | core |
| **`assignee`**: a normalized field, plus a `claim` op where the source has one | "claimed by", so two people do not run one spike | "assigned to me" views; not launching an issue a teammate has taken | core |
| **`checklist`** as a field type (section 6) | the planning checklist | PR and release checklists (GitHub task lists) | core |
| Optional **role tags** per kind, opaque to core (`plan.decision`) | setup is automatic when a board adds the source | any pack that finds kinds by meaning | core stores strings; `collide/plan` reads them |

A provider with one kind and no links needs no change, so the two v0 adapters keep working [inferred].

### 2.3 The fallback for roles a provider lacks

The first proposal says that roles a provider cannot hold "fall back to markdown files in the repo", so that
"nothing is copied into two places" [read prop 1, Each provider declares which roles it can hold]. I disagree with the
fallback as stated. It does not copy, but it does **split** the register. A decision in the plan CLI whose session and
checklist sit in Collide markdown is one planning record held in two systems:
- teammates who use the team's system directly see only half of it;
- links cross sources, and cross-source dependencies are deferred [read plan: Deferred, Cross-source dependencies];
- there are two id spaces and two places to look.

| Option | For | Against |
|---|---|---|
| (a) Per-role fallback to repo markdown (the proposal) | Every feature works with every system | Splits the register silently; links cross sources |
| (b) One planning source per board. A role the source lacks is off, and the UI says why. | One register | Features go missing: the plan CLI has no deferred kind, beads no evidence label |
| (c) (b), plus mapping a missing role onto a kind the source does have, plus an opt-in fallback per role, badged "held in Collide markdown, not in plan" | One register by default, and nothing is impossible | Two mappings to explain |

**Recommendation: (c)** (open question Q1).
- The plan CLI's "deferred" maps to a question with `needs: evidence` and a `deferred` tag.
- beads' "deferred" maps to its status `deferred`.
- A repo with no system uses the markdown provider as its planning source, and that is not a fallback.

### 2.4 The default: a markdown provider in `collide/plan`

- **One file per item.** Collide's own records are already one file per spike.
- **Two roots, one format** (owner decision 4):

  | Root | Where | Scope | Committed |
  |---|---|---|---|
  | private | `planning/` in `collide/plan`'s data directory, per board, under Collide's data directory [read plan 0.7, Pack data] | per user, like scratch tasks [read plan 0.10, scratch tasks] | never |
  | shared | the configured path, `.collide/planning/` by default (owner decision 5) | ref-scoped, like plan | by `record`, as `.collide/views/` is [read plan 0.10, Views] |

  For this provider, Share copies an item from one root to the other. Ids and links do not change.
- **Identity is the frontmatter `id`, never the path.** File names are `<alias or id>-<slug>.md`, as in
  `s9-tokens-context.md`. So a file can move without breaking a link. This is unlike the plan CLI, where the path is the
  identity [proposal].
- **Frontmatter belongs to this provider**, so it can be Collide's own format (section 4).
- **The shared root is data, not config** [inferred]. Unlike views and `board.yaml` [read plan 0.9, Trust], it
  stays out of the trust hash. Its text is fenced as untrusted, like any task text [read plan 0.9, Task text is data].
- **Ids are random and short** (`sp-4k7q`), with an optional human `alias` (`S10`). Collide does not allocate
  sequential numbers, for three reasons (open question Q6):
  - the plan CLI's per-checkout ids need a renumber at merge [read plan 0.10, Identity];
  - its lock once produced duplicates [read the Orchard repo's bug-hunt notes on the plan CLI, #3];
  - beads uses hash ids so that branches and agents do not collide [fetched `bd:readme`].
- **Ops:** `create`, `setStatus`, `resolve { by, text }`, `link`, `claim`, `unclaim`, `supersede`, `setChecklist`
  and `share`.
- **Collide's own repo is the first user.** Its five records gain frontmatter, which is an additive edit. The plan's
  Open questions and Deferred list stay in the plan until the planning view ships (open question Q8).

#### The path setting

**Decided (owner, 2026-11-13):** the shared path is configurable per repo, and `.collide/planning/` is the default.
Where the setting lives and what a change does are this note's recommendation (open question Q10).

**The setting.** It is a `collide/plan` pack setting, declared in the pack's `config:` with a JSON Schema and a
default, and passed to the provider in `initialize` [read plan 0.10, Protocol]. Illustrative shape:

```yaml
# .collide/config.yaml (committed; the repo's value wins)
collide/plan:
  layout:
    root: .collide/planning        # the default: one folder per session
    paths:                         # optional: one directory per kind, instead of the session folder
      spike: docs/decisions        # this project's records
    code: spikes                   # repro scripts and instance records: spikes/<alias or id>/
```

**Where it sits in the plan's layers** (lowest first) [read plan 0.7, Settings]:

| Layer | Holds the path? | Why |
|---|---|---|
| the schema's default | `.collide/planning` | owner decision 5 |
| the user's file, `~/.config/collide/config.yaml`, per board | only until the team shares `.collide/` | board settings stay per user until Share with team [read plan 0.11, First run works with no pack and no tasks] |
| the repo's `.collide/config.yaml` | **yes, and it wins** | every teammate must read and write the same place, so the key is not `overridable` |
| the run's profile | no | a provider starts per board, not per run [inferred] |

- **The alternative was `board.yaml`,** which already holds source configuration [read plan 0.7,
  The Collide directory]. For: the path sits beside the other source settings. Against: `board.yaml` has no per-user
  layer before Share with team, and core would have to parse one pack's provider settings. The pack setting avoids both.
- **Checks at load** [proposal]. Each path is repo-relative and inside the repo, with no `..`. It is not under
  `.git/`, a pack path, or the Collide directory's config (`board.yaml`, `config.yaml`, `views/`, `checklists/`).
  A bad path is refused with the reason.
- **Trust.** `.collide/config.yaml` is in the trust hash [read plan 0.7, Settings], so a change to the path
  asks again.
- **Sharing a session while the path is only in the user's file.** A teammate's Collide would look in the default
  place and find nothing. So the Share preview adds the setting to `.collide/config.yaml`, in the same commit, and
  says so [proposal].
- **This project** sets `paths.spike: docs/decisions` and `code: spikes`. Its five records list once they gain
  frontmatter.

**When the path changes after sessions exist** [proposal]:

| What | Effect |
|---|---|
| private sessions | none: they live in the data directory |
| new items | go to the new path |
| shared items at the old path | stay where they are. Collide lists them and offers **Move**: a `git mv` per file, committed by `record` in one commit with the config change. Links do not change, because identity is the `id`. |
| the old path | Collide adds it to `layout.previous`, which is read-only. The provider still lists items found there, with an "old path" chip, so nothing disappears. |
| writes to an item at the old path | stay at the old path. Only Move moves files. |
| branches forked before the change | their items list through `previous`. When such a branch merges, `doctor` reports new files at the old path and offers Move. |
| removing an entry from `previous` | refused while items remain there |

| Option | For | Against |
|---|---|---|
| (a) Offer Move, and keep reading old paths (above) | Nothing vanishes, on any branch; the move is one reviewable commit | A `previous` list to explain |
| (b) Refuse a change while shared items exist | Simple | Blocks a repo that adopts Collide after it has items |
| (c) Move every file automatically on change | No old paths | A large commit nobody asked for; other branches still have files at the old path |

**Recommendation: (a).**

### 2.5 Hook points, in brief

Section 7 covers the planning lifecycle's hooks in full. In short:
- **No core point is named after planning.** The catalogue names points `object.before_x` or `object.after_x`
  [read plan 0.4, Named points]. Points named after decisions or spikes would put planning policy into core.
- **Core gains generic task points:** `task.after_status`, `task.before_op` and `task.after_op`,
  `task.before_move` and `task.after_move`. `task.before_status` already exists [read plan 0.4, Named points].
- **`collide/plan` names the planning moments** (a decision decided, a spike concluded, a session shared) as
  derived points: a core point plus a filter (7.5).
- **Mirroring that only notifies** stays an observer [read plan 0.4, Observers].

### 2.6 Mappings

**The plan CLI.** This is the Orchard pack's `plan` provider, extended. It is pack work, not core.

| Role | Kind | Status → category [inferred] | Ops (through `./plan`) | Links |
|---|---|---|---|---|
| decision | `adr` | proposed → review; accepted → done; rejected, deprecated, superseded → dropped | `plan new adr`, `plan set`, `plan supersede` | `supersedes`, `evidence` → spike |
| spike | `spike` (PLAN.md + RESULTS.md) | open → ready; running → active; concluded → done; abandoned, superseded → dropped | `plan new spike --question --owners`, `plan set`. The provider writes RESULTS.md, then runs `plan check`. | `supersedes` |
| question | `question` | open → ready; resolved → done; dropped → dropped | `plan new question`, `plan resolve` with `human:<date>` | `blocks` → obligation |
| deferred | none. A question with `needs: evidence` and a `deferred` tag (2.3). | backlog | `plan new question` | — |
| session | `proposal` | draft → active; reviewed → review; stable → done; superseded → dropped | none: `plan new` cannot create one [read the plan CLI's source] | `depends_on`, `adrs_consumed` |
| (claim) | spike `owners` | — | none after creation (corrected, S11: `plan set` refuses array fields) | — |

Costs:
- **More parsing.** Today the provider parses only task-file frontmatter [read plan 0.10, Two first-party adapters,
  the `plan` row]. Adding kinds is parsing work in the Orchard pack.
- **Files with no CLI writer.** RESULTS.md and proposals have none. The provider writes those files itself and
  validates them with `plan check` [inferred]. That is the one place where it does not write through the system's
  own CLI.
- **No CLI writer for array fields after creation** (corrected, S11). The first version of this table gave `plan
  set` as the writer for a claim (`owners`), and the links `evidence`, `depends_on` and `blocks`. `plan set` writes
  scalar fields only and refuses array fields [read the plan CLI's source]. So after creation
  the plan CLI has no CLI writer for any of them. The provider either leaves out `claim` and `link` for the plan CLI, or
  edits the frontmatter itself and checks the content-hash version before the write. It then runs `plan check`, as
  it does for RESULTS.md.
- **New ids at Share.** The plan CLI allocates its own ids then. The provider returns the new keys, and Collide
  rewrites the session's links to them. Run records keep the old key as an alias [inferred].
- **Body, not frontmatter.** Collide's per-claim structure cannot go in the plan CLI's frontmatter (it rejects unknown
  keys), so it goes in the body (section 4).
- **Fewer categories.** Plan CLI spikes have no review status, so a spike with results is still `running`, category
  `active`. A gate bound to `review` never fires for a plan CLI spike (7.2).

**beads.** This is the example provider, which tests the seam. Collide does not ship it.

| Role | Kind | Status → category [inferred] | Ops | Links |
|---|---|---|---|---|
| decision | type `decision` | open → review; closed → done, or dropped (from the close reason) | `bd create -t decision`, `bd close --reason` | `supersedes` (`bd supersede … --with …`) |
| spike | type `spike` | open → ready; in_progress → active; blocked → ready; deferred → backlog; closed → done | `bd create -t spike --design`, `bd update --claim`, `bd close --reason-file` | `validates`, `discovered-from` |
| question | `human` label on any type | open → ready; closed → done; "Dismissed" → dropped | `bd human respond`, `bd human dismiss` | `blocks` |
| deferred | status `deferred` | backlog | a status update (flag unverified) | — |
| session | type `epic`, or a molecule | open → active; closed → done | `bd create -t epic` | `parent-child` |
| (claim) | assignee | — | `bd update --claim` | — |

What beads tests that the plan CLI does not [inferred]:
- **Global scope.** Items are not per branch. Share writes into `.beads/`, and a `bd dolt push` publishes it.
- **A category from a close reason**, not from a status name.
- **Writes that Collide did not make**, by agents using `bd` from the hooks it installs. No Collide gate runs on
  them (7.1). (Corrected, S11: a write made inside a Collide phase is a run's write. The pack sets `BEADS_ACTOR` in
  the phase's `env:`, so the journal attributes the write to the run. `after_*` points then fire once, and the
  `before_*` gates run as a phase-end check. A teammate's write still fires nothing. The pack also sets
  `events-journal: true` and re-baselines after `bd dolt pull`, because the journal is off by default and misses
  sync.)
- **A single writer** in embedded mode. The provider must serialize its writes and retry on a lock (unverified).
- **Record bodies.** beads has design, acceptance and notes fields [fetched `bd:cli`]. The example provider decides
  whether a full record fits there or lives in a repo file that the issue points to.

**Obsidian** (added, S11). Not mapped in this note. S11 read it as a backend, and the pack-level findings are P3 in
`.collide/planning/spikes/sp-0008-bk-derived-point-coverage/record.md`:
- Identity is the path, so a rename in the app looks like a delete plus a create. The pack should require a
  frontmatter id (`uid`), never the path.
- Templater can rewrite a new file just after Collide's create. The provider should settle a create before it reads
  the version, or Collide's own new item shows the outside-change chip.
- An open editor can save stale content over Collide's write. No `conflict` result reports that.
- The CLI writes to the app's vault, not to a worktree. Write through its `move` when the app runs, and otherwise
  rewrite the links.
- Gates are nearly useless here: nothing can veto an in-app edit, so almost every change is an outside change.

### 2.7 What forces a core change

Only four things do:
- the six additions in 2.2;
- the task points and derived points (section 7);
- the move op (5.3);
- items that exist only on a branch (5.6).

Everything else is `collide/plan` or a provider: roles, record parsing, views, the spike workflow, the merge
script and the verify gate.

## 3. Several agents on one spike

### 3.1 Patterns

| Pattern | Shape | Fits today? | What is new |
|---|---|---|---|
| **Panel** | N agents with one brief, each on a different model, agent definition or profile, each in its own workspace. Then a merge. | Different models only: `fanout` is "one role on several models" [read plan 0.7, Phase kinds] | **Fan-out instances**: a model, agent definition, profile or prompt per instance |
| **Adversarial** | A proposer, then challengers who must cite evidence, then an adjudicator who loops or stops, for bounded rounds. | **Yes.** agent → fanout → agent, with `round`, a decision script and `collide/review`'s convergence stop. Orchard's loop has this shape [read plan 0.7, Example: the Orchard pack, `workflows.loop`], [read plan 0.4, The built-in `collide` pack]. | nothing |
| **Specialists** | Fixed lenses (security, performance, UX), each with its own brief. Then a merge that cross-checks overlaps. | No: briefs are identical across a fan-out [read plan 0.7, Example: the Orchard pack, `workflows.loop.phases.review.fanout`] | Fan-out instances with per-instance prompts. A fixed, declared list avoids the deferred runtime-sized fan-out [read plan: Deferred, More of Ferry's patterns in core]. |
| **Replicate** (added) | One agent runs the spike. A second gets only the pre-registration and the reproduction scripts, not the answers, and derives the answers again. | Yes: two agent phases with different prompts | nothing |
| **Challenge the merge** (added) | After a panel's merge, one challenger attacks the merged recommendation. It catches errors the whole panel shares. | Yes: one more agent phase | nothing |

"Agents of different kinds" covers different agent tools as well as different models.
- Other agent tools are deferred until after Stage 10 [read plan: Deferred, Other agent tools and model sources].
- Until then, a different kind means a different Claude agent definition or profile.
- Or it means a `script` phase with `agentWork: true` that runs another tool itself. Its usage is then reported,
  not measured [read plan 0.3, Agent work inside a `script` phase].

### 3.2 Independence until merge

- **Briefs carry the spec, never another agent's output or the lead's hypothesis.** An agent handed a hypothesis
  tends to confirm it [read prop 2, Agents stay independent until a challenge or merge]. Panel instances do not see the
  pre-registered predictions, which are compared at merge [inferred].
- **One workspace and one `phase.dir` per instance.** The plan has instances per fan-out model, and sandbox
  workspaces [read plan 0.8, Phase]. It does not say whether every instance gets its own directory by default.
- **Nothing is pushed before merge.** Instances share one clone's refs, so each instance gets its own branch name
  [inferred].
- **Agent memory.** Claude's auto memory is keyed by project path [inferred], and its behaviour belongs to the
  Claude adapter [read plan 0.1, Orchard is the reference pack,
  not a special case]. Whether two worktrees share memory is unmeasured (S10 Q2).
- **The refusal rule generalizes.** Today a fan-out whose aliases resolve to one model is refused [read
  plan 0.3, Who picks what]. With instances, the refusal is for two instances that are identical in every respect.

### 3.3 The merge phase

Fan-out join already exists. A later phase sees every instance's artifacts through `history[]` [read
plan 0.8, History], and Stage 8 tests "fan-out join" [read plan: the workflow engine stage,
Tests]. The merge is two phases in `collide/plan`, plus a wait:
1. **A `script` phase** aligns the records by question id and claim id (4.4). It writes `merge-table.md`, which
   sorts each claim into one of four cases: agree, disagree, measured by one only, or measured by none.
2. **An `agent` phase** (the merger) writes prose only where the table does not show agreement, and names each
   disagreement it could not settle.
3. **Each disagreement left becomes a question item in the source**, linked `informs` → the spike. A `wait` phase
   holds the run until those questions are resolved (3.6).

### 3.4 Verify gate

The verify gate is a `script` gate in `collide/plan`. Its verdict is the result file the script writes, as with
every gate and `script` phase [read plan 0.5, Result], not an agent's report.
- **What it runs.** For each claim labelled `measured` that has a `repro:`, the gate runs that script in a clean
  workspace at the record's commit. It compares the output with the claim's `value`, within the claim's
  `tolerance` (exact when none is given).
- **Verdicts:**
  - `ok`;
  - `fail`, when a measured claim did not reproduce;
  - `unverified`, when the claim cannot be re-run. That covers live services, credentials, more than an hour of
    idle time as in S9 Q6 [read `rec:s9-tokens-context.md`, "Reproduction"], and spend over the verify budget.
- **Model output is not deterministic.** A cost or token claim needs a tolerance. A claim about what a model said is
  `unverified` unless it is about a recorded transcript [inferred].
- **Cost.** Re-running spends money again.
  - Each claim carries `cost_usd`.
  - The gate verifies every claim marked load-bearing, plus a sample of the rest, under a verify budget.
  - The preview shows that budget.
- **Trust.** A `repro:` script is code.
  - A teammate's script runs as the user, as their tests would: the plan's "same-user reality" [read
    plan 0.14, Isolation].
  - Before running scripts the user did not write, the gate lists them with their hashes and asks once.
  - This is a `collide/plan` policy and needs no core change [inferred].

### 3.5 Cost display and caps

- **The preview shows each part's estimate:** each instance, then the merge, the verify step and any rounds. For a
  panel that is N × estimate + merge + verify, multiplied by the rounds for an adversarial pattern.
- **Per-instance caps reuse the role budget,** which is `--max-budget-usd` per process [read
  plan 0.3, v0 ships one adapter, Claude], [read plan 0.7, Example: the Orchard pack, `roles.builder.budget`].
- **A total cap per run is new.** The plan has per-role and per-phase budgets, but no run total [inferred].
  - At the cap, the run goes to `ask_human` with cause budget.
  - It is generic: an issue → PR → CI fix loop needs one too. So it is core.
- **An instance that hits its cap ends.** The merge goes on with N−1 instances and labels the missing one.

### 3.6 Several decisions at once

A merge can leave five disagreements. The first proposal brings back "several decisions in one human step" [read
prop: Core changes this implies, "Several decisions in one human step"]. Its deferred trigger is "a pack other than an
example solution needs one" [read plan: Deferred, More of Ferry's patterns in core]. `collide/plan` is not an example,
so the trigger would fire. I recommend not bringing it back yet:
- **Question items, a Queue lane and a `wait` phase do the job with existing mechanisms.** The run polls until
  every linked question is resolved.
- **The answers live in the source**, so there is one register.
- **A teammate can answer from another machine.** A Queue-only batch cannot do that.
- **Ferry's pack-only alternative** (a resolution file the person edits) is weaker for sharing [read
  `res:ferry.md`, "5.2 Pack-only gaps", row P9].

The cost is that the Queue must show the questions together, grouped by the run waiting on them. That is the Queue
lane in 8.1, row 3 (open question Q3).

## 4. The spike record format

This is a `collide/plan` format. It keeps every section that today's records have, adds a rich Orchard spike's
pre-registration, and makes claims machine-readable.

### 4.1 Rules

- **Frontmatter is the source's own.** The plan CLI rejects unknown keys [read the plan CLI's
  source], so Collide's structure goes in the body. Only the markdown provider's frontmatter is
  Collide's own.
- **Claims are fenced `claim` blocks of YAML.**
  - They render as code on GitHub, and they parse without a markdown parser.
  - One alternative was all claims in the frontmatter: long, and impossible with the plan CLI.
  - The other was a sidecar JSON file: two files that drift apart.
- **The pre-registration is written and committed before any run.** The results quote it unedited and name its
  commit.
- **Ids are stable:** `Q1`, then `Q1.c1` for each claim. All instances use the same ids, because the ids come from
  the pre-registration. That is what makes merging mechanical.

### 4.2 Frontmatter (markdown provider)

```yaml
---
id: sp-4k7q                     # random, never reused (2.4)
alias: S10
kind: spike
status: concluded               # proposed | preregistered | running | results | concluded | abandoned | superseded
title: Planning seams
question: Can three agents' records merge mostly mechanically?
owners: [lead]                  # the claim
session: ss-9d2m
links:
  - { type: informs, to: dc-2m8x }
  - { type: supersedes, to: sp-1a2b }
classification: measured        # the weakest label among load-bearing answers (the plan CLI's field)
checklist: collide/plan/spike@1
preregistered: 3f1c2a9          # commit of the Pre-registration section
instances: [opus-a, sonnet-b]   # a merged record only
spend: { usd: 0.85, cap_usd: 4 }
---
```

With the plan CLI, each key stays in the file whose schema declares it [read the plan CLI's source]:
- PLAN.md's frontmatter keeps `status`, `owners`, `question`, `supersedes` and `tags`;
- RESULTS.md's keeps `concluded`, `recommendation` and `classification`;
- the other keys go in a fenced `collide` block at the top of the body.

### 4.3 Body sections

The sections come in this order. Sections marked *new* are additions. The rest are today's, unchanged.

1. `# S10: <title>`, then the header bullets: Date, Versions, Reproduction, Method, Labels.
2. `## Pre-registration` *(new)*:
   - Q1…Qn, each with its prediction and what makes it red;
   - stop rules and the spend cap;
   - later amendments, appended as `### Amendment <date>` and never edited in place (as in that Orchard spike's rounds).
3. `## Summary`: | Q | answer | label | claims |.
4. `## Answers`: `### Q1. <question>`, then prose, claim blocks and the closing **Answer:** line.
5. `## Deviations` *(new)*: what changed from the pre-registration, and why.
6. `## Predictions against outcomes` *(new)*.
7. `## Proposed plan changes`: "None applied", cited by plan section.
8. `## Not measured`.
9. `## Spend`.
10. `## Left behind`.
11. `## Decision` *(new)*. The decide step writes it, not an agent:
    - who decided (`human:2026-11-13`);
    - which proposals were taken;
    - links to the decision items created.

A claim block:

````markdown
```claim
id: Q6.c2
label: measured                 # measured | inferred | unmeasured
says: A fresh session from a handoff note cost less than a cold resume at 73K of context.
value: 0.004
unit: usd
tolerance: 25%                  # for the verify gate; exact when absent
repro: .collide/planning/spikes/sp-0007-zf-tokens-context/06b-idle-resume.sh
output: out/06b-idle-resume.txt
cost_usd: 0.02
verifiable: no                  # yes | no: needs more than an hour idle
load_bearing: true
```
````

The value is S9's [read plan 0.3, `on_cold`]. The block itself is illustrative.

### 4.4 Merging

The script in 3.3 works on claim blocks only. It never compares prose mechanically.

| Case | Rule |
|---|---|
| Same claim id, both `measured`, values within tolerance | **agree** |
| Both `measured`, values outside tolerance | **disagree**: verify both, then open a question item if they still differ |
| One `measured`, the other `inferred` or `unmeasured` | **measured by one**: take the measured value if verify passes |
| Neither measured | **none measured**: keep the stronger label and list the claim in Not measured |
| A claim that only one instance made | **one only**: carried over, and marked |
| Different **Answer:** lines over agreeing claims | sent to the merger agent |

- **The merged record's `## Agreement` table** has one row per claim: | claim | opus-a | sonnet-b | case | verify |.
- **Instance records** live under the layout's code path, at `<code>/<alias or id>/instances/<name>.md` (2.4).
- **The merged record** is the spike's one record [inferred].
  - For Collide's own repo, that keeps today's `docs/decisions/<id>.md`, through `paths.spike` (2.4).
  - Elsewhere, it is the planning source's file for the spike.

## 5. Saved and shared sessions

### 5.1 The decisions, and the options they were chosen from

**Git-backed, with a live sync server deferred (owner, 2026-11-13).** For the record:

| Option | For | Against |
|---|---|---|
| **Git-backed** (chosen) | Works offline; reviewable in PRs; needs no server; fits ref-scoped sources | Not live; git conflicts; claims are advisory |
| A sync server | Live co-editing and presence | Auth, hosting and privacy work. The plan has no server and no REST API for now [read plan: Deferred, A local REST API]. |
| An export bundle | Simple | Copies drift apart; no merge |

**Where a session lives (owner, 2026-11-13):**
- outside the repo, in Collide's data directory, until Share;
- then, with the markdown provider, under its configured path (`.collide/planning/` by default; owner decision 5);
- or, with a team system, in that system's own paths, written through its own CLI.

| Option | For | Against |
|---|---|---|
| **Collide's data directory until Share** (chosen) | Nothing reaches the repo by accident; the same flow for every source; Share is one deliberate act | Share is a real move for team systems: new ids, and links rewritten |
| An unpushed local branch as the private place | No move for ref-scoped sources | Items land in the user's checkout early; it does not work for global sources; a branch is easy to push by mistake |

### 5.2 What a session is

Option (b) has no new core object kind. A session is an item of role `session` in the planning source, with its
items linked to it [inferred]:
- markdown provider: `kind: session`, a folder in the private root or under the configured path;
- the plan CLI: a Proposal, with its items in its corpus;
- beads: an epic, with its items as its children.

The session's runs are ordinary Collide runs on that item. They are process state, kept locally [read
plan 0.1, Collide owns process state; the task source owns task state].

### 5.3 Private until shared

Owner decision 4 confirms the first proposal on this point: a session starts in Collide's own store and moves into
the team's system on Share [read prop 3, Private until shared]. It has three consequences.

**1. Private planning items widen the scratch exception.**
- Scratch tasks are the one kind of task state Collide owns, and they "are never committed" [read
  plan 0.1, Scratch tasks are the one exception].
- A private session is the same kind of thing, except that it has files: records, scripts and a checklist.
- So the private root of the markdown provider (2.4) is a second instance of that exception: per user, per board,
  in `collide/plan`'s data directory.
- The spike runs' workspaces are already outside the user's checkout, under Collide's data directory [read
  plan 0.4, Implementations, chosen per workflow].

**2. Share is the deferred move op**, for every source [read plan: Deferred,
Moving a scratch task into a source]. Its trigger, "when a user wants a scratch task tracked by the team",
is now met for planning.

| Planning source | What Share writes | Committed by | Published by |
|---|---|---|---|
| markdown | each item, copied to the configured path (default `.collide/planning/<session>/`) | `record`, exactly those files [read plan 0.10, Committing] | the user's push, or a PR |
| plan CLI | `plan new adr\|spike\|question`, plus RESULTS.md and proposal files written by the provider, in its corpus | `record` | the user's push, or a PR |
| beads | `bd create`, `bd dep add` and close reasons, in `.beads/` | the export hooks of beads (unverified) | `bd dolt push`, by the user or a team hook (7.8) |

**3. Share fires `task.before_move` once for the whole session, then `task.after_move`** (7.4).
- A refusal leaves the whole session private. A session is never half shared [inferred].
- After Share, the session lives in the source. Later work by anyone, including the sharer, writes there.
- The private copy is removed once `record` succeeds [inferred].

### 5.4 What is shared and what stays private

| Shared (committed at Share) | Private (stays on the machine) |
|---|---|
| the session item and its linked items | transcripts, prompts, Claude session files |
| pre-registrations, records, `repro:` scripts | metrics and spans (0.13: "Nothing leaves the machine in v0." [read plan 0.13, Model]) |
| decisions with `human:<date>` | the user's profiles, aliases and model mappings |
| checklist state, with who marked each item | unsent drafts, personal views |
| a handoff note (5.5), reviewed before commit | per-person spend, unless the board opts in (Q5) |
| run summaries: workflow, outcome, timings, labels | |

- **The Share preview lists every file that will leave, as a diff, with the branch it will commit to.** Nothing goes
  until the user confirms [inferred].
- **Spend in run summaries is a privacy choice.** 0.13 keeps metrics on the machine, and "sharing metrics with a
  team" is deferred [read plan: Deferred, Metrics beyond one machine].
  - So spend is off by default, and a board setting turns it on (open question Q5).
  - The first proposal showed spend per person by default [read prop 3, A spike can be marked "claimed by" someone].

### 5.5 How a teammate continues

Claude sessions are local to each machine, so a teammate cannot `--resume` the sharer's session.
- **The note is built at Share, on the sharer's machine,** because only that machine has the transcript.
  - It reuses the `on_cold` builder, with no model call. The note holds the task, the files changed, the commands
    that kept failing, the open items and the last reply [read plan 0.3, `on_cold`].
  - The sharer reviews it in the Share preview, and it is committed with the session.
- **The teammate pulls.** The default line's provider reads every checkout [read plan 0.12, Tasks], so a
  shared session on the teammate's own branch appears at once. One on another branch appears once that branch is
  checked out as a worktree.
- **The teammate launches with `adopt`** at the session's next phase [read plan 0.7, Adopting]. The first agent
  starts fresh from the note.
- **This does not need lines** (Stage 14).
  - Pack code loads from a snapshot of the line's ref [read plan 0.7, Profiles], and until Stage 14 only the
    default line exists [read plan: the hooks, workspaces and record stage,
    Hook executor]. So a teammate's branch contributes data only, never code that loads.
  - That is also the safer default.
- **The note's quality is unmeasured.**
  - S9 measured only that a fresh session with a hand-written note was cheaper. "Whether a note keeps the work
    going as well as the session does is unmeasured" [read plan 0.3, `on_cold`].
  - The value of sharing rests on it, so S10 Q1 measures it.

### 5.6 A gap in the plan: items that exist only on a branch

- **The gap.** A session shared on a branch not yet merged has no stated place on the board. The board shows the
  default line's state, with divergence chips for items that changed [read plan 0.10, Ref-scoped sources].
- **The fix.** A "branch only" chip.
- **Why it is core.** The fix is generic: a build agent's follow-up tasks on its branch have the same problem
  [inferred].

### 5.7 Conflicts

- **Ref-scoped sources.**
  - One file per item, plus an append-only `history/` folder per session, keeps most merges clean.
  - Status and checklist edits to one item by two people conflict in git.
  - Conflicts found before `integrate` already go to the Queue [read plan 0.4, Syncing with the base].
  - A file left with conflict markers after a pull fails to parse. The provider counts it as a parse error, and
    `collide/plan` adds a Queue item to resolve it (7.9).
  - The plan CLI adds two more conflicts: per-checkout ids collide [read plan 0.10, Identity], and `plan generate`
    rewrites generated regions [inferred]. The markdown provider's random ids avoid the first.
- **beads.** Dolt merges at cell level, and `bd dolt pull` reports what it cannot merge [fetched `bd:faq`].
- **Writes.** The source's `version` check returns `conflict` on apply, as it does today [read plan 0.10, Protocol].

### 5.8 Claiming

- **A claim is `assignee` plus category `active`.** It uses the source's own claim where there is one: beads
  `--claim`, the plan CLI's `owners`.
- **Without a server, a claim is advisory.** Others see it only after a push and a fetch.
  - Two offline claims both succeed.
  - The later fetch shows a "claimed by two" chip, the way collisions are shown today [inferred;
    plan 0.10, Identity].
- **"One live run per task key" holds per machine only** [read plan 0.8, Run].

### 5.9 Trust of a teammate's work

- **Task text, records and the handoff note are untrusted data,** fenced in prompts [read plan 0.9, Task text is data].
- **Packs, profiles, config and views on a teammate's branch** load only once that branch is opened as a line, and
  a line has its own trust entry [read plan 0.12, What a line has]. Before Stage 14, they do not load at all.
- **Hooks from a teammate's branch** follow the same rule (7.10).
- **`repro:` scripts run only through the verify gate's explicit ask** (3.4).
- **A PR from the sharing branch is reviewed like any PR.** A diff touching `.collide/`, `.claude/` or a pack path
  is flagged [read plan 0.9, "A diff that touches"].

## 6. Checklists

### 6.1 Decision

**Agents suggest items, and only people confirm them (owner, 2026-11-13).** Two alternatives were considered:
- agents mark items done with evidence, and a person audits afterwards: faster, but a weaker check;
- people only, with no suggestions: slow, and evidence has to be found by hand.

### 6.2 Where it lives

| Option | For | Against |
|---|---|---|
| (a) A core feature: templates, state, gate and UI (the proposal [read prop 4, Checklists aren't planning-only]) | One implementation | Core holds checklist policy: templates, and which category to gate |
| (b) A `checklist` **field type** in the provider protocol, and the drawer in core. Templates and the gate in packs. | Mechanism in core, policy in packs; state stays in the source | A provider must store the field |
| (c) Pack only: state files and a gate, shown through `show:` | No core change | No drawer UI, which is clumsy for the owner's "during planning" use |

**Recommendation: (b)** (open question Q4). Its non-planning users are a PR checklist (GitHub task lists in the PR
body) and a release checklist, in the issue → PR → CI pack [inferred].

### 6.3 Templates and versions

- **Templates are defined in packs** (`checklists:` in `pack.yaml`) **or in the repo**
  (`.collide/checklists/*.yaml`). The repo's templates are config, so they are in the trust hash, as views are.
- **Profiles and views select a template** (`checklist: collide/plan/decision`). They do not define one, because a
  profile holds run settings [read plan 0.7, Profiles].
- **Each template has `version: <n>`.** An item pins `name@n`. A newer version is offered as a diff of added,
  removed and changed items, and is never applied without the person.

```yaml
# collide/plan checklists/decision.yaml
name: decision
version: 1
gates: done                     # the category the gate blocks; a spike template gates ready
items:
  - { id: alternatives, text: "Alternatives listed with pros and cons", required: true }
  - { id: evidence, text: "Every load-bearing claim labelled; measured ones verified", required: true,
      when: 'kind == "spike" || has(links.evidence)' }
  - { id: untrusted, text: "Untrusted input considered (0.9)", required: true, when: '"input" in labels' }
```

### 6.4 Item states

| State | Set by | Requires |
|---|---|---|
| `open` | the template | — |
| `suggested` | a `collide/plan` script phase, from an agent's output | the state suggested (done or n/a), and evidence: a link, a claim id or a file |
| `done` | a person | `by`, a date, and a link to a decision or evidence |
| `n/a` | a person | `by`, a date, and a reason |

- **`when:` is CEL over the item:** its kind, labels, fields and links. An item whose `when:` is false is hidden,
  not marked `n/a`.
- **Who may mark.** Any person who can write to the session's source. Each mark records `by`, and there is no
  separate approver [inferred].
- **How "only people" holds** [inferred]:
  - Agents have no Collide operations; they only write files.
  - In a ref-scoped source, the checklist state is a sidecar file (`<item>.checklist.yaml`). `protectedPaths` with
    `at: phase-end` on `**/*.checklist.yaml` fails any phase that edits one, wherever the configured path is [read
    plan: the workflow engine stage, Adopting]. Agents write their suggestions to `phase.dir`.
  - Before Share, the state is in the private root, which agents' workspaces do not contain.
  - In beads, an agent can run `bd` itself, so the guarantee is only as strong as the source. The drawer shows each
    mark's provenance: made in Collide's UI, or found in the source.

### 6.5 The gate

The gate is `collide/plan`'s checklist gate, bound to `task.before_status` at the template's category.
- It refuses while any required, visible item is `open` or `suggested`, and it names those items.
- The first proposal gates "ready" [read prop 4, A gate blocks "ready"]. For a decision or a session,
  `done` is the natural gate. A spike template gates `ready`, which is when its pre-registration is approved (7.2).
- A template that guards "ready to build" sets `gates: ready`.

### 6.6 The default list

The default list starts from the decision framework the owner uses: testability, readability, consistency,
simplicity and reversibility. It adds what this plan already checks:
- **Alternatives**, with pros and cons and a recommendation.
- **Evidence**: load-bearing claims are labelled, and measured ones are verified or marked `unverified`.
- **Core or pack**: would a pack that is not an example hit it (R4)?
- **Untrusted input** (0.9) and **privacy** (0.13).
- **Cost and caps.**
- **Migration and rollback.**
- **Observability**: which metrics show it working.
- **Dependencies**: new ones are named and asked about.
- **Deferred items** carry a trigger.
- **Owner-asked features** are kept and scoped, not cut.
- **Docs.**

Each item has a `when:`, so a small decision shows only a few of them.

## 7. Planning lifecycle hooks

The owner asked which hooks the planning lifecycle needs. This section starts from the lifecycle, then names each
point the lifecycle needs and fits it into the plan's catalogue (0.4) and hook protocol (0.5).

### 7.1 Three rules

1. **A role's state is `collide/plan`'s reading of the source, never a second copy.** "Recommended" or
   "pre-registered" is computed from the item's source status, fields and links. The source keeps the only state
   [read plan 0.1, Collide owns process state; the task source owns task state].
2. **Item state and run state are different.** A spike item has a few states in the source. While it is running,
   its run moves through phases: instances, merge, questions, verify, decide. Those are process state, and the
   catalogue already has points for them (`phase.*`, `run.*`).
3. **A point fires once, on the machine that makes the write** [proposal].
   - Collide fires points around the writes it makes: a UI write, a workflow op, Share.
   - It fires none for changes it only reads through `watch` [read plan 0.10, Protocol]: a pull, a teammate's push, a
     hand edit, or an agent running `bd` itself. Those reach observers as task-change events.
   - Otherwise every teammate who pulls would run the same reaction again [inferred].
   - The cost: a write made outside Collide skips its gates. `collide/plan` shows "changed outside Collide: gates
     not run" on an item whose category changed that way [proposal].

### 7.2 The lifecycles

Each table gives a role's states, the category each maps to, who or what enters it, and the points it fires.
`task.*_op: x` means both `task.before_op: x` and `task.after_op: x`; the same for `task.*_status`.

**Session** (role `session`):

| State | Category | Entered by | Points |
|---|---|---|---|
| draft | backlog | a person creates it (New planning session) | `task.*_op: create` |
| active | active | a person starts it, or its first spike is pre-registered | `task.*_status: active` |
| review | review | a person asks for sign-off | `task.*_status: review` |
| done | done | a person closes it. The session checklist gate must pass. | `task.*_status: done` |
| dropped | dropped | a person abandons it | `task.*_status: dropped` |

Place is separate from state. It goes from private to shared, one way, by Share (`task.before_move`,
`task.after_move`). A teammate who continues a session changes neither: they claim items (`task.*_op: claim`) and
start runs (`run.before_start`).

**Spike item** (role `spike`, in the source):

| State | Category | plan CLI | beads | Entered by | Points |
|---|---|---|---|---|---|
| proposed | backlog | `open`, no Pre-registration | `open` | a person, or a merge whose disagreement needs evidence | `task.*_op: create` |
| preregistered | ready | `open`, Pre-registration committed | `open`, design field set | a person approves the pre-registration at a `human` phase | `task.before_status: ready`, where the spike checklist gates |
| running | active | `running` | `in_progress` | the spike workflow's `on_start` op | `task.*_status: active` |
| results | review | `running`, with RESULTS.md | `in_progress`, with notes | the workflow's `on_end` op, after verify | `task.*_status: review` |
| concluded | done | `concluded` | `closed` | a person, at the `decide` phase | `task.before_status: done`: the verify verdict and the checklist |
| abandoned | dropped | `abandoned` | `closed`, with a reason | a person | `task.*_status: dropped` |
| superseded | dropped | `superseded` | `closed`, `supersedes` link | a later spike's conclusion | `task.*_op: supersede` |

The plan CLI and beads columns are [inferred]. The plan CLI's `proposed` and `preregistered` are both `open`, and its
`results` stays `running`, so for the plan CLI those pairs share a category. A gate bound to a category is only as
precise as the source's statuses.

**Spike run** (process state; the phases of `collide/plan`'s spike workflows):

| Phase | Kind | Ends with | Points |
|---|---|---|---|
| `prereg` | `agent` or a person writes it; then `human` approves | approved: the item goes to `preregistered`. Changes asked: round 2. | `phase.*`; the approval's op fires `task.before_status: ready` |
| `instances` | `fanout` with `instances:` (panel, specialists), or `agent` (one agent, replicate) | each instance's record in its own `phase.dir` | `phase.before_start`, `phase.after_end`, per instance (unverified) |
| `merge` | `script` | `merge-table.md` | `phase.*` |
| `merger` | `agent` | prose where the table shows no agreement | `phase.*` |
| `questions` | `script` that creates question items, then `wait` | every linked question resolved | `task.*_op: create` per question; `phase.*` |
| `verify` | `script` gate | `ok`, `fail` or `unverified` | `phase.*` |
| `challenge` (optional) | `agent` | findings: back to `instances`, or on | `phase.*` |
| `decide` | `human`, with `show:` on the record | `conclude`, `rerun` or `abandon` | the chosen op fires `task.before_status: done` or `dropped` |

- An instance over its cap ends, and the merge goes on with N−1 (3.5).
- Verify `fail` goes back to `instances` for the failing questions, or to `ask_human`.
- Verify `unverified` holds the run in the Queue: the person retries, proceeds or stops [read
  plan 0.5, `unverified`].

**Decision** (role `decision`):

| State | Category | Entered by | Points |
|---|---|---|---|
| proposed | ready | a person, a spike's Proposed plan changes, or a merge | `task.*_op: create` |
| recommended | review | a spike's decide step or a person links a recommendation (`recommends`, with the option) | `task.*_op: link` |
| accepted | done | a person decides, with `by: human:<date>` and the option chosen | `task.*_status: done` |
| rejected | dropped | a person decides against | `task.*_status: dropped` |
| superseded | dropped | a later accepted decision links `supersedes` | `task.*_op: supersede` |
| deferred | backlog | a person parks it, with its trigger | `task.*_status: backlog` |
| back | ready | a person returns it when the trigger fires | `task.*_status: ready`, from `backlog` |

- Rejected and superseded share a category. The derived points in 7.5 tell them apart by op, never by a source's
  status name.
- Triggers stay prose, as in the plan's Deferred list. A "Deferred" view lists them. Machine-checked triggers are
  not proposed.

**Question** (role `question`):

| State | Category | Entered by | Points |
|---|---|---|---|
| open, needs a person | ready | a person, a merge disagreement, or a record's Not measured | `task.*_op: create` |
| open, needs evidence | ready | the same, with `needs: evidence` | `task.*_op: create` |
| resolved | done | a person answers (`resolved_by: human:<date>`), or a concluded spike resolves it | `task.*_op: resolve`, then `task.*_status: done` |
| dropped | dropped | a person | `task.*_status: dropped` |

A `wait` phase polls its linked questions, so a resolve needs no point to wake it.

**Checklist item** (a field of an item, not an item):

| State | Entered by | Points |
|---|---|---|
| open | the template | — |
| hidden | its `when:` is false | none: it is computed |
| suggested | a `collide/plan` script phase copies an agent's suggestion, with its evidence | `task.*_op: setChecklist`, actor workflow |
| done | a person | `task.*_op: setChecklist`, actor person |
| n/a | a person, with a reason | `task.*_op: setChecklist`, actor person |
| open again | a person rejects a suggestion or reopens an item, or a new template version changes it | `task.*_op: setChecklist` |

The checklist gate sits on the owning item's `task.before_status`, at the template's `gates:` category (6.5).

### 7.3 What the plan's hooks already define

- **Names.** Points are `object.before_x` and `object.after_x`, in one catalogue that Stage 2 defines in full, so a
  pack can attach anywhere [read plan 0.4, Named points]. Adding a point is a schema
  change [read plan 0.4, Named points]. **Packs cannot declare points.**
- **Chain handlers** attach to any point and run in pack order. Each sees the outputs of the handlers before it at
  the same point [read plan 0.4, Chain].
- **Blocking.** At a `before_*` point, `refused` or `error` stops later handlers and the action. At an `after_*`
  point, it stops later handlers and fails the step [read plan 0.4, Named points].
- **Gates** attach to a workflow's phases or to a named point [read plan 0.4, Gates]. The plan shows one binding
  form, `task.before_status: review`, and no other syntax for binding a hook to a point.
- **Results**: `ok`, `fail`, `unverified`, `refused` or `error`, in `$COLLIDE_RESULT`. A non-zero exit or a missing
  file is `error` [read plan 0.5, Result]. At a named point, `unverified` holds the run in the Queue [read
  plan 0.5, `unverified`].
- **Payload**: JSON on stdin, argv only, task text never templated [read plan 0.5, argv only, never a shell string],
  [read plan 0.5, Input].
- **State.** Outputs become run state, one entry per invocation, keyed by `pack/hook` plus phase, round and
  instance [read plan 0.5, State]. Every hook receives `state: { run, workspace, point }` [read
  plan 0.5, State]. Outputs are capped at 64 KB and may have a schema [read plan 0.5, State]. State is
  process state, not a register [read plan 0.5, State].
- **cwd.** `task.before_status` runs in the task's latest workspace if it still exists, otherwise at the line root
  [read plan 0.4, Named points].
- **Timeouts** are declared per hook, as the workspace hooks show (`timeout: 120m`) [read
  plan 0.7, Example: the Orchard pack, `workspaces.worktree`]. Stopping is SIGTERM, a 60 s grace,
  then SIGKILL [read plan 0.5, Order]. The plan sets no default timeout for point hooks [unverified].
- **Every handler gets `pack` and `workflow`,** so it can ignore other packs' runs [read plan 0.4, Named points].
- **Observers** cannot block or change state [read plan 0.4, Observers], are listed in the trust prompt with
  their events [read plan 0.4, Observers], and cannot act on Collide in v0 [read plan 0.4, Observers].

**What planning needs that this does not give** [inferred]:
1. **No `after` point for task writes.** A reaction to a status change outside a run can only be an observer, which
   cannot fail visibly and has no order.
2. **No points for other ops**: create, claim, link, resolve, supersede, setChecklist.
3. **Writes outside a run.** 0.5 puts outputs in run state, and `unverified` holds "the run". A UI write or a Share
   has no run.
4. **An op other than `setStatus` can change a status** (the plan CLI's `resolve`). `task.before_status` must know the
   target category before the op runs, hence op effects (2.2).
5. **Repeated filters.** Every binding would repeat "an ADR, or a beads decision, or a markdown decision". A pack
   cannot name the moment once.
6. **The change itself.** The 0.5 input has no field for what is changing: from, to, by whom.

### 7.4 Core points

| Point | Status | Fires when | Matches | May block? | What blocking means | Run state |
|---|---|---|---|---|---|---|
| `task.before_status` | exists | before a write that moves an item to a category: from the UI, a workflow op, an op with that effect (2.2), or Share | category | **yes**: gates and chain hooks | The write is not applied. The UI shows each refusal with its message ("checklist: alternatives, evidence open"). In a run, the step fails by the workflow's rules. | In a run: reads `state.run`, writes a run-state entry. Outside: reads `latestRun` (new, read-only); outputs go to the write's event entry. |
| `task.after_status` | **new** | after the source returned `ok` for that write | category | no: chain hooks only | `refused` or `error` skips later handlers. In a run, the step fails. Outside, it becomes a Queue item, and the write stays. | as above |
| `task.before_op` | **new** | before any op other than `setStatus` | op name | **yes** | The op is not applied. For `claim`, nobody is assigned. | as above |
| `task.after_op` | **new** | after that op returned `ok` | op name | no | as `task.after_status` | as above |
| `task.before_move` | **new** | once per Share, before anything is written; `tasks[]` holds every item that moves | target source | **yes** | Nothing moves. The whole session stays private, and the preview shows each refusal. | reads `state.point`; outputs go to the Share's event entry |
| `task.after_move` | **new** | after every item was written and `record` committed | target source | no | A Queue item. The items stay shared. | as above |
| `run.before_start` | exists | a run starts, including an adopted continuation | — | yes | The run does not start. | run state |
| `phase.before_start`, `phase.after_end` | exist | each phase | — | `before_start`: yes | The phase does not start; the workflow's failure rules apply. | run state |
| `run.after_end` | exists | on every outcome [read plan 0.4, Lifecycle order] | — | no | as any `after_*` point: later handlers are skipped and the step fails [read plan 0.4, Named points] | run state |

Rules [proposal]:
- **Order for one op:** `task.before_op`, then `task.before_status` if the op moves a category, then the write, then
  `task.after_status`, then `task.after_op`. A `setStatus` fires only the status points.
- **Gates bind only to `before_*` points.** A gate bound to an `after_*` point is refused at load, because it cannot
  block anything. The plan does not say either way.
- **`unverified` outside a run** holds the write in the Queue. The person retries the hook, writes anyway, or
  cancels: the three choices 0.5 gives a run.
- **Timeouts.** Each binding takes `timeout:`, as workspace hooks do. A person waits on a UI write, so the default
  is 30 s for `task.*` points and 120 s for `task.before_move`, which may scan files. A timeout is `error`, and the
  hook is stopped in 0.5's order.
- **cwd.** Task points keep `task.before_status`'s rule. Move points run at the root of the line Share commits to.
- **Accepting a run is the model.** Share is a user action with its own points, as accepting a finished run is [read
  plan 0.4, Lifecycle order]. It is not a run.
- **Non-planning users.** A GitHub pack refuses a comment that carries a secret (`task.before_op: comment`), labels
  a new issue (`task.after_op: create`), and moves a scratch task into GitHub through the move points.

### 7.5 Points that `collide/plan` declares

The plan does not let packs declare points (7.3). Core must allow it in a narrow form.

| Option | For | Against |
|---|---|---|
| (a) None: team packs bind core points with their own `when:` | No new mechanism | Every binding repeats role filters and source status names. A pack that reacts to "a decision was made" must know every source. |
| (b) **Derived points**: a pack declares a named point as a core point plus a filter | Planning words stay in `collide/plan`. Teams attach once, by name. Core evaluates CEL, which it already has [read plan 0.7, Workflow expressions]. No point fires that core does not already fire. | One more manifest key. The trust prompt must show what each derived point expands to. |
| (c) Pack-fired points: a pack's script raises a point | Any moment can be a point | A hook would need a channel into core, and no token reaches a hook's environment [read plan 0.5, Reporting]. Transitions would hide in scripts, against the declared transition table [read plan 0.7, A workflow is a declared transition table]. |

**Recommendation: (b)** (open question Q9). A non-planning pack benefits too: GitHub's `github/issue.before_close`
is `task.before_status: done` filtered to issues.

Shape [proposal]:

```yaml
# collide/plan pack.yaml (proposal)
points:
  decision.before_decide:
    on: task.before_status
    match: [done, dropped]
    when: 'task.kind in config.roles.decision && change.op != "supersede"'
```

- The name is namespaced: `collide/plan/decision.before_decide`.
- `when:` is CEL over `task`, `change` and the declaring pack's `config`. `config.roles` is the kind → role mapping
  (open question Q2).
- Binding to a derived point binds the core point with both filters joined by AND.
- A `before_*` derived point must come from a `before_*` core point, and an `after_*` one from an `after_*` point.

The points `collide/plan` declares:

| Derived point | Expands to | Typical attachment |
|---|---|---|
| `session.before_share` | `task.before_move` | a secret scan of the files in the preview |
| `session.after_share` | `task.after_move` | `bd dolt push`; open a PR; post the link |
| `session.before_continue` | `run.before_start`, when the launch adopts a run on a session's item | refuse when someone else holds the claim |
| `session.before_close` | `task.before_status: done`, session kinds | the session checklist gate |
| `spike.before_preregister` | `task.before_status: ready`, spike kinds | the spike checklist gate |
| `spike.before_claim`, `spike.after_claim`, `spike.after_unclaim` | `task.before_op: claim`, `task.after_op: claim`, `task.after_op: unclaim`, spike kinds | beads: pull, then check the assignee; post who runs it |
| `spike.after_merge` | `phase.after_end`, phase `merge` of a `collide/plan` spike workflow | count disagreements for a team dashboard |
| `spike.after_verify` | `phase.after_end`, phase `verify` | — |
| `spike.before_conclude`, `spike.after_conclude` | `task.before_status: done`, `task.after_status: done`, spike kinds | the verify-verdict gate; mirror results |
| `decision.after_propose` | `task.after_op: create`, decision kinds | tell the deciders |
| `decision.after_recommend` | `task.after_op: link`, where `change.args.type == "recommends"` | — (the "Decisions needed" lane is a view, not a hook) |
| `decision.before_decide`, `decision.after_decide` | `task.before_status`, `task.after_status`: `done` or `dropped`, decision kinds, not `supersede` | an evidence rule as a gate; an ADR mirror; a tracker comment |
| `decision.after_supersede` | `task.after_op: supersede` | update a decision log |
| `decision.after_defer`, `decision.after_return` | `task.after_status: backlog`; `task.after_status: ready` from `backlog` | — |
| `question.after_raise` | `task.after_op: create`, question kinds | open a tracker issue for `needs: human` |
| `question.before_resolve`, `question.after_resolve` | `task.before_status`, `task.after_status`: `done`, question kinds | require `resolved_by` |
| `checklist.before_mark`, `checklist.after_mark` | `task.before_op`, `task.after_op`: `setChecklist`, actor person | require a reviewer group for a security item |
| `checklist.after_suggest` | `task.after_op: setChecklist`, actor workflow, state `suggested` | tell the session's owner |

`collide/plan` binds three gates of its own: the checklist gate, the verify-verdict gate on `spike.before_conclude`
(it refuses unless the latest run's verify verdict is `ok`, or a person proceeded past `unverified`), and the
session close gate. Outside a run, they read `latestRun`.

### 7.6 Payload

Every point gets the 0.5 input [read plan 0.5, Input]. Task points add `change`, and outside a run they add
`latestRun` [proposal]. An example for `collide/plan/decision.after_decide`, from the drawer:

```json
{
  "protocol": 1,
  "hook": "team/adr-mirror",
  "pack": "team",
  "point": "collide/plan/decision.after_decide",
  "board": "b-41c2",
  "line": "main",
  "experiment": null,
  "run": null,
  "launch": null,
  "config": { "adrDir": "docs/adr" },
  "tasks": [
    { "key": "plan:dc-2m8x", "shortId": "dc-2m8x", "path": ".collide/planning/ss-9d2m/dc-2m8x-sharing.md",
      "kind": "decision", "status": "accepted", "category": "done", "version": "9f2c41",
      "links": [{ "type": "evidence", "to": "sp-4k7q" }], "title": "Sharing is git-backed" }
  ],
  "change": {
    "op": "setStatus",
    "args": { "status": "accepted", "option": "git" },
    "from": { "status": "recommended", "category": "review" },
    "to": { "status": "accepted", "category": "done" },
    "actor": { "kind": "person" },
    "checkout": "main",
    "files": [".collide/planning/ss-9d2m/dc-2m8x-sharing.md"]
  },
  "latestRun": { "id": "r-77", "workflow": "collide/plan/spike", "outcome": "concluded", "state": {} },
  "phase": null,
  "workspace": null,
  "history": [],
  "state": { "run": null, "workspace": null, "point": {} }
}
```

- **Data, never instructions.** The payload reaches a hook only on stdin, as JSON. No field reaches argv, env or a
  template [read plan 0.5, argv only, never a shell string]. `title` and every other text field are untrusted task text
  [read plan 0.9, Task text is data].
  - A hook that hands text to an agent fences it with its provenance, as prompts do.
  - A hook that posts text to a tracker passes it in a file or on stdin, never in argv.
  - `collide/plan`'s SDK offers both helpers [proposal].
- **Fields and paths, not bodies.** Records are long. The payload carries fields and paths, and a hook reads the
  file at `path` from its cwd. This keeps stdin small [proposal].
- **`actor`** says person or workflow. The decider's name stays in the item's own field (`by: human:<date>`), so the
  payload carries no identity [proposal].
- **`latestRun`** is the item's latest run, frozen and read-only. It is how a gate outside a run reads a verify
  verdict. The plan rules out board-wide and cross-run state [read plan 0.5, State]. `latestRun` keeps to
  that: it is one finished run's frozen record, which a hook can read but never write.

### 7.7 Provider operations, hooks and observers

**The rule** [inferred]:
- The planning source's own items change only through provider ops: `apply`, with its version check [read
  plan 0.10, Protocol], committed by `record` [read plan 0.10, Committing].
- A hook never edits those items' files directly. That would skip the version check and the gates.
- A reaction that touches anything else is a **hook** when it must block or show its failure, and an **observer**
  when it is best effort.

| Example | What it is | Why |
|---|---|---|
| Create an ADR in the plan CLI when a decision is proposed (`plan new adr`) | provider op | In the plan CLI, the ADR is the decision item. |
| Accept it (`plan set`); resolve a question (`plan resolve`, `human:<date>`) | provider op | Status writes in the register. |
| Write RESULTS.md, then run `plan check` | provider op | The spike item's write, validated by the system itself. |
| `bd create -t decision`, `bd close --reason`, `bd update --claim`, `bd human respond` | provider op | beads' own writes. |
| Write an item under the configured path | provider op | The markdown provider's register. |
| Refuse "accepted" while an evidence spike is open | none for the plan CLI: `apply` → `refused` (corrected, S11) | The plan CLI's write gate runs every rule, including the one that an accepted ADR needs its evidence spikes concluded (severity error) [read the plan CLI's source]. Every write that would accept an ADR with an open evidence spike is refused, whoever writes it. It is not checked only in `plan check`. A pack's `adr-evidence` gate would duplicate it. A hook gate is right for a rule the backend does not hold, such as the markdown provider's. |
| Write a MADR file to `docs/adr/` when the planning source is the markdown provider | hook: chain, a mirror | Not the register. Better: point the provider's path at the ADR folder (2.4), so no mirror is needed. |
| Comment on a GitHub issue or a tracker ticket | hook: chain, or observer | A hook when a failure must show; an observer when best effort. |
| Post to the team's chat | observer | It cannot block. Delivery is at least once, deduped by sequence number [read plan 0.4, Observers]. |
| `bd dolt push` after Share | hook: chain, on `session.after_share` | A team's publishing choice, not the write. |
| Orchard's agent registry | observer | The plan's first real observer [read plan 0.4, Observers]. |

The first proposal listed "writing an ADR" as a chain hook [read prop 1,
New named points in the hook catalogue]. Where the team's ADRs are its register,
that is a provider op. A hook is right only for a mirror into a place Collide does not read.

**Cross-source reactions** (added, S11). "A GitHub issue closed, so close the beads issue" is a reaction across
sources. Observers cannot act in v0 [read plan 0.4, Observers], and a hook runs only around a write Collide or its
run makes. Until acting observers exist (plan, Deferred), use beads' own tracker sync, or accept that the second
close is made by hand.

### 7.8 Worked attachments (examples only)

Each attaches through the same points, and core names none of them. Binding syntax is a proposal: the plan shows
only `task.before_status: review`. `observers` with `events:` and `record.paths` are the plan's own.

**The plan CLI** (the Orchard pack, an example solution). Provider ops: create through `plan new adr|spike|question`;
decide through `plan set` or `plan supersede`; resolve through `plan resolve` (flags unverified); conclude by
writing RESULTS.md, then `plan set`. At Share, `plan new` allocates new ids, and the links are rewritten (2.6).

```yaml
# orchard pack.yaml: example only
# No adr-evidence gate (corrected, S11): the plan CLI's own write gate refuses an accepted ADR
# while an evidence spike is open (7.7), so `apply` → `refused` already covers it.
at:
  - point: collide/plan/spike.before_conclude
    gate: gates/results-check.ts      # ./plan check over the RESULTS.md the provider wrote
observers:
  registry: { script: observers/registry.ts, events: ["task.*", "run.*"] }
```

**beads** (an example provider, not shipped). Provider ops: `bd create -t decision|spike|epic`,
`bd update --claim`, `bd close --reason`, `bd human respond`, `bd dep add`.

```yaml
# example beads pack: example only
at:
  - point: collide/plan/spike.before_claim
    gate: gates/claim-check.ts        # bd dolt pull, then bd show --json: refuse if someone else holds it
    timeout: 60s
  - point: collide/plan/session.after_share
    run: hooks/publish.ts             # bd dolt push; error → Queue item "pull first"; the share stands
observers:
  outside-writes: { script: observers/outside.ts, events: ["task.changed"] }   # writes agents made with bd
```

**The built-in markdown provider** (a team with no planning system). Provider ops write under the configured path.

```yaml
# a team's own pack in its repo: example only
at:
  - point: collide/plan/session.before_share
    gate: gates/secret-scan.ts        # scans change.files; refuses with the file and line
    timeout: 120s
  - point: collide/plan/decision.after_decide
    run: hooks/adr-mirror.ts          # writes docs/adr/NNNN-<slug>.md
  - point: collide/plan/question.after_raise
    run: hooks/open-issue.ts          # gh issue create --body-file, for needs: human
record: { paths: ["docs/adr/**"] }    # this pack's record commits only these [read plan 0.4, Contribute]
```

### 7.9 Sharing

| Moment | Point | Typical attachment | Blocking means |
|---|---|---|---|
| Share | `task.before_move` (`session.before_share`), `task.after_move` (`session.after_share`) | a secret scan; `bd dolt push`; open a PR | Nothing moves; the session stays private. |
| Open a teammate's session | none: reading fires nothing | — | — |
| Continue it | `run.before_start` (`session.before_continue`) | refuse, or ask, when someone else holds the claim | The run does not start. |
| Claim, unclaim | `task.*_op: claim`, `task.*_op: unclaim` (`spike.*_claim`) | beads: pull, then check; post who runs it | Nobody is assigned. |
| Conflict on pull | none | — | — |

- **Why a conflict on pull has no point.** The pull (`git pull`, `bd dolt pull`) happens outside Collide. Collide
  sees its result through `watch`. A file with conflict markers fails to parse; the provider counts it as a parse
  error, and `collide/plan` adds a Queue item to resolve it. Two claims show "claimed by two" (5.8). A gate would
  have nothing left to stop, and a reaction would run on every machine that pulls (7.1).
- **Automatic merging** of checklist files, if a team wants it, is a git merge driver: git config, outside Collide
  [inferred].

### 7.10 Trust of hooks from a teammate's branch

- **Hooks are pack code,** loaded from a snapshot of a line's ref [read plan 0.7, Profiles], and a line's snapshot
  includes its gates and hooks [read plan 0.12, What a line has]. Until Stage 14 only the default line exists [read
  plan: the hooks, workspaces and record stage, Hook executor].
  - So a hook added on a teammate's branch runs on my machine only after the branch merges into my default line and
    I accept the trust prompt. The trust hash covers the packs and `config.yaml` [read plan 0.9, Trust].
  - Opening that branch as a line, later, gives it its own trust entry, with a diff [read plan 0.12, What a line has].
- **The trust prompt lists each binding:** its pack, point and script, and for a derived point the core point and
  filter it expands to [proposal]. It already lists observers and their events [read plan 0.4, Observers].
- **The planning root is data.** A planning file cannot declare a hook, a gate or an observer: its schema has no
  such keys [proposal].
- **`.collide/config.yaml` (the planning path) and `.collide/checklists/` are in the trust hash,** so a change to
  either asks again.
- **Agent-side hooks are passed through** [read plan 0.4, "Agent-side config"], for example the `.claude/` settings that
  `bd setup claude` writes. The run preview lists them with flags [read plan 0.11,
  Launch shows a run preview before anything starts], and a diff that touches `.claude/` is flagged in review [read plan
  0.9, "A diff that touches"].
- **A user pack's hooks run only for that user.** Teammates' gates can differ, so a gate is a team convenience, not a
  boundary between teammates. Review is the boundary [inferred].

## 8. Core changes

### 8.1 Each with the non-planning pack that also benefits

| # | Change | Planning use | Non-planning beneficiary | Stage |
|---|---|---|---|---|
| 1 | `kind`, with per-kind statuses, transitions and ops in `initialize`; **op effects** (the category an op moves to) | ADR vs question vs spike; `resolve` fires `task.before_status` | GitHub issues vs PRs; "close as not planned"; beads types | 2, 4 |
| 2 | Typed `links`, with inverses derived | evidence, supersedes, informs, recommends | GitHub sub-issues, duplicates, "closes" | 2, 4 |
| 3 | **A Queue lane fed by a view**: read-only, answered through source ops | "Decisions needed" | GitHub "review requested from me" | 2, 9 |
| 4 | `assignee`, with `claim` and `unclaim` ops | "claimed by" | "assigned to me"; not launching a teammate's issue | 2, 4 |
| 5 | A `checklist` field type, and the drawer UI | the planning checklist | PR and release checklists | 2, 9 |
| 6 | **Fan-out instances**: a model, definition, profile or prompt per instance | panel, specialists | a review step with a security reviewer and a correctness reviewer | 2, 8 |
| 7 | A run-level total budget, which stops to `ask_human` | multi-agent caps | fix loops in issue → PR → CI | 2, 8 |
| 8 | Linked items in the prompt context, fenced as untrusted and capped in size | decided items feeding a spike | a PR phase reading the linked issue | 2, 8 |
| 9 | **Task points** `task.after_status`, `task.before_op`, `task.after_op`, `task.before_move`, `task.after_move`; `when:` on bindings; `change` and `latestRun` in the payload; outputs and `unverified` outside a run; default timeouts; gates only at `before_*` | every lifecycle reaction (7.4) | label a new issue; refuse a comment with a secret; scratch → GitHub | 2, 8, 9, 12 |
| 10 | **Derived points** (`points:` in `pack.yaml`) | `collide/plan`'s named moments (7.5) | `github/issue.before_close` | 2 |
| 11 | **The move op**: from Collide's store to a source | Share, for every planning source | the deferred "moving a scratch task into a source" | 12 |
| 12 | Items that exist only on a non-default checkout, with a chip | a session shared on a branch | follow-up tasks a build agent created | 6 |
| 13 | `views:` in `pack.yaml`, so packs ship default views | the planning views | the GitHub pack's "my PRs" | 4 |

### 8.2 The first proposal's changes, against this note

| Proposal | Verdict | Why |
|---|---|---|
| Hook points "a decision recorded, a spike finished, a session shared" (its section 1) | **modify** → rows 9 and 10 | Generic task points in core; the planning names are derived points in `collide/plan` (7.5). |
| 1. A source adds a Queue item | **modify** → row 3 | The source keeps the state, and the Queue reads a view. |
| 2. Typed links | **accept** → row 2 | |
| 3. Several decisions in one human step | **not now** | Question items, the Queue lane and a `wait` phase do it (3.6). |
| 4. Linked decided tasks in the prompt | **accept** → row 8 | |
| 5. Per-instance fan-out, adversarial rounds, a merge phase | **modify** → row 6 | Adversarial rounds and fan-out join already exist [read plan 0.7, Example: the Orchard pack, `workflows.loop`], [read plan: the workflow engine stage, Tests]. |
| 6. Moving a private item into a source | **accept** → row 11 | Confirmed by owner decision 4, for every source. |
| 7. Checklists with a gate | **modify** → row 5 | The field type goes in core, and the gate uses `task.before_status`. |
| 8. Roles declared, with fallback to the repo | **modify** → row 1 and 2.3 | Kinds in core, roles in the pack, no silent fallback. |

## 9. Staging

Smallest first, into the existing stages. There is no new stage, and each piece is small enough to land with its
stage. A spike, S10, runs at Stage 11's existing "first, outline Milestone C" step [read plan: the group runs stage,
First, outline Milestone C].

| Order | Stage | Piece | Owner requirement |
|---|---|---|---|
| 1 | 2 | Contracts: rows 1–10 of 8.1, in the schema and the catalogues, with every new point and the `points:` key | all |
| 2 | 4 | The markdown provider (both roots, the layout setting), kinds, op effects, links, the record parser and the planning views. Orchard's plan provider reads ADRs, spikes and questions. | 1 |
| 3 | 6 | A read-only planning view; links both ways; checklists shown read-only; the branch-only chip | 1, 4 |
| 4 | 8 | Fan-out instances, the run total budget, linked items in prompts; task points fired from workflow ops; derived points; the checklist gate (with fakes) | 2, 4, hooks |
| 5 | 9 | The one-agent spike workflow, in a private session; task points fired from UI writes, with outputs and `unverified` outside a run; the Queue lane; checklist marking and suggestions; default templates | 1, 3 (saved), 4, hooks |
| 6 | 11 | S10, then multi-agent workflows, the merge, the verify gate and the cost preview | 2 |
| 7 | 12 | Share (the move op and its points), the handoff note at Share, claims, template versions; path Move; the beads example provider and its attachments | 3 (shared), 4, hooks |

**What requirement 3 costs.** Shared sessions are the most expensive requirement and land last. They need:
- the Share preview;
- the move op, with its points and its id mapping for team systems;
- a handoff note built at Share;
- adopting a run into a teammate's checkout;
- branch-only items;
- conflict handling;
- an example provider with a different scope.

Requirement 3 is split, so part of it arrives early:
- **Saved** sessions arrive in Stage 9, because a private session in the data directory survives restarts.
- **Shared** sessions arrive in Stage 12.

## 10. Open questions for the owner

1. **Roles a source lacks.**
   - (a) Fall back to repo markdown per role, as the first proposal does. Every feature works everywhere, but the
     register is split silently.
   - (b) One planning source per board. Missing roles are off, with a reason. One register, but fewer features.
   - (c) (b), plus mapping onto a kind the source has, plus an opt-in, badged fallback per role.
   - **Recommendation: (c).**
2. **Where the kind → role mapping lives.**
   - (a) The provider tags its kinds (`plan.decision`). Setup is automatic, but planning words cross the protocol
     as opaque strings.
   - (b) Board config, in `collide/plan`'s settings (CEL per role). No strings in the protocol, but every board is
     mapped by hand.
   - (c) Both: the provider's tags are the default, and the board overrides them.
   - **Recommendation: (c).** (b) comes first, in Stage 4, and tags come in Stage 12 with the beads example.
3. **Several decisions at once.**
   - (a) Bring back the batched human step now.
   - (b) Question items in the source, a Queue lane and a `wait` phase. One register, and teammates can answer.
   - (c) A resolution file that the person edits (Ferry's pack-only way).
   - **Recommendation: (b).** (a) stays deferred, with its trigger.
4. **How much of checklists is core.**
   - (a) All of it, as the first proposal says. One implementation, but checklist policy sits in core.
   - (b) The field type and the drawer in core; templates and the gate in packs.
   - (c) Pack only. No core change, but no drawer UI.
   - **Recommendation: (b).**
5. **Spend in shared run summaries.**
   - (a) On by default, and listed in the Share preview. Useful for knowing what planning costs, but metrics leave
     the machine by default, against 0.13.
   - (b) Off by default, and a board setting turns totals on. This keeps 0.13.
   - (c) Never shared.
   - **Recommendation: (b).**
6. **Ids in the markdown provider.**
   - (a) Random short ids plus an optional alias (beads' way). No collisions across branches or teammates, no
     renumbering at Share, and files can move, but the ids are less readable.
   - (b) Sequential ids, with a lock and a renumber at merge (the plan CLI's way). Readable, but they collide and need
     renumbering.
   - (c) Date-and-slug file names. Readable and mostly unique, but long, and renames break links.
   - **Recommendation: (a).** People keep saying "S10" through `alias`.
7. **Spike S10 before Milestone C.**
   - (a) Run it, with a $3 cap. It answers three questions:
     - Q1: on a second clone with another HOME, can a fresh agent working from a note built at Share finish a
       spike's open items as the original session would?
     - Q2: are fan-out instances independent in files, `phase.dir` and Claude's auto memory?
     - Q3: how many claims merge mechanically across two haiku records?
   - (b) Skip it, and learn from the Stage 11 and 12 tests. Cheaper now, but the value of sharing rests on Q1.
   - **Recommendation: (a).**
8. **Collide's own planning records.**
   - (a) When Stage 4 lands, point the provider at `docs/decisions/` and `spikes/` (owner decision 5), add
     frontmatter to the five records, and keep Open questions and Deferred in the plan.
   - (b) Also move Open questions and Deferred into items. One system, but it changes how the plan reads.
   - (c) Leave Collide's records as they are. Nothing changes, but Collide does not use its own planning.
   - **Recommendation: (a).** Revisit (b) once the planning view is in use.
9. **Points that a pack declares.**
   - (a) None: team packs bind core points with their own filters. No new mechanism, but every binding repeats role
     filters and source status names.
   - (b) Derived points: a core point plus a CEL filter, declared and named by a pack. Planning words stay in the
     pack, but it is one more manifest key.
   - (c) Pack-fired points. Any moment can be a point, but hooks would need a channel into core, and transitions
     would hide in scripts.
   - **Recommendation: (b).**
10. **The planning path's setting, and a change to it.**
    - Where: (a) `collide/plan`'s setting in `.collide/config.yaml`, repo value wins, not overridable; (b)
      `board.yaml`'s source configuration. (a) has a per-user layer before Share with team, and core never parses a
      pack's settings.
    - A change: (a) offer Move and keep reading old paths; (b) refuse while shared items exist; (c) move every file
      at once.
    - **Recommendation: (a) for both.**
11. **Points for changes made outside Collide** (a pull, a hand edit, an agent's `bd`).
    - (a) None: observers only, and a chip when a category changed outside Collide. A reaction runs once, but gates
      do not see those changes.
    - (b) Fire `after_*` points on every machine that sees the change. Simple, but every teammate runs the same
      reaction.
    - (c) Fire them on one chosen machine. Needs a coordinator, which means a server.
    - **Recommendation: (a).**

## 11. Draft plan text (proposal, not applied)

> **Proposal only. `IMPLEMENTATION_PLAN.md` was not edited.** Each block names the section it would go under, and
> assumes the recommendations in section 10.

**0.1 Principles**, after "Mechanism in core, policy in packs":
- **Planning is a pack** (owner, 2026-11-13).
  - `collide/plan` is built in, like `collide`.
  - It owns the planning roles (decision, spike, question, deferred, session), the record format, the spike
    workflows, the checklists, the planning views and the planning points.
  - Core knows kinds, links, checklist fields and generic task points, never roles.
  - beads is a third example solution (owner, 2026-11-13). It tests the provider seam with a global source.
- To "Scratch tasks are the one exception", add: **private planning sessions** are the same exception (owner,
  2026-11-13).
  - They live in `collide/plan`'s data directory, private to the user and the board, until Share.
  - Share moves them into the planning source.

**0.4 Extension model**, named points:
- The task row becomes: `task.before_status`, `task.after_status`, `task.before_op`, `task.after_op`,
  `task.before_move`, `task.after_move`.
  - Status points match a category. Op points match an op name. Move points match the target source.
  - For one op: `task.before_op`, `task.before_status` if the op moves a category, the write, `task.after_status`,
    `task.after_op`. `setStatus` fires only the status points.
  - Move points fire once per Share, with every item in `tasks[]`. A refusal moves nothing.
  - Points fire only around writes Collide makes. Changes read through `watch` reach observers only.
- A binding may add `when:`, CEL over `task`, `change` and the pack's `config`.
- Gates bind only to `before_*` points.
- **Derived points.** A pack may declare `points:`: a name in its namespace, one core point, what it matches, and a
  `when:`. Binding to it binds the core point with both filters. A derived point never fires on its own. The trust
  prompt shows what each binding expands to.
- **Outside a run** (a UI write, Share): outputs are kept with the write's event, `unverified` holds the write in
  the Queue, and an `after_*` failure is a Queue item; the write stays.

**0.5 Hook protocol**, input:
- Task points add `change: { op, args, from, to, actor, checkout, files }` and, outside a run, `latestRun` (the
  item's latest run, frozen, read-only). `tasks[]` carries fields and paths, not bodies.
- Point bindings take `timeout:`. The default is 30 s for task points and 120 s for move points. A timeout is
  `error`.

**0.7 Pack manifest**:
- Phase kinds: `fanout` takes one role on several models, or `instances:`. Each instance names any of a model, an
  agent definition, a profile and a prompt. Two instances that are identical in every respect are refused at
  launch.
- A run takes `budget: { usd }` as a total. At the total, the run goes to `ask_human` with cause budget.
- `views:`, `checklists:` and `points:` are pack keys. A board adopts a pack's views by name.
- `collide/plan`'s `layout` setting (`root`, `paths.<kind>`, `code`, `previous`) is not `overridable`.

**0.8 Data model**, Task cache:
- Add `kind`, `links` (`type`, `to`, `blocking`), `assignee`, and checklist fields.

**0.10 Task sources**, protocol:
- `initialize` returns `kinds`, each with its statuses (with categories), its transitions, its ops (each with the
  category it moves to, if any) and optional tags. A provider with one kind may omit `kinds`.
- It also returns `links`: the link types it holds, their direction, and whether they block.
- **A built-in markdown provider** in `collide/plan`:
  - one file per item, identified by its frontmatter `id`, with random ids and an alias;
  - two roots: private, in the pack's data directory, and shared, at the configured path, `.collide/planning/` by
    default, committed as views are (owner, 2026-11-13);
  - the path is set in `.collide/config.yaml`. A change offers Move, and the old path stays readable until it is
    empty;
  - the shared root is data, outside the trust hash;
  - it is the planning source for a repo with no system.
- **A board has one planning source.** A role that source lacks either maps onto a kind it has, or is off with a
  reason. If the board opts in, it is held in markdown, with a badge.
- **Share** is the move op. The markdown provider copies items to its configured path. A team system's provider
  writes through that system's CLI into that system's own paths (owner, 2026-11-13). `record` commits exactly those
  files.
- Items that exist only on a non-default checkout show with a "branch only" chip.

**0.11 Interface direction**, Queue:
- A Queue lane can be fed by a view (`queue: true` on the view). Its items are answered through the source's ops.
  `collide/plan` ships "Decisions needed".
- **Planning view** (`collide/plan`): decisions made, spikes outstanding, results with their summary tables, and
  decisions needed.
- **Share preview:** every file that will leave, the branch it commits to, the handoff note to review, and the
  planning path setting if it is not yet shared.

**0.13 Metrics**, privacy:
- Sharing a planning session commits run summaries without spend, unless the board sets `share.spend: totals`.
  Transcripts, prompts and metrics never leave the machine.

**Stage 1**, after S9:
- **S10 planning seams.** It runs at Stage 11's outline step, with a $3 cap.
  - Q1: a fresh agent working from a note built at Share, on a second clone, against the resumed session.
  - Q2: fan-out instance independence (files, `phase.dir`, auto memory).
  - Q3: the rate of mechanical merging across two records.

**Stage 2**, bullets:
- The provider schema's `kinds`, op effects, `links`, `assignee` and `checklist` field type; fan-out `instances`;
  the run budget; the five new task points, `change` and `latestRun`; `points:`, `views:` and `checklists:` in
  `pack.yaml`.
- **Success:** a provider with one kind and no links validates unchanged. `collide schema` lists every new key and
  point. A derived point that names an unknown core point, or a gate bound to an `after_*` point, is refused at
  load.

**Stage 4**, bullets:
- The markdown provider, with both roots and the layout setting; kinds, op effects and links in the corpus; the
  record parser (claim blocks) in `collide/plan`; pack views. The Orchard pack's plan provider reads ADRs, spikes
  and questions.
- **Success Criteria:**
  - Collide's five records, with frontmatter added and `paths.spike: docs/decisions`, list as concluded spikes in
    `collide tasks --view spikes --json`.
  - Over the Orchard repo, the Decisions needed view lists the same question ids as the human queue in
    `plan status --format json`.
  - An ADR's `evidence` link shows from the spike's side as "evidence for".
  - A malformed claim block is a counted parse error on the view.
  - Two branches that each create ten items in the shared root merge with no id collision.
  - A private-root item never appears in `git status`.
  - A layout path outside the repo or under `.git/` is refused with the reason.
- **Tests:** parser fixtures, including strict plan CLI frontmatter left untouched; provider contract tests against
  markdown and plan fixtures; id generation; view counts; layout checks.

**Stage 6**, bullets:
- A read-only planning view; links both ways in the drawer; checklists shown read-only; the branch-only chip.
- **Success:** a session shared on a branch, checked out as a worktree, shows its items with the chip. The
  planning view's counts match the CLI's.

**Stage 8**, bullets:
- Fan-out instances; the run total budget; linked items in the prompt context, fenced; task points fired from
  workflow ops; derived points; the checklist gate on `task.before_status`.
- **Success Criteria:**
  - Three instances with different prompts run in separate workspaces, and the next phase sees all three
    artifacts.
  - Two identical instances are refused.
  - A run over its total goes to `ask_human` with cause budget.
  - A status write to `done` with a required item still open is refused, and names the item.
  - A workflow op `resolve` fires `task.before_op: resolve`, then `task.before_status: done`, then the `after`
    points, in that order.
  - A hook bound to a derived point runs only when both filters match.
- **Tests:** engine tables for instances; the budget stop; point order; derived-point expansion; gate binding with
  `when:`.

**Stage 9**, bullets:
- `collide/plan/spike`, in a private session:
  - pre-register, with a `human` phase that approves the pre-registration;
  - run;
  - verify;
  - decide, with a `human` phase whose `show:` is the record;
  - write back through the source's ops.
- Task points fired from UI writes: outputs kept with the event, `unverified` holding the write in the Queue, an
  `after_*` failure as a Queue item. The Queue lane from a view. Checklist marking in the drawer, with agent
  suggestions shown with their evidence.
- **Success Criteria:**
  - A one-agent spike in a private session ends with the spike concluded and a decision item with `human:<date>`.
    The record's Decision section is written by the decide step. Nothing appears in `git status`.
  - The session survives an app and daemon restart and reopens where it stopped.
  - An agent phase that edits a checklist state file fails at phase end.
  - A suggested item does not satisfy the gate until a person marks it.
  - A gate that times out on a drawer write leaves the write unapplied, with the hook's name in the message.
  - An `after_status` hook that fails after a drawer write leaves the write in place and adds a Queue item.
- **Tests:** the workflow fixture end to end with the fake adapter; the Queue lane reducer; `protectedPaths` on
  checklist state; point timeouts and failures outside a run.

**Stage 11**, bullets:
- After S10, in `collide/plan`:
  - panel, specialist and adversarial spike workflows;
  - the merge script and the merger phase;
  - disagreements as question items, with a `wait` phase;
  - the verify gate, with tolerance, sampling, a verify budget, and the ask before running teammates' scripts;
  - the cost preview.
- **Success Criteria:**
  - A two-instance panel on fixture records produces a merge table whose cases match a hand-made table.
  - A changed number in one instance becomes a question item, and the run waits until it is resolved.
  - A `repro:` whose output drifts outside tolerance fails the gate. One that needs an hour idle is `unverified`.
  - The preview's estimate shows instances, merge and verify separately.
- **Tests:** the merge cases, one per row of the merge table; verify verdicts; `wait` on linked questions.

**Stage 12**, bullets:
- Share:
  - the Share preview;
  - the move op and its points;
  - the handoff note built at Share;
  - `record` of exactly the written files;
  - a teammate continuing by `adopt`;
  - claims;
  - conflicts to the Queue.
- Checklist template versions. Move for a changed planning path.
- **The beads example provider**, as a fixture pack like codemap, with the attachments of the note's 7.8. It covers a
  global scope, a category taken from a close reason, and writes that Collide did not make.
- **Success Criteria:**
  - Sharing a private session with the markdown provider adds exactly the session's files under the configured
    path, in one commit, and the private copy is gone.
  - Sharing into the plan fixture creates the items through `./plan`. The session's links point at the new ids.
  - On a second clone with another HOME, a teammate adopts a shared session, and a fresh agent starts from the
    committed note.
  - The Share preview lists no transcript, prompt or metric file.
  - Two offline claims on one spike show "claimed by two" after both fetch.
  - A refusal on `task.before_move` leaves the whole session private.
  - A pull that brings a changed decision fires no point; observers get the change, and the item shows the
    outside-change chip if its category moved.
  - After the planning path changes, items at the old path still list, and Move commits the renames with the config
    change in one commit.
  - A newer template version is offered as a diff and changes nothing until it is accepted.
- **Tests:** Share preview contents; the move op for markdown, plan and beads fixtures; adopt from a note; claim
  collision; move-op points; the path Move; beads fixture contract tests.

**Deferred**:
- "More of Ferry's patterns in core": add that the batched human step stays deferred while question items and a
  `wait` phase serve planning.
- "Moving a scratch task into a source": remove it from Deferred. It becomes Stage 12's Share, for planning sources
  (owner, 2026-11-13).
- Add: **points for changes made outside Collide**, when a team needs a reaction that the acting machine cannot
  run.

**Open questions**: questions 1–11 of this note's section 10.

## Where this note differs from the first proposal

1. **No silent fallback to repo markdown.** It splits one planning record across two systems (2.3).
2. **Kinds in core, roles in the pack.** A provider that declares "roles" puts planning vocabulary into core. Kinds
   are generic (2.1, 2.2).
3. **No planning-named hook points in core.** Generic task points go in core, and `collide/plan` names the planning
   moments as derived points, which core must allow (7.4, 7.5).
4. **Writing an ADR is usually a provider op, not a hook.** Where ADRs are the team's register, a hook that writes
   them skips the version check and the gates. A hook fits only a mirror (7.7).
5. **beads is not branch-scoped today.** It is Dolt-backed and syncs through `refs/dolt/data`. So "both are
   git-based" holds only for the plan CLI, and Share has to be the provider's act (1.3, 5.3).
6. **Adversarial rounds and the merge phase already exist.** Only per-instance fan-out is new (3.1, 3.3).
7. **Batched decisions stay deferred.** Question items keep one register, and teammates can answer them (3.6).
8. **The Queue reads a view.** A source does not push Queue items (8.1, row 3).
9. **Checklists are a field type in core,** with templates and the gate in packs (6).
   - Templates come from packs or the repo, not from profiles.
   - The gate's category is set per template.
10. **The verify gate must handle three things:** nondeterminism, its own spend, and teammates' code (3.4).
11. **Spend per person is off by default,** because of 0.13 (5.4).
12. **Claims are advisory without a server** (5.8).
13. **The handoff note is built at Share on the sharer's machine.** Its quality is unmeasured, so S10 measures it
    (5.5).
14. **Staging differs in two ways** (9).
    - Multi-agent support belongs with the engine (Stage 8), not with group runs, which are about many tasks.
    - Sharing does not need lines.
15. **The proposal leaves out some core changes:** `kind`, op effects, `assignee`, branch-only items, pack views and
    a run total budget (8.1).
16. **Share for team systems has costs the proposal does not name.** Ids are allocated again, links are rewritten,
    and the plan CLI has no writer for RESULTS.md or proposals (2.6, 5.3).

The first proposal's "private until shared, in Collide's own store" is confirmed by owner decision 4. This note had
weighed an unpushed local branch as the private place, and records it as the alternative not taken (5.1).

## Not read or not verified

- **beads.** I read only its public docs. I did not read:
  - the CLI reference past 200,000 characters;
  - formulas and molecules;
  - its JSON shapes;
  - what its git hooks do, and whether they export on commit;
  - the exact flag for setting `deferred`.

  I know the older JSONL-in-git model only from search snippets. Nothing was installed or run.
- **The plan CLI.**
  - Whether bug #4 (piped JSON cut at 64 KiB) is fixed on every path.
  - The flags of `plan resolve` and `plan supersede`.
  - The Orchard repo's review folders, which I did not read.
- **The plan.**
  - Whether point-bound gates take `when:`.
  - Whether every fan-out instance gets its own directory by default.
  - Whether `phase.*` points fire once per fan-out instance or once per phase.
  - The plan's syntax for binding a hook to a named point, beyond `task.before_status: review`.
  - A default timeout for point hooks: the plan sets none that I found.
  - The plan was being edited (part G) while this note was written. Citations are for the 3282-line version.
- **Claude.** Whether auto memory is shared between two worktrees of one repo (S10 Q2).
- **Unmeasured.**
  - Every category mapping in 2.6 and 7.2.
  - That one file per item keeps merges clean.
  - The merge rate that the record format allows (S10 Q3).
