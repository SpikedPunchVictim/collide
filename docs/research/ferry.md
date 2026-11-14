# Ferry as a Collide pack: a generality check of core

Date: 2026-11-13. Input to plan revision 10.

**Status:** the owner took the recommendations: C1 (report kinds, plus OpenTelemetry after S2), C2, C3, the
`collide/review` convergence stop, `protectedPaths` at phase end and the wider resource-token trigger. They are in
plan revision 10 part D. Ferry, like Orchard, is an example solution: it shapes Collide's feature set only, and no
Ferry pack is a deliverable (owner, 2026-11-13).

**Question.** Could Collide run Ferry's workflow as a pack, and what would that change in core? This is a check
of core's generality. Orchard is the reference pack.

**Triage rule.** A Ferry need changes Collide core only if another pack (Orchard, or a plain issue → PR → CI pack)
would plausibly hit it too. Otherwise it belongs in the pack, or it is a pack-level gap.

**Sources:**

- **Ferry**: its source tree. Read as data. Its code and tests were not run.
  - Documents: its README, architecture doc (§§0–13, §19), plugin implementation plan, and roadmap docs (the first part
    of the main roadmap, and the second roadmap).
  - Source: the contracts, orchestrator, CLI (pipeline, incremental build, update pipeline, composition root, verdict
    report, build command), runner bridge (protocol, Docker runner, code runner, config loader), schemas (decision
    packet, gate result), the gate runner, and parts of the phases and of the scaffold archetype.
  - Not opened beyond file listings: the project-projection bridge, the reconcile module, the library-vetting module,
    the archetypes (apart from the scaffold) and Ferry's runner image. Claims about them are labelled inferred.
- **The plan**: `IMPLEMENTATION_PLAN.md`, rev 10 in progress. It is cited by section name only.

**Labels.** Every claim carries one:

- **read**: seen in a file, which is named by role or heading;
- **inferred**: derived from what was read.

---

## 1. What Ferry does

**In one line.** Ferry compiles an architecture document into a TypeScript service that passes a gate suite. It is
an oclif CLI (read: its README).

**Commands:**

- `ferry build` is the greenfield path.
- `ferry update` reconciles a changed document against existing code. Its project projection uses a code-index tool, and
  it plans only the delta (read: the update pipeline).
- `build --incremental` builds one vertical slice at a time. Each slice is gated and checkpointed in
  `<build-dir>/increments.json` (read: the incremental build).

**Trigger.** A person runs the CLI with `-a <doc>`. Nothing else starts a run (read: the build command's
flags):

- There is no task source and no queue.
- Ferry's platform calls it as its build engine, adding `--policies build-inputs.json` (read: its architecture doc §19).

**Who runs the models.** Every LLM call is `claude -p` inside a Docker container (Ferry's runner image). The host and
the container talk through files: `.ferry/request.json` in, `.ferry/result.json` out (read: the runner bridge's protocol
and Docker runner). There are two modes:

- **structured**: one call that returns JSON matching a tool schema. It is used by the design phases. Its architecture
  doc §9.1 says the in-runner LLM does not call tools mid-turn.
- **code**: an agent edits the mounted workspace. It reports the changed files, library gaps, context gaps, usage
  and timing (read: the runner bridge's code runner).

**Model choice.** The endpoint and models come from its LLM config file, which holds the real Claude Code variables
(read: the runner bridge's config loader):

- `ANTHROPIC_AUTH_TOKEN`, `ANTHROPIC_BASE_URL`, and Bedrock or Vertex settings;
- `ANTHROPIC_MODEL` for the strong tier and `ANTHROPIC_SMALL_FAST_MODEL` for the cheap tier. The fallback is
  `claude-opus-4-8` (read: the composition root);
- its config directory holds profiles for Sonnet and for Kimi, GLM, Qwen, DeepSeek and MiniMax behind
  `ANTHROPIC_BASE_URL` (read: directory listing).

### The pipeline

Read: the pipeline and its core module, both in full.

| step | kind | input → output |
|---|---|---|
| archetype and pattern selection | LLM classifier plus a deterministic signal floor | doc → `selection.json` |
| package extraction, scaffold, control files | deterministic | doc → workspace skeleton, plus `.ferry/entry-expectations.json` and `.ferry/smoke-contract.json` |
| install | in-container `pnpm install` | — |
| `specify` | structured LLM | doc → spec (`spec.json`) |
| `decompose` | structured LLM | spec → module graph (`modules.json`) |
| validators | deterministic | frozen-entry imports, migration-runner obligation, script targets. Also generates a dependency-cruiser config |
| `provision` | LLM proposal, then vetting (npm and OSV), then `pnpm add` | → `libraries.json` |
| `contract` | code agent, one module per call, in dependency waves | → interfaces, one `contract <module>` commit per module |
| `plan` | structured LLM | → task graph (`plan.json`), the module-coverage check, `.ferry/ac-obligations.json` |
| `implement` | code agent per task, in waves | → code, one `task <id>` commit per task |
| `verify` | the gate suite | → `verification.json` |
| `refine` | code agent, at most 2 rounds | gate failures → fixes, one `refine <n>` commit per round |

**Retry and loop limits:**

- Every phase gets up to 3 attempts. A retry carries typed feedback (the attempt number, the prior output and the
  failures), which holds the failures only (read: the orchestrator's retry module and the gate result schema).
- An implement task runs a gap loop of at most 2 rounds (read: the pipeline core). It also has:
  - per-task token budgets;
  - quarantine of a task that writes nothing;
  - revert and quarantine of a write to a frozen path (read: the pipeline core);
  - a circuit breaker that aborts after 3 consecutive endpoint stalls (read: the pipeline core).
- Refine has at most 2 rounds (read: the pipeline core):
  - it runs only on gates whose status is `fail` (read: the pipeline core);
  - it stops early when a round does not reduce failures or goes over its budget, recording a refine-stalled item (read:
    the decision packet schema).

**Gates.** Gates run one at a time, in a fixed order (read: the gate runner and the composition root).

- The suite: type check, ESLint, dependency-cruiser, structure, declaration, environment-boundary, entry-point,
  depends-on (diagnostic), smoke, harness, Vitest, acceptance-criteria coverage, and a policy gate when `--policies` is
  given.
- Harness uses Testcontainers Postgres when Docker is present. Otherwise an app that needs a database gets an
  unsatisfiable harness result.

**Verdicts.** Each gate returns `pass`, `fail` or `unsatisfiable`. The build verdict follows (read: the gate result
schema):

| build verdict | exit code |
|---|---|
| `production-ready` | 0 |
| `not-production-ready` | 1 |
| `unsatisfiable` | 3 |

- A decision packet gives exit 2 instead (read: the verdict report and the build command).
- Its README's exit-code table lists only 0, 1 and 2.

**Human checkpoints.** Ferry never pauses for a person mid-run (inferred).

- **What happens instead.** The build runs to completion. Whatever it cannot settle goes into a quarantine and into
  `decision-packet.json` (read: the decision packet schema).
  - Item kinds: vetting escalation, task quarantine, install failure, contract change, uncovered acceptance criterion
    and refine stalled.
  - Each item has a subject, reasons and suggested options as free text.
- **How a person answers.** They read the packet and run again: with changed flags, with `--auto-resolve`, or with
  `--resume` for incremental builds.
  - `--auto-resolve` handles only vetting escalations and install failures, and only in the safe direction (read:
    its README).
- **Approvals.** Its architecture doc §12 designs three: after SpecIR, after InterfaceIR, and on a contract-breaking
  update.
  - They are not implemented. The build flags have no stop or approve option, and nothing in `cli/src` pauses for
    one (read: the build command's flag list; inferred from a grep).

**State.** All of it is files (read: its README, the composition root and the incremental build):

- **Per-run records** under `<build-dir>/<runId>/`: `spec.json`, `modules.json`, `plan.json`, `verification.json`,
  `build-report.json`, `report.md`, `task-telemetry.jsonl`, `task-ledger.md` and `decision-packet.json`.
- **A cross-run content-addressed cache** at `<build-dir>/cache`. It holds the design phases and `implement`, the latter
  with a workspace snapshot capped at 32 MB. `refine` is never cached (read: the phase runner, the implement and refine
  phases, and the orchestrator's phase runner).
- **Gitignored control files** in `.ferry/` inside the workspace.
- **Commits in the workspace.** The runner `git init`s it and commits a baseline before each agent call (read:
  the workspace-revert code's doc comment).

**Telemetry.** `BuildReport` holds (read: its architecture doc §4.7a and the pipeline core):

- tokens in, out and cached; wall time; the cache-hit ratio;
- per phase: tokens, wall time, API time, runner time, turns and tool calls;
- per task; gate rounds; the verdict.

There is no dollar cost.

**The planned event spine.** Its plugin implementation plan designs an observer spine of typed run events, emitted
only from orchestration code. Its status is "Not Started" (read).

**Scale.** One real run took 14.91 h: 86.0M input tokens, 0 of them cached, 68 phases, 21 tasks and 37 modules. It
ended `failed` (read: its roadmap appendix).

---

## 2. Mapping: Ferry → Collide

Fit is one of:

- **direct**: Collide already expresses it;
- **pack code**: expressible with hook or provider code, or with a change to Ferry itself;
- **gap**: not expressible today. The gap IDs refer to section 5.

| Ferry element | Collide construct | fit | evidence |
|---|---|---|---|
| Trigger: a person runs the CLI with a doc path | Launch from the board on a task, with a run preview (0.11 Interface direction) | pack code | read: the build command; plan 0.11 |
| Task source: none; the input is an architecture doc | A ref-scoped `ferry/docs` provider, one task per doc, with template field `docId` (0.10 Task sources). A changed doc moves its task back to `ready`, which is Ferry's `update` trigger. Scratch tasks also work, but prompt text is task text and never reaches a template, so a provider is cleaner | pack code | inferred, from plan 0.10 and 0.5 (template fields must be safe tokens; a path containing `/` is not one) |
| Pre-pipeline and design phases | One `script` phase, `design`, running `ferry build --stop-after contract`. That flag does not exist yet (P2) | pack code | read: plan 0.7 phase kinds; the build command's flags |
| implement → verify → refine loop | Stays inside Ferry, run by a second `script` phase, `build`. Because phases are cached, re-running replays `design` from cache | pack code | read: the phase runners |
| Roles and model tiers (strong and cheap) | Collide aliases `strong` and `fast` exist (0.3), but under the recommended option no Collide agent phase runs. Ferry picks models from its own LLM config profile, chosen by a `launch` parameter | pack code under (c); for (a), see section 5 | read: the config loader; plan 0.3 |
| Structured single-call LLM steps | No Collide construct. "Collide never calls a model API itself. Judgment steps are agent phases" (0.3). They stay inside Ferry | pack code under (c) | read: its architecture doc §9.1; plan 0.3 |
| Typed retry feedback (failures only, 3 attempts) | Inside Ferry. In Collide terms: a `round` re-entry with the gate output passed as an artifact prompt, as in Orchard's fix → brief edge | direct, if ever needed | read: the retry module; plan 0.7 `round` |
| Gate suite | A Collide gate `ferry/verify` on the `build` phase that runs `ferry verify` on the workspace. That command does not exist yet (P7), so today the gate reads `verification.json` | pack code | read: its command list holds only build, update and report |
| Three-state gate verdict (`unsatisfiable`) | The result status is `ok`, `fail`, `refused` or `error` (0.5). There is no "could not verify here" | **gap C2** | read: the gate result schema; plan 0.5 |
| Exit codes 0, 1, 2 and 3 | The script phase translates them into the result file and `outputs`. "A child tool's exit code is never forwarded as a verdict" (0.5) | direct | read: the verdict report; plan 0.5 |
| Decision packet (exit 2) | A decision script, `workflow/after-build.ts`, maps it to a `decide` `human` phase or to `ask_human` (0.7) | pack code; showing the packet is **gap C3**; per-item answers are P9 | read: the decision packet schema; plan 0.7 |
| §12 approvals after SpecIR and InterfaceIR, and on a contract break (designed, not built) | `human` phases between `design` and `build`. Collide gives Ferry these one-way doors | pack code, plus P2 and **C3** | read: its architecture doc §12; plan 0.7 |
| Token budgets (task, design, refine) | `launch` parameters turned into flags, like Orchard's `review_budget`. A pack's own budgets are "not counted by Collide" (Stage 8) | direct | read: the build command; plan Stage 8 |
| Collide's own budget, usage-limit pause and permission policy | Apply only to agent phases. Ferry's agents run inside a `script` phase on the user's login, so Collide sees none of it | **gap C1** | read: plan 0.3 "Usage and rate limits", 0.5 "Reporting" |
| Workspace (`--workspace-dir`; the runner `git init`s and commits) | A workspace implementation returning `{path, gitDir}` (0.1). The scaffold rewrites the root `.gitignore` and `package.json` (P12) | direct, with care | read: the scaffold archetype; the workspace-revert code's doc comment |
| Pre-flight (endpoint, Docker, runner image) | `workspace.after_create`, or the workspace `create` hook (0.4) | pack code | read: the build command's `--skip-preflight` flag; plan 0.4 |
| Containers (Ferry's runner, Testcontainers) | Stopping goes through the workspace `cleanup` hook, and measurement through `metrics.processes` (0.5, 0.13). Containers are named `ferry-runner-<uuid>` with no label, and killing the client leaves them running (P4) | pack code | read: the Docker runner; plan 0.5 "Stopping" |
| `record` | Ferry commits inside the workspace. Collide's `record` is needed only for a `ferry.lock.json` (doc hash → built commit) that the provider reads | pack code | inferred |
| Integration (none in Ferry) | `integrate: collide/merge-local` or `github/pull-request`, chosen per workflow (0.4). Squash merging is worth considering, because of the baseline commit made before each agent call | direct | read: plan 0.4; the workspace-revert code's doc comment |
| Planned run-event observer spine | Not a Collide observer, which reads Collide's own event log. It is a `COLLIDE_REPORT` reporter inside Ferry's orchestration code. The Ferry host process inherits `COLLIDE_REPORT`; its containers do not, which matches 0.5 "Who cannot report" | pack code (P6) | read: its plugin implementation plan (Invariants); the Docker runner; plan 0.5 |
| `build-report.json`, `task-telemetry.jsonl` | `metrics.import`, reading `<phase.dir>/ferry/<runId>/`. It is the fallback when live reporting is off (0.13) | pack code | read: plan 0.13 sources table |
| Token counts | Reported only. Never primary, and not in the run's token totals | **gap C1** | read: plan 0.13 "Levels", 0.14 "Comparison" |
| Cross-run cache `<build-dir>/cache` | The pack data directory (`COLLIDE_PACK_DATA`, 0.7). Needs a `--cache-dir` flag separate from the run records (P3) | pack code | read: plan 0.7 "Pack data" |
| Incremental resume state (`increments.json`) | The workspace's `.ferry/`, because a workspace outlives runs (0.8), and Collide's `round` re-entry continues the build | direct | read: the incremental build; plan 0.8 |
| Model and endpoint comparisons (Kimi, GLM, …) | Experiment arms that differ by the `launch.llmProfile` parameter (0.14). Time as the primary metric works today; tokens do not (C1) | pack code | read: plan 0.14 |
| Cache interference in experiments | Each arm gets its own copy of the pack data directory (0.7), so both start alike. A warm cache replays `implement` too, so `experiment.prepare` empties it unless caching is under test | pack code | read: the implement phase; inferred |
| `collide bench` | `ferry verify` on a fixed commit is a good bench step: repeatable, with no agent (needs P7) | pack code | read: plan 0.14 `collide bench` |
| Fingerprint | Collide does not know Ferry's version, the runner image digest or the LLM profile. The pack reports them as cache state (0.13 "Fingerprint") | pack code (P15) | read: plan 0.13 |
| `.ferry/` frozen control files | Ferry enforces them itself, by revert and quarantine. Collide's `protectedPaths` covers the architecture docs, which the hook reads from the run's snapshot commit rather than the workspace (P14) | direct | read: the build command, which writes the smoke contract; plan 0.7, 0.9 |
| npm dependencies (zod, oclif, testcontainers, …) | A pack may not contain `package.json` or `node_modules` (0.6). Ferry is an external tool in `requires.tools`, and its hooks have no dependencies | direct (P11) | read: plan 0.6 |

---

## 3. Orchestrator overlap

Both systems are orchestrators.

- **Ferry** walks a data-dependent DAG. Its outputs drive its own control flow (inferred, from
  the pipeline):
  - the number of modules and tasks is known only after `decompose` and `plan`;
  - it adds retries with feedback, gap rounds, quarantine and convergence guards, all keyed on structured outputs.
- **Collide** runs a declared transition table (0.7):
  - phases are fixed at launch, and loops are `round` re-entries;
  - `fanout` means "one role on several models", not "one instance per item";
  - judgment steps are agent sessions, not single structured calls.

### (a) Collide's engine drives Ferry's phases, with Ferry code as hooks and gates

Pros:

- **Everything is visible.** Every LLM call becomes an agent phase, with measured tokens and cost, a budget, the
  usage-limit pause, permission hosting and a transcript (inferred, from plan 0.3 and 0.13).
- **Experiments get more precise.** They can compare per phase, and the primary metric can come from measured data
  (plan 0.14).
- **Approvals cost nothing.** The §12 approvals are plain `human` phases.

Cons:

- **It rewrites Ferry's control flow** in YAML and hooks. The pipeline core (749 lines) and the pipeline (418
  lines) hold the loops, budgets, quarantine and breaker (read).
- **Two orchestrators of one pipeline would diverge.** Ferry's platform still calls the CLI (read: its architecture doc
  §19).
- **It needs a core change no other pack needs.** Fan-out over a runtime list (P1) is not wanted by Orchard or by
  issue → PR → CI.
- **Each structured call becomes a whole agent session.** Ferry chose single calls for repeatability and token
  economy (read: its architecture doc §9.1). As agent sessions they cost more, because 0.3 makes judgment steps agent
  phases.
- **The content-addressed replay is lost.** Collide has no phase-level cache, so a re-run after a human decision
  pays for everything again (inferred).
- **Non-Anthropic endpoints are lost.** The v0 adapter adds no `ANTHROPIC_*` variable (read: plan 0.3).
- **Container isolation is lost.** Ferry deliberately runs agents in Docker; containers for agents are deferred
  in Collide (read: plan "Deferred").
- **The runs get very long.** Run 5 had 68 phases and 21 tasks (read). With retries, a run becomes hundreds of
  Collide phases (inferred).

### (b) Ferry's whole run inside one `script` phase, reporting through `COLLIDE_REPORT`

Pros:

- **Nothing in Ferry is rewritten.** The cache, Docker isolation and every model profile keep working.
- **Collide adds what Ferry lacks:**
  - a workspace;
  - launch from a board;
  - the Queue on exit 2;
  - run history;
  - stopping the process tree, with containers through `cleanup`;
  - `integrate`;
  - whole-run experiments.

Cons:

- **One opaque phase of up to 15 hours** (read: its roadmap appendix).
- **No §12 approvals mid-run.**
- **Agent spend is invisible to Collide's budgets, usage-limit pause and permission policy (C1).** Every token is
  "reported" at best.
- **One shape for a long, branching process.** A human answer can only restart the whole phase.

### (c) Hybrid: Collide owns the coarse spine and the doors; Ferry owns the inner loop

The workflow is short and readable: `design` (script) → `approve-design` (human) → `build` (script, with gate
`ferry/verify`) → a decision script → `decide` (human) or `ask_human` → back to `build` as round 2, or end.

- Each script phase calls the same CLI. Re-entry is cheap because of Ferry's cache: `build` replays `design`, and
  a resumed build replays completed `implement` tasks (inferred, from the phase runner and the implement
  phase).
- An incremental build maps each increment, or each pause, onto a Collide `round`.

Pros:

- The one-way doors Ferry designed but never built come free as `human` phases.
- Ferry stays a standalone CLI for Ferry's platform.
- Collide's run preview, Queue, Tracks and critical path show the meaningful stages.
- Re-entry costs little.
- Collide's experiments compare Ferry configurations end to end.
- Optional: an agent phase reviews SpecIR before the approval door. That is the kind of adversarial review
  Orchard's loop has and Ferry lacks (inferred).

Cons:

- **Tasks inside `build` are still opaque.** They are visible only through reported spans and progress.
- **Policy is split between two places.** Agent permissions and budgets stay Ferry's (C1).
- **It needs small changes on both sides:**
  - Ferry: a `--stop-after` flag (P2), `--cache-dir` (P3), container labels (P4), an env file for secrets (P5), a
    `COLLIDE_REPORT` reporter (P6) and a `verify` command (P7);
  - Collide core: C1 to C3.

### Recommendation: (c)

- **It keeps each system on what it already does well.** Ferry's DAG-driven inner loop and replay cache are its
  core. Collide's value is the board, the doors, the Queue, `integrate` and measurement.
- **It does not grow core on Ferry's behalf.** Every core change it asks for (section 5) is one that Orchard or
  issue → PR → CI would also use.
- **Option (a) fails the triage rule.** Its main enabler, data-driven fan-out, is wanted by no other pack.

---

## 4. Sketch: the `ferry` pack

Written in the style of 0.7's Orchard example. Lines marked `NOT EXPRESSIBLE` need a change listed in section 5.
Lines marked `SYNTAX?` use a construct that 0.7 names but does not show an example of; the syntax is a guess.

```yaml
apiVersion: collide/v0
name: ferry
version: 0.1.0
requires:
  sdk: ">=0.1 <0.2"
  tools:
    - { name: node, version: ">=22.18" }
    - { name: docker }
    - { name: ferry }                   # external CLI; never vendored into the pack (0.6)
    - { name: code-index }               # update path, and increments 2..N
  agents: []                             # no Collide agent phase: Ferry's agents run in Ferry's runner containers (C1)

config:
  docsGlob:    { type: string, default: "docs/arch/*.md" }
  runnerImage: { type: string, default: "ferry/runner:latest" }  # pin a digest for experiments
  profiles:    { type: object }          # profile → non-secret endpoint fields (base URL, model ids)
  anthropicToken: { type: string, secret: true }   # keychain only; hooks write a 0600 env file in phase.dir (P5)
  gatewayToken:   { type: string, secret: true }   # for proxied endpoints (Kimi, GLM, …)
  maxLiveRuns: { type: integer, minimum: 1, default: 1 }   # Docker capacity (P13)

launch:
  llmProfile:   { type: string, enum: [sonnet, kimik27, glm, qwen3, deepseekv4-flash, minimax3], default: sonnet }
  taskBudget:   { type: integer, minimum: 1 }     # Ferry token ceilings, passed as flags; not counted by Collide
  designBudget: { type: integer, minimum: 1 }
  refineBudget: { type: integer, minimum: 1 }
  incremental:  { type: boolean, default: false }

workspaces:
  app:                                   # cwd = line root; every hook returns { path, gitDir } (0.1)
    create:  { script: hooks/ws-create.ts,  timeout: 30m }   # git worktree add; checks docker and the runner image
    resume:  { script: hooks/ws-resume.ts,  timeout: 30m }
    remove:  { script: hooks/ws-remove.ts,  timeout: 5m }
    cleanup: { script: hooks/ws-cleanup.ts, timeout: 5m }    # docker kill Ferry's runner + Testcontainers by label (P4)

record:
  paths: [ferry.lock.json]                # doc hash → built commit; the provider derives status from it
  script: hooks/record.ts

protectedPaths: [docs/arch/**, ferry.lock.json, .collide/**]

hooks:                                   # SYNTAX?: chain handlers on named points (0.4); 0.7 shows no block for them
  run.before_start: { script: hooks/docker-slot.ts }   # refuse while maxLiveRuns Ferry runs are live (P13)

metrics:
  import:    { script: metrics/ferry-report.ts }      # build-report.json + task-telemetry.jsonl → spans (fallback)
  processes: { script: metrics/ferry-containers.ts }  # container ids by label collide.workspace=<path> (P4)

experiment:
  prepare: { script: hooks/exp-prepare.ts }  # empties the arm's copy of cache/ unless the experiment is about caching

gates:
  verify:
    script: gates/verify.ts              # `ferry verify` (P7); today it reads verification.json
    # NOT EXPRESSIBLE: verdict `unsatisfiable` has no result status; today it maps to `fail` + outputs.cause (C2)

roles: {}                                # optional: a spec-reviewer agent role before approve-design

workflows:
  build:
    accepts: { sources: [docs], fields: [docId] }
    workspace: ferry/app
    integrate: github/pull-request       # or collide/merge-local
    on_start: [{ op: setStatus, args: { category: active } }]
    on_end:   [{ op: setStatus, args: { category: review }, when: 'outcome == "built"' }]
    start: design
    phases:
      design:
        kind: script                     # SYNTAX?: 0.7 names the kind; the example shows none
        hook: { script: phases/ferry.ts, timeout: 6h, grace: 120s }   # build --stop-after contract (P2)
        outputs: [spec.md, interface.md] # rendered from spec.json / interface.json for the human
        then:
          script: workflow/after-ferry.ts # exit code + outputs → token
          on: { ok: approve-design, decide: decide, broken: ask_human }
      approve-design:                    # architecture doc §12 one-way door
        kind: human
        show: [design/spec.md, design/interface.md]   # NOT EXPRESSIBLE: what a human phase shows (C3)
        on: { approve: build, stop: { end: rejected } }   # SYNTAX?: token → target, as in a decision
      build:
        kind: script
        hook: { script: phases/ferry.ts, timeout: 24h, grace: 120s }  # design replays from cache; round > 1 adds --resume
        gates: [verify]
        then:
          script: workflow/after-ferry.ts
          on: { pass: { end: built }, decide: decide, unverified: ask_human, broken: ask_human }
      decide:
        kind: human
        show: [build/decision-packet.md] # NOT EXPRESSIBLE (C3); items answered as a whole, not one by one (P9)
        on: { rerun: build, rerun-auto-resolve: build-auto, accept-as-is: { end: built }, stop: { end: stopped } }
      build-auto:
        kind: script
        hook: { script: phases/ferry.ts, timeout: 24h }   # same script; phase name selects --auto-resolve
        gates: [verify]
        then: { script: workflow/after-ferry.ts, on: { pass: { end: built }, decide: decide, unverified: ask_human, broken: ask_human } }

  update:                                # the doc changed after an integrated build
    accepts: { sources: [docs], fields: [docId] }
    workspace: ferry/app
    integrate: github/pull-request
    start: reconcile
    phases:
      reconcile:
        kind: script
        hook: { script: phases/ferry.ts, timeout: 2h }   # update --stop-after reconcile (P2): writes change-set.json
        then: { script: workflow/after-ferry.ts, on: { breaking: approve-break, ok: apply } }
      approve-break:
        kind: human
        show: [reconcile/change-set.md]  # NOT EXPRESSIBLE (C3)
        on: { approve: apply, stop: { end: rejected } }
      apply:
        kind: script
        hook: { script: phases/ferry.ts, timeout: 24h }
        gates: [verify]
        then: { script: workflow/after-ferry.ts, on: { pass: { end: built }, decide: decide, unverified: ask_human, broken: ask_human } }
      decide:
        kind: human
        show: [apply/decision-packet.md]
        on: { rerun: apply, accept-as-is: { end: built }, stop: { end: stopped } }

providers:
  docs: { script: providers/arch-docs.ts }   # scope: ref; one task per doc matching docsGlob; field docId
```

### Hook scripts

All are TypeScript with no npm dependencies (0.6).

| script | what it does |
|---|---|
| `providers/arch-docs.ts` | A ref-scoped provider. It lists docs matching `docsGlob` as tasks. `docId` is the file stem, checked as a safe token. Status is derived: `ready` when the doc's hash differs from its entry in `ferry.lock.json`; `done` when they match. `apply setStatus` writes `ferry.lock.json`, and `record` commits it |
| `hooks/ws-create.ts` | `git worktree add` with a branch built from `run.slug`. Checks that Docker is up and that the pinned runner image is present. Returns `{path, gitDir}` |
| `hooks/ws-resume.ts`, `hooks/ws-remove.ts` | Re-check the workspace; remove the worktree and branch |
| `hooks/ws-cleanup.ts` | After `interrupted` or `timed_out`: `docker kill` every container labelled with this workspace, both Ferry's runner and Testcontainers. The client being killed does not stop a container (read: the Docker runner's doc comment) |
| `hooks/docker-slot.ts` | On `run.before_start`: refuses while `maxLiveRuns` Ferry runs hold a slot (a lock file in `COLLIDE_PACK_DATA`) |
| `phases/ferry.ts` | The single script behind every script phase. It does six things, listed below this table |
| `workflow/after-ferry.ts` | A decision script. It maps the phase's `outputs` (exit code, verdict, number of packet items, contract-breaking flag) to one declared token |
| `gates/verify.ts` | Runs `ferry verify` (P7) on the workspace, or reads `verification.json` until that exists. Returns `ok` or `fail`, with the violations capped in `outputs` (64 KB). `unsatisfiable` → C2 |
| `hooks/record.ts` | Commits `ferry.lock.json` at the line root under the line lock |
| `metrics/ferry-report.ts` | `metrics.import`: turns `build-report.json` per-phase rows and `task-telemetry.jsonl` into spans. Used when live reporting (P6) is off |
| `metrics/ferry-containers.ts` | `metrics.processes`: returns the container ids labelled with a workspace |
| `hooks/exp-prepare.ts` | Empties the arm's copy of `cache/`, unless the experiment compares caching |

What `phases/ferry.ts` does, in order:

1. Reads the doc at the run's pinned snapshot commit, not from the workspace (P14).
2. Writes the LLM profile and the token from config into a 0600 env file in `phase.dir` (P5).
3. Runs `ferry build|update` with these flags, chosen from the phase name and the round:
   - `--workspace-dir`;
   - `--build-dir <phase.dir>/ferry`, so the per-run records stay with the phase;
   - `--cache-dir $COLLIDE_PACK_DATA/cache` (P3);
   - `--stop-after` (P2);
   - `--resume`, `--auto-resolve`;
   - the budget flags.
4. Wraps the run in `collide report span`.
5. Renders `spec.json`, `interface.json`, `change-set.json` and `decision-packet.json` to Markdown in `phase.dir`.
6. Translates exit codes 0, 1, 2 and 3 into a result `status` and `outputs`.

---

## 5. Gaps

### 5.1 Core gaps

These are the ones the recommended option (c) needs. Each one also passes the triage rule.

| id | gap | plan section | why core | proposed change |
|---|---|---|---|---|
| C1 | **Agent work inside a `script` phase is invisible.** Its tokens never reach the run's totals, and its spend escapes the usage-limit pause and permission policy. Collide accounts only for agent phases. A script that runs `claude -p` on the user's login can use up the shared account limit that every other run's pause relies on (inferred) | 0.3 Agents and model sources ("Usage and rate limits"); 0.5 Hook protocol ("Reporting"); 0.13 Metrics ("Levels") | Wrapping an existing agent CLI in a `script` phase is the obvious route for any tool Collide has no adapter for until Stage 10 and beyond. The usage-limit pause is a core guarantee that any such pack silently breaks | Add a `usage` report kind (`tokensIn`, `tokensOut`, `cacheRead`, `cacheWrite`, `model`, optional `costUsd`), labelled reported and shown beside the measured totals. Add a `limited` report kind (`resetsAt`) that triggers the same account pause. Add a declared `agentWork: true` on `script` and `wait` phases, so the preview and the Queue say "agent work outside Collide's adapter: budget and permissions are the pack's" |
| C2 | **No "could not verify here" result.** A gate or `wait` hook must say `fail`, which sends a fixer at code that is not broken, or `error`, which reads as a crashed hook | 0.5 Hook protocol ("Result"); 0.7 Pack manifest (`then:`, decisions); 0.14 Experiments ("Outcomes") | Orchard's `iter-evidence` gate meets it when Docker or its lanes are down. An issue → PR → CI `wait` phase meets it when a check is cancelled or its runner is lost (inferred). Ferry has proven the distinction in use (read: the gate result schema's doc comment) | Add a result status `unverified` (with `message`) for gates and for `script` and `wait` phases. Workflows route on it like the other statuses. Experiments count it as its own outcome, never as a pass. In the Queue it is a cause distinct from `fail` |
| C3 | **A `human` phase or `ask_human` cannot name what the person should read.** 0.7 says only "a Queue item where a person picks one of the phase's declared tokens" | 0.7 Pack manifest (`human`, `ask_human`); 0.11 Interface direction (Queue) | Orchard's `stop: ask_human` after adjudicate needs the round record in front of the person. Issue → PR → CI's `human` phase needs the PR link and the CI summary (Stage 8 fixture) | `human` phases and decision targets take `show:`: phase artifacts (`<phase>/<file>`) and `outputs` keys. The Queue item renders them, as the drill-down renders a phase report |

**Core changes only option (a) would need.** Each is triaged against the rule, and none is proposed now.

- **Data-driven fan-out** (one phase instance per item of an earlier phase's output list): pack-only (P1).
  - No Orchard or issue → PR → CI step has a runtime-sized item list. Orchard's `fanout` is per model, and grouped
    chains are per task, known at launch (read: plan 0.7, Stage 12).
  - Revisit when a second pack asks for it.
- **Per-alias model source for the Claude adapter** (`ANTHROPIC_BASE_URL` plus a keychain token): already covered by
  "Deferred: other agent tools and model sources". Two notes for when it lands:
  - Claude Code reaches gateways (Bedrock, Vertex, LiteLLM) through these variables;
  - the token belongs in the snapshot settings file rather than the environment, because of 0.5's `ps -E` rule.
- **Single structured LLM calls as a phase kind.** Not core: it contradicts "judgment steps are agent phases"
  (0.3). An agent phase that writes JSON to `phase.dir`, checked against a schema by a gate, is enough.
- **Agents in containers.** Already listed in "Deferred" ("An OS sandbox or containers for agents").

### 5.2 Pack-only gaps

There are 15. The fixes belong in the `ferry` pack or in Ferry itself.

| id | gap | plan section touched | why pack-only | proposed change |
|---|---|---|---|---|
| P1 | Ferry's task count is known only after `plan`; Collide's table is fixed at launch | 0.7 Pack manifest | Under (c), the loop stays inside Ferry. No other pack needs it (see above) | None in core. Keep the DAG walk in Ferry |
| P2 | No stage-split entry point, so the §12 doors cannot sit between stages | 0.7 (`human` phases) | Ferry's own CLI | Ferry: `--stop-after <phase>` on `build` and `update`. The next call replays the cached stages |
| P3 | `<build-dir>` mixes per-run records, resume state and the cross-run cache | 0.7 Pack manifest ("Pack data") | Ferry's own layout | Ferry: `--cache-dir`. Cache → `COLLIDE_PACK_DATA`; records → `phase.dir`; resume state → the workspace's `.ferry/` |
| P4 | Containers carry no labels, and a killed client leaves its container running | 0.5 Hook protocol ("Stopping"); 0.13 Metrics (containers) | The `cleanup` and `metrics.processes` hooks already cover it, as for Orchard's lanes | Ferry: add `--label collide.run=$COLLIDE_RUN --label collide.workspace=<path>` to `docker run`, from an env pass-through |
| P5 | Secrets go into `docker run -e KEY=VALUE`, which puts them in argv, and into a plaintext LLM config file (read: the Docker runner and the config loader) | 0.5 ("settings travel on stdin, never in env"); 0.7 ("Secrets") | Ferry's own launcher | Ferry: accept `--env-file`. The hook writes it as 0600 in `phase.dir` from keychain-backed config |
| P6 | No live progress: the planned event spine is "Not Started" | 0.5 ("Reporting"); 0.13 Metrics | 0.5 already gives `COLLIDE_REPORT` to `script` phases | Ferry: build the spine with a `CollideReporter`, emitting from orchestration code only: a span per step, task progress, gate metrics |
| P7 | No `ferry verify`, so a Collide gate can only trust the build's own `verification.json` | 0.4 ("Gates"); 0.14 (`collide bench`, "Outcomes") | Ferry's own CLI | Ferry: `verify --workspace-dir`, running the gate suite alone. It also gives `collide bench` a step with no agent |
| P8 | Its README's exit-code table omits 3 (`unsatisfiable`) | 0.5 ("Result") | Documentation | Ferry: document exit 3. The hook maps all four codes |
| P9 | A decision packet has N items, each with free-text options. A `human` phase picks one token for the whole run | 0.7 (`human`); 0.11 (Queue) | Neither Orchard nor issue → PR → CI produces decisions with several items | Run-level tokens (rerun, rerun with auto-resolve, accept as is, stop). Item answers go into a resolution file the person edits, which `ferry.ts` reads on the next round |
| P10 | No task source | 0.10 Task sources | A pack provider is the designed route | `providers/arch-docs.ts`, ref-scoped, with status derived from `ferry.lock.json` |
| P11 | Ferry is a pnpm monorepo with npm dependencies | 0.6 Packs in TypeScript | The plan's rule is right: run the tool, don't vendor it | `requires.tools: ferry`. Hooks shell out to it |
| P12 | The scaffold rewrites the root `.gitignore` and `package.json`, and the runner commits a baseline before each agent call | 0.1 Principles (workspaces are git checkouts) | Ferry assumes it owns the repo | The board is a dedicated app repo, or `--workspace-dir` points at a subdirectory. Integrate by squash |
| P13 | Concurrent Ferry runs contend for Docker (runners and Testcontainers) | Deferred ("Resource tokens") | Orchard also throttles its own shared service, inside `iter` (read: plan "Deferred") | A `run.before_start` slot lock in the pack data directory. Note for the owner: two packs now throttle a shared service themselves, which is evidence for widening the resource-token trigger beyond "the first non-Claude adapter" |
| P14 | The architecture doc must not come from the workspace, which agents can edit | 0.7 (snapshot loading); 0.9 Untrusted input | The input is pack policy | `ferry.ts` reads the doc at the run's pinned snapshot commit. `protectedPaths: [docs/arch/**]` |
| P15 | Ferry's version, runner image digest and LLM profile are not in the fingerprint | 0.13 Metrics ("Fingerprint") | 0.13 already takes "the cache state the pack reports" | The pack reports them as cache-state attributes. `runnerImage` is pinned by digest |

### 5.3 Where Ferry is ahead of the plan

1. **A three-state verdict.**
   - `unsatisfiable` is separate from `fail`, and refine runs only on `fail`, so code that was never proven broken is
     never "fixed" (read: the gate result schema and the pipeline core).
   - This is the source of C2.
2. **Failures-only, typed retry feedback.**
   - Each retry carries the prior output plus the violations, capped. Raw gate output never reaches a prompt (read:
     the gate result schema; its architecture doc §7.1, §8.3).
   - Collide's fix loops pass prose artifacts. Suggestion: the `collide/review` pack could render gate `outputs` as
     a fixed-shape artifact for the fix prompt. This is pack policy, not core.
3. **Convergence guards.** A refine round that does not reduce failures, or goes over its budget, stops and
   escalates as `refine-stalled` (read: the decision packet schema).
   - `collide/review` stops only at a maximum round count (read: plan 0.4).
   - Suggestion: `collide/review`'s decision script also stops when the gate's failure count did not fall between
     rounds.
4. **Quarantine and continue, then one decision packet.**
   - A long run finishes everything it can, then asks once (read: the decision packet schema's doc comment).
   - Collide's `ask_human` stops at the first question. For a run of many hours, batching questions matters.
   - The Ferry pack gets this from Ferry. A core change is not proposed until a second pack wants it.
5. **Write-time enforcement of protected paths.**
   - Ferry reverts and quarantines a write to a frozen path the moment it happens (read: the pipeline core).
   - Collide only flags `protectedPaths` in review (read: plan 0.9) and checks `readOnly` at phase end (read: plan
     Stage 8).
   - Suggestion: an optional phase-end check that fails a phase whose range touches `protectedPaths`, as `readOnly`
     does today.
6. **A content-addressed replay cache, including the workspace effect.**
   - Re-running after a human decision replays every unchanged step (read: the orchestrator's phase runner).
   - Collide's nearest equivalent is adopting at a named phase (read: plan 0.7).
   - Not proposed for core. It belongs to a tool whose steps are pure functions of their input.
7. **The "phases never emit" invariant.**
   - All events come from orchestration code, so a cached run and an uncached run tell the same story (read:
     its plugin implementation plan, Invariant 1).
   - The plan reaches the same place from another direction: agent phases cannot report, and Collide measures the
     rest (read: plan 0.5).
   - This is independent support for that rule, not a gap.
8. **An endpoint circuit breaker.**
   - Three consecutive endpoint stalls abort the build with a resumable escalation, rather than quarantining every
     remaining task (read: the pipeline core).
   - Collide's usage-limit pause covers Claude's own limit signal. A dead gateway or a network outage would fail
     each phase in turn (inferred).
   - Worth one line in the adapter's event mapping if ACP adapters bring other endpoints.
