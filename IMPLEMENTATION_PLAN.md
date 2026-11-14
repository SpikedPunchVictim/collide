# Collide — Implementation Plan

Task-oriented desktop IDE: pick tasks from any source, run agents on them in isolated workspaces through
pack-defined workflows. Rust + Tauri + React. v0 runs Claude Code only; the design keeps other agent tools and
model sources (OpenAI, Ollama, …) addable without core changes.

Claims are labelled **measured** (observed, with the observation named), **inferred** or **unmeasured**.

**Status: revision 10, frozen until the next spikes.** S5 and S7 are running, then S6 and S8 (owner, 2026-11-13);
S2, S9 and S11 are applied. Their decision records and the owner's decisions may change this plan; nothing else does until then.

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
  kinds, op effects, typed links, an assignee, a checklist field, five task points and derived points. A built-in
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
  tails, so an owner restart loses nothing. Recovery is reattach by pid, or `claude --resume <session-id>`.
- **The owner is `collide-daemon`, with a per-child shim** (S3; owner, 2026-11-13).
  - The shim is the `collide shim` mode of the same binary. For one child it:
    - calls setsid;
    - starts the child with stdout and stderr going to log files;
    - holds the child's stdin pipe, and for an interactive session the PTY master, across a daemon restart
      (measured S2: a `claude -p` turn and a PTY take-over both went on across a daemon SIGKILL and restart, with no
      gap in input or output);
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
  - Everything Collide starts gets this environment minus the adapter's stripped variables (0.3), terminal session
    ids (`TERM_SESSION_ID`, `ITERM_SESSION_ID`) and `XPC_SERVICE_NAME`/`XPC_FLAGS`.
- **The socket** is at a short fixed path. AF_UNIX paths are limited to 103 bytes on macOS (measured S3); a longer
  path fails fast with a message naming the path and the limit.
- **Durability** (herdr's bugs, read):
  - logging never panics, and a full disk is tested;
  - state is saved without holding a lock that other work waits on;
  - a store or `exit.json` that cannot be read is moved aside and reported, never overwritten;
  - on a full disk the shim keeps its exit record in memory and serves it until the daemon acknowledges it.
- **The workflow engine needs a live owner between phases.** Something has to notice that phase N ended and start
  phase N+1. With the app closed, the next phase started 0.25–0.37 s after the previous one ended, while the daemon
  lived (measured S3). If the daemon dies, children keep running and logs keep being written, but no phase advances
  until it restarts. A restarted daemon reattaches and advances.
- **The UI speaks the owner's protocol.** The webview calls one generic `rpc` command and listens on one event
  channel. The shell forwards each call to the socket and adds the token itself, so the token never reaches the
  webview. The CLI and the UI therefore share one contract, and UI tests run in a browser against a fake owner.
- Every child gets `COLLIDE_RUN=<id>` and `COLLIDE_RUN_START=<start clock>` in its environment. A stop finds
  descendants three ways:
  - a ppid walk;
  - a marker scan of the user's processes, enumerated in-process with libproc. Processes older than the run's start
    clock are skipped, so a reused pid is never hit;
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
- The socket requires a per-client token. The token is never put in an agent's or hook's environment. Agents need
  no access to Collide in v0 (no MCP server; see 0.3).
- Crates: `core` (domain, state machines, workflow engine, pack model, implementation resolution; traits `AgentAdapter`,
  `HookRunner`, `TaskProvider` only; no Tauri, SQLite or process types) and `host` (implementations, store,
  `collide-daemon` and `collide` binaries; the shim is `collide shim`). `app/src-tauri` stays thin.
  `packages/pack-sdk` is the TypeScript SDK.

### 0.3 Agents and model sources

Two separate ideas:

- **Agent tool:** the program running the agent loop. v0: Claude Code. Later: opencode (`opencode acp` installed,
  1.1.56), goose (1.49.0 installed), Codex.
- **Model source:** where inference runs. v0: whatever Claude Code is configured for. Later: OpenAI, Ollama
  (0.40.15 installed), others, reached through the agent tools that support them.

Collide never calls a model API itself. Judgment steps are agent phases.

- `AgentAdapter` (core trait) turns a role + phase into a launch spec, parses the tool's stream into normalized
  events (message, tool call, permission request, usage, final report), answers permission requests, and declares
  **capabilities**: `headless_stream`, `permission_host`, `session_resume`, `background`, `budget_cap`,
  `agent_definitions`, `usage_report`. A role may `require` capabilities; Collide refuses to start a phase on a
  tool that lacks one rather than silently losing it.
- **v0 ships one adapter, Claude.** The launch line, measured by S1 on 2.1.323:

  ```
  claude -p --input-format stream-json --output-format stream-json --verbose --include-hook-events
    --session-id <uuid>                       # a resumed phase passes --resume <uuid> instead
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
    to `ask_human` with cause budget (measured S1).
  - Cost (`total_cost_usd`) adds up per session, across `--resume`. So the adapter passes the remaining budget on
    each resume, and a phase's cost is the last total its session reported (measured S1).
  - **Disallowed tools.** The headless default tool set includes scheduling, worktree and messaging tools
    (`CronCreate`, `ScheduleWakeup`, `RemoteTrigger`, `PushNotification`, `EnterWorktree`, `ExitWorktree`,
    `Workflow`, `SendMessage`; measured S1). They start work the owner cannot see, or move the session out of its
    workspace, so they are disallowed unless a role asks for one (owner, 2026-11-13).
- **The stream protocol** (measured S1):
  - user turns, permission answers (`control_response`, allow or deny) and interrupts (`control_request`
    `interrupt`) go on stdin;
  - the phase report is `result.result` of the final `result` event, which holds the last assistant message only;
  - `result/success` does not mean the work happened, so the adapter also checks refused tool calls (below) and
    `is_error`. A `result/success` with `is_error: true` is an API failure, not a report (measured S1: one with
    `api_error_status: 429`). A 429 is handled as a usage limit (Usage and rate limits, below); any other status
    fails the phase, with the status;
  - an error `result` still carries its spend, and the spend counts (measured S1: a budget error with
    `total_cost_usd` 0.00353);
  - a phase settles once the child has exited and its stdout has reached its end, in either order, so the last
    lines, usually the `result`, are never dropped (inferred: Ferry's platform supervisor settles on exit, which can
    lose them);
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
    143 and no result.
- **Who picks what:**
  - A role's `agent:` picks the tool.
  - A pack's `model:` names an alias (`fast`, `strong`). Each user maps aliases to a model and source *for that
    tool* in `~/.config/collide/models.yaml`. An alias never switches the tool.
  - Each mapping also names `model.name` (`opus`, `sonnet`). Collide requires only a safe token (0.5), because the
    name reaches env values, paths and arguments. A pack whose labels need a narrower form
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
  - **The tool's config paths in a repo.** For Claude: `.claude/**`, `CLAUDE.md` and `.mcp.json`, plus auto memory
    under `~/.claude/projects/<main checkout key>/memory/`.
    - A reviewer works in the builder's worktree, and the builder can edit all of these. So the Claude adapter takes
      settings, agents and MCP config from the line's snapshot (`--setting-sources user`, `--settings`,
      `--mcp-config`, `--strict-mcp-config`).
    - Measured S1: under that line nothing from the worktree loads (project and local settings, CLAUDE.md, skills,
      agents, `.mcp.json`). The user's own CLAUDE.md, hooks and plugins still load, and so does auto memory.
    - Plain `claude` in a worktree, as in a take-over, loads everything, including the main checkout's untracked
      `settings.local.json` and `.mcp.json` servers nobody approved (measured S1).
    - **Auto memory is off for every phase** (`autoMemoryEnabled: false` in the snapshot settings; owner,
      2026-11-13). It is shared by every worktree of a repo, keyed to the main checkout, so it would carry one run's
      notes into another. A role may turn it on.
    - **Acknowledgement** (narrowed by the owner after S1, 2026-11-13). An edit to these paths cannot reach a later
      automatic phase, because those phases load from the snapshot. It matters once it is about to reach the line's
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
    - `compact_at`, which sets `CLAUDE_CODE_AUTO_COMPACT_WINDOW`. Compaction fires at that window minus 33K tokens.
      The smallest window is 100K and the largest the model's; an unusable value is ignored with no warning, and the
      variable overrides the user's own `settings.json` window (all measured S9). So Collide refuses a value under
      100,000;
    - `config: user | none`, which picks `--setting-sources user` or `""`. Both keep `--strict-mcp-config`: with
      `""` alone, one of S9's five runs still listed the user's claude.ai connectors (measured S9);
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
    - The run preview lists the user hooks and plugins a phase will load. It flags those that rewrite tool calls,
      block prompts or set context variables.
    - A prompt blocked by a hook becomes a Queue item with the hook's reason, never a silent `result/success`. The
      adapter sees a `system/informational` event with `prompt_submit_outcome` (`kind` `blocked_by_hook` or
      `stopped_by_hook`, and `reason`), then `result/success` with `num_turns: 0` and a null `terminal_reason`.
      That result's text is the hook's message, never the phase's report. A `/compact` result also has zero turns,
      but carries `local_command` (measured S9).
    - `UserPromptSubmit` hooks emit no hook events in the stream, so the preview's list is the only warning before a
      block (measured S9 without `--include-hook-events`; with it, unmeasured).
    - Experiment arms use `config: none` (S1).
- **Usage and rate limits.** Parallel runs share one account, so a limit is normal in the first week.
  - The adapter turns the tool's limit signal into a `limited` event with the reset time. For Claude that is a
    `rate_limit_event` whose `rate_limit_info.status` is `rejected`, with `resetsAt` in epoch seconds, or an
    assistant message with `error: "rate_limit"`. No text parsing is needed. Read from the binary's schemas and
    local transcripts by S1; a live limit is unmeasured. `status: "allowed_warning"` lets other runs pause early.
  - The phase pauses (`paused: usage limit`) instead of failing. Other runs on the same account pause before their
    next turn.
  - The Queue shows the reset time. The phase resumes by session id when the limit resets, or when the user resumes
    it.
  - Paused time is a wait, not active time (0.13).
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
- Later adapters: an ACP adapter (one implementation for every ACP-speaking tool), then Codex. S7 maps
  `AgentAdapter` onto ACP before the skeleton (Stage 2) outlines the trait, so it is not shaped by Claude alone.
  Local-model capacity (GPU memory) becomes a scheduler resource token with the first non-Claude adapter.

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
  | later: accept | `sync` check; the `before_*` points of the source's operations and of the run's task diff; `run.before_integrate`, `integrate`, `run.after_integrate`; the source's operations (writes, then `after_*` points); `record` |
  | later: removal | `workspace.before_remove`, then `remove` (task workspaces) or `discard` (sandbox and experiment) |

  Accepting is a user action on a finished run, so the integrate points fire after `run.after_end`. The `before_*`
  points of accept run first, so a refused `done` gate leaves the code unmerged (S11).
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
  `{ "status": "ok" | "fail" | "unverified" | "refused" | "error", "message", "outputs": {} }`. The status in this file
  is the verdict. A non-zero exit or a missing or invalid file is `error`. A child tool's exit code is never forwarded
  as a verdict (R1: `iter`, `plan` and `preflight` use exit 2 for usage errors).
  - **`unverified`** (owner, 2026-11-13; Ferry's three-state gate result): a gate, `script` or `wait` hook could
    not check here, for example Docker or Orchard's lanes are down, or a CI check was cancelled. It is neither a
    pass nor a fail, so no fixer is sent at code nobody proved broken.
    - In a workflow or at a named point, it holds the run in the Queue with the hook's message. The person retries
      the hook, proceeds as if it passed, or stops the run.
    - From any other hook (workspace, `record`, `integrate`) it is `error`.
    - Experiments count it as its own outcome, never a pass (0.14).
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
  - A line is at most 4 KB. Each span's file has a size and rate cap, set by S8. Malformed or excess lines are
    counted on the span, not hidden.
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
  So `@collide/pack-sdk` ships as compiled JS + `.d.ts`, with zero runtime dependencies, and Collide supplies it:
  - Collide materializes the pack into `~/.cache/collide/packs/<tree id>/`, keyed by the pack's git tree id,
    whatever the pack's source (0.7). An entry never changes, and lines share it whenever their pack content matches. It
    holds:
    - `pack/`, a read-only, verified copy of the pack directory;
    - Collide's own `package.json` (`{"private": true, "type": "module"}`), so a hook's module type never depends
      on a `package.json` above the cache (measured S4);
    - `node_modules/@collide/pack-sdk`, a symlink to a versioned compiled SDK outside the app bundle
      (`~/.cache/collide/sdk/<version>/`). Node checks the realpath, so a `.ts` SDK is never shipped (measured S4);
    - a marker, written last. The entry is built in a staging directory and renamed into place.
  - **A pack that contains a `node_modules` path or a `package.json` is refused at load.** A committed
    `node_modules/@collide/pack-sdk` shadows Collide's SDK (measured S4; R2's "a snapshot has no `node_modules`" was
    wrong). So the SDK never comes from the repo.
  - `requires.sdk` names the range the pack accepts.
  - v0 packs have no other npm dependencies; a pack that needs one vendors it.
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
  Ferry pack would drive Ferry, declares it here.
- Rejected: embedding a JS engine. Hooks start CLIs anyway, so a sandbox would buy little and cost Node and npm.
- Types: Rust serde types → `schemars` → JSON Schema → TS types. The same schema validates `pack.yaml`,
  `board.yaml` and views.

### 0.7 Pack manifest (`pack.yaml`)

- YAML 1.2 core schema with typed fields; that is what keeps `no` a string. Unknown keys are rejected to catch
  typos. Every `{{ }}` template must be quoted (unquoted it parses as a flow mapping).
- `serde_yaml` is archived; S5 picks a maintained crate. `$schema` points at `collide schema --path`, not a path
  relative to the Collide repo.
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
  - Values come from three layers, lowest first:
    1. the schema's defaults;
    2. the user's file, `~/.config/collide/config.yaml`, per board and per pack;
    3. the repo's shared file, `.collide/config.yaml` (committed). **The repo's value wins**, so a team setting
       applies to every developer (owner, 2026-11-13). The exception is a key the pack marks `overridable: true`,
       such as a lane pool that depends on the machine: there the user's value wins (owner, 2026-11-13; open
       question 7);
    4. the run's profile, if it has one (below).
  - Collide's own settings (concurrency cap, metrics level, retention, default model aliases) use the same layers
    and the same files. Collide marks its machine-dependent keys, such as the concurrency cap, `overridable`.
  - Some keys are per user by nature and are refused in the repo file: secrets, trust entries, source accounts and
    personal views.
  - **Secrets** (`secret: true`) come only from the OS keychain, set in the UI or with `collide config set --secret`.
    A secret in either file is a load error. Hooks receive settings as `config` on stdin (0.5).
  - `.collide/config.yaml` loads from the line's snapshot and is in the trust hash, like `board.yaml` (0.9).
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

  - `git:` fetches into a bare mirror under `~/.cache/collide/git/` with the user's own git credentials, and reads
    the tree from git objects with the same reader as `repo:`. GitHub, GitLab and self-hosted servers all work. A
    tag is a version.
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
  - `collide pack update <name> [--to <ref>]` moves a `git:` pack, shows the diff of its manifest, workflow graph
    and hooks, and asks for trust again. For a `git:` repo pack it changes the lock, which the user commits. A `repo:`
    pack changes by a commit alone.
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
    - `wait`: a hook polled at an interval until it returns `ok` or `fail`, or a timeout passes (CI checks). No
      agent runs while it waits, and the time counts as waiting;
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

prompts:
  build: briefs/build.md.j2                    # carries launch.review_budget; `iter timings --write` gets --run
  review-brief: briefs/review-brief.md.j2      # instructs the orchestrator to write a fresh brief per round
  adjudicate: briefs/adjudicate.md.j2

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

- **App:** several boards, one switcher.
- **Board:** repo root, lines (0.12; the default line is the base ref at the board root), source configuration,
  saved views.
- **Line:** ref, root checkout, the snapshot commit used by each run, applied packs, default workflow, trust entry,
  line lock. `record` refuses to commit when a line root is not on its line's ref.
- **Task (cache):**
  - key `source:shortId`, plus template fields, aliases and, for ref-scoped sources, the path in each checkout;
  - title, body, status + category, deps, labels, collections, source fields;
  - provenance: author and trust level;
  - `kind`, typed `links` (`type`, `to`, `blocking`), `assignee`, and checklist fields (owner, 2026-11-13, for
    planning);
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
  - `ask_human` events with their cause.
- **History:** prompts and hooks see `history[]` (earlier phases: kind, round, heads, artifact paths, verdicts) and
  `round.base` / `round.head` / `chain.base`.
- **Workspace:** a git checkout (`path`, `gitDir`; 0.1). It lives longer than any run, and keeps its workspace state
  (0.5). Kinds:
  - `task`;
  - `sandbox`;
  - `line` (a line root);
  - `experiment` (an experiment root, or an arm's task workspace).

  Lifecycle:
  - States: `provisioning → ready → in_use → idle → removable → removed`, plus `failed`. `failed` resumes through
    the `resume` hook.
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
- A diff that touches `.collide/`, `.claude/`, a pack path or a pack's `protectedPaths` is flagged in review.
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
    difference before it is trusted (0.12).
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
- CEL runs on untrusted task data with a cost limit and an expression size limit (S5). CEL has no unbounded loops,
  but its macros nest superlinearly.
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
user per board, and are not ref-scoped. A prompt becomes a task, its first line the title. Their statuses are the
categories themselves (`ready`, `active`, `review`, `done`, `dropped`); operations `create` and `setStatus`. Moving a
scratch task into another source is deferred.

Why the plan adapter parses files itself:
- Measured R2: `./plan tasks --format json` returns only `id, status, proposal, epic, title, file`. It returns no
  `depends_on`, body or `reviewed_by`, for 1567 tasks in 0.96 s per call.
- Closure needs the dependencies, and a reparse per change per checkout is too slow.
- Parsing frontmatter, as the prototype board's generator does, gives the dependencies and reparses only the
  changed file.
- Writes still go through the plan CLI, so plan's own rules apply. These include refusing illegal transitions and
  regenerating epic tables.

**Protocol** (JSON-RPC over stdio):

- `initialize` receives the pack's settings (`config`, 0.7) and returns:
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
  - `links`: the link types it holds, their direction, and whether they block
- `list({ checkout?, cursor? })` returns tasks and collections. `get({ id, checkout? })`.
- `watch({ checkouts })` sends notifications: `task.collision`, `source.stale`, and (owner, 2026-11-13; S11):
  - `task.changed {key, version, from, to, diff, cause, actor?, automated?}`, where `cause` is `head | worktree |
    remote | journal | startup`, `actor` is the source's own when it is known (GitHub events, the beads journal)
    and `automated` is GitHub's `wasAutomated`;
  - `task.renamed {from, to, checkout}`, with history, runs and `latestRun` following the key;
  - `task.removed {key, cause}`, where `cause` is `deleted | transferred | unknown`;
  - `task.invalid {path, checkout, reason}`, which core turns into a Queue item.
- On start, core reconciles each source against its persisted cache and emits one change per item, with
  `cause: startup`.
- `apply({ op, id?, args, version, checkout? })` returns one of:
  - `ok`, with the new task, its version, and the files the write touched (what `record` commits). It also returns
    `items[]`: every item the op changed, with its version, from and to
  - `conflict`, with the current task
  - `refused`, with the source's message, e.g. plan's "illegal transition"
- Optional capability `preview` takes `apply`'s arguments, writes nothing, and returns `items[]` and `files`.
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
- Every write names its checkout. The UI defaults to the selected line's root.
- **No writes into a checkout while a phase is running there.** They queue until the phase ends. A builder stages
  by explicit path, so a Collide edit would otherwise ride into its commit. Writes into a phase's checkout are also
  what `readOnly` and `dev worktree rm` (which refuses dirty trees) would trip on.
- A write leaves uncommitted changes, which can be several files: `plan set` regenerates epic tables. The UI shows
  uncommitted task edits per checkout.
- **Committing.** Collide never commits task files itself. The pack's `record` hook does, under the line lock.
  - It commits exactly the files that `apply` reported or the phase output, never the whole glob. An unrelated edit
    of the user's under `arch/` stays uncommitted.
  - Workflow ops (`on_start`, `on_end`) record automatically.
  - Edits made in the UI record when the user presses Record.
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
  - An evaluation error excludes the task and is counted on the view, not hidden.
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
    - A question holds its own decision once it is resolved (`resolved_by: human:<date>`).
    - Only a person accepts, defers, rejects or resolves. `collide/plan` refuses those ops from a run
      (`actor.kind: agent`), and refuses `accepted` while a question the proposal includes is open. An agent phase
      that writes a planning file's `status` or `by:` line itself fails at phase end, as an edit to checklist state
      does.
    - There are no decision or deferred files. The planning view's "Decisions made" lists the Decision sections of
      accepted proposals and resolved questions. A team system keeps its own decision kind through the role mapping;
  - **ids** (owner, 2026-11-13): a kind prefix, a number and a two-letter suffix, as in `ts-0012-ua`. Epics use
    three digits, the other kinds four. Names on disk are `<id>-<slug>`. An optional `alias` (`S5`) is shown, never
    linked to:
    - the number is the next free one for that kind across every ref Collide can see (local branches and fetched
      remotes). It keeps creation order and is the short handle. It may repeat after two branches merge;
    - the suffix makes the id unique, so a merge never renumbers a file or rewrites a link. `collide task 12`
      resolves when one item has that number; otherwise it lists the matches and asks for `12-ua`;
    - the suffix is letters only, without `i`, `l` or `o`, so it never reads as part of the number. Ids match
      `^[a-z]{2}-[0-9]{3,}-[a-z]{2}$` and are always strings in YAML (S5). Two files with one id are a
      `task.invalid`;
  - two roots: a private one in the pack's data directory, and a shared one at the configured path,
    `.collide/planning/` by default, committed as views are;
  - the path is set in `.collide/config.yaml` and is part of the trust hash, so a change asks for trust again. A
    path outside the repo, under `.git/` or over Collide's own files is refused;
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
  This plan keeps the design; each stage's **Status** line names its epic, and the items hold the status. Only the
  next milestone is split into items; later stages stay outlines until they start (0.15).
  - Kinds so far: `epic` and `task`.
  - A task's statuses are the five categories themselves (`ready`, `active`, `review`, `done`, `dropped`), as for
    scratch tasks.
  - Links: `blocked-by`, which blocks. The folder gives the epic. A task is **ready to start** when its status is
    `ready` and every `blocked-by` target is `done` or `dropped`. `collide tasks --ready` lists those. A
    `blocked-by` cycle is reported, and none of its items is ready.
  - Frontmatter: `id`, an optional `alias`, `kind`, `status`, `title`, `owners` (the claim) and `links`. Body
    sections: Goal, Success criteria, Tests, Notes.
  - Until the planning slice lands, items are written by hand, and an agent picks a ready one by reading their
    frontmatter. The slice must read every hand-written item with no `task.invalid`.

### 0.11 Interface direction

Collide is not a chat app. Its objects are tasks, runs, phases, gates, workspaces and decisions; conversation is a
detail inside a phase. No chat bubbles, no centred conversation column, no sidebar of chats.

Three surfaces, one keystroke apart:

- **Field:** the corpus through a view, laid out as a board or a run order.
  - Tasks are the primary objects. A task being worked on is lit in its run's colour (the prototype board's lit
    border).
  - The main action is: select tasks, then Launch.
  - A read-only Field ships first (Stage 6); Launch arrives with the run UI (Stage 9).
  - An empty board still offers a way in: a prompt box (type a task, then Launch), **New session**, the sources
    Collide detected, and Add a pack.
  - **Home is the Field** (owner, 2026-11-13; from the Field-first mockup). Its weakness is that a decision sits one step
    from its cause. The menu bar badge and notifications cover most of that (below).
- **Tracks:** every live run as a horizontal track of phase segments (provision → build → brief → review ×2 →
  adjudicate → fix → close).
  - Gates show as checkpoints, with time and spend meters against budget.
  - Workspace provisioning is a segment with its own progress: orchard's takes minutes (worktree 230–330 s, map
    build about 1500 s, measured R1). Its progress comes from the pack's `progress` report lines (0.5).
  - Parallel runs stack, like a mixing desk or a CI pipeline view, so the whole fleet's state fits on one screen.
  - **A real-time axis**, not equal-width phase columns (owner, 2026-11-13; from the Tracks-first mockup). A
    segment's width is its duration. The axis has:
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
    - the exact command or arguments, with untrusted ones flagged (0.9);
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
  - Keyboard triage: `j`/`k` to move, `a` approve, `d` deny, `o` open the context (from a Queue item, its run's track
    at the moment of the decision). S6 settles what `a` and `d` mean for each kind.

Details:

- **Task drawer:** body, dependencies (both directions), collections, per-checkout state with divergence and
  collisions, other lines' states, the task's runs, and the source's operations as forms. Create opens the same
  form.
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
  - a table: model time, each tool by name (MCP tools grouped by server, a subagent's calls nested under it), hooks,
    permission waits and Queue waits, with calls, total, share, median, maximum and errors. Bash splits by the
    command's executable (`cargo`, `pnpm`, `git`), which 0.13's privacy rule keeps;
  - a timeline with one row per tool type on the run's real-time axis. Colour stays reserved for state, so rows, not
    hues, tell the types apart;
  - wall-clock time and summed tool time shown apart, because parallel calls overlap;
  - filters that combine: tool type; **minimum duration**, how long one call took ("every call over 2 s"); and
    **time window**, a span of clock time ("every call between 11:00 and 11:10"), dragged on the axis, typed, or
    set by picking a phase. A call matches when it overlaps the window (owner, part L). Calls, median, maximum and
    the minimum-duration filter use the whole call; total and share count only the time inside the window. The
    chips write CEL over spans, as the Field's chips do for tasks;
  - a table row filters the timeline to that tool, and a bar scrolls the output pane to that call;
  - each phase's times are labelled exact (telemetry) or approximate (stream timing). S8 measures the difference.
- **Talking to an agent is the exception.** You either intervene (send a note to the running phase) or take over
  (interactive `--resume` in a terminal).
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
    (`includes`), for tasks, spikes and questions;
  - a choice the owner makes in the terminal goes into the proposal's Decision section as text. It is a decision
    only once a person accepts the proposal;
  - a choice to make away from the terminal, or one of several at once, becomes a question item. It appears in
    "Decisions needed" and is answered in the Queue, whether or not the run is live. The agent reads the answer
    with `collide task <id>`;
  - the agent moves the proposal to `proposed` when it is ready. Ending the session leaves the proposal as it is.
    Plan… on a draft proposal starts a new session that reads the proposal and its items, not the old
    conversation;
  - everything stays private until Share (0.10). A diff in the session's worktree goes to review, as for New
    session.
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
  - the user hooks and plugins each phase will load, with flags on those that change behaviour (0.3).
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
- **Lines.** Each board has a line switcher. Launch lists the selected line's workflows, and a run's track is tagged
  with its line.
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
    the verdict rule's answer, labelled "on this task".
- **Visual language:**
  - dense and keyboard-first, with a command palette
  - monospace for ids and refs
  - colour reserved for state
  - motion only for live state: working pulses, blocked holds steady

  The particle-physics flavour of the name may shape the brand; it does not shape navigation.

S6 tests this direction with a clickable prototype before Stages 6 and 9 build it.

**Planning** (owner, 2026-11-13; `collide/plan`):
- A Queue lane can be fed by a view (`queue: true` on the view). Its items are answered through the source's ops.
  `collide/plan` ships "Decisions needed": open questions and `proposed` proposals. Several decisions at once are
  question items answered there, with a `wait` phase holding a headless run.
- **Planning view:** decisions made, spikes outstanding, results with their summary tables, and decisions needed.
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

- **Per line:** the `packs` list (its order and sources), the default workflow and the profiles.
  - Everything else in `board.yaml` is board-level and comes from the default line: sources, the status mapping,
    the checkout filter and views.
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
  - Agents still work only in workspaces.
  - A workspace's edits reach a line only through `integrate`, and `protectedPaths` flags them in review.
- **Why not one board per branch.** Lines share one corpus, one set of views and one set of sources. Separate boards
  would duplicate all three and lose cross-line task state in the drawer.
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
  - Resource samples are tied to spans.
  - Everything is stored in SQLite next to the event log, per machine. Nothing leaves the machine in v0.
  - **Sleep is not work.** A laptop that sleeps mid-run would otherwise add hours of "active" time. The owner
    records sleep intervals and counts them as waiting. On macOS the uptime clock stops during sleep while the
    continuous clock does not, so the gap between them is the sleep (inferred; S8 checks it).
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
  - `full`, opt-in per board or per run: resource samples, per-process attribution, network, device GPU,
    container and detached-process stats, pack span import, and the machine part of the fingerprint. S8 measures
    its overhead, and the UI shows it.
- **Sources:**

| source | gives | accuracy and granularity |
|---|---|---|
| Collide's own spans | phases, hooks, gates, provisioning, `record`, `integrate` | exact |
| Waits | Queue decisions, permission prompts, usage limits, `wait` phases, the concurrency cap, the line lock, queued writes, machine sleep | exact; shown apart from active time |
| Agent stream (stream-json) | turns, tool calls, tokens (cache reads, and cache writes split 1-hour and 5-minute), cost, wall and model time per turn, context size per response, compactions (`system/compact_boundary`) | input and cache tokens exact, counted once per `message.id`: the stream repeats one usage per content block (measured S9). Output tokens on those rows are partial, so per-turn totals come from `result.usage`; a compaction call, which `result.usage` leaves out, comes from `model_usage_since_last_result` (measured S9). A result's `duration_ms` covers its turn, but `duration_api_ms` (time in the model) and `total_cost_usd` are running totals over the process, so a turn's model time and cost are differences between results. A result with `num_turns: 0` (a `/compact`, a blocked prompt) reports `duration_api_ms` 0, so the difference is taken from the last result with turns (measured: S9's recordings). Timings approximate, because the owner tails log files |
| Interactive sessions (New session, take-over) and `agentWork` phases | the same as the agent stream, plus per-call cost, time to first token, and the title and suggestion calls the transcript lacks | Claude Code's telemetry to Collide's local receiver (measured S2: tokens equal to the transcript's). A tool's `duration_ms` is the whole tool call, spawn included, not the command alone (a 2.0 s command, a 3.3–4.4 s call). The transcript is the fallback when the user's own endpoint is set. Only the phase's session ids count, followed across `/clear`. A `claude` that an agent starts from its Bash tool does not report, because Claude strips `OTEL_*` from its tools' environment (measured S2) |
| Past sessions (opt-in import; owner, 2026-11-13) | usage per response, tool names, timings, command shapes, compactions; never prompt, reply or tool content | from `~/.claude/projects/<repo key>/`, one usage per `message.id`: transcripts repeat usage per content block (measured in 84 of S1's 105 sessions). A baseline only; the contract test pins the format |
| Process exit totals (`wait4` rusage, from the shim) | CPU time and peak memory | exact for every child started through the shim, including across a daemon restart (measured S3). A child's total includes the descendants it waited for; orphans are excluded. Peak memory is the largest single process's, not the tree's |
| Process sampling (S8 picks the interval) | CPU, memory and disk I/O over time (`proc_pid_rusage`; its CPU fields are mach time units, converted with `mach_timebase_info`, 125/3 here, measured S3) | process-level. A process is attributed to the span in its `COLLIDE_SPAN`; tool subprocesses are matched to tool calls by parent pid and time overlap. Misses processes shorter than the interval |
| Disk space | size of each workspace and each cache directory | per workspace. R3 measured a provisioned orchard worktree at 4.1 GB |
| `nettop -P` | network bytes per process | measured: per process, without root |
| GPU | device utilization and memory (Stage 13); per-process GPU time (spiked in S8, collected from the first local-model adapter) | measured without root: `ioreg -r -c AGXDeviceUserClient` gives `AppUsage.accumulatedGPUTime` per client, with the owner's pid in `IOUserClientCreator`; `ioreg` IOAccelerator gives device utilization. In v0, Claude's inference is remote, and orchard's Ollama runs in Docker with no Apple GPU (measured R3), so per-process GPU matters only from the first local-model adapter |
| Containers and detached processes | lane CPU, memory, I/O, network (Docker API); the same for host processes a pack starts outside Collide's tree | per container or process. The pack's `metrics.processes` hook returns a workspace's container ids and pids. orchard: lane containers carry only a per-clone label, so the pack reads each lane's record, which also gives its host server and UI pids (measured: the dev CLI's lane records). Containers need the hook because Docker Desktop runs them in a VM the host cannot see (measured: context `desktop-linux`) |
| Pack span import (`metrics.import`) | e.g. orchard's `iter-timings.jsonl`: stage, label, seconds, verdict, load average | Selected by the run's worktree paths (or the arm's own file), never by label alone. iter stamps sub-calls with their parent's start, so they nest as durations without a timeline position. Existing history (3545 lines, measured R3) is backfilled as a baseline without resources |
| Reported (`COLLIDE_REPORT`, 0.5) | spans, metrics and progress from hooks, gates, `script` and `wait` phases and their subprocesses, live; never from agent phases. `usage` lines from `agentWork` script phases (0.3) | as accurate as the reporter; labelled "reported". Reported usage is shown beside measured tokens and cost, never added to them. A tool that reports live makes its import a fallback |

- **Claude Code's own telemetry** (OpenTelemetry). The 2.1.323 binary contains `CLAUDE_CODE_ENABLE_TELEMETRY`,
  `OTEL_METRICS_EXPORTER` and `OTEL_LOGS_EXPORTER` (measured).
  - Interactive sessions and `agentWork` phases use it, with a local receiver (measured S2; owner, 2026-11-13).
    Headless phases keep the stream unless S8 shows better timings.
  - Collide sets the export intervals short (`OTEL_LOGS_EXPORT_INTERVAL`, `OTEL_METRIC_EXPORT_INTERVAL`; defaults
    5 s and 60 s). At 1 s, log records arrived p50 978 ms after the event (measured S2).
  - Records are attributed by `session.id` and by `OTEL_RESOURCE_ATTRIBUTES` (`collide.run`, `collide.phase`). Each
    run's processes carry a token in `OTEL_EXPORTER_OTLP_HEADERS`, and the receiver refuses records without one.
  - Every record carries the account's identity (`user.email`, account and organization ids; measured S2). The
    receiver drops those attributes on arrival, before anything is stored.
  - `OTEL_LOG_USER_PROMPTS`, `OTEL_LOG_TOOL_DETAILS` and `OTEL_LOG_TOOL_CONTENT` are forced off (all measured in the
    binary).
  - Inherited `OTEL_*` variables are stripped.
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
      time is kept apart from wall-clock time, because parallel calls overlap (part K). A refused call is never
      counted as a tool error;
    - per hook: calls, blocks and failures (exit 1 or a timeout), from `hook_response` (0.3).
  - Seven detectors work over these shapes, never over content: cold return, heavy start, compaction mid-task,
    oversized tool output, the same command failing again unchanged, a hook that failed open, and a call to a tool
    the session does not have. Each finding says what happened, what it
    cost (measured) and what a change would save (estimated).
    - Oversized tool output shows in the tool result: `persistedOutputPath` when Bash output was cut and saved to a
      file, and the error `exceeds maximum allowed tokens` (with `is_error: null`) when an MCP result was dropped
      (measured S9). That error is Claude's own fixed text, not tool content.
    - A hook that failed open is a `hook_response` with exit 1, or a `hook_no_verdict`, where the tool then ran: a
      guard that did not guard. A call to a missing tool costs a turn and usually means the agent definition and
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
    - the Collide, claude and node versions;
    - the model each alias resolved to;
    - the context options in effect, and the names of the user hooks and plugins the phase loaded (0.3);
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
- **Volume and retention.** S8 estimates store growth at the chosen interval. Retention is set per board, default 90
  days.

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
  - The primary metric comes only from what Collide measured or from spans that hooks reported. A reported `metric`
    line is never primary, because the code under test may have written it.
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

- **Spikes come first.** S1, S3, S5 and S7 decide the Claude launch line, the process owner, the YAML and CEL crates
  and the shape of `AgentAdapter`. An outline written before them would be rewritten.
- **Pass 1, the skeleton (Stage 2).** Two parts (owner, 2026-11-13, choosing R4's middle option):
  - **Every cross-process contract, in full:** the hook protocol (stdin, result file, report lines), the provider
    and observer protocols, the JSON-RPC between UI or CLI and the owner, the event log, and the schemas for
    `pack.yaml`, `board.yaml`, `config.yaml` and views. That includes the point and event catalogues (0.4) and keys
    used only in Milestones C and D, so a manifest written today keeps validating.
  - **Code outlines for Milestones A and B** (Stages 3–10):
    - Rust crates and modules, traits, types, state enums and function signatures;
    - the SDK's functions and types;
    - the shell's `rpc` command and event channel;
    - the React component tree for Field, Tracks and Queue, with typed props.
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

  A stage is complete when CI counts zero stubs carrying its tags.
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

Ends usable from the CLI and a read-only desktop board:
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
- **S5 schema and expressions.**
  - Rust → JSON Schema → TS.
  - Choice of YAML crate and CEL crate.
  - YAML hazards: `no`, `0.10`, unquoted `{{ }}`.
  - The CEL cost limit: a nested-macro expression over 1567 tasks is stopped within the limit.
- **S6 interface prototype.** A clickable prototype of Field, Tracks and Queue (0.11) over fake data: fake-agent
  streams and an Orchard-shaped corpus. Nine scenarios:
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
  2026-11-13):
  - **Run identity against state colour.** Colour is reserved for state (0.11). How is one run told apart from
    another: by id, lane or position? The Field's "lit in its run's colour" waits on the answer.
  - **`a` and `d` per Queue kind.** What approve and deny mean for each kind: permission, failed gate, review,
    budget, and so on.
  - **`o` from a Queue item** opens its run's track at the moment of the decision.
  - **Where the line switcher lives.**
  - **"review ×2" means two reviewers, not two rounds.** The mockups showed rounds. Correct the prototype's fake
    data.

  The owner sets time and error thresholds before the sessions. Participants are the owner and at least one person
  who did not design it, using it for the first time.
- **S7 `AgentAdapter` against ACP (paper check).**
  - Map the trait's launch spec, events, permission answers and capabilities onto ACP messages.
  - Run `opencode acp` once by hand with one Ollama model.
  - Record every trait member that ACP cannot express, and change the trait before the skeleton (Stage 2)
    outlines it.
  - Nothing ships.
- **S8 metrics sources.** On an orchard `iter` run and a lane test:
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

**Success Criteria**:
- Ten decision records (S1–S9 and S11), with measured numbers.
- The process owner is chosen, and the record says whether runs continue between phases with the app closed. The
  Claude launch line is fixed, including where settings come from. The snapshot + SDK path works.
- S1, S3 and S4 run first. Their records are read against this plan before the other spikes start (owner,
  2026-11-13). Done: revision 10 applies them, S2 and S9 (part E), and S11 (part J). S5 and S7 run now, then S6
  and S8.
- Revision 11 applies S5–S8. Then an adversarial review, **R5** (Fable and Opus, read-only, as in R1–R4), attacks the
  finalized plan, and the owner's decisions on its findings are applied. No product code is written before that;
  spike code is throwaway and exempt (owner, 2026-11-13).
- The non-owner participant completes each S6 scenario within the thresholds. The record lists what changed in the
  prototype, or says "none" with the reason.
- The trait has no member that S7 found ACP cannot express, or each such member is marked as a capability.
- S8 names a sampling interval whose overhead is below 1% of one core. It states each resource's granularity and
  what it cannot attribute.

**Tests**: Each reproduction script runs from a clean checkout and prints its measurement.
**Status**: In Progress (S1–S4, S9 and S11 complete; S5 and S7 have results awaiting the owner). Items: epic
`ep-001-ef` in `.collide/planning/epics/`; records in `.collide/planning/spikes/` (part L).

## Stage 2: Skeleton

**Goal**: Every cross-process contract is defined in full, and Milestones A and B exist as outlines that compile and
do nothing (0.15). One piece works for real: the planning slice, so Collide tracks its own build from here on (0.1;
part L).

- Repo layout:
  - the `core` and `host` crates, `app/src-tauri` and `app/` (React);
  - `packages/pack-sdk`;
  - two fixture packs: a review loop (`fixtures/packs/loop`) and an issue → PR → CI shape (`fixtures/packs/pr-ci`);
  - a fixture copy of the reference pack's manifest and hooks. The pack itself lives in the Orchard repo (0.7);
  - `packs/collide`, the built-in pack (0.4).
- Contracts in full, with JSON Schema and TypeScript types generated from them:
  - `pack.yaml` (including `config`, workflows as transition tables, and the keys used only in Milestones C and D),
    `board.yaml`, `config.yaml`, views, profiles, `packs.lock` with its `source` field, the user's `packs.yaml`, and
    the `.collide` pointer file;
  - the hook, provider and observer protocols, and the report line format;
  - the shim protocol, versioned on its own, and `exit.json`;
  - the JSON-RPC between UI or CLI and the owner;
  - the point and event catalogues (0.4);
  - state, events and spans.
- `core` outlines for Milestones A and B: the traits (`AgentAdapter`, `HookRunner`, `TaskProvider`), domain types,
  state enums, the workflow engine and implementation resolution, as signatures with doc comments and tagged
  stubs. S7's trait changes are in.
- `host` outlines: the Claude adapter, `collide-daemon` with its launchd start, `collide shim`, the store, the hook
  executor, the provider supervisor, the sampler and the CLI commands.
- `@collide/pack-sdk`: `defineHook`, `defineProvider`, `defineObserver`, the `ctx` object and the generated types,
  with stubbed bodies.
- The shell's `rpc` command and event channel. The React component tree for Field, Tracks and Queue, following S6,
  with typed props and placeholder rendering.
- Test doubles as outlines: a fake adapter that emits normalized events, a fake provider, a fake hook runner.
- CI: build, lint, typecheck, the stub count per tag, the dependency-direction check and the no-names check.
- **Planning contracts** (owner, 2026-11-13), defined first (part L): the provider schema's `kinds`, op effects,
  `links`, `assignee` and checklist field type; fan-out `instances:`; the run budget; the five new task points with
  `change` and `latestRun`; `points:`, `views:` and `checklists:` in `pack.yaml`.
  - From S11 (owner, 2026-11-13): `change.diff`, `batch` and `effects`; `change.via` and `actor.kind: agent` with
    `run` and `phase`, reserved now so run-made writes need no later schema change; `apply` `items[]`; `preview`;
    the `watch` notification shapes and `task.invalid`; the run-diff points at accept.
  - The skeleton fixes the contracts only. The workflow-engine stage builds the run-diff points, and the
    second-pack stage builds attribution for global sources.
- **Planning slice** (owner, 2026-11-13; part L), built for real as soon as the planning contracts, the provider
  protocol and the pack SDK's outline exist, ahead of the rest of this stage:
  - `collide/plan` loads as a built-in pack, from the build;
  - the markdown provider's shared root at its default path, `.collide/planning/`, read in 0.10's layout (only
    the paths it names): kinds `task` and `epic`, epic membership from the folder, `blocked-by` links, and the ops
    `create`, `setStatus`, `link`, `claim` and `unclaim`. `create` writes into the epic's `tasks/` folder and takes
    the next number for the kind across every ref Collide can see (0.10);
  - provider protocol v0's `initialize`, `list`, `get` and `apply`, and a working `defineProvider` in
    `@collide/pack-sdk`;
  - the CLI's `collide tasks [--ready] [--kind <kind>] [--json]`, `collide task <id> [--json]` and
    `collide task <id> <op> …`. `<id>` may be the short form (`12` or `12-ua`) when it matches one item. Until
    the headless-core stage's daemon exists, the CLI starts the provider itself; the corpus stage moves it behind
    the provider supervisor;
  - left to the corpus stage: the private root, the layout setting, views and CEL, `watch`, trust and the
    persisted cache.

**Success Criteria**:
- `cargo build`, `cargo clippy`, `tsc --noEmit` and the app build pass.
- Every stub carries a tag, every tag belongs to a stage, and CI prints the count per tag. CI also lists each
  catalogue point with the stage that makes it fire.
- The built-in pack's, the reference pack's and both fixture packs' manifests validate against the generated
  schema. A manifest with an unknown key fails.
- Serde round-trips pass for every protocol type, and the generated TypeScript types match the schema.
- A test fails if `core` gains a Tauri, SQLite or process dependency.
- A seeded `orchard` string in `core`, and a seeded `CLAUDECODE` or `ANTHROPIC_` string outside the Claude adapter,
  each fail CI.
- A provider with one kind and no links validates unchanged. `collide schema` lists every new key and point. A
  derived point that names an unknown core point, or a gate bound to an `after_*` point, is refused at load.
- Planning slice (part L):
  - `collide tasks --json` lists every hand-written item in Collide's `.collide/planning/`, with no `task.invalid`;
  - `collide tasks --ready` equals the ready list checked by hand;
  - `collide task <id> setStatus active` changes only that item's `status` line (`git diff`);
  - a task file moved into another epic's folder is listed under that epic, and its links are unchanged;
  - with two tasks numbered 12, `collide task 12` lists both and asks for the suffix;
  - from here on, Collide's own build tasks are read and updated through the CLI.

**Tests**: Schema generation; manifest validation (all four packs, plus invalid fixtures); serde round-trips;
dependency-direction test; the no-names check with its positive controls.
Planning: derived-point validation; the new keys' schemas; the `change` and `watch` shapes. The slice: provider
contract tests over fixture items; the ready rule (blocked, unblocked by `done` and by `dropped`, a cycle); ids
(the next number across refs, including a fetched remote branch; a repeated number after a merge; short-form
lookup; two files with one id is a `task.invalid`); epic membership from the folder; an item with an unknown kind or
a broken link is counted, not dropped.
**Status**: Not Started. Items: epic `ep-002-va` in `.collide/planning/epics/` (part L).

## Stage 3: Headless core (CLI, no UI)

**Goal**: `collide run` starts one Claude phase on a task given as a file, in a new git worktree, and records it.

- `core` state machines (Run, Phase, Workspace), with the terminal states and pauses of 0.7 and 0.8, and `lost`. The
  `host` Claude adapter per S1.
- **The daemon and the shim** (0.2), speaking the skeleton's JSON-RPC:
  - the shim is synchronous: `std::process::Command` with `pre_exec(setsid)`, then `libc::wait4` on the pid. An
    agent's exit is never taken from std's or tokio's `wait`, and a tokio `Child` is never dropped (measured S3);
  - the daemon reaps shims with tokio, because it reads totals from `exit.json`;
  - on-demand start under the lock file, as a one-shot launchd job on macOS; the base environment captured once;
  - the durability rules of 0.2.
- `AgentAdapter` capabilities per S7. `models.yaml` loading and alias resolution, including `model.name`.
- File-backed child logs; SQLite store + event log; per-client socket token; version handshake.
- Built-in workspace: `git worktree add`, returning `{ path, gitDir }`; refuses to remove a dirty or unmerged
  worktree.
- Permission requests answered from the CLI (`--permission-prompts host`).
- Stop per 0.5: the ppid walk, the marker scan through libproc, their descendants, the order, the hooks' `detached`
  records and the survivor report.
- **The adapter's declarations (0.3):**
  - its environment is stripped from everything Collide starts (Claude: `CLAUDECODE` and every `CLAUDE_*`);
  - a usage or rate limit becomes a `limited` event. The phase pauses (`paused: usage limit`) and resumes by
    session id at the reset time, or when the user resumes it.
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
  (the fake alone cannot detect it).
- **Dogfood** (part L): `collide run` takes an item from `.collide/planning/` as its task file.

**Success Criteria**:
- A real run on the Orchard repo in a fresh worktree.
- One of Collide's own build tasks runs through `collide run` in a fresh worktree of Collide (part L).
- The owner restarts mid-run, and the run continues or resumes.
- Stopping leaves zero processes, including a setsid'd grandchild, a double-forked orphan and a child that ignores
  SIGHUP and SIGTERM (`ps`). The test children are python3, not platform binaries. A double-forked `/bin/sleep`
  orphan that its hook recorded is stopped by the cleanup hook; an unrecorded one appears in the survivor report
  (0.5).
- A phase that ends while the daemon is down keeps its exit code and totals, and the next phase starts with the app
  closed.
- A shim killed with SIGKILL makes its phase `lost`, even though the agent reported success. A SIGSTOPped shim is
  reported as unresponsive and does not stall other runs.
- When a shim is killed, a child that ignores EOF and hangup is stopped as its phase turns `lost` (S2).
- The daemon started by the one-shot launchd job has `USER`, `HOME` and `PATH`, and a real `claude -p` phase runs
  under it.
- A daemon started from a terminal survives closing that terminal.
- Logging on a full disk crashes neither the daemon nor a shim, and an unreadable store is moved aside and reported.
- A permission request is answered from the CLI.
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
- A sleep gap injected through fake clocks counts as waiting, not active time.

**Tests**: State-machine unit tests; store migrations; hostile-process tests adapted from herdr-core (SIGKILL and
SIGSTOP of the shim, children that ignore HUP and TERM, python3 fixtures, a recorded `/bin/sleep` orphan); a
`/dev/full` logging test; store quarantine;
alias resolution; env stripping; limit pause and resume; usage counting over repeated content blocks and a
compaction; per-turn differences of running totals; missing values stored as null; error results'
spend; other model calls; settling on exit and end of output in either order; the hook-blocked prompt;
refusal classification and hook outcomes;
context-option validation; sleep accounting with fake clocks; span-tree tests
against fake-Claude streams; fake-Claude end-to-end; real-claude contract test (opt-in in CI).
**Status**: Not Started. Items: epic `ep-003-da` in `.collide/planning/epics/` (part L).

## Stage 4: Pack loading, settings, plan adapter, corpus and views

**Goal**: Tasks come from a ref-scoped source that can be read and written, are combined into a corpus, and are
filtered by views. Everything loads from a trusted snapshot, with the pack's settings.

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
    materialized by one code path. `git:` fetches into a bare mirror under `~/.cache/collide/git/` with the user's
    git credentials;
  - user packs in `~/.config/collide/packs.yaml` and their lock, switched on per board;
  - trust per source (0.9);
  - working copies (0.7): `collide pack link <name>` with no folder, and new `.collide/packs/` folders in the
    user's checkout offered as working copies;
  - `collide pack add|link|enable|disable|list`.
- **Settings (0.7):**
  - three layers: the schema's defaults, then `~/.config/collide/config.yaml`, then the snapshot's
    `.collide/config.yaml`, which wins except for keys marked `overridable`;
  - profile files from the snapshot's `.collide/profiles/`, `~/.config/collide/profiles/` and packs' `profiles:`,
    applied as the top layer by `collide config get <key> --profile <name>`. A user profile that sets a key not
    marked `overridable` is refused, naming the key;
  - per-user-only keys refused in the repo file, naming the key;
  - secrets from the OS keychain only (`collide config set --secret`). A secret in either file is a load error;
  - Collide's own settings through the same layers;
  - `collide config get <key>` shows the value and the layer it came from.
- **Pack data:** one directory per pack per board, passed as `COLLIDE_PACK_DATA` (0.7).
- **Provider protocol v0** per 0.10, with the pack's settings passed to `initialize`. A provider supervisor restarts
  providers with backoff, isolated from the process owner.
- `@collide/pack-sdk` v0, compiled: `defineProvider`, `defineHook`, `ctx.config`, `ctx.dataDir`.
- **`plan` adapter (TypeScript):**
  - parses frontmatter per checkout and keys tasks by (shortId, path), and declares the `labelId` template field;
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
  there, and says why.
- **Corpus** with status categories, identity and collisions.
  - The corpus cache persists in the store. On start, core reconciles each source against it and emits one
    `task.changed` per item with `cause: startup` (0.10; S11).
  - Links are resolved one hop out, both directions and across sources, into `tasks[].links` (0.5; S11).
  - An unparseable item (conflict markers, a sync conflict file) emits `task.invalid` and counts on its view (S11).
  - The plan adapter declares `preview`, using the plan CLI's `--dry-run` (S11).
- **Views** from the snapshot's `.collide/views/*.yaml` and from personal views: the set algebra, normalized fields,
  closure per the prototype board's generator, and per-view error counts.
- **CLI:** `collide init`, `collide tasks [--view <name>] [--checkout <path>]`, `collide task <id> [--json]` (one
  item: fields, status, links one hop out, version and latest run; part K), `collide task <id> <op> …`,
  `collide config get|set`, `collide pack …`.
- **Planning** (owner, 2026-11-13): the rest of the markdown provider, after the skeleton's slice: the private
  root and the layout setting; kinds, op effects and links in the corpus; `collide/plan`'s record parser (claim
  blocks) and `roles` mapping; pack views, with the slice's `--ready` as a built-in view. The slice's CLI moves
  behind the provider supervisor. The Orchard pack's plan provider reads ADRs, spikes and questions.

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
- An edited provider script, view, `board.yaml` or `config.yaml` on a branch is not used. A changed hash asks again.
- A key set in both the user's file and `.collide/config.yaml` resolves to the repo's value, and `collide config get`
  names the repo as its layer.
- A secret in either file fails the load and names the key. A source account in the repo file is refused.
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
- A scratch task created with `collide task new` is listed for that board only.
- Planning:
  - Collide's spike records in `.collide/planning/spikes/` list in `collide tasks --view spikes --json` with their
    statuses (`concluded`, or `results` while the owner has not decided), and no item from inside a spike folder
    other than its record;
  - over the Orchard repo, the Decisions needed view lists the same question ids as the human queue in
    `plan status --format json`;
  - an ADR's `evidence` link shows from the spike's side as "evidence for";
  - a malformed claim block is a counted parse error on the view;
  - two branches that each create ten items in the shared root merge with no id collision;
  - a private-root item never appears in `git status`;
  - a layout path outside the repo or under `.git/` is refused with the reason.

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
  - the cost limit.
- Snapshot-not-workspace test; materialization refusals and the re-hash; id and template-field validation,
  including a value that fails the safe-token rule.
- Settings: layering, per-user-only keys, the secret rule against a fake keychain, the pointer file (a `dir:`
  outside the repo is refused), `overridable` keys, profile layering.
- Pack sources: resolution for each kind; a `dev:` folder's tree id equals `git write-tree` over the same files;
  the name clash; trust per source.
- Root busy, against a fixture repo stopped mid-rebase.
- Planning: parser fixtures, with strict plan CLI frontmatter left untouched; provider contract tests against
  markdown and plan fixtures; id generation; view counts; layout checks.
**Status**: Not Started. Items: epic `ep-004-qd` in `.collide/planning/epics/` (part L).

## Stage 5: GitHub adapter

**Goal**: A global source with live updates and writes, built against the same protocol before the UI depends on it.

- `github` adapter (TypeScript, built-in pack). `board.yaml` names the repos and projects. The `gh` account is per
  user, in the user's settings (0.7), and is read with `gh auth token --user <name>`.
  - Issues over REST: `since` + `sort=updated`, ETags. Pull requests are shown read-only and are not launchable.
  - Projects v2 items and fields over GraphQL.
  - Draft items are keyed `github:<owner>/projects/<n>/draft/<id>` and are untrusted (0.9).
  - Issues from repos without a board are shown but not launchable, and the reason is given.
  - `initialize` checks token scopes. Projects v2 needs `project`, and the error names
    `gh auth refresh -s project`.
  - A periodic full reconcile emits `task.removed` for deleted or transferred issues, which `since` never returns.
- **Operations:** `setStatus` (close/reopen with a reason, or the Projects Status field; categories map through
  `board.yaml`'s status mapping) and `comment`. Versions are per item (0.10). A stale version is detected before the
  write and returns `conflict`.
- **Provenance:** `author_association` per task, for 0.9's trust rules.
- Measure what one Projects v2 poll costs on a real project, and pick the interval from it.
- Any protocol change this adapter forces goes back into 0.10 and the plan adapter.

**Success Criteria**:
- An issue edit appears within 60 s (measured).
- A Projects Status change made in Collide shows on github.com.
- `setStatus` with category `done` closes an issue, or sets the mapped Projects status.
- A write with a stale version (issue or Projects item) is detected before the write and returns `conflict`.
- A deleted issue leaves the corpus within one reconcile interval.
- A view over both plan and GitHub tasks lists both.

**Tests**: Recorded GitHub fixtures (no network in CI); scope check; per-item version conflict tests; reconcile
removal; draft and PR items; category mapping; mixed-source view test.
**Status**: Not Started. Items: epic `ep-005-hs` in `.collide/planning/epics/` (part L).

## Stage 6: Task board (read-only desktop)

**Goal**: The desktop app opens a board and shows the corpus through views, live, before any workflow runs from it
(owner, 2026-11-12).

- The Tauri shell: one `rpc` command and one event channel, forwarding to the owner's socket and adding the token
  (0.2). The UI's client speaks the owner's JSON-RPC (handshake, reconnect), so it also runs in a browser against a
  fake owner.
- **Field:** views; board and run-order layouts; live updates from `watch`; divergence and collision chips.
- **Filter bar:** chips that write CEL; saving a view writes a view file.
- **Task drawer, read-only:** body, dependencies both ways, collections, per-checkout state.
- **Board switcher.**
- **Settings:** each key's effective value and the layer it came from (0.7). Secrets are set into the keychain from
  here.
- **No-setup open** (0.11): any git repo opens with the built-in packs, a GitHub source when `origin` is on GitHub
  and `gh` is logged in, and the default aliases. Board settings stay per user until Share with team writes
  `.collide/`. First-run checks: `claude` installed and logged in, a git repo (else offer `git init`), `gh` optional.
  The trust prompt appears only for repo packs.
- **Empty board:** a prompt box that creates a scratch task (launching it arrives in Stage 9), New session (Stage 9),
  the detected sources, and Add a pack.
- **Planning** (owner, 2026-11-13): a read-only planning view; links both ways in the drawer; checklists shown
  read-only; the branch-only chip.

**Success Criteria**:
- The non-owner participant completes S6 scenario 2, and scenario 1 up to the board, in the real app within S6's
  thresholds.
- A fresh git repo with no `.collide/`, on a machine profile with no Collide config, opens to a board with no setup
  step. Share with team writes `.collide/`, and a second clone opens with it after the trust prompt.
- A folder that is not a git repo offers `git init`. A missing or logged-out `claude` is named, with the fix.
- The MVP view in the app lists the same tasks as `collide tasks --view edge-mvp`.
- A `plan set` in a worktree shows as a divergence chip within the watch debounce plus the 10 s recheck.
- A key set in both settings files shows the repo's value, labelled as coming from the repo.
- The UI's tests run in a browser against a fake owner, with no Tauri.
- A proposal shared on a branch and checked out as a worktree shows its items with the branch-only chip. The
  planning view's counts match the CLI's.

**Tests**: Reducers over provider notifications; view save round-trip; the no-setup open and first-run checks; the
settings view; one
Playwright smoke run of scenarios 1 and 2.
Planning: planning-view counts against the CLI; the branch-only chip.
**Status**: Not Started. Items: epic `ep-006-wq` in `.collide/planning/epics/` (part L).

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
- Hook executor per 0.5, with the pack's settings as `config` on stdin and `COLLIDE_PACK_DATA`. The line lock, with
  the root-busy wait. Only the default line exists yet.
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
  - `collide report` and the SDK's `ctx.report`.

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
  Collide does nothing and says so.
- Line lock serialization; the root-busy wait.
- Env:
  - role before phase, so the phase's value wins;
  - a template using task text is rejected at load;
  - the safe-token rule rejects `..`, a leading `-` and a leading `.`;
  - a value that fails its provider's pattern is rejected, and so is a `model.name` that fails the pack's
    `requires.modelName`.
- YAML hazards.
- Injection: a positive-control hook that shells out on task text must trip the test; the real executor must not.
**Status**: Not Started

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
- a gate that returns `unverified` holds the run, no fix phase starts, and a retry after the fake environment is
  restored passes;
- a fake `script` phase with `agentWork: true` reports `usage`, shown apart from the measured totals, and `limited`,
  which pauses another fake run on the account;
- under a profile that switches a fixture gate pack off and sets `compact_at`, the gate does not fire and the
  phase's launch carries the option;
- after a fake builder edits `.claude/settings.json`, the next automatic phase runs without waiting, and the run is
  flagged for an acknowledgement before integrate.
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
`ask_human` cause recorded; `when:` over run state; an `after_*` refusal fails its phase; the config-path flag.
Planning: engine tables for instances; the budget stop; point order; derived-point expansion; gate binding
with `when:`; the run-diff points against the fake provider.
**Status**: Not Started

## Stage 9: Desktop UI: runs

**Goal**: Launch, Tracks and Queue (0.11) over the real core and engine, as validated in S6. A repo with no pack
gets real use here: prompt, run, review, accept.

- **Field:** Launch with a run preview (implementations, accepted tasks, the reason for each refused one; the
  profile, its reason and each setting's layer; dev and `agentWork` badges). Profiles picked in the preview or
  taken from the view's `profile:`. **Use working copy** on a repo pack whose folder in the user's checkout
  differs from the ref (0.7). The
  drawer's operation forms, create, and Record.
- **Launch from a prompt:** the empty board's prompt box creates a scratch task and launches the board's default
  workflow on it (`collide/quick` with no pack).
- **New session** (0.11): interactive Claude in a fresh worktree through the owner's PTY (S2), as a one-phase run on a
  new scratch task. The worktree is under the repo's worktree root, and Claude's trust prompt is left to the user
  (0.4, 0.9).
- **Cold resume:** a phase with `on_cold: fresh` that resumes after its cache expired starts a new session from the
  handoff note (0.3).
- **Review and accept a single run** (moved here from the integration stage; owner, 2026-11-13): diff, then accept,
  re-prompt or discard. Accept follows 0.4's order: the `sync` check with `git merge-tree --write-tree`, the
  `before_*` points of the source's operations and of the run's task diff (S11), `run.before_integrate`, the
  workflow's integrate implementation (`collide/merge-local` here),
  `run.after_integrate`, the source's operations, then `record`. An integrate that carries an edit to the agent
  tool's config waits for an acknowledgement (0.3). So does the next run that would load a working-copy pack the
  integrate changed (0.7).
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
  id followed across `/clear` (0.11).
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
- Command palette. Reattach after the app restarts. Owner logs viewable.
- **Menu bar** (0.11): the icon, badge, countdown, notifications and actions. Closing the window destroys the webview
  and keeps the app running. The app restarts a daemon that stopped answering, with 0.2's back-off.
- **Planning** (owner, 2026-11-13):
  - `collide/plan/spike` in a private proposal: pre-register, with a `human` phase that approves it; run; verify;
    decide, with a `human` phase whose `show:` is the record; write back through the source's ops;
  - Plan… (0.11): a New session on a new private proposal, from the Planning view, an epic or a task, with the
    planning workflow's prompt; proposal statuses and the person-only ops (0.10);
  - task points fired from UI writes; the Queue lane from a view; checklist marking in the drawer, with agent
    suggestions shown with their evidence.

**Success Criteria**:
- These are completed in the real app with the fake adapter, and the non-owner participant stays within S6's
  thresholds:
  - S6 scenarios 4 and 6;
  - single-task versions of scenarios 3 and 5: launch one task, then review its finished run.
- The critical path matches hand-built fixtures with known answers, including a fan-out and a human wait.
- The app quits and relaunches mid-run without loss.
- **No pack, real Claude:** S6 scenario 1 in full. A fresh repo with no pack and no tasks → a prompt → a finished
  `collide/quick` run, reviewed and accepted. The base ref has the change, and the scratch task is `done`.
- **New session:** a terminal opens in a fresh worktree, a prompt typed there is answered, closing the view
  detaches, and exiting `claude` puts the diff into review. The first session in a repo shows Claude's trust prompt
  and Collide sends it no keys; the second shows none.
- An accept that would conflict goes to the Queue, and no checkout changes (`git status`).
- **Run-diff points on the real accept path** (owner, 2026-11-13; S11): a plan-fixture task file set to `in_review` in
  a run's worktree fires `task.before_status: review` at accept. A refusing gate stops `integrate`, the base ref is
  unchanged (`git log`), and the Queue says why. A `task.before_status: done` gate refuses before anything merges.
- **Take over, with real Claude:** a running headless phase is taken over, a prompt typed in the terminal is
  answered, closing the terminal detaches, and **End take-over** ends the phase and runs its gates. Quitting the app
  mid-take-over and relaunching returns to the same session with its output replayed, as S2 measured. After a
  `/clear`, a headless resume uses the new session id.
- A take-over past its phase budget raises a Queue warning, from telemetry, and keeps running.
- One of Collide's own build tasks runs from the Field through `collide/quick` to accept, and one of its spikes runs
  through `collide/plan`'s spike workflow (part L).
- A Queue item's context view opens with `i` and makes no model request.
- **Menu bar:** with the window closed, the app and its WebKit processes stay under 80 MiB, measured as S2's addendum
  does. A new Queue item raises a notification that holds no task text while the setting is off (checked with a
  seeded title). With the window closed, a killed daemon is restarted by the app, and the run's next phase starts.
- A usage limit from the fake adapter appears in the Queue with its reset time, and the run resumes after it
  without a click. The Queue shows the resume's cache-write cost.
- **Metrics with no pack, real Claude:** the finished run of scenario 1 shows tokens, cache hit rate, CPU time, peak
  memory, disk and its context findings. An imported history shows the same for past sessions, and the store holds
  no prompt, reply or tool text (checked by searching it for seeded strings).
- A fake-adapter phase with `on_cold: fresh`, resumed after the cache TTL, starts a new session whose first prompt is
  the handoff note.
- Group launch and group accept are graded in Stage 11.
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

**Tests**: Reducer tests for Tracks and Queue against fake-adapter streams; keyboard triage tests; critical-path
fixtures; export round-trip; detector fixtures; the output pane on hostile output (HTML, ANSI, links, a 10 MB
result); the breakdown's filters (a ≥2 s filter and a clock window, counted against a fixture) and wall-clock against
summed time with parallel calls; a call that starts before the window counted, with only its in-window time in the
total; the Queue context view for each kind, built with the fake adapter counting zero model requests, the agent's
words escaped and placed after Collide's facts, a path outside the worktree flagged, and the history counts; the transcript import with repeated usage rows; the handoff-note
builder; single-run accept order, including the run-diff and `done` gates before `integrate`; the merge-tree conflict check; the notification text builder; the daemon restart
back-off; take-over, New session and
the prompt-to-accept path with real Claude (opt-in in CI, like the contract test); one Playwright smoke run of the
scenarios above.
Planning: the spike workflow end to end with the fake adapter; Plan… with the fake adapter scripting the CLI
calls; the person-only ops refused from a run; the Queue lane reducer; `protectedPaths` on checklist state and
on a planning file's `status` and `by:` lines; point timeouts and failures outside a run.
**Status**: Not Started

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
**Status**: Not Started

## Milestone C: Integration and ecosystem

## Stage 11: Group runs, integration, write-back

**Goal**: Several tasks at once, and a finished run that ends integrated and recorded.

- **First, outline Milestone C** (Stages 11 and 12) against the real loop of Stage 10 (0.15).
- Group runs from a selection or a view:
  - one run per `ready` task, under a concurrency cap;
  - a dependent starts once its dependencies are done at the base ref (integrated), not merely started;
  - a cycle is an error.
- Review and accept per run as in Stage 9, for every run of a group.
- Syncing (0.4): how far each workspace's base is behind its line's ref (the conflict check before `integrate`
  ships in Stage 9); a pack's `sync` implementation, run as a `script` phase or before `integrate`.
- The two integrate implementations:
  - `orchard/merge-local`: a local merge into the base ref;
  - `github/pull-request`, from the built-in GitHub pack: push and open a PR linked to the issue, with no closing
    keyword. The issue closes through the accept op (second-pack stage, S11).
- Write conflicts and merge conflicts go to the Queue. A task from an untrusted author needs confirmation.
- **Planning** (owner, 2026-11-13), after S10, in `collide/plan`: panel, specialist and adversarial spike
  workflows; the merge script and the merger phase; disagreements as question items, with a `wait` phase; the
  verify gate, with tolerance, sampling, a verify budget, and an ask before running a teammate's scripts; the cost
  preview.

**Success Criteria**:
- Three tasks with dependencies run under a cap of 2. The dependent starts only after its dependency is
  integrated.
- Launching the MVP view starts only its `ready` tasks.
- Accepting an Orchard run merges locally and records.
- Accepting a GitHub-sourced run opens a PR linked to its issue.
- One board runs an Orchard task under `orchard/loop` and a GitHub issue under a PR workflow at the same time, and
  each run is accepted through its own integrate implementation.
- A workspace whose base has moved on shows how far behind it is. An accept that would conflict goes to the Queue,
  and no checkout changes (`git status`).
- Accepting into the board root during a rebase waits, and the Queue says why.
- Re-prompting a run reuses no other run's workspace name or iter label run.
- A task from an untrusted author asks for confirmation.
- A failing gate on `run.before_integrate` blocks accept, and the Queue says why.
- S6 scenarios 3 and 5 pass in the real app at full scope (group launch, accept).
- Planning:
  - a two-instance panel on fixture records produces a merge table whose cases match a hand-made table;
  - a changed number in one instance becomes a question item, and the run waits until it is resolved;
  - a `repro:` whose output drifts outside tolerance fails the verify gate. One that needs an hour idle is
    `unverified`;
  - the preview's estimate shows instances, merge and verify separately.

**Tests**: Order, readiness and cycle tests; conflict-to-Queue test; implementation resolution per workflow; the
merge-tree conflict check; both integrate implementations against fixtures; untrusted-author gate.
Planning: the merge cases, one per row of the merge table; verify verdicts; `wait` on linked questions.
**Status**: Not Started

## Stage 12: Second pack, composition, observers, grouped chains

**Goal**: Prove the extension points with packs that were not the reason they exist.

- Plain GitHub pack:
  - an issue → PR → CI workflow: a `script` phase pushes and opens the PR, and a `wait` phase polls its checks;
  - PRs Collide opens carry no closing keywords (`Closes #n` and the like). The issue closes through the accept op,
    so its `task.before_status: done` gate runs before the merge and GitHub's own close does not race it. A
    Projects automation that moves the item after Collide's own close (`automated`) shows no outside chip (S11);
  - a lint/test gate on `task.before_status: review`.
- Standalone codemap pack: a `workspace.after_create` chain hook, composed with another pack's workspace
  implementation.
- `collide pack test`: fixture repo + fake adapter + `tsc --noEmit`.
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
**Status**: Not Started

## Milestone D: Measurement and experiments

Ends with a developer able to see where a real run's time and resources go, and to test an optimization against
the current workflow.

## Stage 13: Full metrics

**Goal**: With `full` on, a real orchard run shows where its CPU, memory, disk, network and GPU went, at the
granularity S8 established.

- **First, outline Milestone D** (Stages 13–15) against the measurements of Stages 9–12 (0.15).
- The sampler at S8's interval. Disk I/O per workspace, and disk space per cache. Exit totals and disk space per
  workspace are always recorded from Stage 3.
- Network through `nettop -P`. Device GPU utilization and memory (IOAccelerator). Per-process GPU time waits for the
  first local-model adapter (0.13; owner, 2026-11-13).
- Containers and detached host processes through the pack's `metrics.processes` hook, which returns container ids
  and pids.
- `metrics.import`, selected by the run's worktree paths, with the backfill of existing history.
- Machine sleep recorded as waiting, by S8's method.
- The machine part of the fingerprint; load sampled throughout.
- Privacy defaults per 0.13; retention.
- Claude telemetry only if S8 showed it adds precision.

**Success Criteria**: On a real orchard run with `full` on:
- the lane container's usage, and its host server processes' usage (`serverPid`, `uiPid`), appear under the
  workspace's spans through the pack's `metrics.processes`;
- a known GPU workload (a fixed Metal compute loop run for a set time) shows device utilization over that interval,
  within one sample at each end;
- iter's lines for that run's worktrees import as child spans, and no other worktree's lines do;
- the backfilled history appears as baseline spans;
- the sampler stays within S8's overhead budget, measured on the run;
- argv is stored as basenames and flag names, and a credential seeded in a URL argument is absent from the store;
- a run during which the machine slept shows the sleep as waiting.

**Tests**: Sampler attribution against a fake process tree; the `metrics.processes` contract; importer fixtures,
including iter sub-calls; redaction fixtures; sleep accounting; retention.
**Status**: Not Started

## Stage 14: Lines

**Goal**: Several refs open on one board, each running its own workflows and its own scripts.

- `collide line add <ref>`; line roots through a workspace implementation's `line` hook (kind `line`).
- Per-line packs and default workflow; snapshot pinned per run.
- Per-line trust, with the `protectedPaths` diff against the default line.
- Line locks per root. Hooks at the line root. Writes run the owning line's `./plan`.
- Launch resolves (shortId, path) at the line's ref. One live run per task across lines.
- Observers receive only their own lines' events (0.4).
- The optional Collide-owned default root (`defaultRoot: managed`, 0.12).
- The line switcher in the app.

**Success Criteria**:
- A second line whose `ws-create` hook differs creates workspaces with its own hook. The default line keeps the
  original.
- A line whose dev CLI differs from the default line's shows that diff before it is trusted.
- Running a task on a second line while it has a live run on the first is refused.
- Two lines `record` at the same time without waiting on each other.
- An observer of a pack applied only on the second line receives no events from the default line's runs.
- With `defaultRoot: managed`, Collide provisions the default root as a worktree on the base ref. While the user's
  own checkout has the base branch checked out, it refuses, and says why.

**Tests**: Line snapshot and trust isolation; lock scope; per-line provider writes; collision refusal on launch;
observer scoping; the managed root's refusal.
**Status**: Not Started

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
  - a reported `metric` line cannot be chosen as the primary metric.
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
- The non-owner participant completes S6 scenario 7 within its threshold.

**Tests**:
- Isolation, with positive controls.
- The `discard` path and per-arm discard.
- The experiment-root reset.
- All three orders against a fake with a known order effect. The crossover seed reproduces its order.
- The fixture-pack experiment.
- The exact crossover test and Holm over secondaries against fixed fixtures (smallest p 0.029 at 8 pairs, 0.008 at
  10).
- Abort mid-arm.
**Status**: Not Started

---

## Deferred (with the trigger that brings each back)

- **Other agent tools and model sources** (ACP adapter, Codex, OpenAI, Ollama): after Stage 10. S7 has already
  checked the trait against ACP, and aliases and capabilities exist from Stage 3. With other endpoints, repeated
  endpoint failures pause the runs that use it, like a usage limit, rather than failing each phase in turn (Ferry's
  circuit breaker).
- **Per-process GPU time:** with the first local-model adapter (owner, 2026-11-13). S8 still spikes it, and device
  GPU ships in Stage 13. In v0, Claude's inference is remote, so no process of the user's uses the GPU for it.
- **Resource tokens** (local-model GPU capacity, shared services): with the first non-Claude adapter, or when two
  packs in use throttle the same shared service themselves. R1 noted that orchard lanes are throttled by `iter`
  itself, and Ferry throttles its Docker runners the same way.
- **Observers that act** (start a run, write a task), such as pre-provisioning a workspace when a task becomes
  `ready`: when measured provisioning wait becomes the bottleneck. Acting needs an authenticated way in, which v0
  deliberately does not give scripts.
- **A local REST API** for external tools (CI, chat bots, team dashboards) to read or control Collide: when one
  needs it (owner, 2026-11-13: not now). Until then, `collide … --json` is the query interface.
- **More task operations:** GitHub `create` and `setFields`, and plan notes: when a user needs them in the UI. They
  are not needed in Milestone A.
- **Moving a scratch task into a source** (GitHub `create`, plan `create`): when a user wants a scratch task tracked
  by the team. The move op that Share adds for planning (the second-pack stage) is the mechanism.
- **Points for foreign changes** (a teammate, a pull, a hand edit, a bot): when a team needs a reaction that the
  acting machine cannot run. Until then, observers and the outside-change chip (owner, 2026-11-13).
  - Advisory gate checks on observed changes come first. A pack's `before_*` gates run as checks on the machine that
    sees the change, and report "gate X would refuse". It needs `via` and the `task.changed` shape, not a new point.
  - Run-made writes are covered in 0.4 (S11).
- **Read access to the corpus for hooks and agents** (owner, 2026-11-13; part K): a read-only
  `collide tasks --where '<CEL>' --json` that hooks may call under a per-call read capability, or a read-only MCP
  server for agents. When a gate needs links two hops out, or an agent needs planning state its backend's own CLI
  cannot give. Until then hooks get `tasks[].links` one hop out (0.5), and agents use the backend's CLI.
- **A GitHub App identity for agents** (owner, 2026-11-13; part K): agents write through an App installation token,
  so GitHub shows the change as the App's and attribution is exact. When the attributed matching (0.4) gets it wrong
  in practice, or a team wants agents' writes marked on GitHub.
- **Ask the agent from a Queue item** (owner, 2026-11-13; part L): a button that resumes the session as a fork at the
  request, asks why, and leaves the live run untouched, with its cost shown before it is sent. Whether a session
  waiting on a permission can be forked is unverified. When S6's scenario 9 or real use shows the context view is not
  enough to decide.
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
- **Claude Code telemetry for headless phases:** when S8 shows it adds precision over the stream. Interactive
  sessions and `agentWork` phases use it from Stage 9 (measured S2).
- **A daemon kept alive by launchd** (a `KeepAlive` LaunchAgent): when users want a crashed daemon back with the app
  quit. v0 starts it on demand, and the resident app restarts it (0.2).
- **Open at login** (an `SMAppService` login item that starts the app hidden in the menu bar): when users ask. The
  resident app then keeps the daemon up.
- **A separate menu bar helper without WebKit:** when the resident app's memory with its window closed matters
  (53 MiB, the S2 record's addendum).
- **Live observation of Claude sessions started outside Collide:** when the past-session import shows users want it.
- **More detectors** (files read again, prompt size, cache churn, model fit): when the first five are in use.
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

## Open questions (owner's call)

1. **Where the Orchard pack lives.** Settled in revision 9: the Orchard repo (`.collide/packs/orchard/`), with a fixture
   copy in the Collide repo for its tests (0.7). Either place is safe, because pack code loads from the base-ref
   snapshot, so the owner can still move it.
2. **Platforms.** macOS first, then Linux?
3. **Distribution.** Signed and notarized builds plus the Tauri updater, or source-only at first.
4. **Stage count.** Fifteen stages in four milestones, well beyond the usual 3–5.
   - R1 and R3 both found stages too large to land as small changes. The owner's additions (an early board,
     metrics, lines, experiments, the skeleton) each became a stage.
   - Each milestone ends usable.
   - Milestone D can be deferred whole without touching A–C.
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

## Risks

- **Claude Code's stream-json is not a versioned contract.** Mitigation: the real-claude contract test (Stage 3)
  and a pinned version.
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
- **Metrics overhead.** Sampling must stay within S8's budget, or `full` mode distorts what it measures.
- **Experiment cost.** Each arm provisions several worktrees: an experiment root, a task workspace and sandboxes. A
  orchard arm can spend tens of minutes and several GB of disk before an agent starts. A verdict needs at least 8
  crossover pairs, which for an Orchard loop means days. Effect sizes and `collide bench` are the cheaper paths.
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
  gains kinds, op effects, links, an assignee, a checklist field, five task points and derived points; a markdown
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
