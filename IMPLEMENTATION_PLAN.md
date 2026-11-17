# Collide — Implementation Plan

Task-oriented desktop IDE: pick tasks from any source, run agents on them in isolated workspaces through
pack-defined workflows. Rust + Tauri + React. v0 runs Claude Code only; the design keeps other agent tools and
model sources (OpenAI, Ollama, …) addable without core changes. The one exception is the ask box (0.11), which also
takes an ACP agent or an external MCP client.

Claims are labelled **measured** (observed, with the observation named), **inferred** or **unmeasured**.

**Status: revision 23.** Revision 23 makes task statuses data: each source declares its own, any names and any
number, the six categories under them stay fixed, and board columns follow a source's own statuses (owner,
2026-11-15). Revision 22 lets a pack say where its agents start (the repository root by default for agents that make
their own worktrees), notices those worktrees and tracks them under their run until their work lands, lets each
workflow say whether Collide or the repository lands its work, and puts every repository and branch in one line
selector, with a Lines control on Tracks and the Queue and one Up next list per repository. When Collide starts
again after a long close, it checks everything it keeps against the world before anything acts on it (owner,
2026-11-15). Revision 21 adds Up next, an ordered list of tasks and groups that start one after another, tells an
agent about the group it works in, and loads a repository's own agent instructions from the run's worktree, with a
warning when a run changes them (owner, 2026-11-15). Revision 20 gives Tracks two layouts, List and Timeline, makes
the free-space floor a setting, adds repo-only settings keys, and puts every file Collide writes on macOS in one
folder (owner, 2026-11-15). Revision 19 keeps the tab names Field, Tracks, Queue and Planning, each with a plain
subtitle (owner, 2026-11-15). Revision 18 adds planning quality checks from the Field mockup: a Decision log, an
Assumption ledger, a coverage map, a kill criterion for spikes, a red-team pass and a session budget meter (owner,
2026-11-15). Revision 17 picks shadcn/ui and sets the interface's audience: product managers as well as engineers
(owner, 2026-11-15). Revision 16 shows Claude's versions as two chips in place of a Queue warning (owner,
2026-11-15). Revision 15 takes two findings from the tokensafe research, and revision 14 puts features in proposals,
both from the owner's decisions of 2026-11-14 (Review log). R5 is applied (Review log; ledger at the end of the item
`ts-0006-ze` in `.collide/planning/epics/ep-001-ef-spikes/tasks/`). S1–S9 and S11 are applied; S10 runs when the
group-runs stage starts. The owner accepted revision 13 (2026-11-14); Stage 2 is in progress.

Revision 18 (2026-11-15) adopts the planning checks the owner reviewed as ideas on the Field mockup (A). A proposal
gains a Decision log, with the alternatives each decision passed on and its evidence, an Assumption ledger and a
Findings part. Collide derives a coverage map from features and `implements` links. A spike's pre-registration gains
a kill criterion. A red-team pass, with a pre-mortem mode, reads a draft and writes nothing, and a Plan… session
shows a budget meter that warns and never stops. A Readiness checklist gathers them as advice: Collide still refuses
`proposed` only for a problem in the Features part. A panel of agents on one spike waits for Stage 15 (0.10, 0.11,
Stages 4c, 9d and 15).

Revision 17 (2026-11-15) picks shadcn/ui, on Radix and Tailwind, for the app, after the owner compared it with MUI
on the Tracks mockup, and sets who the interface is for: Collide is a higher-level development environment, so its
default look is approachable to a product manager, and an engineer's detail stays one step away (0.11, 0.15).

Revision 16 (2026-11-15) shows Claude's versions as two chips, the `claude` in use and the latest published, on the
Agents page, in the run preview and in the thin loop's window. A version outside the tested range that passes the
smoke check marks its chip and raises no Queue item; a failed smoke check still refuses the phase (0.3, Stages 3b,
3c, 6b and 9a).

Revision 15 (2026-11-14) takes two findings from the tokensafe research (`docs/research/tokensafe.md`):
- The run preview flags a redirected model endpoint (`ANTHROPIC_BASE_URL`, from the captured environment or a
  `settings.json` `env`, which survives Collide's strip of `CLAUDE_*`), and user hooks by the event they hook,
  because a `SessionStart` or `UserPromptSubmit` hook can add text to the context and a `PostToolUse` hook can
  replace a tool's result. The fingerprint records the endpoint's host (0.3, 0.13, Stages 3b and 9a).
- A saving that a reporter claims is an estimate, never subtracted from measured totals (0.5, Stage 7).

Revision 14 (2026-11-14) puts features in proposals. The owner's request: planning is about features, and the
Planning view should group them into sections that the agent names.
- A proposal's body has a fixed `## Features` part that Collide reads. `###` headings are sections, and each entry
  is `- **F<n> Title**: summary`. An entry under `### Unsorted`, or under no heading, has an empty section. The move
  to `proposed` is refused while the part has a problem (0.10).
- A task links to the feature it builds with `implements`, whose target carries the feature: `pr-0003-km#F2`. A
  feature's state is derived, from its proposal's status and then from its tasks (0.8, 0.10).
- Sections are matched by heading name across proposals, with no registry and no feature kind. `collide sections`
  lists the names in use. The Plan… prompt reuses them first. The Planning view flags near-duplicates and offers to
  merge them, and its Sort button has the ask agent suggest sections as chips the person applies (0.10, 0.11).
- Stage 2 gains the contracts and the slice's part: the `proposal` kind in the slice, the Features part and its
  check, the feature link and the derived state, and `collide sections`. The ops that write features are Stage 4c's,
  the read-only Features view is Stage 6a's, and Sort, merging and Plan…'s naming are Stage 9d's.

Revision 13 (2026-11-14) applies R5, the adversarial review of revisions 10–12 by Fable and Opus (both verdicts:
ready after fixes), and the owner's decisions on it:
- Contracts Stage 2 freezes, now stated before it starts (0.2, 0.3, 0.4, 0.5, 0.7, 0.10, 0.11, Stage 2):
  - session replay is one ordered shim journal, with a storable timestamp and the journal written before the child
    sees the bytes (0.2, 0.3);
  - the `wait` hook can answer `pending` with `retryAfter`, and a `wait` phase has `interval:` and `timeout:` (0.5,
    0.7);
  - an actor is read from peer credentials, only the app does person-only operations, the socket directory is `0700`
    with a uid check, and the same-user guardrail is stated as what it is (0.2, 0.10, 0.11);
  - a PR integrate answers `merged` or `pending`, with an `integrating` state, and side-effect steps write an intent
    and an outcome event, so a restart checks the world and never retries blind (0.4, 0.5);
  - the endpoint's shell answers `initialize`, `ping` and `tools/list` from a build-time manifest, forwards only
    `tools/call` as `ui_tools.request`, and cancels asks when the window closes or reloads (0.2);
  - provider writes hold a per-root lock and are atomic, the slice reads only `epics/`, and built-in packs ship
    compiled in the bundle (0.6, 0.10, Stage 2);
  - Stage 2's items are brought to revision 12, and the `spike` kind and its statuses are defined (0.10).
- Fixes placed in the stage that builds each one (B1–B23 in the Review log): webview CSP and sanitized markdown, a
  paged `query`, the ask line's `--add-dir`, `--permission-prompts none` and a `--restricted` measurement, an opencode
  probe, a realpath read rule, a Claude version range with a settings canary, a full-disk path, a clock table, an OTLP
  receiver in the daemon, an interactive-launch MCP rule, an opencode listener check, an ask cancelled with its
  connection, `git:` pack fetch rules, hook-own report lines as the only primary source, ACP `configOptions` first,
  named protected files, caps on CEL-bound fields only, repo budget and timeout defaults, id collision tests and
  `collide task reid`, walkthrough records, the contradictions fixed, and a daemon start that survives SSH.
- Stages are split into lettered sub-stages, with no renumbering: Stage 3 (3a–3d, with the thin loop as 3c), Stage 4
  (4a–4c), Stage 6 (6a–6b, then Projects v2 as 6c) and Stage 9 (9a–9d). Each has its own goal, criteria and Status,
  and Status names the plan revision its items came from. Only the next stage is split into items.
- Owner decisions on R5's challenges (2026-11-14):
  - a thin loop (prompt → `collide/quick` run → review → accept, in the app, against scratch tasks) follows the Claude
    adapter as Stage 3c, before the ACP engine (3d), the plan adapter, GitHub and the board (Milestone A, Stage 3c);
  - GitHub Projects v2 moves from Stage 5 to Stage 6c, after the board; Stage 5 keeps Issues, and Stage 2 still
    freezes the provider contract in full;
  - the endpoint stays in the app shell. The stdio bridge and the daemon-hosted endpoint were considered and not
    chosen (0.2);
  - rejected: a three-letter id suffix, the markdown provider in Rust, and named per-client endpoint tokens;
  - deferred: a Collide-managed copy of Claude (Deferred).
- The product will be built by Sonnet agents (owner, 2026-11-14). Each Stage 2+ item is sized to be finished by one
  such agent in one session from the item file alone (0.15).

Revision 12 (2026-11-14) adds the in-app agent, from S6's in-app agent section
(`.collide/planning/spikes/sp-0009-bb-interface-prototype/record.md`) and the owner's design and decisions:
- An agent in the UI changes the UI through tool calls. It does the analysis with its own read-only tools, then calls
  five generic tools (`query`, `set_view`, `show_groups`, `highlight`, `open`). The UI knows nothing about code
  files, and no tool launches, accepts, edits or deletes in v0 (0.11).
- A **view** is typed, live and saveable as a View. A **lens** is the agent's timestamped judgement, with a reason per
  group, and is saved only as a snapshot. Every call is a typed reducer action with a snapshot Undo, and the chip,
  not the agent's prose, says what changed (0.11, 0.15).
- The ask agent is a separate role from run agents. Run agents still have no access to Collide. The ask agent's
  launch line is strict (`config: none`, `--tools Read,Grep,Glob`, `--no-session-persistence`), with a per-agent
  opt-in "use my Claude config". The default is Claude Code with haiku (0.3).
- The UI-tools endpoint lives in the app process, which owns the window. It needs a token from day one: one for
  external clients, shown and rotated in Settings, and one per ask for the agents Collide launches. Where it lives is
  inferred, and R5 checks it (0.2).
- Agent entries are Collide's own settings in the same layers, with a visible source per field, secrets by name from
  the keychain, readable folders and tool kinds as data, a test action, and snippets for external clients generated
  from the real endpoint (0.7).
- Agent tool inputs are untrusted: unknown ids are rejected and nothing is applied, groups are limited, and the error
  tells the agent what to do (0.9).
- A budget is optional and off by default, and Claude Code is the only agent that can enforce it. Every agent has a
  timeout (0.3, 0.7).
- Work lands in Stages 2, 4, 6, 9 and 11. A real-agent contract test (opt-in) and an owner walkthrough grade it
  (Stages 4, 6, 9, 11). Tools that act, and the cases the record left unmeasured, are deferred with triggers
  (Deferred, Risks).

Revision 11 (2026-11-14) applies spikes S5–S8 (`.collide/planning/spikes/`) and the owner's decisions on them:
- Schemas: Rust types → `schemars` → JSON Schema → `json-schema-to-typescript`, a Node build step. YAML is read by
  `serde-saphyr` 2.x with a hardened option set in Rust, and by the `yaml` package in core mode in TypeScript. Tagged
  enums use `Wait {}`, not a unit variant (0.6, 0.7, 0.9, 0.15; S5).
- Expressions: the `cel` crate has no cost limit. Collide vendors 0.15 with a step counter and offers it upstream,
  and adds per-task and per-pass budgets, a 1 KB source cap, list caps at load, `catch_unwind` around compile, no
  corpus variable in views and a static cost estimate when a view is saved or loaded (0.9, 0.10, 0.15; S5).
- `AgentAdapter` is two traits: a stateless adapter and a sans-IO `AgentSession` per child. The session id is an
  event, permission answers are Allow, Deny or Cancel, usage says how complete it is, capabilities are Yes, No or
  Unknown, and an empty turn is a failed phase. `model.id` is opaque beside `model.name`. An ACP adapter is an engine
  plus a profile in `host`, with no `fs`, `terminal` or `elicitation` client capability in v0 (0.2, 0.3, 0.11, 0.13,
  Deferred, Stage 2, Stage 3; S7).
- S6's sessions were skipped, because the owner has no participant who did not design Collide. Every criterion that
  named a non-owner participant now asks for S6's scripted pilots ported to UI tests against the real app, plus an
  owner walkthrough recorded as owner judgement. S6's time and error limits stay recorded and are advisory. The
  open questions take their defaults (run identity `id`, Queue keys `literal`, `o` opens the `moment`, the line
  switcher in the `top` bar), and "review ×2" means two reviewers in parallel. Waits are spans, a breakdown gives
  three time totals, the context view is a data contract, colour never identifies a run, the Queue has one keys
  table with undo, and an experiment result states its scope (0.11, 0.13, Stage 1, Stages 2, 6, 9, 11, 15; S6).
- Metrics sources: sampling at 1 s, a run's lane containers always counted through the Docker API, a streamed
  `nettop`, the GPU polled at 5 s only when a local model is declared, a 2 s sleep threshold, about 34 bytes per
  sample row, report-file caps, telemetry traces for headless phases that need exact timing, and three breakdown
  choices. The sleep test and a run measured from its start are deferred until before Stage 13 (0.5, 0.11, 0.13,
  Deferred, Stage 13; S8).
- Not taken: S5's id rule `^[a-z]{2,}-[0-9a-z]{4,}$`, replaced by the owner's id rule of 2026-11-13 (0.10), and S6's
  usability gate with a first-time user. The owner chose S8's GPU rule over revision 10's: the device is polled
  only when a local model is declared (Open questions, item 8). The ledger of every recommendation is at the end of
  the item `ts-0005-st` in `.collide/planning/epics/ep-001-ef-spikes/tasks/`.

Revision 10 (2026-11-13) applies spikes S1–S4 and S9 (`.collide/planning/spikes/`), two research notes
(`docs/research/`) and the owner's decisions on them:
- The process owner is `collide-daemon` with a per-child shim. The daemon starts on demand, outside the app's
  coalition (0.2).
- The Claude launch line is fixed: settings from the snapshot, auto memory off, scheduling tools disallowed (0.3).
- Packs load by git tree id, and the SDK is linked from a versioned cache (0.6, 0.7).
- Collide works with no pack and no tasks: the built-in `collide` pack, scratch tasks, New session (0.4, 0.10, 0.11).
- Metrics work with no pack: token and context metrics, exit totals and disk always on, and an opt-in import of
  past sessions (0.13).
- Context options per role, cold-cache handling, and a token-strategy experiment preset (0.3, 0.7, 0.14).
- Packs come from the repo, any git URL or a folder in development, each pinned to a git tree id. A pack built for
  a team or a repo needs only a commit, never publishing. A pack is the repo's or the user's, packs switch per board,
  and profiles bundle a setup for a kind of job (0.6, 0.7, 0.9).
- Ferry joins Orchard as an example solution. It adds `agentWork` script phases, an `unverified` result and `show:`
  on human steps (0.1, 0.3, 0.5, 0.7).
- S2 measured interactive sessions: a take-over detaches when its view closes and survives app and daemon restarts;
  telemetry measures interactive sessions and `agentWork` phases; the app stays in the menu bar with its window
  closed (0.2, 0.3, 0.11, 0.13).
- S9 measured tokens and context: usage counted once per response with per-turn totals from `result.usage`,
  compactions from `compact_boundary`, a 100K floor on `compact_at`, separate Bash and MCP output caps, measured
  start weights, and hook-blocked prompts read from the stream (0.3, 0.7, 0.13, 0.14).
- From Ferry's runner (owner; `docs/research/ferry-runner-metrics.md`): model time per turn; a metric its
  source did not give is stored as missing, never as 0; spend read from error results and other model calls;
  `is_error` checked on `result/success`; one cache hit-rate formula; changed-file lists that collapse dependency
  and build folders (0.3, 0.5, 0.11, 0.13, 0.14).
- S9's addendum measured hook and permission outcomes: refused tool calls classified from `tool_result_meta`,
  hook failures read from `--include-hook-events`, and two more detectors (0.3, 0.13).
- Planning is a built-in pack, `collide/plan` (owner, 2026-11-13; `docs/research/planning.md`). Core gains item
  kinds, op effects, typed links, `owners`, a checklist field, five task points and derived points. A built-in
  markdown provider keeps shared planning at a path set per repo. Spikes can run several agents, a session is
  private until Share, and spike S10 checks the seams (0.1, 0.4, 0.5, 0.7, 0.8, 0.10, 0.11, 0.13).
- S11 checked the point catalogue and the `change` payload against five backends (owner, 2026-11-13;
  `.collide/planning/spikes/sp-0008-bk-derived-point-coverage/record.md`). Points also fire around writes a run's
  agent makes, from the run's task diff at accept or from attribution to a phase. Accept checks `done` gates before
  the merge. `change` gains a diff, a cause, an actor and batch and effect fields, and tasks carry their links one
  hop out. Outside changes get one event shape. Collide's PRs carry no closing keywords (0.4, 0.5, 0.10, Deferred,
  the spikes, skeleton, workflow-engine, run UI, group-runs and second-pack stages).
- Three interface mockups the owner built (Field-first, Queue-first, Tracks-first) settled three things: the Field
  is home, the Queue is a list with a detail pane, and Tracks use a real-time axis. S6 keeps five open questions
  from them (0.11, the spikes stage).
- Part K (owner, 2026-11-13): an agent-output pane, and a tool-call time breakdown filtered by tool type, minimum
  duration and time window (0.11, 0.13, S6, the run UI stage); `collide task <id> --json` (the corpus stage); read
  access for hooks and agents, and a GitHub App identity for agents, deferred with triggers (Deferred).
- Part L (owner, 2026-11-13): Collide is built with Collide. Planning comes first: its contracts lead the skeleton,
  and the skeleton stage ends with a working planning slice, so Collide's own build tasks live in its markdown
  provider. Until then they are written by hand in the same format. Queue items gain a context view built with no
  model call, and the time window matches calls that overlap it. An adversarial review, R5, follows revision 11 and
  comes before any product code (0.1, 0.10, 0.11, Deferred, the spikes, skeleton, headless-core, corpus and run UI
  stages).
- Part M (owner, 2026-11-13): the markdown provider's layout and ids. A planning session is now a proposal, and
  decisions live in proposals and questions, so there are no decision or deferred files. Items sit in
  `epics/<epic>/` folders, which give the epic, with spikes at the top level. Ids are a number plus a letter suffix,
  so branches never renumber. Collide's spike records and their scripts move into `.collide/planning/spikes/` (0.1,
  0.4, 0.10, 0.13, the spikes, skeleton, corpus, task-board, run UI, Orchard-loop and second-pack stages).
- Part N (owner, 2026-11-14): planning with an agent. "Plan…" on the Planning view, an epic or a task starts a New
  session on a new private proposal. The agent writes items through the CLI, and a choice to make away from the
  terminal becomes a question in "Decisions needed". Proposals get their statuses, and only a person accepts,
  defers, rejects or resolves (0.10, 0.11, run UI stage).

Revision 9 (2026-11-13) applied the R4 fixes (Fable + Opus) and the owner's answers to R4's challenges:
- Workflows are a declared transition table, with `script`, `wait` and `human` phases, and each workflow names its
  own workspace and integrate implementations (0.4, 0.7).
- Experiment verdicts come only from a randomized, balanced crossover with at least 8 pairs. Every other design
  gives effect sizes (0.14).
- Pack settings: a `config` schema, values from the user's file and the repo's shared file (the repo wins), secrets
  from the keychain, and a data directory per pack (0.7).
- The skeleton outlines Milestones A and B, with every cross-process contract in full. Milestones C and D are
  outlined when they start (0.15).
- Report files are no longer given to agents, state is kept per invocation, and the run lifecycle has a fixed order
  (0.4, 0.5).

Revision 8 (2026-11-13) adds, at the owner's request:
- One catalogue of named points, defined in full by the skeleton. Gates and chain hooks attach to any point (0.4).
- Observers: pack scripts that watch the event log without blocking it (0.4).
- Report files: any script, or any subprocess it starts, reports spans, metrics and progress live (0.5).
- Run and workspace state: hook outputs passed on to later steps (0.5).

Revision 7 (2026-11-13) applied the owner's feedback on revision 6:
- Orchard is the reference pack, never a special case. Everything specific to it is declared by its pack, its
  provider or the Claude adapter (0.1).
- The user designs each experiment: run order (`AB` or `ABBA`), number of pairs, warm-up, an A/A baseline, and
  whether to wait for other runs. The preview explains each choice and what it costs (0.14).
- The code is built skeleton-first and test-first. Stage 2 outlines the codebase, and every later stage fills part
  of it in (0.15; revision 9 narrowed the first outline to Milestones A and B).

Revision 6 applied the R3 fixes (Fable + Opus) and moved the task board early. See "Review log" at the end.

---

## 0. Decisions this plan rests on

### 0.1 Principles

- **Collide owns process state; the task source owns task state.** Run, phase, log, timing and workspace records
  are Collide's. Status, dependencies, `reviewed_by` and handoffs belong to the provider and the pack. Collide
  must not become a second register for a repo that has one (orchard's "One register" rule). Views hold
  membership only, never status, dependencies or evidence.
  - **Scratch tasks are the one exception** (owner, 2026-11-13). A prompt typed on a board becomes a task in
    Collide's own store, private to the user and the board, so every run still belongs to a task. Collide owns their
    status. They are for work no source tracks, or a repo with no source at all, and they are never committed.
  - **Private planning proposals** are the same exception (owner, 2026-11-13; a proposal was called a session
    before). They live in `collide/plan`'s data directory, private to the user and the board, until Share moves
    them into the planning source (0.10).
- **Mechanism in core, policy in packs.** A new kind of extension mechanism exists only when a real pack needs it.
  Lifecycle points come from one complete catalogue (0.4), so a pack can attach anywhere. Core never decides what
  to commit, how finished work is merged, or what state a pack's tools share across worktrees. Packs declare these
  through `record`, `integrate` and `experiment`.
- **Planning is a pack** (owner, 2026-11-13; `docs/research/planning.md`). `collide/plan` is built in, like
  `collide`.
  - It owns the planning roles (proposal, question, spike, decision, deferred), the record format, the spike
    workflows, the checklist templates and their gate, the planning views and the planning points.
  - Core knows kinds, links, a checklist field and generic task points, never roles.
  - Orchard's plan CLI and beads are example backends that shape the feature set, as Orchard and Ferry shape the
    rest. beads is not shipped.
- **Collide is built with Collide** (owner, 2026-11-13; part L). Each piece carries Collide's own work as soon as it
  lands:
  - planning first: its contracts lead the skeleton, and the skeleton stage ends with a working planning slice.
    From then on, Collide's build tasks live in its markdown provider (0.10);
  - runs from the headless-core stage: `collide run` takes one of those items;
  - packs from the run UI stage: Collide's tasks run through `collide/quick` and its spikes through `collide/plan`'s
    spike workflow.
- **Orchard is the reference pack, not a special case.** It is the real workflow the design is built against, so it
  is the example throughout this plan. Nothing in `core`, `host`, the app or the SDK names it or depends on how it
  works:
  - Repo conventions (label formats, branch prefixes, shared state, containers) are declared by a pack or a
    provider.
  - Agent-tool behaviour (Claude's auto memory, its flags, its env vars) lives in that tool's adapter.
  - Core tests run against two small fixture packs: a review loop, and a workflow shaped like issue → PR → CI. A
    grep cannot see structural coupling, so the second pack is the real check that core has no loop-shaped
    assumptions. Orchard is the end-to-end acceptance test.
  - **Ferry is a second example solution** (owner, 2026-11-13). Like Orchard, it is a real system the design is
    tested against, and it shapes the feature set only. No Ferry pack is a deliverable. Ferry compiles an
    architecture document into a service through its own loop of `claude -p` calls in containers
    (`docs/research/ferry.md`). Its needs change core only where Orchard or the issue → PR → CI pack would hit the
    same thing.
  - CI fails if a reference-pack name appears outside pack directories and test fixtures, or a Claude env var name
    (`CLAUDE*`, `ANTHROPIC_*`) appears outside the Claude adapter (0.15).
- **Workspaces are git checkouts** (worktrees or clones) in v0. Workspace hooks return `{ path, gitDir }`, and core
  tracks checkouts from its own records as well as `git worktree list`. Ref-scoped sources, `readOnly` and the
  inventory diff rely on this. Other kinds of workspace (plain directories, containers, jj) are deferred.
- **Same-user reality.** Agents with Bash run as the user and can do anything the user can. Permission prompts and
  Collide's checks are guardrails, not a security boundary, especially once gates run the branch's own code
  (orchard's `iter` runs the branch's dev CLI). A real boundary needs an OS sandbox or container; deferred, stated.
  - This covers Collide's own checks too (R5). The socket's peer credentials, the person-only operations (0.10) and the
    CLI token file (0.2) stop mistakes: an agent calling `collide` by accident, or a script that does not know the
    rules. An agent with Bash that means to can read the CLI token file, the user's files and the keychain entries
    the user's own session can read, so none of these claims more than a guardrail.

### 0.2 Process model

```
React UI ──one `rpc` command + one event channel──▶ Tauri shell (thin: forwards, adds the token)
                                              │ local socket, JSON-RPC, per-client token, version handshake
                                              ▼
                                   collide-daemon (started on demand; one `collide shim` per child)
                     ┌───────────────┬──────────────┼──────────────┬──────────────┐
                 agent adapters   hook executor   task providers   metrics       store (SQLite + event log)
```

- Children never talk to the owner over pipes. They write stdout, stderr and stream-json to files that the owner
  tails, so an owner restart loses nothing. Recovery is reattach by pid, or the adapter's resume
  (`claude --resume <session-id>`).
- **The owner is `collide-daemon`, with a per-child shim** (S3; owner, 2026-11-13).
  - The shim is the `collide shim` mode of the same binary. For one child it:
    - calls setsid;
    - starts the child with stdout and stderr going to log files;
    - holds the child's stdin pipe, and for an interactive session the PTY master, across a daemon restart
      (measured S2: a `claude -p` turn and a PTY take-over both went on across a daemon SIGKILL and restart, with no
      gap in input or output);
    - **journals the child's traffic in one ordered file** (R5), because an ACP session is rebuilt by replaying both
      directions (S7; 0.3). Each record is `{seq, wall_ns, dir, bytes}`: `seq` is a counter, `wall_ns` is the wall
      clock when the shim saw the bytes, and `dir` is `in` (written to the child's stdin) or `out` (read from its
      stdout). One file with one counter gives replay a single order, which two logs cannot give. The journal is
      written before the child sees the bytes, so an `in` record never goes missing, and a record cut short by a
      crash is dropped at replay. The daemon-to-shim messages are length-framed, so a half-written message is never
      mistaken for a whole one. Plain stdout and stderr files stay as they are, for tailing. The journal of
      host-to-agent bytes is new, and no measurement covers it;
    - for a PTY, keeps the last 1 MiB of output and replays it on attach, atomically with subscribing, so a view
      opened after an app restart shows the session so far (measured S2);
    - reaps the child with `libc::wait4`;
    - writes `exit.json` atomically (tmp, fsync, rename, fsync the directory), with exit status, signal, CPU time
      and max RSS (measured S3).
  - The daemon reattaches by pid plus start time and reads exit totals from `exit.json`, so a daemon restart loses
    nothing (measured S3: reattached and advanced in 0.18–0.34 s).
  - The shim protocol is versioned apart from the daemon build, and a daemon accepts the previous shim version. After
    an upgrade it meets shims the old build started, as herdr-core's core meets old holders (inferred).
  - **A shim that dies without writing `exit.json` makes its phase `lost`,** whatever the agent's stream reported.
    Its stdin closed, so pending permission requests were silently denied (0.3). The daemon sweeps the run's marker.
    A kqueue `NOTE_EXIT | NOTE_EXITSTATUS` registration made at reattach still gives the exit code, without totals
    (measured S3). Measured S2: the phase turned `lost` 22–26 ms after a SIGKILL. A pipe child saw EOF and exited, a
    PTY child exited within 600 ms (the hangup, inferred), and a child that ignores both stayed alive. So the shim
    records its child's pid and start time in the run directory, and the daemon stops that child when it marks the
    phase `lost`.
  - **Agents are `claude -p` children under the shim, not `claude --bg` sessions.** Measured S3: `--bg` gives no
    stream-json, does not enforce `--max-budget-usd`, takes its environment from claude's own daemon, leaves the
    tool's tree behind on `stop`, and gives no exit totals.
- **Starting the daemon.** In v0 the app and the CLI start it on demand when the socket does not answer (owner,
  2026-11-13).
  - An flock on a lock file serializes starts. The lock file holds the socket path. A start waits up to 15 s, and
    the daemon adopts existing shims before it binds the socket (herdr-core's protocol).
  - **On macOS the daemon leaves its launcher's coalition.** A setsid child stays in its launcher's coalition, the
    app's or Terminal's (measured S2). herdr-core shipped a daemon that ended with its app and fixed it with a
    one-shot launchd job (read: its `macos_launchd.rs` and changelog). Collide does the same: `launchctl bootstrap
    gui/<uid>` of a temporary plist, removed once the daemon runs (owner, 2026-11-13).
    - Measured S2 from a Tauri `.app` and from Terminal: the job puts the daemon, its shims and their children in a
      coalition of their own, and the daemon answers about 70 ms after the job starts (500 ms on the first launch).
    - The job is insurance. Quit and force-quit of a debug build left even a setsid daemon running (measured S2). A
      real Force Quit dialog, logout and a signed release build are unmeasured.
    - launchd starts the job in `/`, with a bare environment. Every path in the plist is absolute and canonical,
      and the plist sets `USER`, `HOME` and `PATH`: `claude` fails with `api_error` without `USER` (measured S2).
      The daemon then captures the login-shell environment (below).
    - **Over SSH, or with no GUI session, `launchctl bootstrap gui/<uid>` can fail** (inferred; the job needs a GUI
      session). The daemon then starts with setsid, outside the coalition, and the start prints a warning that it may
      end with its launcher. The run is not refused.
  - A shim whose child exits while the daemon is down starts the daemon, so a phase boundary is not missed while the
    app is closed.
  - The daemon records its login session and restarts in a new one. Ending at logout is recorded as host shutdown,
    not as a crash (herdr-core; inferred to apply).
  - It raises its soft open-file limit at start. launchd's default of 256 broke herdr at about 40 panes, and the
    daemon tails three files per child.
  - A LaunchAgent that keeps the daemon alive, or an `SMAppService` login item, is deferred.
  - **The resident app restarts the daemon** (owner, 2026-11-13). With its window closed the app stays in the menu
    bar (0.11). When the socket stops answering, the app starts the daemon again the same way. After three failed
    starts in five minutes it stops trying, and the menu shows the error and the daemon's log.
- **The base environment.** An app launched from Finder or launchd gets a minimal `PATH` and no `LANG`.
  - The daemon captures the login-shell environment once: `$SHELL -l -i -c` prints `env -0` between nonce markers,
    with stdin from `/dev/null`, in its own session that is killed after reading, under a timeout, with output
    capped at 1 MiB (measured S4; the session kill and the cap from herdr-core). On failure it falls back to the
    minimal environment and logs the shell and the reason.
  - `LANG` is set when missing.
  - **The capture is repeated when the app starts** (R5), and the daemon uses the newest one for each new phase. A
    capture made once at daemon start would hold a `PATH` or `ANTHROPIC_*` value for as long as the daemon lives,
    and the daemon outlives the app. A phase already running keeps the environment it started with.
  - Everything Collide starts gets this environment minus the adapter's stripped variables (0.3), terminal session
    ids (`TERM_SESSION_ID`, `ITERM_SESSION_ID`) and `XPC_SERVICE_NAME`/`XPC_FLAGS`.
- **The socket** is at a short fixed path. AF_UNIX paths are limited to 103 bytes on macOS (measured S3); a longer
  path fails fast with a message naming the path and the limit.
- **Where Collide's files live on macOS** (owner, 2026-11-15; revision 20). Everything Collide writes for itself is
  under `~/Library/Application Support/Collide`: the socket, the token and the lock file in its `run/` folder, and the
  store, the run folders and the daemon's log beside it. Settings a person edits are elsewhere: theirs in
  `~/.config/collide/`, a repo's in its `.collide/` (0.7). Collide makes no folder of its own in the home directory.
  The socket path fits the limit for a user name of up to 49 bytes. Linux and Windows get their
  own locations when they are supported (Deferred).
  - **Its directory is mode `0700`, and the daemon checks the directory's owner and mode at start** and refuses to
    bind in one that is not the user's or is open to others (R5). A connecting peer's uid must equal the daemon's
    (`getpeereid`), so a second local user cannot even reach the token check.
  - **The CLI's token** is in a file of mode `0600` in that directory, made at the daemon's first start. The CLI
    reads it, as the shell reads its own. It is never put in an environment variable. Under the same-user rule
    (0.1) an agent with Bash can read it, which is why who is asking comes from the peer's process, not the token
    (0.10, below).
  - **A shim's socket follows the same rules:** a directory of mode `0700` under the run directory, the same uid
    check, and no token, because only the daemon talks to it.
- **Who is asking** (R5). A connection's peer pid is read with `LOCAL_PEERPID`. The daemon walks that process's
  parents and looks for the markers below. A process with `COLLIDE_RUN` in its tree is a **run's agent**; one whose
  tree holds the app's process is the **app**; anything else is the **CLI from a terminal**. The result is the
  `actor` the daemon records on every event. Only the app may do the person-only operations (0.10); the CLI asks for
  them as a Queue confirmation instead. Processes whose environment macOS hides are found through their parents, as
  in the sweep (below). This is a guardrail (0.1).
  The app names itself: before it connects, it writes its executable path to `app-executable` in the daemon's run
  folder (mode 0600), and the daemon reads that file at each check, so a daemon the CLI started first still knows
  the app. The run marker is checked first, so the file never makes a run's agent the app (ts-0077-ws).
- **Clocks** (R5). macOS has three clocks, and a wrong choice breaks a timeout or a countdown the first time a laptop
  sleeps. Collide never uses one where another is meant:

  | clock | stops in sleep | use |
  |---|---|---|
  | uptime (`mach_absolute_time`, `CLOCK_UPTIME_RAW`) | yes | work timeouts: a phase, an ask, a `wait` timeout (0.7) |
  | continuous (`CLOCK_MONOTONIC_RAW`) | no | the cache countdown (0.3, 0.13) and spans shown as elapsed |
  | wall (`SystemTime`) | no, and can jump | the usage-limit reset (0.3), event timestamps, the journal's `wall_ns`, display |

  - A phase that slept for an hour has not worked for an hour, so work timeouts use the uptime clock. A cache that
    expires in five minutes, and a limit that resets at a time of day, do not wait for the laptop to wake, so they
    use the others.
  - The owner's sleep test (0.13) moves to Stage 3b, where it checks these choices.
- **Durability** (herdr's bugs, read):
  - logging never panics, and a full disk is tested with a writer that fails with `ENOSPC` on demand, not `/dev/full`
    (R5), so the failing write is a chosen one;
  - state is saved without holding a lock that other work waits on;
  - a store or `exit.json` that cannot be read is moved aside and reported, never overwritten;
  - on a full disk the shim keeps its exit record in memory and serves it until the daemon acknowledges it;
  - **free space is checked before a workspace is made and before each phase** (R5). The floor is 5 GiB by default, Collide's
    `disk.floor_gib` setting, `overridable` because it depends on the machine (owner, 2026-11-15; revision 20). Under
    the floor the run goes to
    `paused: disk` with a Queue item, and nothing is started. The check reads both the workspace's volume and the
    run directory's; a volume that cannot be read refuses the start instead of pausing it. A phase that ends with no
    `result` on a full volume fails with cause `disk`, not `lost`;
  - a child's stream lines and each phase's log have caps, so one runaway tool cannot fill the disk: a line over the
    cap is cut and marked, and a log over its cap stops growing with a final marker.
- **The OTLP receiver** (R5; 0.13) is part of the daemon. It binds `127.0.0.1` on a port the daemon picks, checks
  `Host`, wants a per-run token as a bearer header, and caps a request body. The token is made per run and handed to
  that run's child in its telemetry settings, so it is in the child's environment but is good for the receiver only.
  Telemetry of an interactive session uses the same receiver.
- **The workflow engine needs a live owner between phases.** Something has to notice that phase N ended and start
  phase N+1. With the app closed, the next phase started 0.25–0.37 s after the previous one ended, while the daemon
  lived (measured S3). If the daemon dies, children keep running and logs keep being written, but no phase advances
  until it restarts. A restarted daemon reattaches and advances.
- **The UI speaks the owner's protocol.** The webview calls one generic `rpc` command and listens on one event
  channel. The shell forwards each call to the socket and adds the token itself, so the token never reaches the
  webview. The CLI and the UI therefore share one contract, and UI tests run in a browser against a fake owner.
  A subscriber's events go through a bounded queue, so the daemon never waits for a client. A client that falls
  behind has its stream ended after what was queued, and subscribes again from the last `seq` it has (ts-0076-cw).
  The shell tells the window when the daemon's event stream ends (`shell.connection`, `lost`) and when it is back
  (`back`, sent before the replay). On `back` the window reloads its runs and Queue from the daemon and never trusts
  the replay to fill a gap (ts-0081-wm).
- Every child gets `COLLIDE_RUN=<id>` and `COLLIDE_RUN_START=<start clock>` in its environment. A stop finds
  descendants three ways:
  - a ppid walk;
  - a marker scan of the user's processes, enumerated in-process with libproc. Processes older than the run's start
    clock are skipped, and a process is identified by pid plus `p_uniqueid` (R5), which macOS never reuses, so a
    reused pid is never hit;
  - the descendants of everything the first two found. macOS hides the environment of its own binaries, but not
    their parents' (read: herdr-core `sweep.rs`).

  Measured S3 on macOS 26.3.1: `ps -E` reads a same-user process's environment, except for Apple platform binaries
  (`sh`, `bash`, `zsh`, `sleep`, `tail`, `perl`, …), for which it gives argv only. A double-forked orphan whose final
  image is a platform binary therefore escapes all three, and only its hook's record and the cleanup hook reach it
  (0.5).
  - The daemon and the shims carry `COLLIDE_RUNTIME=1` and are never swept. A shim must outlive the stop to reap its
    child and write `exit.json`.
  - When the CLI starts a daemon from inside a run (an agent's Bash), it strips `COLLIDE_RUN`, `COLLIDE_RUN_START`
    and `COLLIDE_SPAN` first, so that run's stop cannot kill the daemon.
- The socket requires a per-client token. The token is never put in an agent's or hook's environment. Run agents
  are given no access to Collide in v0 (0.3): nothing hands them a token or a way in. Under the same-user rule (0.1)
  an agent with Bash can still read the CLI token file, so this is a guardrail, and a call from a run's tree is
  recognised by its peer process and gets the `run agent` actor, which cannot do the person-only operations (0.10,
  above). The one other way in is the UI-tools endpoint below, which gives an agent five view-only tools and nothing
  else.
- **A step with a side effect writes an intent before it and an outcome after it** (R5). The side-effect steps are an
  integrate (a merge or a PR), a source op, an `after_integrate` hook, a remote write, and a discard (the deletion of
  a run's worktree and branch, with the branch's last commit in its events; ts-0080-tq). The intent names the step
  and what it will do; the outcome names the result. After a restart the engine finds an intent with no outcome and
  checks the world first, with the step's own check (is the branch merged, does the PR exist, is the label set), and
  never repeats the step blind. When the check cannot tell, the run waits in the Queue as "outcome unknown", for a
  person to say. Until Stage 9a's answer that says what happened, the item's one answer is Dismiss: it closes the
  step as failed and gives the run its review item back, so the run is never left with no way out (ts-0079-pu). A
  merge that finds its change already at the target is a success, not a conflict. Hooks run under the shim like any
  child, so a hook's side effect is covered by the same rule (Stage 8 tests it with a SIGKILL after the intent).
- **Coming back after a long close** (owner, 2026-11-15; revision 22). Collide may be closed for weeks while tasks,
  branches and worktrees change. Before anything acts on what Collide keeps, each board checks it against the world
  (a catch-up), and the person is told what changed.
  - **When.** At daemon start, and on wake from sleep (the gap between the continuous and uptime clocks, 0.13), since
    a daemon that stayed alive through a closed lid watched nothing. The daemon records both clocks and the wall clock
    at each event it handles and at a clean stop, not on a timer. The wall-clock gap only words the summary; a
    negative or implausible one, or a record that failed to write (a full disk), reads as unknown, and unknown
    means check.
  - **Hold.** While a board catches up, it shows "checking" and holds every new start (Up next included), its queued
    writes (0.10), the next phase of a run that survived, and the reaper. Reattaching to surviving children and the
    intent checks (above) still run.
  - **A check that cannot finish** (offline, an expired `gh` token or `claude` login, a repository that moved) is not
    a check that found nothing. Its source is "could not check": nothing is removed or closed on its account, what
    depends on it (its tasks' runs, Up next entries and Queue items) stays held, and one Queue item names the cause
    and the fix (sign in again, locate the repository, retry), with "continue anyway" for the person.
  - **Tasks.** Each source reconciles against its persisted cache (0.10). A remote source whose last full reconcile
    is older than its period runs one first, so issues deleted or transferred meanwhile are seen.
  - **Boards and lines.** A board whose repository moved or is gone is shown as missing, with "locate", and nothing
    of it is deleted (its scratch tasks, Up next list, trust entry and pack data). A line whose ref or root checkout
    is gone is shown as missing, nothing starts on it, and it leaves the Lines control's hand-picked sets; a line
    selector left on it moves to the board's default line and says why. Each line's snapshot is reloaded, and a
    changed trust hash raises its Queue item (0.11).
  - **Workspaces.** `git worktree list` is read for each repository (0.4). A workspace whose folder is gone is
    `prunable`, and one whose branch landed meanwhile is marked landed. The reaper takes it only on ancestry or patch
    evidence, with a clean tree, after the person has seen the summary; an inferred landing (0.4) is listed instead.
    A worktree first seen after a gap is attached only by `identify`, and otherwise listed for a person, since the
    person may have made it.
  - **Runs.** A run whose processes did not survive (a reboot) is recorded as interrupted by host shutdown, not as
    failed, and waits in the Queue to be resumed or stopped. A waiting run whose task changed since the run started
    says so on its item, with the diff, and the person chooses: resume, re-prompt with the task as it is now, or stop.
    A resume that cannot resume (the agent tool removed the session, or its login expired) says so and offers a
    re-prompt in the same workspace. A take-over whose process is gone is closed with its output kept. An
    experiment with an arm interrupted is marked interrupted and never resumed on its own (0.14), because its order
    and pairs no longer hold.
  - **Queue.** An item whose subject is gone (its task removed or closed elsewhere, its workspace removed) is closed
    as gone while Collide was closed, and listed; one whose subject changed is marked as changed since it was raised.
  - **Up next** is held when the world under it changed: an entry's task changed or went away, the line's ref moved
    (each entry builds on the last), the trust hash changed, or a check could not finish. An entry whose task was
    done or removed elsewhere is taken off, and a group from a view is computed again. A Queue item lists what
    changed, entry by entry, and nothing on the list starts until the person resumes it. A setting may also hold it
    after any stop longer than a set time; it is off by default.
  - **Views.** A personal view whose expression names a status or field the line's packs no longer have shows its
    error count (0.10) and is kept.
  - **One summary.** What a catch-up found notifies once, as "While Collide was closed", with counts of tasks
    changed, added and removed and a lens of them on the Field; never one notification for each.
  - **Caches on disk** need no clearing. Packs are keyed by git tree id and SDK version, and the SDK by version (0.6),
    so a changed pack is a new entry, and a `git:` pack stays at the tree its lock pins, so nothing is fetched at
    start. The agent tool's version, its tested range and the store's schema are checked as at any start (0.3, Stage
    3a).
  - **Limit.** A child that survived a daemon restart keeps sending telemetry to the receiver port it was given; when
    the new daemon binds another, that run's telemetry from then on is lost, and the run says so.
- **The UI-tools endpoint** (owner, 2026-11-14; S6's in-app agent section). It lets an agent change what the window
  shows (0.11). The transport was measured in S9 and R5 checked where it lives. It stays in the app shell (owner,
  2026-11-14), with the fixes below.
  - **It lives in the app process** (the Tauri shell), which owns the window. It is stateless streamable HTTP with
    JSON responses, on 127.0.0.1 only, on a port picked once and kept in the user's settings. A port already in use
    fails with a message that names it.
  - **The shell answers the protocol, and the window answers the tools** (R5). The shell refuses a foreign `Host` or
    `Origin`, a content type that is not JSON, a body over 1 MB, and a missing or wrong token. Then, by method:
    - `initialize`, `ping` and `tools/list` are answered by the shell alone, from a **manifest generated at build
      time** from the TypeScript tool definitions (0.11), so a client can connect and list tools with the window
      closed or busy, and the manifest cannot drift from the tools;
    - a notification (no `id`) gets `202 Accepted` and no body;
    - `GET` and `DELETE` get `405`, because the endpoint has no stream and no session to end;
    - only `tools/call` goes on to the window, as the event `ui_tools.request {requestId, caller, deadline,
      message}`. `caller` is the ask id or `external`, and `deadline` is when the shell will give up (10 s). The window
      runs the tools against its own reducer state and answers through `rpc` (`ui_tools.reply {requestId, …}`). A
      reply to a request the shell has dropped is ignored;
    - the shell counts the **per-ask cap of 20 screen-changing calls** (0.9), so a window reload cannot reset it;
    - the shell handles the `ui_tools.*` methods itself and does not forward them, so one `rpc` command and one
      event channel still carry everything (above).
  - **Asks are cancelled when the window closes or reloads** (R5): the shell sends `ask.cancel` for every ask it
    knows, because the page that was going to answer their calls is gone. A new window starts with no asks.
  - **Two kinds of token**, never put in an environment variable and compared in constant time:
    - the **endpoint token** is for external clients (0.7). It is 32 random bytes in the OS keychain, made at first
      use, shown and rotated in Settings. Rotating it ends every client that used the old one. It is sent as an
      `Authorization: Bearer` header, or as the last path segment (`/mcp/<token>`) for a client that cannot send
      headers;
    - an **ask token** is made by the shell for each ask the app launches, and works for that ask only. The shell
      adds the endpoint URL and the ask token to `ask.start` as it forwards it, so the webview never holds one.
      Claude gets it in a config file of mode 0600 in the ask's directory, never in argv, because `ps` shows argv. An
      ACP agent gets it in `session/new`, which the shim's stdin log records (above). That is why it is per ask: the
      log never holds the endpoint token, and the ask token is dead when the ask ends.
  - **The ask agent child is launched and owned the normal way:** the UI calls `ask.start`, the daemon builds the
    launch spec from the adapter, and a shim starts the child (0.3). Stop, the sweep and a daemon restart treat it as
    they treat a run's child, and it carries the same markers, with the ask's id in `COLLIDE_RUN`.
    **The daemon cancels an ask when the connection that started it drops** (R5), so a crashed shell or a closed
    window does not leave a model running with nobody to read it.
  - **A closed or silent window is a plain error to the agent.** With the window closed (0.11) the app stays in the
    menu bar and the endpoint stays up. `initialize`, `ping` and `tools/list` still answer, and a `tools/call` fails
    with `Collide has no window open; open Collide and ask again`, so the person is told what to do. A window that
    does not answer in time gives `the Collide window did not answer within 10 s`. Stage 6 tests the case of a
    window closed when the client starts.
  - **Alternatives, with what each gives up.**
    - **Considered and not chosen (owner, 2026-11-14): a stdio bridge, `collide mcp`,** that a client launches and
      that talks to the endpoint, and **the endpoint in the daemon**, which reaches the window over the event
      channel. The transport was measured in S9 and the shell is the smallest change from revision 12. The daemon
      would add a hop for every call and still has no use with the window closed.
    - The MCP layer in the shell, in Rust: no TypeScript SDK in the window, but the tool schemas are written twice
      and the validation still needs the window's ids.
    - S6's Vite plugin and websocket: dev only.
    - The choice costs a listener in the shell, which is new attack surface, so the guards and tokens above come
      first. A window that is hidden or throttled may answer slowly, and that is unmeasured.
- Crates: `core` (domain, state machines, workflow engine, pack model, implementation resolution; traits `AgentAdapter`,
  `AgentSession`, `HookRunner`, `TaskProvider` only; no Tauri, SQLite or process types) and `host` (implementations,
  store, `collide-daemon` and `collide` binaries; the shim is `collide shim`). `app/src-tauri` stays thin: it calls
  `host` (for example the `ui_tools` request handler) and adds only Tauri's commands, window and listener.
  `packages/pack-sdk` is the TypeScript SDK.

### 0.3 Agents and model sources

Two separate ideas:

- **Agent tool:** the program running the agent loop. v0: Claude Code. Later: opencode (`opencode acp` installed,
  1.1.56), goose (1.49.0 installed), Codex.
- **Model source:** where inference runs. v0: whatever Claude Code is configured for. Later: OpenAI, Ollama
  (0.40.15 installed), others, reached through the agent tools that support them.

Collide never calls a model API itself. Judgment steps are agent phases, or an ask (the ask agent, below).

- **Two core traits** (S7, `sp-0006-kq-agentadapter-acp`; owner, 2026-11-14). A one-shot command line whose stdout
  describes itself is Claude's shape. ACP is a stateful JSON-RPC conversation: the host sends `initialize`,
  `session/new` and `session/prompt`, matches request ids and replies to the agent's requests. A parser alone cannot
  do that, so there are two traits, and neither names a process, Tauri or SQLite type:
  - `AgentAdapter` is stateless, one per tool. It declares its **capabilities**, turns a role and phase into a
    launch spec (argv, env, working directory, the variables to strip), builds a session, lists the tool's config
    paths in a repo and names an interactive command for take-over.
  - `AgentSession` is one per child, a sans-IO state machine with `start`, `feed` (one line of the child's stdout and
    the **storable wall-clock time it arrived**, `wall_ns`: the session never reads a clock itself), `send_turn`,
    `answer`, `cancel` and `finish` (the exit info, returning the phase outcome). Each call returns events and the
    bytes to write. The same input gives the same output, so a daemon restart rebuilds a session with `replay`: it
    feeds the shim's journal back in `seq` order, `in` records as the bytes the session wrote and `out` records
    through `feed` with their stored `wall_ns` (0.2). A replay writes nothing to the child. A Claude session is nearly
    empty: every turn, the first included, is a `user` message on stdin (measured S1), and `feed` is the S1 parser.
    An ACP session holds its handshake, request ids and pending permission requests.
  - **Normalized events:** message, tool call, permission request, `SessionBound`, `Usage`, `TurnEnd`,
    `FinalReport`, `Limited { reset_at_ms }` (a usage or rate limit, with its reset time when known),
    `HookCall { name, event, outcome, exit_code, decision }` (never the hook's `stdout` or `stderr`; `decision` is the
    word the hook's output gave, when it gave one), `HookFailedOpen { name, event, tool_call_id, cause }` (a
    `PreToolUse` or `PostToolUse` hook that timed out or exited with a code other than 0 or 2, and whose tool then
    ran: the failed-open detector's observation, 0.13), `ToolRefused { id, cause }` with `cause` one of `hook`,
    `host`, `rule`, `missing_tool` or `unknown`, `ToolResultSize { tool_call_id, bytes }` (the size of a tool result,
    for each result, with no content), `Compaction { trigger, context_before, duration_ms, mid_task }` (from
    `compact_boundary`, each value null when the line lacks it; the context after is the next response's, 0.13), and
    `Other` (revision 18 errata, from `ts-0060-mm`, `ts-0063-rw`, `ts-0064-hw` and `ts-0067-kw`).
  - **A session starts as `Assign(uuid)`, `AgentAssigned` or `Resume(id)`.** Claude supports the first and last; ACP
    the last two. The id is recorded on `SessionBound`: at once for Claude, on the `session/new` response for ACP. If
    the daemon dies between the spawn and `SessionBound`, the id is lost, and recovery lists sessions by working
    directory when the agent offers that, or fails the phase as `lost`.
  - **Permissions.** A request carries its tool call, `kind`, `input: Option` (opencode sends `rawInput: {}` for a
    Bash call, so the command is unknown until the host answers; measured S7), why it was asked and the agent's own
    options. An answer is `Allow`, `Deny { message: Option }` or `Cancel`.
    - Claude maps `Allow` and `Deny` to `behavior`, with the message. ACP maps them to the options of kind
      `allow_once` and `reject_once`, drops the message and, when no `allow_once` exists, answers `Cancel` and
      reports a `PermissionUnanswerable` event.
    - **No adapter picks an "always" option.** The agent owns the scope and storage of those, and they would write
      rules into its own config.
    - A request whose input is unknown shows in the Queue as "input not available until approved" and is never
      auto-allowed by a pack rule that reads the input (0.11).
  - **Usage** is an event with a scope (`Turn`, `Step`, `RunningTotal`, `Compaction`, the model call of a
    compaction, or `OtherCalls`, any other model call outside the turn's own; neither is ever added into a `Turn`,
    and a `Step` is for context per response and is never added to a `Turn` either), every number an `Option`, a
    cost with its currency, `model_time_ms` (the model time, a difference of `duration_api_ms` for Claude), and a
    `quality`: `Exact`, `Partial` or `Unreported`. A total built from partial figures is shown as "at least" and never summed
    with exact ones (0.13). An ACP turn's usage is `Unreported`, every figure absent, when the agent sends none or
    its profile claims none; a figure never shows as 0 unless the agent said 0 (ts-0091).
  - **A turn ends with** `TurnEnd { reason, cancel_requested, empty }`. `reason` is `EndTurn`, `MaxTokens`,
    `MaxTurns`, `Refusal`, `Cancelled`, `Error { status }`, `Blocked { reason }` (a prompt a hook refused) or
    `Budget` (the native budget ran out; the session is kept). `empty` is true when the turn gave no message, no tool
    call and zero or unreported usage. `FinalReport` carries `source`: `ResultField` (Claude) or `Reconstructed` (ACP:
    the message chunks after the last tool call).
  - **Capabilities.** Each is `Yes`, `No` or `Unknown` (a profile for an ACP agent that has not been probed). A role
    may `require` capabilities, and Collide refuses to start a phase on a tool whose adapter says `No` or `Unknown`
    for one, rather than silently losing it:

    | capability | meaning |
    |---|---|
    | `headless_stream` | the host reads a stream the tool writes without a terminal |
    | `permission_host` | the host answers permission requests. Sub-flags `deny_message`, `input_at_request`, `request_reason` |
    | `session_resume` | `none`, `replay` or `silent` |
    | `background` | the tool can run detached. Claude has it; ACP does not |
    | `budget_native` | the tool enforces a budget itself (Claude). Any adapter with `usage_report.cost` can have the budget enforced by the core instead (below) |
    | `system_prompt` | `none`, `append` or `override` |
    | `tool_allowlist` | the tool takes a list of the tools an agent may use |
    | `named_agent` | the tool takes a named agent definition (Claude's `--agents`) |
    | `usage_report` | what usage gives: `granularity` (`turn`, `step`, `running_total`, `compaction`, `other_calls`), `cost`, `cache_write_tiers`, `context`, `quality` |
    | `limit_signal` | a structured usage limit with its reset time. Claude has it; ACP does not |
    | `compaction_events` | the tool reports compactions. Claude does; ACP only if the agent sends the update |
    | `hook_events` | hook outcomes appear in the stream. Claude has it; ACP does not |
    | `refusal_source` | a refused call says who refused it. Claude has it; ACP does not |
    | `strict_config` | the tool can run with nothing from the worktree or the user's agent config (Claude: the snapshot line; ACP: per profile) |
    | `context_options` | which of `compact_at`, `bash_output_max` and `mcp_output_max` it supports (Claude: all three) |
    | `model_select` | `launch` (Claude), `session` (ACP) or `none` |
    | `take_over` | it can name an interactive command for a session |
    | `mcp_servers` | `stdio`, `http`, `sse` |
- **v0 ships one adapter, Claude, for runs.** The launch line, measured by S1 on 2.1.323:

  ```
  claude -p --input-format stream-json --output-format stream-json --verbose --include-hook-events
    --session-id <uuid>                       # `Assign`; a resumed phase passes --resume <uuid> instead
    --permission-prompts host --permission-prompt-tool stdio
    --model <model.name> --max-budget-usd <budget left for this phase>
    --agents <snapshot>/agents.json --agent collide-<pack>-<role>
    --setting-sources user --settings <snapshot>/settings.json
    --mcp-config <snapshot>/mcp.json --strict-mcp-config
    --disallowed-tools <scheduling, worktree and messaging tools>
    [--plugin-dir <snapshot>/plugins/<name> …] [--permission-mode <mode>]
  ```

  - `--verbose` is required: without it, `-p` with stream-json output exits 1 (measured S1).
  - `--include-hook-events` makes every tool hook emit `system/hook_started` and `system/hook_response` (with
    `outcome` and `exit_code`). Without it a `PreToolUse` or `PostToolUse` hook leaves no event, and a hook that
    failed cannot be seen (measured: S9's addendum, 2.1.325).
  - `--permission-prompt-tool stdio` routes permission prompts to the host. Without it every prompt is denied
    automatically (measured S1). It is not in `--help`: it is the Agent SDK's own protocol (seen in the binary's
    strings; inferred). The contract test (Stage 3) pins it. If a release drops it, the adapter moves onto the
    Agent SDK (owner, 2026-11-13).
  - `--agents <file>` works under `-p`, so prompt text never appears in argv or in `ps`. `--agents` wins a name
    clash with a worktree's `.claude/agents/` (measured S1), and the `collide-<pack>-<role>` names are a second
    safeguard. A definition with a `tools:` list must name its MCP tools too, or they are hidden.
  - `--max-budget-usd` is a soft cap on this process's spend, checked after each model call, so it can overshoot
    by one call. A process over budget ends with `error_max_budget_usd` and refuses further turns; the phase goes
    to `ask_human` with cause budget (measured S1). Until Stage 4 has an engine to resume into, that is a
    `paused: ask_human` run with a `budget_exceeded` Queue item that takes Stop; raising the cap comes with Stage 4.
  - Cost (`total_cost_usd`) adds up per session, across `--resume`. So the adapter passes the remaining budget on
    each resume, and a phase's cost is the last total its session reported (measured S1).
  - **A budget is native or host-enforced** (S7). Claude's flag above is `budget_native`. For an adapter whose
    `usage_report` has cost, the core sends `cancel` when the running cost passes the budget, and may overshoot as
    Claude's cap does. A role with a budget requires one of the two, and is refused on a tool with neither.
  - **Disallowed tools.** The headless default tool set includes scheduling, worktree and messaging tools
    (`CronCreate`, `ScheduleWakeup`, `RemoteTrigger`, `PushNotification`, `EnterWorktree`, `ExitWorktree`,
    `Workflow`, `SendMessage`; measured S1). They start work the owner cannot see, or move the session out of its
    workspace, so they are disallowed unless a role asks for one (owner, 2026-11-13).
  - **A tested version range** (R5). The adapter names the range of `claude` versions its contract test passed on,
    starting at 2.1.323 (S1) and 2.1.325 (S9's addendum). The daemon reads `claude --version` at start and before a
    phase, cached by the binary's path and modification time. Outside the range, the daemon runs a one-turn smoke
    check (a prompt that must come back as a `result` with the pinned stream shape), and refuses the phase, with the
    version and the range, when it fails. A pass raises no Queue item; it marks the version chip (next bullet). The
    version goes in the fingerprint (0.13). Recorded against 2.1.325 (ts-0070-mk): a hook line carries no call id,
    so a failing `PreToolUse` hook is joined to the latest call of its tool; a failing tool that `system/init` lists
    is not `missing_tool`; SessionStart fires on `--resume`; `duration_api_ms` and cost run on across `--resume`; the
    smoke check took 2.1 s. An event the adapter does not know is an `Other`, counted per run, and a rising count is a finding
    (0.13). Where a documented flag does what a variable read from the binary does, the adapter prefers the flag once
    a test shows they agree: `--autocompact` is in 2.1.325's `--help`, and the contract test compares it with
    `CLAUDE_CODE_AUTO_COMPACT_WINDOW` (below).
  - **Two version chips** (owner, 2026-11-15; revision 16). Collide shows the `claude` version in use and the latest
    published version side by side: on the Agents page's Claude entry (0.7), in the run preview (0.11) and in the
    thin loop's window (Stage 3c). A version outside the tested range marks the in-use chip, whose tooltip names the
    range and when the smoke check passed. The published version is the `latest` dist-tag of
    `@anthropic-ai/claude-code` on the npm registry, with `stable` in the tooltip (measured 2026-11-15: `latest`
    2.1.325, `stable` 2.1.311). The daemon fetches it at start and once a day, through the system proxy, with a 5 s
    timeout and a request that carries no user data. A failed fetch shows the last value and its age, or "unknown".
    `claude.check_published: false`, a per-user key in `~/.config/collide/config.yaml` (default true), turns the
    fetch off and hides the chip. Collide does not update
    `claude`; the chips only show the gap.
  - **Settings that fail validation are ignored silently under `-p`** (R5; read from 2.1.325's own text). The
    contract test validates the generated `settings.json` against the published settings schema. At run time a
    **canary** covers the rest: the snapshot's SessionStart hook writes `collide-settings-canary` to stderr, and the
    adapter looks for it on that hook's `hook_response` line in the first turn (`hook_started` carries no command, so
    a user's own SessionStart hook cannot be told from Collide's). A first turn that ends without it fails the phase
    with `Claude ignored Collide's settings`, and the Queue says so. A
    phase with ignored settings would load the user's settings and hooks while the page promised it did not.
- **The stream protocol** (measured S1):
  - user turns, permission answers (`control_response`, allow or deny) and interrupts (`control_request`
    `interrupt`) go on stdin;
  - the phase report is `result.result` of the final `result` event, which holds the last assistant message only;
  - `result/success` does not mean the work happened, so the adapter also checks refused tool calls (below) and
    `is_error`. A `result/success` with `is_error: true` is an API failure, not a report (measured S1: one with
    `api_error_status: 429`). A 429 is handled as a usage limit (Usage and rate limits, below); any other status
    fails the phase, with the status;
  - **an adapter whose protocol has no failure flag (ACP) fails an empty turn.** A turn that ends `end_turn` with no
    output and no usage is a failed phase, cause `empty_turn`, and the record keeps the agent's stderr tail as the
    evidence (`failure_evidence`, at most 2 KiB from the end; a JSON-RPC error keeps its code there; ts-0091).
    Measured S7: a model Ollama lacked gave `end_turn`, zero usage and no message, and its error text
    appeared only on stderr. A turn that ends `refusal` or `max_tokens` fails the phase with that cause;
  - an error `result` still carries its spend, and the spend counts (measured S1: a budget error with
    `total_cost_usd` 0.00353);
  - a phase settles once the child has exited and its stdout has reached its end, in either order, so the last
    lines, usually the `result`, are never dropped (inferred: Ferry's platform supervisor settles on exit, which can
    lose them). That rule decides when reading stops, not whether the phase succeeded. The shim drains the child's
    stdout and stderr before it writes `exit.json`, so a reported exit means the journal is whole. An ACP phase's
    outcome is its prompt response, and a child that exits before one is `lost` (measured S7: closing stdin with a
    permission request pending made opencode exit 0 with no response);
  - **refused tool calls and hook outcomes** (measured: S9's addendum, Bash on haiku only). The `result` looks the
    same whether a call ran, a hook denied it or the host did, so:
    - a refused call is classified from the `user` event's `tool_result_meta`, joined on `tool_use_id`, never from
      the tool result's text. `non_execution_kind` says the call did not run, and `permission_decision.source`
      says who refused it: `hook`, `user_reject` (Collide's own deny) or `config` (a rule or an auto-deny);
    - a call that errors with no meta went to a tool the session does not have (`No such tool available`). Any
      other shape is recorded as an unknown refusal;
    - `result.permission_denials` lists hook, host, rule and auto denies alike, and leaves out calls to missing
      tools. It is never shown as "prompts the user refused";
    - a hook `ask` arrives as an ordinary `can_use_tool` request with `decision_reason_type: "hook"`. Its Queue
      item shows `decision_reason`, the hook's text, and is answered like any other request;
    - a hook that fails, fails open: on exit 1 or invalid JSON the tool still runs. Only `hook_response` shows the
      exit 1 (`outcome: "error"`). A timeout also emits `system/informational` with `tag: "hook_no_verdict"`.
      Invalid JSON looks like a pass;
    - a `PostToolUse` block leaves the tool result unchanged and `permission_denials` empty. An exit-2 block shows
      as `exit_code: 2`, and its stderr reaches the model. A JSON `{"decision":"block"}` shows only in the hook's
      `stdout`, and the model learns that a block happened but not why. So a Claude hook that a pack supplies (a
      plugin, an experiment arm) blocks with exit 2 and its reason on stderr, and the pack SDK's docs say so;
    - the record keeps each hook call's name, event, outcome and exit code. A hook's `stdout` and `stderr` are
      untrusted text that may hold secrets: the adapter reads `stdout` for a `decision` and stores neither;
  - **stdin must stay open.** If it closes while a request is pending, every remaining tool call is denied and the
    run still ends `result/success` with exit 0. The shim holds stdin across an owner restart (0.2), and a
    `permission stream closed` denial fails the phase;
  - stopping a phase sends `interrupt`, then closes stdin, then follows 0.5. SIGTERM alone ends `claude` with exit
    143 and no result. An ACP session sends `session/cancel`, waits for the prompt response, closes stdin, then
    follows 0.5. The session reports `Cancelled` itself, because opencode answered a cancel with `end_turn`, not
    `cancelled` (measured S7).
- **Who picks what:**
  - A role's `agent:` picks the tool.
  - A pack's `model:` names an alias (`fast`, `strong`). Each user maps aliases to a model and source *for that
    tool* in `~/.config/collide/models.yaml`. An alias never switches the tool.
  - Each mapping also names `model.name` (`opus`, `sonnet`). Collide requires only a safe token (0.5), because the
    name reaches env values, paths and arguments. It may also name an opaque `model.id`, which only the adapter
    reads and which never reaches env, paths or argv: ACP model ids such as `ollama/qwen2.5:7b` fail the safe token.
    For Claude, `model.id` is `model.name`. For ACP it is the agent's `modelId`, checked after `session/new` against
    the list the agent advertises. A pack whose labels need a narrower form
    declares it in `requires.modelName`. Orchard's pack declares `^[a-z]+$`, because its iter labels end in
    `[a-z]+`. Launch refuses a mapping that fails a pack's pattern, and names the pattern.
  - A fan-out whose aliases resolve to the same model is refused at launch, rather than silently losing its second
    model.
  - The role's `model` overrides any `model:` in the definition's frontmatter, because Collide passes `--model`
    explicitly (measured S1).
  - A user can override a role's tool per board, and Collide refuses the override if the tool lacks a capability
    the role requires.
- **Honest scope.** A pack whose roles use Claude-format definitions is Claude-bound.
  - The Orchard pack is Claude-bound: its definitions list its own MCP tools, and the dev CLI checks the iter label
    only when `CLAUDECODE` is set. It declares `requires.agents: [claude]`.
  - A pack that wants portability uses plain prompts and capability requirements instead.
- **What the adapter declares besides its launch line:**
  - **Environment to strip** from everything Collide starts. The Claude adapter strips `CLAUDECODE` and every
    `CLAUDE_*` variable; S1 found `CLAUDE_PID`, `CLAUDE_EFFORT`, `CLAUDE_ENV_FILE` and `CLAUDE_PROJECT_DIR` besides
    `CLAUDE_CODE_*`. Without this, a Collide started from a Claude Code terminal makes every hook look as if it runs
    inside Claude (measured: the dev CLI's agent label check then demands an iter label).
    - `claude` sets `CLAUDECODE=1` and its own `CLAUDE_*` values again in its children, so an agent's Bash calls
      always see them (measured S1). That is intended: Orchard's label check applies to agent work.
    - Collide adds no `ANTHROPIC_*` variable; the login comes from the user's own Claude config.
    - Nor does it strip one, so a user who routes Claude through a gateway or proxy keeps it. An
      `ANTHROPIC_BASE_URL` from the captured environment, or from a `settings.json` `env` (which `claude` applies
      itself, after the strip), is flagged in the run preview, and its host is recorded in the fingerprint (0.13). A
      proxy sees every request and may edit it, so cost and cache comparisons across it are not like for like
      (tokensafe research: its proxy mode sets this variable in the user's settings).
  - **The tool's config paths in a repo.** For Claude: `.claude/**`, `CLAUDE.md` and `.mcp.json`, plus auto memory
    under `~/.claude/projects/<main checkout key>/memory/`.
    - A reviewer works in the builder's worktree, and the builder can edit all of these. So the Claude adapter takes
      settings, agents and MCP config from the line's snapshot (`--setting-sources user`, `--settings`,
      `--mcp-config`, `--strict-mcp-config`).
    - Measured S1: under that line nothing from the worktree loads (project and local settings, CLAUDE.md, skills,
      agents, `.mcp.json`). The user's own CLAUDE.md, hooks and plugins still load, and so does auto memory.
    - Plain `claude` in a worktree, as in a take-over, loads everything, including the main checkout's untracked
      `settings.local.json` and `.mcp.json` servers nobody approved (measured S1).
    - **The repository's own instructions load from the worktree** (owner, 2026-11-15; revision 21). A phase reads
      the repository's `CLAUDE.md` files and skills from the folder it starts in (its worktree, or the root: 0.4,
      revision 22), as plain `claude` in that folder would, so
      a team's setup works unchanged: the guides those files name and the scripts they say to run are paths in the
      same checkout, at the worktree's root. Freezing `CLAUDE.md` at the line's commit would protect little, since
      the guides it names are read from the worktree anyway, and an agent outside Collide can edit it too. What runs
      code or grants access without the model asking still comes from the snapshot: settings and their hooks,
      permissions, MCP servers and agent definitions. The exception is a skill's own frontmatter (below).
      - A phase that starts after its run changed an instruction file shows a warning that names the files, on the
        run in Tracks and in the run record. The acknowledgement before `integrate` (below) is unchanged. The files
        are `CLAUDE.md` at any depth and `.claude/skills/**`, against the commit the workspace was made from, ignored
        files left out; `AGENTS.md` and `@` imports are not followed (`ts-0104-gx`).
      - A role may set `repo_instructions: none`, and experiment arms do (S1).
      - **The line** (measured `ts-0103-sa`, `claude` 2.1.326; owner, 2026-11-16): `--add-dir <the folder the phase
        starts in>` with `CLAUDE_CODE_ADDITIONAL_DIRECTORIES_CLAUDE_MD=1` loads that folder's `CLAUDE.md` and its `@`
        imports through `claude`'s own loader, while settings, hooks, MCP servers and agents stay from the snapshot
        and auto memory stays off. A nested `CLAUDE.md` does not load. The variable is not in `claude --help`, so the
        contract test pins it. Skills load through a plugin in the snapshot whose `skills` is a link to the
        folder's `.claude/skills` (measured `ts-0106-nr`), so they are read live and named `repo-skills:<skill>`; the
        plugin is written only when that folder exists. A skill's own frontmatter takes effect through it (measured
        `ts-0107-rj`): its `hooks` run and its `allowed-tools` skip the permission request. The owner accepted that
        as plain `claude` does (2026-11-17), so this is the one thing that runs code or skips a request and is not
        from the snapshot. Each `Skill` call still asks the host first, and `repo_instructions: none` leaves the
        plugin off; the Roles page says so.
    - **Auto memory is off for every phase** (`autoMemoryEnabled: false` in the snapshot settings; owner,
      2026-11-13). It is shared by every worktree of a repo, keyed to the main checkout, so it would carry one run's
      notes into another. A role may turn it on.
    - **Acknowledgement** (narrowed by the owner after S1, 2026-11-13). An edit to settings, hooks, MCP config or agent
      definitions cannot reach a later automatic phase, because those phases load them from the snapshot; an edit to
      an instruction file can, with the warning above (revision 21). It matters once it is about to reach the line's
      ref, and in a take-over. So the Queue asks for an acknowledgement before an `integrate` that carries such an
      edit, and before a take-over of a workspace whose range touches the paths.
- **Context and cache** (owner, 2026-11-13; `docs/research/tokens-and-context.md`).
  - Headless phases write the 1-hour prompt cache only (measured: no 5-minute writes in S1's 105 sessions). The one
    exception is the compaction call, which writes a small prefix at the 5-minute tier (181 tokens; measured S9).
    Both were measured under a claude.ai login; the tier under an API-key login is unmeasured. A phase resumed after
    more than an hour writes its whole context again at the write rate (measured S9: after 62 minutes idle, only the
    11K shared prefix was read). Every usage-limit resume is such a resume.
  - **`on_cold`** on an agent phase: `resume` (the default) keeps the session; `fresh` starts a new session from a
    handoff note. Collide builds the note from the transcript with no model call: the task, the files changed,
    commands that kept failing, open items and the last reply. A take-over can use the same note.
    - Measured S9 (haiku, 62 minutes idle): a cold resume cost $0.013 at 73K of context and $0.105 at 114K; a fresh
      session with a hand-written note cost $0.004 both times. Whether a note keeps the work going as well as the
      session does is unmeasured, so `resume` stays the default.
    - A Queue item that has waited past the cache's hour shows what resuming and starting fresh would cost, and so
      does the cold-return finding (0.13).
  - **Context options.** A role may set:
    - `compact_at`, which sets `CLAUDE_CODE_AUTO_COMPACT_WINDOW` (or `--autocompact` once the contract test shows it
      agrees, above). Compaction fires at that window minus 33K tokens.
      The smallest window is 100K and the largest the model's; an unusable value is ignored with no warning, and the
      variable overrides the user's own `settings.json` window (all measured S9). So Collide refuses a value under
      100,000;
    - `config: user | none`, which picks `--setting-sources user` or `""`. Both keep `--strict-mcp-config`: with
      `""` alone, one of S9's five runs still listed the user's claude.ai connectors (measured S9);
    - `repo_instructions: worktree | none` (default `worktree`), the repository's own instructions (above;
      revision 21): `worktree` adds the measured line, `none` leaves it out;
    - `bash_output_max`, in characters, which sets `BASH_MAX_OUTPUT_LENGTH` (default 30,000). Longer output is saved
      to a file that the model reads in parts (measured S9);
    - `mcp_output_max`, in tokens, which sets `MAX_MCP_OUTPUT_TOKENS` (default 25,000). A longer MCP result is
      replaced by an error (measured S9).

    A board or an experiment arm may override them. Collide sets them after stripping the inherited environment,
    and the fingerprint records the values in effect, including any that the user's `settings.json` sets (0.13;
    `bashOutputMaxChars` there does what the Bash variable does). The effective window comes from the
    `get_context_usage` control request (`maxTokens`), which is read from the binary's strings, not documented; when
    it fails, the fingerprint says `unknown`. The variable names are read from the 2.1.323 binary; S9 measured all
    three changing what the agent sees.
  - The user's config is the largest fixed cost. Measured S9 (haiku, an empty repo, the first response's context):
    `config: user` 30.9–32.6K; `config: none` 18.8K, and 14.4K with auto memory off and the disallowed tools, as
    Collide launches it; no tools at all 3.2K.
    - Fewer tools start lighter, so an agent definition with a short `tools:` list should too (inferred: S9 removed
      tools with `--disallowedTools` and `--tools`, not through a definition).
    - `--disable-slash-commands` raises the start weight (22.0K, measured S9) and is never passed.
    - The run preview shows the breakdown from `get_context_usage` (skills, memory files, MCP tools, messages);
      `system/init` gives names and counts only.
  - **The user's own tooling loads into phases** under `config: user`. S1 found a user `PreToolUse` hook (rtk)
    rewriting Bash commands, and token tools add hooks that hold prompts or set compaction variables.
    - The run preview lists the user hooks and plugins a phase will load. It flags those that can change what the
      model sees, by the event they hook, because a hook's output is unknown until it runs: `PreToolUse` (can
      rewrite a call), `UserPromptSubmit` (can block a prompt or add context), `SessionStart` (can add context)
      and `PostToolUse` (can replace a tool's result with its own summary). It also flags a settings `env` that
      sets a context variable or the model endpoint (above). Token tools install all of these (tokensafe research).
    - A prompt blocked by a hook becomes a Queue item with the hook's reason, never a silent `result/success`. The
      adapter sees a `system/informational` event with `prompt_submit_outcome` (`kind` `blocked_by_hook` or
      `stopped_by_hook`, and `reason`), then `result/success` with `num_turns: 0` and a null `terminal_reason`.
      That result's text is the hook's message, never the phase's report. A `/compact` result also has zero turns,
      but carries `local_command` (measured S9).
    - `UserPromptSubmit` hooks emit no hook events in the stream, so the preview's list is the only warning before a
      block (measured S9 without `--include-hook-events`; with it, unmeasured).
    - Experiment arms use `config: none` (S1).
- **Usage and rate limits.** Parallel runs share one account, so a limit is normal in the first week.
  - A `limited` event exists only for an adapter with `limit_signal`. For any other, the phase fails with the agent's
    message, and a role may require `limit_signal` to get the pause and resume below.
  - The adapter turns the tool's limit signal into a `limited` event with the reset time. For Claude that is a
    `rate_limit_event` whose `rate_limit_info.status` is `rejected`, with `resetsAt` in epoch seconds, or an
    assistant message with `error: "rate_limit"`. No text parsing is needed. One limit is one `limited` event: the
    signals of a turn (the rejected `rate_limit_event`, the message, a 429 error result) are held until the turn's
    `result`, and the event carries the first reset time any of them has. Read from the binary's schemas and
    local transcripts by S1; a live limit is unmeasured. `status: "allowed_warning"` lets other runs pause early.
  - The phase pauses (`paused: usage limit`) instead of failing. Other runs on the same account pause before their
    next turn.
  - The Queue shows the reset time. The phase resumes by session id when the limit resets, or when the user resumes
    it.
  - Paused time is a wait, not active time (0.13).
  - A limit pause lasts at least 30 s on the continuous clock before its reset time resumes it, so a stale reset
    cannot loop. A pause that an earlier daemon began is resumed by the next daemon at its reset time, with no CLI
    connected.
  - **The reset time is a wall-clock time** (R5). The countdown shown to the person is computed from the wall clock,
    and the pause's own duration is measured on the continuous clock (the table in 0.2), so a laptop that sleeps
    through the reset resumes the phase when it wakes, and does not wait out the rest of a countdown.
- **Agent work inside a `script` phase** (owner, 2026-11-13). A tool that runs its own agents is a `script` phase
  (Ferry runs `claude -p` in containers), so Collide never sees its stream. Such a phase declares
  `agentWork: true`:
  - the run preview and its Queue items say "agent work outside Collide's adapter: its budget and permissions are
    the pack's";
  - it reports `usage` lines, labelled reported and shown beside the measured totals, never added to them (0.5,
    0.13);
  - a `limited` line pauses the account's other runs, as the adapter's own limit event does;
  - its report lines and its telemetry never feed an experiment's primary metric, because the agents it starts
    could write them;
  - its `claude` processes are measured through Claude Code's telemetry. Collide puts the receiver's settings in the
    phase's environment (0.13), and a `claude -p` that the phase starts reports to it: its token totals equalled
    claude's own JSON usage, and a short run flushed before it exited (measured S2). That usage counts as measured.
    A phase whose sessions reported this way has its `usage` lines shown but not added, so nothing counts twice.
    Passing the settings into a container is the pack's job (unmeasured).
- **The ask agent is a separate role from run agents** (owner, 2026-11-14; S6's in-app agent section). It answers a
  request typed in the ask box (0.11) by reading, then calling the UI tools (0.2). Run agents still have no access to
  Collide in v0: they get no UI-tools endpoint and no token for one.
  - **It is not a run.** It has no task, workspace, phase or Tracks entry, and it is never resumed. Its adapter builds
    the launch spec from an `ask` profile (a model, a folder, tool kinds, an MCP config path, a timeout and an
    optional budget) instead of a role and phase. The daemon starts it under a shim, so stop (0.5), the sweep and a
    restart treat it as they treat a child of a run. Its record holds the argv without secrets, timings, each tool
    call, its usage with its quality and its outcome. If its shim dies the ask ends `lost`, and the user asks again.
    Its spend is shown on the Agents page (0.7) and is never added to a run or an experiment.
  - **The ask box takes a Claude Code entry or an ACP entry.** An External entry (0.7) is an agent the user runs
    themselves, so Collide launches nothing for it and the ask box cannot use it.
  - **The ask role requires `mcp_servers: http`.** Collide refuses to start an ask on a tool whose adapter says `No`
    or `Unknown` for it, as it does for a run's required capability, and the message names the agent and says so. For
    an ACP agent the `initialize` response is the probe.
  - **Claude Code's launch line.** The default entry is Claude Code with haiku (owner, 2026-11-14). It is stricter
    than a run's default (`config: user`): settings from Collide's own file and no user hooks, CLAUDE.md or memory
    (S9's `config: none`), only its own MCP server, three read tools and no session file. The user can opt in per
    agent to "use my Claude config", which passes `--setting-sources user` and nothing else different. S6's prototype
    did none of this, and each of its asks loaded 28K to 107K input tokens (measured). How much of that was the
    user's config is unmeasured, and Stage 6 measures it.

    ```
    claude -p --output-format stream-json --verbose
      --model <entry's model>                   # haiku by default
      --setting-sources ""                      # `config: user` passes `user` instead
      --settings <ask dir>/settings.json        # { "autoMemoryEnabled": false }
      --mcp-config <ask dir>/mcp.json --strict-mcp-config      # the endpoint and the ask token (0.2)
      --add-dir <extra readable folder>         # one per folder after the first (R5)
      --permission-prompts none                 # a request the line does not allow is denied, never asked
      --tools Read,Grep,Glob
      --allowedTools Read,Grep,Glob,mcp__collide__query,mcp__collide__set_view,mcp__collide__show_groups,
                     mcp__collide__highlight,mcp__collide__open
      --no-session-persistence
      --append-system-prompt "<Collide's fixed ask prompt>"
      [--max-budget-usd <budget>]               # only when the entry sets one
    ```

    - The request goes on stdin, so it is not in argv. The working directory is the entry's first readable folder, and
      each further readable folder is passed with `--add-dir`, which is how a second folder becomes readable at all
      (R5; `--add-dir` is in 2.1.325's `--help`; its effect under `-p` is unmeasured and Stage 6 tests it).
    - `--permission-prompts none` makes a read the line does not allow a plain denial. Without it the child could
      wait on a prompt nobody answers until the timeout (read from 2.1.325's `--help`; unmeasured under this line).
    - **`--restricted`** (R5) is a 2.1.325 flag that may confine reads to the working directory and added
      directories. Stage 6 measures it under `-p`, and the adapter adopts it if it does confine reads. `--bare` is
      not adopted (owner, 2026-11-14): it changes how the login is read.
    - `--tools` removes Bash, Edit and Write from the child. `--allowedTools` alone would only skip prompts (the S6
      record's reasoning; not tested on its own). With `--tools` cut to three, the scheduling and worktree tools of
      the run line's `--disallowed-tools` are absent already, so the ask line does not list them.
    - **`--disable-slash-commands` is not passed.** S6's prototype passed it. It raises the start weight (22.0K,
      measured S9), so this line drops it.
    - The child's environment is the base environment (0.2) minus the adapter's stripped variables. S6 removed every
      `CLAUDE*` and `OTEL_*` variable, and the adapter strips the `CLAUDE*` ones (above).
    - Whether a Read outside the working directory is refused without a prompt under `-p` is unmeasured. Stage 6 tests
      it and records the result. Until it does, the Agents page and the ask box say the agent "starts in this folder",
      and never that it "can read only" it. The contract test (opt-in) plants a file outside every readable folder
      with a known marker and checks that the marker never reaches the agent's answer, and that the Stage 6 criterion
      on the ask line equals the adapter's pinned line.
  - **ACP profile.** An ACP ask agent gets the engine of Stage 3 and an ask profile in `host`, with no `fs`,
    `terminal` or `elicitation` client capability, as above. The profile answers permission requests by kind:
    - `read` and `search` are allowed (`allow_once`) when every path the request reports, after `realpath`, is inside
      the entry's readable folders, also after `realpath` (R5). The comparison is made on resolved paths, because a
      symlink in a readable folder, or `..`, would otherwise reach out of it. A request that reports no path is
      refused, so the rule fails closed. Whether opencode asks at all for a read it is configured to allow is
      unmeasured;
    - everything else, `edit`, `execute`, `fetch`, `delete` and the rest, is refused with a message;
    - when no `allow_once` exists the answer is `Cancel`, as in every adapter, and no "always" option is ever picked.

    Collide cannot cut an ACP agent's own tools down by flags. S6 found that the readable folder is only its working
    directory and what it is told, and that opencode's own configuration decides what it can read or write. So the
    folder and tool kinds are checked by Collide on each permission request, and the Agents page says that an ACP
    agent's own settings are its promise, not Collide's guarantee (0.7).

    **opencode's default is unrestricted** (R5; S6 measured that its own configuration decides what it can read or
    write, and that it asks for nothing it is configured to allow). So the Test-this-agent action (0.7) runs a probe:
    the agent is asked to write a file and to run `true` in a temporary directory. If either happens, the probe is
    **red**, and the agent is refused as an ask agent until the person acknowledges that its own settings are not
    confined. The untrusted bodies of tasks and issues are withheld from `query` for such an agent (0.11), because it
    could act on instructions in them. Stage 6 also measures whether a permission configuration that Collide supplies
    changes the result, and records it.
  - **An ACP agent's listener.** `opencode acp` has `--port`, `--hostname`, `--mdns` and `--cors` (read from
    `opencode acp --help`, 1.1.56). Stage 6 checks with `lsof`, by the child's pid, whether a started agent listens on
    a port. The profile refuses `--mdns`, a `--hostname` that is not loopback and any `--cors`, and the Agents page
    names a port the agent opened.
  - **Budget and timeout.** A per-ask budget is optional and off by default (owner, 2026-11-14). Only Claude Code
    enforces one (`--max-budget-usd`, a soft cap that can overshoot by a call, above). An ACP agent reports no cost
    (measured S6), so a budget on one is a load error that says only a timeout applies. Every agent has a timeout,
    which is Collide's: it stops the child as a phase is stopped (above) and the ask box says `timed out after N s`.
    The Agents page states this plainly: ACP and External agents get a timeout only.
  - **Models.** A Claude entry's model is a name such as `haiku`. An ACP entry's model is an opaque id such as
    `ollama/qwen2.5:7b`. The adapter reads `configOptions` from the `session/new` response and, when one is a model
    selector, sets it with `session/set_config_option` (R5; the current ACP way). Only when the agent has no model
    option does it fall back to `session/set_model`, the call S7 measured. The id is checked against the list the
    agent advertises, as above.
- Later adapters for runs: an ACP adapter, opencode first, then Codex. S7 mapped `AgentAdapter` onto ACP before the
  skeleton outlined the traits, so they are not shaped by Claude alone. The ask agent's ACP profile (above) is the
  first use of the engine with a real agent, and it ships in Stage 6, before any run uses ACP.
  - **An ACP adapter is one engine plus a profile per agent, both in `host`.** The engine handles JSON-RPC, the
    handshake, update parsing and the permission mapping. The profile is data: argv, the env to strip and to set, the
    repo config paths and how to stop the agent reading other tools' config, the take-over command, the model-id
    mapping and known deviations. `core` holds only the two traits and the capability names.
  - A profile is probed by the agent's `initialize` response, and a capability it did not probe is `Unknown`.
  - **No client capability `fs`, `terminal` or `elicitation` in v0.** The agent keeps its own file and shell tools, so
    Collide stays out of the data path and a daemon restart cannot stall a write. Per-command CPU and memory then
    come only from the sampler (0.13). `elicitation` is added when a Queue item type for agent questions exists. The
    client advertises `session.notices`, `session.compaction` and `configOptions`, which only add observability.
  - Local-model capacity (GPU memory) becomes a scheduler resource token with the first non-Claude adapter.

Run modes: **headless** (workflow phases; stream-json → timeline) and **interactive** (portable-pty →
`ipc::Channel` → xterm.js, for ad-hoc work and for taking over a phase with `claude --resume <session-id>`).
Measured S2: a keystroke reached the page in 2 ms (p50) through the app, well under Claude's own redraw (p50 13 ms,
p95 59 ms), and xterm's DOM renderer was as fast as WebGL, so WebGL is optional.

herdr is out as a dependency. R1's reason, snapshots only, holds for herdr's API (`pane.read`, 0.9.5) but not for
herdr-core, which streams. The reasons now (`docs/research/herdr.md`, 2026-11-13):
- herdr-core has no long-lived stdin pipe for headless agents;
- it records no exit totals and has no per-client token;
- it is unpublished, three weeks old, and needs Zig to build.

Collide adapts its patterns instead: the coalition escape, the start lock, the sweep, the durability rules and the
hostile-process tests (0.2, 0.5, Stage 3).

### 0.4 Extension model

| mechanism | lifetime | can block | examples |
|---|---|---|---|
| **hook** | one command per call | yes, via its result | `workspace.create`, `record`, `integrate`, `sync`, gates, workflow decisions, `script` and `wait` phases |
| **provider** | long-lived process, JSON-RPC over stdio | n/a | task sources |
| **observer** | long-lived process, events as JSONL on stdin | no | notifications, exporters, dashboards |

How packs share points:

- **Implementations, chosen per workflow.** Packs contribute named `workspace`, `integrate` and `sync`
  implementations (`orchard/worktree`, `orchard/merge-local`, `github/pull-request`). Each workflow names the ones it
  uses.
  - GitHub is a source on every board, so a board routinely holds Orchard's local-merge loop and a PR workflow at
    once. A per-board slot would make them two claimants.
  - A workflow that names no workspace implementation uses the built-in one (`git worktree add`, Stage 3). One that
    names no `integrate` cannot be accepted, and review says why.
  - The built-in workspace puts a repo's worktrees under one root, in Collide's data directory, per repo
    (`worktreeRoot` in config). Interactive Claude asks once whether to trust that root, and does not ask again in
    a folder inside a trusted one (measured S2 for a subfolder), so one answer covers the repo's later sessions
    (0.11).
  - The run preview names the implementations a run will use.
- **Where an agent starts** (owner, 2026-11-15; revision 22). Collide sits on top of the way a team already works,
  and most developers start their agents at the repository root. A role says where its agent starts with `start`:
  `workspace` (a workspace the workflow's implementation makes), `root` (the line root, which for the default line
  is the person's own checkout), or a folder under the root.
  - **The default follows the workflow.** A workflow whose agents make their own worktrees says so with
    `agentWorktrees: true`. Its roles start at the root by default (owner), and Collide provisions nothing for it.
    Every other workflow's roles start in the workspace by default, so a pack that says nothing keeps its agents
    out of the person's checkout.
  - `root` is refused for a `readOnly` role (it cannot be checked at the root), a sandbox phase, an experiment arm
    (0.14) and a task whose author is not trusted (0.9). A person may override `start` per board in settings (0.7),
    and the effective value is checked again.
  - The run preview says where each phase starts, and warns when that is the person's checkout and the workflow may
    merge into its branch there.
  - **The root session** launches with the snapshot's flags like any phase (0.3, 0.7), so untracked local settings
    and MCP servers in the person's checkout are not loaded (measured, `ts-0103-sa`).
  - **Collide's writes at a root with a live phase** do not wait (0.10's queue is for workspaces). A task write and
    its `record` run as one step under the line lock, retrying while git's index lock is held, so no uncommitted
    Collide edit is left for the agent's `git add` or merge to sweep up.
  - One phase that starts at the root runs per line at a time; a workflow may raise it with `rootPhases`. Another
    waits, shown as waiting on the root, because two agents merging into one branch would collide.
  - Collide cannot tell an agent's edits at the root from the person's. It records HEAD and `git status` when the
    phase starts, shows the commits added and the files changed when it ends, and flags each file-edit tool call
    (0.3) the agent makes to a path at the root outside its worktrees. Writes made through a shell command escape
    that flag (0.1, same-user reality). A root left on another branch at the phase's end is flagged, and `record`
    refuses there, as on any line root off its ref.
- **Worktrees an agent makes for itself** (owner, 2026-11-15; revision 22). A repository's own instructions may have
  an agent make worktrees itself: Orchard's orchestrating session runs `dev worktree new` for each task from the
  root, has the work done in those worktrees, and merges it back. Collide notices them, and the agent and the
  repository change nothing. A workflow with `agentWorktrees: true` names, with `adopt`, the workspace implementation
  whose `identify` and `release` hooks apply; nothing is provisioned through it.
  - **Reading.** Collide reads `git worktree list` and the repository's branches once for each repository, not for
    each run: after each tool call of a run there that ran a command, at the end of each phase, every minute while a
    run there is live, and when the daemon starts. The entries present when a run starts are its baseline, kept in
    the store, so a restart keeps it. Entries of the baseline are never attached.
  - **Collide's own provisioning is excluded.** Before a workspace hook starts (`create`, `line`, `sandbox`, an
    experiment root), Collide records that it is provisioning on that repository. An entry that appears while the
    hook runs is held until it returns: the entry it returns is Collide's workspace, and only the others are
    attributed.
  - **Attribution**, first answer wins:
    1. the optional `identify` hook of the workflow's workspace implementation: it gets the path and branch and
       returns task keys. Its input is written by an agent, so it is untrusted (0.9), and an answer outside the
       tasks of the runs live on that repository is ignored and counted. Orchard's reads the task that `dev
       worktree new` records when it is given one, then the creator label, which a role's `ORCHARD_ITER_AGENT`
       sets (0.7);
    2. the run whose command call was running when the entry appeared, when exactly one was. A worktree the person
       makes in a terminal meanwhile is attached by this rule too, and the person detaches it;
    3. the working directories of the runs' processes;
    4. a person: the entry is listed under its line as made during a run, with the candidate runs.

    With one run live and no task named, the entry is attached to the run with its task unknown. Tracks shows how
    each entry was attached, and a person can detach it in one step.
  - **An adopted workspace** (kind `adopted`, 0.8) belongs to its task, or to its run when the task is unknown.
    Collide tracks its branch, diff and size, its per-checkout task state, and whether its branch has landed (below),
    as for a workspace Collide made, and shows it under its run in Tracks. A later run of the same task attaches it
    again, and a phase may start in it (Orchard's reviewers work read-only in the build worktree, and its fix
    rounds reuse it), with the `readOnly` check. A workflow that provisions for each task starts its `workspace`
    roles in the task's adopted worktree when it has one and skips `create`; with two, the latest, and the preview
    names it.
  - **Disk.** Each new entry's volume is read by the free-space check (0.2), and under the floor the run goes to
    `paused: disk`. Size is measured at each phase end and on request, not on every read.
  - **Removal.** Collide never removes or changes an adopted worktree, and the reaper skips it. On a stop, its
    processes are stopped with the run's (0.2), and the implementation's `cleanup` hook takes its lanes down as for
    any workspace, removing nothing. The tool that made it, or the agent, removes it under that tool's own rules
    (Orchard's `worktree rm` refuses a worktree another agent made). A person may ask Collide to remove one through
    the optional `release` hook, which runs with the creator's label. A half-made one, left by a creation command
    that was stopped, is listed like any other. One whose folder is gone is `prunable`; one no longer listed is
    `removed`.
  - **Limits.** Worktrees made in another repository are not seen. A worktree made and removed between two reads
    leaves only its branch, which is listed as made during a run.
  - **Two Orchard modes.** A workflow whose workspace implementation provisions for each task (Orchard's `loop`,
    through `ws-create`) and one whose agent makes its own worktrees (Orchard's `session`, with `agentWorktrees:
    true`) are separate workflows. A run in a workspace Collide made is told in its brief that its worktree exists,
    so the repository's instruction to make one is already met.
- **How a workflow's work lands** (owner, 2026-11-15; revision 22). Each workflow says, with `landing`:
  - `integrate` (the default, except with `agentWorktrees: true`): every workspace of the run, adopted ones
    included, goes through accept, the gates and the workflow's `integrate` (0.4, Stage 9). Commits that a root
    phase adds to the line's ref itself are flagged in review as landed outside accept;
  - `observed` (the default with `agentWorktrees: true`): the repository's own flow merges, as Orchard's
    orchestrating session does. A branch has landed when its commits are on the line's ref, by ancestry or by patch
    (a rebase or a cherry-pick), or, inferred, when the line's ref already has its changes to the files it touched
    (a squash). The item shows which rule matched, and the reaper never removes on an inferred one. Its task is
    then landed, and Up next moves on task by task (Stage 11). A worktree whose task is unknown lands no
    task: its merge is shown, and the person names the task. Collide checks each merged range against
    `protectedPaths` (0.9) after the fact, and a range that touched them raises a Queue item. A merge that changes
    the line's trust hash holds the line's next start, Up next included, behind the trust Queue item (0.11).
  - **Work that never lands** (owner). When a run ends, fails or is stopped with commits or uncommitted changes in
    its workspaces that are not on the line's ref, a Queue item lists each workspace with its branch, its changes
    and its size. The person opens the diff; re-prompts (a new run of the task in that workspace, told to land it);
    lands it through the workflow's `integrate`, with accept and the gates; removes it (`release`, for an adopted
    one); or keeps it, which leaves the task unlanded and asks whether its Up next entry waits or is skipped. Until
    then the entry stays paused.
- **The built-in `collide` pack** ships with the app and is trusted with it, so it needs no trust prompt (owner,
  2026-11-13). It makes a repo with no pack usable (0.11, first run):
  - `collide/quick`: one agent phase in a worktree, then review: accept, re-prompt or discard;
  - `collide/review`: build, review, then fix or close, with the maximum number of rounds as a `launch` parameter.
    It also stops, to `ask_human`, when a round leaves as many open findings as the round before (Ferry's
    convergence guard);
  - `collide/merge-local`: integrate by a local merge into the base ref.

  Its prompts are plain, with no Claude-format definitions, so it is also the first check that a pack can stay
  tool-neutral. With GitHub's `pull-request` it covers a GitHub repo too.
- **Contribute:** roles, prompts, gates, workflows, providers, observers, `record` paths, `metrics`, `experiment`,
  `config`, `profiles`.
  - Names are namespaced `pack/name`, so two packs may each have a `builder`.
  - Several workflows can be applied. `board.yaml` names the default, a profile may name another (0.7), and a run
    may choose any.
  - Each pack's `record` hook commits only files under the paths that pack declares.
  - **Built-in packs ship compiled in the app bundle** (R5), with their own copy of the SDK, so they need no
    materialization into the data directory, no git tree id and no trust prompt, and they cannot be edited in place.
    Their hooks run with `node` from the captured environment (0.2). When `node` is missing, the message says that
    the built-in pack needs it and where it looked, not a bare "command not found".
- **Named points.** One catalogue, defined in full by the skeleton (Stage 2), so a pack can attach anywhere:

  | object | points |
  |---|---|
  | run | `run.before_start`, `run.after_end`, `run.before_integrate`, `run.after_integrate` |
  | workspace | `workspace.after_create`, `workspace.before_remove` |
  | phase | `phase.before_start`, `phase.after_end` |
  | task | `task.before_status`, `task.after_status`, `task.before_op`, `task.after_op`, `task.before_move`, `task.after_move` |
  | experiment | `experiment.before_arm`, `experiment.after_arm` |

  - **Chain** handlers attach to any point. They run in pack order (the board's packs, then the user's; 0.7), and each
    sees the outputs of the handlers before it at the same point (0.5).
  - At a `before_*` point, `refused` or `error` stops later handlers and the action.
  - At an `after_*` point the action has already happened. `refused` or `error` stops later handlers and fails the
    step, and the workflow's failure rules apply.
  - `run.after_end` handlers all run, on every outcome, including a run that failed while provisioning. Nothing is
    left to stop.
  - Each handler receives `pack` and `workflow`, so it can ignore other packs' runs.
  - A point fires once the stage that owns it lands. Adding a point to the catalogue is a schema change.
  - `task.before_status` fires before every status write, from a workflow op or from the UI. It matches a status
    **category** (`task.before_status: review`), never a source's status name, so a gate is not tied to one source.
    Its cwd is the workspace of the task's latest run if that workspace still exists, otherwise the line root. The
    input's `workspace` says which.
  - The other task points (owner, 2026-11-13; planning needs them, and each serves other packs too):
    - status points match a category, op points an op name, and move points the target source;
    - for one op the order is `task.before_op`, then `task.before_status` if the op moves the category, the write,
      `task.after_status`, then `task.after_op`. `setStatus` fires only the status points;
    - move points fire once per Share, with every item in `tasks[]`. A refusal moves nothing;
    - points fire around writes Collide makes and writes its runs make (owner, 2026-11-13; S11). A run's write
      reaches its points one of two ways:
      - A ref-scoped source's change in the workspace fires from the run's task diff when the run is accepted,
        before `integrate`, with `change.via: run` and `change.op: null` (status, `create` and `delete` points
        only). A refusal stops `integrate`.
      - A global source's change that core attributes to a phase fires its `after_*` points once, on this machine,
        and runs the `before_*` gates as a phase-end check. A refusal fails the phase and the write stands. The
        second-pack stage builds this for beads and for GitHub issues, and `via` is reserved now.
        - beads: `BEADS_ACTOR` is set in the phase's `env:`, so the attribution is exact.
        - GitHub: writes carry the user's own login, so core matches the phase's tool calls to the issue's
          timeline events. The attribution is labelled "attributed", never exact.
    - a change Collide only reads from elsewhere (a pull, a teammate, a hand edit, a bot) fires no point. It reaches
      observers as `task.changed`, and the item carries `outside`: `{at, cause, categoryMoved}`, a field that views,
      `when:` and a `queue: true` lane can read. The chip shows for any outside change, and is emphasized when the
      category moved (owner, 2026-11-13; S11);
    - status points fire on every change of status name. `match` tests the target category, and
      `change.categoryMoved` says whether it moved. An op fires them when it changes the status name;
    - a call on several items fires per item, in order, with `change.batch`. A refusal skips that item. Move points
      stay once per Share;
    - `apply` → `conflict` fires no `after_*` point. A retry re-runs the `before_*` points;
    - outside a run (a UI write, a Share), outputs are kept with the write's event, `unverified` holds the write in
      the Queue, and an `after_*` failure becomes a Queue item while the write stays.
  - A binding may add `when:`, CEL over `task`, `change` and the binding pack's `config`. Gates bind only to
    `before_*` points: a gate bound to an `after_*` point is refused at load.
  - **Derived points** (owner, 2026-11-13). A pack may declare `points:`, each a name in its namespace, one core
    point, what it matches and a `when:`:

    ```yaml
    points:
      decision.before_decide:                # bound as collide/plan/decision.before_decide
        on: task.before_status
        match: [done, dropped]
        when: 'task.kind in config.roles.decision && change.op != "supersede"'
    ```

    - Binding to a derived point binds its core point with both filters joined by AND. Each `when:` reads its own
      pack's `config`, so a team's pack never needs to know the backend's kind names.
    - A derived point never fires on its own. It comes from a core point on the same side (`before_*` from
      `before_*`), never from another derived point.
    - The trust prompt shows what each binding expands to.
    - `collide/plan` declares the planning moments this way, such as `proposal.before_share`,
      `spike.before_conclude` and `decision.after_decide` (the initial set: the planning note's section 7.5). A
      GitHub pack could declare `issue.before_close` the same way.
- **Lifecycle order.** One run, in order. A refusal at a `before_*` point stops the run there.

  | step | points and hooks |
  |---|---|
  | 1. start | `run.before_start` |
  | 2. workspace | the workflow's workspace `create` (or `resume` for a failed one), then `workspace.after_create` |
  | 3. start ops | `on_start` ops (each fires `task.before_status`), then `record` |
  | 4. each phase | `phase.before_start`, the phase, its gates, `phase.after_end`, `record` if the phase declares it, then its `then:` (0.7) |
  | 5. end ops | `on_end` ops, then `record` |
  | 6. cleanup | `cleanup`, only after `interrupted` or `timed_out` |
  | 7. end | `run.after_end`, on every outcome |
  | later: accept | `sync` check; the `before_*` points of the source's operations and of the run's task diff; `run.before_integrate`, `integrate` (answers `merged` or `pending`, 0.5); once `merged`: `run.after_integrate`, the source's operations (writes, then `after_*` points); `record` |
  | later: removal | `workspace.before_remove`, then `remove` (task workspaces) or `discard` (sandbox and experiment) |

  Accepting is a user action on a finished run, so the integrate points fire after `run.after_end`. The `before_*`
  points of accept run first, so a refused `done` gate leaves the code unmerged (S11).
  - **`integrating` and `pending`** (R5). A pull request is not merged when `integrate` returns, so a result of
    `pending {url, head}` puts the run in the state `integrating`, not `done`. The source's operations and
    `run.after_integrate` wait for `merged`. A workflow polls with a `wait` phase (0.7), and a pending `integrate`
    that is polled again replaces its entry in run state (0.5). A group is ready only when every member's merge
    commit is an ancestor of the line's ref (0.7, Stage 11), so a group never counts a PR that was merged and then
    reverted, or closed without merging.
  - The integrate step writes an intent before the merge or the PR and an outcome after (0.2), so a restart in the
    middle checks whether the branch is already merged or the PR exists, and does not make a second one.
- **Gates** attach either to a workflow's phases or to a named point (`task.before_status: review`), so a codemap or
  lint pack's gate applies whatever workflow is running.
- **Syncing with the base.** Long runs fall behind their base.
  - Core shows how far each workspace's base is behind its line's ref.
  - Before `integrate`, core checks for conflicts without touching any checkout (`git merge-tree --write-tree`). A
    conflict goes to the Queue.
  - Syncing itself is policy: a pack's `sync` implementation fetches and then rebases or merges. A workflow runs it
    as a `script` phase, or names it to run before `integrate`.
- **Observers** watch the event log. They cannot block anything or change any state.
  - A pack declares the event types each observer wants (`events: ["run.*", "ask_human.*"]`). The trust prompt
    lists each observer and its events, because events can carry tool input and agent messages.
  - An observer receives run events only from runs on lines that apply its pack, never from other lines' runs.
  - An observer also receives source events (`task.changed`, `task.renamed`, `task.removed`, `task.invalid`) for
    every line that applies its pack, whoever made the change (owner, 2026-11-13; S11).
  - Phase events carry the paths of the phase's logs and transcript, because a registry needs them.
  - The first real observer is Orchard's agent registry (`agents.jsonl`). It starts as a phase hook in the Orchard
    loop stage and moves onto an observer in the second-pack stage, so observers ship with a real user (0.1).
  - Events arrive as JSONL on stdin, each with a sequence number, and the observer acknowledges each one on stdout.
    After the observer or the owner restarts, delivery resumes after the last acknowledgement. Delivery is
    therefore at least once, and observers dedupe by sequence number.
  - A slow observer falls behind in the on-disk log, not in Collide's memory.
  - The supervisor that restarts providers restarts observers too, with backoff. After repeated crashes it
    disables the observer and puts an item in the Queue.
  - Events carry `experiment`, so an observer can skip side effects during an experiment, as hooks do.
  - In v0, observers cannot act on Collide (start runs, write tasks): see "Deferred".
  - The **event catalogue** is defined in full by the skeleton, like the points. It covers:
    - run and phase transitions;
    - gate verdicts, permission requests and answers, `ask_human`, budgets;
    - task changes and writes;
    - workspace states;
    - reported metrics;
    - experiments.
- **Line lock.** Everything that writes a line's root checkout holds that root's lock: `record`, `integrate` and
  provider writes there. Writes to one root run one at a time. Different lines' roots do not block each other.
- **Root busy.** The default line's root is the user's own checkout. While a rebase, merge, cherry-pick or bisect is
  in progress there, writes to it wait, and the Queue says why (0.12).

Agent-side config (Claude Code hooks, skills, plugins, the repo's `.claude/`) is passed through, never reinvented.

### 0.5 Hook protocol

- **argv only, never a shell string.** No templating into argv. Task text reaches hooks only as JSON on stdin.
- **Input** on stdin:
  `{ protocol, hook, pack, board, line, experiment, run, launch, config, tasks[], phase, workspace, history[], state }`.
  - `config` holds the pack's settings, secrets included (0.7). Settings travel on stdin, never in env, because
    `ps -E` reads a same-user process's environment (0.2).
  - `tasks` is an array from v0, even though grouped runs come later.
  - Each task carries `key`, `shortId`, `path` and its template fields (below).
  - Workspace hooks get `tasks[]` too, so Orchard's `ws-create` can derive the label that `dev worktree new`
    requires.
  - `experiment` is null outside an experiment. Inside one it names the arm and its directory, so a hook can skip
    side effects such as a push or a registry line (0.14).
  - Task points add `change: { op, args, from, to, diff, categoryMoved, via, actor, batch, effects, effectOf,
    renamed, checkout, files }` (owner, 2026-11-13; S11).
    - `via` is `ui | workflow | share | integrate | run`.
    - `actor` is `{ kind: person | workflow | agent, run?, phase? }`.
    - `diff` maps each changed field to `{from, to}`, or `{added, removed}` for sets. It is predicted at `before_*`
      (from `preview` when the provider has it) and actual at `after_*`.
    - `from` is null at create and `to` is null at delete.
    - Each `tasks[]` entry carries its links one hop out, in both directions, as `{type, dir, key, kind, status,
      category, stale}`, resolved across sources. It still carries fields and paths, never bodies.
    - Outside a run, `latestRun` as before: the item's latest finished run, read-only.
  - Point bindings take `timeout:`. The default is 30 s for task points and 120 s for move points, which may scan
    files. A timeout is `error`.
- **Result**: the file named by `$COLLIDE_RESULT`:
  `{ "status": "ok" | "fail" | "unverified" | "pending" | "refused" | "error", "message", "outputs": {},
  "retryAfter"?: seconds }`. The status in this file is the verdict. A non-zero exit or a missing or invalid file is
  `error`. A child tool's exit code is never forwarded as a verdict (R1: `iter`, `plan` and `preflight` use exit 2 for
  usage errors).
  - **`unverified`** (owner, 2026-11-13; Ferry's three-state gate result): a gate, `script` or `wait` hook could
    not check here, for example Docker or Orchard's lanes are down, or a CI check was cancelled. It is neither a
    pass nor a fail, so no fixer is sent at code nobody proved broken.
    - In a workflow or at a named point, it holds the run in the Queue with the hook's message. The person retries
      the hook, proceeds as if it passed, or stops the run.
    - From any other hook (workspace, `record`, `integrate`) it is `error`.
    - Experiments count it as its own outcome, never a pass (0.14).
  - **`pending`** (R5) is the answer of a `wait` hook, or of `integrate` for a pull request, when the thing it checks
    has not happened yet and may later. It is not a verdict and ends no span as a failure.
    - `retryAfter` is a hint in seconds for when to ask again. The engine takes the larger of it and the phase's
      `interval:` (0.7), never less than 1 s, and a `pending` with no hint uses the interval.
    - Each poll is a new invocation of the hook, and a `pending` entry **replaces** the previous entry for the same
      `pack/hook`, phase and round in run state, so a CI wait polled every 30 s for an hour keeps one entry and not
      120 (0.5, State). The last non-pending result is kept as before.
    - A `wait` phase that reaches its `timeout:` while still `pending` ends as `error`, with the last message.
    - From any hook other than `wait` and a PR `integrate`, `pending` is `error`.
- **State.** A hook's `outputs` are kept and passed on, so a step can hand data to later steps.
  - Outputs of workspace hooks become **workspace state**. It lasts as long as the workspace, across runs. So
    `cleanup`, `remove`, `resume` and `doctor` receive what `create` learned instead of rediscovering it.
    - A later hook's outputs merge over earlier ones key by key (`resume` updates what `create` wrote).
    - Workspace state is a hint, not a fact: the world can change under it. `doctor` checks it and rewrites what it
      finds.
  - Outputs of other hooks and gates become **run state**, one entry per invocation. An entry is keyed by
    `pack/hook` plus the phase, round and fan-out instance, so a gate that runs at build, at every fix and at close
    keeps every result. Readers get the latest entry by default, and every entry on request. Run state is frozen when
    the run ends and kept with its history.
  - Every hook receives `state: { run, workspace, point }`. `point` holds the outputs of earlier chain handlers at
    the same point.
  - Who else reads state:
    - `when:` expressions read run state;
    - env templates may use a state value only if it is a safe token (below);
    - prompts never do. Agents get artifacts instead.
  - State is written only when a hook ends, together with its verdict. A hook that crashes leaves none.
  - A pack may declare a JSON Schema for a hook's `outputs`. Outputs that fail it make the result `error`. Each
    output object is capped at 64 KB; larger data belongs in an artifact.
  - State is process state, not a register. Task status, dependencies and evidence still change only through the
    provider's operations (0.1). There is no board-wide or cross-run state. A pack that needs memory across runs
    keeps its own files in its data directory (0.7).
- **cwd:**
  - the line root for `workspace.*`, `record`, `integrate` and provider writes (0.12);
  - the workspace for phase hooks and gates.
- **Logs:** stdout and stderr go to files, streamed to the UI.
- **Stopping:**
  - Hooks run in their own process group. The stop combines the ppid walk, the marker scan and their descendants
    (0.2), since `iter` runs each step in its own group.
  - **Order** (measured S3):
    1. SIGTERM to everything found. An agent first gets `interrupt`, then its stdin closes (0.3).
    2. The grace period, configurable per hook: default 60 s; `iter` escalates after 30 s.
    3. A rescan, then SIGKILL to everything found.
    4. The workspace implementation's `cleanup` hook.
    5. A final rescan and the survivor report.
  - The shim checks that its child's process group still exists (`waitid` with `WNOWAIT`) before it signals the
    group, so a reused group id is never hit (herdr-core).
  - A workspace implementation's `cleanup` hook always runs after `interrupted` or `timed_out`. It is the only thing
    that reaches what the scans miss:
    - processes started detached with a cleaned environment. Orchard's lanes are Docker containers plus host server
      processes started with only `PATH` and `HOME`, so they carry no `COLLIDE_RUN` (measured: the dev CLI's
      lane code);
    - double-forked orphans whose final image is an Apple platform binary (0.2).
  - **Hooks record what they detach** (owner, 2026-11-13). A hook that starts something in the background returns
    its pid and start time, or its container id, in its outputs under `detached`. The cleanup hook receives the list.
  - **The survivor report** lists recorded processes still alive, plus suspects: parent pid 1, the same user, started
    after the run, environment unreadable. It reports suspects; it never kills them.
- **Timing and metrics:** every hook, gate and phase is a span (0.13). Every child gets `COLLIDE_SPAN=<span id>`
  beside `COLLIDE_RUN`. The environment is fixed when a process starts, so a tool subprocess carries its phase's span
  (0.13 says how finer attribution works).
- **Reporting.** Every hook, gate, `script` phase and `wait` phase gets `COLLIDE_REPORT=<span dir>/report.jsonl`.
  - **Agent phases do not.** An agent, or task text injected into it, could otherwise write `tests.passed` or a
    span that becomes an experiment's primary metric. Work done inside an agent phase is measured by sampling and by
    the pack's import (0.13).
  - **Only a hook's own report lines may be primary** (R5). A line counts toward an experiment's primary metric only
    when it comes from a hook, gate or phase whose `COLLIDE_REPORT` descriptor that process opened itself. A child
    that inherited the descriptor, for example a tool the hook started that started an agent, writes lines that are
    shown and never primary. Collide checks this by giving each hook its own file and marking lines written by a
    process other than the hook's own pid as `inherited`.
  - Anything in the process tree, in any language, appends lines, and Collide tails the file as it tails logs:

    ```
    {"v":1,"t":"2026-11-13T09:00:00.120Z","kind":"span.start","id":"4121-1","name":"map build"}
    {"v":1,"t":"2026-11-13T09:25:03.004Z","kind":"span.end","id":"4121-1","status":"ok","attrs":{"cache":"miss"}}
    {"v":1,"t":"2026-11-13T09:25:03.010Z","kind":"metric","name":"tests.passed","value":412,"unit":"count"}
    {"v":1,"t":"2026-11-13T09:25:03.011Z","kind":"progress","done":3,"total":10}
    {"v":1,"t":"2026-11-13T09:25:04.000Z","kind":"usage","model":"sonnet","tokensIn":1200,"tokensOut":800,"cacheRead":50000,"cacheWrite":3000}
    {"v":1,"t":"2026-11-13T09:25:05.000Z","kind":"limited","resetsAt":1763049600}
    ```
  - Every line carries its timestamp `t`. A line without one is rejected and counted, because a time stamped on
    reading would be wrong by the tailing lag.
  - `usage` and `limited` lines count only from a `script` phase with `agentWork: true` (0.3). Elsewhere they are
    rejected and counted.
  - A `metric` is never combined with measured tokens or cost. A saving that a reporter claims (tokens or cost a
    hook says it avoided) is an estimate whatever its name, shown labelled reported and never subtracted from the
    measured totals, which already contain any real effect. A hook that answers after a tool returned, such as a
    `PostToolUse` hook that blocks with a summary, does not take back what the model read (inferred; tokensafe
    research). The pack author docs say so.
  - A `usage` line keeps `cacheRead` and `cacheWrite` apart, and may split `cacheWrite` into `cacheWrite1h` and
    `cacheWrite5m`. A line with a merged `cached` count instead is rejected and counted, because a write and a read
    are priced far apart. A field the reporter does not know is left out and shows as "not reported" (0.13).
  - **Span ids.** `collide report` and the SDK generate ids as `<pid>-<counter>`, so writers sharing a file do not
    collide. A hand-written id that repeats one already open is rejected and counted.
  - **Nesting.** A reported span nests under the span that owns the file, or under `parent`. `collide report span`
    exports `COLLIDE_REPORT_PARENT=<its id>` to the command it wraps, so a nested `collide report` nests under it.
  - A `span.start` with no `span.end` is closed when the owning span ends, and marked `unclosed`.
  - `collide report` writes lines for shell scripts:
    - `collide report span "map build" -- ./dev iter map-build` wraps a command, with its exit code;
    - `collide report metric tests.passed 412 --unit count` records a value.

    It writes the file directly and needs no socket or token. Outside Collide it does nothing and says so on stderr.
  - **Who cannot report:** a process in a container, and a process started with a cleaned environment (Orchard's
    lane servers). Neither has `COLLIDE_REPORT`. A hook can copy their numbers into its own report afterwards.
  - **Caps** (S8; proposed, so inferred): a line is at most 4 KB, a file takes 100 lines per second sustained with a
    burst of 1,000 (a token bucket), and 16 MiB in all. At the size cap the span is marked "report truncated", and
    the tailer keeps counting. Malformed or excess lines are counted on the span, not hidden.
  - **Collide polls the file every 50 ms** and holds a partial last line until its newline. Measured S8: no line was
    lost or torn at any rate tried, up to 50,000 lines per second, with p99 lag 52–57 ms. The caps protect the store
    and the view, not the tailer.
  - Reported data is labelled "reported" and never mixed with what Collide measured. It is data, never templated,
    and the redaction list applies to its attributes (0.13).
  - Why a file and not an API:
    - writes are append-only and need no reply;
    - no token reaches a hook's environment (0.2);
    - an owner restart loses nothing.
- **Env:** `COLLIDE_*` plus role, phase and experiment `env:` templates, applied in that order, so an experiment's
  value wins over a role's (both set `ORCHARD_ITER_TIMINGS` for a reviewer). The variables the agent adapter
  declares are stripped first (0.3). Templates may use only these values:
  - `run.slug`: generated by Collide, lower-case `[a-z0-9-]`, because it reaches workspace and branch names;
  - `phase.label`, `phase.dir`, `round`, `model` (the alias), `model.name`;
  - `arm.dir` (one per arm per repetition) and `git.commonDir`;
  - `group.name`: a safe token the user gives a run of several tasks at launch;
  - `task.<field>` for each **template field** the provider declares. `task.*` is defined only in a run of exactly
    one task. A workflow that uses it is refused at launch for several tasks, and the preview says why.
    - A provider declares template fields in `initialize`, each with a regex. Every value must also be a **safe
      token**: 1–64 characters from `[A-Za-z0-9._-]`, not starting with `-` or `.`, and never containing `..`. So no
      provider can route task text into a template, a value cannot climb out of a directory, and it cannot become a
      flag.
    - Collide checks each value when it reads the task. A task whose value fails is shown with the error and
      cannot be launched.
    - Example: the plan provider declares `labelId: '^t\d{2}-\d{3}$'`. Orchard's keys are upper case
      (`T-01-042`), but its iter label pattern needs the
      lower-case form (measured). Core knows neither form.

  Task text never reaches a template.

### 0.6 Packs in TypeScript

- Any executable works (`run: [python3, x.py]`). TypeScript is first-class: `script: x.ts` → `node x.ts`, with
  types stripped natively. Measured by R1 on Node 24.18.0: inline TS strips, `enum` is rejected, cold start is
  0.08–0.09 s over 3 runs. Constraint: erasable syntax only, `.ts` import extensions. Measured S4: a hook that
  imports the SDK starts in 123 ms (median of 10, on a loaded machine) against 65 ms for `node -e 0`, and in 81 ms
  with a warm `NODE_COMPILE_CACHE`.
- **Node refuses to strip types under `node_modules`** (measured by R1: `ERR_UNSUPPORTED_NODE_MODULES_TYPE_STRIPPING`).
  So `@collide/pack-sdk` ships as compiled JS + `.d.ts`, with one runtime dependency, `yaml` 2.9.1, in the SDK copy's
  own `node_modules` (owner, 2026-11-17; `ts-0109-qj`), and Collide supplies it:
  - Collide materializes the pack into `~/Library/Application Support/Collide/cache/packs/<tree id>-<SDK version>/`,
    keyed by the pack's git tree id, whatever the pack's source (0.7), and by the SDK it links, so a new SDK makes new
    entries and a pack runs with the SDK its `requires.sdk` was checked against (ts-0095-kp). An entry never changes,
    and lines share it whenever their pack content matches. It holds:
    - `pack/`, a read-only, verified copy of the pack directory;
    - Collide's own `package.json` (`{"private": true, "type": "module"}`), so a hook's module type never depends
      on a `package.json` above the cache (measured S4);
    - `node_modules/@collide/pack-sdk`, a symlink to a versioned compiled SDK outside the app bundle
      (`~/Library/Application Support/Collide/cache/sdk/<version>/`). Node checks the realpath, so a `.ts` SDK is never
      shipped (measured S4);
    - a marker, written last. The entry is built in a staging directory and renamed into place.
  - **A pack that contains a `node_modules` path or a `package.json` is refused at load.** A committed
    `node_modules/@collide/pack-sdk` shadows Collide's SDK (measured S4; R2's "a snapshot has no `node_modules`" was
    wrong). So the SDK never comes from the repo.
  - `requires.sdk` names the range the pack accepts.
  - v0 packs have no other npm dependencies but `yaml`: the SDK supplies `yaml` 2.9.1 as `@collide/pack-sdk/yaml`
    (owner, 2026-11-17; `ts-0109-qj`). A pack that needs another dependency vendors it.
  - **Built-in packs are not materialized** (R5). `collide`, `collide/plan` and any later built-in pack are compiled
    into the app bundle with their own copy of the SDK, read-only, with no cache entry, no trust prompt and no tree
    id to pin (the app's version is the pin, and the fingerprint records it). They run with `node` from the captured
    environment (0.2), and a missing `node` fails with a message that names the built-in pack and what it needs.
    Their `requires` are not checked at load, so a line with no `node` still loads; the provider's start reports
    it (ts-0096-ph).
- **The context object.** `defineHook(async (input, ctx) => …)` gives:
  - `ctx.state`, to read state;
  - `ctx.config`, the pack's settings, and `ctx.dataDir`, its data directory (0.7);
  - `ctx.report.span`, `.metric` and `.progress`;
  - `ctx.ok(outputs)`, `ctx.fail(message)` and `ctx.refuse(message)`, which write the result file.

  `defineObserver` acknowledges each event when its handler returns. The context is a convenience over stdin, the
  result file and the report file. Those files are the contract, so a Python or shell hook can do the same without
  the SDK.
- Stripping does not type-check: `collide pack test` runs `tsc --noEmit`. S4 measured it with TypeScript 7.0.2, the
  native compiler: it passes a good pack and fails a type error, given `@types/node`. Whether Collide bundles `tsc`
  (about 30 MB) or uses the pack repo's own is decided in the second-pack stage.
- `requires.tools` (e.g. `node >= 22.18`) is checked when a pack loads. A missing tool fails the load with the
  pack's install `hint:`. Collide never installs anything (owner, 2026-11-13). A pack that drives another CLI, as a
  Ferry pack would drive Ferry, declares it here. A tool's version is the first `major.minor[.patch]` that
  `<tool> --version` prints, run once per load with a 5 s limit and a cleared environment; a range is comparators
  joined by spaces (`>=0.1 <0.2`), each with a `major.minor[.patch]` version (ts-0096-ph).
- Rejected: embedding a JS engine. Hooks start CLIs anyway, so a sandbox would buy little and cost Node and npm.
- **Types** (S5, `sp-0005-fv-schema-expressions`; owner, 2026-11-14): Rust serde types → `schemars` → JSON Schema →
  TypeScript types with `json-schema-to-typescript`, which is a Node step in the build. Not `specta` (a release
  candidate when tested) and not `ts-rs` (it types a `#[serde(default)]` field as required). The same schema
  validates `pack.yaml`, `board.yaml` and views.
  - Every struct and tagged variant sets `additionalProperties: false`, so unknown keys are rejected (measured S5: 0
    of 13 invalid cases accepted, and the generated TypeScript accepted every valid case).
  - **`Wait {}`, not a unit variant, in an internally tagged enum:** serde does not enforce `deny_unknown_fields` on a
    unit variant.
  - Input and output contracts are generated separately: an input has defaults, an output is complete. The
    validator needs the `uint*` formats registered or relaxed.
  - Numeric ranges are invisible to the generated types. The schema is the contract.

### 0.7 Pack manifest (`pack.yaml`)

- **YAML is read with `serde-saphyr` 2.x and a hardened option set, never its defaults** (S5; owner, 2026-11-14).
  Its defaults take YAML 1.1 booleans (`no` becomes `false`), which would break the promise that `no` stays a
  string. The options are part of the loader and a test pins them: `strict_booleans`, `no_schema`, merge keys as
  errors, no anchors or aliases, at most 100,000 nodes and depth 32, and duplicate keys an error.
  - Typed String fields keep `no`, `0.10` and `1e10` strings in every crate tested (measured S5). An ambiguous
    unquoted scalar in a string field is an error.
  - Duplicate keys are an error everywhere, inside maps too, where the `serde_yaml` family keeps the last. Anchors,
    aliases and merge keys are rejected, and the error text says so. Files are size-capped before parsing.
  - Unknown keys are rejected to catch typos. Names and ids need `minLength: 1`, and a `config` map is checked
    against its schema.
  - Every `{{ }}` template must be quoted: unquoted it parses as a flow mapping. The loader reports it at its
    column with the hint "quote the template".
- `serde_yaml` is archived and not used. `$schema` points at `collide schema --path`, not a path relative to the
  Collide repo.
- **The Collide directory** holds every committed Collide file: `board.yaml`, `packs.lock`, `config.yaml` and
  `views/`. It is `.collide/` at the repo root. A repo that wants it elsewhere commits a `.collide` **file** instead,
  containing `dir: <path relative to the repo root>`, the same way git's `.git` file points a worktree at its git
  directory. In this plan, `.collide/` means that directory, wherever it is.
- `.collide/board.yaml` (committed) holds:
  - the packs, in order, each from a source (below);
  - the default workflow, and the profiles: `profiles.default` and `profiles.rules` (below);
  - source configuration: repos and projects for GitHub. The account is per user, in the user's settings;
  - the status mapping;
  - the checkout filter.

  `.collide/packs.lock` pins each `git:` pack by its source, commit and tree id, so the whole team loads the same
  commit. A pack in the repo needs no lock entry: its pin is the tree of `<ref>:<pack path>`, the repo's own content
  hash (S4), computed at load, and the trust hash covers it (0.9; owner, 2026-11-13). `collide init` writes
  `board.yaml`, and the lock appears with the first `git:` pack. 0.12 says which keys a line may override.
- **Settings** (pack config and Collide's own settings):
  - A pack declares its settings in `config:`, one JSON Schema per key, with defaults:

    ```yaml
    config:
      lanePool:   { type: integer, minimum: 1, default: 2 }
      notifyHook: { type: string, secret: true }
    ```
  - Values come from four layers, lowest first (R5 corrected the count):
    1. the schema's defaults;
    2. the user's file, `~/.config/collide/config.yaml`, per board and per pack;
    3. the repo's shared file, `.collide/config.yaml` (committed). **The repo's value wins**, so a team setting
       applies to every developer (owner, 2026-11-13). The exception is a key the pack marks `overridable: true`,
       such as a lane pool that depends on the machine: there the user's value wins (owner, 2026-11-13; open
       question 7);
    4. the run's profile, if it has one (below).
  - Collide's own settings (concurrency cap, metrics level, retention, default model aliases) use the same layers
    and the same files. Collide marks its machine-dependent keys, such as the concurrency cap, `overridable`.
  - Some keys are per user by nature and are refused in the repo file: secrets, trust entries, source accounts,
    personal views, and an agent's kind, command, arguments, model, environment, secrets, folders and tool kinds
    (below).
  - Some keys describe the repo and are refused in the user's file and in a user profile: a pack marks them
    `repoOnly: true`, as `collide/plan` does `layout` (owner, 2026-11-15; revision 20). The message names the key.
  - **Secrets** (`secret: true`) come only from the OS keychain, set in the UI or with `collide config set --secret`,
    which reads the value from standard input and refuses one on the command line. A secret in either file is a load
    error. Hooks receive settings as `config` on stdin (0.5). `collide config set` does not rewrite a file that holds
    a comment; edit it by hand (ts-0099-bw).
  - `.collide/config.yaml` loads from the line's snapshot and is in the trust hash, like `board.yaml` (0.9).
  - **Agent entries** (owner, 2026-11-14; S6's in-app agent section). The agents the ask box can use (0.3, 0.11) are
    Collide's own settings under `agents:`, one entry per id, and `ask.agent` names the one the ask box uses. They
    follow the layers above. The first layer holds one built-in entry, Claude Code with haiku, which the user's file
    may change field by field.

    ```yaml
    # ~/.config/collide/config.yaml
    agents:
      claude-haiku:                  # the built-in entry, shown with every field it has
        kind: claude-code            # claude-code | acp | external
        model: haiku
        config: none                 # none | user ("use my Claude config", 0.3)
        read: ["<board root>"]       # folders it may read; the default is the board's default line root
        tools: [read, search]        # tool kinds it may use
        timeout_s: 180               # every agent has one: 5 to 1800
        budget_usd: null             # optional, off by default; Claude Code only
      local-opencode:
        kind: acp
        command: opencode
        args: [acp]
        model: ollama/qwen2.5:7b     # opaque ACP model id (0.3)
        env: { OPENCODE_DISABLE_CLAUDE_CODE: "1" }
        secrets: { SOME_API_KEY: keychain-item-name }    # a reference by name, never a value
      my-client: { kind: external }  # an agent the user runs; Collide launches nothing
    ask: { agent: claude-haiku }
    ```

    - **Layers and sources.** The repo's file may set only two defaults for every ask, `ask.budget_usd` and
      `ask.timeout_s` (R5), because a team has a say in cost and not in what runs on a developer's machine. They are
      not per agent id: ids are the user's own names, and a committed `agents.<id>` would name an agent the repo
      cannot know. The repo's `ask.budget_usd` wins over an entry's own `budget_usd`, so a team's cap holds (owner,
      2026-11-16); an entry's own `timeout_s` in the user's file wins over the repo's default.
      `ask.timeout_s` is `overridable`, since a slow local model is a fact about one machine. A repo budget that
      reaches an agent that cannot enforce one (ACP, External) is ignored, with a notice on the Agents page naming the
      agent, and is not a load error, because the repo's file cannot know which agent a developer picked. Every other
      `agents` field is refused in the repo file, naming the key, because a command in a committed file would be code
      that Collide runs on the strength of a trust prompt. The Agents page and `collide config get
      agents.<id>.<field>` show each value with its layer: built-in, the user's file or the repo's file.
    - **An ACP entry's `args` are checked** (R5). The profile refuses `--mdns`, a `--hostname` that is not a loopback
      address, and `--cors` in `args`, with a message that names the flag (0.3). The page names a port the started
      agent opened, if Stage 6's check finds one.
    - **Secrets are names.** An entry's `secrets` map an environment variable to an item in the OS keychain. The value
      is read at launch and goes only into the child's environment, never into argv, a log, the ask's record or a
      file. A missing item is a launch error that names it. An `env` line whose name ends in `KEY`, `TOKEN`, `SECRET`
      or `PASSWORD` and carries a literal value is a load error that says to use `secrets` (inferred: a name test
      catches the common slip, not every one).
    - **Folders and tool kinds are data, shown in words.** `read` and `tools` are checked by Collide for any agent
      that flags cannot cut down (0.3). The page says "can read these folders, cannot write or run commands". `tools`
      accepts only `read` and `search` in v0, and any other kind is a load error that names the key. For an ACP agent
      the page adds that its own settings decide what it can really do.
    - **Budget and timeout in words.** A budget on an entry that cannot enforce one is a load error. The page shows
      "budget: Claude Code only; ACP and External agents get a timeout only".
    - **Test this agent.** A button that runs the smallest ask: the agent is told to call `query` once for one task
      and say done. It changes nothing on the screen. The result shows whether the endpoint was reached with the
      ask's token, the time to the first tool call, the finish time, the cost (Claude) or "not reported" (ACP), the
      readable folders and tool kinds in effect, and a plain failure text, such as `claude was not found on PATH`
      or `the agent did not call a tool within 180 s`. An untested entry shows "not tested".
      - **The probe for confinement** (R5). Test also asks the agent to write a file and to run `true`, both in a
        temporary directory made for the test, and checks whether either happened. If so, the result is **red**:
        "this agent is not confined; its own settings let it write and run commands". A red agent is refused as an
        ask agent until the person acknowledges it, the acknowledgement is recorded with the entry, and the ask box
        shows a standing warning while it is in use. `query` withholds untrusted bodies from a red agent (0.11).
        opencode's default is red (0.3).
    - **Endpoint card.** The card shows the endpoint URL, whether a window is connected, the time and name of the
      last call, and the endpoint token, masked, with Reveal, Copy and **Rotate** (0.2). Each agent row shows when it
      was last used and its spend this week, from the ask records. An external client's name comes from its
      `initialize` message and is shown as unverified, with the time it was last seen.
    - **Snippets for external clients are generated** from the real endpoint and token, never written by hand: the
      `claude mcp add --transport http collide <url> --header "Authorization: Bearer <token>"` command, a JSON
      `mcpServers` block, and an opencode `mcp` block. A client that cannot send headers gets the URL with the token
      as its last path segment. Collide writes to no agent's configuration. S6 measured the forms without a token
      (opencode's `mcp list` reported the endpoint connected); the header forms are inferred, and Stage 6's opt-in
      client test measures them. A snippet that a user pastes into a client's own config puts the token in a file
      Collide does not control, and the card says so beside the Rotate button.
    - **Interactive launches get the same MCP rule as runs** (R5). A New session, a take-over and a Plan… session
      (0.11) pass the snapshot's `--mcp-config` with `--strict-mcp-config`, so a worktree's `.mcp.json` servers that
      nobody approved do not load (measured S1 for plain `claude` in a worktree). The endpoint token is never in
      that file; the interactive session is given no UI-tools endpoint. The endpoint's other risk is a client the
      user pasted a snippet into, so the endpoint has a per-client call limit for `external` callers, counted in the
      shell (0.2), with its number set in Stage 6. There is one endpoint token for now, rotatable in Settings. Named
      per-client tokens are not built (owner, 2026-11-14).
- **Pack data.** Each pack gets a data directory per board under Collide's data directory, passed as
  `COLLIDE_PACK_DATA` and `ctx.dataDir`. That is where "a pack keeps its own files" (0.5). Each experiment arm gets
  its own copy, taken at the start, so a pack that keeps its state there is isolated without declaring anything
  (0.14).
- **Paths in `pack.yaml`.** Script, prompt and definition paths are relative to the pack directory. `repo:` marks a
  path in the line's snapshot (`repo:.claude/agents/builder.md`). Globs (`record.paths`, `protectedPaths`,
  `unchanged`) match repo paths.
- **No publishing needed** (owner, 2026-11-13). Most packs are built for a team, or for development in their own
  repo, and are used from the repo they live in. Committed, such a pack is a `repo:` pack. On a branch, it loads on
  that branch's line (0.12). Uncommitted, it runs as a working copy (below). Publishing, through a `git:` URL now and
  npm later, is for sharing a pack across repos. Developers may publish, and are never required to.
- **Pack sources** (owner, 2026-11-13). A source decides only how a pack's files arrive. Every source resolves to a
  git tree id, so the cache (0.6), the refusals, trust (0.9) and the fingerprint (0.13) share one code path.

  | source | example | pinned by |
  |---|---|---|
  | `repo:` | `repo:.collide/packs/orchard`, in the line's snapshot | the tree of `<ref>:<path>`, computed at load; no lock entry |
  | `git:` | `git:github.com/org/ferry-pack//pack@v1.2`; any git URL, a repo on disk included (`git:~/dev/ferry-pack//pack@main`) | the commit, and the tree at the path |
  | `dev:` | `dev:~/dev/ferry-pack`, a folder being developed; per user only | nothing: re-hashed at each load |

  - `git:` fetches into a bare mirror under `~/Library/Application Support/Collide/cache/git/` with the user's own git
    credentials, and reads the tree from git objects with the same reader as `repo:`. GitHub, GitLab and self-hosted
    servers all work. A tag is a version.
  - **Fetch rules** (R5). A fetch is a command that talks to a host the pack's author chose, so:
    - the first fetch from a host Collide has not fetched from before asks the user, naming the host;
    - git runs with `GIT_TERMINAL_PROMPT=0`, an allowlist of protocols (`https`, `ssh` and `file`), `--` before the
      URL, and a timeout;
    - a URL that starts with `-` is refused at load, because git would read it as a flag.
  - A `dev:` folder's tree id is computed as git computes it, so the same files give the same id as their commit.
    `board.yaml` refuses `dev:`.
  - The Orchard pack is `repo:` in the Orchard repo (`.collide/packs/orchard/`). The Collide repo keeps a fixture
    copy for its tests.
  - Package managers (npm) and a pack index are deferred. The lock's `source` field leaves room for them.
- **Repo packs and user packs** (owner, 2026-11-13):
  - Repo packs are listed in `board.yaml`, and `git:` ones are pinned in `.collide/packs.lock`. They are shared with
    the team, per line (0.12), from `repo:` and `git:` sources.
  - User packs are added once (`collide pack add <source>`) to `~/.config/collide/packs.yaml` and pinned in
    `~/.config/collide/packs.lock`. They are available on every board and switched on per board in the user's own
    board settings, from `git:` and `dev:` sources. That is how a pack kept in its own repo reaches a repo that does
    not vendor it.
  - Pack order: the board's packs, then the user's.
  - A user pack never replaces a repo pack. The same name is a load error that names both, so the team's gates and
    reviewers stay the team's. The exception is for pack authors: `collide pack link <name> [<folder>]` overrides a
    repo pack with a `dev:` folder, for one user on one board. The override is badged in the preview, in Tracks and
    in the fingerprint, and experiments refuse it.
- **Working copies** (owner, 2026-11-13). Developing a pack in the repo you work in takes no extra step:
  - `collide pack link <name>` with no folder uses the pack's folder in the user's own checkout, the default line's
    root;
  - the board offers **Use working copy** on any repo pack whose folder in that checkout differs from the line's
    ref;
  - a new folder under `.collide/packs/` in that checkout is offered as a working copy, with no `board.yaml` edit;
  - a working copy is a `dev:` pack: hashed at each load, badged, recorded in the fingerprint, refused in
    experiments;
  - an integrate writes into that checkout, so it can bring an agent's edit into a working copy. When it does, the
    next run that would load that working copy waits in the Queue for an acknowledgement that shows the diff (0.9).
- **Switching packs** (owner, 2026-11-13):
  - User packs switch on and off per board (`collide pack enable|disable`). Repo packs switch through `board.yaml`,
    a line (0.12) or a repo profile.
  - `collide pack update <name> [--to <ref>] [--yes]` moves a `git:` pack. It previews the old and new commit and the
    diff of the pack's manifest, workflow graph and hooks, writes the lock only on a yes, and asks for trust again. A
    host never fetched from is asked about first. For a `git:` repo pack it changes the lock, which the user commits.
    A `repo:` pack changes by a commit alone. A `git:` URL with a password in it is refused: a fetch uses the
    person's git credentials (ts-0100-pu).
  - A run pins every pack's tree id at launch, so nothing switched later reaches a live run.
  - An experiment arm can put one pack at another ref (0.14).
- **Profiles** (owner, 2026-11-13): a named setup for a kind of job.

  ```yaml
  # .collide/profiles/small-fix.yaml
  description: small bug fixes, one review round, a cheaper strong model
  packs: { off: [codemap] }                    # a repo profile switches repo packs; a user profile, user packs
  workflow: collide/review                     # the default workflow under this profile
  launch: { maxRounds: 1 }
  models: { strong: sonnet }                   # alias → model name; the model source and account stay the user's
  roles:
    collide/builder: { compact_at: 120000, config: none }   # context options (0.3)
  config:
    orchard: { lanePool: 1 }
  ```

  - Profiles live in `.collide/profiles/` (in the line's snapshot and the trust hash), in
    `~/.config/collide/profiles/`, or in a pack's `profiles:` (`ferry/greenfield`).
  - **Which profile a run gets**, first match:
    1. the one picked in the run preview;
    2. the view's `profile:`;
    3. the board's `profiles.rules`, each `{ when: <CEL over the task>, profile: <name> }`;
    4. the board's `profiles.default`;
    5. none: the settings as they are.
  - A profile is the top settings layer (above).
    - A repo profile may set any key and switch repo packs.
    - A pack's profile covers only that pack's own settings and workflows.
    - A user profile switches only user packs and sets only `overridable` keys. Anything else is refused, naming the
      key.
  - The preview shows the profile, why it applies, and each resolved value with its layer (0.11).
  - Each run's fingerprint carries its profile (0.13). So "where time goes" and run comparisons can be split by
    profile and task label (inferred, no verdict), and an experiment whose arms are two profiles gives the verdict
    (0.14).
- **Pack code, agent definitions, `board.yaml` and committed views load from a snapshot of the line's ref, never
  from a workspace.**
  - Loading resolves the pack's tree id before reading anything: at the line's ref for a `repo:` pack, and from the
    lock for a `git:` pack. Files are read from git objects, never with `git archive`, which changed content or let a
    symlink out of the pack in S4. Symlinks and submodules are refused, and the result is re-hashed against the tree
    id.
  - An agent on a branch must not be able to edit its own gates, its decision scripts, its reviewer or the board's
    views.
  - Agent definitions reach Claude as an `--agents` file built from that snapshot. They are selected with
    `--agent collide-<pack>-<role>`.
  - A run pins the snapshot commit at launch.
  - A `git:` pack loads from its pinned tree in the cache, through the same reader. A `dev:` pack, a working
    copy included, is the one exception: it loads from its folder, hashed at each load (above).
- **Repo code that a provider or hook runs** (Orchard's `./plan`, which runs the plan CLI from a checkout) runs from a
  line root, never from a workspace. A line root is a checkout the user opened. Workspaces are branches that agents
  edit.
- **Workflow expressions** (`when:`) are CEL, the same language as views. There is no second expression language.
- **A workflow is a declared transition table.** The engine, the run preview, the critical path, `pack test` and
  experiments can all read the graph, because no transition is hidden inside a script.
  - **Phase kinds:**
    - `agent`: one role;
    - `fanout`: one role on several models, or `instances:`, each naming any of a model, an agent definition, a
      profile and a prompt (owner, 2026-11-13, for planning). Two identical instances are refused at launch;
    - `script`: a hook in the workspace (push a branch, open a PR, sync with the base). `agentWork: true` marks one
      that runs agents itself (0.3);
    - `wait`: a hook polled every `interval:` until it returns `ok` or `fail`, or `timeout:` passes (CI checks). The
      hook may answer `pending`, with an optional `retryAfter` (0.5), which keeps it polling and replaces its entry
      in run state. `interval:` defaults to 30 s and `timeout:` is required, both shown in the run preview. A timeout
      while still pending is `error`, and the timeout is measured on the uptime clock (0.2). No agent runs while it
      waits, and the time counts as waiting;
    - `human`: a Queue item where a person picks one of the phase's declared tokens.
  - **`on_cold:`** on an `agent` phase: `resume` (default) or `fresh` (0.3). A role's context options (`compact_at`,
    `config`, `bash_output_max`, `mcp_output_max`) sit beside its `model` and `budget`.
  - **`then:`** says what follows a phase. Without it, the next phase in declaration order follows, and the last
    phase ends the run with outcome `done`. Otherwise it is one of:
    - a phase name;
    - `{ end: <outcome> }`;
    - `ask_human`;
    - a decision: `{ script, on: { <token>: <target> } }`. The script returns one declared token per task, and each
      token maps to a phase, an end or `ask_human`. An undeclared token makes the decision `error`.
  - **`round`** starts at 1 and goes up by one each time the run re-enters a phase it has already run. Orchard's
    fix → brief edge makes the second review `r2`, after `fix1`.
  - **Outcomes and terminal states.** A run ends `ended(<outcome>)`, `failed` or `stopped`. `outcome` is the `end`
    token it reached, and `on_end`'s `when:` reads it.
  - **`ask_human`** pauses the run in the Queue with its cause. The person picks one of the decision's tokens or
    stops the run, so a human answer follows the same table.
  - **`show:`** (owner, 2026-11-13), on a `human` phase or on a decision that can reach `ask_human`, lists what the
    person should read: phase artifacts (`adjudicate/round-record.md`) and run-state outputs (`github/open-pr.url`).
    The Queue item renders them as the drill-down renders a phase report. Orchard's `stop` needs the round record,
    and an issue → PR → CI `human` phase needs the PR link and the CI summary.
  - **Adopting.** A run can start at a named phase and round, in an existing workspace, so a chain already in
    flight (a task at its second review) moves into Collide without starting over. The preview lists the history
    Collide does not have.
  - **`accepts`** says which tasks a workflow can take: the sources and the template fields it needs. The run
    preview names, per task, the field or operation it lacks. A GitHub issue has no `labelId`, so it cannot launch
    under `orchard/loop`, and a mixed view's group launch says so per task.
  - **Workflow ops address status categories** (`setStatus { category: active }`). Each provider maps a category to
    the status it writes (0.10), so one workflow's ops work for any source it accepts.

Example: the Orchard pack (Stages 7–10, with `metrics` and `experiment` used from Stages 13–15).

```yaml
apiVersion: collide/v0
name: orchard
version: 0.1.0
requires:
  sdk: ">=0.1 <0.2"
  tools:
    - { name: node, version: ">=22.18" }
    - { name: pnpm }
    - { name: rush }
  agents: [claude]                             # Claude-bound: see 0.3
  modelName: "^[a-z]+$"                        # iter labels end in [a-z]+ (0.3)

launch:                                        # asked in the run preview; reaches prompts and hooks as `launch`
  review_budget: { type: integer, minimum: 1 } # its stop rules: the build brief carries it, approved by the owner

workspaces:                                    # named implementations (0.4); cwd = line root; tasks[] on stdin
  worktree:                                    # every hook returns { path, gitDir } (0.1)
    create:                                                             # `dev worktree new`; name includes run.slug
      script: hooks/ws-create.ts
      timeout: 120m
      grace: 60s
      outputs: { mapCommit: { type: string } } # workspace state (0.5): the fingerprint's cache state reads it
    sandbox: { script: hooks/ws-sandbox.ts, timeout: 120m }             # detached, installed worktree for mutants
    line:    { script: hooks/ws-line.ts,    timeout: 120m }             # a line root or an experiment root
    resume:  { script: hooks/ws-resume.ts,  timeout: 90m }              # `doctor --fix`, then `iter map-build`
    doctor:  { script: hooks/ws-doctor.ts,  timeout: 5m }               # checks workspace state and rewrites it
    identify: { script: hooks/ws-identify.ts, timeout: 1m }             # a worktree an agent made: its tasks (0.4)
    release:  { script: hooks/ws-release.ts,  timeout: 15m }            # a person asks to remove an adopted one
    remove:  { script: hooks/ws-remove.ts,  timeout: 15m }              # task workspaces, once the task is done at base
    discard: { script: hooks/ws-discard.ts, timeout: 15m }              # sandbox and experiment kinds: lanes down,
                                                                        # `git worktree remove`, delete the branch
    cleanup: { script: hooks/lanes-down.ts, timeout: 5m }               # after interrupted / timed_out

integrations:
  merge-local: { script: hooks/merge-local.ts }  # local merge into the base ref, as the lead does today

record:                                        # commits at the line root, under its lock
  paths: [arch/**, .agents/review-feedback/**] # an upper bound: only the files a write or a phase touched
  script: hooks/record.ts                      # places phase outputs, runs `plan generate`, commits

protectedPaths: [.agents/**, tools/dev/**, tools/plan/**]   # flagged in review

metrics:
  import: { script: metrics/iter-timings.ts }  # iter lines from this run's worktrees (or the arm's file) → spans
  processes: { script: metrics/lanes.ts }      # a workspace's lane containers and host server pids (from its lane
                                               # records); the clone label alone spans all worktrees

experiment:                                    # per-arm isolation for state the clone shares (0.14)
  prepare: { script: hooks/exp-prepare.ts }    # copies the current codemaps into arm.dir
  branchPrefix: scratch/collide-exp/           # the prototype board still lights `doing` tasks on these checkouts
                                               # (its live view); accepted: an experiment's task shows as lit
  env:
    ORCHARD_ITER_TIMINGS: "{{ arm.dir }}/iter-timings.jsonl"
    ORCHARD_CODEMAPS_DIR: "{{ arm.dir }}/codemaps"
  appendOnly:                                  # other work appends here during an experiment
    - "{{ git.commonDir }}/orchard-dev/iter-timings.jsonl"
    - "{{ git.commonDir }}/orchard-dev/iter-shadow.jsonl"
    - "{{ git.commonDir }}/orchard-dev/agents.jsonl"
  unchanged: ["{{ git.commonDir }}/orchard-dev/codemaps"]

roles:
  builder:
    agent: claude
    definition: repo:.claude/agents/builder.md # from the snapshot; passed as collide-orchard-builder
    model: fast                                # alias, resolved per user
    budget: { usd: 20 }
    env: { ORCHARD_ITER_AGENT: "{{ task.labelId }}-{{ phase.label }}" }   # labelId: a plan-provider template field
  reviewer:
    agent: claude
    definition: repo:.claude/agents/reviewer.md
    model: strong
    readOnly: true                             # Collide checks HEAD and status unchanged outside phase.dir
    env:
      ORCHARD_ITER_AGENT: "{{ task.labelId }}-{{ phase.label }}"
      ORCHARD_ITER_TIMINGS: "{{ phase.dir }}/timings.jsonl"
  orchestrator:
    agent: claude
    definition: agents/orchestrator.md         # pack file: writes briefs, adjudicates, writes the round record
    model: strong
    readOnly: true                             # writes only to phase.dir; `record` commits its outputs
  lead:                                        # Orchard's own orchestrating session (revision 22)
    agent: claude
    model: strong
    start: root                                # the default with agentWorktrees, written out (0.4)
    env: { ORCHARD_ITER_AGENT: "{{ run.slug }}-lead" }   # `dev worktree new` refuses without a label

prompts:
  build: briefs/build.md.j2                    # carries launch.review_budget; `iter timings --write` gets --run
  review-brief: briefs/review-brief.md.j2      # instructs the orchestrator to write a fresh brief per round
  adjudicate: briefs/adjudicate.md.j2
  session: briefs/session.md.j2                # the group's tasks; the repository's own rules do the rest

gates:
  iter-evidence:                               # checks the agent's own iter run; does not re-run it
    script: gates/iter-evidence.ts             # label, stage, exit 0, HEAD == phase end HEAD

workflows:
  loop:
    accepts: { sources: [plan], fields: [labelId] }
    workspace: orchard/worktree
    integrate: orchard/merge-local
    on_start: [{ op: setStatus, args: { category: active } }]                      # plan writes `doing`; then record
    on_end:   [{ op: setStatus, args: { category: review }, when: 'outcome == "closed"' }]   # `in_review`
    start: build
    phases:                                    # without `then:`, the next phase in this order follows
      build:      { role: builder, prompt: build, label: build, gates: [iter-evidence] }
      brief:                                   # its adversarial brief template §§2–4: ranked claims, completeness list,
        role: orchestrator                     # its review-bundle script, archived copy
        prompt: review-brief
        outputs: [review-brief.md]
        record: true
      review:
        role: reviewer
        prompt: { artifact: brief/review-brief.md }
        label: "r{{ round }}-{{ model.name }}"
        sandbox: { models: [strong] }          # optional, per model: see below
        fanout: { models: [strong, fast], when: "round == 1" }   # two reviewers, identical briefs
      adjudicate:
        role: orchestrator
        prompt: adjudicate
        outputs: [round-record.md, fix-brief.md]
        record: true
        then:                                  # a decision: reads the round record's **Next** line
          script: workflow/next.ts
          on: { fix: fix, close: close, stop: ask_human }
      fix:   { role: builder, prompt: { artifact: adjudicate/fix-brief.md }, label: "fix{{ round }}", gates: [iter-evidence], then: brief }
      close: { role: builder, prompt: { artifact: adjudicate/fix-brief.md }, label: close, gates: [iter-evidence], then: { end: closed } }
  session:                                     # Orchard's own orchestrating session (revision 22)
    accepts: { sources: [plan], fields: [labelId] }
    group: whole                               # one run given the whole group (Stage 11)
    agentWorktrees: true                       # roles start at the root; landing defaults to observed (0.4)
    adopt: orchard/worktree                    # its `identify` and `release`; nothing is provisioned
    integrate: orchard/merge-local             # for work that never lands, with accept and the gates
    phases:
      lead: { role: lead, prompt: session }    # makes its own worktrees, and merges them

providers:
  plan: { script: providers/plan.ts }          # scope: ref; parses arch/ directly; writes via a line root's ./plan
```

Why the lead's actions are hooks and workflow ops: in orchard the lead, not the builder, does four things
(measured R2):
- **Moves status.** Build briefs say "Do not set `in_review`; the lead does".
- **Commits the round records on the base branch.**
- **Merges the chain locally.** 14 of 715 merges on the main line since 2026-10-15 mention a pull request.
- **Sets the review budget in the brief.** `review_budget` appears in no task's frontmatter.

Collide plays the lead, so it does all four, through the pack.

**Review sandboxes** are optional and set per model.
- Its task guide says "There are no review worktrees": a reviewer works read-only in the build worktree.
- The two newest briefs do give the opus
  reviewer a "detached, disposable worktree … installed and built" for mutants (measured).
- The pack follows the newest practice.
- `dev worktree rm` refuses detached worktrees, with or without `--force` (measured: the dev CLI README). So `discard`
  removes sandboxes with `git worktree remove`.

**Keys added for planning** (owner, 2026-11-13):
- A run takes `budget: { usd }` as a total. At the total, the run goes to `ask_human` with cause budget.
- `views:`, `checklists:` and `points:` are pack keys. A board adopts a pack's views by name.
- `collide/plan`'s settings live in `.collide/config.yaml`:
  - `layout` (`root`, `paths.<kind>`, `code`, `previous`), which is not `overridable`;
  - `roles`, the kind-to-role mapping, which a provider's role tags fill by default.

### 0.8 Data model (core)

- **App:** several boards, one line selector over all their lines (0.11).
- **Board:** repo root, lines (0.12; the default line is the base ref at the board root), source configuration,
  saved views.
- **Line:** ref, root checkout, the snapshot commit used by each run, applied packs, default workflow, trust entry,
  line lock. `record` refuses to commit when a line root is not on its line's ref.
- **Task (cache):**
  - key `source:shortId`, plus template fields, aliases and, for ref-scoped sources, the path in each checkout;
  - title, body, status + category, deps, labels, collections, source fields;
  - provenance: author and trust level;
  - `kind`, typed `links` (`type`, `to`, `blocking`), `owners` (the people or agents who claimed the item; 0.10 says
    how a claim works; R5 aligned this with 0.10, which already said `owners`), and checklist fields (owner,
    2026-11-13, for planning);
  - on a proposal, `features` (revision 14): each feature's `F<n>` id, title, section (empty when unsorted) and
    summary, read from the body's Features part, and `featureProblems`: each line of that part that did not parse,
    with its reason (0.10). A link's `to` may name a feature, as `pr-0003-km#F2`;
  - on a proposal (revision 18), `decisions`, `assumptions` and `findings`, read from those parts of the body
    (0.10), with `partProblems` for the lines that did not parse, and `coverage`, derived: its features that no
    task implements and its included tasks that implement none;
  - `version`, required.

  See 0.10.
- **Run:**
  - `tasks[]` with per-task membership, path at launch and status;
  - line and snapshot commit, workflow, `launch` values, workspace and the implementations it resolved (0.4);
  - `chain.base`, rounds;
  - run state (0.5);
  - its terminal state (`ended(<outcome>)`, `failed`, `stopped`) and any pause with its cause (`ask_human`, usage
    limit);
  - fingerprint (0.13), annotations, and its experiment arm, if any (0.14).

  There is one live run per task key, except inside an experiment.
- **Phase:**
  - kind (`agent`, `fanout`, `script`, `wait`, `human`), round, instances (one per fanout model or instance), role,
    session ids;
  - `start_head`, `end_head`, `dir` (a scratch directory outside the worktree);
  - sandbox workspaces, declared outputs, final reports, gate results, labels;
  - `ask_human` events with their cause;
  - a resumed phase keeps its key and directory, and the earlier attempt's files move to `attempt-<n>/`.
- **History:** prompts and hooks see `history[]` (earlier phases: kind, round, heads, artifact paths, verdicts) and
  `round.base` / `round.head` / `chain.base`.
- **Workspace:** a git checkout (`path`, `gitDir`; 0.1). It lives longer than any run, and keeps its workspace state
  (0.5). Kinds:
  - `task`;
  - `sandbox`;
  - `line` (a line root);
  - `experiment` (an experiment root, or an arm's task workspace);
  - `adopted` (a worktree an agent made during a run, 0.4). Collide neither provisions nor removes it. It is `in_use`
    while a run works in it, `idle` after, `prunable` once its folder is gone, and `removed` once `git worktree
    list` no longer has it.

  Lifecycle:
  - States: `provisioning → ready → in_use → idle → removable → removed`, plus `failed`, and `prunable` for an
    adopted workspace whose folder is gone. `failed` resumes through the `resume` hook.
  - A reaper removes `task` workspaces once their tasks are done at the base ref (committed by `record`, merged by
    `integrate`), and reports disk use.
  - `sandbox` workspaces are discarded when their phase ends. An arm's task workspaces and sandboxes are discarded
    once that arm's metrics are captured; experiment roots go when the experiment ends (0.14).
- **Event log:** append-only, the source for timeline, timings, recovery and observers. Its event types come from
  one catalogue (0.4).
- **Spans and samples:** the run's trace and its resource samples (0.13).
- **Experiment:** the task, its arms, the primary metric, the design (order, its seed, the fixed pair count),
  repetitions, the before/after inventory, and the runs it produced (0.14).

### 0.9 Untrusted input

- Task text is data. Prompts fence it as untrusted, with its provenance.
- Tasks from authors outside the repo's trust list (GitHub `author_association`) need a confirmation before a run,
  and never run in `bypassPermissions`. GitHub draft items have no author association and count as untrusted.
- Collide never acts automatically on events from untrusted sources.
- A diff that touches one of the protected files is flagged in review. The protected files are named (R5): the
  committed Collide files (`board.yaml`, `packs.lock`, `config.yaml`, `views/` and `profiles/`), `.claude/`, a pack
  path and a pack's `protectedPaths`. The `.collide/` directory as a whole is not protected, because the planning
  folders under it (`planning/`) are the files Collide's own agents write (0.10), and a flag on every diff would
  flag nothing.
  A pack may fail the phase at its end instead, checked like `readOnly`:
  `protectedPaths: { paths: [...], at: phase-end }` (owner, 2026-11-13; Ferry reverts such a write the moment it
  happens).
- **Trust:**
  - Opening a repo whose packs, providers, `board.yaml`, shared `config.yaml` or committed views are not yet trusted
    asks first.
  - Trust is stored outside the repo, in `~/.config/collide/trust.yaml`.
  - It is keyed by a hash over the tree ids of the repo's packs, the lock, the `.collide` pointer file if there is
    one, `board.yaml`, `config.yaml`, the views and the tree hashes of `protectedPaths`. It is asked again when that
    hash changes.
  - Each line has its own entry. A line whose packs or `protectedPaths` differ from the default line's shows the
    difference before it is trusted (0.12). The entry holds the hash and the inputs it was made from, and trusting
    a new hash replaces it, so going back to an older commit asks again. A `protectedPaths` glob is hashed at its
    folder before the first wildcard, or at the root when it starts with one (ts-0097-kw; Stage 7's matcher may
    narrow it).
  - The ref most worth testing is often one an agent wrote, and its dev CLI becomes that line's gate.
  - **Per pack source** (owner, 2026-11-13):
    - repo packs are covered by the repo's hash: a `repo:` pack by its tree id at the ref, a `git:` pack through the
      lock. So a committed pack change asks again, with the diff, and needs no lock edit. `.collide/profiles/` is in
      the hash too;
    - a user pack is trusted when it is added, keyed by its tree id. The prompt shows the source, the commit, the
      hooks, providers and observers it runs, and its `requires`. A new tree id asks again, with the diff (the
      second-pack stage);
    - a `dev:` pack is trusted per folder. A change does not ask again, but every run that loads it carries the dev
      badge and its tree id, and experiments refuse it. A `dev:` folder inside a Collide workspace is refused,
      because agents could edit their own pack.
    - a working copy that an integrate changed asks for an acknowledgement, with the diff, before the next run
      loads it (0.7).
- **YAML and frontmatter are untrusted input** (S5). The Rust loader and the TypeScript parser both cap the file
  size before parsing, reject anchors and duplicate keys, and check every field's type instead of coercing it
  (0.7, 0.10). `gray-matter` is never used: its default runs the JavaScript in a `---js` frontmatter fence (measured
  S5).
- **CEL runs on untrusted task data, and the `cel` crate has no cost, step or time limit** (S5; measured on 0.15.0,
  and none of the other Rust candidates found has one). CEL has no unbounded loops, but its macros nest
  superlinearly: the plan's nested example over 1,567 tasks took 3.4 s, and a triple nesting did not finish in 30 s.
  Collide adds:
  - **a step counter** in the vendored `cel` (0.15). It is a 34-line patch, one tick at the top of
    `Value::resolve_val`. Its overhead was within noise on ordinary views, and it stopped every runaway case
    measured. Collide offers the patch upstream;
  - **a per-task budget and a per-pass budget**. S5's suggested limits are 10,000 steps per task and 2,000,000 per
    view pass (inferred from its numbers; not tested as a set). A task that runs out is excluded and counted on the
    view (0.10);
  - **a 1 KB source cap.** It is a safety property, not a style one: a 4 KB chain of `+` overflows a 2 MB stack and
    aborts the process, and a deeply nested expression panics inside the parser's error formatter. Compile also runs
    under `catch_unwind`;
  - **list-length caps when a task is loaded, on the fields bound into CEL only** (R5): `labels`, `collections` and
    `depends_on` at 1,000 entries, and the bound string fields at about 4 KiB. The task's body is not bound into CEL,
    so a long body is never cut by this cap. Hiding the corpus is not enough: one task with a 5,000-entry `labels`
    list took 4.2 s under an innocent `all`/`exists` pair;
  - **no corpus variable in views**, and a static cost estimate when a view is saved or loaded (0.10).

  A thread with a deadline is not a limit: the caller moves on and the thread keeps running (measured S5: about one
  core until the expression ends).
- **An agent's tool inputs are untrusted** (owner, 2026-11-14; S6's in-app agent section). The ask agent reads task
  text from authors outside the repo and may be steered by it, so every call to the UI tools (0.11) is input, like a
  YAML file:
  - **Validated before anything is applied** against the window's live state: an object with no unknown keys, a known
    surface, known enumerations, known sort keys and phase names, a well-formed clock window. A call that fails
    changes nothing.
  - **Invented ids are rejected, never repaired.** A task or run id the window does not have rejects the whole call,
    and so does an id in two groups. Collide never picks the nearest id. The error names the ids and says what to do:
    `unknown task id T-01-20; nothing was shown. Call query for real ids.` S6 measured the rule on a weak model: it
    invented 8 ids and nothing wrong reached the screen, and in its two other runs it recovered from refused
    calls in 2 to 4 more calls.
  - **Limits.** A lens has 1 to 12 groups. Each group has a name, at least one id and a non-empty reason. A call
    names at most 500 ids, the size of the largest `query`, and its body is at most 1 MB (0.2). A title is cut at 80
    characters, a group name at 60, a reason at 600 and a note at 400. An ask makes at most 20 screen-changing calls,
    so a loop cannot flood the Undo stack (inferred: S6's asks made one or two). The shell counts them (0.2), so a
    window reload does not reset the count.
  - **The agent's words are text, and Collide's words say what changed.** A title, a group name, a reason or a note is
    shown as plain text, with no markdown or HTML and with control characters removed, and is marked as the agent's.
    The statement of what changed (the expression, the number of groups and ids) is built from the validated action,
    never from the agent's prose. S6's weak model described a grouping in prose that Collide had refused.
  - **Error messages tell the agent what to do.** They name the unknown value and list the known ones: `unknown
    surface "x" for set_view; supported: field, tracks`. They never hold a token or a path.
  - **`query` returns untrusted text.** A task's title and body come with its source and a `trusted` flag from the
    author rule above, and Collide's ask prompt says task text is data, never instructions. No tool acts, so such text
    can at worst mislead the view, and the chip, the flag in 0.11 and Undo answer that. An ACP agent's own tools are
    limited only by the permission mapping (0.3).
  - **The endpoint refuses before it parses** a foreign Host or Origin, a non-JSON content type, an oversized body and
    a wrong token (0.2). It counts wrong tokens and never records their value.
- **The webview shows untrusted text, so it is locked down** (R5). Task bodies, issue text, an agent's words and
  planning files are written by people outside the repo, and the webview holds the app's whole `rpc` surface:
  - a strict Content-Security-Policy: scripts, styles and images only from the bundle, `connect-src` only the app's
    own channel, no `unsafe-inline` and no `unsafe-eval`;
  - markdown is rendered through a sanitizer with raw HTML turned off. `javascript:`, `file:` and `data:` links are
    removed, and an external link opens in the operating system's browser after a confirmation that shows the
    address, never inside the webview;
  - Tauri's capability set for the window is the `rpc` command and the one event channel, and nothing else: no
    filesystem, shell or HTTP plugin;
  - Stage 6 tests it with a hostile issue (script tags, event-handler attributes, `javascript:` links, a markdown
    image that points at a remote host) and checks that nothing runs and nothing loads.
- Hook outputs and report lines can come from processes running branch code, because a gate runs the branch's
  tools. They are data: never templated into prompts, and passed to env only as safe tokens (0.5).
- Metrics hold no prompt, task or tool content. By default they keep only executable names and flag names (0.13).
- Collide never answers or pre-accepts Claude's own folder-trust prompt, and never writes Claude's settings to do
  so. The terminal shows the prompt and the user answers (0.11).
- Notifications and the menu bar use Collide's own words: the board, workflow, run id and kind of item. Task titles
  appear only with a setting on, because outside authors write them and notifications show on the lock screen
  (0.11).

### 0.10 Task sources, corpus and views

A board's **corpus** is the union of the tasks from its sources. **Views** are named subsets of the corpus:
projects, MVP scenarios. Sources are adapters (the providers of 0.4) that read and write tasks in their own way.

Two first-party adapters, both built in Milestone A, so that two real implementations shape the protocol before
anything else depends on it:

| adapter | scope | reads | writes (v0) | live updates |
|---|---|---|---|---|
| `github` (built-in pack) | global | Issues (REST); Projects v2 items and fields (GraphQL) | `setStatus`, `comment` | REST `since` + ETag; GraphQL at a measured interval; a periodic full reconcile |
| `plan` (Orchard pack) | ref | parses task-file frontmatter in each checkout's `arch/` | `setStatus` (`plan set`), `create` (`plan new task`), via `./plan --root <checkout>/arch` run from the root of the line that owns the checkout; never commits | file watch on each checkout's `arch/` and HEAD, debounced, plus a 10 s recheck |

A third source is built in and needs no setup: **scratch tasks** (0.1). They live in Collide's store, one list per
user per board, and are not ref-scoped. A prompt becomes a task, its first line the title. Their statuses are named
for the categories (`ready`, `active`, `review`, `done`, `dropped`) and declared like any source's, so nothing
special-cases them (revision 23); operations `create` and `setStatus`. A scratch task's key is `scratch:<n>`,
counted within its board, and a write that asks to commit is refused (0.1). Moving a scratch task into another
source is deferred.

Why the plan adapter parses files itself:
- Measured R2: `./plan tasks --format json` returns only `id, status, proposal, epic, title, file`. It returns no
  `depends_on`, body or `reviewed_by`, for 1567 tasks in 0.96 s per call.
- Closure needs the dependencies, and a reparse per change per checkout is too slow.
- Parsing frontmatter, as the prototype board's generator does, gives the dependencies and reparses only the
  changed file.
- Writes still go through the plan CLI, so plan's own rules apply. These include refusing illegal transitions and
  regenerating epic tables.

**Protocol** (JSON-RPC over stdio):

- `initialize` receives the pack's settings (`config`, 0.7) and `root`, the main checkout's absolute path (owner,
  2026-11-14; a provider never infers it from its working directory), and returns:
  - `scope`, `idPattern` and `shortIdPattern`
  - `templateFields`: the name and regex of each field templates may use (0.5)
  - `statuses`, each with a category, and for each category the status a category write produces (plan:
    `active → doing`, `review → in_review`; GitHub: from the status mapping in `board.yaml`)
  - `fields`: id, type, filterable
  - `ops`, each with a JSON Schema for its arguments and the checkouts it may target
  - capabilities: `watch`, `create`, `collections`, and the optional `preview`
  - `kinds` (owner, 2026-11-13), each with its statuses and their categories, its transitions, its ops (each with
    the category it moves to, if any) and optional role tags, which core treats as plain strings. A provider with
    one kind may omit `kinds`
  - `links`: the link types it holds, their direction, whether they block, and whether their target is an item or a
    feature (revision 14)
- `list({ checkout?, cursor? })` returns tasks, collections and `invalid[]`, each item it could not read as
  `{path, checkout, reason}`, the shape of `task.invalid` (owner, 2026-11-14). `get({ id, checkout? })` returns the
  task, or no task when no item has that id: a missing id is an answer, not an error (owner, 2026-11-14).
- `watch({ checkouts })` sends notifications: `task.collision`, `source.stale`, and (owner, 2026-11-13; S11):
  - `task.changed {key, version, from, to, diff, cause, actor?, automated?, checkout?}`, where `cause` is
    `head | worktree | remote | journal | startup`, `actor` is the source's own when it is known (GitHub events,
    the beads journal), `automated` is GitHub's `wasAutomated`, and `checkout` names the checkout whose copy
    changed, absent for a global source (owner, 2026-11-14);
  - `task.renamed {from, to, checkout}`, with history, runs and `latestRun` following the key;
  - `task.removed {key, cause, checkout?}`, where `cause` is `deleted | transferred | unknown` and `checkout` is as
    in `task.changed`;
  - `task.invalid {path, checkout, reason}`, which core turns into a Queue item.
- On start, core reconciles each source against its persisted cache and emits one change per item, with
  `cause: startup`.
- `apply({ op, id?, args, version, checkout? })` returns one of:
  - `ok`, with the new task, its version, and the files the write touched (what `record` commits). It also returns
    `items[]`: every item the op changed, with its version, from and to
  - `conflict`, with the current task
  - `refused`, with the source's message, e.g. plan's "illegal transition"
- Optional capability `preview` takes `apply`'s arguments, writes nothing, and returns `apply`'s three outcomes
  (owner, 2026-11-14): `ok` with `items[]` and `files`, `conflict` with the current task, or `refused` with the
  source's message.
- The op name `move` is reserved for Share. plan's `move --epic` is declared as `rehome`.

**Versions.**
- Every task has a `version`. For plan, it is a hash of the task file's content.
- For GitHub it is per item:
  - the issue's `updated_at` for issue operations;
  - `ProjectV2Item.updatedAt` for Projects fields, because a field edit does not change the issue (inferred).
- GitHub has no conditional writes. A `conflict` is detected by re-reading before the write, so a short race window
  remains (stated, not hidden).

**Writes are operations, not field patches.**
- Each source names what it can do, with argument schemas that the UI renders as forms.
- The source enforces its own rules. Collide offers only the operations a source declares and the status
  transitions it lists.
- v0 operations:
  - plan: `setStatus`, `create`;
  - GitHub: `setStatus`, `comment`.
- `setStatus` takes a `status` or a `category`. Workflow ops use categories (0.7). Before every status write, the
  `task.before_status` point fires (0.4).
- Handoffs and `reviewed_by` stay with the builder and the pack, as in orchard today.

**Identity.**
- A task's key is `source:shortId` (`plan:T-01-042`, `github:acme/edge#212`).
- Long forms are aliases. For orchard that is the file stem: plan's JSON id is
  `T-01-042-e-02-<slug>`, while `depends_on` uses `T-01-042` (measured R2).
- The plan provider also supplies a `labelId` template field (`t01-042`) for orchard's labels (0.5).
- Ref-scoped sources identify a task in a checkout by (shortId, path).
  - The same shortId at different paths in two checkouts is a **collision**, shown as a collision, never as
    divergence.
  - Collisions are real in orchard: `plan new task` allocates ids per checkout and `plan renumber` reconciles them
    at merge. Measured: its id-map file (96 KB) exists.
- Launch resolves a task by (shortId, path) at the line's ref, and refuses on a collision.
- Renames come from its id map and git rename detection, never from a remove + add guess.
- A run records each task's path at launch, so a later renumber does not orphan it.

**Collections.**
- Sources expose their own groupings as collections, each with ordered groups:
  - orchard epics, with the slices and streams from the epic file's tables (as the prototype board reads them);
  - GitHub milestones and Projects.
- Every task carries `collections: [...]`. That is how a view reaches membership that is not a task field.

**Status categories.**
- Every source status maps to one of `backlog | ready | active | review | done | dropped`, so views, kanban columns
  and Tracks (0.11) work across sources. "Open" means not `done` or `dropped`.
- **Statuses are data; the six categories are fixed** (owner, 2026-11-15; revision 23). A source declares its own
  statuses in `initialize`, any names and any number, in its order, each with a category. No status name is written
  into core, the host, the shell or the app. Behavior reads the category: workflow ops, gates, "open", Up next and
  the reaper. The UI always shows the source's own status name, coloured by its category. Collide's own planning
  and scratch tasks declare theirs the same way.
- **Board columns** (`layout.groupBy`) are `status` or `category`.
  - `status` gives one column per declared status, in the source's order, empty ones too. It is the default when
    the view's tasks share one status list: one source and one kind.
  - `category` gives the six columns, each card showing its own status name. It is the default for a view that
    mixes sources or kinds.
  - A saved view can pick either.
- **A status no one mapped.** A Projects Status option missing from `board.yaml`'s status mapping shows under its
  own name, in a column of its own, and counts as `backlog`. A category op never writes it. The board's checks name
  it, with the fix: map it in `board.yaml`. A task whose status its source did not declare is listed with the
  source's invalid items, with the status named.
- Orchard: `specified → backlog`, `ready`, `doing → active`, `in_review → review`, `done`, `dropped`.
- GitHub: an issue's open/closed state (closed as not planned → `dropped`), or a Projects v2 Status field, mapped
  in `board.yaml`.

**Ref-scoped sources** (per-branch tasks, as in Orchard):
- One task has one state per checkout: line roots, workspaces, and other `git worktree list` entries that pass the
  board's checkout filter. The filter is board configuration: the Orchard repo's excludes `scratch/*`. (Its prototype
  board excludes nothing: measured R4.) Branches under a pack's experiment prefix are always
  excluded (0.14).
- The board shows the default line's state. Divergence chips compare a line's root with the workspaces forked from
  it. Other lines' states appear in the task drawer, so a long-lived line does not mark every task divergent.
- Every write names its checkout. The UI defaults to the selected line's root, and the CLI to the line whose
  checkout it runs in, else the default line (revision 22).
- **No writes into a checkout while a phase is running there**, except at a root that a phase starts at, where a
  write and its `record` are one step (0.4, revision 22). They queue until the phase ends. A builder stages
  by explicit path, so a Collide edit would otherwise ride into its commit. Writes into a phase's checkout are also
  what `readOnly` and `dev worktree rm` (which refuses dirty trees) would trip on.
- A write leaves uncommitted changes, which can be several files: `plan set` regenerates epic tables. The UI shows
  uncommitted task edits per checkout.
- **Committing.** Collide never commits task files itself. The pack's `record` hook does, under the line lock.
  - It commits exactly the files that `apply` reported or the phase output, never the whole glob. An unrelated edit
    of the user's under `arch/` stays uncommitted.
  - Workflow ops (`on_start`, `on_end`) record automatically.
  - Edits made in the UI record when the user presses Record.
- **Provider writes are safe against each other** (R5). The markdown provider takes a lock file in each root for every
  write, and writes each file by temp file, `fsync` and rename, so a reader never sees half a file and two writers never
  interleave. The lock file is created only when absent and is never seen without the writer's pid: the pid is written
  to a temp file, which is hard-linked into place, and the link fails when the lock exists. A waiter retries for up to
  15 s, and a lock whose pid is dead is taken over, never by two waiters at once (owner, 2026-11-14: Node has no
  `flock`, and a native add-on was not taken). A test runs concurrent `setStatus` calls on one file and checks that
  every call's result is true to what is on disk.
- **The planning slice checks out the main checkout's shared root by default** (R5; `--checkout` names another). A
  write there leaves an uncommitted file. `--commit` commits that one file and nothing else, as a stopgap until the
  `record` hook exists (Stage 7). A claim (`owners`) is refused when the file already shows an owner, unless the
  claimant is that owner.
- **Launch reads each task at the run's line ref, not the working tree.** A task with unrecorded edits must be
  recorded before launch. The run preview shows its status at that ref, and refuses a task that is not launchable
  there, with the reason.

**Views:**

```yaml
# .collide/views/edge-mvp.yaml   (committed: shared with the team)
# personal views: ~/.config/collide/boards/<board-id>/views/
name: MVP
where: '"plan:E-07" in collections || "mvp" in labels'   # CEL
include: [plan:T-01-042, github:acme/edge#212]
exclude: [plan:T-01-043]
closure: { goal: plan:T-01-042, follow: depends_on }
workflow: orchard/loop                                    # default for runs launched from this view
layout: { default: board, groupBy: category }
```

- **Membership = (where ∪ include ∪ closure) − exclude.**
- **Fields:**
  - Normalized fields are always present: `key`, `source`, `title`, `status`, `category`, `labels` (empty when a
    source has none), `collections`, `deps`, `author`.
  - Source-specific fields live under `fields.*` and are tested with `has(fields.x)`.
  - An evaluation error, or a task that runs out of its step budget (0.9), excludes the task and is counted on the
    view, not hidden.
  - **Views bind per-task fields only, never the corpus** (S5; owner, 2026-11-14). There is no `tasks` variable, so
    a view cannot compare one task with all the others.
  - **A static cost estimate runs when a view is saved and when it is loaded.** It walks the expression and
    multiplies each macro's iterations by the size of the list it ranges over (S5: it never underestimated, and
    overestimated by 1.1 to 11 times). A view whose estimate passes the per-pass budget is refused, with the
    estimate shown (inferred from S5's "this view is cubic in the corpus: refused"). It bounds only expressions
    whose list sizes it knows, so it adds to the runtime budget and never replaces it. The source is capped at 1 KB
    (0.9). `matches` recompiles its regex on every call and costs about 20 times a plain test, so the view editor
    says so.
- **`closure`** is the prototype board's run order. Its set and its steps are identical to the prototype board's
  generator's for the same goal at the same commit: it walks open tasks only, and it handles the goal the same way.
  - R2 measured why the open-only rule matters: walking every path adds three stranded tasks (T-01-044, T-01-045,
    T-01-046).
- A view is a lens owned by Collide. It never writes membership into a source. Where the source has the grouping (a
  collection), the view reads it, so membership stays the source's truth. `include` and `exclude` cover the
  hand-picked subsets the source does not encode.
- A view's `workflow:` resolves on the line it launches on. It is an error if that line lacks the workflow.
- Launching from a view starts a group run of its `ready` tasks (Stage 11). A dependent starts only once its
  dependencies are done at the base ref, merged by `integrate`, not merely started earlier.

**Planning sources** (owner, 2026-11-13; `docs/research/planning.md`):
- **A built-in markdown provider** in `collide/plan`, for a repo with no planning system:
  - one file per item, identified by its frontmatter `id`, never its path. A spike is a folder;
  - **layout** (owner, 2026-11-13), the same in both roots:

    ```
    epics/ep-002-va-<slug>/epic.md
                          /proposals/pr-0003-km-<slug>.md
                          /questions/qu-0007-bx-<slug>.md
                          /tasks/ts-0012-ua-<slug>.md
    spikes/sp-0005-rt-<slug>/          record.md, scripts, out/, instances/
    ```

    - the provider reads only these paths: `epic.md`, the `.md` files directly in `proposals/`, `questions/` and
      `tasks/`, and each spike's `record.md`. It never walks the rest of a spike folder, which can hold
      dependencies and gigabytes of run output, and `watch` ignores it too;

    - the folder is the epic membership. The provider reports `part-of` from the path, so frontmatter has no
      `part-of` and the two cannot disagree. Moving a file into another epic's folder moves the item, and its links
      do not change. An item with no epic yet lives in `epics/ep-000-<suffix>-general/`;
    - spikes sit at the top level, because one spike often informs several epics. Its links name them;
  - **kinds** (owner, 2026-11-13): `epic`, `proposal` (formerly a session), `question`, `task` and `spike`.
    - A proposal holds its decisions in its Decision section. Its statuses (owner, 2026-11-14) merge planning.md's
      session and decision roles: `draft` (`active`) while it is written; `proposed` (`review`) while it waits for
      the owner; `accepted` (`done`), with `by: human:<date>`; `deferred` (`backlog`), with its trigger, until a
      person returns it to `proposed`; `rejected` and `superseded` (`dropped`).
    - **A proposal's features** (owner, 2026-11-14; revision 14). Planning is mostly features before there are
      tasks, so a proposal holds its feature set in a fixed `## Features` part of its body, which Collide reads:

      ```markdown
      ## Features

      ### Packs
      - **F1 Trust prompt for repo packs**: a repo pack asks before its first run.
        A continuation line is indented by two spaces.
      - **F2 Pack lock**: `packs.lock` pins each pack to a tree.

      ### Unsorted
      - **F3 Menu bar badge**: the Queue count in the menu bar.
      ```

      - only the part from `## Features` to the next `##` heading is read. In it are blank lines, `### <section>`
        headings and entries. An entry is `- **F<n> <title>**: <summary>`, and its continuation lines are indented by
        at least two spaces. Anything else in the part is a problem;
      - the `###` heading is the section: 1 to 40 characters once trimmed, with no `#`. An entry under `### Unsorted`
        (in any case), or above the first heading, has an empty section;
      - `F<n>` is unique in the proposal and counts from 1. A removed feature's number is not reused, so a link never
        comes to mean another feature. A link to a number the proposal does not have is a broken link;
      - a proposal needs no Features part. A problem in a `draft` is reported on the item (`featureProblems`, 0.8),
        never as a `task.invalid`, because a draft is still being written. **The move to `proposed` is refused while
        the part has a problem**, with every problem and its line in one message, so the agent that wrote it can fix
        it. A problem that appears after that is reported the same way;
      - a task links to the feature it builds with **`implements`**, which does not block. Its target carries the
        feature, `pr-0003-km#F2`, and names a proposal that has that feature: `link` refuses any other target and
        names the features the proposal has. `reid` keeps the feature when it rewrites a link;
      - **a feature's state is derived**, never written: `planned` while its proposal is `draft` or `proposed`;
        `deferred` with the proposal; `dropped` when the proposal is `rejected` or `superseded`; and once the proposal
        is `accepted`, `accepted` until a task implements it, then "n of m done" over its tasks that are not dropped,
        and `done` when all of them are;
      - **sections are matched by name across proposals**: two proposals with `### Packs` share the section. There is
        no section registry and no feature kind, so nothing has to be kept in step. Names match after trimming, in
        any case. **Near-duplicates** are names that match once spaces, hyphens and underscores are taken out and a
        final `s` is dropped (`Pack` and `packs`, `Task view` and `task-view`). Core holds that rule, so the CLI and
        the view agree. `collide sections [--json]` lists the names in use, with the features and proposals under
        each, and the near-duplicate groups;
      - a team system's provider reports features in the same shape. On GitHub, the Features part of a proposal
        issue's body reads as a file's does. Where a team keeps a feature as an issue of its own, its section is an
        `area:<name>` label (Stage 5) or a Projects v2 single-select field that `board.yaml` names (Stage 6c). That
        board key is optional and is added in Stage 6c, so no existing file changes.
    - **A proposal's quality parts** (owner, 2026-11-15; revision 18), from the ideas on the Field mockup (A). They
      use the Features part's entry form, `- **<letter><n> <title>**: <text>`, with continuation lines indented by
      at least two spaces, and a continuation line may start with a label (`Passed on:`). Numbers count from 1 and
      are not reused. A problem in one of these parts is reported on the item (`partProblems`, 0.8) and never
      refuses a move: they are advice, and only the Features part gates `proposed`.
      - **Decision log.** Entries `D<n>` in the Decision section. Labels: `Passed on:`, the alternatives not taken,
        each with its reason; `Evidence:`, item ids, a feature as `pr-0003-km#F2`, or repo paths; `Revisit if:`,
        what would reopen it; and, optional, `Confidence:` `low`, `medium` or `high`, the person's own and never
        derived. An evidence id that names no item is a problem, and a cited spike shows its status. Text in the
        section that is not an entry is kept and shown as written, so a Decision section written before revision
        18 reads unchanged;
      - **Assumption ledger.** A `## Assumptions` part with entries `A<n>`, each a claim the plan leans on.
        Labels: `State:`, one of `unverified` (when the label is missing), `verified`, `contradicted` and
        `accepted risk`, which the agent writes only on the person's word; `Evidence:`, as above; and `Spike:`, the
        spike that would settle it;
      - **Findings.** A `## Findings` part with `### Red-team` and `### Pre-mortem` headings and entries
        `R<n> <severity> <title>`, where the severity is `high`, `medium` or `low`. Labels: `Target:`, an `F<n>`,
        `D<n>` or `A<n>` of this proposal; then either `Accepted:`, with the change it made to the plan, or
        `Dismissed:`, with the reason. A finding with neither is open;
      - **coverage** is derived, never written: the proposal's features that no task implements, and the tasks the
        proposal includes that implement no feature. Both break the link a feature's derived state needs, so a
        feature with no task never shows progress.
    - A question holds its own decision once it is resolved (`resolved_by: human:<date>`).
    - Only a person accepts, defers, rejects or resolves. `collide/plan` refuses those ops from a run
      (`actor.kind: agent`), and refuses `accepted` while a question the proposal includes is open. An agent phase
      that writes a planning file's `status` or `by:` line itself fails at phase end, as an edit to checklist state
      does. **The actor is read from the connection** (R5), never from an argument: the daemon takes it from the
      peer's process tree (0.2). Only the app is a person; a call from the CLI in a terminal that asks for one of
      these ops becomes a Queue confirmation that the person answers in the app. Under the same-user rule (0.1) this
      stops a mistake, not an agent that means to get around it. The phase-end check hashes the planning files that
      hold a `status:` or `by:` line before and after the phase and compares the lines that changed, so an edit that
      bypasses the CLI is caught as well.
    - **A `spike`** (R5) is a folder with a `record.md` at the top level of the shared root. Its statuses, as
      the records in `.collide/planning/spikes/` use them: `planned` (`ready`); `running` (`active`) while its scripts
      run; `results` (`review`), when the results are in and the conclusions are not yet written down; `concluded`
      (`done`), when the record's conclusions are written; and `dropped` (`dropped`). The records in this repo use
      `results` and `concluded`; the others are for the spike workflow of Stage 9. A spike links to the
      epics it informs. The slice reads a spike as an item of kind `spike` only from Stage 4c; until then it reads
      only `epics/` (below).
      A spike's pre-registration (revision 18) is a `## Pre-registration` part of its record, with the labels
      `Hypothesis:`, `Cheapest test:`, `Kill criterion:` (the result that would end the idea, written before any
      run) and `Budget:`. Approving it writes `preregistered:` with the part's hash. A later change to the part is
      an amendment kept below the original, and the view shows that the hash no longer matches.
    - There are no decision or deferred files. The planning view's "Decisions made" lists the Decision sections of
      accepted proposals and resolved questions. A team system keeps its own decision kind through the role mapping;
  - **ids** (owner, 2026-11-13): a kind prefix, a number and a two-letter suffix, as in `ts-0012-ua`. Epics use
    three digits, the other kinds four. Names on disk are `<id>-<slug>`. An optional `alias` (`S5`) is shown, never
    linked to:
    - the number is the next free one for that kind across every ref Collide can see (local branches and fetched
      remotes). It keeps creation order and is the short handle. It may repeat after two branches merge;
    - the suffix makes the id unique, so a merge never renumbers a file or rewrites a link. `collide task 12`
      resolves when one item has that number; otherwise it lists the matches and asks for `12-ua`;
    - **the id never carries its epic** (owner, 2026-11-14). The folder is the epic, so a move between epics keeps
      the id and every link to it. Wherever Collide prints an id (the task CLI, the board, a link's target), it
      prints the item's epic beside it when the source reports one, as `ep-002-va / ts-0007-ga` or
      `blocked-by ts-0009-cu (ep-002)`. An epic inside the id, as plan's ids have, was considered and not taken: a
      move would rename the item, and a link made to it on another branch would break at the merge;
    - **a collision of whole ids is possible** (R5): 23 letters make 529 suffixes per number. The suffix is drawn
      from a seeded generator, a test pins that, and a second test forces a collision and checks that the draw is
      repeated until the id is free among every ref Collide can see. If two branches still create the same id, the
      second file is a `task.invalid`, and **`collide task reid <id>`** gives it a new suffix, rewrites the links that
      name it, and commits nothing. The owner's two-letter rule of 2026-11-13 stands (a three-letter suffix was
      considered and not taken, owner, 2026-11-14);
    - the suffix is letters only, without `i`, `l` or `o`, so it never reads as part of the number. Ids match
      `^[a-z]{2}-[0-9]{3,}-[a-z]{2}$` and are always strings in YAML (S5: a hyphen inside the scalar rules out every
      number, boolean and null form in the five parsers tried, and two letters avoid the `e-` prefix that `yaml`'s
      1.1 mode reads as NaN). A field that holds a hash, such as `preregistered`, is written quoted, because an
      unquoted short sha can parse as a number (`3e10`, `1234567`). Two files with one id are a `task.invalid`;
    - frontmatter is parsed with the `yaml` package in its default (core) mode, never `gray-matter` and never the 1.1
      mode (S5). **The frontmatter block is capped before parsing** (S5 suggests 64 KiB), not the file (R5): a long
      body is the point of an item, and the body is never parsed as YAML. A body has its own cap, set when Stage 2
      builds the provider, and an item over it is a `task.invalid`. Duplicate keys and the alias limit stay on, and a
      schema checks every field's JavaScript type, with no coercion and no `Date`. A parse takes about 42 us a file
      (measured S5: 1,599 task files, 67 ms), so the provider needs no parse cache;
  - two roots: a private one in the pack's data directory, and a shared one at the configured path,
    `.collide/planning/` by default, committed as views are;
  - the path is set in `.collide/config.yaml` and is part of the trust hash, so a change asks for trust again. A
    path outside the repo, under `.git/` or over Collide's own protected files (`board.yaml`, `packs.lock`,
    `config.yaml`, `views/` and `profiles/`; 0.9) is refused, and the message names the file it would have covered
    (R5);
  - after a change, new items go to the new path. Old items stay readable through `layout.previous`, with an "old
    path" chip, and Collide offers Move: a `git mv` committed with the config change. An entry leaves `previous`
    only when nothing remains there;
  - the shared root is data, outside the trust hash: a planning file cannot declare a hook, gate or observer.
- **A board has one planning source.** A role that source lacks either maps onto a kind it has, or is off with a
  reason. If the board opts in, that role is held in markdown, with a badge (owner, 2026-11-13).
- **Share** is the move op. The markdown provider copies items to its configured path. A team system's provider
  writes through that system's CLI into that system's own paths, and a backend that is not branch-scoped publishes
  in its own way. `record` commits exactly the files written.
- Items that exist only on a non-default checkout show a "branch only" chip.
- Collide's own records are spikes in the shared root, `.collide/planning/spikes/` (owner, 2026-11-13: "we eat our
  own dogfood"). Each folder holds the record, with frontmatter, beside its reproduction scripts. Open questions and
  Deferred stay in this plan.
- **Collide's own build tasks** (owner, 2026-11-13; part L) are items in the shared root, `.collide/planning/`, in
  the layout above. Each stage is an epic (`ep-001` is Stage 1), and its tasks sit in the epic's `tasks/` folder.
  This plan keeps the design; each stage's **Status** line names its epic and **the plan revision its items came
  from** (R5), and the items hold the status. **Only the next stage is split into items** (R5; it was the next
  milestone): a later stage stays an outline until the one before it nears its end, so a revision of this plan does
  not leave a hundred items written against an old design. A stage with sub-stages (3a, 3b, …) keeps one epic, and
  its items say which sub-stage they belong to. If this plan changes after a stage's items were written, the Status
  line says which items were checked against the new revision.
  - Kinds so far: `epic`, `task` and `spike` (above).
  - A task's statuses are the five categories themselves (`ready`, `active`, `review`, `done`, `dropped`), as for
    scratch tasks.
  - Links: `blocked-by`, which blocks. The folder gives the epic. A task is **ready to start** when its status is
    `ready` and every `blocked-by` target is `done` or `dropped`. `collide tasks --ready` lists those. A
    `blocked-by` cycle is reported, and none of its items is ready.
  - Frontmatter: `id`, an optional `alias`, `kind`, `status`, `title`, `owners` (the claim) and `links`. Body
    sections: Goal, Success criteria, Tests, Notes.
  - Until the planning slice lands, items are written by hand, and an agent picks a ready one by reading their
    frontmatter. The slice must read every hand-written item **under `epics/`** with no `task.invalid`. It does not
    read `spikes/` at first: a spike record is a different shape from a task (its statuses, above), and the slice
    criterion would fail on it. Stage 4c adds the `spike` kind to the provider, and from then the slice reads the
    records too.

### 0.11 Interface direction

Collide is not a chat app. Its objects are tasks, runs, phases, gates, workspaces and decisions; conversation is a
detail inside a phase. No chat bubbles, no centred conversation column, no sidebar of chats.

**Who it is for** (owner, 2026-11-15; revision 17). Collide is a higher-level development environment, used by
product managers as well as engineers. It must not read as an IDE:
- the default look is approachable: plain words before ids and terms of art, a state named in words as well as in
  colour, a readable sans-serif type, and room around things;
- an engineer's detail (terminal output, tool calls, tokens, timings, CEL, raw JSON) is kept in full and is one step
  away, behind a disclosure, a tab or a toggle, never removed;
- monospace is for code, commands and output, not for the interface around them.

Three surfaces, one keystroke apart:

**Tab names and subtitles** (owner, 2026-11-15; revision 19). Each tab shows its name with a plain subtitle beneath
it, so a person who has not learned the names can still find their way: **Field**, "the whole board"; **Tracks**,
"what's running now"; **Queue**, "what needs you"; and **Planning** (the Planning view below), "what to build next".
The names stay; the subtitles are the audience rule above applied to navigation.

- **Field:** the corpus through a view, laid out as a board or a run order.
  - Tasks are the primary objects. A task being worked on has a lit border (the prototype board's), in the state
    colour. Colour never says which run it is: a run is named by its label (below).
  - The main action is: select tasks, then Launch.
  - A read-only Field ships first (Stage 6); Launch arrives with the run UI (Stage 9).
  - An empty board still offers a way in: a prompt box (type a task, then Launch), **New session**, the sources
    Collide detected, and Add a pack.
  - **Home is the Field** (owner, 2026-11-13; from the Field-first mockup). Its weakness is that a decision sits one step
    from its cause. The menu bar badge and notifications cover most of that (below).
- **Tracks:** every live run as a horizontal track of phase segments (provision → build → brief → review ×2 →
  adjudicate → fix → close). "Review ×2" is two reviewers, A and B, who run in parallel and whose findings are shown
  separately. It is not two rounds (owner, 2026-11-13; S6 built it that way).
  - **A run is named by its id label** (`R-118`), never by colour, which is reserved for state (owner, 2026-11-14;
    S6 Q1, default `id`). S6 also built a lane letter, a position (`#2`, which can change when the order does) and a
    colour control for comparison. The sessions that would have tested them were skipped, so the default is not a
    result. The owner may change it while iterating on the prototype.
  - Gates show as checkpoints, with time and spend meters against budget.
  - Workspace provisioning is a segment with its own progress: orchard's takes minutes (worktree 230–330 s, map
    build about 1500 s, measured R1). Its progress comes from the pack's `progress` report lines (0.5).
  - Parallel runs stack, like a mixing desk or a CI pipeline view, so the whole fleet's state fits on one screen.
  - **Two layouts, toggled inside Tracks** (owner, 2026-11-15; revision 20). Each is useful for a different
    question, and each is a different starting point when the in-app agent reshapes the screen (The in-app agent,
    below):
    - **List:** each run is a row with its phases broken out in order as steps, the same width each, with the
      current step named in words. It answers "where is each run?" at a glance, and it is the default, by the
      audience rule above;
    - **Timeline:** the real-time axis below. It answers "where did the time go?".

    The toggle is per person and remembered. Both layouts show the same runs in the same order, so switching keeps
    one's place.
  - **Up next** (owner, 2026-11-15; revision 21; ships in Stage 11). Below the live runs, in both layouts: the
    tasks and groups waiting to start, in the person's order, each with what it waits on; one list per board
    (revision 22). The tab named Queue stays
    "what needs you", so this list is never called a queue.
  - **The Timeline's real-time axis**, not equal-width phase columns (owner, 2026-11-13; from the Tracks-first
    mockup). A segment's width is its duration. The axis has:
    - a now line and the usage-limit reset line;
    - wait time hatched apart from active time;
    - the projected remainder, dashed;
    - markers for the gate checkpoints above and for detector findings;
    - zoom.

    Only real time shows stalls, time spent waiting on the user, and cold-cache risk.
- **Queue:** every decision waiting on the user, ordered by how long it has been blocking work.
  - It holds:
    - permission requests, `ask_human` and `human` phases with their `show:` list (0.7), budget exceeded;
    - usage limits with their reset time;
    - for a live phase, how long its prompt cache stays warm and what answering later costs ("warm for 12 min, then
      $1.30 to resume"; 0.3);
    - prompts blocked by a user hook, with the hook's message (0.3);
    - take-overs and New sessions past their phase budget, as a warning (below);
    - `unverified` gate and `wait` results: retry, proceed or stop (0.5);
    - write conflicts, merge conflicts found before `integrate`, queued writes, a busy line root, unrecorded edits;
    - integrates and take-overs waiting on an acknowledgement because their range edits the agent tool's config
      (0.3);
    - runs waiting on an acknowledgement because an integrate changed a working-copy pack they load (0.7);
    - disabled observers and finished runs to review.
  - **Layout** (owner, 2026-11-13; from the Queue-first mockup): a list grouped by kind, and a detail pane. It is
    not a narrow rail, because a permission request with untrusted arguments needs the room. The pane shows:
    - the exact command or arguments, with untrusted ones flagged (0.9), or "input not available until approved"
      when the adapter's request carries none (0.3);
    - the prompt-cache countdown and the cost of answering later;
    - a mini track of the run;
    - recent activity;
    - a context view, **Why is this asked?** (`i`; owner, 2026-11-13; part L), which Collide assembles from what it
      already holds, with no model call. Collide's facts come first and the agent's words last:
      1. why it asks: the rule that matched, a hook's `decision_reason`, a failed gate check with the values it
         read, a budget or a limit;
      2. what it would do: an edit's diff; a command's working directory, its paths outside the worktree, and
         deletes, pushes, installs and network use, flagged from the command text; a package script resolved to
         its text; a fetch's domain; an MCP call's server, tool and arguments;
      3. the task: title, body, checklist, links one hop out, and the phase with its goal and round;
      4. what led here: the phase's last calls and their outcomes, and the files changed so far (the handoff-note
         builder, 0.3);
      5. history: the same request in this run and this repo, how often it was approved and denied, and what
         followed the last approval;
      6. the agent's own words: the call's `description` and its last message before asking, marked unverified,
         because an agent fed a hostile issue can argue for anything (0.9).

      Each Queue kind has a default set, and a pack adds to it through the phase's `show:` list (0.7). Asking the
      agent itself is deferred.

      **The context view is a data contract** (S6; owner, 2026-11-14). The six sections are the fields of one record
      that Collide and the pack fill, and the UI shows a section the data lacks as "Not available in this view",
      never hides it. The gap shows what people want and cannot get (S6's scenario 9 left two sections empty on
      purpose). The agent's own words come last.
  - **Keyboard triage:** `j`/`k` move, `i` opens the context view, `o` opens the item's run on its track, zoomed on
    the moment of the decision (S6 Q3, default `moment`), `u` undoes the last answer and `Esc` backs out. `a`, `d`
    and `r` mean this for each kind (S6 Q2, default `literal`; owner, 2026-11-14):

    | kind | `a` | `d` | `r` |
    |---|---|---|---|
    | permission request | allow once | deny | |
    | command | allow once | deny | |
    | edit | apply the edit | reject the edit | |
    | failed gate | proceed unchecked | stop the run | retry the gate |
    | budget exceeded | raise the cap (the prototype raised it by $2) | stop the run | |
    | review | accept | discard | re-prompt |

    `a` never means "always allow" (0.3). Every kind in the list above that is not in the table gets its row when its
    stage builds it. S6 also built `safe` keys (`a` is retry or accept) and `confirm` (the literal keys, with a second
    press for a consequential action), and the sessions that would have chosen between them were skipped. Whatever
    the choice, an undo stays: S6's `u` restored the run's state. The real undo can reach only what Collide has not
    yet done, which the run UI stage lists (inferred: an answer already sent to the agent cannot be taken back).

Details:

- **Task drawer:** body, dependencies (both directions), collections, per-checkout state with divergence and
  collisions, other lines' states, the task's runs, and the source's operations as forms. Create opens the same
  form. The body is **sanitized markdown** (R5): raw HTML off, `javascript:`, `file:` and `data:` links removed,
  images from remote hosts not loaded, and an external link opens in the browser after a confirmation (0.9). Any
  other place that shows a task's or an agent's text follows the same rule.
- **Filter bar → view.** Typing a filter in the Field is an ad-hoc view, and saving it writes a view file. CEL is
  never the only way in: chips for collection, status, label and source write the CEL for you.
- **Drill-down from a phase segment.** An **activity digest** comes first: files touched, commands with exit
  codes, tests, gate verdicts, the phase report rendered as a document, and the diff. The raw transcript and the
  terminal sit behind a toggle.
  - Files touched and the diff collapse dependency stores and build output that `.gitignore` does not already hide
    (`node_modules`, `.pnpm-store`, `target`, `dist` and `coverage` by default; packs and boards add to the list). A
    collapsed folder shows its file count and size. Lock files are always shown, because a dependency change is
    part of the review.
  - The same list applies to the handoff note's files changed and to the run record, and the stored list is capped,
    with the rest counted (after Ferry's runner, where an unignored pnpm store in the changed-file list
    overflowed `JSON.stringify`).
- **Agent output** (owner, 2026-11-13; part K). Selecting a track, a phase segment or a task with a run opens a pane
  with that run's output, rendered from the agent's stream rather than shown as a raw terminal:
  - the agent's text; each tool call as one row (tool, short input, duration, exit code or error) that expands to
    its input and result; compactions, blocked hooks, and permission requests linked to their Queue item;
  - it follows the newest output. Scrolling up pauses that, with **Jump to latest**. One tab per phase, and an
    instance picker for a group run;
  - the output is untrusted (0.9): markdown without HTML, links shown but never followed on their own, ANSI
    stripped, long results cut with their size shown. The list is virtualized and paged from the daemon's log;
  - the raw JSONL and the terminal stay behind a toggle.
- **Tool-call time breakdown** (owner, 2026-11-13; part K), for a track or a phase:
  - a table: model time, each tool by name (MCP tools grouped by server), hooks, permission waits and Queue waits,
    and an "unattributed" row, with calls, total, share, an "of wall" share, median, maximum and errors. Bash
    splits by the command's executable (`cargo`, `pnpm`, `git`), which 0.13's privacy rule keeps. Three choices
    settled by S8 (`sp-0010-bn-metrics-sources`; labelled inferred, from nine small runs):
    - **Model time and the gap threshold.** From telemetry spans when the phase has them. Otherwise from the stream:
      the time from the previous input line (a tool result or the prompt) to `message_stop`. A gap of up to 1 s
      is model time (client overhead between a tool's end and the next request measured 13–185 ms). A longer gap
      that no tool, hook or wait explains is "unattributed (hooks, start-up, waits)", never model time. A turn started
      by a background-agent notification has no input line, so it uses the span, or reads "not reported";
    - **A subagent's calls.** The Agent row shows self time: its span minus the time covered by its own nested model
      and tool spans. The nested calls appear under their own tool rows, marked "in subagent". Counting the Agent
      span in full as well double-counts (foreground subagent runs summed to 144–158% of wall time). Nesting uses
      `agent_id` and the parent id, never time containment, because a background Agent's span ends before its nested
      spans do;
    - **Share.** The share column is a part of the sum of the table's rows, so the rows add to 100%, and under
      parallel calls it can exceed wall time. A separate "of wall" column splits each instant among what runs, with
      the "unattributed" row, so it adds to 100% of wall time. A share of plain wall-clock time is not shown, because
      it passes 100% when calls overlap;
  - a timeline with one row per tool type on the run's real-time axis. Colour stays reserved for state, so rows, not
    hues, tell the types apart;
  - **three time totals shown apart** (S6): the wall time of the phase, the sum of its calls, and the union of its
    calls (the time with at least one running). They differ once calls run in parallel or nest. The span model needs
    a parent id for this (0.13);
  - filters that combine: tool type; **minimum duration**, how long one call took ("every call over 2 s"); and
    **time window**, a span of clock time ("every call between 11:00 and 11:10"), dragged on the axis, typed, or
    set by picking a phase. A call matches when it overlaps the window (owner, part L). Calls, median, maximum and
    the minimum-duration filter use the whole call; total and share count only the time inside the window. The
    chips write CEL over spans, as the Field's chips do for tasks;
  - a table row filters the timeline to that tool, and a bar scrolls the output pane to that call;
  - each phase's times are labelled exact (telemetry) or approximate (stream timing). Measured S8: the stream is
    within 150 ms of the span for an unqueued call, and wrong by the whole wait for a call queued behind another
    (0.13).
- **Talking to an agent is the exception.** You either intervene (send a note to the running phase) or take over
  (interactive `--resume` in a terminal). The ask box (below) is a request that changes the screen, not a
  conversation with a run.
  - Closing the terminal view detaches. The session goes on in its shim, Tracks marks it detached, and reopening
    replays its recent output. So a take-over survives an app quit and a daemon restart (measured S2).
  - The take-over ends when `claude` exits or the user picks **End take-over**, and the phase's gates then run as
    usual.
  - Interactive Claude takes no `--max-budget-usd` (measured S2). Its live cost comes from telemetry (0.13), and at
    the phase's budget the Queue warns. Nothing is stopped, because the user is at the keyboard.
  - In a folder Claude has not trusted, it opens at its trust prompt, whose default answer exits (measured S2). The
    terminal shows it, and the user answers (0.4, 0.9).
  - Collide follows the session id: `/compact` keeps it and `/clear` starts a new one (measured S2, from the
    `SessionStart` hook and telemetry). A later headless resume uses the newest id.
- **New session** (owner, 2026-11-13): interactive Claude in a fresh worktree, the familiar way in for someone coming
  from Claude Code. It is a one-phase run on a new scratch task, shown in Tracks. It detaches and ends as a take-over
  does, and when it ends, its diff goes to review like any finished run: accept, re-prompt or discard.
- **Plan…** (owner, 2026-11-14): planning with an agent is a New session on a proposal. "Plan…" on the Planning
  view, an epic or a task creates a private `draft` proposal (0.10) and starts an interactive session on it with
  `collide/plan`'s planning workflow. The run belongs to the proposal, not to a scratch task:
  - the proposal goes in the epic it was started from, the task's epic, or `ep-000` from the Planning view. Started
    from a task, it links `informs` to that task;
  - the conversation is the run's terminal; there is no chat surface. The workflow's prompt names the proposal and
    the CLI ops the agent writes with: `create` into the private root, then `link` from the proposal
    (`includes`), for tasks, spikes and questions. `collide` is on the session's `PATH` (R5): Collide puts a
    directory with the `collide` binary first on `PATH` for this session, because the agent's Bash would otherwise
    not find it. The agent's calls reach the daemon as a run agent (0.2), so the person-only ops (0.10) fail for it
    and become Queue confirmations;
  - a choice the owner makes in the terminal goes into the proposal's Decision section as text. It is a decision
    only once a person accepts the proposal;
  - a choice to make away from the terminal, or one of several at once, becomes a question item. It appears in
    "Decisions needed" and is answered in the Queue, whether or not the run is live. The agent reads the answer
    with `collide task <id>`;
  - features go in the proposal's Features part (0.10), written with the `addFeature` op (Stage 4c), whose
    `section` argument is required and may be empty (owner, 2026-11-14). The prompt has the agent run
    `collide sections` first and reuse a name in use. A new name is a product noun of one to three words, a board
    aims for about 5 to 12 sections, and a feature with no clear section goes in empty, to be sorted later;
  - decisions go in the Decision section as Decision log entries, and assumptions in the Assumptions part (0.10;
    revision 18). The prompt has the agent name at least one alternative each decision passed on, and cite
    evidence for an assumption or leave it `unverified`;
  - the agent moves the proposal to `proposed` when it is ready. Ending the session leaves the proposal as it is.
    Plan… on a draft proposal starts a new session that reads the proposal and its items, not the old
    conversation;
  - everything stays private until Share (0.10). A diff in the session's worktree goes to review, as for New
    session;
  - **a panel beside the terminal** (revision 18) follows the proposal's files live. Its Plan tab shows the
    proposal and its status, the features, the spikes, the questions, the tasks and the Readiness checklist. Its
    Quality tab shows the coverage map, the Decision log, the Assumption ledger, the findings and each spike's
    pre-registration. Its buttons type a request into the terminal and write nothing themselves, except a
    person's own ops (accept, answer a question), which take the drawer's path;
  - **the Readiness checklist is advice** (owner, 2026-11-15): the Features part has no problems (the one line
    Collide enforces, at `proposed`); no feature is Unsorted; every spike is concluded or dropped; no included
    question is open (enforced at `accepted`); every feature has a task, and every task implements a feature;
    each assumption is verified or an accepted risk; a red-team pass has run, with no open high finding; and every
    decision names an alternative it passed on;
  - **a red-team pass** (revision 18): a fresh agent, `collide-plan-red-team` from the planning pack, with
    read-only tools, reads the draft and its items and reports findings, each with a severity and a target:
    contradictions, unmeasured claims, work with no owner. Its second mode, the pre-mortem, assumes the plan failed
    three months on and writes why, each story with a mitigation. The session's agent starts it on request and
    writes its report into the Findings part. The person accepts a finding, and the agent makes the change and
    writes `Accepted:`, or dismisses it with a reason. A pass that would start an agent shows the cost of the last
    pass first, when there was one;
  - **a budget meter** (revision 18) above the terminal shows the session's spend against its phase's budget,
    and its elapsed time, from telemetry (0.13). A breakdown by part shows the session's agent, each red-team or
    pre-mortem pass (its calls nest under its Agent call) and the spike runs started from the proposal. The meter
    warns at 85% and the Queue at the budget, as for a take-over, and nothing stops the session, because the
    person is at the keyboard.
- **Menu bar** (owner, 2026-11-13). Closing the window leaves Collide in the menu bar, so the Queue stays in sight
  while runs go on:
  - the icon shows the live runs and a badge with the Queue count, and the menu shows the cache closest to going
    cold, with its countdown;
  - notifications for new Queue items, finished runs and caches about to go cold;
  - actions: open the Queue at an item, New session, open a board, Quit;
  - no approve or deny from the menu or a notification, because a permission request's arguments can come from
    untrusted task text. Both only open the item;
  - text in Collide's own words, with task titles off by default (0.9);
  - closing the window destroys its webview. Measured on S2's app (debug build, idle terminal page; the S2 record's
    addendum): 138 MiB with the window open and 53 MiB with it closed (the app 33, WebKit's GPU and network
    processes 20), against 143 MiB with the window only hidden. Reopening loaded the page in 267 ms.
- **Launch shows a run preview before anything starts:**
  - the line, workflow, phases, the workspace and integrate implementations, budget and the pack's `launch`
    parameters (Orchard: review budget);
  - each task's status at the line's ref, and, for a task the workflow does not accept, the field or operation it
    lacks;
  - the user hooks and plugins each phase will load, with flags on those that change behaviour, and a redirected
    model endpoint (0.3).
  - the two `claude` version chips, in use and latest published (0.3).
  - the profile, why it applies (picked here, the view's, a rule, the board default), and each resolved setting
    with its layer (0.7);
  - a badge on packs loaded from a `dev:` folder and on `agentWork` phases (0.3).
  - The expected duration appears only once three or more earlier runs of that workflow have timings; otherwise it
    says "unknown".
- **First run works with no pack and no tasks** (owner, 2026-11-13). Any git repo opens with no setup:
  - checks: `claude` is installed and logged in; the folder is a git repo (if not, Collide offers `git init`); `gh`
    is optional;
  - the built-in packs (0.4), plus a GitHub source when `origin` is on GitHub and `gh` is logged in (otherwise a
    hint);
  - default model aliases (`fast` → sonnet, `strong` → opus), editable later;
  - board settings stay per user until **Share with team** writes `.collide/` into the repo (`collide init`);
  - a trust prompt only for packs from the repo or ones the user adds;
  - source auth when a source needs it, e.g. `gh auth refresh -s project` with the account to use.
- **Line selector** (owner, 2026-11-15; revision 22). One menu in the top bar, visible on every surface (S6 Q4,
  default `top`), lists every open board (one repository each) and, under each, its lines (0.12). The person picks
  a line and need not care whether it is another repository or another branch; boards stay underneath.
  - Selecting a line of the same board swaps its packs, workflows and views in place. Selecting a line of another
    board also swaps the sources, the corpus, the status mapping, the checkout filter and the person's own views.
  - The Field, Planning and Launch follow the selected line.
  - **The Lines control** on Tracks and on the Queue: the selected line, all lines, or a hand-picked set (a
    multiselect grouped by board). Each item is tagged with its line. Both start on all lines, because a decision
    on another line still needs the person, and each remembers its own choice. A line added later joins "all
    lines", not a hand-picked set.
  - Nothing is hidden from the count: the Queue's badge and the menu bar count every line, and a Queue that hides
    items says how many, with a link to show them. `a` and `d` act on an item in place; `o` or a notification for
    an item on a hidden line shows it, with its line named, without changing the selector or the control.
  - Up next is one list per board (owner): each board's list shows in Tracks under its runs, filtered by the Lines
    control. The concurrency cap is the app's, because it is this machine's capacity.
  - Switching stops and restarts nothing. A line's loaded snapshot stays in memory after the person leaves it, up
    to the eight lines used last, so switching back is immediate; an older one reloads from git.
  - When the selected line's ref moves, its snapshot and views reload. A trust hash that changes on any line,
    selected or not, raises a Queue item, and nothing on that line starts, Up next included, until it is trusted
    (0.9).
- **Metrics.** A run opens to a waterfall:
  - phases, hooks, gates and reported spans on a time axis, with the critical path over the phase graph marked.
    Reported spans are marked as reported;
  - active time and waiting time shown apart;
  - resource lanes beneath, each labelled with its granularity (process, phase or machine);
  - a phase opens to its tool calls, as in the tool-call time breakdown above.

  "Where time goes" aggregates span names across runs, filtered by view, line, profile or fingerprint. Any two finished
  runs can be compared side by side, with no verdict. Runs can be annotated with a hypothesis or excluded as outliers,
  with a reason. Spans and samples export as JSONL or CSV.
- **Experiments.** Pick a task, then two arms.
  - The preview shows:
    - `git diff --stat A...B`, with `protectedPaths` highlighted;
    - a warning when B is not A plus its change (the merge base is not A);
    - the primary metric;
    - the design choices (0.14), each with a line on when to pick it, and whether the chosen design can give a
      verdict at all;
    - how many secondary metrics the pair count can support (0.14);
    - how many other runs are live;
    - the total cost and the expected duration from this workflow's earlier runs (or "unknown"), updated as the
      choices change. An Orchard loop takes hours per run, so a verdict design is shown in days.
  - The result puts the arms side by side: per-phase-kind and per-span deltas, the critical-path delta, outcomes and
    the verdict rule's answer. It states its scope (S6): "on this task" stands beside the verdict, with the
    critical-path difference and whether the outcomes are equal, so one run is not read as a general rule.
- **Visual language:**
  - dense and keyboard-first, with a command palette
  - monospace for ids and refs
  - colour reserved for state
  - motion only for live state: working pulses, blocked holds steady

  The particle-physics flavour of the name may shape the brand; it does not shape navigation.

S6 built this direction as a clickable prototype over fake data (`sp-0009-bb-interface-prototype`): Field, Tracks, Queue,
run detail with Output, Time breakdown and Review, Compare and Experiments, with its UI state in one reducer. Its
scripted pilots complete all nine scenarios (50 of 50 tests pass; measured). No person who did not design Collide
has used it: the owner skipped the sessions (2026-11-14), so the interface is the owner's judgement, and the later
stages check it with ported UI tests and an owner walkthrough (Stage 1).

**The in-app agent: the ask box** (owner, 2026-11-14; S6's in-app agent section in
`.collide/planning/spikes/sp-0009-bb-interface-prototype/record.md`). The interface is built to be flexible in ways
that are not obvious, and an agent can use that. A request typed in the ask box, in the top bar of every surface,
starts an **ask**. The agent reads whatever it needs with its own read-only tools (for Claude Code, Read, Grep and
Glob over the code), decides, and then changes the screen through five generic tools. The UI knows nothing about
code files. It presents what it is told, and the agent does the analysis (0.3 for the launch, 0.2 for the endpoint).
- **Two requests the owner gave as examples.** On the Field: "Can you show me tasks that can be grouped together,
  where code changes would affect similar files?" The agent reads the code, then shows groups of tasks with a reason
  for each, so that a person can run a group together (Stage 11). On Tracks: "Can you show me the tracks that have
  taken the longest in their build portion?" The agent sorts the tracks by seconds in the build phase.
- **It is not a chat.** One request gives tool calls and chips, with no transcript and no session kept, so the next
  ask starts fresh. The agent's closing words show under the chip, marked as the agent's and unverified.
- **The five tools.** Each takes ids and labels, never a file path. A surface a stage has not built is refused with
  the supported ones listed.

  | tool | input | effect |
  |---|---|---|
  | `query` | `subject` (`tasks` or `runs`), `filter?`, `fields?`, `limit?` (1 to 500, default 100), `cursor?` | none. Returns one page of the rows the window holds, and a `next_cursor` when there are more. A task: id, title, epic, status, labels, source, `depends_on`, `trusted`, and no body unless `fields` names `body`. A run: id, task, status, workflow, started, `total_seconds`, `wait_seconds`, `spend_usd`, and each phase's seconds |

  - **A page fits under the output cap** (R5). The MCP output cap (25,000 tokens by default, 0.3) replaces an oversized
    result with an error, and a corpus of about 1,500 tasks would hit it with bodies in. So `query` leaves the body
    out unless `fields` asks for it, and a body is returned only when the filter names ids (at most 20), cut at 2 KB
    with its full size shown. The page ends before its estimated size reaches 20,000 tokens, not at `limit`, and the
    cursor continues. A test pages a fixture of about 1,500 tasks and checks that every page is under the cap and
    that the pages together hold every row once. For a red ACP agent (0.7) a task's body is withheld altogether.
  | `set_view` | `surface` (`field`, `tracks`), `title`, `reason?`, `filter?` (`{}` clears), `sort?` `{by, dir}`, `group_by?` (Field), `window?` `{from, to}` (Tracks) | sets those parts of that surface's view and leaves the rest. A chip shows the expression |
  | `show_groups` | `surface`, `title`, `groups` (1 to 12 of `{name, ids, reason}`), `rest?` (`dim` or `hide`) | draws a lens: the groups with their reasons, and the rest dimmed or hidden. An id in one group only. A chip |
  | `highlight` | `surface`, `ids`, `note?`, `title?` | marks the ids and filters nothing. A chip |
  | `open` | `surface` (`field`, `tracks`, `queue`, `experiments`), `id?` | opens that surface, and one item on it |

  - Task filters are epic, status, label, source, text and ids. Run filters are status, task, workflow and ids. Field
    sorts are id, title, epic, labels and status. Tracks sorts are total, start, end, spend, wait and
    `phase:<name>`, the seconds a run spent in that phase, with a missing value last. `group_by` is status
    (the source's own), category, epic, label or source. A `window` is two clock times such as `09:15`.
  - **No tool launches, accepts, edits or deletes in v0.** A group the agent shows is a selection a person can launch
    from the Field (Stage 11), and the agent has no tool for it. Tools that act are deferred (Deferred).
- **A view and a lens are two things** (S6 proposal 2). Keeping them apart means a saved View never silently holds an
  agent's guess.
  - A **view** is typed state: filter, sort and grouping on the Field, and filter, sort and window on Tracks. It stays
    live, because it is an expression over the corpus. On the Field, **Save as View** writes the filter as CEL through
    the same chips that the filter bar uses, with its static cost estimate (0.9, 0.10), and the grouping as
    `layout.groupBy`. A sort is saved as `layout.sort`, a key the view schema gains (Stage 2). A Tracks view saves as
    a personal view in the user's board settings, with its filter and sort. It drops its window, because a window is
    a span of clock time on one day, and says so. Views of runs do not exist in 0.10, so this is new (inferred).
  - A **lens** is the agent's own judgement: named groups of ids, each with a reason, stamped with the time it was
    made and with the agent and model that made it. An id is in one group at most, and the ids left out show as
    "not grouped". A lens does not follow the corpus: a task that changes after it was made stays where it was put.
    It is saved only as a **snapshot**, a personal file under the board's data directory, and never as a View, because
    a View holds membership by expression and a snapshot holds a judgement at a moment. The Field draws a lens as a
    board of groups with the reason under each name. Tracks draws it as bands, and writes the sort value on each row.
- **Chips** (S6 proposal 1). Every call that changes the screen becomes a typed reducer action (0.15) and a chip.
  - The chip's first line is Collide's statement of what changed, built from the validated action: the expression
    (`sorted by seconds in build, descending: 9 of 9 runs`), or the number of groups and tasks. Below it, marked as
    the agent's words, are its title and its reasons. The chip also names the source (the agent and model, or
    "external client") and the time.
  - **Undo** restores exactly the view state that the change replaced, from a snapshot taken at that call, and undoes
    the last change only. A person's own change to the view after it ends that Undo, and the chip stops offering it,
    so Undo never discards a person's edit (owner, 2026-11-15). **Save as View** or **Save snapshot** is on the chip,
    as above.
  - A lens made from task text alone carries the flag below.
- **The ask box.**
  - It shows one line per tool call as it happens, written by Collide from the call (`query tasks: 12 rows`,
    `show_groups: 4 groups, 11 tasks`), then the chip. **Cancel** works while the ask runs.
  - A failure is plain text, never an empty box. The texts are: `the agent finished without changing the screen (N
    tool calls, none changed anything)`, `timed out after N s`, `stopped at the $X budget`, `cancelled`, `agent
    error: <the first line of its error>`, `<command> was not found on PATH`, `this agent cannot use HTTP MCP
    servers`, `Collide has no window open; open Collide and ask again`, `the Collide window did not answer within
    10 s`, `cancelled: the window was closed or reloaded` and `this agent is not confined; acknowledge it in
    Agents first` (0.7). The first two window texts are the endpoint's own (0.2). A change made before a failure
    keeps its chip and its Undo.
  - **"No files were read"** (S6 proposal 3). S6's run 5 grouped 107 tasks by title wording and never read the code,
    and said so in prose. So Collide, not the prompt, checks it. Collide's `AgentSession` already sees the ask's
    stream: Claude's Read, Grep or Glob calls, and an ACP agent's tool calls of kind `read` or `search` (inferred for
    ACP: S6's opencode runs never read the folder). When the ask ends with no such call in it, a `show_groups` chip
    carries "No files were read in this ask. These groups come from task text alone." For an external client Collide
    cannot see reads, and the chip says "this client's reads are not visible". The flag informs and blocks nothing,
    since a request about risk or order needs no code.
- **What S6 measured** (prototype, fixture data only, Claude Code haiku through `claude -p`, nine live runs):
  - **Time.** The first tool call came 2.2 to 3.5 s after the request, mostly Claude Code starting. A sort appeared
    after 3.5 to 5.4 s and finished in 6.0 to 8.3 s. A lens appeared after 14.8 to 22.4 s, nearly all of it the
    model writing a reason for every group, and finished in 18.2 to 25.4 s (measured). Showing the first group
    before the rest was not tried (unmeasured).
  - **Cost and size.** One ask cost $0.0005 to $0.0082 and loaded 27.6K to 107.3K input tokens (measured). The whole
    task, nine runs plus three more, cost about $0.04. That launch was not strict (0.3), so how much of the load
    was the user's config is unmeasured.
  - **Right or wrong.** Four sort runs were right. Of three grouping runs, the first (prompt v1, which said to read
    the code "when the request is about code") was wrong, and two (prompt v2, which says it must read first) were
    right: four groups equal to the intended ones, with the two ambiguous tasks in either area. Six of seven runs
    that have a reference answer were right, and a prompt change repaired the seventh, which is one data point for
    a prompt tuned to one demo (inferred).
  - **A weak local model** (opencode with `ollama/qwen2.5:7b`) did not sort in either of its two sort runs (one
    drew three bands, one highlighted the right two runs), and it invented 8 task ids in the grouping run. Collide
    refused all of it, so nothing wrong reached the screen, and the model's prose still described a grouping that
    was not shown. That is why the chip states what changed (0.9). opencode's own hosted default timed out at 90 s
    with no event, and no reason was found (unmeasured).
  - **A closed or restarting window** failed every call with the 10 s message, and the agent said it had changed
    nothing and guessed no ids (measured, run 9).

**Planning** (owner, 2026-11-13; `collide/plan`):
- A Queue lane can be fed by a view (`queue: true` on the view). Its items are answered through the source's ops.
  `collide/plan` ships "Decisions needed": open questions and `proposed` proposals. Several decisions at once are
  question items answered there, with a `wait` phase holding a headless run.
- **Planning view:** features, decisions made, spikes outstanding, results with their summary tables, and decisions
  needed.
  - **Features** (owner, 2026-11-14; revision 14) come first, because planning is mostly features before there are
    tasks: every feature of every proposal that is not dropped, grouped by section name across proposals, each with
    its proposal, derived state and tasks. Features with an empty section come last, as Unsorted. Near-duplicate
    sections (0.10) are flagged, and the view offers to merge them: the person picks the name, and the app writes it
    with `renameSection`.
  - **Sort** asks the ask agent to sort the features into sections. It answers with `show_groups` over the features,
    one group per section it suggests, each with its reason, so the five tools stay five. The view turns the
    difference from the current sections into chips (a move, a rename, a merge). The person applies a chip, the app
    writes it through the provider's ops, and Undo writes the reverse. The agent writes nothing, as for every in-app
    agent tool in v0.
- **Checklists:** core renders the checklist field in the drawer. Agents suggest an item with its evidence, and only
  a person marks it.
- **Share preview:** every file that will leave, the branch it commits to, the handoff note to review, and the
  planning path setting if it is not shared yet.

### 0.12 Lines: several branches on one board

A **line** is a ref the user has opened on a board. It has its own root checkout and its own packs, so different
branches can offer different workflows at the same time.
- The default line is the board's base ref at the board root.
- The user adds other lines (`collide line add <ref>`). Agents never do.
- In this plan, "line root" is the checkout where a line's hooks run. For the default line, it is the board root.
- **The default root is the user's own checkout**, so `record` and `integrate` write where the user works.
  - Writes wait while a rebase, merge, cherry-pick or bisect is in progress there, and the Queue says why (0.4).
  - Option: Collide owns the default root, a worktree on the base ref (`defaultRoot: managed` in config, 0.7).
    - It costs a full provisioned checkout: about 4.1 GB and 230–330 s for orchard (R1, R3).
    - git refuses to check out a branch that another worktree has checked out. So the option needs the user's own
      checkout on another branch, and Collide refuses it otherwise.

- **Per line:** the `packs` list (its order and sources), the default workflow, the profiles and the views
  (revision 22). Committed views load from the line's snapshot, and the line's packs contribute theirs. A person's
  own views are per board and show on each of its lines.
  - Everything else in `board.yaml` is board-level and comes from the default line: sources, the status mapping
    and the checkout filter.
  - A line's snapshot follows its ref between runs. A run pins the commit it launched from, and every pack's tree
    id.
- **What a line has:**
  - A ref and a root checkout: an existing `git worktree list` entry, or one that the workspace owner provisions
    (kind `line`).
  - Its own snapshot: packs, workflows, roles, prompts, gates and hooks, loaded and checked as in 0.7.
  - Its own trust entry. The hash includes the tree hashes of `protectedPaths`. Before trusting, Collide shows
    `git diff --stat` against the default line over pack paths and `protectedPaths` (0.9).
  - Its own line lock.
- **Hooks that run at "the root" run at the line root.** So `dev worktree new`, `record` and `integrate` all use the
  line's own scripts. That is what lets a change to a repo script be tested on a line.
- **Tasks:**
  - The default line's provider reads every checkout.
  - A write into a checkout runs the `./plan` of the line that checkout belongs to. Workspaces belong to their
    run's line.
  - If a line changed the task file format, the default line's parser shows that as parse errors, counted, not
    hidden.
- **Launch on a line:**
  - The task is resolved by (shortId, path) at the line's ref, and a collision is refused.
  - One task cannot have live runs on two lines at once, except inside an experiment.
- **Why this does not weaken 0.7.** The snapshot rule stops agents from editing their own gates.
  - A line is a ref that the user opened, with its gate code in the trust hash.
  - Agents work in workspaces, or at a root only where a workflow says so (0.4, revision 22): never for an
    untrusted author's task, an experiment arm or a sandbox, and with the start record and the preview's warning.
  - A workspace's edits reach a line through `integrate`, where `protectedPaths` flags them in review, or, in a
    workflow whose landing is `observed`, through the repository's own merge, checked against `protectedPaths`
    after the fact (0.4).
- **Why not one board per branch.** Lines share one corpus and one set of sources, and a person's own views; a
  line's packs add views of their own (revision 22). Separate boards would duplicate the corpus and the sources and
  lose cross-line task state in the drawer.
- **Cost.** A line root is a full checkout. A provisioned orchard worktree takes 230–330 s, plus about 1500 s of map
  build (measured R1).
  - A line more than 50 commits from every existing map needs its own map (measured: the dev CLI README, the 50-commit
    window).
  - Two lines provisioning at once wait on orchard's map lock.
  - Tracks show all of this as provisioning.

### 0.13 Metrics

Every run is a trace. The core needed for run state is always recorded; the resource detail is opt-in.

- **Model.**
  - Spans are shaped like OpenTelemetry's. The trace is the run. A span is a phase, agent turn, tool call, hook,
    gate, provisioning step, subprocess or wait, with attributes.
  - Every span has an id, a kind and a parent id. Spans nest (a subagent's calls under its Agent call, a reported
    span under the span that owns the file), and the breakdown's three totals need the parent (0.11).
  - **A wait on the user is a span of its own kind**, with the id of the Queue item it waited on (S6). A phase's time
    then splits into work and waiting, and a question such as "where did this run wait" is answerable from the
    spans.
  - A span is measured on the continuous clock, with wall time kept for display. Active time is a span's length less
    the union of the waits inside it, so overlapping waits count once. Spans are kept in the store and survive a
    daemon restart.
  - Resource samples are tied to spans.
  - Everything is stored in SQLite next to the event log, per machine. Nothing leaves the machine in v0.
  - **Sleep is not work.** A laptop that sleeps mid-run would otherwise add hours of "active" time. The owner
    records sleep intervals and counts them as waiting. The clock table in 0.2 says which clock each timeout and
    countdown uses (R5). On macOS the uptime clock stops during sleep while the
    continuous clock does not, so the gap between them is the sleep: `slept = Δcontinuous − Δuptime`, computed on
    each sampler tick. A gap over 2 s (two ticks at 1 s) is recorded as a sleep interval and subtracted from active
    time.
    - Measured S8: awake, the formula gave 0.05–0.19 ms. Since boot the two clocks differ by the machine's total
      sleep, and the OS's own record of its last sleep lies inside that total. A dark wake runs the CPU, so it is not
      counted (inferred).
    - **No sleep mid-run has been tried.** The owner's step (`sleep-check.sh`) moves to Stage 3b (R5; it
      was deferred until before Stage 13), because the timeouts and countdowns that depend on the clocks are built
      there. It checks the table in 0.2 on a real run that sleeps.
  - **Missing is not zero** (owner, 2026-11-13, after Ferry's runner). A value its source did not give is stored as
    null and shown as "not reported". A value nobody asked for, because an opt-in was off, is shown as "not
    recorded". Neither is ever 0, which would read as "instant" or "free".
    - A total over spans with missing values shows the known part and how many spans lack the value.
    - Detectors and experiments never treat a missing value as 0.
- **Levels.**
  - Always recorded, because budgets, the Queue, the run preview and the no-pack view depend on them: span
    durations, wait time apart from active time, exit codes, tokens (including cache reads and writes), cost, the
    token and context metrics below, the shim's exit totals (CPU time and peak memory per reaped child), and disk
    space per workspace at phase end (owner, 2026-11-13). Reported data (0.5) is also always recorded, because a pack
    chose to send it.
  - `full`, opt-in per board or per run: resource samples, per-process attribution, network, device GPU (when a
    local model is declared), container and detached-process stats, pack span import, and the machine part of the
    fingerprint. Measured S8: the sampler costs 0.11% of one core at 1 s for a run's tree, and 0.08–0.09% on real
    Orchard runs. The UI shows its cost.
- **Sources:**

| source | gives | accuracy and granularity |
|---|---|---|
| Collide's own spans | phases, hooks, gates, provisioning, `record`, `integrate` | exact |
| Waits | Queue decisions, permission prompts, usage limits, `wait` phases, the concurrency cap, the line lock, queued writes, machine sleep | exact; shown apart from active time |
| Agent stream (stream-json) | turns, tool calls, tokens (cache reads, and cache writes split 1-hour and 5-minute), cost, wall and model time per turn, context size per response, compactions (`system/compact_boundary`) | input and cache tokens exact, counted once per `message.id`: the stream repeats one usage per content block (measured S9). Output tokens on those rows are partial, so per-turn totals come from `result.usage`; a compaction call, which `result.usage` leaves out, comes from `model_usage_since_last_result` (measured S9). A result's `duration_ms` covers its turn, but `duration_api_ms` (time in the model) and `total_cost_usd` are running totals over the process, so a turn's model time and cost are differences between results. A result with `num_turns: 0` (a `/compact`, a blocked prompt) reports `duration_api_ms` 0, so the difference is taken from the last result with turns (measured: S9's recordings). Timings approximate, because the owner tails log files |
| Agent stream through another adapter (ACP; S7) | turns, tool calls, and usage when the agent sends it | `Usage.quality` says how complete it is. opencode's per-prompt usage covers the last model step only (9,433 tokens reported, 18,847 spent; measured S7) and is zero after a cancel. For `Partial` usage, tokens are a lower bound shown as "at least", and cost is null, not 0, when the agent reports none. They are never summed with exact figures |
| Interactive sessions (New session, take-over) and `agentWork` phases | the same as the agent stream, plus per-call cost, time to first token, and the title and suggestion calls the transcript lacks | Claude Code's telemetry to Collide's local receiver (measured S2: tokens equal to the transcript's). A tool's `duration_ms` is the whole tool call, spawn included, not the command alone (a 2.0 s command, a 3.3–4.4 s call). The transcript is the fallback when the user's own endpoint is set. Only the phase's session ids count, followed across `/clear`. A `claude` that an agent starts from its Bash tool does not report, because Claude strips `OTEL_*` from its tools' environment (measured S2) |
| Past sessions (opt-in import; owner, 2026-11-13) | usage per response, tool names, timings, command shapes, compactions; never prompt, reply or tool content | from `~/.claude/projects/<repo key>/`, one usage per `message.id`: transcripts repeat usage per content block (measured in 84 of S1's 105 sessions). A baseline only; the contract test pins the format |
| Process exit totals (`wait4` rusage, from the shim) | CPU time and peak memory | exact for every child started through the shim, including across a daemon restart (measured S3). A child's total includes the descendants it waited for; orphans are excluded. Peak memory is the largest single process's, not the tree's |
| Process sampling: 1 s on the run's tree, 250 ms optional (S8) | CPU, memory and disk I/O over time (`proc_pid_rusage`; its CPU fields are mach time units, converted with `mach_timebase_info`, never a constant: 125/3 here, measured S3). Disk bytes come from the sampler, because `wait4` reports none (`ru_oublock` is 0; measured S8). Reads count only uncached I/O, so a build that re-reads warm files shows almost none | process-level, one point per second. A process is attributed to the span in its `COLLIDE_SPAN`; tool subprocesses are matched to tool calls by parent pid and time overlap. **Sampling is the time series and the shim's `wait4` totals are the truth.** It misses short processes at every interval: on three real Orchard runs, attached part-way through, the 1 s sampler missed 33–73% of the processes a 50 ms reference saw, yet caught 93–100% of their CPU, and the processes it never saw used 0–4.3% (measured S8). A synthetic workload spread over many short processes had only 60% of its CPU in the 1 s samples. Shortening the interval does not fix this: 50 ms costs 16 times the CPU and still misses most processes. The UI says that per-process rows are samples, and shows the missed share when a reference exists. Cost: 0.11% of one core at 1 s, 0.37% at 250 ms, 0.91% at 100 ms, 1.80% at 50 ms (measured S8, one synthetic tree), so the default stays at or above 250 ms |
| Disk space | size of each workspace and each cache directory | per workspace. R3 measured a provisioned orchard worktree at 4.1 GB |
| `nettop -P` | network bytes per process, without root | one streaming `nettop` per host while a `full` run is live, started with stdin as an open pipe that is never written: with stdin at end-of-file it spins at 56–134% of a core, and with an open pipe it costs about 1% (measured S8). It is read through a pty, because a pipe delivers 4 KB blocks and the first second arrived after 7–8 s. A closed socket's counters vanish, so a process's total is the sum of the stream's deltas, and the stream must run for the whole span. Per process only: no remote address, and a name cut to 16 characters |
| GPU | device utilization and memory (Stage 13), when a local model is declared; per-process GPU time (measured S8, collected from the first local-model adapter) | measured without root: `ioreg -r -c AGXDeviceUserClient` gives `AppUsage.accumulatedGPUTime` per client, with the owner's pid in `IOUserClientCreator`; `ioreg` IOAccelerator gives device utilization. Each `ioreg` call costs about 25 ms of CPU, 48 ms for both, so they are polled every 5 s (about 1% of a core for both). The device and the per-process counters are polled only when a process in the run's tree or its pack declares a local model (S8; owner, 2026-11-14). A run that declares none has no GPU column, rather than one that shows the desktop. A process's counter is cumulative busy time and ends with the process, so a delta needs two reads while it lives. It is shown as "GPU busy seconds" beside the device's utilization, which is never per process. A busy device is not the run's: on two real Orchard runs it reached 63% and 44% while no process of the run's tree had GPU time. In v0, Claude's inference is remote, and orchard's Ollama runs in Docker with no Apple GPU (measured R3), so per-process GPU matters only from the first local-model adapter |
| Containers and detached processes | lane CPU, memory, I/O, network (Docker API); the same for host processes a pack starts outside Collide's tree. **A run's lane containers are always counted** with the sampler (owner, 2026-11-14), never left as a setting | per container or process, from the Docker API one-shot at 1 s or slower (the daemon samples once a second), never the CLI: `docker stats` takes 2 s of wall time and 30 ms of CPU, an API call 8–14 ms (measured S8). In one real lane run the three lane containers did 612 s of CPU against 551 s in the host tree, so 53% of the run's CPU would be invisible to a sampler that watches only the tree (measured S8, one run). Containers' processes cannot be told apart, and a process whose pid was never reported stays unseen. The pack's `metrics.processes` hook returns a workspace's container ids and pids. orchard: lane containers carry only a per-clone label, so the pack reads each lane's record, which also gives its host server and UI pids (measured: the dev CLI's lane records). Containers need the hook because Docker Desktop runs them in a VM the host cannot see (measured: context `desktop-linux`) |
| Pack span import (`metrics.import`) | e.g. orchard's `iter-timings.jsonl`: stage, label, seconds, verdict, load average | Selected by the run's worktree paths (or the arm's own file), never by label alone. iter stamps sub-calls with their parent's start, so they nest as durations without a timeline position. Existing history (3545 lines, measured R3) is backfilled as a baseline without resources |
| Reported (`COLLIDE_REPORT`, 0.5) | spans, metrics and progress from hooks, gates, `script` and `wait` phases and their subprocesses, live; never from agent phases. `usage` lines from `agentWork` script phases (0.3) | as accurate as the reporter; labelled "reported". Reported usage is shown beside measured tokens and cost, never added to them. A tool that reports live makes its import a fallback |

- **Claude Code's own telemetry** (OpenTelemetry). The 2.1.323 binary contains `CLAUDE_CODE_ENABLE_TELEMETRY`,
  `OTEL_METRICS_EXPORTER` and `OTEL_LOGS_EXPORTER` (measured).
  - Interactive sessions and `agentWork` phases use it, with a local receiver (measured S2; owner, 2026-11-13).
    Headless phases keep the stream by default. S8 showed telemetry is more exact for four things: calls queued
    behind another call, permission and hook waits, model time with time to first token, and subagent nesting. A
    headless phase that needs exact timing may turn on the traces beta flag (`CLAUDE_CODE_ENHANCED_TELEMETRY_BETA=1`
    with `OTEL_TRACES_EXPORTER=otlp`).
    - Measured S8: `tool_result.duration_ms` equals the `tool.execution` span within 0–6 ms (30 calls). The stream
      overestimates an unqueued call by 8–149 ms (median 54 ms), and for a call queued behind another it is wrong by
      the whole wait (6.3–6.5 s against a true 3.1 s), because Claude runs Bash calls one at a time even when two are
      asked for together.
    - The flag is a beta, so the stream stays the fallback. Records reach the receiver up to a second late, so a live
      view shows stream timing and is corrected when the spans arrive.
  - Collide sets the export intervals short (`OTEL_LOGS_EXPORT_INTERVAL`, `OTEL_METRIC_EXPORT_INTERVAL`; defaults
    5 s and 60 s). At 1 s, log records arrived p50 978 ms after the event (measured S2).
  - **The receiver is the daemon's** (R5; 0.2): it binds `127.0.0.1`, checks `Host`, caps a request body and takes
    records only with the run's token.
  - Records are attributed by `session.id` and by `OTEL_RESOURCE_ATTRIBUTES` (`collide.run`, `collide.phase`). Each
    run's processes carry a token in `OTEL_EXPORTER_OTLP_HEADERS`, and the receiver refuses records without one.
  - Every record carries the account's identity (`user.email`, account and organization ids; measured S2). The
    receiver drops those attributes on arrival, before anything is stored.
  - `OTEL_LOG_USER_PROMPTS`, `OTEL_LOG_TOOL_DETAILS` and `OTEL_LOG_TOOL_CONTENT` are forced off (all measured in the
    binary).
  - Inherited `OTEL_*` variables are stripped, **after** the adapter has looked at them (R5): it records whether the
    user's environment or settings set an OTLP endpoint before the strip removes the evidence.
  - A user's existing endpoint is never overridden. When one is set, the session falls back to the transcript.
- **Token and context metrics** (owner, 2026-11-13; `docs/research/tokens-and-context.md`). They need no pack:
  - per response: input, cache read and cache write (1-hour and 5-minute) tokens, and the context size; output
    tokens per turn, from `result.usage`, because the stream's per-response output count is partial (measured S9);
  - per turn: wall time (`duration_ms`) and model time (from `duration_api_ms`), so a turn splits into the model's
    time and the rest: tools, hooks, MCP start-up and permission waits. In S9's one-turn runs the time outside the
    model was 2.4-4.0 s under `config: user` with the user's hooks, and 0.2-0.7 s under `config: none` (2.1 s in
    the one run that loaded the claude.ai connectors; measured, one run per line);
  - other model calls, per turn and model: `model_usage_since_last_result` minus `result.usage`. That covers
    compaction summaries and helper-model calls (read: a Ferry fixture whose `total_cost_usd` includes a haiku
    call that `result.usage` lacks). `modelUsage` is a running total over the process, so it is not used per turn
    (measured: S9's recordings);
  - a total sums turn, compaction and other-call usage, never a response's own usage or a running total. Partial
    figures are "at least" and kept apart from exact ones;
  - a phase killed before its `result` keeps the input and cache tokens of the responses seen so far; its output
    tokens and cost are "not reported", never 0;
  - per phase:
    - the start weight (the first response's context) and the peak context;
    - compactions, from `system/compact_boundary`: trigger, tokens before, duration, and whether one landed
      mid-task (the response before it holds an open `tool_use`). "After" is the next response's context, because
      the event's `post_tokens` runs far lower (10.4K against 33.0K; measured S9). A compaction's cost comes from
      `model_usage_since_last_result`;
    - the cache hit rate, `cacheRead / (input + cacheRead + cacheWrite)` over the phase's responses, or "not
      reported" when the source gives no cache fields. Reads and writes are never merged into one number, because
      a 1-hour write costs about 20 times a read (inferred in S9 from a cost fit). Ferry's runner merged them, and
      Ferry counted writes as hits;
    - cold returns with their cause: a Queue wait, a usage limit, a take-over or an owner restart;
    - per tool name: calls, refused calls by who refused them (0.3), tool errors among the calls that ran,
      tool-result bytes, and total, median and maximum time. Bash also splits by the command's executable. Summed
      time is kept apart from wall-clock time and from the union of call time, because parallel calls overlap (part K;
      0.11). A refused call is never
      counted as a tool error;
    - per hook: calls, blocks and failures (exit 1 or a timeout), from `hook_response` (0.3).
  - Seven detectors work over these shapes, never over content: cold return, heavy start, compaction mid-task,
    oversized tool output, the same command failing again unchanged, a hook that failed open, and a call to a tool
    the session does not have. Each finding says what happened, what it
    cost (measured) and what a change would save (estimated).
    - Oversized tool output shows in the tool result: `persistedOutputPath` when Bash output was cut and saved to a
      file, and the error `exceeds maximum allowed tokens` (with `is_error: null`) when an MCP result was dropped
      (measured S9). That error is Claude's own fixed text, not tool content.
    - A hook that failed open is a `HookFailedOpen` event: a `hook_response` that is `cancelled`, or `error` with an
      exit code other than 2, or a `hook_no_verdict`, where the tool then ran: a guard that did not guard (Claude
      treats exit 2 as a block and any other non-zero exit as a non-blocking error). A call to a missing tool costs a turn and usually means the agent definition and
      the prompt disagree (measured: S9's addendum, where the model called `Bash` after it was disallowed).
    - Hook denies and `PostToolUse` blocks are counts per hook and tool, not detectors: a guard doing its job is
      not a problem.
  - Every number is labelled measured, inferred or estimated. A projected saving is never shown as measured.
  - Collide's own runs use the cost `claude` reports. Imported sessions carry none, so their cost is estimated from
    a dated rate-card file shipped with Collide, and labelled with the card's date.
- **Fingerprint** on every run:
  - always:
    - the line's ref and commit;
    - each pack's name, source and tree id, with a `dev:` pack marked, and the `board.yaml` hash;
    - the profile, and a hash of its resolved values;
    - the Collide, claude and node versions (the claude version is the one the daemon read for this phase, 0.3);
    - `apiKeySource` from `system/init` (R5): whether the login came from a claude.ai session, an API key or a helper,
      because the cache tier and the cost differ (0.3). The run preview shows it before the run starts, from the
      captured environment, and the fingerprint records what `system/init` then said;
    - the model each alias resolved to;
    - the context options in effect, and the names of the user hooks and plugins the phase loaded (0.3);
    - the host of the model endpoint when `ANTHROPIC_BASE_URL` is set, from the environment or a `settings.json`
      `env`: the host only, never the path, query or a key (0.3);
    - the tools the phase had: its agent definition's `tools:` list and the disallowed tools. A tool that is only
      pre-approved, not removed, still costs context (0.3);
    - whether the workspace was fresh or reused;
    - the cache state the pack reports (orchard: whether a map is in range, and its distance);
  - with `full`: the machine, and load sampled throughout the run.
- **Critical path** is computed over the engine's phase graph: phases, gates and hooks in sequence, where a fan-out
  waits for its slowest instance.
  - Waits (human, line lock, concurrency cap) are their own category beside active time. They are never folded into
    a "shorter if this took no time" estimate.
  - Below phase level the waterfall shows self time, not a critical path. Spans form a tree, not a dependency
    graph.
- **Privacy.**
  - By default, a process is recorded as its executable's basename plus its flag names: no values, no positionals.
    A tool's command line is tool content. One exception: the first positional is kept when it is a lowercase safe
    token of at most 32 characters, such as a subcommand (`cargo test`; owner, 2026-11-13).
  - Full argv is a separate opt-in. It removes flag values and env names on a redaction list, and strips
    credentials from URLs (`postgres://user:pass@…`). It still cannot see inside `sh -c` or `node -e` strings, and
    says so.
  - Imported pack lines are reduced the same way.
  - Sharing a planning proposal commits run summaries without spend, unless the board sets `share.spend: totals`
    (owner, 2026-11-13). Transcripts, prompts and metrics never leave the machine.
- **Volume and retention.** Sample rows take about 34 bytes each: 0.6–6 MB per hour at 1 s for 5–50 live
  processes (measured S8; 2.5–24 MB at 250 ms). Retention is set per board, default 90 days.

### 0.14 Experiments (A/B)

An experiment runs one task under two **arms**, one after the other on this machine, and compares the results.

- **An arm** is a line plus launch parameters, env overrides, a profile and pack overrides (owner, 2026-11-13).
  - A line can be any ref, so a change to a repo script, a hook or a pack is testable.
  - Parameters and env cover changes that were built as toggles.
  - A profile covers a whole setup. A pack override puts one pack at another ref or source, so pack v1 and v2
    compare without a branch. A `dev:` pack cannot be pinned, so an arm refuses it.
- **Arms differ only in what the user names.** Collide pins everything else at the start: resolved models, budgets,
  launch parameters and a permission allowlist.
  - The task is read at each arm's ref. If its text differs between arms, Collide shows the difference, and the user
    must accept it.
  - A permission prompt outside the allowlist is denied and recorded, rather than waiting on a human.
- **Isolation.** Each row says what an experiment does to a kind of state:

| state | in an experiment |
|---|---|
| Task files at real line roots | untouched: ref-scoped writes go to the arm's experiment root |
| Global sources (GitHub) | no `apply` calls are made; the intended operations are recorded |
| Commits | `record` commits on the arm's experiment root, whose branch is under the experiment prefix; `integrate` never runs. Before each repetition, Collide resets the experiment root to the arm's pinned commit, so the task is launchable again. It resets only roots it created |
| Branches and worktrees | workspace hooks get a branch prefix (the pack's `experiment.branchPrefix`, default `collide-exp/`, plus `<id>/<arm>/<repetition>/`) and create branches under it; `discard` removes the worktrees, containers and branches |
| Disk | an arm's task workspaces and sandboxes are discarded once its metrics are captured, not at the end. An orchard worktree is about 4.1 GB (R3), and a long experiment would otherwise hold a hundred GB or more |
| State a pack's tools share across worktrees | the pack's `experiment.prepare` and `experiment.env` give each arm its own copy (orchard: iter timings, and codemaps seeded from the current ones). Each arm also gets its own copy of the pack's data directory (0.7). Hooks see `experiment` in their input and skip side effects (orchard: the registry line). The pack declares `unchanged` paths, which must be byte-identical afterwards, and `appendOnly` paths, which other work may append to but whose earlier bytes must not change |
| The agent tool's own state | declared by the adapter: env that turns it off, and paths to inventory. The Claude adapter sets `CLAUDE_CODE_DISABLE_AUTO_MEMORY=1` (in the 2.1.323 binary) and lists the repo's memory directory. An adapter that declares nothing gets a warning in the preview |
| Anything an agent's Bash touches elsewhere | not isolated (same-user reality); the inventory diff reports what it can see |

  An **inventory diff** runs before and after every experiment, and any difference is reported. Core contributes
  only what it owns: `git for-each-ref` and `git worktree list`, because workspaces are git checkouts (0.1). The rest
  is declared:
  - by the pack: hashes of its `unchanged` paths, the prefixes of its `appendOnly` paths, and the containers and
    processes its `metrics.processes` hook reported for the experiment's own workspaces, compared by id;
  - by the agent adapter: its state paths.

  Other work can change shared state while an experiment runs. A difference therefore counts against the
  experiment only if it waited for other runs. With "start now", differences are listed beside the number of runs
  that were live.
- **Token strategy preset** (owner, 2026-11-13). The arms are two profiles that differ by one context option (0.3):
  `compact_at`, `config`, `bash_output_max`, `mcp_output_max`, the model, `on_cold`, or an added plugin such as an
  output-condensing hook. Each option changes the stream (measured S9), and its effect is read from
  `compact_boundary`, the persisted-output keys and the MCP error text.
  A pack can supply other arms as hooks, such as a read budget that denies `Read` after N calls (Ferry's runner
  has one, with no measured effect). None is a core default.
  The result rows are cost, tokens, cache hit rate, compactions, files read again, tool errors and active time, beside
  the outcome: gates, CI and accepted. A saving shows what it cost. The verdict rule is unchanged.
  - A change made outside an experiment (a setting, a pack version) gets a before/after comparison split at the
    fingerprint change. It is labelled inferred and never gives a verdict.
- **Changed measuring tools.** An arm on a branch brings its own gates, reviewer, briefs and iter.
  - When `protectedPaths` differ between the arms, the result marks its outcome comparison "measured by different
    gates", rather than hiding the difference.
  - Re-judging B with A's gates is deferred.
- **Design choices.** The user makes each one when creating the experiment. The preview explains every option and
  shows its cost in runs, tokens and expected time, so the user can trade rigour for money and waiting.
  - **Order.** A later run benefits from what earlier ones warmed: package stores, build caches, the OS file cache,
    the model provider's prompt cache, and any persistent state the pack's tools keep (orchard's codemap: the first
    run in its window pays about 1500 s). `experiment.prepare` gives every arm the same starting copy of the pack's
    state. The other caches stay shared.
    - `AB`: each pair runs A, then B. The fewest runs and the fastest first answer. B always runs warmer, so an
      order effect is mixed into the difference. The result says "order not balanced" and gives effect sizes only.
    - `ABBA`: pairs alternate (A B, then B A). A steady order effect cancels out, and the result reports its size.
      Pairs come in twos, so the smallest `ABBA` experiment is four runs, not two. The order is fixed rather than
      random, so it gives effect sizes only.
    - `crossover`: each pair's order is drawn at random from a recorded seed, with as many A B pairs as B A pairs.
      It is the only design that can give a verdict, and it needs at least 8 pairs (see Comparison).
  - **Pairs.** One or more, fixed when the experiment is created. More runs later make a new experiment. Its results
    sit beside the first one's but are never pooled into one verdict, because adding pairs until a result looks good
    inflates false positives. Effect sizes from a few pairs are often enough to decide whether an idea deserves a
    verdict.
  - **Warm-up.** An optional run of A whose results are discarded. Recommended when provisioning is compared,
    because the first run fills caches that later runs reuse.
  - **A/A baseline.** Optional pairs of A against itself (see Comparison).
  - **Other runs.** The preview shows how many other Collide runs are live, and how many of the pack's containers
    are running. The user starts now, or has the experiment wait until no other runs are live. Either way, the
    number of concurrent runs and the machine load are recorded throughout, and a result whose runs overlapped
    other work is flagged with that number.

  Every result records its position in the order.
- **Comparison:**
  - The user picks the **primary metric** when creating the experiment. The default is active time on the critical
    path.
  - The primary metric comes only from what Collide measured or from spans that hooks reported. A reported line
    counts only when it is the hook's own (0.5): a line a child wrote through an inherited descriptor is never
    primary, because the code under test may have written it. A `metric` line from the code under test is never
    primary either.
  - **Verdict rule** (`crossover` only): the exact crossover test.
    - Each pair gives its period difference (first run minus second). The test compares the A B pairs' differences
      with the B A pairs' over every way of splitting the pairs into two equal groups. That also removes a steady
      order effect.
    - With n pairs, the smallest two-sided p is 2/C(n, n/2): 0.10 at 6 pairs, 0.029 at 8 and 0.008 at 10 (derived).
      So a verdict needs at least 8 pairs.
    - Secondary metrics use a Holm correction among themselves; the primary is not part of it. A secondary can pass
      only if the smallest possible p is at most 0.05 divided by their number. So 8 pairs support one secondary
      metric and 10 pairs up to six. The preview says how many the chosen pair count supports.
    - Every other design, and a `crossover` below 8 pairs, shows effect sizes with their ranges and says
      "no verdict".
    - A verdict is about this task. The result says "on this task", never that a workflow is faster in general.
  - An **A/A run** can come first, and B−A is then shown against the A/A spread. R3 measured why this matters: the
    same orchard suite spans 1.5–6× across passing runs, and `worktree new` took 330 s and 230 s back to back.
  - **Agent work is compared per phase kind** (build, review, all fixes, close), because rounds differ: one task
    reached `fix7`.
  - **Outcomes:** gate verdicts (`unverified` is its own outcome, never a pass), review rounds to close, findings per
    round, `ask_human` count, tests passed and diff size. Faster is not better if quality drops. Outcomes come from gate
    results and Collide's own records, never from reported lines.
  - Human waits are excluded. Provisioning is reported separately.
- **`collide bench`** runs only an arm's repeatable steps, N times on a fixed commit, with no agent. It is the cheap
  way to test a provisioning or gate optimization, and the cheap way to a verdict: it uses `crossover` with 10 pairs
  by default, and runs A/A pairs automatically.
- **Abort** stops the current arm, runs `discard` and the inventory diff, and keeps what was measured.
- **Cost.** No experiment spending cap (owner, 2026-11-12). Per-role budgets apply, and the preview shows the
  total.
  - An Orchard loop takes hours. Opus R4 summed one task's iter seconds at 2.5–7.7 h. That is an upper bound,
    because sub-calls nest.
  - So 8 crossover pairs, sixteen runs in sequence, take days. The preview shows that before anything starts.

### 0.15 How the code is built

Skeleton first, then test-first passes (owner, 2026-11-13).

- **Spikes come first.** S1, S3, S5 and S7 decided the Claude launch line, the process owner, the YAML and CEL crates
  and the shape of `AgentAdapter`. An outline written before them would have been rewritten.
- **Chosen crates and packages** (S5; owner, 2026-11-14). Beyond the platform's own, the plan names these. All three
  Rust crates are pure Rust, so `core` may use them:
  - `schemars`: Rust types to JSON Schema (0.6);
  - `serde-saphyr` 2.x: YAML in Rust, with the hardened option set (0.7);
  - `cel` 0.15, patched and vendored: views and `when:`, with a step counter (0.9);
  - `json-schema-to-typescript`: JSON Schema to TypeScript, a Node build step only, never shipped (0.6);
  - `yaml`, in its default core mode: YAML in TypeScript, such as the markdown provider's frontmatter (0.10);
  - shadcn/ui, on Radix primitives and Tailwind, for the app's components, copied into `app/` so the code is the
    repo's own (owner, 2026-11-15; revision 17, after comparing MUI on the Tracks mockup). Its look follows 0.11's
    audience rule, not shadcn's defaults.

  Not used: `specta`, `ts-rs`, `serde_yaml` (archived), `gray-matter`, any YAML parser's 1.1 mode, and MUI.
- **Pass 1, the skeleton (Stage 2).** Two parts (owner, 2026-11-13, choosing R4's middle option):
  - **Every cross-process contract, in full:** the hook protocol (stdin, result file, report lines), the provider
    and observer protocols, the JSON-RPC between UI or CLI and the owner, the event log, and the schemas for
    `pack.yaml`, `board.yaml`, `config.yaml` and views. That includes the point and event catalogues (0.4) and keys
    used only in Milestones C and D, so a manifest written today keeps validating. It also includes the keys that
    only GitHub Projects v2 needs, though Projects is built after the board (Stage 6c; owner, 2026-11-14): the
    provider contract, the status mapping and the fields are frozen in full now, and Stage 6c adds code, not
    contract.
  - **Code outlines for Milestones A and B** (Stages 3–10):
    - Rust crates and modules, traits, types, state enums and function signatures;
    - the SDK's functions and types;
    - the shell's `rpc` command and event channel;
    - the React component tree for Field, Tracks, Queue, run detail (Output, Time breakdown, Review), Compare and
      Experiments, following S6's surfaces, with typed props, and one reducer for UI state in which every user
      action is a reducer action, as in S6's prototype, so reducer tests need no browser. So is every call of the
      in-app agent's tools (0.11), with a snapshot of the state it replaced for Undo.
  - Milestones C and D are outlined by the first step of their first stage (Stages 11 and 13). By then a real loop
    has run, and their outlines are written against it rather than guessed.

  Details:
  - Bodies are stubs tagged with a stable name for the work that fills them: `todo!("engine: fan-out join")` in
    Rust, `unimplemented("engine: fan-out join")` in TypeScript. The skeleton keeps a table of tags and the stage
    that fills each one. Tags are names, not stage numbers, because the stages have been renumbered three times. The
    tag plays the role of the issue number that a TODO needs.
  - Data types are written in full, because a type is a definition, not behaviour. So the schemas, the generated
    TypeScript types and manifest validation are real from the first day.
  - Outlines will change once earlier stages teach something. Each stage starts by revising its part of the
    outline. Changing a signature is cheap, which is why outlining comes first.
- **Pass 2 onward, the fill (Stages 3–15).** Each stage:
  1. revises its outline;
  2. writes its tests and sees each one fail;
  3. implements until they pass;
  4. refactors with them passing.

  A stage is complete when CI counts zero stubs carrying its tags. A stage split into lettered sub-stages (3a, 3b, …)
  completes one sub-stage at a time, each with its own tags and its own success criteria.
- **Builders are Sonnet agents** (owner, 2026-11-14).
  - Each Stage 2+ item is sized and written so that one Sonnet agent can finish it in one session from the item file
    alone. The item names the plan sections and contracts it implements (by section, never by line), the crate or
    package area it touches, the tests to write first, and when it is done.
  - Design reviews stay with Fable and Opus. The main session spot-checks each result.
  - This strengthens the split of large stages (S1, R5): a stage too big for a handful of such items is split into
    lettered sub-stages, and only the next stage is split into items (0.10).
- **Every commit passes all tests** (the owner's standing rule). So a test lands in the same commit as the code that
  makes it pass, and its failure is observed locally first.
  - A failure caused by a stub only proves the code is missing.
  - So each success-criteria test is also observed failing on its own assertion once: break the implementation,
    see the test fail, restore it. The commit message says how.
- **Dependency injection.** Core logic takes its collaborators as the core traits, so tests use fakes and the engine
  runs without processes.
- **Checks from the skeleton on:**
  - `core` depends on no framework, storage or process crate;
  - no reference-pack name appears outside pack directories and test fixtures;
  - no Claude env var name (`CLAUDE*`, `ANTHROPIC_*`) appears outside the Claude adapter (0.1);
  - from the engine stage on, both fixture packs pass the engine suite with a fake adapter that emits normalized
    events, so no engine test depends on Claude's stream format.

---

## Milestone A: Foundations

Ends usable from the CLI and a read-only desktop board, with a thin loop in the app along the way (owner,
2026-11-14):
- type a prompt in the app, run it through `collide/quick` in a worktree, review the diff and accept it, against
  scratch tasks (Stage 3c), as soon as the process owner and the Claude adapter work. This is the first dogfooding:
  Collide's own work can go through it before the plan adapter, GitHub or the board exist;
- browse the corpus through views, in the app;
- write tasks from the CLI;
- run one agent phase on one task.

## Stage 1: Spikes

**Goal**: Answer the questions the plan depends on, with numbers. Throwaway code; each spike leaves a runnable
reproduction script and a record, together in one spike folder under `.collide/planning/spikes/` (0.10).

- **S1 Claude headless** (done: `.collide/planning/spikes/sp-0001-de-claude-headless/record.md`).
  - Multi-turn stream-json.
  - `--permission-prompts host`, answered from our process.
  - `--agents <file>` built from a `.claude/agents/*.md` file, selected with `--agent`. Which definition wins
    when a worktree has a conflicting `.claude/agents/<name>.md`?
  - `--max-budget-usd`.
  - Is `--verbose` needed?
  - The final message as the phase report; resume by session id; cancel mid-turn.
  - Do repo hooks (rtk), `.mcp.json` servers and user plugins load under `-p` in a worktree?
  - **Settings from the snapshot (0.3).** With `--setting-sources user`, `--settings` and `--mcp-config` from a
    snapshot, and `--strict-mcp-config`: does a settings hook, MCP server, skill or CLAUDE.md that was edited in the
    worktree still load? List what does.
  - What the stream shows when a usage or rate limit is hit, and whether it carries a reset time. Use a recorded
    session if a limit cannot be triggered on purpose.
- **S2 interactive** (done: `.collide/planning/spikes/sp-0002-gm-interactive/record.md`; its addendum measures the
  menu bar's memory).
  portable-pty → `ipc::Channel` → xterm.js with Claude's TUI: latency, fidelity, resize.
  - Take over a headless session with `--resume`. The PTY lives in the shim, so a take-over survives an app restart
    and a daemon restart; if it cannot, the record says so.
  - The shim holds an agent's stdin and the PTY master across a daemon restart (0.2). A shim killed with SIGKILL
    makes its phase `lost`.
  - The daemon started as a one-shot launchd job from a real Tauri `.app` and from Terminal: compare coalition ids
    (`proc_pidinfo` flavor 20), then Cmd-Q, force-quit and closing the terminal. The daemon and its runs survive.
  - Metrics for interactive sessions: Claude Code's OpenTelemetry to a local receiver, against tailing the
    transcript. Which gives tokens and tool timings, and how precisely?
  - The session id after `/compact` or `/clear` in a take-over, so a later headless resume uses the right one.
  - Whether a `claude` started by a `script` phase, not by the adapter, reaches the same receiver when the phase
    passes the receiver's settings, so `agentWork` usage can be measured (0.3).
  - Left unmeasured: a real Force Quit dialog, logout and a signed release build; the DOM key path; telemetry from
    a container.
- **S3 process ownership** (done: `.collide/planning/spikes/sp-0003-ye-process-ownership/record.md`). Options: (a)
  `collide-daemon`, (b) app-owned children with pid files, (c) either, with `claude --bg` for agents. For each:
  - Quit and relaunch the app mid-run, and restart the owner, with nothing lost.
  - With the app closed, a phase ends and the next phase starts (0.2). An option that fails this is acceptable only
    with runs that visibly pause between phases while the app is closed.
  - Stop a tree that includes a setsid'd grandchild, a double-forked orphan and a Docker container (through the
    cleanup hook): zero survivors, checked with `ps` and `docker ps`.
  - Which exit-time resource totals survive (0.13)?
- **S4 TS hooks** (done: `.collide/planning/spikes/sp-0004-vu-ts-hooks/record.md`).
  - Materialize a pack from a git snapshot into the cache and link the bundled SDK. Imports resolve, and a hook
    runs.
  - Hook cold start including the SDK.
  - An app launched from Finder finds `node`, `claude`, `pnpm`, `gh` (login-shell environment captured).
- **S9 tokens and context** (done: `.collide/planning/spikes/sp-0007-zf-tokens-context/record.md`; owner, 2026-11-13;
  `docs/research/tokens-and-context.md`). About $0.85 of spend, haiku only.
  - Does stream-json repeat usage per content block? Does it emit `compact_boundary` with its metadata (a manual
    `/compact` on stdin makes this cheap)?
  - The cache tier under an API-key login, if a key is available.
  - Start weight under `config: user` and `none`, with the breakdown `system/init` gives.
  - The effect of `CLAUDE_CODE_AUTO_COMPACT_WINDOW`, `BASH_MAX_OUTPUT_LENGTH` and `MAX_MCP_OUTPUT_TOKENS` set in a
    phase's environment.
  - What the stream shows when a `UserPromptSubmit` hook blocks a resumed prompt. The hook comes from a spike-local
    `--settings` file, never the user's.
  - One pair: a resume after more than an hour idle against a fresh session with a handoff note, for cost.
  - Left unmeasured: the cache tier under an API-key login; Sonnet and Opus token counts; whether a handoff note
    keeps the work going as well as a resume; `get_context_usage` across versions.
  - Addendum (owner, 2026-11-13; about $0.03, haiku, 2.1.325): how hook denies, hook `ask`, hook failures,
    `PostToolUse` blocks, host denies and disallowed tools show in the stream, with and without
    `--include-hook-events`. Left unmeasured: tools other than Bash, MCP tools, a hook's `allow` or `updatedInput`,
    hooks in subagents, Sonnet and Opus, and the default hook timeout.
- **S10 planning seams** (owner, 2026-11-13; $3 cap). It runs when the group-runs stage starts:
  - Q1: a fresh agent working from a handoff note built at Share, on a second clone, against the resumed session;
  - Q2: whether fan-out instances stay independent (files, `phase.dir`, auto memory);
  - Q3: how often two spike records merge mechanically.
- **S11 derived-point coverage** (done: `.collide/planning/spikes/sp-0008-bk-derived-point-coverage/record.md`;
  owner, 2026-11-13). A paper spike: nothing was installed or run.
  - Do the point catalogue and the `change` payload cover every real touchpoint of the plan CLI, beads, GitHub issues,
    Obsidian and the markdown provider? The planning note's Q11 (no point for changes Collide only reads) was
    measured per backend.
  - Result: 69 touchpoints, ten core gaps (G1–G10) and six pack gaps. G1, writes a run's agent makes through the
    backend, is the largest. Applied in part J (0.4, 0.5, 0.10, the skeleton, workflow-engine, run UI, group-runs
    and second-pack stages, Deferred). `docs/research/planning.md` is corrected where S11 found it wrong.
- **S5 schema and expressions** (done: `.collide/planning/spikes/sp-0005-fv-schema-expressions/record.md`).
  - Rust → JSON Schema → TS.
  - Choice of YAML crate and CEL crate.
  - YAML hazards: `no`, `0.10`, unquoted `{{ }}`.
  - The CEL cost limit: a nested-macro expression over 1567 tasks is stopped within the limit.
  - Result: `schemars` then `json-schema-to-typescript`; `serde-saphyr` 2.x hardened, and `yaml` in core mode in
    TypeScript; `cel` has no limit, and the step-counter patch stops the plan's nested example within the limit
    (applied in revision 11: 0.6, 0.7, 0.9, 0.10, 0.15).
  - Left unmeasured: `specta` through a real Tauri command; a Rust JSON Schema validator; the step counter across
    threads; memory bounds during evaluation; the real Orchard corpus through CEL; Windows and Linux.
- **S6 interface prototype** (done, with its sessions skipped:
  `.collide/planning/spikes/sp-0009-bb-interface-prototype/record.md`). A clickable prototype of Field, Tracks and
  Queue (0.11) over fake data: fake-agent streams and an Orchard-shaped corpus. Nine scenarios:
  1. A fresh repo with no pack and no tasks, through to a finished run, reviewed.
  2. Build a view from the filter bar and save it.
  3. Launch a group of 3 tasks from a view.
  4. Clear 5 pending decisions.
  5. Review a finished run and accept it.
  6. Compare two finished runs and say where the time went.
  7. Set up an A/B experiment from two branches and read its result.
  8. Watch a live run's output. Then find the tool calls in one phase that took over 2 s between two clock times,
     and jump from the slowest to its output (part K).
  9. Decide three Queue items (a command, an edit and a failed gate) from their context view alone, and note what
     was missing (part L).

  Open questions from the owner's three mockups (Field-first, Queue-first, Tracks-first), for S6 to settle (owner,
  2026-11-13). The sessions were skipped, so each takes its default (owner, 2026-11-14), which the owner may change
  while iterating on the prototype:
  - **Run identity against state colour.** Colour is reserved for state (0.11). Default `id`: a run's id label.
  - **`a` and `d` per Queue kind.** Default `literal`: the table in 0.11.
  - **`o` from a Queue item** opens its run's track at the moment of the decision. Default `moment`.
  - **Where the line selector lives.** Default `top`: a menu in the top bar.
  - **"review ×2" means two reviewers in parallel (A and B), not two rounds.** The mockups showed rounds. The
    prototype's data is corrected.

  The owner set time and error thresholds before the sessions (the record's "Thresholds"). They stay recorded and are
  advisory, not a gate. The planned sessions, with the owner and at least one person who did not design the
  prototype using it for the first time, did not run: the owner has no participant who did not design Collide
  (owner, 2026-11-14). S6's proposed usability gate with a first-time user is withdrawn. Its scripted pilots (50 of
  50 tests pass) show each scenario is completable and its end state detected, and nothing about how long a person
  takes or where they go wrong. **From here, an S6 scenario is checked in the real app by two things:** a UI test
  ported from S6's pilot, which completes the scenario and detects its end state, and the owner's walkthrough,
  recorded as owner judgement in the stage's task item.
  - Left unmeasured: all human performance, learning and carry-over, scale (140 tasks), real agent text, other
    screens and the light theme.
  - **The in-app agent** (added after the pilots, at the owner's request, 2026-11-14): five MCP tools, an ask box and
    an Agents page, tried live with Claude Code haiku (nine runs) and with opencode on a local model (three runs),
    with 47 tests beside the 50 above. Revision 12 applies it (0.2, 0.3, 0.7, 0.9, 0.11; Stages 4, 6, 9, 11). Left
    unmeasured: the token load of a strict launch line, a hosted opencode model, a second local model, the ACP
    permission path under a real write attempt, and a window in the background.
- **S7 `AgentAdapter` against ACP (paper check)** (done:
  `.collide/planning/spikes/sp-0006-kq-agentadapter-acp/record.md`).
  - Map the trait's launch spec, events, permission answers and capabilities onto ACP messages.
  - Run `opencode acp` once by hand with one Ollama model.
  - Record every trait member that ACP cannot express, and change the trait before the skeleton (Stage 2)
    outlines it.
  - Nothing ships.
  - Result: fourteen gaps, each declared as a capability or an optional field. The trait is two traits (0.3).
  - Left unmeasured: other ACP agents (goose, Zed's Claude adapter), a real limit, hook or compaction, `terminal/*` and
    `elicitation/*`, signals. S6 later measured an HTTP MCP server through `session/new` with opencode 1.1.56.
- **S8 metrics sources** (done, with two owner steps deferred:
  `.collide/planning/spikes/sp-0010-bn-metrics-sources/record.md`). On an orchard `iter` run and a lane test:
  - Sampler overhead at 1 s, and at a shorter interval if 1 s misses too much.
  - The share of short-lived processes that sampling misses, compared with `wait4` totals.
  - Per-process disk I/O through `proc_pid_rusage`.
  - Parsing `nettop -P` per process.
  - GPU:
    - per-process GPU time from `AGXDeviceUserClient`;
    - device utilization from IOAccelerator;
    - both under a host-run local model.
  - Docker API stats, and detached host processes, through a pack's `metrics.processes` hook.
  - Sleep detection: put the machine to sleep mid-run, and check that the gap between the uptime and continuous
    clocks matches the sleep.
  - Store growth per hour of run at the chosen interval.
  - Claude Code telemetry received locally: does it give tool timings more precise than tailing stream-json?
  - Report files: tailing lag under a chatty writer, and the size and rate caps.
  - Result: 1 s sampling at 0.11% of a core; three real Orchard runs measured, one with its lane containers.
    The mid-run sleep test moved to Stage 3b (R5). Deferred (see Deferred): a run measured from its start.

**Success Criteria**:
- Ten decision records (S1–S9 and S11), with measured numbers.
- The process owner is chosen, and the record says whether runs continue between phases with the app closed. The
  Claude launch line is fixed, including where settings come from. The snapshot + SDK path works.
- S1, S3 and S4 ran first. Their records were read against this plan before the other spikes started (owner,
  2026-11-13). Done: revision 10 applies them, S2 and S9 (part E), and S11 (part J). Revision 11 applies S5–S8.
- An adversarial review, **R5** (Fable and Opus, read-only, as in R1–R4), attacked the finalized plan, and the
  owner's decisions on its findings are applied in revision 13. No product code is written before the owner accepts
  revision 13; spike code is throwaway and exempt (owner, 2026-11-13).
- S6: its pilots complete each scenario with the end state detected (measured: 50 of 50 tests), and the record
  lists what changed in the prototype and why. Its in-app agent answered both of the owner's requests live (0.11).
  The sessions with a first-time participant were skipped (owner, 2026-11-14), so the owner's judgement on the
  prototype stands in, and the later stages check each scenario with ported UI tests and an owner walkthrough.
- The trait has no member that S7 found ACP cannot express, or each such member is marked as a capability.
- S8 names a sampling interval whose overhead is below 1% of one core. It states each resource's granularity and
  what it cannot attribute.

**Tests**: Each reproduction script runs from a clean checkout and prints its measurement.
**Status**: Complete except S10, which runs when the group-runs stage starts (S1–S9 and S11 done; revision 11 applies
S5–S8; revision 13 applies R5). Items: epic `ep-001-ef` in `.collide/planning/epics/`; records in
`.collide/planning/spikes/` (part L).

## Stage 2: Skeleton

**Goal**: Every cross-process contract is defined in full, and Milestones A and B exist as outlines that compile and
do nothing (0.15). One piece works for real: the planning slice, so Collide tracks its own build from here on (0.1;
part L).

- Repo layout:
  - the `core` and `host` crates, `app/src-tauri` and `app/` (React);
  - `packages/pack-sdk`;
  - two fixture packs: a review loop (`fixtures/packs/loop`) and an issue → PR → CI shape (`fixtures/packs/pr-ci`);
  - a fixture copy of the reference pack's manifest and hooks (`fixtures/packs/orchard`). The pack itself lives in
    the Orchard repo (0.7);
  - the two built-in packs (0.4): `packs/collide` (workflows such as `collide/quick`) and `packs/plan` (`collide/plan`,
    the markdown provider the planning slice runs on).
- Contracts in full, with JSON Schema and TypeScript types generated from them. The pipeline is `schemars` →
  JSON Schema → `json-schema-to-typescript`, a Node build step (0.6; S5):
  - the rules of 0.6 hold from the first schema: `additionalProperties: false` on every struct and tagged variant,
    `Wait {}` rather than a unit variant in tagged enums, and separate input and output generations;
  - YAML loads through `serde-saphyr` 2.x with the hardened option set of 0.7, pinned by a test, and TypeScript
    reads YAML with `yaml` in core mode (0.10). The Rust side checks the schema through serde's
    `deny_unknown_fields`; S5 did not measure a Rust JSON Schema validator, so this stage picks one or keeps serde
    alone, and says which;
  - `pack.yaml` (including `config`, workflows as transition tables, and the keys used only in Milestones C and D),
    `board.yaml`, `config.yaml`, views, profiles, `packs.lock` with its `source` field, the user's `packs.yaml`, and
    the `.collide` pointer file;
  - the hook, provider and observer protocols, and the report line format;
  - the shim protocol, versioned on its own, and `exit.json`, with the shim's **journal** (R5; 0.2): one ordered file
    of `{seq, wall_ns, dir, bytes}` records, written before the child sees the bytes, and the length-framed
    daemon-to-shim messages. `AgentSession::feed` takes the storable `wall_ns` and `replay` takes the journal (0.3);
  - the hook result with `pending` and `retryAfter`, and `interval:` and `timeout:` on a `wait` phase (0.5, 0.7); the
    `integrate` answer `merged {commit}` or `pending {url, head}`, and the `integrating` run state (0.4);
  - the intent and outcome events around a side-effect step, and the `actor` the daemon derives from the connection's
    peer credentials, with the socket directory, uid check and CLI token file of 0.2 (R5);
  - the JSON-RPC between UI or CLI and the owner, including the Queue item's context record: six sections, each
    filled or "Not available" (0.11; S6). Its method list is written in this stage, since no earlier section lists
    it;
  - the ask methods `ask.start`, `ask.cancel` and `ask.test`, and their `ask.*` events, which carry the normalized
    stream events and the ask's outcome (0.3, 0.11);
  - the shell's own `ui_tools.*` methods (`reply`, `token`, `rotate`) and its `ui_tools.request {requestId, caller,
    deadline, message}` event. The shell handles the methods and never forwards them. It answers `initialize`,
    `ping` and `tools/list` from a build-time manifest, returns 202 for notifications and 405 for GET and DELETE,
    forwards only `tools/call`, counts the per-ask cap and cancels asks when the window closes or reloads (0.2; R5).
    The manifest's file format is a contract here. The five tools' definitions are TypeScript in the window, not
    Rust contracts, because the window answers every call, and Stage 6 and the later stages that add surfaces write
    them and the generator that makes the manifest from them;
  - the `agents` and `ask` settings keys (0.7), including the repo's `ask.budget_usd` and `ask.timeout_s` defaults,
    and `layout.sort` in a view (0.11);
  - the point and event catalogues (0.4);
  - state, events and spans, with a parent id and a kind on every span, and a wait span that names its Queue item
    (0.13; S6);
  - the normalized events and the capability table of 0.3, as types with `Yes`, `No` and `Unknown` (S7).
- `core` outlines for Milestones A and B: the traits (`AgentAdapter` with `AgentSession`, `HookRunner`,
  `TaskProvider`), domain types, state enums, the workflow engine and implementation resolution, as signatures with
  doc comments and tagged stubs. S7's trait changes P1–P9 are in: two traits, `SessionStart` and `SessionBound`,
  permission answers `Allow`, `Deny` and `Cancel`, `Usage.quality`, the capability table, `TurnEnd`, `model.id`.
  The ACP engine and profile belong to `host` and are not outlined here.
- `host` outlines: the Claude adapter, `collide-daemon` with its launchd start, `collide shim`, the store, the hook
  executor, the provider supervisor, the sampler and the CLI commands, with the clocks of 0.2's clock table and the
  OTLP receiver declared in 0.2. The `ui_tools` request handler is real here, as a pure function with no listener
  (the listener is Stage 6b).
- `cel` 0.15, patched and vendored with S5's step counter, and the test that a runaway expression stops (0.9; S5).
  Views and `when:` start using it in Stage 4b.
- `@collide/pack-sdk`: `defineHook`, `defineProvider`, `defineObserver`, the `ctx` object and the generated types,
  with stubbed bodies.
- The shell's `rpc` command and event channel. The React component tree for Field, Tracks, Queue, run detail (Output,
  Time breakdown, Review), Compare and Experiments, following S6's surfaces, with typed props and placeholder
  rendering, and one reducer for UI state (every user action, and every in-app agent tool call, a reducer action).
- Test doubles as outlines: a fake adapter that emits normalized events, a second `AgentSession` double that is fed
  scripted ACP messages in `core` (sans-IO; the ACP engine and its fake child process are Stage 3d), a fake provider,
  a fake hook runner.
- CI: build, lint, typecheck, the stub count per tag, the dependency-direction check and the no-names check.
- **Planning contracts** (owner, 2026-11-13), defined first (part L): the provider schema's `kinds` (including
  `spike` and its statuses, 0.10), op effects, `links`, `owners` and checklist field type; fan-out `instances:`; the
  run budget; the five new task points with `change` and `latestRun`; `points:`, `views:` and `checklists:` in
  `pack.yaml`.
  - From S11 (owner, 2026-11-13): `change.diff`, `batch` and `effects`; `change.via` and `actor.kind: agent` with
    `run` and `phase`, reserved now so run-made writes need no later schema change; `apply` `items[]`; `preview`;
    the `watch` notification shapes and `task.invalid`; the run-diff points at accept.
  - The skeleton fixes the contracts only. The workflow-engine stage builds the run-diff points, and the
    second-pack stage builds attribution for global sources.
  - From revision 14 (owner, 2026-11-14): a proposal's `features` and `featureProblems` on the task; `target` (item
    or feature) on a declared link type; and in `core`, the parse of a feature target (`pr-0003-km#F2`), the derived
    feature state and the near-duplicate rule (0.10).
- **Planning slice** (owner, 2026-11-13; part L), built for real as soon as the planning contracts, the provider
  protocol and the pack SDK's outline exist, ahead of the rest of this stage:
  - `collide/plan` loads as a built-in pack, compiled into the app bundle with its own copy of the SDK, so it needs
    no materialization or trust prompt (R5; 0.6). Its provider is TypeScript on `node` from the captured
    environment, because this stage must prove the TypeScript provider path, the extension model's main risk (the
    provider in Rust was considered and not chosen, owner, 2026-11-14);
  - the markdown provider's shared root at its default path, `.collide/planning/`, read in 0.10's layout (only
    the paths it names, and under `epics/` only; spike records come in Stage 4c): kinds `task` and `epic`, epic
    membership from the folder, `blocked-by` links, and the ops `create`, `setStatus`, `link`, `claim` and
    `unclaim`. `create` writes into the epic's `tasks/` folder and takes the next number for the kind across every
    ref Collide can see (0.10);
  - writes hold the per-root lock file and are temp, `fsync` and rename (0.10; R5). They go to the main checkout's
    shared root by default (`--checkout` names another), `--commit` makes a one-file commit until `record` exists, and
    `claim` refuses when an owner is shown (0.10);
  - provider protocol v0's `initialize`, `list`, `get` and `apply`, and a working `defineProvider` in
    `@collide/pack-sdk`;
  - the CLI's `collide tasks [--ready] [--kind <kind>] [--json]`, `collide task <id> [--json]` and
    `collide task <id> <op> …`. `<id>` may be the short form (`12` or `12-ua`) when it matches one item. New ids
    draw their suffix from a seeded source in tests and redraw on a collision, and `collide task reid <id>` gives an
    item a new id and rewrites the links that name it (0.10; R5). Until
    the headless-core stage's daemon exists, the CLI starts the provider itself; the corpus stage moves it behind
    the provider supervisor;
  - **features** (revision 14): the slice reads `proposals/` as kind `proposal`, with 0.10's statuses, `create` (as
    `draft`, with `pr-` ids) and `setStatus` among them. The transitions, the person-only rule and `by:` wait for
    Stage 4c, which has the actor. The provider parses each proposal's Features part and refuses the move to
    `proposed` while it has a problem. `link` takes `implements` to a feature. `collide task <proposal>` prints the
    features by section with their derived state and tasks, and `collide sections [--json]` lists the sections;
  - left to the corpus stage: the private root, the layout setting, views and CEL, `watch`, trust and the
    persisted cache.

**Success Criteria**:
- `cargo build`, `cargo clippy`, `tsc --noEmit` and the app build pass.
- Every stub carries a tag, every tag belongs to a stage, and CI prints the count per tag. CI also lists each
  catalogue point with the stage that makes it fire.
- Both built-in packs', the reference pack's and both fixture packs' manifests validate against the generated
  schema. A manifest with an unknown key fails.
- Serde round-trips pass for every protocol type, and the generated TypeScript types match the schema.
- The pipeline rejects what S5's cases rejected: a manifest with an unknown key at any depth, and a tagged variant
  with a key its definition lacks, including `Wait {}`. A test pins the YAML loader's options: `no` stays a string, a
  duplicate key, an anchor and an unquoted `{{ }}` are errors, and the last carries the hint "quote the template".
- A fake `AgentSession` replayed over the shim's journal reaches the same state as the live one, including a pending
  permission request and the events' timestamps (from the stored `wall_ns`). A journal whose last record is cut short
  replays up to the last whole record. A framed daemon-to-shim message cut in half is never taken as whole.
- A test fails if `core` gains a Tauri, SQLite or process dependency.
- A seeded `orchard` string in `core`, and a seeded `CLAUDECODE` or `ANTHROPIC_` string outside the Claude adapter,
  each fail CI.
- The `agents` and `ask` settings keys are in the generated schema. An agent entry with an unknown key, or with a tool
  kind other than `read` or `search`, fails validation.
- A provider with one kind and no links validates unchanged. `collide schema` lists every new key and point. A
  derived point that names an unknown core point, or a gate bound to an `after_*` point, is refused at load.
- The new contract rules (R5) each have a test over the schema or a fixture: a `wait` hook answering `pending` three
  times leaves one run-state entry; an `integrate` of `pending` leaves the run `integrating` and holds the source's
  ops back; a `ui_tools.request` round trip with the shell's manifest answers (`initialize`, `ping`, `tools/list`),
  202, 405 and the cap; an actor read from a fake peer is `app`, `cli` or `run agent` by its process tree, and a
  person-only op from a `run agent` is refused; a path that is not a safe `0700` socket directory is refused.
- Planning slice (part L):
  - `collide tasks --json` lists every hand-written item **under `epics/`** in Collide's `.collide/planning/`, with
    no `task.invalid`. It does not read `spikes/` in this stage. The slice criterion is narrowed, not the spike
    records made read-only items, because a spike record's statuses and shape are not a task's (0.10; R5);
  - `collide tasks --ready` equals the ready list checked by hand;
  - `collide task <id> setStatus active` changes only that item's `status` line (`git diff`);
  - a task file moved into another epic's folder is listed under that epic, and its links are unchanged;
  - `collide tasks` and `collide task <id>` print each item's epic beside its id, and each link's target with its
    epic (0.10, ids);
  - with two tasks numbered 12, `collide task 12` lists both and asks for the suffix;
  - concurrent `setStatus` calls on one file all report what is on disk (the lock test), a lock left by a dead
    writer is taken over, and a kill between the temp write and the rename leaves the old file whole;
  - a claim on an item that shows an owner is refused;
  - features (revision 14): over proposal fixtures, `collide task <id> --json` lists each feature with its section,
    and an Unsorted entry's section is empty. `setStatus proposed` on a proposal whose part has problems is refused,
    with every problem and its line, while the same part in a `draft` lists with its `featureProblems`. `link
    implements` to a feature the proposal lacks is refused, naming the features it has. Each feature's derived state
    is right for every proposal status, and for no tasks, some done and all done. `reid` of a proposal keeps the
    feature in every link it rewrites. `collide sections --json` gives the fixtures' names and counts, and groups
    `Pack` with `packs`;
  - from here on, Collide's own build tasks are read and updated through the CLI.

**Tests**: Schema generation; manifest validation (all five packs, plus invalid fixtures); serde round-trips;
dependency-direction test; the no-names check with its positive controls.
The YAML hazards (`no`, `0.10`, `1e10`, a duplicate key, an alias bomb, 100,000-deep nesting). The step counter's
patch, vendored, stops a runaway expression. Session replay.
Planning: derived-point validation; the new keys' schemas; the `change` and `watch` shapes. The slice: provider
contract tests over fixture items; the ready rule (blocked, unblocked by `done` and by `dropped`, a cycle); ids
(the next number across refs, including a fetched remote branch; a repeated number after a merge; short-form
lookup; two files with one id is a `task.invalid`; the seeded suffix draw; a forced collision redrawn; `collide task
reid` rewriting the links that name the item); epic membership from the folder; an item with an unknown kind or a
broken link is counted, not dropped; a frontmatter block over its cap is invalid while a long body is not.
Contracts: the journal and its replay; the `pending` poll; the `integrating` state; the intent and outcome events;
the manifest; the actor from peer credentials.
Features (revision 14): the part's parser over good and malformed fixtures; the check at `proposed`; a feature link
refused and taken; the derived state; the near-duplicate rule.
**Status**: Complete (2026-11-15). Every item of epic `ep-002-va` is done, and the epic with it: `ts-0007` to
`ts-0044` from revision 13, and `ts-0047-fb`, `ts-0048-pg` and `ts-0049-kn` (features) from revision 14. Each success
criterion and Tests entry was checked against a named test. The check added two things: a CI step that lists the
catalogue points with their stages, and JSON round trips for the config and lock files.

## Stage 3: Headless core (CLI, no UI), and the thin loop

**Goal**: `collide run` starts one Claude phase on a task given as a file, in a new git worktree, and records it. Then
the thin loop (3c) puts that run behind a prompt box in the app. Four sub-stages, each with its own goal, criteria and
Status (owner, 2026-11-14; R5 S1). Each keeps the epic `ep-003-da`, with its items grouped by letter.

### Stage 3a: Process owner, with python children

**Goal**: The daemon and the shim own a child process, record every byte, and survive hostile children, a dead daemon
and a full disk, with no agent involved. The children are python3 fixtures.

- `core` state machines (Run, Phase, Workspace), with the terminal states and pauses of 0.7 and 0.8, and `lost`.
- **The daemon and the shim** (0.2), speaking the skeleton's JSON-RPC:
  - the shim is synchronous: `std::process::Command` with `pre_exec(setsid)`, then `libc::wait4` on the pid. An
    agent's exit is never taken from std's or tokio's `wait`, and a tokio `Child` is never dropped (measured S3);
  - the shim writes the **journal** of 0.2: one ordered file of `{seq, wall_ns, dir, bytes}`, each record written
    before the child sees the bytes. Daemon-to-shim messages are length-framed (R5; S7);
  - the daemon reaps shims with tokio, because it reads totals from `exit.json`;
  - on-demand start under the lock file, as a one-shot launchd job on macOS, with the `setsid` fallback and its warning
    over SSH; the base environment captured once and again at app start (0.2);
  - the durability rules of 0.2.
- **Socket security** (0.2; R5): the `0700` directory with the uid check, the CLI token file at `0600`, the shim socket
  the same, and the actor the daemon derives from the connection's peer credentials and process tree.
- **Clocks and disk** (0.2; R5): the clock table (uptime for work timeouts, continuous time for the cache countdown,
  wall time for display and limit reset); the free-space check before a workspace or a phase (`paused: disk`, cause
  `disk`); the caps on a child's stream line and on a phase's log.
- File-backed child logs; SQLite store + event log; per-client socket token; version handshake.
- Built-in workspace: `git worktree add`, returning `{ path, gitDir }`; refuses to remove a dirty or unmerged
  worktree.
- Stop per 0.5: the ppid walk, the marker scan through libproc (`p_uniqueid`), their descendants, the order, the
  hooks' `detached` records and the survivor report.
- The shim's exit totals, and disk space per workspace at phase end (0.13).

**Success Criteria**:
- Stopping leaves zero processes, including a setsid'd grandchild, a double-forked orphan and a child that ignores
  SIGHUP and SIGTERM (`ps`). The test children are python3, not platform binaries. A double-forked `/bin/sleep`
  orphan that its hook recorded is stopped by the cleanup hook; an unrecorded one appears in the survivor report
  (0.5).
- A phase that ends while the daemon is down keeps its exit code and totals, and the next phase starts with the app
  closed.
- A shim killed with SIGKILL makes its phase `lost`, even though the agent reported success. A SIGSTOPped shim is
  reported as unresponsive and does not stall other runs.
- When a shim is killed, a child that ignores EOF and hangup is stopped as its phase turns `lost` (S2).
- The daemon started by the one-shot launchd job has `USER`, `HOME` and `PATH`, and a python3 child runs under it. The
  real-`claude` check is in 3b.
- A daemon started from a terminal survives closing that terminal.
- Writing the child's stream and logs through a writer that fails with `ENOSPC` on demand crashes neither the daemon
  nor a shim, and an unreadable store is moved aside and reported. With free space under the floor, a phase does not
  start and the run shows `paused: disk` (R5).
- A session replayed from the shim's journal reaches the same state as the live one. A journal cut short replays up to
  the last whole record (R5).
- A socket directory that is not `0700`, or a connection from another uid, is refused. A connection from a python3
  child of a run is identified as `run agent`, and a person-only operation from it is refused (R5).
- A sleep gap injected through fake clocks counts as waiting, not active time, and a work timeout counts uptime only
  (0.2).

**Tests**: State-machine unit tests; store migrations; hostile-process tests adapted from herdr-core (SIGKILL and
SIGSTOP of the shim, children that ignore HUP and TERM, python3 fixtures, a recorded `/bin/sleep` orphan); the
`ENOSPC` writer; store quarantine; the journal's order and its cut-short replay; the framed message cut in half; the
socket checks; sleep accounting with fake clocks.
**Status**: Complete (2026-11-15) except four parts, each recorded in its item and carried on. Items `ts-0050-xc` to
`ts-0059-xs` of epic `ep-003-da` are done; the epic stays open for 3b to 3d. Each success criterion and Tests entry
was checked against a named test. Carried to 3b: the free-space check wired into phase start, so a live run shows
`paused: disk` and its Queue item (`ts-0055-cr`; only the decision and the state are tested now); sleep time taken
out of active time by the sampler (`ts-0051-wh`; the fake-clock gap and the uptime timeout are tested); the next
phase started by an engine after a phase ends with the daemon down (`ts-0057-ck`; shown through a test hook).
Carried to Stage 7: the cleanup-hook executor (`ts-0059-xs`; the recorded `/bin/sleep` orphan is stopped through a
hook the test injects). The real launchd check runs only with `COLLIDE_REAL_LAUNCHD=1` and was run on macOS for
`ts-0058-ad`. The foreign-uid refusal uses a fake peer, because a real one needs root.

### Stage 3b: Claude adapter and the first run

**Goal**: `collide run` starts one real Claude phase on a task file in a fresh worktree, and records it.

- The `host` Claude adapter per S1. The two traits of 0.3 (`AgentAdapter`, `AgentSession`) with S7's changes P1–P9, and
  the capability table with `Yes`, `No` and `Unknown`. The Claude adapter is the first `AgentSession`.
  `AgentSession::feed` takes the journal's `wall_ns` (0.3). `models.yaml` loading and alias resolution, including
  `model.name`, with `model.id` kept opaque.
- Permission requests answered from the CLI (`--permission-prompts host`).
- **The adapter's declarations (0.3):**
  - its environment is stripped from everything Collide starts (Claude: `CLAUDECODE` and every `CLAUDE_*`);
  - the user hooks and settings `env` a phase loads, read for the run preview's flags, and the model endpoint's
    host, from the environment or a `settings.json` `env`, recorded in the fingerprint (0.3, 0.13; revision 15);
  - a usage or rate limit becomes a `limited` event. The phase pauses (`paused: usage limit`) and resumes by
    session id at the reset time, or when the user resumes it.
- **Claude's version and settings (0.3; R5):**
  - the adapter's tested version range, read when the daemon starts and before each phase (cached by path and
    modification time). A version outside it gets a one-turn smoke check: a failure refuses the phase through the
    Queue, and a pass marks the version chip (0.3). The daemon fetches the published version for the second chip,
    and `collide run` prints both versions before it starts. The version is in the fingerprint, and an event the
    adapter does not know is counted as Other;
  - the generated settings are validated against Claude's published schema in the contract test, because a file that
    fails validation is silently ignored under `-p`. A runtime canary (the marker on the SessionStart hook's `hook_response`, else the phase fails
    with "Claude ignored Collide's settings") catches the rest.
- Always-recorded metrics (0.13):
  - spans for runs, phases, turns and waits, with active time apart from waiting time. Usage-limit pauses and
    machine sleep count as waiting;
  - tokens, including cache reads and writes, and cost: input and cache tokens once per response, output tokens
    and per-turn totals from `result.usage`, and a compaction call from `model_usage_since_last_result` (S9);
  - wall and model time per turn, with model time and cost taken as differences of the running totals (0.13);
  - the token and context metrics of 0.13 (context size, compactions, cold returns, tool-result bytes);
  - the shim's exit totals, and disk space per workspace at phase end;
  - the always-on part of the fingerprint.
- **Fake Claude:** replays recorded stream-json (tool calls, a permission request, a limit message, a compaction, a
  prompt blocked by a hook, an error result, hook denies, failures and blocks, a host deny, a call to a
  disallowed tool, a final report, an exit code) for the Claude adapter's tests.
  Engine tests use the normalized-event fake adapter instead (0.15).
- **Contract test:** recorded from the real, pinned `claude` and re-run on upgrade, so format drift fails a test
  (the fake alone cannot detect it). The live half is `host/tests/adapters/claude/real_claude.rs`
  (`COLLIDE_REAL_CLAUDE=1`, haiku, `--max-budget-usd 0.05` a case, a spend ledger that refuses a run that could pass
  the item's cap). It checks each live line against the shapes of the redacted recordings in
  `fixtures/real-recordings/` (the paths every recorded line of a kind has, and their JSON types): a lost path or a
  changed type fails. `COLLIDE_REAL_LAUNCHD=1` runs a fake-Claude phase through a SIGKILL of the launchd-started
  daemon, and with `COLLIDE_REAL_CLAUDE=1` a real `claude -p` phase under it
  (`host/tests/adapters/claude/launchd_run.rs`).
- **Dogfood** (part L): `collide run` takes an item from `.collide/planning/` as its task file. Collide's own tests
  pass inside a run (ts-0074-cu): the daemon reads the environment a process was started with, so a test keeps
  itself out of the walk. An in-process daemon gets a process tree that stops at the test, and a `collide` command
  starts detached from the test.
- `collide run` on a file task: a new worktree, one Claude phase, the record. The run uses the built-in
  `collide/builder` role with the alias `fast` (a run may name another), and the three context variables are set
  after the strip, so they win over a role's `env:`.
- `collide run <task-file> [--budget <usd>] [--alias <name>]` starts the daemon when none answers, prints the
  `claude` version in use and the published one (only the one in use when `claude.check_published` is false), and
  follows the phase. A permission request is asked on the terminal (y allows once, n denies; never an always option)
  and answered with `queue.answer`; input that ends unanswered leaves the request in the Queue. At the end it prints
  the record from `run.record`: state, report, and for each model call its wall time, model time, tokens and cost,
  "not reported" for any value the agent did not give. `collide run --resume <run>` reattaches to a live run,
  resumes a paused one with the Queue's Resume answer, and shows the record of one that is over; a `lost` phase is
  not resumed by hand in 3b. Exit codes: 0 ended, 1 could not do its work, 2 wrong command line, 3 no such task file
  or run, 5 refused, 7 failed, 8 stopped, 9 waiting for a person.

**Success Criteria**:
- A real run on the Orchard repo in a fresh worktree.
- One of Collide's own build tasks runs through `collide run` in a fresh worktree of Collide (part L).
- The owner restarts mid-run, and the run continues or resumes.
- A real `claude -p` phase runs under the daemon that the one-shot launchd job started.
- A permission request is answered from the CLI.
- Over a fixture `settings.json` (never the user's own) with an `ANTHROPIC_BASE_URL` in its `env` and a
  `SessionStart`, a `UserPromptSubmit`, a `PreToolUse` and a `PostToolUse` hook, a phase's fingerprint records the
  endpoint's host and no path, query or key, and each hook with its flag. The same variable in the captured
  environment is recorded the same way (revision 15).
- A role whose alias is unmapped is refused with a message that names the alias.
- A run's trace shows its phase and permission-wait spans, with the wait counted as waiting. Its tokens and cost
  match `result.usage` per turn plus its compaction calls, with no content block counted twice (S9).
- Over a recorded multi-turn session with a `/compact` and a blocked prompt, each turn's model time equals the
  difference of `duration_api_ms` from the last result with turns. A recorded result without the field stores
  null, and the CLI prints "not reported".
- A recorded hook-blocked prompt ends the turn as blocked, with the hook's reason, and its result text is not taken
  as the phase's report.
- A recorded `result/success` with `is_error: true` is not taken as the phase's report: a 429 pauses the phase as a
  usage limit, and another status fails it. A recorded budget-error result's cost counts toward the phase.
- A phase killed before its `result` keeps its responses' input and cache tokens and shows its cost as "not
  reported".
- Over S9's recorded hook cases, each refused call gets the right cause (hook, host, rule or missing tool) from
  `tool_result_meta`, a hook that exited 1 raises the failed-open detector, and no hook `stdout` or `stderr` reaches
  the store.
- A role with `compact_at` under 100,000 is refused with a message that names the floor.
- Started from a shell with `CLAUDECODE=1` and other `CLAUDE_*` values set, neither the phase nor its children see
  them (`ps -E`).
- A recorded limit message pauses the phase with its reset time, and the phase resumes by session id. The pause
  counts as waiting.
- A permission request with unknown input shows "input not available until approved", and no answer offers an
  "always" option.
- Over the same recorded session, the Claude `AgentSession` replayed from the journal reaches the same state as the
  live one.
- A `claude` outside the tested range gets the smoke check: a pass marks the in-use version and raises no Queue item,
  and a failure refuses the phase. The published-version fetch runs against a recorded registry response, with
  offline, timeout and `claude.check_published: false` cases (no network in CI). A generated settings file that fails
  Claude's schema fails the contract test, and a phase whose canary never appears fails with the message that names it
  (R5).
- The owner's sleep test (`sleep-check.sh`, 0.13) runs on a real run that sleeps: the table in 0.2 holds, and the sleep
  counts as waiting (R5; moved here from Stage 1's deferred step). It is recorded as owner judgement in the stage's
  task item.

**Tests**: Alias resolution; env stripping; limit pause and resume; usage counting over repeated content blocks and a
compaction; per-turn differences of running totals; missing values stored as null; error results' spend; other model
calls; settling on exit and end of output in either order; the hook-blocked prompt; refusal classification and hook
outcomes; context-option validation; span-tree tests against fake-Claude streams; fake-Claude end-to-end; the version
range and the canary; `TurnEnd` reasons; `Usage.quality`; real-claude contract test (opt-in in CI).
**Status**: Not Started. Items from revision 18 (with revisions 15 and 16), split on 2026-11-15: `ts-0060-mm` to
`ts-0071-fd` in epic `ep-003-da`, built one at a time in id order (agent events and aliases, Claude launch, fake Claude
and session, error and hook parsing, usage parsing, file-task run, permissions and limits, metrics, version checks,
`collide run`, real-`claude` steps, owner checks). Each Stage 3b criterion is named in one item, and the parts carried
from 3a are placed in `ts-0065-kx`, `ts-0066-zb` and `ts-0067-kw`. The real-`claude` steps (`ts-0070-mk`) each need the
owner's OK; the Orchard run, the restart and the sleep test are owner-run (`ts-0071-fd`).

### Stage 3c: The thin loop, in the app

**Goal**: In the app, type a prompt, run it through `collide/quick` in a worktree, review the diff and accept it,
against scratch tasks. This is the first dogfooding: Collide's own work can go through it before the plan adapter,
GitHub or the board exist (owner, 2026-11-14; R5 challenge 1).

What it pulls forward, and from where. Nothing is cut from the later stages; each of these is built here in its
smallest form and finished there:
- **From Stage 4b:** scratch tasks in Collide's store, with `create` and `setStatus` and nothing else (no corpus, no
  views). The prompt box creates one per prompt.
- **From Stage 6:** the shell's `rpc` command and event channel with the per-client token, a window with one page, and
  a one-item Queue (a permission request and a "ready to review" item). The board, views and endpoint wait for 6.
- **From Stage 7:** the built-in worktree's lifecycle only (create, and remove after accept or discard). Hooks and
  workspace implementations wait for 7.
- **From Stage 8:** a hard-coded run sequence for `collide/quick` (one agent phase, then review), in place of the
  transition table and its engine.
- **From Stage 9a:** launching from a prompt, the diff review for one run, and accept with the `merge-local`
  implementation (a local merge of the run's branch, with the worktree removed). Re-prompt in the same session, and
  discard, are included. The context view, take-over, tracks and metrics views wait.
- The built-in `collide` pack's `collide/quick` is compiled into the bundle (0.4, 0.6), so there is nothing to
  trust.
- The window shows the two `claude` version chips (0.3; revision 16), so a version outside the tested range is
  visible before the Agents page exists.

What it costs (owner, 2026-11-14): the launch path and the accept path are written twice. When task sources land,
Stage 9a launches from a task in the Queue through the engine of Stage 8, and accept runs the pack's `integrate`
implementation and its after-steps. The code in 3c that hard-codes the sequence is replaced then, not extended, and
the 3c tests are rewritten against the engine. The cost is accepted for an earlier first loop.

**Success Criteria**:
- In a browser, over the generic `rpc` command and a fake Claude, a prompt creates a scratch task and starts a
  `collide/quick` run in a fresh worktree. The one-item Queue shows "ready to review" with the run's diff. Accept
  merges the branch into the checkout and removes the worktree. Re-prompt continues the same session. Discard removes
  the worktree and leaves the checkout unchanged.
- A permission request from the run shows in the Queue, and answering it lets the run continue.
- Closing the window and reopening it shows the same run and the same review item (the daemon owns the run).
- The owner runs one real prompt on a worktree of Collide through the app, reviews the diff and accepts it. Recorded
  as owner judgement in the stage's task item, with what was awkward.
- After this stage, the owner's Collide work that fits a prompt goes through the thin loop (dogfooding).

**Tests**: The loop over `rpc` in a browser against the fake Claude; permission answer; accept, re-prompt and discard
against a fixture repo; window close and reopen; scratch-task create and status. The browser tests drive the page
against a real daemon through a test-only bridge, a binary behind the cargo feature `test-bridge` that the app never
builds; the page's own tests stay over `FakeOwner` (ts-0082-fp).
**Status**: In Progress. Items from revision 22, split on 2026-11-15: `ts-0075-hj` to `ts-0083-uf`, in epic
`ep-003-da`, built one at a time in id order (part L). It runs after 3b. The ACP engine (3d) follows it; the thin
loop needs only Claude.

### Stage 3d: ACP engine

**Goal**: A second `AgentSession`, over a scripted fake ACP child, keeps the trait honest.

- **The ACP engine** (`host`), tested with a scripted fake ACP child: the handshake, a prompt turn, a permission
  request, `session/cancel`, an empty turn (`empty_turn`) and a child that exits first (`lost`). A real ACP agent
  is Deferred for runs (below); the ask box uses one from Stage 6b. The second `AgentSession` keeps the trait honest
  meanwhile (S7).
- The model is chosen through the child's `configOptions` and `session/set_config_option`, with `session/set_model` as
  the fallback when the child offers no `configOptions` (0.3; R5).

**Success Criteria**:
- Over the fake ACP child, a turn that ends with no output is `empty_turn`, never a clean finish. A cancel sends
  `session/cancel` and the phase settles from the prompt response; a child that exits before it is `lost`. A usage
  record whose quality is `Partial` or `Unreported` is shown as such and never as zero.
- A child that offers `configOptions` has its model set through `session/set_config_option`. One that offers only
  `session/set_model` gets that call (R5).

**Tests**: The fake ACP child, with the same scenarios as the fake Claude where they apply; the `configOptions` and
`set_model` paths; `Usage.quality`; `TurnEnd` reasons.
**Status**: Complete (2026-11-16). Split on 2026-11-15 into `ts-0086-tm` to `ts-0092-tw` (revision 23 scope), in
epic `ep-003-da`, built one at a time in id order (part L). Every item runs against a scripted fake ACP child, and the
protocol is written by hand. A rebuilt session reports the writes it still owes (`take_owed_writes`) and the daemon
sends them once. A real ACP agent, its first profile and an ACP agent in file runs come with Stage 6b.

## Stage 4: Pack loading, settings, plan adapter, corpus and views

**Goal**: Tasks come from a ref-scoped source that can be read and written, are combined into a corpus, and are
filtered by views. Everything loads from a trusted snapshot, with the pack's settings. Three sub-stages, each with its
own goal, criteria and Status (owner, 2026-11-14; R5 S1). Each keeps the epic `ep-004-qd`, with its items grouped by
letter.

### Stage 4a: Packs, trust and settings

**Goal**: Packs and settings load from a trusted snapshot, with every refusal that guards the load. No provider runs
yet.

- **Minimal pack loading.**
  - The Collide directory: `.collide/`, or the directory a `.collide` pointer file names (0.7).
  - `pack.yaml` validated against the skeleton's full schema. This stage uses `requires`, `providers` and
    `config`. Stage 7 uses the rest.
  - `board.yaml` (packs, sources, status mapping, checkout filter) and `packs.lock`.
  - `collide init`, which writes `board.yaml`. The lock appears with the first `git:` pack.
  - Snapshot materialization per S4 and 0.6. The pack's git tree id is resolved before anything is read: at the
    ref for a `repo:` pack, from the lock for a `git:` pack.
    Files come from git objects and are re-hashed. Symlinks, submodules, `node_modules` and `package.json` are
    refused. The cache is keyed by tree id, built in a staging directory and renamed into place.
  - The trust prompt over the hash of 0.9 (the repo packs' tree ids, the lock, the pointer file, `board.yaml`,
    `config.yaml`, views and `protectedPaths` tree hashes), stored in `~/.config/collide/trust.yaml`.
  - The `requires.tools` check, with the pack's install hint, and the `requires.sdk` check.
  - The built-in `collide` pack loads from the app, trusted with it (0.4).
- **Pack sources and scope (0.7):**
  - sources `repo:`, `git:` (any git URL, a repo on disk included) and `dev:`, each resolved to a git tree id and
    materialized by one code path. `git:` fetches into a bare mirror under
    `~/Library/Application Support/Collide/cache/git/` with the user's git credentials, under the fetch rules of 0.7
    (R5): a new host asks first, git runs with `GIT_TERMINAL_PROMPT=0`, a protocol allowlist, `--` before the URL and a
    timeout, and a URL that starts with `-` is refused;
  - user packs in `~/.config/collide/packs.yaml` and their lock, switched on per board;
  - trust per source (0.9);
  - working copies (0.7): `collide pack link <name>` with no folder, and new `.collide/packs/` folders in the
    user's checkout offered as working copies;
  - `collide pack add|link|enable|disable|list`.
- **Settings (0.7):**
  - four layers, lowest first: the schema's defaults, then `~/.config/collide/config.yaml`, then the snapshot's
    `.collide/config.yaml`, which wins except for keys marked `overridable`, then the run's profile (next bullet);
  - profile files from the snapshot's `.collide/profiles/`, `~/.config/collide/profiles/` and packs' `profiles:`,
    applied as the top layer by `collide config get <key> --profile <name>`. A user profile that sets a key not
    marked `overridable` is refused, naming the key;
  - per-user-only keys refused in the repo file, naming the key;
  - repo-only keys (`repoOnly: true`) refused in the user's file and in a user profile, naming the key;
  - secrets from the OS keychain only (`collide config set --secret`). A secret in either file is a load error;
  - Collide's own settings through the same layers, among them `disk.floor_gib` (default 5, `overridable`);
  - **agent entries** (0.7; owner, 2026-11-14): the `agents` and `ask` keys through the same layers, with the
    built-in Claude Code and haiku entry in the first layer. The repo's file may set only `budget_usd` and
    `timeout_s` (overridable); an entry's kind, command, arguments, model, environment, secrets, folders and tool
    kinds are per user and refused in the repo file. Each agent has a timeout (5 to 1800 s, default 180). `tools`
    takes only `read` and `search`. A budget on an agent that cannot enforce one, a literal value on a secret-named
    `env` line and a `read` folder that does not exist are load errors that name the key. A secret is a keychain item
    name, resolved at launch only. `collide config get agents.<id>.<field>` shows the layer;
  - `collide config get <key>` shows the value and the layer it came from.
- **Pack data:** one directory per pack per board, passed as `COLLIDE_PACK_DATA` (0.7).
- **The repository's own instructions** (0.3; revision 21). First measure, with the fake and then once with the real
  `claude`, a launch line that loads the worktree's `CLAUDE.md` files and skills while settings, hooks and MCP config
  stay from the snapshot. Then build it, with the role key `repo_instructions` and the warning when a run changed an
  instruction file. Measure also (revision 22): that a session started at the person's checkout with the snapshot's
  flags loads none of its untracked local settings or MCP servers; and what a `claude` that the agent starts itself,
  inside a worktree it made, loads from that worktree, since the snapshot's flags reach only the session Collide
  starts. Measured in `ts-0103-sa`: the checkout session loads none of them; the agent's own `claude` loads all of
  its worktree's settings, hooks, MCP servers, agents, skills and instructions, with auto memory on. Skills through
  a snapshot plugin are measured in `ts-0106-nr` (owner, 2026-11-16) before `ts-0104-gx` builds the line.

This sub-stage checks agent entries at load only. Launching an agent, and the secrets test at launch, are in 6b.

**Success Criteria**: On fixture repos:
- An edited `pack.yaml`, hook, `board.yaml` or `config.yaml` on a branch is not used. A changed hash asks again.
- A key set in both the user's file and `.collide/config.yaml` resolves to the repo's value, and `collide config get`
  names the repo as its layer.
- A secret in either file fails the load and names the key. A source account in the repo file is refused. A
  `repoOnly` key in the user's file is refused, naming the key.
- In a fixture repo whose `.collide` file says `dir: tools/collide`, the board, views and settings load from
  `tools/collide/`.
- A pack that commits `node_modules/@collide/pack-sdk`, or a symlink, is refused at load, and the message names the
  path.
- A pack from a `git:` source on a local bare repo loads by its pinned tree id. Moving the remote's ref changes
  nothing until `collide pack update`. A lock whose commit the remote no longer has fails, naming the pack and the
  commit.
- A user pack named like a repo pack is refused, naming both. `collide pack link` overrides a repo pack for that user
  only, and `collide pack list` marks it dev. A `dev:` folder inside a workspace is refused.
- A repo pack change committed at the line's ref, with no lock edit, loads after a trust prompt that shows the diff.
- `collide pack link <name>` with no folder runs the uncommitted pack folder from the user's checkout, badged dev. A
  new `.collide/packs/` folder there is listed as a working copy.
- An `overridable` key resolves to the user's value. A user profile that sets any other key is refused.
- A `git:` URL that starts with `-` is refused at load. The first fetch from a new host asks, naming the host, and a
  fetch that takes longer than the timeout fails with the pack's name (R5).
- Agent entries (owner, 2026-11-14):
  - with no config at all, the built-in entry resolves to Claude Code, `haiku`, `config: none`, a 180 s timeout and no
    budget, and `ask.agent` names it;
  - a repo file that sets `command`, `args`, `env`, `secrets`, `read`, `tools`, `model`, `config` or `ask.agent` fails
    the load and names the key. The same file's `budget_usd` and `timeout_s` load, and the repo's `budget_usd` wins
    over the user's while the user's `timeout_s` wins over the repo's;
  - `collide config get agents.claude-haiku.timeout_s` names the layer, and so does every other field;
  - `tools: [write]` fails. A `budget_usd` on an ACP entry fails with "only a timeout applies". An `env` line
    `OPENAI_API_KEY: sk-test` fails with the hint to use `secrets`;
  - a `secrets` entry names a keychain item and holds no value in any file. A missing item is an error that names the
    item. The launch test (the value reaches the agent and appears in no file, argv or log) is in 6b;
  - the owner runs `collide config get` on an entry they wrote and reads the layers back, recorded as owner
    judgement in the stage's task item.

**Tests**:
- Snapshot-not-workspace test; materialization refusals and the re-hash; id and template-field validation,
  including a value that fails the safe-token rule.
- Settings: layering, per-user-only keys, the secret rule against a fake keychain, the pointer file (a `dir:`
  outside the repo is refused), `overridable` keys, profile layering.
- Agent entries: layering and the per-field source; each refusal above; the secret-name test with names that should
  and should not match; the timeout range.
- Pack sources: resolution for each kind; a `dev:` folder's tree id equals `git write-tree` over the same files;
  the name clash; trust per source; the fetch rules, against a fixture remote that prompts, a hanging remote and a
  URL that starts with `-`.
**Status**: Not Started. Split on 2026-11-16 into `ts-0093-am` to `ts-0105-jv` (revision 23 scope), in epic
`ep-004-qd`, built one at a time in id order (part L). It runs after 3d. The keychain uses `security-framework`
(owner, 2026-11-16). `ts-0103-sa`'s one real `claude` run is approved ($0.50 in all); the real keychain and the
trust prompt over a real repository are the owner's (`ts-0105-jv`). `ts-0106-nr` (skills through a snapshot plugin,
one real session at $0.10; owner, 2026-11-16) is built before `ts-0104-gx`. `ts-0104-gx` builds the line with the
plugin off; `ts-0107-rj` (one gated session, $0.0034) measured that a skill's frontmatter takes effect through it,
and the owner accepted that (2026-11-17), so the plugin is on. Both are built before `ts-0105-jv`.

### Stage 4b: Providers, plan adapter, corpus and views

**Goal**: Tasks from the plan adapter and the scratch source are read and written through the provider protocol,
combined into a corpus, and filtered by views.

- **Provider protocol v0** per 0.10, with the pack's settings passed to `initialize`. A provider supervisor restarts
  providers with backoff, isolated from the process owner. A call that is not answered within its method's timeout
  (`initialize` 30 s, `list` 60 s, `get` 10 s, `apply` and `preview` 120 s, `watch` 10 s) fails and its child is killed.
  Restarts wait 1 s, doubling to 60 s, and stop after 5 crashes in a row; an `apply` in flight when its child ended is
  not asked again (`ts-0108-dq`).
- `@collide/pack-sdk` v0, compiled: `defineProvider`, `defineHook`, `ctx.config`, `ctx.dataDir`.
- **`plan` adapter (TypeScript):**
  - parses frontmatter per checkout with `yaml` in core mode, behind a size cap (0.10), and keys tasks by (shortId,
    path), and declares the `labelId` template field;
  - declares its statuses with their categories, and the status each category write produces;
  - exposes collections (epics with slices and streams);
  - reports renames from the id map and reports collisions;
  - `watch` debounced, plus a 10 s recheck;
  - operations `setStatus` (by status or by category) and `create`:
    - run through a line root's `./plan --root <checkout>/arch`;
    - return the files they touched;
    - queue while a phase runs in that checkout;
    - never commit.
- **Scratch source** (0.10): tasks in Collide's store, per user per board, with `create` and `setStatus`.
  `collide task new <prompt>` creates one.
- **Root busy (0.4):** a write to the board root waits while a rebase, merge, cherry-pick or bisect is in progress
  there, and says why. The daemon stores the write, answers `queued` and applies it when the root is free, and a Queue
  item says why; the CLI waits in place (owner, 2026-11-17; `ts-0114-hr`).
- **Corpus** with status categories, identity and collisions.
  - A task whose status its source did not declare is listed with the source's invalid items, with the status named
    (0.10, revision 23).
  - The corpus cache persists in the store. On start, core reconciles each source against it and emits one
    `task.changed` per item with `cause: startup` (0.10; S11).
  - Links are resolved one hop out, both directions and across sources, into `tasks[].links` (0.5; S11).
  - An unparseable item (conflict markers, a sync conflict file) emits `task.invalid` and counts on its view (S11).
  - The plan adapter declares `preview`, using the plan CLI's `--dry-run` (S11).
- **Views** from the snapshot's `.collide/views/*.yaml` and from personal views: the set algebra, normalized fields,
  closure per the prototype board's generator, and per-view error counts. CEL runs as 0.9 says: the vendored
  `cel` 0.15 with S5's step-counter patch, a per-task and a per-pass step budget, a 1 KB source cap, list caps at
  load, `catch_unwind` around compile, no corpus variable, and a static cost estimate when a view is saved and when
  it loads.
- **CLI:** `collide task new <prompt>`, `collide tasks [--view <name>] [--checkout <path>]`, `collide task <id>
  [--json]` (one item: fields, status, links one hop out, version and latest run; part K), `collide task <id> <op> …`.
  The new forms go through the daemon; `tasks`, `task <id>` and `task create` stay direct until 4c (owner, 2026-11-17;
  `ts-0120-wm`). `collide init`, `collide config get|set` and `collide pack …` were built in 4a.

**Success Criteria**: On the Orchard repo:
- A task shows `doing` in one worktree and `ready` at the board root, with the divergence reported.
- The MVP view's `closure` for T-01-042 has the same task set and the same steps as the prototype board's
  generator's for T-01-042 at the same commit.
- `"plan:E-07" in collections` returns the same tasks as the prototype board's E-07 epic.
- `setStatus` with an illegal transition comes back `refused`, with plan's message.
- A legal `setStatus` reports exactly the task file and its regenerated epic table as touched. `setStatus` with
  category `active` writes `doing`.
- A write into a checkout with a running phase is queued, not applied.
- A write to the board root during a rebase waits, and the CLI says why. It applies once the rebase ends.
- An edited provider script or view on a branch is not used. A changed hash asks again.
- A scratch task created with `collide task new` is listed for that board only.

**Tests**:
- Protocol contract tests with a fake provider.
- The plan adapter against a fixture `arch/` tree in two checkouts, including:
  - a renumbered task (rename);
  - a same-id, different-path pair (collision).
- `watch` emits `task.changed` after `plan set` in a worktree.
- CEL view tests:
  - set algebra;
  - a missing source field;
  - a closure over a dependency cycle;
  - the step budgets: S5's nested-comprehension runaway stops at the per-task budget, and a pass of many cheap
    tasks stops at the per-pass budget;
  - a source over 1 KB and a list over its cap are refused at load, with the reason;
  - a view that panics in compile is an error on that view, and the others still load (`catch_unwind`);
  - the static estimate on a view that cannot finish within the budget;
  - a view that names a corpus variable is refused.
- Root busy, against a fixture repo stopped mid-rebase.
**Status**: Not Started. Split into `ts-0108-dq` to `ts-0121-xp` (revision 23 scope, 2026-11-17), in epic `ep-004-qd`,
built one at a time in id order (part L). The build does not wait for 4a's owner checks. Nothing in it runs a real
`claude`. The "On the Orchard repo" criteria are the owner's, in `ts-0121-xp`.

### Stage 4c: Planning

**Goal**: The rest of the markdown provider and the planning pack, so the spike records and Orchard's plan items are
items too (owner, 2026-11-13). This adds the `spike` kind, so the slice reads `spikes/` from here (0.10; R5).

- **Planning** (owner, 2026-11-13): the rest of the markdown provider, after the skeleton's slice: the private
  root and the layout setting; kinds, op effects and links in the corpus; `collide/plan`'s record parser (claim
  blocks) and `roles` mapping; pack views, with the slice's `--ready` as a built-in view. The slice's CLI moves
  behind the provider supervisor. The Orchard pack's plan provider reads ADRs, spikes and questions.
- **Feature ops** (revision 14): `addFeature {section, title, summary}` on a proposal, whose `section` is required
  and may be empty (owner, 2026-11-14), takes the next `F<n>` and writes under that section's heading, or under
  `### Unsorted`, made at the end of the part when it is missing; `moveFeature {feature, section}`; and
  `renameSection {from, to}` over every proposal that uses `from`, which merges when `to` is in use and reports every
  file it changed in `apply`'s `items[]`. Each leaves the part without problems. The `proposal` kind gets its
  transitions, the person-only rule and `by:` (0.10), with the actor.
- **Quality parts** (owner, 2026-11-15; revision 18): the provider reads the Decision log, the Assumptions and
  Findings parts and a spike's Pre-registration part (0.10), reports their problems in `partProblems`, and derives
  a proposal's `coverage`. The Decision log comes first, then the Assumption ledger.

**Success Criteria**:
- Planning:
  - Collide's spike records in `.collide/planning/spikes/` list in `collide tasks --view spikes --json` with their
    statuses (all `concluded` today; a later record shows `results` until the owner decides), and no item from inside
    a spike folder other than its record;
  - over the Orchard repo, the Decisions needed view lists the same question ids as the human queue in
    `plan status --format json`;
  - an ADR's `evidence` link shows from the spike's side as "evidence for";
  - a malformed claim block is a counted parse error on the view;
  - two branches that each create ten items in the shared root merge with no id collision;
  - a private-root item never appears in `git status`;
  - `collide task reid` on a colliding item rewrites the links that name it, and a repeated suffix draw is redrawn
    (0.10; R5);
  - a layout path outside the repo or under `.git/` is refused with the reason;
  - features (revision 14): `addFeature` with an empty section writes under `### Unsorted` with the next number;
    `renameSection` of `Pack` to `packs` over three proposals changes the three files, returns them in `items[]`,
    and leaves each part without problems;
  - quality parts (revision 18): a fixture proposal with a decision that names no alternative, an evidence id that
    names no item, an assumption with no `State:` and an open high finding lists them in `decisions`,
    `assumptions` and `findings`, with the bad id in `partProblems`, and still moves to `proposed`. With three
    features and two included tasks, one implementing a feature and one implementing none, `coverage` names the two
    features with no task and the second task. A Decision section of plain text reads unchanged, with no problem.

**Tests**:
- Planning: parser fixtures, with strict plan CLI frontmatter left untouched, and S5's hazards (an unquoted
  `no`, a hash like `1e10`, a duplicate key, an alias bomb, a file over the cap); provider contract tests against
  markdown and plan fixtures; id generation; view counts; layout checks. Quality parts (revision 18): parser
  fixtures for each part and the Pre-registration part, and coverage cases.
**Status**: Not Started. Items from revision 13, not yet split into items. Epic `ep-004-qd`, items lettered `4c`
(part L).

## Stage 5: GitHub adapter

**Goal**: A global source with live updates and writes over Issues (REST), built against the same protocol before the
UI depends on it. Projects v2 comes after the board, in Stage 6c (owner, 2026-11-14; R5 challenge 5). Stage 2 still
freezes the provider contract in full, including the Projects keys.

- `github` adapter (TypeScript, built-in pack). `board.yaml` names the repos. The `gh` account is per user, in the
  user's settings (0.7), and is read with `gh auth token --user <name>`.
  - Issues over REST: `since` + `sort=updated`, ETags. Pull requests are shown read-only and are not launchable.
  - Issues from repos without a board are shown but not launchable, and the reason is given.
  - `initialize` checks the token's scopes for what Issues needs and names the fix.
  - A periodic full reconcile emits `task.removed` for deleted or transferred issues, which `since` never returns.
    It also runs at start when the last one is older than its period (0.2; revision 22). An expired token at start
    makes the source "could not check": nothing is removed, and one Queue item says to sign in again.
- **Operations:** `setStatus` (close or reopen with a reason; categories map through `board.yaml`'s status mapping)
  and `comment`. Versions are per item (0.10). A stale version is detected before the write and returns `conflict`.
- **Provenance:** `author_association` per task, for 0.9's trust rules.
- Any protocol change this adapter forces goes back into 0.10 and the plan adapter.

**Success Criteria**:
- An issue edit appears within 60 s (measured).
- `setStatus` with category `done` closes an issue.
- A write with a stale version is detected before the write and returns `conflict`.
- A deleted issue leaves the corpus within one reconcile interval.
- A view over both plan and GitHub tasks lists both.

**Tests**: Recorded GitHub fixtures (no network in CI); scope check; per-item version conflict tests; reconcile
removal; PR items; category mapping; mixed-source view test.
**Status**: Not Started. Items from revision 13, not yet split into items. Epic `ep-005-hs` in
`.collide/planning/epics/` (part L).


## Stage 6: Task board (read-only desktop)

**Goal**: The desktop app opens a board and shows the corpus through views, live, before any workflow runs from it
(owner, 2026-11-12). Three sub-stages, each with its own goal, criteria and Status (owner, 2026-11-14; R5 S1). Each
keeps the epic `ep-006-wq`, with its items grouped by letter.

### Stage 6a: The board

**Goal**: The Field, filter bar, drawer, settings and no-setup open, over the corpus, live. Stage 3c built a minimal
shell and window; this finishes them.

- The Tauri shell: one `rpc` command and one event channel, forwarding to the owner's socket and adding the token
  (0.2). Stage 3c built the minimal form. The webview's CSP and Tauri capabilities are set here (0.9; R5): the
  capabilities are `rpc` and one event channel, and nothing else. The UI's client speaks the owner's JSON-RPC
  (handshake, reconnect), so it also runs in a browser against a fake owner.
- **Field:** views; board and run-order layouts; live updates from `watch`; divergence and collision chips.
  Board columns come from the declared statuses, by status or by category (0.10, revision 23). Stage 2's fixed
  category columns go.
- **Filter bar:** chips that write CEL; saving a view writes a view file.
- **Task drawer, read-only:** body, dependencies both ways, collections, per-checkout state.
- **Line selector** (0.11): every open board with its default line. Switching swaps views and workflows in place
  and stops nothing.
- **Settings:** each key's effective value and the layer it came from (0.7). Secrets are set into the keychain from
  here. The Agents page is part of it (6b).
- **No-setup open** (0.11): any git repo opens with the built-in packs, a GitHub source when `origin` is on GitHub
  and `gh` is logged in, and the default aliases. Board settings stay per user until Share with team writes
  `.collide/`. First-run checks: `claude` installed and logged in, a git repo (else offer `git init`), `gh` optional.
  The trust prompt appears only for repo packs.
- **Empty board:** a prompt box that creates a scratch task (built in 3c; launching from the board arrives in 9a), New
  session (9b), the detected sources, and Add a pack.
- **Planning** (owner, 2026-11-13): a read-only planning view; links both ways in the drawer; checklists shown
  read-only; the branch-only chip; and the view's Features (0.11; revision 14): features by section across
  proposals, with their derived state and tasks, and near-duplicate sections flagged.
- **Markdown in the webview** (0.9; R5): task, issue and agent text is rendered with raw HTML off, no `javascript:` or
  `file:` links, and external links opened by the OS after a confirmation.

**Success Criteria**:
- **A missing board** (0.2; revision 22): a board whose repository folder was moved shows as missing with
  "locate", deletes nothing, and after "locate" shows its tasks and Up next list as they were.
- S6 scenario 2, and scenario 1 up to the board, pass as UI tests ported from S6's pilots against the real app, each
  completing and detecting its end state. The owner's walkthrough of both is recorded as owner judgement in the
  stage's task item, which lists what to record: the owner's time per scenario beside S6's advisory limit, the
  errors, and what was awkward (R5). S6's thresholds are advisory (Stage 1).
- A fresh git repo with no `.collide/`, on a machine profile with no Collide config, opens to a board with no setup
  step. Share with team writes `.collide/`, and a second clone opens with it after the trust prompt.
- A folder that is not a git repo offers `git init`. A missing or logged-out `claude` is named, with the fix.
- The MVP view in the app lists the same tasks as `collide tasks --view edge-mvp`.
- A `plan set` in a worktree shows as a divergence chip within the watch debounce plus the 10 s recheck.
- A key set in both settings files shows the repo's value, labelled as coming from the repo.
- The UI's tests run in a browser against a fake owner, with no Tauri.
- **Statuses as data** (revision 23). A fixture source with eight statuses, two of them mapped to `review`, shows
  eight columns in its order. Renaming its statuses changes the columns with no code change. A view mixing it with
  the plan source shows the six categories, each card with its own status name.
- A proposal shared on a branch and checked out as a worktree shows its items with the branch-only chip. The
  planning view's counts match the CLI's, and its sections and their feature counts match `collide sections --json`.
- A recorded GitHub issue whose body holds raw HTML, a `javascript:` link, a `file:` link and an image that fetches a
  remote URL is shown in the drawer as text: nothing runs, nothing is fetched, and the CSP blocks the image. An
  external link asks before it opens (R5).

**Tests**: Reducers over provider notifications; view save round-trip; the no-setup open and first-run checks; the
settings view; the hostile-issue fixture in a browser, against the CSP; one Playwright smoke run of scenarios 1 and 2.
Columns from a fixture source's declared statuses, and from categories for a mixed view.
Planning: planning-view counts against the CLI; the branch-only chip.
**Status**: Not Started. Items from revision 13, not yet split into items. Epic `ep-006-wq`, items lettered `6a`
(part L).

### Stage 6b: The in-app agent

**Goal**: The ask box, the endpoint and the Agents page work with Claude Code and one ACP agent, with every guard of
0.2, 0.3, 0.7 and 0.9 tested. It changes nothing in a repo and starts no run.

- **The in-app agent on the Field** (owner, 2026-11-14; 0.2, 0.3, 0.7, 0.9, 0.11; S6's in-app agent section). It
  changes nothing in a repo and starts no run, so it belongs to a read-only board:
  - **The endpoint** in the shell, with its guards, the endpoint token in the keychain, ask tokens and the
    `ui_tools.*` methods (0.2). The HTTP server is a crate already in Tauri's dependency tree if one fits, and the
    stage says which it picked. The window answers each JSON-RPC message with the MCP layer and the tools in
    TypeScript, ported from S6's `exec.ts` and `mcp.ts`.
  - **The Field's tools:** `query` over tasks, `set_view` and `highlight` and `show_groups` on the Field, and `open`
    for the Field, Queue and Experiments. Each call is a typed reducer action with a snapshot Undo. Views, lenses,
    chips, **Save as View** (the chips' CEL, `layout.groupBy` and `layout.sort`) and **Save snapshot** are as 0.11
    says. A call for Tracks, or for runs, is refused with the supported values listed, until Stage 9.
  - **The ask box** in the top bar, with one line per call, the chip, Cancel, the failure texts and the "no files were
    read" flag (0.11). `ask.start`, `ask.cancel` and `ask.test` go to the daemon. The Claude adapter's `ask` launch
    spec and the ACP engine's ask profile for opencode are built here, on Stage 3's traits, with no new trait (0.3).
  - **The Agents page** in Settings (0.7): the entries with each field's source, the words for folders and tool kinds
    and for budget and timeout, **Test this agent**, the endpoint card with Rotate, per-agent last use and spend this
    week, the snippets for external clients, the choice of the ask agent, and Claude's two version chips (0.3).
  - **The token cost of the strict line is measured here.** The two example asks run under `config: none` and under
    `config: user`, and the input tokens of each go in the record. S6 measured 28K to 107K under a line that was not
    strict (0.11).
  - **Measured here, for the ask line (0.3; R5):**
    - whether `--add-dir` makes a second folder readable under `-p`;
    - whether `--restricted` confines reads under `-p`, and the adapter adopts it if it does. `--bare` is not adopted;
    - whether a read outside the folders is refused without a prompt under `--permission-prompts none`;
    - `lsof` by the child's pid, for an ACP agent's listener, and whether a permission configuration that Collide
      supplies to opencode changes its default of unrestricted.
    Until each is measured, the page says the agent "starts in this folder".
  - **The launch tests that moved here from 4a:** the secrets test. A `secrets` entry resolves from a fake keychain at
    launch, and the value is searched for, as a seeded string, in every file, argv and log, and is found in none.
  - **A closed window** (0.2): the shell fails `tools/call` with the "open Collide" message, including when the client
    started while the window was already closed.

**Success Criteria**:
- **The in-app agent** (owner, 2026-11-14). The first six need no model and no browser:
  - S6's 47 agent tests are ported and pass against the real reducer and tool code: each tool's valid call, each
    refusal (an unknown id applies nothing, an id in two groups, a group with no reason, 13 groups, a bad sort key, an
    unknown surface or subject, a call for a surface not built yet), Undo restoring exactly the replaced state, a lens
    that does not change when a task does, a lens saved only as a snapshot, a view saved as a View through the chips'
    CEL, and the 21st call of one ask refused. The cap is counted in the shell (0.2);
  - the endpoint, tested with a fake window: a wrong token, a foreign `Host`, a foreign `Origin`, a content type that
    is not JSON and a body over 1 MB are each refused and nothing reaches the window; a closed window, a window closed
    when the client starts, and a window that stays silent for 10 s each give the plain text of 0.2; `initialize`,
    `ping` and `tools/list` are answered from the manifest with no window, a notification gets 202, and a GET or
    DELETE gets 405; an ask is cancelled when the window closes or reloads; after **Rotate** the old token is refused
    at once; an ask token stops working when its ask ends; the endpoint token appears in no log, no argv and no `ask`
    record;
  - the launch specs, with no process: the Claude `ask` argv equals the adapter's pinned line in 0.3 exactly, holds no
    token and no request text, passes `--add-dir` once per extra folder and `--permission-prompts none`, and has no
    `--disable-slash-commands`; `config: user` changes only `--setting-sources`; `--max-budget-usd` appears only when
    the entry sets a budget; `mcp.json` is mode 0600 and gone after the ask. The fake ACP child shows a read inside
    the readable folders allowed, a read outside them, a read with no path, an edit and a command each refused, a
    symlink inside a readable folder that points out of it refused after `realpath`, `Cancel` when no `allow_once`
    exists, an agent with no HTTP MCP support refused with the plain text, a timeout that stops the child and says so,
    and Cancel that ends the ask;
  - the flag: over fake streams, a `show_groups` ask with no read shows "No files were read in this ask"; with a
    Read, a Grep or a Glob (Claude) or a `read` or `search` tool call (the fake ACP child), it does not; for an
    external client it says the reads are not visible;
  - the Agents page: each field shows its layer, the budget and timeout words are on the page, **Test this agent**
    against a fake agent reports the endpoint reached, the time to the first tool call and the cost or "not
    reported", and each failure of the test names its cause. A snippet's URL and token are parsed out and used
    against the endpoint, and each works; after **Rotate** each shows the new token;
  - **Real-agent contract test** (opt-in with `COLLIDE_REAL_AGENT=1`, never in the default suite, because it is
    nondeterministic and costs money; in the style of Stage 3's real-claude contract test). It asks the Field request
    of 0.11 three times through the real ask path with Claude Code haiku, against S6's fixture repo (24 files) and its
    12-task epic, copied into the fixtures. A run passes when the groups hold the intended sets (quantities 001 and
    002; expiry 004, 005 and 006; reports 008, 009 and 010; data file 011 and 012) with no intended set split or
    mixed, tasks 003 and 007 in either area or left out, and no "no files were read" flag. The test passes when two
    of three runs pass, and the pass count, time, cost and input tokens of each run go in the stage's task item. S6's
    runs of this request were one wrong on prompt v1 and two right on v2 (0.11), so this is a re-check on a model or
    prompt change, not a gate. Three more opt-in checks: the Claude snippet of the Agents page connects a real
    `claude -p` that calls `query` once; `opencode mcp list` shows the endpoint connected with its snippet, skipped
    when opencode is not installed; and the strict line's token comparison above;
  - **Owner walkthrough, recorded as owner judgement** in the stage's task item, alone: ask the Field request on
    S6's fixture board and read the chip against the code for two groups; Undo; Save snapshot, restart the app and
    open it; ask "what should I work on next?" and read the lens; on the Agents page add an ACP agent, **Test** it,
    read each field's source, put a key in the keychain and see that it is in no file, set a budget on the Claude
    entry and see the words for the ACP one, then rotate the token and connect an external client with a fresh
    snippet; close the window during an ask and read the error.
  - **`query` at scale** (R5): over a fixture of about 1,500 tasks, `query` returns no body by default, honours
    `fields`, pages by cursor with each page under 20K tokens, and returns a body by id, truncated. No page nears
    Claude's 25K output cap.
  - **The probe and the listener** (R5): over a fake agent that writes a file and runs `true`, Test this agent is red,
    the agent is refused as an ask agent until acknowledged, and `query` withholds untrusted bodies for it. An ACP
    profile that is given `--mdns`, a non-loopback `--hostname` or `--cors` is refused.
  - **Measurements recorded** in the stage's task item: `--add-dir`, `--restricted`, the outside read, opencode's
    listener and its permission configuration. If `--restricted` confines reads, the pinned line adopts it and its
    criterion above changes with it.
  - **The secrets launch test:** a seeded secret reaches the agent's environment and appears in no file, argv or log.
  - The owner's walkthrough also lists what to record: time, errors and what was awkward (R5).

**Tests**: In-app agent: the ported reducer and tool tests; the endpoint against a fake window, in Rust, with the
guards and both tokens; the manifest answers, the 202 and 405 cases and the cap; the launch specs and the ACP profile
over the fake ACP child, including the realpath case; the flag over fake streams; the Agents page and its test action
over a fake agent, with the probe; the secrets launch test; the real-agent contract test (opt-in), with a file
planted outside every readable folder.
**Status**: Not Started. Items from revision 13, not yet split into items. Epic `ep-006-wq`, items lettered `6b`
(part L).

### Stage 6c: GitHub Projects v2

**Goal**: Projects v2 items and fields join the GitHub source, after the board exists to show them (owner,
2026-11-14; R5 challenge 5). Stage 5 built Issues over REST, and Stage 2 froze the Projects keys in the contract.

- Projects v2 items and fields over GraphQL. `board.yaml` names the projects.
- Draft items are keyed `github:<owner>/projects/<n>/draft/<id>` and are untrusted (0.9).
- `initialize` checks that the token has the `project` scope, and the error names `gh auth refresh -s project`.
- `setStatus` through the Projects Status field, with categories mapped through `board.yaml`'s status mapping. A stale
  version on a Projects item is detected before the write and returns `conflict` (0.10).
- Measure what one Projects v2 poll costs on a real project, and pick the interval from it.
- Any protocol change this forces goes back into 0.10 and the plan adapter.

**Success Criteria**:
- A Projects Status change made in Collide shows on github.com.
- `setStatus` with category `done` on a Projects item sets the mapped Projects status.
- A write with a stale version on a Projects item is detected before the write and returns `conflict`.
- A token without the `project` scope fails with the message that names the command.
- A draft item shows as untrusted. The cost of one poll and the chosen interval are in the stage's task item.
- A Status option missing from `board.yaml`'s mapping shows under its own name, counts as `backlog`, is never written
  by a category op, and the board's checks name it with the fix (0.10, revision 23).

**Tests**: Recorded GraphQL fixtures (no network in CI); the scope check; version conflict on an item; draft items;
category mapping; an unmapped Status option.
**Status**: Not Started. Items from revision 13, not yet split into items. Epic `ep-006-wq`, items lettered `6c`
(it extends the adapter of epic `ep-005-hs`; part L).

## Milestone B: Workflows and packs (Orchard drives it)

Ends usable in the desktop app: one Orchard task through the full loop, watched and steered in Field, Tracks and
Queue.

## Stage 7: Hooks, workspaces and record

**Goal**: The Orchard pack provisions, resumes and removes workspaces through `dev worktree`, and commits through
`record`.

- The rest of `pack.yaml` in use: roles, prompts, gates, `launch`, `workspaces`, `integrations`, `record` and
  `protectedPaths`. `metrics` and `experiment` are validated from Stage 2 and used from Stage 13.
- Namespaced contribute. Named workspace implementations, resolved per workflow, with the built-in one as the
  fallback (0.4). Chain order and failure rules.
- Hook executor per 0.5, with the pack's settings as `config` on stdin and `COLLIDE_PACK_DATA`. Each hook runs under
  a shim (0.2; R5), so the stop and survivor rules of 0.5 cover it. The line lock, with the root-busy wait. Only the
  default line exists yet.
- State per 0.5:
  - run state per invocation (`pack/hook` plus phase, round and fan-out instance);
  - workspace state merged key by key, which `doctor` checks and rewrites;
  - point state for chains;
  - output schemas and the 64 KB cap.
- Lifecycle steps 1, 2, 6 and 7 (0.4): points `run.before_start`, `workspace.after_create` and `run.after_end`, and
  `workspace.before_remove` on removal.
- Report files per 0.5, for hooks and gates:
  - a required `t` on every line;
  - generated `<pid>-<counter>` span ids and `COLLIDE_REPORT_PARENT`;
  - `unclosed` spans; the size and rate caps;
  - `collide report` and the SDK's `ctx.report`;
  - only a hook's own report lines, written through a descriptor its children do not inherit, may carry the primary
    metric (0.5, 0.14; R5).

  Reported spans are stored now. The UI shows them from Stage 9.
- Workspace kinds and lifecycle states:
  - workspace names include `run.slug`;
  - `resume`;
  - `discard` for sandboxes;
  - reaper.
- Env per 0.5: the adapter's variables stripped first, then `COLLIDE_*`, then role and phase templates (experiment
  templates come with Stage 15). Templates use validated values only: template fields, `phase.dir`, and
  `model.name` checked against the pack's `requires.modelName`.
- `record` commits exactly the touched files, and refuses when the line root is off its ref.

**Success Criteria**:
- Orchard `create` → a `failed` map step → `resume` → `ready`.
- `cleanup` runs after a timed-out hook and takes its lane down, including the lane's host server processes, which
  carry no `COLLIDE_RUN` (`ps`, `docker ps`).
- `remove` is refused until the task is done at base.
- A `setStatus` at the board root followed by Record produces one commit containing exactly the task file and its
  regenerated epic table. An unrelated edit under `arch/` stays uncommitted.
- Two concurrent `record` calls on one root run one after the other.
- A sandbox created for a phase is gone after `discard` (`git worktree list`).
- A fixture pack's `create` outputs reach its `cleanup` hook after a timed-out run, with no rediscovery. A `resume`
  output updates one key and leaves the others.
- A gate that runs twice in one run keeps two run-state entries. A reader gets the latest by default, and both on
  request.
- A hook killed mid-run leaves no state entry. An output that fails its declared schema makes the hook `error`.
- A shell script two processes below a hook reports a span with `collide report span`, and the span nests under
  the hook's. A `collide report` run inside that span nests under it. A line without `t`, and a 5 KB line, are
  rejected and counted.
- `run.after_end` handlers run after a run that failed while provisioning.
- A fixture workflow that names no workspace implementation runs in a built-in `git worktree add` workspace.
- A hook running when the daemon is killed with SIGKILL is stopped, and no hook process survives (`ps`; R5).
- A `metric` line written by a hook's child through an inherited descriptor is not taken as the primary metric. The
  hook's own line is (R5).

**Tests**:
- Executor:
  - a timeout kills the tree
  - `status` in the result file wins over the exit code
  - a missing result file is an error
  - exit 2 from a child is not a veto
  - `config` arrives on stdin and never in the environment (`ps -E`)
- Chain: a refusal stops later handlers; a later handler sees an earlier one's outputs at the same point.
- State: workspace state survives an owner restart; per-invocation keys; key-by-key merge.
- Report: nesting by `parent` and by `COLLIDE_REPORT_PARENT`; two writers sharing a file get distinct ids; a
  repeated open id, a missing `t` and malformed lines are counted; `unclosed` marking; `collide report` outside
  Collide does nothing and says so; a `metric` that claims tokens saved is shown labelled reported and leaves the
  run's measured tokens and cost unchanged (revision 15).
- Line lock serialization; the root-busy wait.
- Env:
  - role before phase, so the phase's value wins;
  - a template using task text is rejected at load;
  - the safe-token rule rejects `..`, a leading `-` and a leading `.`;
  - a value that fails its provider's pattern is rejected, and so is a `model.name` that fails the pack's
    `requires.modelName`.
- YAML hazards.
- Injection: a positive-control hook that shells out on task text must trip the test; the real executor must not.
- Hook under a shim: the daemon killed mid-hook; the inherited-descriptor report line.
**Status**: Not Started. Items from revision 13, not yet split into items.

## Stage 8: Workflow engine (driven by fakes)

**Goal**: The transition table of 0.7 runs against fakes: every phase kind, decisions, rounds, gates, workflow ops
and sandboxes, with no real agent.

- Phase kinds `agent`, `fanout`, `script`, `wait` and `human`. Declared outputs; `record: true`; `phase.dir`;
  `start_head` / `end_head`; `history[]`.
- Transitions per 0.7:
  - `then:` to a phase, an end, `ask_human`, or a decision script. An undeclared token makes the decision `error`;
  - `round` counted on re-entry;
  - terminal states `ended(<outcome>)`, `failed` and `stopped`.
- **Adopting** a run at a named phase and round, in an existing workspace (0.7).
- **Intent and outcome** (0.5; R5): the engine writes an intent event before each side-effect step (a provider `apply`,
  a push, an `integrate`) and an outcome event after it. After a restart it checks the world and never retries a step
  blindly. A step whose outcome cannot be told becomes a Queue item, "outcome unknown". A merge that finds its change
  already at the target is a success, not a conflict.
- **`wait` phases** poll with `interval:` and end at `timeout:`. A hook that answers `pending`, with `retryAfter`,
  replaces its run-state entry and is polled again (0.5, 0.7; R5).
- **`integrating`** (0.4; R5): an `integrate` that answers `pending {url, head}` leaves the run `integrating`, with the
  source's ops and after-steps held back. They run when the same `integrate` later answers `merged {commit}`.
- The built-in `collide` pack's workflows (0.4) run in the engine suite next to the fixture packs.
- `accepts`: the run preview checks each task and names the field or operation it lacks. A workflow that uses
  `task.*` is refused for several tasks.
- Lifecycle steps 3–5 (0.4): `on_start` and `on_end` ops by status category through the provider's `apply`, each
  firing `task.before_status`, then `record`. `on_end`'s `when:` reads `outcome`.
- Sandbox workspaces per phase instance, for the models listed.
- Prompts: minijinja templates, or an earlier phase's artifact.
- Points `phase.before_start`, `phase.after_end` and `task.before_status`. Run state readable in `when:`, with
  fan-out entries per instance.
- `when:` in CEL.
- `launch` parameters. A fan-out whose aliases resolve to one model is refused.
- One live run per task key.
- Gates on phases and on named points.
- `readOnly` check: HEAD unchanged, and tracked plus untracked-but-not-ignored files unchanged outside `phase.dir`.
  Ignored files are out of scope, because a reviewer's own `iter` run rebuilds `dist/`. That limit is stated.
- Agent phases get no `COLLIDE_REPORT` (0.5).
- **Where an agent starts** (0.4; revision 22): `start: root | workspace | <folder>`, its default from
  `agentWorktrees`, the refusals, `rootPhases`, a write and its `record` as one step at a root, the start record and
  the file-edit flags, and the preview's start and warning.
- **Worktrees an agent makes for itself** (0.4; revision 22): the reads, the baseline in the store, the
  provisioning hold, attribution with `identify` (named by `adopt`), kind `adopted` and its states, attaching to
  later runs of its task, `cleanup` of its lanes, `release` on request.
- **`landing: integrate | observed`**, the landed rule, and the Queue item for work that never lands (0.4; revision
  22).
- The adapter's config paths (0.3): a run whose range edits them is flagged, and its integrate waits for an
  acknowledgement (Stage 9).
- `ask_human` carries its cause: the decision token that led to it, a budget, or an agent's stop-and-report.
- The `unverified` result (0.5): the run held in the Queue, then retried, passed on or stopped.
- `task.invalid` becomes a Queue item (0.10; S11).
- `change` built in full (0.5; S11): `diff`, predicted at `before_*` (from `preview` when the provider has it) and
  actual at `after_*`; `effects` and `effectOf` from `apply`'s `items[]`; a call on several items fires per item
  with `change.batch`, and a refusal skips only that item.
- `show:` on `human` phases and on decisions (0.7).
- `agentWork: true` on `script` phases, with `usage` and `limited` report lines accepted from them only (0.3, 0.5).
- `protectedPaths` with `at: phase-end` fails the phase, checked like `readOnly` (0.9).
- Profiles (0.7): a run resolves its profile (`--profile` on the CLI), which sets the packs switched on, the default
  workflow, launch values, model aliases and context options.
- Budgets: `--max-budget-usd` per role. A pack's own budgets (Orchard's review budget) are `launch` parameters
  carried into prompts, not counted by Collide.
- **Planning** (owner, 2026-11-13): fan-out instances; the run's total budget; linked items in the prompt context,
  fenced; task points fired from workflow ops; derived points; the checklist gate on `task.before_status`.
- **Run-diff points** (owner, 2026-11-13; S11): the engine logic that fires a run's own task writes at accept. Core
  diffs the workspace's items against the line and fires `task.before_status` for each category moved, and
  `task.before_op` for `create` and `delete`, with `via: run` and `op: null`. They fire after the source's own
  `before_*` points and before `run.before_integrate`, and a refusal stops `integrate`. The `after_*` points fire
  after `integrate`. Built here against a fake ref-scoped provider; the real accept path is the run UI stage.

**Success Criteria**: Both fixture packs pass the engine suite, using the normalized-event fake adapter and fake
gates:
- the review loop: build → brief → fanout review → adjudicate → fix → … → close, with the `on_start` and `on_end`
  ops recorded. The second review runs as round 2, after `fix1`;
- issue → PR → CI: a `script` phase pushes and opens a fake PR, a `wait` phase polls a fake CI until it passes, and a
  `human` phase takes a token. A CI that never finishes times out, and the wait counts as waiting;
- a fan-out with one instance failing;
- a `readOnly` violation;
- a decision script that returns an undeclared token is `error`, and the message names the token;
- a sandbox created and discarded for the strong reviewer only;
- a second launch of a task with a live run is refused;
- a task the workflow does not accept is refused in the preview, naming the field it lacks;
- a run adopted at `review`, round 3, in an existing workspace continues from there;
- `collide/quick` and `collide/review` (max rounds 2) run to their ends, and `collide/review` stops to `ask_human`
  when a round leaves as many open findings as the round before;
- an agent phase's environment has no `COLLIDE_REPORT`;
- a `wait` hook that answers `pending` three times leaves one run-state entry, is polled at its interval and ends at
  its `timeout:` (R5);
- a fake `integrate` that answers `pending {url, head}` leaves the run `integrating` with the source's ops held back,
  and the ops run once it answers `merged {commit}` (R5);
- a fake `integrate` killed with SIGKILL after its intent event and before its outcome (R5): on restart, a fake target
  that shows the merge is recorded as a success with no second merge; one that shows nothing runs the step again;
  one that cannot be checked puts "outcome unknown" in the Queue. The same holds for a killed `apply` and a killed
  push;
- a daemon killed with SIGKILL just after it sent an ACP write that the shim had not yet journaled: the new daemon's
  replay does not send it a second time (carried from ts-0092; the shim serves each connection on its own thread, so
  the old connection may still be draining);
- a gate that returns `unverified` holds the run, no fix phase starts, and a retry after the fake environment is
  restored passes;
- a fake `script` phase with `agentWork: true` reports `usage`, shown apart from the measured totals, and `limited`,
  which pauses another fake run on the account;
- under a profile that switches a fixture gate pack off and sets `compact_at`, the gate does not fire and the
  phase's launch carries the option;
- after a fake builder edits `.claude/settings.json`, the next automatic phase runs without waiting, and the run is
  flagged for an acknowledgement before integrate.
- Worktrees an agent makes (revision 22):
  - a fake agent that runs `git worktree add` in its phase gets the worktree attached to its run as `adopted`, with
    its branch and diff; the reaper leaves it once the task is done at base; a worktree made before the run is not
    attached;
  - a fixture `create` hook's worktree is never attached to another run while the hook runs;
  - with two runs live, a fixture `identify` hook that names a task attaches the worktree to that task's run; one
    that names a task of neither run is ignored and counted; one that times out falls through to the next rule;
  - after the daemon is killed and restarted mid-run, the baseline holds, and a worktree made while it was down is
    attributed by `identify` or listed for a person;
  - a second run of the same task attaches the adopted worktree, starts its builder there without `create`, and
    runs a `readOnly` reviewer in it;
  - a new worktree on a volume under the floor pauses the run with `paused: disk`; one whose folder is deleted
    becomes `prunable`;
  - a run that ends, or is stopped, with commits or uncommitted changes in an adopted worktree that are not on the
    line's ref raises the Queue item, and `release` runs only when a person chooses it.
- Where an agent starts and how work lands (revision 22):
  - a phase of a workflow with `agentWorktrees: true` starts at the line root by default, one of any other workflow
    starts in its workspace, and a `<folder>` start runs in that folder; the preview names each and warns for the
    person's checkout;
  - a task write at a root with a live phase is committed by `record` in the same step; a second root-started phase
    on the line waits; a fake agent's file-edit call to a root file is flagged;
  - `start: root` is refused for a `readOnly` role, an experiment arm and an untrusted author's task, also when it
    comes from a per-board override;
  - with `landing: observed`, a merge, a rebase and a squash of a branch each land its task; a merged range that
    touches `protectedPaths` raises a Queue item.
- Coming back (0.2; revision 22):
  - with the daemon stopped, a waiting run's task changed, another task removed and a workspace folder deleted,
    the next start marks the run's item as changed since it was raised, closes the removed task's item as gone,
    makes the workspace `prunable`, and notifies once;
  - with the daemon alive throughout and a fake wake after a long sleep, the same catch-up runs;
  - until a board's catch-up ends, nothing starts, no queued write applies, a surviving run's next phase waits and
    the reaper takes nothing;
  - a source that fails with an auth error removes and closes nothing, and raises one Queue item;
  - a wall clock set backwards reads as unknown and still checks;
  - a worktree first seen after the gap, with no `identify` answer, is listed for a person, not attached.
- Planning:
  - three instances with different prompts run in separate workspaces, and the next phase sees all three
    artifacts. Two identical instances are refused;
  - a run over its total goes to `ask_human` with cause budget;
  - a status write to `done` with a required checklist item still open is refused, and names the item;
  - a workflow op `resolve` fires `task.before_op: resolve`, then `task.before_status: done`, then the `after`
    points, in that order;
  - a hook bound to a derived point runs only when both filters match;
  - run-diff points, with a fake ref-scoped provider: a fake builder's `in_review` in its workspace fires
    `task.before_status: review` at accept, before `run.before_integrate`. A refusing gate stops the fake
    `integrate`, and the `after_*` points fire only after it succeeds. An item the diff adds or removes fires
    `before_op: create` or `delete` with `op: null`.

**Tests**: Engine table tests; fan-out join; artifact-missing failure; `record` after outputs; per-task decisions
with `tasks[]` of length 1 and 2; round counting; terminal states and `outcome`; `wait` timeout; adopting;
`ask_human` cause recorded; `when:` over run state; an `after_*` refusal fails its phase; the config-path flag; the
`pending` poll; `integrating`; intent and outcome with a kill at each step.
Planning: engine tables for instances; the budget stop; point order; derived-point expansion; gate binding
with `when:`; the run-diff points against the fake provider.
**Status**: Not Started. Items from revision 13, not yet split into items.

## Stage 9: Desktop UI: runs

**Goal**: Launch, Tracks and Queue (0.11) over the real core and engine, as validated in S6. Stage 3c built the loop
from a prompt to an accepted run in a thin, hard-coded form. Here it runs through the engine, and the rest of the run
surfaces are built. Four sub-stages, each with its own goal, criteria and Status (owner, 2026-11-14; R5 S1). They keep
the Stage 9 epic, with items grouped by letter.

### Stage 9a: Launch, Queue and accept

**Goal**: A task, or a prompt, launches a run through the engine. Its decisions arrive in the Queue, and a finished run
is reviewed and accepted. This replaces 3c's hard-coded sequence and accept path.

- **Field:** Launch with a run preview (implementations, accepted tasks, the reason for each refused one; the
  profile, its reason and each setting's layer; dev and `agentWork` badges; the flagged user hooks and a redirected
  model endpoint, 0.3; the two `claude` version chips, 0.3). Profiles picked in the preview or
  taken from the view's `profile:`. **Use working copy** on a repo pack whose folder in the user's checkout
  differs from the ref (0.7). The
  drawer's operation forms, create, and Record.
- **Launch from a prompt:** the empty board's prompt box creates a scratch task and launches the board's default
  workflow on it (`collide/quick` with no pack), through the engine. Stage 3c's hard-coded sequence is removed here.
- **Cold resume:** a phase with `on_cold: fresh` that resumes after its cache expired starts a new session from the
  handoff note (0.3).
- **Review and accept a single run** (moved here from the integration stage; owner, 2026-11-13): diff, then accept,
  re-prompt or discard. Accept follows 0.4's order: the `sync` check with `git merge-tree --write-tree`, the
  `before_*` points of the source's operations and of the run's task diff (S11), `run.before_integrate`, the
  workflow's integrate implementation (`collide/merge-local` here),
  `run.after_integrate`, the source's operations, then `record`. An integrate that carries an edit to the agent
  tool's config waits for an acknowledgement (0.3). So does the next run that would load a working-copy pack the
  integrate changed (0.7).
- **Reattach after the app restarts.** The app finds the daemon's runs and shows them as they were.
- **Queue:**
  - permission requests, `ask_human` with its cause, `human` phases, each with its `show:` list; `unverified`
    results; failures;
  - usage limits with their reset time;
  - the cache countdown and resume cost of a live phase, and prompts blocked by a user hook (0.3);
  - acknowledgements for edits to the agent tool's config;
  - write conflicts, queued writes, a busy line root, unrecorded edits;
  - keyboard triage; notifications through the menu bar (below);
  - the context view (`i`; 0.11, part L), built from the transcript, the corpus, the rules and the store, with no
    model call.
- **Dogfood** (part L): from this stage, Collide's own tasks run through `collide/quick`, and its spikes through
  `collide/plan`'s spike workflow.

**Success Criteria**:
- These pass as UI tests ported from S6's pilots, in the real app with the fake adapter, and the owner's walkthrough of
  each is recorded as owner judgement in the stage's task item. The item lists what to record: the owner's time per
  scenario beside S6's advisory limit, the errors, and what was awkward (R5):
  - S6 scenario 4;
  - single-task versions of scenarios 3 and 5: launch one task, then review its finished run.
- The app quits and relaunches mid-run without loss.
- **No pack, real Claude:** S6 scenario 1 in full. A fresh repo with no pack and no tasks → a prompt → a finished
  `collide/quick` run, reviewed and accepted. The base ref has the change, and the scratch task is `done`.
- Stage 3c's hard-coded launch and accept paths are gone, and the 3c flows pass against the engine.
- An accept that would conflict goes to the Queue, and no checkout changes (`git status`).
- **Run-diff points on the real accept path** (owner, 2026-11-13; S11): a plan-fixture task file set to `in_review` in
  a run's worktree fires `task.before_status: review` at accept. A refusing gate stops `integrate`, the base ref is
  unchanged (`git log`), and the Queue says why. A `task.before_status: done` gate refuses before anything merges.
- One of Collide's own build tasks runs from the Field through `collide/quick` to accept, and one of its spikes runs
  through `collide/plan`'s spike workflow (part L).
- A Queue item's context view opens with `i` and makes no model request.
- A usage limit from the fake adapter appears in the Queue with its reset time, and the run resumes after it
  without a click. The Queue shows the resume's cache-write cost.
- A fake-adapter phase with `on_cold: fresh`, resumed after the cache TTL, starts a new session whose first prompt is
  the handoff note.
- Group launch and group accept are graded in Stage 11.

**Tests**: Reducer tests for the Queue against fake-adapter streams; keyboard triage tests; the Queue context view for
each kind, built with the fake adapter counting zero model requests, the agent's words escaped and placed after
Collide's facts, a path outside the worktree flagged, and the history counts; the handoff-note builder; single-run
accept order, including the run-diff and `done` gates before `integrate`; the merge-tree conflict check; the
prompt-to-accept path with real Claude (opt-in in CI, like the contract test); one Playwright smoke run of the
scenarios above.
**Status**: Not Started. Items from revision 13, not yet split into items (Stage 3c's code is the starting point).

### Stage 9b: Tracks, output, take-over and the menu bar

**Goal**: Live runs are watched and steered: phase tracks, the agent's output, the tool-call time breakdown, take-over,
New session and the menu bar.

- **Tracks:** live runs as phase tracks with meters, including provisioning. A phase opens to its activity digest,
  report and diff, with the transcript and terminal behind a toggle. Provisioning progress from `progress` report
  lines. Dependency and build folders are collapsed with counts; lock files are always shown (0.11).
- **Agent output and tool-call time breakdown** (0.11; part K): the output pane on selecting a track, a phase or a
  task with a run, live and paged; the breakdown table and timeline per track or phase, with the tool-type,
  minimum-duration and time-window filters and their CEL; a bar opens its call in the output pane. The window
  matches overlapping calls, and totals count only the time inside it (part L).
- **Take over:** the phase's session opens in a terminal through the owner's PTY (S2), with xterm's DOM renderer.
  Closing the terminal detaches and reopening replays the shim's buffer. `claude`'s exit or **End take-over** ends
  the phase and runs its gates. Live cost from telemetry, with a Queue warning at the phase budget, and the session
  id followed across `/clear` (0.11). The telemetry arrives at the daemon's OTLP receiver (0.2; R5): `127.0.0.1` only,
  a `Host` check, a per-run token and a body cap. The user's own OTEL endpoint is detected before the `OTEL_*`
  variables are stripped, and the preview names it (0.13).
- **New session** (0.11): interactive Claude in a fresh worktree through the owner's PTY (S2), as a one-phase run on a
  new scratch task. The worktree is under the repo's worktree root, and Claude's trust prompt is left to the user
  (0.4, 0.9). New session, take-over and Plan… launch with the snapshot's `--mcp-config` and `--strict-mcp-config`, so
  they hold no endpoint token unless the person adds one (0.7; R5).
- **Menu bar** (0.11): the icon, badge, countdown, notifications and actions. Closing the window destroys the webview
  and keeps the app running. The app restarts a daemon that stopped answering, with 0.2's back-off.
- **The Lines control** on Tracks and the Queue (0.11; revision 22): the selected line, all lines or a hand-picked
  set; the Queue's badge and the menu bar count every line.

**Success Criteria**:
- **Lines control:** with two boards open and Tracks set to one line, a Queue item on the other board still counts
  in the badge and in the Queue's hidden-items link, and `o` on it shows it without changing the selector.
- **Coming back:** after a start that found three tasks changed and one removed (0.2), the menu bar shows one "While
  Collide was closed" notification, and its lens on the Field shows the three and names the removed one.
- **New session:** a terminal opens in a fresh worktree, a prompt typed there is answered, closing the view
  detaches, and exiting `claude` puts the diff into review. The first session in a repo shows Claude's trust prompt
  and Collide sends it no keys; the second shows none.
- **Take over, with real Claude:** a running headless phase is taken over, a prompt typed in the terminal is
  answered, closing the terminal detaches, and **End take-over** ends the phase and runs its gates. Quitting the app
  mid-take-over and relaunching returns to the same session with its output replayed, as S2 measured. After a
  `/clear`, a headless resume uses the new session id.
- A take-over past its phase budget raises a Queue warning, from telemetry, and keeps running.
- **Telemetry and interactive launches** (R5): the OTLP receiver refuses a request with a foreign `Host`, a missing or
  wrong run token, or a body over the cap, and it is bound to `127.0.0.1` (`lsof`). A New session's `claude` argv holds
  `--strict-mcp-config` and no endpoint token. A user's own `OTEL_EXPORTER_OTLP_ENDPOINT` is seen before the strip and
  named in the preview.
- **Menu bar:** with the window closed, the app and its WebKit processes stay under 80 MiB, measured as S2's addendum
  does. A new Queue item raises a notification that holds no task text while the setting is off (checked with a
  seeded title). With the window closed, a killed daemon is restarted by the app, and the run's next phase starts.
- On live and finished runs, selecting a track opens the output pane, live and paged. The breakdown's tool-type,
  minimum-duration and window filters select the known calls of a fixture, and a bar opens its call in the pane. The
  owner's walkthrough records time, errors and what was awkward (R5).

**Tests**: Reducer tests for Tracks against fake-adapter streams; the output pane on hostile output (HTML, ANSI,
links, a 10 MB result); the breakdown's filters (a 2 s minimum and a clock window, counted against a fixture) and
wall-clock against summed time with parallel calls; a call that starts before the window counted, with only its
in-window time in the total; the notification text builder; the daemon restart back-off; take-over and New session with
real Claude (opt-in in CI, like the contract test).
**Status**: Not Started. Items from revision 13, not yet split into items.

### Stage 9c: Metrics, import, compare and export

**Goal**: The always-recorded data is shown, compared and exported, with no pack needed.

- **Metrics from the always-recorded data, with no pack needed** (owner, 2026-11-13):
  - cost labelled API-equivalent; tokens and the cache hit rate; active time apart from waiting; time per tool and
    per command;
  - CPU time and peak memory per phase (the shim's exit totals), and disk per workspace;
  - context per phase: start weight, peak, compactions and cold returns, with the seven detectors and what each
    would save (0.13);
  - the opt-in import of the repo's past Claude sessions, shown as a baseline (0.13);
  - the run waterfall, with the phase-graph critical path, active time apart from waiting time, and reported spans;
  - "where time goes";
  - comparing two finished runs;
  - annotations;
  - JSONL/CSV export.

**Success Criteria**:
- S6 scenario 6 passes as a UI test ported from S6's pilots, in the real app with the fake adapter, and the owner's
  walkthrough is recorded as owner judgement in the stage's task item.
- The critical path matches hand-built fixtures with known answers, including a fan-out and a human wait.
- **Metrics with no pack, real Claude:** the finished run of scenario 1 shows tokens, cache hit rate, CPU time, peak
  memory, disk and its context findings. An imported history shows the same for past sessions, and the store holds
  no prompt, reply or tool text (checked by searching it for seeded strings).

**Tests**: Critical-path fixtures; export round-trip; detector fixtures; the transcript import with repeated usage
rows.
**Status**: Not Started. Items from revision 13, not yet split into items.

### Stage 9d: Planning in the app, and the agent on Tracks

**Goal**: The planning workflows and the in-app agent's Tracks tools work in the app, and the command palette ties the
surfaces together.

- **The in-app agent on Tracks** (owner, 2026-11-14; 0.11): the Stage 6 tools learn the surfaces this stage builds.
  - `query` over runs returns each run's id, task, status, workflow, start, total and wait seconds, spend, and the
    seconds in each phase. Seconds in a phase are the phase's wall time, the width of its segment, taken from the
    spans (0.13). A phase with no span has no value.
  - `set_view` on Tracks takes the run filters, the sorts (total, start, end, spend, wait and `phase:<name>`, where
    the name is a phase a workflow on the board declares, and a missing value sorts last) and a clock window. An
    unknown phase name is refused, and the error lists the known ones. `show_groups` draws bands on Tracks with the
    sort value on each row. `highlight` marks runs, and `open` opens a run, a Queue item or an experiment.
  - A Tracks view saves as a personal view in the board's settings, with its filter and sort, and not its window
    (0.11).
- Command palette. Owner logs viewable.
- **Planning** (owner, 2026-11-13):
  - `collide/plan/spike` in a private proposal: pre-register, with its kill criterion (revision 18) and a `human`
    phase that approves it; run; verify; decide, with a `human` phase whose `show:` is the record and the result
    set against the kill criterion; write back through the source's ops;
  - Plan… (0.11): a New session on a new private proposal, from the Planning view, an epic or a task, with the
    planning workflow's prompt; proposal statuses and the person-only ops (0.10);
  - task points fired from UI writes; the Queue lane from a view; checklist marking in the drawer, with agent
    suggestions shown with their evidence;
  - features (0.11; revision 14): Sort, with `query` and `show_groups` learning the features surface; its chips,
    applied through the feature ops (Stage 4c) with Undo; merging near-duplicate sections; and the naming rule in
    Plan…'s prompt;
  - the quality checks (owner, 2026-11-15; revision 18; 0.11): the Plan… panel with its Readiness checklist,
    coverage map, Decision log and Assumption ledger; the red-team pass and its pre-mortem mode, with
    `collide-plan-red-team` in the planning pack; and the budget meter.

**Success Criteria**:
- **The in-app agent on Tracks** (owner, 2026-11-14):
  - the Stage 6 reducer and tool tests gain Tracks cases, with no model and no browser: over a fixture of nine
    finished runs with distinct build seconds, `set_view` sorting by `phase:build` descending gives the known order,
    and a run with no build span sorts last; a run filter and a clock window select the known runs; a lens draws
    bands with the sort value on each row; an unknown phase name is refused with the known names listed; a Tracks
    view saved and reloaded has its filter and sort and no window;
  - **real-agent contract test** (opt-in with `COLLIDE_REAL_AGENT=1`, never in the default suite, as in Stage 6):
    the Tracks request of 0.11 ("the tracks that have taken the longest in their build portion") runs three times
    with Claude Code haiku against that fixture. A run passes when the chip's expression is a sort by `phase:build`
    descending and the rows show the known order. The test passes at two of three, and the counts, times and costs go
    in the stage's task item (S6 measured four of four right, 6.0 to 8.3 s, $0.0005 to $0.0023);
  - **owner walkthrough, recorded as owner judgement** in the stage's task item, alone: on live and finished runs, ask
    the Tracks request and check the order against the Time breakdown of the top run; ask for the runs that waited
    longest and read the chip; Undo; on a run with no build span see it last.
- Planning:
  - a one-agent spike in a private proposal ends with the spike concluded and the proposal's Decision section
    marked `human:<date>`. The decide step writes the record's Decision section, and nothing appears in `git status`;
  - the proposal survives an app and daemon restart and reopens where it stopped;
  - Plan… on an epic creates a `draft` proposal in that epic's private folder and opens a session on it. From the
    terminal, real Claude creates a task and a question through the CLI, both linked from the proposal, and moves
    the proposal to `proposed`. The question appears in "Decisions needed", and answering it there marks it
    `resolved_by: human:<date>`. Accepting the proposal in the drawer writes `by: human:<date>`. Nothing appears in
    `git status` until Share;
  - from that run, the CLI refuses `accepted` on the proposal and `resolve` on the question. The drawer refuses to
    accept a proposal while a question it includes is open;
  - an agent phase that edits a checklist state file fails at phase end;
  - a suggested checklist item does not satisfy the gate until a person marks it;
  - a gate that times out on a drawer write leaves the write unapplied, with the hook's name in the message;
  - an `after_status` hook that fails after a drawer write leaves the write in place and adds a Queue item.
- Features (revision 14): over a fixture of twelve features in four sections with one near-duplicate pair, the
  reducer and tool tests (no model) show a Sort answer's groups and chips. Applying a move chip writes through
  `moveFeature`, and Undo restores the file byte for byte. Merging the pair leaves one section in
  `collide sections`. Plan…'s prompt names `collide sections` and the naming rule.
- Quality checks (revision 18), with the fake adapter:
  - a red-team pass on a fixture draft adds entries under `### Red-team`, the red-team agent's spans hold no write
    call, and no other part of the proposal changes. Accepting a finding leaves an `Accepted:` line, and
    dismissing one leaves `Dismissed:` with the reason. The pre-mortem mode writes under `### Pre-mortem`;
  - the panel's Readiness lines match the fixture, and the agent's move to `proposed` succeeds while advice lines
    fail;
  - on scripted telemetry the meter warns at 85%, the Queue warns at the budget, and the session keeps running.
    The breakdown puts the red-team pass's spend under its own part;
  - approving a spike's pre-registration writes `preregistered:`, and an edit to the part after that shows as an
    amendment. The decide step shows the result beside the kill criterion.

**Tests**: The in-app agent's Tracks tools over the nine-run fixture and the real-agent contract test (opt-in).
Planning: the spike workflow end to end with the fake adapter; Plan… with the fake adapter scripting the CLI
calls; the person-only ops refused from a run; the Queue lane reducer; `protectedPaths` on checklist state and
on a planning file's `status` and `by:` lines; point timeouts and failures outside a run. Features: Sort's reducer
cases, chips and Undo over the twelve-feature fixture; the merge. Quality checks: the red-team pass and its
pre-mortem mode with the fake adapter; the Readiness lines; the meter's warning.
**Status**: Not Started. Items from revision 13, not yet split into items.

## Stage 10: Orchard loop end-to-end

**Goal**: A real Orchard task goes `ready → doing → in_review` through the pack, with Collide as the lead.

- The pack is committed in the Orchard repo under `.collide/packs/orchard/`. Collide's fixture copy is refreshed
  from it.
- Roles from snapshot definitions, passed as `collide-orchard-<role>`. Settings and MCP config also come from the
  snapshot (`--setting-sources user`, `--settings`, `--mcp-config`, `--strict-mcp-config`), so a reviewer in the
  builder's worktree does not load the builder's edits to `.claude/` (0.3).
- `on_start` sets category `active` (`doing`) and records. The build brief carries the launched `review_budget` and
  tells the builder to pass `--run` to `iter timings --write`.
- A brief phase before each review round, per its adversarial brief template.
- Round-1 fan-out review: two models, identical briefs, a mutation sandbox for the strong reviewer.
- The orchestrator phase adjudicates against its stop rules §1. It writes the round record from its template and the
  fix brief, using its tally script as the orchard briefs do. `record` commits them on the base branch.
- The decision script `workflow/next.ts` parses `**Next**` and returns `fix`, `close` or `stop`. `iter-evidence`
  gate.
- Handoff by the builder in the worktree, as today. `on_end` sets category `review` (`in_review`) when the outcome is
  `closed`, and records.
- A `phase.before_start` / `phase.after_end` hook appends the prototype board's registry lines
  (`<git common dir>/orchard-dev/agents.jsonl`), so that board lights Collide's runs too. It moves onto an observer
  in Stage 12 (0.4).
- A chain already in flight, started by hand before Collide, is adopted at its next review round (0.7).

**Success Criteria**:
- One real task reaches `in_review`, with the status moves and round records committed by `record` on the base
  branch.
- Every `ask_human` is recorded with its cause. Each cause is a case from its stop rules §3 or a builder's
  stop-and-report. None comes from Collide's own policy.
- `reviewed_by` names a reviewer that ran.
- The prototype board shows the run lit while it runs.
- An adopted chain continues the round numbering of its existing round records.
- A settings hook added to the build worktree's `.claude/settings.json` does not run in the reviewer's session.
  Positive control: it does run without the snapshot flags.

**Tests**: The pack's own `pack test` with fakes passes in CI. The real run's timings and `ask_human` causes are
recorded in a spike folder under `.collide/planning/spikes/`.
**Status**: Not Started. Items from revision 13, not yet split into items.

## Milestone C: Integration and ecosystem

## Stage 11: Group runs, integration, write-back

**Goal**: Several tasks at once, and a finished run that ends integrated and recorded.

- **First, outline Milestone C** (Stages 11 and 12) against the real loop of Stage 10 (0.15).
- Group runs from a selection or a view:
  - one run per `ready` task, under a concurrency cap;
  - a dependent starts once its dependencies are done at the base ref (integrated), not merely started. A run is
    integrated when its `integrate` has answered `merged {commit}` and that commit is an ancestor of the line's ref. A
    `pending {url, head}` answer, a PR that is open, does not count (0.4; R5);
  - a cycle is an error.
- Review and accept per run as in Stage 9, for every run of a group.
- **A group from a lens** (owner, 2026-11-14; 0.11). Grouping tasks to run together is an option, and the in-app agent
  proposes the groups. A group on the Field's lens selects its tasks and opens the group-run preview above. The agent
  has no tool for it: a person presses Launch, and the lens stays as the agent made it.
- **Up next** (owner, 2026-11-15; revision 21; 0.11). An ordered list of entries waiting to start, kept by the
  daemon in the store, so it survives a restart. One list per board (owner, revision 22).
  - An entry is a task, or a group: an epic, a view or a hand-picked selection. A person adds entries in the order
    they want (the Field: select, then Add to Up next; the CLI: `collide next add|list|move|remove`), and may
    reorder or remove an entry until it starts. Adding is the person's Launch: the in-app agent may propose an
    order, as it proposes groups, but has no tool to add one.
  - The next entry starts once the one before it has landed (integrated, by the rule above; for a workflow whose
    `landing` is `observed`, merged as Collide sees it on the line's ref, 0.4), so each builds on the last. A switch
    on the list lets independent entries overlap, up to the concurrency cap.
  - A group is one entry. Its tasks run as a group run (above): in parallel under the cap, in the order their
    `blocked-by` links give. The list moves on once every task of the group has landed.
  - A run of an entry that fails, is stopped or is refused pauses the list and raises a Queue item. Nothing after
    it starts until the person resumes, skips or removes that entry.
- **Orchard's `session` workflow, for real** (0.4, 0.7; revision 22): once, with the owner, on a small group. Its
  label passes `dev worktree new`'s check, its worktrees are attached with their tasks, its merges land them, and Up
  next moves on. Whether the lead's one label fits review records that name the reviewer that ran is checked here.
- **What an agent is told about its group** (owner, 2026-11-15; revision 21). How a group is worked is the rule of
  the applied packs and of the repository's own instructions (0.3), never Collide's: a team may already have rules
  for how its agents treat a group of tasks. Collide hands the agent what those rules need, in the phase's brief and
  as `COLLIDE_GROUP` beside the run's other context variables: the group's name and kind, the source's own handle
  for it (an epic id, a collection), its tasks in order with their status, which of them this run is for, and the
  branch and commit each earlier task landed as. A workflow may declare `group: per_task` (one run for each task,
  the default) or `group: whole` (one run given the whole group), for a pack whose rules work on a group at once.
- Syncing (0.4): how far each workspace's base is behind its line's ref (the conflict check before `integrate`
  ships in Stage 9); a pack's `sync` implementation, run as a `script` phase or before `integrate`.
- The two integrate implementations:
  - `orchard/merge-local`: a local merge into the base ref;
  - `github/pull-request`, from the built-in GitHub pack: push and open a PR linked to the issue, with no closing
    keyword. It answers `pending {url, head}` and the run is `integrating` until the PR merges, then `merged {commit}`.
    The issue closes through the accept op, after the merge (second-pack stage, S11; R5).
- Write conflicts and merge conflicts go to the Queue. A task from an untrusted author needs confirmation.
- **Planning** (owner, 2026-11-13), after S10, in `collide/plan`: panel, specialist and adversarial spike
  workflows; the merge script and the merger phase; disagreements as question items, with a `wait` phase; the
  verify gate, with tolerance, sampling, a verify budget, and an ask before running a teammate's scripts; the cost
  preview.

**Success Criteria**:
- **Coming back** (0.2; revision 22): with three Up next entries and the daemon stopped, one entry's task done
  elsewhere and the line's ref moved, the start takes that entry off, holds the list with a Queue item that names
  both, and starts nothing until the person resumes it. A stop in which nothing under the list changed holds nothing.
- Three tasks with dependencies run under a cap of 2. The dependent starts only after its dependency is
  integrated.
- Launching the MVP view starts only its `ready` tasks.
- Accepting an Orchard run merges locally and records.
- Accepting a GitHub-sourced run opens a PR linked to its issue. The run is `integrating` while the PR is open, and a
  task that depends on it does not start until the PR is merged and its merge commit is an ancestor of the line's ref
  (R5).
- One board runs an Orchard task under `orchard/loop` and a GitHub issue under a PR workflow at the same time, and
  each run is accepted through its own integrate implementation.
- A workspace whose base has moved on shows how far behind it is. An accept that would conflict goes to the Queue,
  and no checkout changes (`git status`).
- Accepting into the board root during a rebase waits, and the Queue says why.
- Re-prompting a run reuses no other run's workspace name or iter label run.
- A task from an untrusted author asks for confirmation.
- A lens group of three tasks, made by a fake agent, selects exactly those three on the Field. Launch opens the
  group-run preview for them, and nothing starts until the person confirms. The preview names a task in the group
  that is not `ready`, as it does for any selection. The tool list of 0.11 has no tool that launches (a test counts
  the five names). The owner asks the Field request on the fixture board and launches one group the agent found,
  recorded as owner judgement in the stage's task item.
- A failing gate on `run.before_integrate` blocks accept, and the Queue says why.
- S6 scenarios 3 and 5 pass as UI tests ported from S6's pilots, in the real app at full scope (group launch,
  accept), and the owner's walkthrough is recorded as owner judgement in the stage's task item.
- Planning:
  - a two-instance panel on fixture records produces a merge table whose cases match a hand-made table;
  - a changed number in one instance becomes a question item, and the run waits until it is resolved;
  - a `repro:` whose output drifts outside tolerance fails the verify gate. One that needs an hour idle is
    `unverified`;
  - the preview's estimate shows instances, merge and verify separately.

**Tests**: Order, readiness and cycle tests; conflict-to-Queue test; implementation resolution per workflow; the lens
group to preview path; the merge-tree conflict check; both integrate implementations against fixtures;
untrusted-author gate.
Planning: the merge cases, one per row of the merge table; verify verdicts; `wait` on linked questions.
**Status**: Not Started. Items from revision 13, not yet split into items.

## Stage 12: Second pack, composition, observers, grouped chains

**Goal**: Prove the extension points with packs that were not the reason they exist.

- Plain GitHub pack:
  - an issue → PR → CI workflow: a `script` phase pushes and opens the PR, and a `wait` phase polls its checks (a
    check still running is `pending` with `retryAfter`, 0.5; R5). The `github/pull-request` integrate then leaves the
    run `integrating` until the PR merges (0.4; R5);
  - PRs Collide opens carry no closing keywords (`Closes #n` and the like). The issue closes through the accept op,
    so its `task.before_status: done` gate runs before the merge and GitHub's own close does not race it. A
    Projects automation that moves the item after Collide's own close (`automated`) shows no outside chip (S11);
  - a lint/test gate on `task.before_status: review`.
- Standalone codemap pack: a `workspace.after_create` chain hook, composed with another pack's workspace
  implementation.
- `collide pack test`: fixture repo + fake adapter + `tsc --noEmit`. Owner, 2026-11-14 (backlog): users need an easy
  way to test their packs without going through the UI, so this is a first-class CLI path for pack authors, usable
  in CI, and is scoped from that ask when this stage is split into items. Stage 10's tests already run the Orchard
  pack's `pack test` with fakes, so the basic command may have to land by Stage 10; settle it when Stage 10 is split.
- `collide pack update <name> [--to <ref>]`, and update trust: show a diff of the changed pack (manifest, workflow
  graph, hooks), board config, settings or views before re-approving.
- Observers per 0.4: delivery, acknowledgement, restart, the disabled-observer Queue item, and a fixture observer
  that writes every event it receives to a file. Orchard's agent registry moves from its phase hook onto an
  observer.
- Grouped chains: one run of several tasks whose decision script returns a token per task, so tasks leave the chain
  in different rounds and the leavers share a closing fix. Orchard's version is its stop rules §5, with
  `g-<name>-rN` labels built from `group.name`.
- **Planning** (owner, 2026-11-13):
  - Share: the preview; the move op and its points; the handoff note built at Share; `record` of exactly the
    written files; a teammate continuing by `adopt`; claims, which are advisory without a server; conflicts to the
    Queue;
  - checklist template versions; Move for a changed planning path; provider role tags as defaults for `roles`;
  - a beads example provider as a fixture pack, not shipped. It covers a source that is not branch-scoped, a
    category taken from a close reason, and writes that Collide did not make. The pack sets `events-journal: true`
    and `BEADS_ACTOR` in its roles' `env:`, and re-baselines after `bd dolt pull`, because the journal is off by
    default and does not record sync (S11).
- **Global sources: attribution and phase-end checks** (owner, 2026-11-13; S11), for beads and for GitHub issues
  (GitHub issues are a common choice for developers). A write made inside a phase is attributed to the run. Its
  `after_*` points fire once, and its `before_*` gates run as a phase-end check. A refusal fails the phase and the
  write stands. The failure rules decide what follows (a `fix` phase can reopen the issue).
  - **beads:** `BEADS_ACTOR` is set in the phase's `env:`, from `run.slug` and `phase.label` (0.5, Env). The
    journal's actor names the run. The attribution is exact.
  - **GitHub:** writes carry the user's own login, so the actor cannot tell the agent from the person. Core matches
    the phase's tool calls (`gh issue …`, `gh api` on the issue, GitHub MCP tool calls, all read from the stream's
    `tool_use` inputs) to the issue's timeline events by issue, event type and the phase's time window. The result is
    labelled "attributed", never exact.
  - GitHub updates are polled, so core re-reads the run's issues at phase end, before the phase-end check.
  - A change that matches no phase is an outside change: no point, and the chip.

**Success Criteria**:
- Orchard, GitHub and codemap packs are applied to one board with no core change for any of them:
  - the codemap hook runs after Orchard's `create`;
  - the GitHub gate fires under Orchard's workflow when a task moves to `review`.
- On a test repo, the GitHub pack takes an issue to a PR whose checks pass, and the checks' time shows as waiting.
  A cancelled check makes the `wait` phase `unverified`, and the Queue offers a retry.
- `pack test` catches a broken decision script (an undeclared token).
- A two-task grouped chain closes the tasks in different rounds.
- The fixture observer receives every event of a run, in order.
  - Killed mid-run, it is restarted and resumes after its last acknowledgement, and the run is unaffected.
  - An observer that never reads does not grow Collide's memory.
  - The trust prompt lists each observer and its event patterns.
- The prototype board lights Collide's runs through the registry observer, and a crashing registry observer does
  not fail the run.
- Planning:
  - sharing a private proposal with the markdown provider adds exactly the proposal's files under the configured
    path, in one commit, and the private copy is gone;
  - sharing into the plan fixture creates the items through `./plan`, and the proposal's links point at the new
    ids;
  - on a second clone with another HOME, a teammate adopts a shared proposal, and a fresh agent starts from the
    committed note;
  - the Share preview lists no transcript, prompt or metric file;
  - two offline claims on one spike show "claimed by two" after both fetch;
  - a refusal on `task.before_move` leaves the whole proposal private;
  - a pull that brings a changed decision fires no point. Observers get the change, and the item shows the
    outside-change chip, emphasized when the category moved;
  - a beads fixture agent's `bd close` inside a phase fires `task.after_status` once, attributed to the run;
  - a `task.before_status: done` gate that refuses at phase end fails the phase, and the write stands;
  - a `gh issue close` inside a phase is attributed (by tool call, issue, event type and time window) and checked the
    same way;
  - the same `gh issue close` made by a teammate on the web fires no point and shows the outside chip;
  - after the planning path changes, items at the old path still list, and Move commits the renames with the
    config change in one commit;
  - a newer template version is offered as a diff and changes nothing until it is accepted.

**Tests**: Composition tests (implementation resolution, chain order, gate binding by status category); pack-test
harness in CI; grouped-chain engine tests; observer delivery (order, restart, duplicates after restart, a reader
that stalls).
Planning: the Share preview's contents; the move op for markdown, plan and beads fixtures; adopt from a note;
claim collision; move-op points; the path Move; the beads fixture's contract tests.
Global sources: beads attribution from the journal's actor; GitHub attribution from `tool_use` inputs and timeline
events (matched, ambiguous and unmatched cases); the phase-end re-read; the phase-end gate.
**Status**: Not Started. Items from revision 13, not yet split into items.

## Milestone D: Measurement and experiments

Ends with a developer able to see where a real run's time and resources go, and to test an optimization against
the current workflow.

## Stage 13: Full metrics

**Goal**: With `full` on, a real orchard run shows where its CPU, memory, disk, network and, when it declares a
local model, GPU went, at the granularity S8 established.

- **First, outline Milestone D** (Stages 13–15) against the measurements of Stages 9–12 (0.15).
- **Before this stage starts, the owner runs the one step S8 left unmeasured** (0.13; Deferred): a run measured from
  its start, to price a container's startup. The machine-sleep test moved to Stage 3b (R5), where it checks the clock
  table of 0.2 and the 2 s threshold.
- The sampler at 1 s (S8: 0.11% of one core). Disk I/O per workspace, and disk space per cache. Exit totals and disk space per
  workspace are always recorded from Stage 3.
- Network through one streamed `nettop` with its stdin held open (0.13). Device GPU utilization and memory
  (IOAccelerator), polled every 5 s while a run declares a local model (0.13; owner, 2026-11-14). Per-process GPU
  time waits for the first local-model adapter (0.13; owner, 2026-11-13).
- Lane containers are always counted, listed through the Docker API (`one-shot=true`, 8-14 ms a call) and not the CLI (2 s a call; S8: 53% of CPU in one run).
  Detached host processes and the container ids come through the pack's `metrics.processes` hook, which returns
  container ids and pids.
- `metrics.import`, selected by the run's worktree paths, with the backfill of existing history.
- Machine sleep recorded as waiting, by S8's method (a gap over 2 s).
- The machine part of the fingerprint; load sampled throughout.
- Privacy defaults per 0.13; retention.
- Claude telemetry traces (a beta flag) for the headless phases whose pack asks for exact timing, as 0.13 describes.
  The stream stays the default.

**Success Criteria**: On a real orchard run with `full` on:
- the lane container's usage, and its host server processes' usage (`serverPid`, `uiPid`), appear under the
  workspace's spans through the pack's `metrics.processes`;
- a run from a fixture pack that declares a local model and runs a known GPU workload (a fixed Metal compute loop
  for a set time) shows device utilization over that interval, within one sample at each end, and a run that
  declares no local model shows no GPU column;
- iter's lines for that run's worktrees import as child spans, and no other worktree's lines do;
- the backfilled history appears as baseline spans;
- the sampler stays within 1% of one core at 1 s, measured on the run (S8 measured 0.11%);
- argv is stored as basenames and flag names, and a credential seeded in a URL argument is absent from the store;
- a run during which the machine slept shows the sleep as waiting.

**Tests**: Sampler attribution against a fake process tree; the `metrics.processes` contract; importer fixtures,
including iter sub-calls; redaction fixtures; sleep accounting; retention; a lane container listed through the
Docker API; the sampling row's wording: exact for CPU and approximate for short-lived processes.
**Status**: Not Started. Items from revision 13, not yet split into items.

## Stage 14: Lines

**Goal**: Several refs open on one board, each running its own workflows and its own scripts.

- `collide line add <ref>`; line roots through a workspace implementation's `line` hook (kind `line`).
- Per-line packs and default workflow; snapshot pinned per run.
- Per-line trust, with the `protectedPaths` diff against the default line.
- Line locks per root. Hooks at the line root. Writes run the owning line's `./plan`.
- Launch resolves (shortId, path) at the line's ref. One live run per task across lines.
- Observers receive only their own lines' events (0.4).
- The optional Collide-owned default root (`defaultRoot: managed`, 0.12).
- Branch lines in the line selector (0.11), each with its own views.

**Success Criteria**:
- A second line whose `ws-create` hook differs creates workspaces with its own hook. The default line keeps the
  original.
- A line whose dev CLI differs from the default line's shows that diff before it is trusted.
- Running a task on a second line while it has a live run on the first is refused.
- Two lines `record` at the same time without waiting on each other.
- An observer of a pack applied only on the second line receives no events from the default line's runs.
- With `defaultRoot: managed`, Collide provisions the default root as a worktree on the base ref. While the user's
  own checkout has the base branch checked out, it refuses, and says why.
- With a run live on each of two lines, switching the selector from one line to the other and back stops nothing.
  The Field shows each line's own views, and Tracks shows both runs.
- A trust hash that changes on a line that is not selected raises a Queue item, and that line's Up next entry does
  not start until it is trusted.

**Tests**: Line snapshot and trust isolation; lock scope; per-line provider writes; collision refusal on launch;
observer scoping; the managed root's refusal.
**Status**: Not Started. Items from revision 13, not yet split into items.

## Stage 15: Experiments and bench

**Goal**: A developer tests a workflow optimization: one task runs under two arms in sequence, the comparison shows
what changed in time, resources and outcome, and nothing outside the experiment changes.

- Experiments per 0.14:
  - arms from lines, launch parameters, env overrides, profiles and pack overrides (a `dev:` pack refused);
  - points `experiment.before_arm` and `experiment.after_arm`;
  - experiment roots on branches under the pack's experiment prefix, reset to the arm's pinned commit before each
    repetition;
  - isolation per the table: the pack's `experiment` section (`prepare`, `env`, `unchanged`, `appendOnly`), a copy
    of each pack's data directory per arm, and the inventory diff;
  - an arm's workspaces discarded once its metrics are captured;
  - the pinned permission allowlist;
  - the design choices: `AB`, `ABBA` or `crossover` (with its seed), a fixed pair count, warm-up, A/A, and waiting
    for other runs;
  - abort.
- The comparison:
  - the primary metric, from Collide's measurements or hook-reported spans only;
  - the exact crossover test, with Holm over the secondary metrics only;
  - effect sizes for every other design;
  - the A/A baseline;
  - per-phase-kind agent totals;
  - outcomes;
  - "measured by different gates";
  - "on this task".
- `collide bench` for repeatable steps: a 10-pair crossover with A/A pairs by default.
- The token strategy preset as two profiles, and the before/after comparison at a fingerprint change (0.14).
- Profile rules: the board's `profiles.rules` pick a run's profile by `when:` over the task (0.7).
- The experiment UI (S6 scenario 7), with the preview's cost and duration estimate.
- **Panel of agents** (owner, 2026-11-15; revision 18, from the Field mockup's ideas): agents of mixed models take
  one spike's question as a `fanout` phase's `instances:` in their own worktrees, and see nothing of each other
  until they finish. Their claims show side by side as agreed, split or raised by one agent, and a split or a
  claim raised by one agent can be added to the spike as a check. A split is a reason to test, not a vote. It
  waits for this stage's comparison.

**Success Criteria**:
- **Positive control:** arm B is arm A plus a known `sleep 60` in a dev CLI step that `ws-create` runs. The
  comparison recovers that span's delta as 60 s, within the A/A spread.
- **A/A:** on a fake with no difference, it reports "no verdict".
- **Inventory:** after an Orchard experiment, the inventory diff is empty:
  - refs and worktrees;
  - the pack's `unchanged` paths (the codemaps byte-identical);
  - the pack's `appendOnly` paths, whose earlier bytes are unchanged;
  - the containers and processes `metrics.processes` reported;
  - the Claude adapter's memory directory.

  The same run outside experiment mode does change it (a positive control for the check).
- **Generic:** the same experiment on the fixture pack passes, with no Orchard code involved.
- **Global sources:** a fake global provider receives no `apply` call during an experiment.
- **Order:** against a fake whose later runs are faster by a fixed amount, `ABBA` and `crossover` report that order
  effect and remove it from the difference. `AB` shows the difference marked "order not balanced". Every result
  records its position.
- **Verdict rule:**
  - `AB`, `ABBA` and a 6-pair `crossover` show effect sizes and "no verdict", and say why;
  - an 8-pair `crossover` on a fake with a large fixed difference gives a verdict, labelled "on this task";
  - at 8 pairs the preview allows one secondary metric with a verdict, and says so;
  - a reported `metric` line cannot be chosen as the primary metric, and a line a hook's child wrote through an
    inherited descriptor does not count as a hook's own report (0.5, 0.14; R5).
- **Repetitions:** a status written by `on_start` in repetition 1 does not stop repetition 2 from launching,
  because the experiment root was reset.
- **Pack data:** a hook that writes to `ctx.dataDir` in an arm leaves the board's data directory unchanged.
- **Disk:** an arm's task workspaces and sandboxes are gone (`git worktree list`) before the next arm starts.
- **Other runs:** with a fake run live, "wait" holds the start until it ends. "Start now" starts, the result is
  flagged with the number of concurrent runs, and an inventory difference is listed beside that number instead of
  failing the experiment. A B that is not A plus its change warns.
- **Bench:** `collide bench` on the fixture pack's `workspace.create`, with B adding `sleep 2`, gives a verdict from
  its default 10-pair crossover and shows the A/A pairs.
- **Estimate:** the preview of an 8-pair Orchard crossover shows its expected duration from earlier runs, in days.
- S6 scenario 7 passes as a UI test ported from S6's pilot, and the owner's walkthrough is recorded as owner
  judgement in the stage's task item (S6's threshold is advisory).

**Tests**:
- Isolation, with positive controls.
- The `discard` path and per-arm discard.
- The experiment-root reset.
- All three orders against a fake with a known order effect. The crossover seed reproduces its order.
- The fixture-pack experiment.
- The exact crossover test and Holm over secondaries against fixed fixtures (smallest p 0.029 at 8 pairs, 0.008 at
  10).
- Abort mid-arm.
**Status**: Not Started. Items from revision 13, not yet split into items.

---

## Deferred (with the trigger that brings each back)

- **Other agent tools and model sources for runs** (a real ACP agent, Codex, OpenAI, Ollama): after Stage 10. S7
  checked the traits against ACP, and Stage 3d builds the ACP engine against a fake child. The first real agent is
  opencode: a profile in `host` and a probe of what it supports, so a capability that cannot be known is `Unknown`.
  The ask box already uses opencode from Stage 6b, with a read-only profile (0.3), and a run profile is still this
  item. Aliases and capabilities exist from Stage 3. With other endpoints, repeated
  endpoint failures pause the runs that use it, like a usage limit, rather than failing each phase in turn (Ferry's
  circuit breaker).
- **Per-process GPU time:** with the first local-model adapter (owner, 2026-11-13). S8 measured what macOS gives,
  and device GPU ships in Stage 13. In v0, Claude's inference is remote, so no process of the user's uses the GPU for it.
- **Resource tokens** (local-model GPU capacity, shared services): with the first non-Claude adapter, or when two
  packs in use throttle the same shared service themselves. R1 noted that orchard lanes are throttled by `iter`
  itself, and Ferry throttles its Docker runners the same way.
- **Observers that act** (start a run, write a task), such as pre-provisioning a workspace when a task becomes
  `ready`: when measured provisioning wait becomes the bottleneck. Acting needs an authenticated way in, which v0
  deliberately does not give scripts.
- **A local REST API** for external tools (CI, chat bots, team dashboards) to read or control Collide: when one
  needs it (owner, 2026-11-13: not now). Until then, `collide … --json` is the query interface. The UI-tools endpoint
  (0.2) is not that API: it changes the open window's view and reads what the window holds, and it cannot act.
- **More task operations:** GitHub `create` and `setFields`, and plan notes: when a user needs them in the UI. They
  are not needed in Milestone A.
- **Moving a scratch task into a source** (GitHub `create`, plan `create`): when a user wants a scratch task tracked
  by the team. The move op that Share adds for planning (the second-pack stage) is the mechanism.
- **Points for foreign changes** (a teammate, a pull, a hand edit, a bot): when a team needs a reaction that the
  acting machine cannot run. Until then, observers and the outside-change chip (owner, 2026-11-13).
  - Advisory gate checks on observed changes come first. A pack's `before_*` gates run as checks on the machine that
    sees the change, and report "gate X would refuse". It needs `via` and the `task.changed` shape, not a new point.
  - Run-made writes are covered in 0.4 (S11).
- **Read access to the corpus for hooks and run agents** (owner, 2026-11-13; part K): a read-only
  `collide tasks --where '<CEL>' --json` that hooks may call under a per-call read capability, or a read-only MCP
  server for run agents. When a gate needs links two hops out, or an agent needs planning state its backend's own CLI
  cannot give. Until then hooks get `tasks[].links` one hop out (0.5), and run agents use the backend's CLI. The ask
  agent is separate: its `query` tool (0.11) reads the window's tasks and runs.
- **A GitHub App identity for agents** (owner, 2026-11-13; part K): agents write through an App installation token,
  so GitHub shows the change as the App's and attribution is exact. When the attributed matching (0.4) gets it wrong
  in practice, or a team wants agents' writes marked on GitHub.
- **Ask the agent from a Queue item** (owner, 2026-11-13; part L): a button that resumes the session as a fork at the
  request, asks why, and leaves the live run untouched, with its cost shown before it is sent. Whether a session
  waiting on a permission can be forked is unverified. When S6's scenario 9 or real use shows the context view is not
  enough to decide.
- **Tools that act** (launch, accept, edit, delete; owner, 2026-11-14): when the owner has launched groups from a
  lens by hand in real use (Stage 11) and wants the agent to do the last step. The token the endpoint needs exists
  from Stage 6, so the older condition is met. The first such tool would not launch: it would open the run preview
  with the selection filled in, so a person still presses Launch. Accept, edit and delete each need their own design
  and a confirmation that the agent cannot answer (0.9).
- **Pack-supplied agent entries and a profile per kind of work** (S6's list for a real Settings page): when a pack
  author wants to ship a recommended ask agent, or the owner wants one agent for asks and another for plans. In v0
  agent entries come from the built-in default and the user's file (0.7).
- **Settings changes with a diff before save and an undo** (S6): when a bad agent entry costs the owner a wasted ask
  or a lost key twice. In v0 a save is validated and the page shows each field's layer.
- **What S6 left unmeasured about the ask agent**, each run before the case is named supported:
  - a hosted opencode model, since its own default timed out at 90 s and no reason was found;
  - a second local model, and three runs per request on opencode instead of one;
  - the ACP permission path under a real write attempt, and under a read that reports no path (0.3);
  - a window that is in the background and may throttle its answer;
  - a lens whose first group is shown before the rest, to cut the 15 to 22 s;
  - another MCP client through the endpoint's token forms, such as goose or Codex.

  The trigger for each is the owner naming that agent or case for daily use.
- **Packs from package managers** (npm: the tarball fetched by Collide over the registry API, never `npm install`,
  integrity-checked, only its `collide-pack/` folder used, hashed to a tree id) and **a pack index** (a git repo
  that maps names to git URLs): npm when a pack author asks for semver ranges or a team for a private registry; the
  index when there are packs to discover. Other package managers (brew, cargo, pip) are not planned.
- **More of Ferry's patterns in core** (a fan-out sized at runtime, one batched decision at the end of a long run,
  a replay cache): when a pack other than an example solution needs one (`docs/research/ferry.md`). The batched
  decision stays deferred while question items and a `wait` phase serve planning (owner, 2026-11-13).
- **When a pack outside this repo and the Orchard repo exists:**
  - a table layout;
  - a full dependency-graph layout (run order ships in Stage 6);
  - a third-party provider contract suite;
  - freezing `collide/v1`.
- **Cross-source dependencies** (a GitHub issue waiting on a plan task) and cross-board launch for multi-repo
  Projects: when a real board needs one.
- **Metrics beyond one machine.** Sharing metrics with a team, and OTLP export to an external collector: when
  someone asks to compare across machines. Spans are shaped like OpenTelemetry's, so export is cheap.
- **Claude Code telemetry on by default for headless phases:** when the traces flag leaves beta. S8 showed it is
  more exact for some things, so Stage 13 offers it to a pack that asks (0.13). Interactive sessions and `agentWork`
  phases use it from Stage 9 (measured S2).
- **S8's owner step** (owner, 2026-11-14): a run measured from its start so that a container's startup cost is priced.
  Before Stage 13 (Full metrics) starts. The machine-sleep test moved into Stage 3b (R5).
- **A Collide-managed copy of Claude** (owner, 2026-11-14; R5 Opus challenge 5): Collide would keep its own pinned
  copy of `claude` and run it, so an update on the machine could not change what a run does. Trigger: a Claude update
  breaks a run despite the version-range check and smoke check (0.3, Stage 3b).
- **A daemon kept alive by launchd** (a `KeepAlive` LaunchAgent): when users want a crashed daemon back with the app
  quit. v0 starts it on demand, and the resident app restarts it (0.2).
- **Open at login** (an `SMAppService` login item that starts the app hidden in the menu bar): when users ask. The
  resident app then keeps the daemon up.
- **A separate menu bar helper without WebKit:** when the resident app's memory with its window closed matters
  (53 MiB, the S2 record's addendum).
- **Live observation of Claude sessions started outside Collide:** when the past-session import shows users want it.
- **More detectors** (files read again, prompt size, cache churn, model fit): when the first seven are in use.
- **In-session guards** (a whole-file read, a third identical failing command): a pack or plugin can ship them, and
  the token strategy preset can test them. Core adds them when such an experiment shows they pay.
- **Cohorts** (automatic grouping of ordinary runs by fingerprint, with verdicts): after experiments have real use.
  Comparing two finished runs ships in Stage 9.
- **Re-judging an arm with the other arm's gates:** when "measured by different gates" blocks a real decision.
- **Arms in parallel**, when a machine is large enough that contention does not distort the comparison. Sequential
  is the default because parallel arms compete for CPU, disk and lanes.
- **An OS sandbox or containers for agents** (the real security boundary).
- **Windows** (ConPTY, Job Objects).
- **Linux:** a systemd user service with `KillMode=process`, `/proc/<pid>/environ` for the marker scan,
  `PR_SET_CHILD_SUBREAPER` for orphans, and `ru_maxrss` in KiB (S3, herdr-core).
- **Secrets on Windows and Linux** (owner, 2026-11-16): each is one more store behind the `SecretStore` trait
  (`host`), from the keyring-rs store crates: `windows-native-keyring-store` and `zbus-secret-service-keyring-store`,
  over `keyring-core`. First check that a store can be used without `keyring-core`'s process-wide default, which the
  injected store of the tests needs. Moving macOS onto `apple-native-keyring-store` too needs `security-framework`
  3.7 or later (now 2.11.1): decide then. A Linux machine with no desktop keyring (headless) has no Secret Service,
  and the kernel keyring forgets on reboot: a product question for that stage.

## Open questions (owner's call)

1. **Where the Orchard pack lives.** Settled in revision 9: the Orchard repo (`.collide/packs/orchard/`), with a fixture
   copy in the Collide repo for its tests (0.7). Either place is safe, because pack code loads from the base-ref
   snapshot, so the owner can still move it.
2. **Platforms.** Settled (owner, 2026-11-15): macOS first; Linux and Windows both follow, later (Deferred).
3. **Distribution.** Signed and notarized builds plus the Tauri updater, or source-only at first.
4. **Stage count.** Fifteen stages in four milestones, well beyond the usual 3–5.
   - R1 and R3 both found stages too large to land as small changes. The owner's additions (an early board,
     metrics, lines, experiments, the skeleton) each became a stage.
   - Each milestone ends usable.
   - Milestone D can be deferred whole without touching A–C.
   - R5 (revision 13) split Stages 3, 4, 6 and 9 into lettered sub-stages, with no renumbering: 3a–3d, 4a–4c, 6a–6c
     and 9a–9d. That is 15 stages and 15 sub-stages, each sized as one small change. Two are new work, not splits: the
     thin loop (3c) and GitHub Projects v2 (6c, moved from Stage 5) (owner, 2026-11-14).
5. **When the desktop app appears.** Decided (owner, 2026-11-12): a read-only task board in Stage 6, and the run UI
   in Stage 9.
6. **An experiment spending cap.** Decided (owner, 2026-11-12): none for now. Per-role budgets still apply, and the
   preview shows the total.
7. **A personal override of a shared setting.** The repo's value wins (owner, 2026-11-13), so a developer cannot
   change a shared key on their own machine: for example, a smaller lane pool on a small laptop. Options:
   - (a) No override, as now. Simplest, and every machine behaves alike. A key that really differs per machine has
     to stay out of the repo file.
   - (b) A pack marks a key `overridable: true`, and for those keys the user's file wins. The pack author knows which
     keys depend on the machine. It adds one flag to the schema.
   - (c) A git-ignored `.collide/config.local.yaml` above the repo's file. Familiar from other tools, but it
     overrides every key, so it quietly undoes the "team setting applies to everyone" rule.

   Decided (owner, 2026-11-13): (b). User profiles follow the same rule (0.7).
8. **When the device GPU is polled** (S8, revision 11). S8 measured about 25 ms of CPU per `ioreg` call, so a 5 s
   poll is cheap, and recommended polling only when a process in the run's tree or its pack declares a local model.
   Revision 10 and Stage 13 poll the device in every `full` run, and Stage 13's Metal-loop test depends on it.
   Options:
   - (a) Poll the device in every `full` run, at 5 s: about 0.5% of a core for the device call alone, and the Metal
     test runs as written. A run with no local model gets a utilization column that mostly shows the desktop.
   - (b) Poll only when a local model is declared (S8): no cost and no noise for Claude-only runs, but Stage 13's
     test then needs a fixture that declares one, and device GPU does not appear until the first local-model
     adapter.

   Decided (owner, 2026-11-14): (b). Stage 13's test uses a fixture pack that declares a local model (0.13).
9. **The in-app agent's four open questions** (S6, revision 12). The prototype left these open:
   - which agent the ask box uses by default;
   - whether the UI-tools endpoint needs a token before any tool can act, or only once a tool launches;
   - whether Collide launches Claude Code for an ask with the user's own config or with the strict line a run gets;
   - whether a per-ask budget is a promise of the product, given that only Claude Code can enforce one.

   Decided (owner, 2026-11-14), applied in 0.2, 0.3 and 0.7:
   - (a) Claude Code with haiku. ACP agents and External MCP clients are set up on the Agents page;
   - (b) a per-session token from day one, in a header or the URL, shown in the External snippets and rotatable from
     Settings, in the spirit of the socket's per-client token;
   - (c) the strict line, with settings from Collide's snapshot and not the user's CLAUDE.md, hooks or memory, and a
     per-agent opt-in "use my Claude config";
   - (d) a budget that is optional and off by default, enforceable for Claude Code only, with a timeout for every
     agent. The Agents page says plainly that ACP and External agents get a timeout only.

## Risks

- **Claude Code's stream-json is not a versioned contract.** Mitigation: the real-claude contract test (Stage 3b), a
  tested version range with a smoke check and a settings canary (0.3; R5), and the version in the fingerprint. A
  Claude update can still break a run between checks. A Collide-managed copy of `claude` is Deferred, with that
  as its trigger.
- **Same-user agents** (0.1): guardrails only until an OS sandbox exists. This includes experiment isolation, which
  cannot stop an agent's Bash from writing outside it.
- **Packs from anywhere run as the user.** A `git:` pack is someone else's code, and its hooks run with the user's
  rights. Mitigation: trust per tree id, with the hooks, providers and observers listed; a new tree id asks again
  with the diff; no package-manager source in v0, so no dependency chain (0.7, 0.9).
- **Cost:** `--max-budget-usd` per role. A concurrency cap.
- **Usage limits.** Parallel runs share one account, so they hit limits together. Runs pause rather than fail (0.3),
  but a busy day can stall every run until the reset.
- **GitHub writes race.** There are no conditional writes, so a conflict check narrows the window but cannot close
  it.
- **Ref-scoped identity.** Per-checkout id allocation makes collisions normal in orchard. The UI must make them
  visible, or users will act on the wrong task.
- **Orphaned processes.** The marker scan works on macOS, but two kinds of process escape it: one started with a
  cleaned environment, as Orchard's lane servers are, and a double-forked orphan whose final image is an Apple
  platform binary, whose environment `ps -E` cannot read (measured S3). Clearing the variable inside a process does
  not escape it; only exec with a cleaned environment does. The fallbacks are the hooks' `detached` records, the
  cleanup hook, and the survivor report (0.5).
- **The daemon dies with the app.** On macOS a daemon left in the app's coalition can end when the app quits.
  herdr-core shipped this; S2 did not reproduce it with quit or force-quit. Mitigations: the one-shot launchd start,
  which gives the daemon its own coalition (measured S2), and the resident app restarting a dead daemon (0.2).
- **Token cost of each phase.** With the user's config a phase starts at about 31K tokens, against 14.4K under
  `config: none` as Collide launches it (measured S9), and a cold resume writes the whole context again. Mitigations:
  the context metrics, `config: none` per role, and `on_cold` (0.3).
- **User tooling changes phases.** Hooks and plugins in the user's config load into every phase under `config:
  user`. Mitigations: the preview lists them, a blocked prompt goes to the Queue, and experiment arms use `config:
  none` (0.3).
- **Writes into the user's own checkout.** The default line root is where the user works (0.12). Writes wait while a
  rebase or merge is in progress, and `record` commits only the files it touched, but `integrate` still merges into
  the branch the user has checked out. The managed root avoids this at the cost of a full checkout.
- **Shared settings reach everyone at once.** A bad value in `.collide/config.yaml` applies to every developer. It
  passes through the trust prompt like any other committed Collide file, and the settings view shows which layer
  each value came from.
- **Misleading comparisons.** Agent phases vary from run to run, the second arm runs with warmer caches, and an arm
  can change its own gates. Mitigations (0.14):
  - verdicts only from a randomized `crossover` with at least 8 pairs;
  - A/A baselines;
  - balanced orders and seeded pack state;
  - concurrent runs recorded and flagged;
  - "measured by different gates";
  - outcomes beside timings.
- **Metrics overhead.** Sampling must stay under 1% of one core, or `full` mode distorts what it measures. S8
  measured 0.11% at 1 s for the sampler, and 8-14 ms per container for one Docker API stats call (measured). The
  sampler's figure does not include a run with many lanes (unmeasured).
- **Experiment cost.** Each arm provisions several worktrees: an experiment root, a task workspace and sandboxes. A
  orchard arm can spend tens of minutes and several GB of disk before an agent starts. A verdict needs at least 8
  crossover pairs, which for an Orchard loop means days. Effect sizes and `collide bench` are the cheaper paths.
- **The in-app agent's judgement can be wrong** (owner, 2026-11-14). A lens can put tasks that touch different code
  in one group, and a person could launch that group. S6's haiku was right in 6 of 7 runs that have a reference
  answer, and the seventh was repaired by a prompt change, so the prompt may be tuned to one demo (inferred).
  Mitigations (0.9, 0.11):
  - the chip says what changed in Collide's words, and the agent's words are marked as its own;
  - a `show_groups` chip carries "No files were read" when the ask read nothing;
  - Undo restores the screen, and a lens is a timestamped snapshot, never a View, so it cannot pass as a fact;
  - invented ids are rejected, never repaired;
  - the agent has no tool that launches, and a person presses Launch (Stage 11);
  - the opt-in real-agent contract tests re-run on any change of model or prompt, and record their pass counts.
- **The UI-tools endpoint is a new local door** (owner, 2026-11-14). Any process that holds a token can change what
  the window shows. Mitigations: 127.0.0.1 only, Host and Origin checks, a token from day one (the endpoint token in
  the keychain and rotatable, an ask token per ask), no tool that acts, and tokens never in argv or the environment
  (0.2). An ACP ask agent's own tools are limited only by Collide's permission answers, and its own configuration is
  its promise (0.3), so a hostile task text could steer such an agent further than a Claude Code ask, which has only
  three read tools.
  - R5 notes two more doors. The ask child's token is the endpoint's, so a process that reads the ask's `mcp.json`
    during the ask can call the endpoint until the ask ends (the ask token's lifetime bounds it). Interactive
    launches (New session, take-over, Plan…) pass the snapshot's `--mcp-config` with `--strict-mcp-config`, so
    they hold no endpoint token unless the person adds one (0.7). One rotatable endpoint token is kept. Named
    per-client tokens were considered and not built (owner, 2026-11-14).
  - The endpoint stays in the app shell. The stdio bridge (`collide mcp`) and a daemon-hosted endpoint were
    considered and not chosen: the shell is the only process that can answer a screen-changing call, and a bridge or
    daemon would add a hop to it (0.2).
- **An ask's token load and wait.** S6's asks loaded 28K to 107K input tokens on a launch that was not strict, and
  a lens takes 15 to 22 s (measured). The strict line, the budget (Claude only) and the timeout bound the cost, and
  Stage 6 measures the strict line.
- **Scope:** each milestone ends usable on the Orchard repo.

---

## Review log

**R1, 2026-11-12.** Two adversarial reviewers on revision 1: Fable (22 findings) and Opus (23). Both reached the
verdict "not ready". Stages are named here, not numbered, because the numbering has changed since.

Accepted, and where each change landed:
- No orchestrator or adjudication step: orchestrator role, `adjudicate` phase, `next` reads `**Next**` (0.7, the
  pack and workflow-engine stages).
- No data passed between phases, and no per-round ranges: `history[]`, artifacts, heads, `phase.dir` (0.8).
- Stages depending on later stages: `update` moved into the task-source stage; permissions through
  `--permission-prompts host` in the headless-core stage; SDK in the task-source stage.
- Task state is per ref: provider `scope`, `checkout` parameter (0.10, the task-source stage).
- Gates re-ran `iter`, and exit 2 meant two things: evidence gates; the result-file `status` is the verdict (0.5).
- `ORCHARD_ITER_AGENT` label: env per role and phase, `phase.label`, validated ids (0.5, 0.7).
- Grouped chains: `tasks[]` from v0, implemented in the group-runs stage.
- Two reviewers in round 1, and `readOnly` undefined: `fanout`, the `readOnly` check (workflow-engine stage).
- An agent could edit its own gates and reviewer: base-ref snapshot, `--agents` JSON (0.7).
- Trust too late, and prompt injection: first-open trust in the task-source stage; untrusted-input rules (0.9).
- Agents could reach the daemon: per-client token, no agent access (0.2).
- Pipes die with the daemon, and the daemon was chosen before its spike: file logs; S3 compares three owners
  including `--bg` (0.2).
- Stop missed containers and other process groups: descendant scan, cleanup hook, setsid test (0.5).
- Workspace lifetime, resume and timeouts: workspace states, `resume`, reaper (0.8).
- SDK under `node_modules` cannot strip types: compiled SDK, `tsc --noEmit` (0.6).
- Slots too rigid: namespaced contribute, several workflows, gates on named points (0.4).
- Stages too large: split (nine in revision 2, ten from revision 3).
- Spend cap promised but not planned: `--max-budget-usd`.
- herdr gives snapshots only: dropped.
- GitHub scope and polling details: the GitHub adapter stage.
- YAML reasoning wrong: corrected (0.7).
- Collide as a second register: principle in 0.1; budgets read from the pack.
- Cuts: ACP spike, herdr runtime, event handlers, scopes, graph view, `fieldSchema`, v1 freeze, lane tokens.

Rejected: none. Modified: Fable's "too many crates and traits" → two crates and three traits, keeping core free of
framework types.

**Revision 3, 2026-11-12 (owner).**
- Renamed to Collide.
- Task sources, corpus and views (0.10): the operation-based write interface, status categories, ref-scoped
  sources, views as lenses with CEL and closure.
- GitHub moved into Milestone A, so two adapters shape the protocol. `fieldSchema` is back, as `initialize`
  `fields`.
- Interface direction (0.11) and the S6 prototype.
- Stages renumbered to ten.

**R2, 2026-11-12.** Two adversarial reviewers on revision 3: Fable (15 findings, 7 R1 fixes not cleanly landed)
and Opus (14 findings, 8 R1 fixes not cleanly landed, 4 cuts). Both reached the verdict "not ready". The author
re-measured the load-bearing claims against the Orchard repo:
- `plan tasks` JSON keys;
- the id and epic formats;
- the label regex;
- `git worktree list`;
- the lead-moves-status brief text;
- `review_budget` in briefs and no task frontmatter;
- the id map;
- 14 of 715 merges mentioning a PR;
- the `agents.jsonl` registry.

All of them held.

Accepted, and where each change landed (stages named):
- **The UI stage needed the engine and group runs** (both, blocking): the run UI moved after the engine. Group launch
  and accept are graded in the group-runs stage.
- **The task-source stage used the pack system before it existed** (Fable, blocking): minimal pack loading is
  explicit task-source work.
- **`plan tasks` JSON lacks dependencies; ids and epics did not match the example** (both, blocking): the adapter
  parses frontmatter; `shortId` keys, aliases, normalized fields, collections (0.10).
- **Ref-scoped writes had no committer and broke `readOnly` and `worktree rm`** (both, blocking): no writes into a
  checkout with a live phase; the `record` hook under a lock; launch reads the base ref (0.4, 0.10).
- **The lead's actions were given to the wrong actor** (Opus, blocking): workflow `on_start`/`on_end` ops,
  `record`, the `integrate` owner slot with local-merge and PR implementations (0.7; the hooks, Orchard and
  group-runs stages). Revision 9 replaced the slot with implementations named per workflow.
- **Undefined view algebra, and a closure criterion that fails** (both): the set algebra; closure identical to
  the prototype board's generator at one commit, set and steps (0.10).
- **Views could not reach epics, slices or projects** (Fable): collections (0.10).
- **Identity across checkouts** (both): (shortId, path), collisions, renames from the id map (0.10).
- **GitHub versions, races, deletes, drafts, PRs, multi-repo, account** (both): per-item versions, a stated race,
  reconcile, configuration in `board.yaml`.
- **Snapshot leaks** (both):
  - SDK: Collide materializes the snapshot and supplies the SDK (0.6);
  - agent selection: `--agent collide-<pack>-<role>`, plus an S1 clash test;
  - the plan CLI: runs from a root, never a workspace (0.7);
  - views and `board.yaml`: in the trust hash (0.9).
- **Review rounds unlike the team's** (Opus): a fresh brief phase per round, sandbox workspaces, `review_budget` as
  a launch parameter, `ask_human` with its cause.
- **Env template contradictions** (both): `phase.dir` and `model.name` in 0.5. Revision 6 replaced `shortId` in
  labels with `labelId` (R3), and revision 7 made it a provider-declared template field.
- **Model aliases built by no stage** (Fable): `models.yaml` in the headless-core stage.
- **OpenAI/Ollama extensibility nominal** (both): the tool-vs-alias precedence, an honest scope statement, S7 (0.3).
- **Missing UI surfaces** (both): the task drawer, operation forms, the filter bar saving views, first run, the
  board switcher, provisioning progress, prototype-board interop.
- **Interface criteria did not test "intuitive"** (both): thresholds set beforehand, a non-owner first-time
  participant, a record that must say what changed (S6).
- **Views and `board.yaml` outside trust; personal views global** (Fable): snapshot + trust hash; personal views per
  board (0.9, 0.10).
- **CEL "cannot loop, so safe"** (Fable): cost and size limits (0.9, S5). **Three expression languages** (Opus):
  `when:` is CEL (0.7).
- **Watch cost and noise** (both): direct parsing, debounce, 10 s recheck, the checkout filter (0.10).
- **Orphan processes** (Opus): the `COLLIDE_RUN` marker scan; S3 adds a double-forked orphan (0.2).
- **`protectedPaths`** (Opus): pack-declared and flagged in review (0.7, 0.9).
- **Cuts:**
  - GitHub `create` and `setFields`;
  - plan `appendNote` and `recordReview`;
  - the `table` layout;
  - the expected duration without history.
- **The R1 log cited stale stage numbers** (Fable): the R1 entry now names stages.

Modified:
- Fable's S7, a 1–2 day spike driving `opencode acp`: became a paper check plus one manual run, since v0 is
  Claude-only.
- Opus's two non-designer participants: became at least one.
- Opus's `readOnly` blind to ignored files: documented as a limit rather than changed, because a reviewer's own
  `iter` run rebuilds ignored `dist/` files.

Rejected:
- Opus's "cut view `include`/`exclude`". The owner asked for hand-picked subsets such as MVP scenarios, which a
  source may not encode. Views hold membership only, never status, dependencies or evidence, so this is not a
  second register for anything "One register" covers.

Not re-measured by the author:
- the closure counts (26 vs 29);
- GitHub API behaviour (inferred by both reviewers);
- the fs.watch delivery times;
- the count of renumber commits.

**Revision 5, 2026-11-12 (owner).**
- Lines (0.12), metrics (0.13) and sequential A/B experiments (0.14), with spike S8.
- The owner's answers:
  - all resources (CPU, memory, disk, network, GPU), with GPU spiked;
  - A/B runs on the same task, in sequence;
  - metrics per machine;
  - several branches open with branch-specific workflows.
- Author measurements without root on an M2 Pro running macOS 26.3.1:
  - `ioreg` device GPU;
  - `nettop -P` per process;
  - Docker Desktop is the active context;
  - the OTel variables in the claude binary;
  - 3534 iter timing lines.

**R3, 2026-11-12.** Two adversarial reviewers on revision 5: Fable (12 findings, 7 R2 fixes not cleanly landed)
and Opus (14 findings, 7 not cleanly landed). Both reached the verdict "not ready", blocking on experiments.

The author re-measured:
- `<git common dir>/orchard-dev/` holds `agents.jsonl`, `codemaps`, `iter-shadow.jsonl` and `iter-timings.jsonl`.
- Its task guide says "There are no review worktrees", while the two newest briefs give the opus reviewer a detached
  sandbox worktree.
- `iter timings --write` refuses a label with several runs ("add --run").
- `dev worktree rm` refuses detached worktrees.
- Lane containers carry only `orchard-dev.*` labels.
- The claude 2.1.323 binary:
  - `claude --help` shows that `--agents` takes a file under `--print`;
  - it contains `CLAUDE_CODE_DISABLE_AUTO_MEMORY` and `OTEL_LOG_USER_PROMPTS`, `_TOOL_DETAILS` and
    `_TOOL_CONTENT`.
- Per-process GPU time is readable without root, through `AGXDeviceUserClient` `AppUsage.accumulatedGPUTime` with
  `IOUserClientCreator` pids.

All of these held. The last one corrects revision 5, which said per-process GPU needed root.

Accepted, and where each change landed:
- **Experiments had side effects through clone-wide state** (both, blocking):
  - the isolation table, the pack's `experiment` section (env, `prepare`, `unchanged`) and the inventory diff;
  - `scratch/collide-exp/…` branches, the `discard` hook and experiment workspace kinds;
  - auto memory disabled; the registry hook skips (0.7, 0.14, the experiments stage).
- **Arms and repetitions collided on workspace names and iter labels** (both, blocking): workspace names include
  `run.slug`; import selects by worktree path or the arm's file; the build brief passes `--run` (0.7, 0.13).
- **The statistics did not support the claims** (both, blocking):
  - the primary metric and an exact paired permutation test, needing at least 6 pairs, with Holm correction
    (revision 9: the crossover test, at least 8 pairs, Holm over secondary metrics only);
  - A/A baselines; seeded maps and a mandatory warm-up (optional since revision 7);
  - the quiet-machine check (since revision 7: concurrent runs recorded, and an optional wait) and per-phase-kind
    agent totals;
  - human waits excluded, and a pinned permission allowlist (0.14).
- **The experiments stage's criteria were vacuous** (both):
  - a `sleep 60` positive control and A/A "no verdict";
  - an inventory diff with its own positive control;
  - a fake global provider receiving no `apply`;
  - the merge-base warning (the experiments stage).
- **An arm can change its own measuring tools** (Opus): the `git diff --stat A...B` preview with `protectedPaths`;
  "measured by different gates" (0.11, 0.14).
- **Lines reopened the snapshot rule** (Opus): `protectedPaths` tree hashes in the trust hash, and the diff shown
  before trusting (0.9, 0.12).
- **Lines were under-specified** (both):
  - the per-line keys;
  - one lock per root;
  - writes run the owning line's `./plan`;
  - the snapshot pinned per run;
  - divergence within a line;
  - launch by (shortId, path) with collision refusal;
  - one live run per task key;
  - the map-window cost (0.12).
- **Workspace hooks contradicted the dev CLI's refusals** (both): `discard` uses `git worktree remove` and deletes
  branches; the `remove` rule applies to `task` workspaces only (0.7, 0.8).
- **Metrics overstated exactness and attribution** (both):
  - an accuracy and granularity column;
  - `wait4` limits;
  - `COLLIDE_SPAN` fixed at spawn, with tool attribution by parent pid and time overlap;
  - the pack container selector;
  - iter sub-call positions;
  - the GPU correction (0.13).
- **Privacy rule insufficient and self-contradictory** (both):
  - basenames and flag names by default;
  - full argv opt-in with URL credential stripping;
  - `--agents` as a file;
  - forced-off `OTEL_LOG_*`, stripped `OTEL_*`, and a user endpoint never overridden (0.3, 0.13).
- **Critical path undefined** (both): over the phase graph with the fan-out join; waits apart; self time below phase
  level; hand-built fixtures (0.13, the run UI stage).
- **The hooks, workspaces and lines stage was too large** (both): split into a hooks and workspaces stage, a full
  metrics stage (after a real loop exists) and a lines stage, with experiments last. Group runs and integration
  come before measurement, because they complete the core loop (Opus).
- **"Optional" metrics recorded without consent** (Fable): only what run state needs is always recorded. Resource
  data and the machine fingerprint are opt-in (0.13).
- **`shortId` case contradiction** (Opus): `labelId` and `labelIdPattern` separate from keys (0.5, 0.10).
- **`model.name` unvalidated** (both): lower-case letters only, and a fan-out on one model is refused (0.3, 0.5).
- **`record` swept the whole glob and only warned off-ref** (Opus): it commits exactly the touched files and refuses
  off-ref (0.8, 0.10).
- **Missing for a developer's first week** (both):
  - comparing two finished runs;
  - export;
  - disk space per workspace and cache;
  - annotations and outlier exclusion;
  - cache-read and cache-write tokens;
  - abort and cleanup;
  - backfill of iter history;
  - store growth estimated in S8.
- **S6 did not cover the new surfaces** (Fable): scenarios 6 and 7.

Modified:
- **Review sandboxes:** Fable proposed a probe directory or `git clone --local`, and Opus noted the cost. The pack
  follows the newest briefs: optional, per model, for the strong reviewer only.
- **Lines' provider:** Opus proposed one provider per line. Instead, one provider reads every checkout, writes run
  the owning line's `./plan`, and format skew is counted as parse errors.
- **Claude telemetry:** both proposed cutting it. It is deferred behind S8 instead.
- **Cohorts:** both proposed cutting them. They are deferred, and comparing two finished runs ships instead.
- **Agent A/B:** Fable proposed comparing only deterministic steps in v0. Agent phases are compared as effect sizes,
  with no verdict below 6 pairs (8 `crossover` pairs since revision 9).

Rejected:
- **Cutting GPU metrics** (Opus): the owner asked for GPU, and per-process GPU time turned out to be readable without
  root, so it is cheap.
- **Cutting `nettop`** (Opus): the owner asked for network. S8 measures its cost.
- **Replacing sequential A/B on arbitrary refs with bench plus finished-run comparison** (Opus): the owner asked for
  sequential A/B of the same task from two branches. Bench and finished-run comparison ship as the cheaper paths.
- **Replacing lines with one board per branch** (Opus): lines share the corpus, views and sources.
- **Using lines only for experiment roots** (Fable): the owner asked for branch-specific workflows.

Not re-measured by the author:
- the suite spreads (1.5–6×) and the recorded loads;
- the 4.1 GB worktree size;
- auto memory being active for the Orchard repo;
- orchard's Ollama having no Apple GPU.

**Revision 6, 2026-11-13 (owner).** The task board is built early, right after the GitHub adapter. There is no
experiment spending cap.

**Revision 7, 2026-11-13 (owner feedback on revision 6).**
- **Orchard is the reference pack, not a special case** (0.1). Moved out of core:
  - `labelId` became a provider-declared template field with a pattern (0.5, 0.10);
  - the lower-case `model.name` rule became a safe token in core plus the pack's `requires.modelName` (0.3);
  - the `scratch/` branch prefix became the pack's `experiment.branchPrefix` and board configuration (0.10, 0.14);
  - Claude's auto memory moved into the Claude adapter's declared isolation (0.14);
  - the inventory diff: core keeps git refs and worktrees, and the pack and adapter declare the rest (0.14).
- **Experiment design is the user's choice** (0.11, 0.14):
  - order (`AB` or `ABBA`), pairs, warm-up, A/A, and whether to wait for other runs;
  - the preview explains each one and its cost;
  - the number of live runs is shown before the start and recorded during it.
- **Skeleton first, test first** (0.15): a skeleton stage after the spikes, and each later stage fills its part in.
  Stages renumbered to fifteen.

**Revision 8, 2026-11-13 (owner).** The owner asked whether hooks and events cover most scenarios, and how scripts
and their subprocesses report metrics and pass state. The answers, accepted by the owner:
- **Points:** one catalogue defined by the skeleton. It adds `run.before_start`, `run.after_end`,
  `run.before_integrate`, `run.after_integrate`, `workspace.before_remove`, `experiment.before_arm` and
  `experiment.after_arm`, with the rules for `before_*` and `after_*` points (0.4). The 0.1 rule now applies to new
  kinds of mechanism, not to points in the catalogue.
- **Observers** moved from "Deferred" into the second-pack stage. They do not block, and delivery is at least once
  from the on-disk log (0.4). Observers that act on Collide stay deferred.
- **Report files**, not a REST API (owner's choice): `COLLIDE_REPORT`, the `collide report` command, and labelled
  reported data (0.5, 0.13). They land with the hooks stage rather than the full metrics stage, because the run UI
  uses `progress` lines and Milestone D can be deferred whole.
- **State:** hook `outputs` become run or workspace state, written only when the hook ends, with optional schemas
  and a size cap. There is no cross-run store, so state does not become a second register (0.5).
- **SDK `ctx`:** a convenience over the three files (0.6).

**R4, 2026-11-13.** Two adversarial reviewers on revision 8: Fable and Opus, asked to challenge the design as well as
find gaps. Both reached the verdict "not ready".

The author re-measured:
- the smallest exact p-values: 0.031 for a 6-pair paired test, and 0.10 for a crossover of 3 + 3 pairs;
- the dev CLI starts lane servers detached with only `PATH` and `HOME`, and lane records carry
  the server and UI pids;
- lane containers carry only the `orchard-dev.clone`, `.created`, `.id`, `.demo` and `.env` labels;
- the prototype board does not exclude `scratch/` checkouts (in its generator and worktree script);
- `iter-timings.jsonl` keeps growing from work outside Collide (3596 lines);
- `claude --help` (2.1.323) lists `--setting-sources`, `--settings`, `--mcp-config` and `--strict-mcp-config`;
- the pack example used `outcome` without the plan defining it.

All of them held.

The owner asked whether Fable's findings were about Orchard's current work rather than Collide. Four that read that
way (the verdict maths, the safe token, the CI name check, container labels) were Collide plan defects, relayed
without their plan locations. Both reviewers did spend effort on how Orchard runs day to day. So from this round on,
**a finding changes core only if a pack that is not Orchard (issue → PR → CI) would hit it too.** Orchard-only
findings went into the pack example or were dropped:
- the prototype board lighting experiment checkouts (accepted, and noted in the pack);
- work outside Collide appending to iter timings (`appendOnly`);
- the codemap install path;
- the lead's timings tables;
- one reviewer per grouped round;
- the "lanes are Docker" text.

Accepted from both reviewers, and where each change landed:
- **`integrate` was one slot per board,** while GitHub is on every board: named implementations chosen per workflow
  (0.4, 0.7; the hooks, group-runs and second-pack stages).
- **Workflows had no defined contract:** a declared transition table with `script`, `wait` and `human` phases,
  decision tokens, `round`, outcomes and terminal states, `accepts`, and ops by status category (0.7; the engine
  stage). Syncing with the base, through a `sync` implementation and a `merge-tree` conflict check (0.4; the
  group-runs stage).
- **The statistics could not give the verdict they promised:**
  - the randomized `crossover` design with at least 8 pairs;
  - `AB` and `ABBA` give effect sizes only;
  - pair counts fixed, never pooled;
  - Holm over the secondary metrics only;
  - "on this task";
  - the experiment root reset per repetition (0.14; the experiments stage).
- **State was overwritten across rounds:** run state per invocation; workspace state merged key by key and checked
  by `doctor` (0.5).
- **Report files:**
  - none for agent phases;
  - the primary metric only from Collide's measurements or hook spans;
  - generated span ids, `COLLIDE_REPORT_PARENT`, required timestamps;
  - containers and cleaned-environment processes stated as unable to report (0.5, 0.14).
- **Packs had no settings or storage:** the `config` schema and layers, keychain secrets, and a data directory
  copied per arm (0.7; the task-source and experiments stages). The owner added the repo's shared file.
- **Usage limits and sleep:** `paused: usage limit` with resume, and sleep counted as waiting (0.3, 0.13; the
  headless-core stage).
- **Coupling a grep cannot see:** a second fixture pack (issue → PR → CI) and a normalized-event fake adapter run
  the engine suite (0.1, 0.15).
- **`task.*` in a run of several tasks:** defined for one task only, and refused at launch otherwise (0.5).

Accepted from one reviewer:
- S3 tests that the next phase starts with the app closed (0.2).
- The lifecycle order table (0.4).
- `task.before_status` matches status categories, and fires from the engine stage.
- Workspace hooks return `{ path, gitDir }` (0.1).
- Stub tags are stable names, not stage numbers (0.15).
- The schema contradictions between the skeleton, task-source and hooks stages: the skeleton defines the full
  schema, and later stages use it.
- The safe token rejects `..` and a leading `-` or `.` (0.5).
- An arm's workspaces are discarded when its metrics are captured (0.8, 0.14).
- Take-over is tested with real Claude in the run UI stage. Fable said no stage built it; the run UI stage listed
  it, but nothing tested it.
- The GPU criterion uses a workload with known GPU use (the full-metrics stage).
- The Orchard pack lives in the Orchard repo, with a fixture copy (0.7).
- The shell forwards one generic `rpc` command, so UI tests run in a browser (0.2). Tauri stays.
- Found through Orchard, fixed generically:
  - lane servers are host processes, so `metrics.processes` returns pids as well as containers (0.13);
  - a chain in flight could not be picked up, so runs can be adopted at a phase and round (0.7; the engine and
    Orchard loop stages);
  - a reviewer in the builder's worktree loaded the builder's `.claude/`, so settings and MCP config come from the
    snapshot, S1 measures what still loads, and edits to that config wait for an acknowledgement (0.3);
  - hooks inherited `CLAUDECODE`, so the adapter declares the environment to strip (0.3);
  - an `unchanged` file that other work appends to could not stay byte-identical, so the pack declares it
    `appendOnly` (0.14).

Challenges to the owner's decisions, answered by the owner (revision 9 below): the whole-codebase skeleton (a),
same-task A/B verdicts (b), observers (c), GPU and network metrics (d), the default line root (e), and more review
rounds before any spike (f).

Not taken:
- Per-hook concurrency limits, and long-lived services per workspace: no pack needs them yet.
- Cutting the point catalogue back: the owner's revision 8 decision holds.
- One-shot providers; dropping CEL; a manifest declaration for skipping side effects in experiments (hooks already
  see `experiment`).
- Replacing Tauri with a localhost web app: the generic `rpc` command gives browser tests and keeps the desktop
  shell.
- Cutting lines: the owner asked for branch-specific workflows.
- Dropping the break-and-restore note from commit messages as unverifiable: it stays. Seeing each test fail is the
  owner's standing rule, and the note is how a reader knows it happened (0.15).
- Dropping the same-model fan-out refusal and the sandbox workspace kind: both stay.

Not re-measured by the author:
- Opus's 2.5–7.7 h for one Orchard loop (an upper bound, because sub-calls nest);
- the managed root's cost beyond R1 and R3's numbers.

**Revision 9, 2026-11-13 (owner).** The owner took the author's recommendation on each R4 challenge:
- **(a) Skeleton:** every cross-process contract in full, and code outlines for Milestones A and B only. C and D are
  outlined when they start (0.15).
- **(b) Experiments:** same-task sequential A/B stays, with effect sizes by default. Verdicts only from a
  randomized `crossover` with at least 8 pairs, with a time and cost estimate in the preview. `collide bench` is the
  cheap way to a verdict (0.14).
- **(c) Observers** stay. Orchard's `agents.jsonl` registry is the first one, and observers see only their own
  lines' runs (0.4).
- **(d) Metrics:** device GPU and network stay. Per-process GPU waits for the first local-model adapter (0.13).
- **(e) Default line root:** it stays the user's checkout. Writes wait while it is busy, and the Queue says why. A
  Collide-owned root is an option (0.12).
- **(f) Process:** the plan is frozen after this revision. Spikes S1, S3 and S4 run before any further review round.

The owner also asked for settings shared by every developer: `.collide/config.yaml` in the repo, whose values win
over the user's file. The Collide directory can live elsewhere, through a `.collide` pointer file (0.7). Open
question 7 asks whether a developer may override a shared key on their own machine.

**Revision 10, 2026-11-13 (spikes S1, S3, S4; owner).** Decision records in `docs/decisions/`, research notes in
`docs/research/`. The owner took every recommendation.
- **Zero-pack first run:** the built-in `collide` pack, scratch tasks, New session, no-setup open, and single-run
  accept moved into the run UI stage (0.4, 0.10, 0.11).
- **S1:** the launch line with `--permission-prompt-tool stdio` and `--verbose`; settings from the snapshot; auto
  memory off; scheduling, worktree and messaging tools disallowed; the acknowledgement narrowed; the limit signal
  (0.3). Raw `claude -p` stays, pinned by the contract test, and the adapter moves onto the Agent SDK only if the
  flag breaks.
- **S3:** `collide-daemon` with a per-child shim. `--bg` dropped. The shim holds stdin and the PTY master, measured
  in S2. The daemon starts on demand. Platform-binary orphans need hook records and the cleanup hook, so the
  headless-core stage's zero-survivor criterion is narrowed (0.2, 0.5).
- **S4:** the pack lock is the git tree id; files are extracted from git objects; the SDK is linked from a versioned
  cache; the environment is captured from the login shell (0.2, 0.6, 0.7).
- **herdr research:** the daemon leaves the app's coalition through a one-shot launchd job; a lost shim makes its
  phase `lost`; the marker scan adds descendants and a start clock; runtime marks; durability rules; herdr-core's
  hostile-process tests. Depending on herdr-core: no, for new reasons (0.2, 0.3, 0.5, the headless-core stage).
- **Metrics with no pack:** shim exit totals and disk always recorded; the subcommand kept in command shapes; an
  opt-in past-session import; OpenTelemetry for interactive sessions if S2 confirms it (0.13, the run UI stage).
- **Tokens and context:** always-recorded context metrics with five detectors; the cache countdown in the Queue;
  `on_cold`; context options per role; user tooling listed in the preview; a token strategy experiment preset; spike
  S9 (0.3, 0.7, 0.11, 0.13, 0.14).
- Not taken: depending on optimaizr (a Node tool for interactive sessions; it keeps working on Collide's
  transcripts), and in-session guards in core (deferred to packs).
- **Pack sources and profiles:** packs from the repo, any git URL or a `dev:` folder, each pinned to a git tree id;
  repo packs and user packs, with no silent replacement of a repo pack; switching per board, `pack update` with a
  diff, and arms that override a pack; profiles per kind of job, chosen in the preview, by the view or by a rule.
  Open question 7 decided as (b) (0.6, 0.7, 0.9, 0.11–0.14, the pack-loading stage).
- **No publishing needed** (part D2): a pack built for a team or a repo is used from that repo. `repo:` packs need no
  lock entry, so a commit is enough and the trust prompt shows the diff. Working copies run uncommitted packs from
  the user's checkout, and an integrate that changes one needs an acknowledgement before its next run (0.7, 0.9,
  0.11, the pack-loading stage, the run UI stage).
- **Ferry, a second example solution** (`docs/research/ferry.md`): `agentWork` script phases report usage and
  limits; the `unverified` result; `show:` on human steps; `collide/review`'s convergence stop; `protectedPaths` at
  phase end (0.1, 0.3, 0.4, 0.5, 0.7, 0.9).
- Not taken: package-manager sources and a pack index (deferred); a fan-out sized at runtime, batched end-of-run
  decisions and a replay cache in core (deferred until a non-example pack needs one).
- **S2** (part E): the shim's stdin, PTY and output buffer measured across daemon restarts; a lost shim's child
  stopped; the launchd start kept as insurance, with absolute paths and an explicit environment; take-overs detach
  on close and end on exit or End take-over, with a soft budget from telemetry and the session id followed across
  `/clear`; Claude's trust prompt left to the user, under one worktree root per repo; telemetry for interactive
  sessions and `agentWork` phases, with short intervals, a run token and identity dropped on arrival (0.2, 0.3,
  0.4, 0.9, 0.11, 0.13, the headless-core stage, the run UI stage).
- **Menu bar** (owner): the app stays in the menu bar with its window closed and its webview destroyed (53 MiB,
  against 143 MiB with the window hidden; the S2 record's addendum). It notifies in its own words and restarts a
  dead daemon. No approvals from the menu. Open at login and a separate helper are deferred (0.2, 0.9, 0.11, the
  run UI stage).
- **S9** (part E): input and cache tokens counted once per response, per-turn totals from `result.usage` and a
  compaction's cost from `model_usage_since_last_result`; compactions from `compact_boundary`, "after" from the next
  response; `compact_at` refused under 100K; `tool_output_max` split into `bash_output_max` and `mcp_output_max`;
  measured start weights; hook-blocked prompts read from `prompt_submit_outcome`; cold-resume costs shown with
  `resume` kept as the default; `config: none` keeps `--strict-mcp-config` (a spot-check of S9's runs) (0.3, 0.7,
  0.13, 0.14, the spikes stage, the headless-core stage).
- **Ferry's runner** (owner, part F): model time per turn from `duration_api_ms`, a running total taken as
  differences; a metric its source did not give is stored as null and shown as not reported, never 0 (0.13, the
  headless-core stage).
- **Ferry's runner** (owner, part G): `is_error` checked on `result/success`; spend read from error results and other
  model calls; a phase settles after exit and end of output; one cache hit-rate formula, with reads and writes
  never merged; calls, errors and denials per tool name; the tool list in the fingerprint; a read budget as a
  pack-supplied arm; changed-file lists collapse dependency and build folders but never hide lock files (0.3, 0.5,
  0.11, 0.13, 0.14, the headless-core stage, the run UI stage).
- **S9 addendum** (owner, part H): `--include-hook-events` on the launch line; refused tool calls classified from
  `tool_result_meta`, with `permission_denials` never shown as refused prompts; a hook `ask` as an ordinary request;
  hook failures fail open and are read from `hook_response`; a pack's Claude hooks block with exit 2; the record
  keeps hook outcomes, never hook output; two detectors (a hook that failed open, a call to a missing tool), with
  denies and blocks as counts (0.3, 0.13, the spikes stage, the headless-core stage, the run UI stage).
- **Planning** (owner, part I; `docs/research/planning.md`): planning is the built-in `collide/plan` pack; core
  gains kinds, op effects, links, `owners`, a checklist field, five task points and derived points; a markdown
  provider with private and shared roots at a path set per repo; Share as a move op; fan-out instances and a run
  budget; spike S10. The owner took the note's recommendations on its eleven questions: one planning source with a
  mapping and an opt-in badged fallback; the mapping in `collide/plan`'s settings with provider tags as defaults;
  question items and a `wait` phase for several decisions; the checklist field and drawer in core; spend off in
  shared summaries unless the board opts in; random ids with an alias; S10; Collide's records under
  `docs/decisions/` and `spikes/`; derived points; the path in `.collide/config.yaml` with Move and `previous`; no
  points for changes made outside Collide (0.1, 0.4, 0.5, 0.7, 0.8, 0.10, 0.11, 0.13, Deferred, the spikes,
  skeleton, pack-loading, task-board, engine, run UI, group-runs and second-pack stages).
- **S11 and the interface mockups** (owner, part J;
  `.collide/planning/spikes/sp-0008-bk-derived-point-coverage/record.md`): the point model is right for Collide's own
  writes, but the `change` payload was too thin. Ten core gaps are closed, G1 (writes a
  run's agent makes through the backend) the largest. Owner's decision on G1 (2026-11-13): S11's recommendation, widened:
  - the skeleton reserves `change.via` and `actor.kind: agent` with `run` and `phase`;
  - ref-scoped sources fire from the run's task diff at accept, before `integrate`: the logic and a fake-driven test
    in the workflow-engine stage, the real accept-path test in the run UI stage;
  - global sources get attribution and phase-end gate checks in the second-pack stage, for beads (exact, by
    `BEADS_ACTOR`) and for GitHub issues (attributed from the phase's tool calls, never exact).

  Also applied: accept checks `done` gates before the merge; PRs Collide opens carry no closing keywords; beads
  packs set `events-journal: true`. `docs/research/planning.md` is corrected on the plan CLI's writers and write gate.
  The mockups put the Field at home, the Queue as a list with a detail pane, and Tracks on a real-time axis. S6
  keeps five open questions from them (0.4, 0.5, 0.10, 0.11, Deferred, the spikes, skeleton, workflow-engine, run UI,
  group-runs and second-pack stages).
- **Agent output and tool timing** (owner, part K): selecting a track or a task shows the run's rendered output, and a
  track or phase breaks its time down by tool, filtered by tool type, minimum duration (how long one call took) and
  time window (a span of clock time). The CLI reads one item as JSON. Read access to the corpus for hooks and agents,
  and a GitHub App identity for agents, are deferred with triggers (0.11, 0.13, Deferred, the spikes, corpus and run
  UI stages).
- **Planning first, Queue context and the time window** (owner, part L): Collide is built with Collide. Planning is
  designed and built first: its contracts lead the skeleton, and the skeleton stage ends with a working planning
  slice (the markdown provider's shared root, kinds `task` and `epic`, `blocked-by` links and a ready rule, and the
  task CLI). Collide's build tasks move out of this plan into `.collide/planning/`, written by hand until the slice
  reads them; runs and packs follow at the headless-core and run UI stages. A Queue item opens a context view that
  Collide assembles with no model call, the task's context included; asking the agent is deferred. The time window
  matches calls that overlap it, and totals count only the time inside it. After revision 11, an adversarial review
  (R5) runs before any product code is written (0.1, 0.10, 0.11, Deferred, the spikes, skeleton, headless-core,
  corpus and run UI stages).
- **Planning layout and ids** (owner, part M): the user found the flat folder hard to browse and proposed a tree.
  A planning session is renamed a proposal (`pr-`). A proposal holds its decisions, `deferred` is one of its
  statuses, and a question holds its own decision, so decision and deferred files go. Items live in
  `epics/<epic>/{epic.md, proposals/, questions/, tasks/}`, and the folder replaces `part-of`. Spikes are top-level
  folders, because one spike informs several epics. Sequential numbers alone would need a renumber after every
  merge, as the plan CLI's do. Each id is a kind prefix, the next free number across all refs, and a two-letter suffix
  (`ts-0012-ua`). The suffix keeps ids unique, so a merge never rewrites a link, and the number stays the short
  handle. Collide's 28 items were moved with the ready set unchanged. Collide's eight spike records and their
  scripts moved from `docs/decisions/` and `spikes/` into `.collide/planning/spikes/`, one folder each, with
  frontmatter added; the provider reads only each folder's `record.md` (0.1, 0.4, 0.10, 0.13, the spikes, skeleton,
  corpus, task-board, run UI, Orchard-loop and second-pack stages).
- **Planning with an agent** (owner, part N): the user asked how they talk to the agent while planning. The plan
  had a spike workflow and a Queue lane, but no way to plan in conversation. The owner chose an interactive run, a
  New session on a new private proposal started with "Plan…", over a headless workflow that asks only through the
  Queue. The headless workflow's rule is kept inside the run: a choice to make away from the terminal becomes a
  question item in "Decisions needed". The conversation stays in the run's terminal, so Collide is still not a
  chat app. Proposal statuses merge planning.md's session and decision tables. `proposed` replaces the `open`
  first suggested, because 0.10 already uses "open" for not `done` or `dropped`. Only a person accepts, defers,
  rejects or resolves, as only a person marks a checklist item (0.10, 0.11, run UI stage).

**Revision 11, 2026-11-14 (spikes S5–S8; owner).** The four records are applied with the owner's decisions of
2026-11-14: `sp-0005-fv-schema-expressions`, `sp-0006-kq-agentadapter-acp`, `sp-0009-bb-interface-prototype` and
`sp-0010-bn-metrics-sources`. Every recommendation and proposed change has a row in the ledger at the end of
`.collide/planning/epics/ep-001-ef-spikes/tasks/ts-0005-st-apply-s5-s8-to-the-plan.md`, with its disposition, where it
landed and, for each rejection or deferral, the reason or trigger.
- **Changed:** the revision summary; 0.2 (shim logs stdin); 0.3 (two traits, session start, permissions, usage,
  `TurnEnd`, capability table, budgets, ACP rules, `model.id`, later adapters); 0.5 (report caps, poll); 0.6 (schema
  pipeline); 0.7 (YAML loader); 0.9 (untrusted YAML, CEL limits); 0.10 (fields, ids, frontmatter); 0.11 (review ×2
  as two reviewers, literal keys, context view as a data contract, tool-call breakdown, lines switcher, S6 closing
  paragraph); 0.13 (spans, sleep, sampling, `nettop`, GPU, containers, telemetry, volume); 0.15 (chosen crates and
  packages, component tree); Stages 1, 2, 3, 4, 6, 9, 11, 13 and 15; Deferred; Risks.
- **Owner's decisions:** S6's sessions skipped, so every non-owner participant criterion is a ported UI test plus an
  owner walkthrough recorded as owner judgement, and S6's thresholds are advisory; S6's usability gate withdrawn;
  S6 defaults `id`, `literal`, `moment`, `top`; "review ×2" is two reviewers in parallel; CEL vendored with the step
  counter and offered upstream; `schemars` then `json-schema-to-typescript`; S7 P1–P9 and changes 1–13; S8's
  extras (lane containers always counted through the Docker API, the three breakdown choices, honest sampling
  wording).
- **Rejected:** S5's id rule `^[a-z]{2,}-[0-9a-z]{4,}$` (the owner's rule of 2026-11-13 stands), and S6's usability gate.
- **Owner's decision after the draft:** the device GPU is polled only when a local model is declared (S8 rec 5;
  0.13, Stage 13, Open questions item 8).
- **Deferred:** S8's sleep mid-run test and a run measured from its start, before Stage 13 starts.

**Revision 12, 2026-11-14 (S6's in-app agent; owner).** The in-app agent section of
`sp-0009-bb-interface-prototype` is applied with the owner's design and the owner's decisions of 2026-11-14 (the section
"Owner's decisions (2026-11-14)" in that record repeats the four decisions).
- **Changed:** the status line and the revision summary; the intro (the ask box takes more than Claude Code); 0.2 (the
  token bullet, the UI-tools endpoint); 0.3 (the ask agent as a role, its launch line, the ACP profile, budget and
  timeout, and the later-adapters note); 0.7 (the repo file's per-user keys, agent entries); 0.9 (an agent's tool
  inputs); 0.11 (the ask box, tools, views and lenses, chips, the flag, what S6 measured, and one sentence on talking
  to an agent); 0.15 and Stage 2's reducer sentence; Stage 1 (S6's addendum and S7's unmeasured list); Stage 2
  (contracts and criteria); Stage 3 (the ACP sentence); Stages 4, 6, 9 and 11; Deferred; Open questions (item 9);
  Risks.
- **Owner's design** (applied): an agent in the UI that changes the UI through tool calls; generic tools and a UI that
  knows no code files; the agent does its analysis with read-only tools; bring your own agent (Claude Code, an ACP
  agent, an external MCP client) from a Settings page; grouping tasks to run together is an option, and sorting and
  filtering Tracks by seconds in a phase. 0.11 holds the design, 0.7 the settings, Stages 6, 9 and 11 the work. Not cut
  on any reviewer-style ground.
- **Owner's decisions 1 to 4** (applied):
  1. The ask box's default agent is Claude Code with haiku; ACP agents and External clients are selectable on the
     Agents page (0.3, 0.7). An External entry cannot be the ask agent, because Collide launches nothing for it. It is
     there for its snippets and its last-seen record.
  2. A token from day one (0.2, 0.7): the endpoint token for external clients, shown and rotated in Settings, and an
     ask token per ask for what Collide launches.
  3. The strict launch line with a per-agent opt-in "use my Claude config" (0.3). The default is `config: none`.
  4. A per-ask budget, optional and off by default, Claude only, and a timeout for every agent (0.3, 0.7).
- **S6's proposed plan changes** (record, "Proposed plan changes"):
  1. Typed reducer actions with a snapshot Undo: applied (0.11, 0.15, Stage 2, Stage 6).
  2. Views and lenses kept apart: applied (0.11, Stages 6 and 9).
  3. The "no files were read" flag: applied, computed by Collide from the ask's stream and not by the prompt (0.11,
     Stage 6).
  4. Reject invented ids and let the chip state what changed: applied (0.9, 0.11).
  5. A per-session token: the owner's decision 2.
  6. The budget as a promise: the owner's decision 4.
  7. A test action and the layers as acceptance: applied (0.7, Stages 4 and 6).
- **S6's list for a real Settings page:** layers and a visible source (applied, 0.7, Stage 4); secrets by name from the
  keychain (applied); permissions as data, in words (applied); connection state, the test action, last use and spend
  this week (applied, 0.7, Stage 6); endpoint security (applied, 0.2); External snippets and a record of which clients
  connected (applied, 0.7). Packs and profiles bringing agent entries, and a diff before save with an undo of
  settings changes: deferred with triggers (Deferred).
- **Changed from the prototype:**
  - the endpoint moves from a Vite plugin and an HMR websocket to the app process (0.2);
  - the ask line drops `--disable-slash-commands`, which 0.3 never passes (S9: 22.0K), and the per-ask budget of $0.25
    becomes off by default;
  - the token no longer travels in argv: Claude reads a file of mode 0600, and an ACP agent gets it in `session/new`;
  - the prototype's one flat file becomes layered settings, and its single folder becomes `read` and `tools` as data
    (0.7). Arguments are a list, not a string;
  - `config: none` is the ask default, where a run's default is `config: user`. The owner's words (settings from the
    snapshot, not the user's CLAUDE.md, hooks or memory) describe `config: none` (0.3).
- **Rejected:** none. The owner's design and decisions are applied as given.
- **Deferred** (Deferred): tools that act, with the trigger that a person has launched lens groups by hand; agent
  entries supplied by packs, and a profile per kind of work; settings diff and undo; and what S6 left unmeasured (a
  hosted opencode model, a second local model, the ACP permission path under a real write, a background window, a
  lens that shows its first group early, other MCP clients).
- **For R5 to check:**
  - the endpoint in the shell with the MCP layer in the window, against the alternatives in 0.2, and the three
    `ui_tools.*` methods that the shell handles itself and does not forward;
  - the two kinds of token, and the endpoint token reaching the webview on a user's Reveal;
  - the ACP read rule that fails closed on a request that reports no path;
  - the repo file's two allowed agent keys, and `timeout_s` as `overridable`.
- **For the owner to confirm:** a Tracks view saved as a personal view (0.10 has views of tasks only), and a view
  schema that gains `layout.sort`; the cap of 20 screen-changing calls per ask; the default timeout of 180 s, the
  value S6 used; and the choice of `config: none` as the ask default.

**R5 and revision 13, 2026-11-14 (owner).** Two adversarial reviewers on revision 12: Fable (25 findings, 5
challenges) and Opus (20 findings, 5 challenges). Both reached the verdict "ready after the fixes". Neither found a
case for a redesign. Both found Stage 2's contracts incomplete (replay, `wait`, the actor, `integrate` for PRs,
intent and outcome, the UI-tools request) and the largest stages too big. Every recommendation and challenge has a row
in the ledger at the end of
`.collide/planning/epics/ep-001-ef-spikes/tasks/ts-0006-ze-adversarial-review-of-revisions-10-12.md`, with its
disposition, where it landed and, for each rejection or deferral, the reason or trigger.
- **Changed:**
  - the status line and the revision summary;
  - 0.1 (the same-user rule), 0.2 (the shim and journal, daemon start, the socket and "Who is asking", the clock table,
    durability, the OTLP receiver, intent and outcome events, the UI-tools endpoint), 0.3 (`AgentSession`, Claude's
    version range, smoke check and canary, the ask line, opencode's probe and listener, ACP paths and models), 0.4
    (the accept row, `integrating`, built-in packs compiled in), 0.5 (`pending`, hook-own report lines), 0.6, 0.7
    (layers, `wait`, agent entries, interactive-launch MCP rule, `git:` fetch rules), 0.8, 0.9 (protected files, list
    caps, the webview), 0.10 (the `spike` kind, ids, atomic writes, the slice, build-task rules), 0.11 (`query`, Plan…,
    ask failures, the drawer), 0.13, 0.14 and 0.15;
  - Milestone A;
  - Stages 1 and 2 (contracts, slice, criteria, Status);
  - Stage 3, now 3a, 3b, the new 3c and 3d; Stage 4, now 4a, 4b and 4c; Stage 5 (Issues only); Stage 6, now 6a, 6b
    and the new 6c; Stages 7, 8, 11, 12, 13 and 15; Stage 9, now 9a, 9b, 9c and 9d; the Status line of every stage;
  - Deferred, Open questions (item 4) and Risks;
  - the four spike records' `status:` line, and the R5 task's file name.
- **Owner's decisions** (applied):
  1. Every accepted item of the merged list: M1–M10 (Stage 2's contracts), B1–B23 (the stage that builds each) and S1
     (the split). Items for Stage 2 are in 0.x and in its contracts and criteria.
  2. **The endpoint stays in the app shell.** The shell answers `initialize`, `ping` and `tools/list` from a
     build-time manifest, returns 202 to notifications and 405 to GET and DELETE, forwards only `tools/call` as
     `ui_tools.request {requestId, caller, deadline, message}`, counts the per-ask cap of 20, fails `tools/call`
     with an "open Collide" message when the window is closed, and cancels asks when the window closes or reloads.
     Stage 6b tests a window closed when the client starts. The stdio bridge (`collide mcp`) and a daemon-hosted
     endpoint were considered and not chosen (0.2).
  3. **A thin loop, Stage 3c,** after Stage 3b: prompt, `collide/quick` run, review, accept, in the app, against
     scratch tasks, before the plan adapter, GitHub and the board. It names what it pulls forward from Stages 4b, 6,
     7, 8 and 9a, and its cost: the launch and accept paths are written twice and replaced in 9a. Nothing is cut.
     Milestone A and the first-dogfooding wording are updated.
  4. **GitHub Projects v2 moved after the board.** Stage 5 keeps Issues over REST. Stage 6c holds the GraphQL items and
     fields, the `project` scope check and the poll-cost measurement. Stage 2 still freezes the full provider
     contract, Projects keys included.
  5. **Stages 3, 4, 6 and 9 split into lettered sub-stages** with no renumbering, each with its own goal, criteria and
     Status (Fable F8 and Opus F10, merged). Stage 4's agent-secrets "at launch" test moved to Stage 6b, which is
     where an agent launches.
  6. **Rejected:** a three-letter id suffix (the owner's rule of 2026-11-13 stands); the markdown provider in Rust
     (Stage 2 must prove the TypeScript provider path, the extension model's main risk); named per-client endpoint
     tokens (one rotatable token is kept).
  7. **Deferred:** a Collide-managed copy of Claude (Opus C5). Trigger: a Claude update breaks a run despite the
     version-range check and smoke check.
  8. **`--restricted` and `--bare`** (Fable challenge 4) are handled by B3: Stage 6b measures `--restricted` under `-p`
     and the adapter adopts it if it confines reads. `--bare` is not adopted, because it changes how the login is read.
  9. **Builders are Sonnet agents** (0.15). Each Stage 2+ item is sized for one such agent and one session.
- **Contradictions fixed** (B22):
  - detectors: seven, as 0.13 lists them. Deferred said "the first five" and now says seven;
  - layers: four (schema defaults, the user's file, the repo's file, the run's profile). Stage 4's settings bullet said
    three and now says four;
  - `owners`, not `assignee`: 0.10 and the item files use `owners`, and 0.8 meant the same claim, so 0.8 and the
    revision 10 summary now say `owners`.
- **Spike kind** (M4): 0.10 defines `spike` with the statuses `planned` (ready), `running` (active), `results`
  (review), `concluded` (done) and `dropped` (dropped), taken from the records. `sp-0005-fv-schema-expressions`,
  `sp-0006-kq-agentadapter-acp`, `sp-0009-bb-interface-prototype` and `sp-0010-bn-metrics-sources` are now
  `concluded`. The slice criterion is narrowed, not the records made read-only items: the slice reads `epics/` only,
  and Stage 4c adds `spikes/`, because a spike record's statuses and shape are not a task's.
- **Status-line rule** (M4): each stage's Status names the plan revision its items came from, and only the next stage
  is split into items. Stage 2's items were written against revision 12 and are to be brought to revision 13 before
  they start.
- **Where an item's placement differs from the first sketch:** the hostile-issue webview test is in Stage 6a, where the
  drawer is built, not 6b.
- **Rejected:** a three-letter id suffix, the markdown provider in Rust, and named per-client tokens (above).
- **Deferred:** a Collide-managed copy of Claude (above).
- **For the next review to check:** the replay journal's byte-order guarantee under a daemon crash; the shell-side MCP
  manifest against a client that sends `tools/list_changed`; that 3c's hard-coded sequence is replaced and not
  extended in 9a; and the id-collision redraw against a fetched remote.

**Revision 14, 2026-11-14 (owner).** Features in proposals. The owner's request: planning is about features, and
Collide had none until the tasks existed. The Planning view should group the feature set into sections (packs, task
view, planning view, daemon, …) that the agent names, and a planning tool should take a section name, which may be
empty at first. Asked whether features could go into a proposal ("in Orchard we put feature sets in proposals"), the
plan puts them there.
- **Changed:** the status line and the revision summary; 0.8 (a proposal's `features` and `featureProblems`, a link
  to a feature); 0.10 (the provider's `links` capability, a proposal's features, sections, `implements`, the derived
  state, GitHub's mapping); 0.11 (Plan…'s naming, the Planning view's Features and Sort); Stage 2 (planning contracts,
  the slice, criteria, tests, Status); Stages 4c, 6a and 9d.
- **Owner's decisions** (applied; "1-4 all your recs"):
  1. **A fixed Features part that Collide reads,** not free markdown that the view only renders: `###` sections,
     entries `- **F<n> Title**: …`, `### Unsorted` or no heading for an empty section, and the move to `proposed`
     refused while the part has a problem. The op that adds a feature takes `section` as required and possibly
     empty.
  2. **Tasks link to features** with a target that carries the feature, `pr-0003-km#F2`, and the view shows each
     feature's derived state.
  3. **Sections match by heading name across proposals,** with no registry and no feature kind. `collide sections`
     lists the names, the Plan… prompt reuses them first, the view offers to merge near-duplicates, and Sort has the
     ask agent suggest sections as chips with Undo. On GitHub a section is an `area:` label or a Projects v2
     single-select field.
  4. **Stage 2 now** for the contracts and the slice's part. The ops are Stage 4c's, and the view's parts are Stages
     6a's and 9d's.
- **Considered and not taken:** free markdown (no per-feature state, and nothing to link to); a feature kind with a
  file per feature (a feature set reads best whole, in its proposal, and it would add more files than there are
  items); a section registry file (one more thing to keep in step, when a heading already names the section).
- **Choices made in writing it** (the owner may change any): the link type `implements`; the derived states' names;
  the near-duplicate rule; the 40-character section name limit; `featureProblems` on the item for a draft rather
  than a `task.invalid`; the op names `addFeature`, `moveFeature` and `renameSection`; Sort answering through
  `show_groups`, so the five tools stay five; the slice reading the `proposal` kind now, with its transitions, the
  person-only rule and `by:` in Stage 4c; and the GitHub board key added in Stage 6c as an optional key.

**Revision 15, 2026-11-14 (owner).** Two findings from the tokensafe research. The owner asked whether anything from
tokensafe (an MIT token-saving tool: an MCP server, agent hooks and an optional local proxy) belongs in Collide. A
Sonnet agent read it without running it, and the note is `docs/research/tokensafe.md`. The owner took findings 1 and
2 ("yes add 1 & 2").
- **Changed:** the status line and the revision summary; 0.3 (an `ANTHROPIC_BASE_URL` is kept and flagged; the
  preview flags hooks by event, including those that add context or replace a tool's result); 0.5 (a claimed saving
  is an estimate); 0.11 (the run preview); 0.13 (the fingerprint records the endpoint's host); Stage 3b (scope and a
  criterion); Stage 7 (a test); Stage 9a (the Field's preview).
- **Owner's decisions:** 1. flag a redirected model endpoint and context-changing hooks, and record the endpoint's
  host; 2. a reported saving is an estimate.
- **Considered and not taken** (in the note): tokensafe as a dependency; its code outliner, import graph, proxy,
  prompt compaction and cache normalizer (Collide owns the launch, not the API request). Kept for later, each behind
  its cheap test in the note: a tokensafe-on-versus-off experiment arm at Stage 15, a guard pack of deny rules, a
  run-once test environment, and a detector for whole-file rewrites.
- **Choices made in writing it** (the owner may change any): the endpoint is flagged, not stripped, so a gateway the
  user chose keeps working; hooks are flagged by the event they hook, because their output is unknown before they
  run; only the host is recorded, never the path, query or a key; a claimed saving needs no new report field, and is
  told apart by docs and labels.

**Revision 16, 2026-11-15 (owner).** Claude's versions as chips. The owner's proposal: "instead of a queue warning
you could have a chip/label that is the version that is being used, and a chip/label that is the published Claude
version."
- **Changed:** the status line and the revision summary; 0.3 (a pass of the smoke check raises no Queue item; the
  two chips and the published-version fetch); 0.11 (the run preview); Stage 3b (scope and a test); Stage 3c (the
  window); Stage 6b (the Agents page); Stage 9a (the Field's preview).
- **Kept:** the tested range, the smoke check, the refusal when it fails, and the version in the fingerprint.
- **Also (owner, 2026-11-15):** a person's change to the view ends the Undo of the agent's change before it (0.11),
  the owner's answer to ts-0042's decision 3.
- **Choices made in writing it** (the owner may change any): the published version is npm's `latest` dist-tag, with
  `stable` in the tooltip, because Claude Code's two update channels carry the same names (inferred); the fetch is daily and at start, carries
  no user data, and can be turned off per user; a version outside the range marks its chip rather than adding a
  third chip; an installed version behind `latest` gets no mark, since the two chips side by side show it.

**Revision 17, 2026-11-15 (owner).** The UI library and the interface's audience. The owner compared MUI with shadcn/ui
on the Tracks mockup (C, versions 6 and 7) and chose shadcn/ui, with a condition: "the look and feel must feel more
approachable to those who aren't engineers. It looks too engineering focused now. It indexes too far into the IDE look
and feel. Since we are making a higher level development environment, it should still be approachable by product
managers, while still having the advanced features an engineer would expect."
- **Changed:** the status line and the revision summary; 0.11 (who it is for); 0.15 (shadcn/ui among the chosen
  packages; MUI not used).
- **Next:** the mockups get an approachable pass, Tracks first, before the app's theme is set; the component tree
  (`ts-0043-fr`) renders placeholders, so it does not wait for it.

**Revision 18, 2026-11-15 (owner).** Planning quality checks, from the ideas on the Field mockup (A). The owner asked
for more description of the ideas, then: "For Mockup A's decision, I Agree with all of your recs".
- **Changed:** the status line and the revision summary; 0.8 (a proposal's `decisions`, `assumptions`, `findings`,
  `partProblems` and `coverage`); 0.10 (the quality parts, coverage, and a spike's Pre-registration part with its
  kill criterion); 0.11 (Plan…'s panel, the Readiness checklist, the red-team pass with its pre-mortem mode, the
  budget meter); Stages 4c, 9d and 15.
- **Owner's decisions:** adopt the coverage map; the Decision log, with the alternatives passed on and the evidence,
  and confidence optional; the kill criterion; the Assumption ledger, built after the Decision log; the red-team
  pass, in the planning pack, with the pre-mortem as its second mode; and the budget meter, a soft cap that warns at
  85%, never stops an interactive session and breaks the spend down by part. The Readiness checklist stays advice:
  Collide refuses `proposed` only for a problem in the Features part. The panel of agents waits for Stage 15.
- **Choices made:** the parts reuse the Features part's entry form, and their problems never refuse a move;
  confidence is the person's, never derived from the evidence; the red-team agent writes nothing and the session's
  agent records its findings, so no new op is needed; the panel's buttons type into the terminal; the meter's cap
  is the phase budget the take-over rule already warns at, and time is shown, not capped. Stage 2 is unchanged.

**Revision 19, 2026-11-15 (owner).** Tab names. The owner compared the names on the Tracks mockup (C, version 9),
which shows each tab with a subtitle, and kept them: "we can keep these with the subtitles".
- **Changed:** the status line; 0.11 (the tab names and their subtitles).
- **Choices made:** the subtitles are the mockup's words; Planning is the `collide/plan` pack's Planning view, so
  its tab is there when that pack is enabled.

**Revision 20, 2026-11-15 (owner).** Four answers in one message.
- **Tracks:** "We have 2 different views for tracks: the straight view, with each track in a list with the stages
  broken out, and a more detailed timeline view. I think we should keep both views, togglable from inside the Tracks
  view." Changed: 0.11 (List and Timeline).
- **The free-space floor:** 5 GiB stays the default and becomes the per-machine setting `disk.floor_gib`. Changed:
  0.2 and Stage 4a.
- **Repo-only keys:** the mirror of the per-user keys; project settings win over a person's, and some exist only in
  the repo. Changed: 0.7 and Stage 4a.
- **File locations:** the daemon's socket, token and lock move from `~/.collide/run` into
  `~/Library/Application Support/Collide/run`, because `~/.collide` reads like a repo's `.collide/`. Changed: 0.2.
  The code moves in `ts-0073-ac`, before the owner's checks in `ts-0071-fd`. Open question 2 is settled: Linux and
  Windows follow macOS.
- **Choices made:** List is the default layout (the audience rule: the engineer's detail is one step away); the
  toggle is remembered per person; both layouts keep the same order; the setting's name and unit.

**Revision 21, 2026-11-15 (owner).** Up next, groups, and a repository's own instructions.
- **Up next:** "Another feature we should consider supporting is queuing up tasks. The user selects the tasks they
  want, in the order they want, and the system should be able to queue them. We should also support queuing a group
  of tasks too." The owner chose the name Up next (the Queue tab keeps its meaning), that the next entry starts once
  the one before it has landed, that a group is one entry run as a group run, and that it ships in Stage 11.
  Changed: 0.11 (Tracks) and Stage 11.
- **Groups:** the applied packs decide how a group is treated; Collide sends the agent the right information, and
  the agent applies the rules it gets from the repository. Changed: Stage 11.
- **A repository's own instructions:** "Ideally, we work seamlessly with how the developers already work. Without
  using collide, agents can already change the CLAUDE.md file. Maybe we should provide a warning for when the
  CLAUDE.md file has changed as part of a task run." Changed: 0.3 (the tool's config paths, the acknowledgement and
  the context options) and Stage 4a.
- **Choices made:** a switch lets independent entries overlap; a failed, stopped or refused entry pauses the list
  and raises a Queue item; entries are reordered or removed until they start; the list shows in Tracks under the live
  runs in both layouts; `collide next` is the CLI; adding is the person's Launch, so the in-app agent has no tool to
  add; `group: per_task | whole` on a workflow; the group context goes in the brief and `COLLIDE_GROUP`. Settings,
  hooks, permissions, MCP servers and agent definitions still load from the snapshot, because they run code or grant
  access without the model asking (S1); `repo_instructions: none` for a role and for experiment arms. Which launch
  line does this is unmeasured, so Stage 4a measures it first.

**Revision 22, 2026-11-15 (owner).** Where agents start, worktrees an agent makes, how work lands, and one line
selector.
- **Worktrees an agent makes:** the owner asked how Collide tracks worktrees that a repository's own tools make
  (Orchard's `dev worktree`), and chose that Collide notices them, with nothing asked of the agent or the repository.
  Changed: 0.4, 0.7 (the example), 0.8 and Stage 8.
- **Where agents start:** "Ideally, Collide would be a layer that sits on top of the developer's workflows,
  providing all of this extra intelligence. Most developers spin up their agents from the repo root - but not
  always(?). It seems like we may need to provide settings for where orchestrating agents start from. This could be
  a setting within the pack, and the default should be the repo root." The root is the default for a workflow whose
  agents make their own worktrees (`agentWorktrees: true`); other workflows keep their agents in workspaces unless a
  role says otherwise. Changed: 0.3, 0.4, 0.7, 0.10, 0.12, Stage 4a and Stage 8.
- **How work lands:** the owner chose that each workflow decides, and: "what if the agent doesn't perform the merge
  and it just sits there? Collide should be able to flag this scenario and have the user take action." Changed: 0.4,
  0.12, Stage 8 and Stage 11.
- **One line selector:** "We currently have a code line selector in our UI. These code lines can be different
  repositories or different branches of the same repository (collide shouldn't care). But each of them has their own
  pack they load. We will need to support swapping out packs and views on the fly when this changes." Changed: 0.8,
  0.10, 0.11, 0.12, Stage 6a and Stage 14.
- **Lines on Tracks and the Queue:** "For the queue and the tracks, we should have toggles to view just that line, or
  to view several lines (a multiselect in that case?)." The owner chose one Up next list per repository. Changed:
  0.11, Stage 9b and Stage 11.
- **Coming back after a long close:** "Another aspect to consider is closing collide, and opening it several weeks
  later, when tasks have changed. Collide will need to clear/update its cache to reflect the current state of the
  world." Changed: 0.2, Stage 5, Stage 6a, Stage 8, Stage 9b and Stage 11.
- **Adversarial review** (Fable, three passes). The first found that Collide's own provisioning would be adopted by
  another run's read, that adopted work had no landing path, that a read-only phase in the person's checkout
  contradicted 0.10 and could not be checked, that the Orchard example had no label for the orchestrating session,
  and that "a line can be another repository" contradicted 0.12. The second found that `root` as every role's
  default would put other packs' agents in the person's checkout, and that a root agent's own commits bypass accept.
  The third found that the catch-up missed a daemon that stayed alive through sleep, held nothing while it ran, and
  read a check that could not finish as one that found nothing. Each is answered above; the real `session` run is in
  Stage 11.
- **Choices made:** attribution order (`identify`, the call that was running, working directories, a person), with
  the baseline kept in the store and Collide's own provisioning held out; an adopted worktree belongs to its task
  and is attached to that task's later runs; Collide removes one only when a person asks, through `release`; one
  root-started phase per line unless `rootPhases` says more; `root` refused for `readOnly` roles, sandboxes,
  experiment arms and untrusted authors' tasks; a write and its `record` are one step at a live root; a squash or a
  rebase counts as landed; the Lines control starts on all lines and never hides a count; eight snapshots stay
  loaded; the concurrency cap is the app's; after a long close, Up next is held when the world under it changed,
  not after a set time (a setting, off by default), and what changed notifies once.

**Revision 23, 2026-11-15 (owner).** Task statuses are data.
- **The ask:** "One thing to note about task statues - while we have a default for them with our own planning
  system, other projects may have different stages for their tasks. We should support dynamic statuses, and not hard
  code the status throughout the IDE."
- **What was there:** each source already declared its own statuses, each mapped to one of six categories, and
  behavior read the categories. Board columns grouped by category only, and nothing said what happens to a status no
  one mapped.
- **Owner's choices:** the six categories stay fixed, so one workflow works on any project's statuses. A view whose
  tasks share one status list shows one column per status, in the source's order; a mixed view shows the six
  categories. Changed: 0.10, 0.11, Stage 4b, Stage 6a and Stage 6c.
- **Choices made:** an unmapped status counts as `backlog` and is flagged with its fix; a status a source did not
  declare is an invalid item; scratch tasks declare their statuses like any source (applied in `ts-0075-hj`).
