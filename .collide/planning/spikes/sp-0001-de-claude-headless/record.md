---
id: sp-0001-de
alias: S1
kind: spike
status: concluded
title: "S1: Claude Code headless"
owners: []
links: []
---
# S1: Claude Code headless

- **Date:** 2026-11-13. Plan revision 9 (`IMPLEMENTATION_PLAN.md`, 0.3, 0.5, Stage 1 → S1). The plan is not edited here;
  the last section proposes changes.
- **Versions:** `claude` 2.1.323 (claude.ai login, `apiKeySource: "none"`), node v24.18.0, macOS 26.3.1 arm64.
  Aliases resolved to `claude-haiku-5-5` (`haiku`) and `claude-sonnet-5-5` (`sonnet`).
- **Reproduction:** `.collide/planning/spikes/sp-0001-de-claude-headless/`. `README.md` lists the scripts `00`–`09` and their order;
  `run-all.sh` runs them from a clean state and writes `out/<script>.txt`. Each script prints its own measurement.
- **Method:** every call ran through `claude-clean.sh` (every `CLAUDE*` variable removed) except Q9's nested case,
  with `--model haiku` (except Q3's model question) and `--max-budget-usd 0.25`. `driver.mjs` plays the adapter: it
  writes turns and control messages on stdin and logs both directions. Q6/Q7 use a scratch repo `work/repo`
  (committed config tagged BASE, untracked `settings.local.json` and `CLAUDE.local.md` tagged MAIN-LOCAL), a
  worktree `work/wt` with a builder's edits (tags WT and WT-LOCAL: settings hooks, `.mcp.json`, a skill, CLAUDE.md,
  an agent), and a snapshot built from `main` (tag SNAP). Hooks append tagged marks to a file; a canary MCP server
  marks its start; canary words in CLAUDE.md and memory files are asked back from the model.
- **Labels:** **measured** (script and observed output), **inferred** (reasoned from measurements, `--help` or the
  binary's strings), **unmeasured**.

## Launch line

```
claude -p
  --input-format stream-json
  --output-format stream-json
  --verbose
  --session-id <uuid>                  # a resumed phase uses --resume <uuid> instead
  --permission-prompts host
  --permission-prompt-tool stdio
  --model <model.name>
  --max-budget-usd <budget left for this phase>
  --agents <snapshot>/agents.json
  --agent collide-<pack>-<role>
  --setting-sources user               # "" for experiment arms (see Q7)
  --settings <snapshot>/settings.json  # includes "autoMemoryEnabled": false
  --mcp-config <snapshot>/mcp.json
  --strict-mcp-config
  --plugin-dir <snapshot>/plugins/<name>   # one per snapshot plugin, if any
  [--permission-mode <mode>]           # only when the role sets one
```

| flag | why | evidence |
|---|---|---|
| `-p` | headless; `--max-budget-usd` and `--agents <file>` need it | `--help` |
| `--input-format stream-json` | turns, permission answers and interrupts go on stdin | measured, Q1, Q2, Q5 |
| `--output-format stream-json` | events on stdout | measured, Q1 |
| `--verbose` | **required**: without it `claude` exits 1 with `Error: When using --print, --output-format=stream-json requires --verbose` | measured, Q1 |
| `--session-id <uuid>` | Collide owns the id; every stdout line carries it | measured, Q1 |
| `--resume <uuid>` (instead of `--session-id`) | new process, same session id, context kept, `SessionStart:resume` hook | measured, Q5 |
| `--permission-prompts host` | the default value already (`--help`: `default: "host"`); kept to state intent | measured, Q2 (variant d gives the same result without it) |
| `--permission-prompt-tool stdio` | **required for prompts to reach the host.** Without it every prompt is auto-denied. Not in `--help`; the bundled Agent SDK passes `--permission-prompt-tool=stdio` when a `canUseTool` callback is set (binary strings) | measured, Q2; the SDK detail inferred |
| `--model` | overrides the definition's frontmatter `model:` | measured, Q3 |
| `--max-budget-usd` | soft cap on what **this process** spends, checked after each model call; pass the phase's remaining budget on every resume | measured, Q4 |
| `--agents <file>` + `--agent` | file path works under `-p`; prompt text is not in argv; `--agents` wins a name clash with the worktree's `.claude/agents/` | measured, Q3 |
| `--setting-sources user` | drops the worktree's (and main checkout's) project and local settings, CLAUDE.md, skills and agents | measured, Q7 |
| `--settings` | snapshot hooks and settings apply; `autoMemoryEnabled: false` turns off the only worktree-independent channel left (auto memory) | measured, Q7 a, g, h |
| `--mcp-config` + `--strict-mcp-config` | only snapshot MCP servers start; user and claude.ai servers are dropped | measured, Q7 a |
| `--plugin-dir` | snapshot plugin skills load | measured, Q7 a |

**Environment** (inferred from Q9): start from Collide's environment, remove `CLAUDECODE` and every `CLAUDE_*`
variable (not only `CLAUDE_CODE_*`), then add `COLLIDE_RUN` and `COLLIDE_SPAN`. Set no `ANTHROPIC_*` variable; the
login is read from the user's own config. `cwd` is the workspace.

**Protocol** (measured, Q1, Q2, Q5):

- user turn on stdin: `{"type":"user","message":{"role":"user","content":[{"type":"text","text":"…"}]},"parent_tool_use_id":null,"session_id":""}`;
- permission answer: `{"type":"control_response","response":{"subtype":"success","request_id":"<from the request>","response":{"behavior":"allow","updatedInput":<input>}}}`,
  or `"behavior":"deny","message":"…"`;
- cancel: `{"type":"control_request","request_id":"<id>","request":{"subtype":"interrupt"}}`. The process
  stays up. To end the process, close stdin after the `result`. Hard stop: SIGTERM (exit 143).
- final report: `result.result` of a `result/success` event.

## Answers

### Q1. Multi-turn stream-json; is `--verbose` needed?

- **Yes, `--verbose` is required** (measured, `01` A and B). A text prompt and stream-json input both exit 1 with
  stderr `Error: When using --print, --output-format=stream-json requires --verbose` and no stdout.
- **Event order, two turns** (measured, `01` C):
  - turn 1: `system/hook_started` ×3 and `system/hook_response` ×3 (the user's SessionStart hooks), `system/init`,
    then `assistant` messages (`thinking` blocks and `system/thinking_tokens` appear on some turns),
    `rate_limit_event status=allowed`, `result/success`;
  - turn 2: `system/init` **again**, `assistant`, `result/success`;
  - after stdin closes: exit 0.
- Every line carries `session_id` equal to `--session-id` (measured). `rate_limit_event` came once per process, in
  turn 1 (measured; the binary describes it as "emitted when rate limit info changes").
- `total_cost_usd` is **cumulative per session**: 0.00351 after turn 1, 0.00700 after turn 2 (measured, `01` C).
  A `--resume`d process continues the total, with the earlier cost included (measured, `05` b: 0.00503 = 0.00445
  before + 0.00057 for its turn). The per-turn figure is `model_usage_since_last_result`.
- An `initialize` control_request is optional. Its `control_response` comes after the hooks, before `init`, with
  keys including `account`, `agents`, `commands`, `models`, `pid`, `session_state` (measured, `01` D).

### Q2. `--permission-prompts host`: request shape and answer

- **`--permission-prompts host` alone does not route prompts to the host** (measured, `02` a, b). The Bash call
  `touch s1-first.txt` is denied at once: `system/permission_denied` with `touch in '…/s1-first.txt' needs approval…`,
  an error `tool_result`, and the command in `result.permission_denials`. Sending `initialize` first changes
  nothing. `--help` says `host` is the default.
- **With `--permission-prompt-tool stdio`** (measured, `02` c; same without `--permission-prompts`, `02` d) the CLI
  writes on stdout:

  ```json
  {"type":"control_request","request_id":"89b41f36-…","request":{"subtype":"can_use_tool","tool_name":"Bash",
   "display_name":"Bash","input":{"command":"touch s1-first.txt","description":"Create first empty file s1-first.txt"},
   "description":"…","permission_suggestions":[{"type":"addRules","rules":[{"toolName":"Bash","ruleContent":"touch s1-first.txt"}],
   "behavior":"allow","destination":"localSettings"},{"type":"addDirectories",…,"destination":"session"},
   {"type":"setMode","mode":"acceptEdits","destination":"session"}],"blocked_path":"…/s1-first.txt","tool_use_id":"toolu_…"}}
  ```

  - **allow** (stdin): `{"type":"control_response","response":{"subtype":"success","request_id":"89b41f36-…","response":{"behavior":"allow","updatedInput":{"command":"touch s1-first.txt","description":"…"}}}}`.
    The tool ran: `(Bash completed with no output)`, and the file exists.
  - **deny** (stdin): `{"type":"control_response","response":{"subtype":"success","request_id":"7cf2921f-…","response":{"behavior":"deny","message":"S1 host denied this tool call."}}}`.
    The model sees an error `tool_result` with our message verbatim; the call appears in
    `result.permission_denials`; no `system/permission_denied` event.
- The allow result can also carry `updatedPermissions`, and deny can carry `interrupt` (binary schema, inferred).
  One suggestion's destination is `localSettings`: echoing it back would make Claude write the worktree's
  `.claude/settings.local.json` (inferred). The adapter should answer without `updatedPermissions`.
- **A host that goes away loses the phase's permissions** (measured, `02` e). When stdin closes with a request
  pending, that call and every later one fail with `Tool permission request failed: AbortError: Tool permission
  stream closed before response received` (then `…: Stream closed`). The model finishes the turn without the
  tools. The process emits `result/success`, `is_error: false`, with both calls in `permission_denials`, and exits 0.

### Q3. `--agents <file>` + `--agent`: file path, name clash, `--model`

- **A file path works under `-p`** (measured, `03` a): the reply starts with the definition's marker `SNAPAGENT`.
  The argv check finds the agent prompt in no process's argv (`0`).
- **`--model haiku` overrides the frontmatter's `model: sonnet`** (measured, `03` a): `modelUsage` has only
  `claude-haiku-5-5`.
- Without `--model`, the frontmatter's `model: sonnet` beats a `--settings` default of `haiku` (measured, `03` b:
  `claude-sonnet-5-5`).
- **On a name clash, `--agents` wins** (measured, `03` c). In the worktree, which has its own
  `.claude/agents/collide-test-builder.md` (marker `WTAGENT`), the reply is `SNAPAGENT`. Control `03` d: without
  `--agents`, the same `--agent` gives `WTAGENT`.
- `init.agents` lists the name once and says neither which definition nor which agent the session runs (measured).
- **A definition's `tools:` list hides MCP tools** (measured, `07` b). With `tools: Read, Grep, Glob`,
  `init.tools` is exactly `["Read","Grep","Glob"]`, and `mcp__s1snap__canary` from `--mcp-config` is gone. A
  snapshot definition must name its MCP tools.

### Q4. A tiny `--max-budget-usd`

- **Budget 0.0001, two turns** (measured, `04` a):
  - The first turn completes (assistant text `ONE`) and then reports `result/error_max_budget_usd`:
    `is_error: true`, `result: null`, `errors: ["Reached maximum budget ($0.0001)"]`, `total_cost_usd: 0.00353`.
    **The cap is checked after a call is paid for**, so spend overshoots by up to one call.
  - The process stays up. The second turn gets an immediate `result/error_max_budget_usd` with `stop_reason: null`
    and no model call (cost unchanged).
  - Exit code **1** once stdin closes.
- **Budget 0.002, a turn that needs several tool calls** (measured, `04` b): it stops mid-turn after the first
  `tool_use` (`stop_reason: "tool_use"`, the tool is not run), with `result/error_max_budget_usd`. Exit 1.
- **The budget counts only the current process's spend, not the session's** (measured, `04` c). A session that
  had spent 0.00298 was resumed with `--max-budget-usd 0.004471`. The resumed turn cost 0.00310 and ended
  `result/success`, although the reported session `total_cost_usd` was 0.00608, above the cap. So the adapter
  passes the phase's remaining budget on every resume. With a cap below one turn's cost (0.002809 against 0.0031),
  the resumed turn ran and then ended `error_max_budget_usd`.

### Q5. Final message as the report; resume; cancel

- **Report** (measured, `05` a): `result.result` is the text of the **last** assistant message only
  (`"REPORT\nquokka\nEND"`); the earlier `Reading now.` is not included. Error results have `result: null`.
- **Resume** (measured, `05` b): a new process with `--resume <id>` keeps the session id on every line, runs the
  `SessionStart:resume` hook and remembers the earlier turn (answers `quokka`).
- **Cancel with the interrupt control request** (measured, `05` c), 3 s into a 41.5 s Bash call:
  - `control_response` `{"subtype":"success","request_id":"host-interrupt-1","response":{"still_queued":[]}}`;
  - an error `tool_result` (`The user doesn't want to proceed with this tool use…`), a user text
    `[Request interrupted by user for tool use]`;
  - `result/error_during_execution`, `is_error: true`, `stop_reason: "tool_use"`,
    `errors: ["[ede_diagnostic] result_type=user last_content_type=n/a stop_reason=tool_use"]`;
  - the process stays up, and the next turn works (`AFTER`). No `sleep` survived.
- **SIGINT** (measured, `05` d): the same events, with the result 28 ms after the signal. The process then exits by
  itself with **code 0** 1.2 s later, although stdin is still open. Resume works.
- **SIGTERM** (measured, `05` d): an error `tool_result`, **no `result` event**, exit **143** 1.3 s after the
  signal. The tool's `sleep` is killed. Resume works; the resumed model reports the killed call as exit 137.
- A process killed before its `result` reports no cost (measured: `05` d-SIGTERM has no `total_cost_usd`). A
  later `--resume` includes it: `05` d-SIGTERM-resume reports 0.00461, of which 0.00103 is its own turn.

### Q6. Plain `-p` in a git worktree: what loads

Measured, `06` a (plain line in `work/wt`, no approval of anything):

| item | loads? | observation |
|---|---|---|
| worktree `.claude/settings.json` hooks | yes | marks `SessionStart/UserPromptSubmit/Stop tag=WT` |
| worktree `.claude/settings.local.json` hooks (untracked) | yes | `tag=WT-LOCAL` |
| **main checkout's untracked `.claude/settings.local.json`** | **yes** | `tag=MAIN-LOCAL` fires in the worktree |
| main checkout's committed settings (BASE) | no | no `tag=BASE` mark |
| worktree `CLAUDE.md` | yes | `InstructionsLoaded ["Project","session_start","work/wt/CLAUDE.md"]`; model answers `WT-CANARY-HERON` |
| main checkout's `CLAUDE.local.md` | no | not in `InstructionsLoaded`; canary not reported |
| worktree `.mcp.json` server | yes, **without approval** | `mcp-start name=s1wt`, `s1wt=connected` |
| worktree skills | yes | `init.skills`: `s1-skill-base s1-skill-wt` |
| worktree agent | yes | `init.agents` has `collide-test-builder` |
| user plugins | yes | 7 user plugins and 4 built-in `cc-plugin-*` in `init.plugins` |
| user hooks, user CLAUDE.md (+ its `@` include) | yes | 3 user SessionStart hooks; `InstructionsLoaded ["User",…]` |
| user MCP server, claude.ai connectors | yes | in `init.mcp_servers` |
| auto memory | yes | `memory_paths.auto` = `~/.claude/projects/<key of work/repo>/memory/`: **keyed to the main checkout**, while the transcript goes under the worktree's key |

- `06` b: adding `"enableAllProjectMcpServers": true` to the worktree settings changes nothing: the server already
  starts (measured). A builder that adds a server to `.mcp.json` gets it started in the next plain `-p` run in that
  worktree (inferred).
- The plan's "repo hooks (…, rtk)": on this machine `rtk` is a user hook, not a repo hook. The Orchard repo was not
  touched. Its hooks are covered by the generic fixture hooks (inferred), not checked themselves (unmeasured).

### Q7. Settings from the snapshot: what still loads

Measured, `07` a: the plan's line, in the worktree:
`--setting-sources user --settings SNAP --mcp-config SNAP --strict-mcp-config --plugin-dir SNAP`.

- **From the worktree: nothing.**
  - no WT, WT-LOCAL or MAIN-LOCAL hook mark;
  - no project `InstructionsLoaded` (only the user CLAUDE.md and its include);
  - no `s1wt` server; skills `s1-*` only `s1snap:s1-skill-snap`;
  - `init.agents` without `collide-test-builder`;
  - the model answers `canaries=none`.
- **From the snapshot:** SNAP hooks, the `s1snap` server and the plugin skill.
- **Still loading from outside the worktree:**
  - user CLAUDE.md;
  - user hooks (4 SessionStart hooks run: 3 user, 1 SNAP);
  - user plugins and their skills: 139 skills in `init.skills`, 108 of them from plugins (one is the snapshot's);
  - built-in plugins and 20 bundled skills;
  - **auto memory.** `07` g writes a canary to the `memory_paths.auto` directory; the model reports
    `canaries=MEMORY-CANARY-IBIS`. That directory belongs to the main checkout, is shared by every worktree of the
    repo, and any phase can write it. It is the one builder-to-reviewer channel the snapshot line leaves open.
- User MCP servers and claude.ai connectors are dropped by `--strict-mcp-config` (`init.mcp_servers` is only
  `s1snap`).

Variants (all measured, `07`):

| variant | result |
|---|---|
| b: + `--agents/--agent` | `SNAPAGENT`; the agent's `tools:` hides MCP tools (Q3) |
| c: + `--disable-slash-commands` | no skills at all, including the snapshot plugin's |
| d: + `claudeMdExcludes` for the worktree | no change (worktree CLAUDE.md already absent) |
| e: + `--safe-mode` | no hooks (not even `--settings`), no MCP (not even `--mcp-config`), `--agents` ignored, no memory: unusable |
| f: `--setting-sources ""` | accepted; SNAP hooks, MCP and plugin skill work; user CLAUDE.md, user hooks (1 SessionStart) and user plugins gone (20 skills, 1 from a plugin); auto memory path still set |
| h: g + `"autoMemoryEnabled": false` in `--settings` | `memory_paths: null`, canary not reported |
| i: g + `CLAUDE_CODE_DISABLE_AUTO_MEMORY=1` | `memory_paths: null`, canary not reported |
| j: g + `--restricted --tools Bash,Read,Grep,Glob` (no `--setting-sources`) | as f, plus `memory_paths: null` and only `Bash, Glob, Grep, Read` and the snapshot MCP tool |

Options for the user level (proposal; the owner decides):

| option | pros | cons |
|---|---|---|
| `--setting-sources user` (plan) | a phase behaves like the user's own `claude`: their CLAUDE.md, hooks and plugins | personal config changes agent behaviour and cost (108 plugin skills in context; a user `PreToolUse` hook here rewrites Bash commands); phases differ between users |
| `--setting-sources ""` | the same config for every user and every arm; fewer tokens | the user's own conventions and hooks are absent from phases |
| `--restricted` | as `""`, plus memory off, an explicit tool list; per `--help`, file tools confined to the working directories and writes to settings, git and tool-config files need the permission handler (unmeasured) | removes code-running tools unless `--tools` names them; its write rules are untested here |

The launch line keeps `user`, as the plan says, and adds `autoMemoryEnabled: false`. Experiment arms use `""`.

### Q8. Usage and rate limits

No limit was hit on purpose. Sources: the 2.1.323 binary, the live stream, and two local transcripts that hit
one (`08`).

- **Live** (measured, `01` C via `08`): `{"type":"rate_limit_event","rate_limit_info":{status, resetsAt,
  rateLimitType, unifiedWindows, overageStatus, overageDisabledReason, isUsingOverage},"uuid","session_id"}` with
  `status: "allowed"`, `rateLimitType: "seven_day"`, `resetsAt: 1794960000` (epoch seconds, 2026-11-18T00:00:00Z).
- **Schema** (binary, measured by `08`):
  - `status` is `allowed | allowed_warning | rejected`;
  - `rateLimitType` is `five_hour | seven_day | seven_day_opus | seven_day_sonnet | seven_day_overage_included |
    overage`;
  - `resetsAt` is an int, optional;
  - assistant messages carry an optional `error` field with these values: `authentication_failed`, `oauth_org_not_allowed`,
    `account_on_hold`, `verification_required`, `billing_error`, `rate_limit`, `overloaded`, `invalid_request`,
    `model_not_found`, `server_error`, `unknown`, `max_output_tokens`, `cloud_credential_error`;
  - `result/success` carries `is_error` and `api_error_status`.
  - A 429 with quota information becomes an assistant message with `error: "rate_limit"` whose text ends in
    `· resets <resetsAt as local time>`. When it is a usage cap, it also gets
    `apiError: "usage_limit_reached"`.
- **Recorded** (measured, `08`: shape only, masked; 2 transcripts, `claude` 2.1.288): an assistant message with
  `error: "rate_limit"`, `isApiErrorMessage: true`, `apiErrorStatus: 429`, text `You've hit your <window> limit ·
  resets <time> (<tz>)`, and `quotaLimits: {status: "rejected", rateLimitType: "seven_day", resetsAt: <number>,
  overageStatus, overageDisabledReason, isUsingOverage, unifiedRateLimitFallbackAvailable}`.
- **At a limit, stream-json would show** (inferred from the above):
  - a `rate_limit_event` with `status: "rejected"` and `resetsAt`;
  - an `assistant` message with `error: "rate_limit"` and the "You've hit your … limit · resets …" text;
  - a `result/success` with `is_error: true` and `api_error_status: 429`.

  **The reset time is machine-readable** (`resetsAt`, epoch seconds), so the adapter need not parse the text.
  `allowed_warning` is an early signal before a limit.

### Q9. Nested `claude -p` with `CLAUDECODE` set; what children see

- **Nested behaves the same** (measured, `09` a vs b): no refusal, no stderr, no warning event, the same event
  sequence, the requested session id, transcript written, exit 0.
- **`claude` sets these in every child itself**, even when started clean (measured, `09` b, Bash tool):
  - `CLAUDECODE=1`, `CLAUDE_CODE_ENTRYPOINT=sdk-cli`, `CLAUDE_CODE_CHILD_SESSION=1`,
    `CLAUDE_CODE_SESSION_ATTENDED=0`;
  - `CLAUDE_CODE_SESSION_ID=<its own session>`, `CLAUDE_EFFORT=medium`;
  - `CLAUDE_CODE_EXECPATH`, `CLAUDE_CODE_MESSAGING_SOCKET`, `CLAUDE_CODE_MESSAGING_TOKEN`, `CLAUDE_PID`.

  Hooks also get `CLAUDE_ENV_FILE` and `CLAUDE_PROJECT_DIR`. MCP servers also get `CLAUDE_PROJECT_DIR`.
- **Nested leaks the parent's values** to hooks and MCP servers (measured). With the parent's variables left set, the
  hook also saw `CLAUDE_CODE_EXECPATH` and `CLAUDE_EFFORT`, and the MCP server also saw `CLAUDE_CODE_EXECPATH`,
  `CLAUDE_CODE_SESSION_ATTENDED`, `CLAUDE_EFFORT` and `CLAUDE_PID`. The clean run passed none of these. The Bash
  tool's values are overwritten by the child.
- **No `ANTHROPIC_*` variable** reached any child in either case (measured; the parent had none either).
- The binary also sets `AI_AGENT` for some children (inferred from strings; not checked).

## Proposed plan changes

The plan was not edited. Each item names the section it affects.

1. **0.3 launch line; Stage 3 "Permission requests answered from the CLI"**. These contradict the plan.
   - Add `--verbose` (required) and `--permission-prompt-tool stdio`. Without the second, `--permission-prompts
     host` auto-denies every prompt.
   - Replace "Whether `-p` stream-json also needs `--verbose` is unmeasured" with the measurement.
   - `--permission-prompt-tool` is not in `--help`, so pin it in the Stage 3 contract test.
2. **0.2 "an owner restart loses nothing"; S3; Stage 3.** These contradict the plan.
   - The phase's stdin carries permission answers, interrupts and turns. If stdin closes with a request pending,
     every remaining tool call of that turn is denied, and the run ends `result/success`, `is_error: false`, exit 0.
   - Proposal:
     - S3 tests a pending permission request across an owner restart;
     - the owner keeps stdin alive across its own restart, for example with a FIFO held open by a holder process
       (inferred, unmeasured);
     - the adapter treats `permission stream closed` denials as a failed phase, not a success.
3. **0.3 "Whatever S1 finds still loading…"; 0.11 Queue; Stage 8 and Stage 9 acknowledgement.** This sharpens the plan.
   - Under the snapshot line nothing from the worktree loads (Q7). So a builder's config edits cannot reach
     later automatic phases that use this line.
   - The acknowledgement still matters in three cases:
     - interactive take-over (plain `claude` in the worktree loads everything, Q6);
     - edits that get committed and so reach the next snapshot;
     - auto memory.
   - Proposal: tie the acknowledgement to a config change reaching the line's snapshot (`record` / `integrate`)
     and to take-overs, rather than to every automatic phase whose range touches the paths. Or keep the rule and
     state the narrower reason.
4. **0.3 config paths, 0.14 isolation.** This sharpens the plan.
   - Auto memory loads under the snapshot line and is shared by all worktrees of the repo, keyed to the main
     checkout.
   - Proposal: every headless phase gets `autoMemoryEnabled: false` in the snapshot settings. Both this key and
     `CLAUDE_CODE_DISABLE_AUTO_MEMORY=1` were measured to work; the env variable must be added after stripping.
     A role may opt in.
   - Add `~/.claude/projects/<main checkout key>/memory/` to the adapter's declared paths for every phase, not only
     experiments.
5. **0.3 / 0.5 env stripping; Stage 3 success criterion.** This contradicts the plan.
   - Strip `CLAUDECODE` and all `CLAUDE_*`, not only `CLAUDE_CODE_*`. `CLAUDE_PID`, `CLAUDE_EFFORT`,
     `CLAUDE_ENV_FILE` and `CLAUDE_PROJECT_DIR` would otherwise leak.
   - Stage 3's "Started from a shell with `CLAUDECODE=1` set, neither the phase nor its children see it" cannot hold
     for an agent phase's children. Claude sets `CLAUDECODE=1` in them itself (Q9).
   - Reword it: Collide's own hooks and the `claude` process do not inherit the shell's `CLAUDE*`; the agent's
     children see only the values `claude` sets for its own session.
   - Orchard's agent-label check therefore applies to every agent phase's Bash calls, which is intended.
6. **0.3 usage limits; Stage 3 limit pause; fake Claude.** This sharpens the plan.
   - `limited` comes from `rate_limit_event.rate_limit_info.status == "rejected"` with `resetsAt` (epoch
     seconds), or from an `assistant` message with `error: "rate_limit"`. No text parsing is needed.
   - `allowed_warning` can drive "other runs pause before their next turn" early.
   - The fake Claude replays this event triple. The contract test records a real one when it next happens.
7. **Stage 3 adapter and metrics.** These sharpen the plan.
   - The report is `result.result` of `result/success`, and it holds the last assistant message only. Error results
     have `result: null`.
   - `result/success` with `is_error: false` does not mean the work happened: check `permission_denials`.
   - `total_cost_usd` is cumulative per session and carries across `--resume`, including a SIGTERM'd turn. A
     phase's cost is therefore the last total its session reported. Do not sum the processes: that double counts.
     A phase that is SIGTERM'd and never resumed has no reported cost for its last turn. Stage 3's criterion "its
     tokens and cost match the stream's final usage" holds per session.
   - `system/init` repeats on every turn.
   - Budget is a soft cap on the current process's spend, checked after each call, so it overshoots by up to one
     call. The adapter passes the remaining budget (role budget minus the session's total) on resume. After
     `error_max_budget_usd`, the process refuses further turns and exits 1. Map it to `ask_human` with cause
     budget (Stage 8).
   - Stop order for a phase: interrupt control request, then close stdin, then SIGTERM. SIGTERM gives exit 143
     and no `result`; SIGINT gives a result and exit 0. This differs from 0.5's SIGTERM-first rule, which is
     written for hooks.
8. **0.3 agents.** This confirms and sharpens the plan.
   - `--agents` wins a name clash, so the `collide-<pack>-<role>` naming is a second safeguard, not the only one.
   - `--model` beats the frontmatter.
   - A definition with `tools:` must list its MCP tools.
9. **0.3 / Stage 3 tools.** This is new and inferred.
   - The headless default tool set includes `CronCreate`, `ScheduleWakeup`, `RemoteTrigger`, `PushNotification`,
     `EnterWorktree`, `ExitWorktree`, `Workflow` and `SendMessage` (measured, `07` a `init.tools`). Some of these
     schedule work the owner does not know about, or move the session out of its workspace.
   - Proposal: the adapter passes `--disallowed-tools` for these unless a role asks for them.
10. **0.14 experiments.** This sharpens the plan.
    - Arms use `--setting-sources ""`, so user-level config is identical across arms.
    - "A permission prompt outside the allowlist is denied and recorded" is what happens without a prompt tool:
      `system/permission_denied` plus `result.permission_denials` (measured, Q2 a). `--permission-prompts none`
      is the documented way to get it (unmeasured).

## Not measured

- A live usage or rate limit: the stream shape at a limit is inferred from the binary and transcripts (Q8).
- `--permission-prompts none` and `--restricted`'s write rules for settings, git and tool-config files.
- Whether `--agents` and `--agent` must be passed again on `--resume`. `05` b resumed without them and
  without an agent.
- Hooks, agents and MCP servers that user plugins bring in, and whether settings edited mid-run are re-read.
- The Orchard repo's own hooks; the Orchard repo was not touched.
- `--bare`, which needs `ANTHROPIC_API_KEY` and never reads the OAuth login.
- How notional `total_cost_usd` on a claude.ai subscription relates to real billing.
- Descendants beyond the tool's own process (setsid'd grandchildren, orphans): left to S3.
- Keeping stdin alive across an owner restart, for example with a FIFO.

## Spend

Notional `total_cost_usd` (claude.ai subscription). Per process, what it added to its session's total (`spend.sh`).
The clean pass recorded in `out/`:

| script | spend |
|---|---|
| `01` | $0.0105 |
| `02` | $0.0136 |
| `03` | $0.0048 |
| `04` | $0.0114 |
| `05` | $0.0146 |
| `06` | $0.0043 |
| `07` | $0.0169 |
| `08` | $0 |
| `09` | $0.0054 |
| total | $0.082 |

All runs together, including development and reruns (about 110 sessions), come to about $0.40, well under $1.
This is an estimate: cost is not stored in transcripts.
