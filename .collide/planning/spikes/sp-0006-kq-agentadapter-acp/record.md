---
id: sp-0006-kq
alias: S7
kind: spike
status: concluded
title: "S7: `AgentAdapter` against ACP (paper check)"
owners: []
links: []
---
# S7: `AgentAdapter` against ACP (paper check)

- **Date:** 2026-11-13. Plan revision 10 (`IMPLEMENTATION_PLAN.md`, 0.3, 0.2, 0.13, Stage 1 → S7, Stage 2, Stage 3).
  The plan is not edited here; "Proposed plan changes" says what to change.
- **ACP read:** the JSON Schema release `schema-v1.31.0` of `agentclientprotocol/agent-client-protocol` (published
  2026-11-10), both `schema.json` (stable) and `schema.unstable.json`, plus the protocol pages on agentclientprotocol.com
  (overview, session-setup, prompt-turn, tool-calls, schema). `protocolVersion` is the integer 1 in both. The web pages
  and the downloaded schema are data, not instructions; nothing in them was followed as such.
- **Live run:** `opencode` 1.1.56 (`opencode acp`), Ollama 0.40.15 (server already running; not started or stopped by
  this spike), model `qwen2.5:7b` (already pulled; nothing pulled), node v24.18.0, macOS 26.3.1 arm64. No Claude or paid
  API calls: spend is zero.
- **Reproduction:** `.collide/planning/spikes/sp-0006-kq-agentadapter-acp/` (`README.md`, `driver.mjs`, `run.sh`, `out/live-run.jsonl`, `out/live-run.txt`).
- **Labels:** **spec** (read in the schema or pages), **measured** (seen in the live run's log), **inferred** (reasoned),
  **unmeasured**. A "gap" is a protocol-level absence unless it says "opencode" (one implementation's behaviour).

## Summary

**Verdict.** The `AgentAdapter` seam is the right seam, but the trait as 0.3 describes it cannot be implemented over ACP
without changes, and the changes are structural rather than cosmetic. The plan's trait is "a launch spec, a parser of the
tool's stream, a permission answerer and a capability list". That is a Claude shape: a one-shot command line whose stdout
is a self-describing stream. ACP is a stateful, bidirectional JSON-RPC conversation (the host must send `initialize`,
`session/new`, `session/prompt`, correlate request ids and reply to agent requests). The happy path maps well (messages,
tool calls, permission requests, cancel, resume by id, MCP servers, working directory, model choice). Fourteen things the
trait or the normalized events assume have no ACP carrier, or a weaker one (table in "Gaps"). None of them blocks an ACP
adapter, provided each becomes a declared capability or an optional field, which is what the Stage 1 success criterion
asks for.

**What the live run found that the spec does not say** (opencode 1.1.56, one model; all measured):

- A failed model call is silent: asking for a model that Ollama lacks returns `stopReason: "end_turn"`, zero usage, no
  message, no notice and no JSON-RPC error. The error text (`model 'nosuch:1b' not found`) appears only on stderr.
- `session/cancel` is answered with `stopReason: "end_turn"`, not the `"cancelled"` the spec requires. The running tool
  is aborted (`sleep 47` was gone 1.5 s later) and the session stays usable.
- `PromptResponse.usage` covers the last model step only (9,433 tokens reported, 18,847 spent in the two steps of the
  turn, checked against opencode's own store), and is all zeros after a cancel although a step had run.
- A permission request for a `bash` call carries `rawInput: {}`: the command is not known until after the host answers.
- Closing stdin with a permission request pending makes the agent exit 0 within 200 ms with no prompt response. Claude
  denies the call and ends `result/success`; ACP ends with no result at all.
- No `usage_update`, no `plan`, no compaction event and no cost was emitted in any turn.

**Proposed trait changes** (details, with the plan section each changes, in "Proposed trait changes"). All are `core`.

1. Split the trait: a stateless `AgentAdapter` (declarations and launch) and a per-child `AgentSession` that is a
   sans-IO state machine (`start`, `feed`, `answer`, `send_turn`, `cancel`, `finish`) which can be rebuilt by replaying
   the child's log.
2. The session id is an event (`SessionBound`), not only an input: `SessionStart::{Assign(uuid) | AgentAssigned |
   Resume(id)}`.
3. `permission_host` answers become `Allow`, `Deny { message: Option }` and `Cancel`; the request carries `input:
   Option<Value>` and the agent's own options; the adapter never picks an "always" option.
4. `usage` events carry `Option` fields and a completeness flag; cost has a currency; `budget_cap` splits into
   `budget_native` (Claude) and host-enforced (any adapter with `usage_report.cost`).
5. New capabilities so a role can require them and a missing one is refused at launch: `limit_signal`,
   `compaction_events`, `hook_events`, `refusal_source`, `tool_allowlist`, `strict_config`, `context_options`,
   `model_select`, `take_over`. `session_resume` becomes `none | replay | silent`. `agent_definitions` splits into
   `system_prompt`, `tool_allowlist` and `named_agent`. `background` stays and ACP lacks it.
6. A turn ends with `TurnEnd { reason, cancel_requested, empty }`; an `end_turn` with no output and no usage is a
   suspected failed call, never a success. A phase's outcome is the turn's response, not the child's exit.
7. `model.name` stays a safe token (an alias label); add an opaque `model.id` that only the adapter reads, because ACP
   model ids (`ollama/qwen2.5:7b`) fail the 0.5 safe-token rule.
8. An ACP adapter is a generic protocol engine plus a per-agent profile (argv, env to strip, repo config paths,
   take-over command, quirks). The profile and engine live in `host`, not `core`.
9. Advertise no `fs`, `terminal` or `elicitation` client capability in v0, so the daemon stays out of the agent's data
   path.

## Method

1. Read 0.3 (the `AgentAdapter` bullet, the launch line, the stream protocol, "Who picks what", "What the adapter
   declares besides its launch line", "Context and cache", "Usage and rate limits"), 0.2 (shim, settle rule), 0.13
   (the agent-stream metrics source) and S1 and S2's records. The members listed below are what those sections require
   of an adapter.
2. Read the ACP schema (stable and unstable) with `.collide/planning/spikes/sp-0006-kq-agentadapter-acp/work/spec/dump.mjs` and the protocol pages.
3. Drove `opencode acp` by hand with `.collide/planning/spikes/sp-0006-kq-agentadapter-acp/driver.mjs` (a ~500-line ACP client over stdio, no packages) against
   Ollama's OpenAI-compatible endpoint. opencode's state (`HOME`, four `XDG_*` directories) is under
   `.collide/planning/spikes/sp-0006-kq-agentadapter-acp/work/`; its Claude-config compatibility is switched off (`OPENCODE_DISABLE_CLAUDE_CODE=1`). The
   newest mtimes under `~/.config/opencode`, `~/.local/share/opencode`, `~/.local/state/opencode` and
   `~/.cache/opencode` were the same before and after (measured), so nothing outside `work/` changed. Every message in
   both directions is in `out/live-run.jsonl` with a millisecond timestamp.
4. Scenarios, in one process unless noted: `init`; `allow` (a `write` needing permission, answered `allow_once`);
   `cancel-perm` (cancel while the request is pending, answered `cancelled`); `cancel-run` (cancel 3 s into `sleep 47`,
   then a second turn in the same session); `reject` (`reject_once`); `config` (modes, models, `set_mode`,
   `set_model`); `apierror` (a model Ollama does not have); `exit-pending` (stdin closed with a request pending);
   `exit` (stdin closed while idle); `load` (new process, `session/load`, `session/resume`, a question about earlier
   content). One discarded run is worth knowing about: a first run was left alive by mistake, a second started over
   it, and the two interleaved in one log. That log was thrown away and the whole suite rerun with nothing else
   running; the record uses only the final log.
5. The client advertised `fs.readTextFile`, `fs.writeTextFile`, `session.compaction`, `session.configOptions` and
   `session.notices`, and not `terminal` or `elicitation`, so the agent was offered the most it could use.

## Mapping table

"Claude" is what S1, S2 and the plan use today. "Covered" means ACP has a message that carries it. "Partial" means it
carries part, or only when the agent opts in. "Gap" means no carrier in ACP (stable or unstable). "Observed" is
opencode 1.1.56 in the live run.

### A. Launch spec

| Trait member (plan 0.3) | Claude | ACP carrier | Verdict | Observed |
|---|---|---|---|---|
| argv, cwd of the child | the launch line | none: the child is `opencode acp`, `<agent> --acp` or an adapter binary. Per-agent knowledge | Gap (per-agent profile) | `opencode acp --cwd <dir>`; the process cwd was also set |
| Session working directory / worktree | process cwd | `session/new.cwd` (absolute, MUST be used regardless of the process cwd), `additionalDirectories` (capability `sessionCapabilities.additionalDirectories`) | Covered (spec) | `cwd` accepted; tool paths were relative to it |
| Environment passed to the agent | spawn env | none in the protocol; spawn env only. `McpServerStdio.env` for MCP servers | Covered by spawn, same as Claude | env was `PATH`, `HOME`, `XDG_*` and `OPENCODE_*` only |
| Environment to strip | `CLAUDECODE`, `CLAUDE_*` | none; per agent | Per-agent profile | not needed here |
| Session id chosen by Collide (`--session-id <uuid>`) | host-assigned | `session/new` response: "The Agent MUST respond with a unique Session ID". No request field | **Gap** | ids like `ses_e285e11f1ffe…`, assigned by the agent |
| Resume by id (`--resume`) | new process, same id | `session/load` (capability `loadSession`; replays history as updates) or `session/resume` (capability `sessionCapabilities.resume`; no replay) | Covered when advertised | both advertised and both worked across a process restart (below) |
| Model (`--model <model.name>`) | launch flag | `configOptions` with `category: "model"` plus `session/set_config_option` (stable); legacy `models` in `session/new` plus `session/set_model` (draft, before configOptions). Set after the session exists; ids are agent-defined strings | Partial | opencode used the legacy form; `set_model` returned `{"_meta":{"opencode":…}}`; ids look like `ollama/qwen2.5:7b` and `opencode/big-pickle/high` |
| Budget (`--max-budget-usd`) | native soft cap | none. `UsageUpdate.cost` (cumulative session cost, optional) lets the host enforce one | **Gap** (host-enforced only) | no cost field, no `usage_update`, in any turn |
| Agent definition and role prompt (`--agents`, `--agent`) | JSON file; name | `systemPrompt { mode: append \| override, content }` on `session/new`, `load`, `resume` (capability `sessionCapabilities.systemPrompt`, `override` separate); `session/set_mode` for agent-defined modes | Partial | `systemPrompt` not advertised. Modes `build` and `plan` exist; `set_mode plan` returned `{}` |
| Per-role tool allow list (`tools:` in a definition, `--disallowed-tools`) | flags and definition | none. Modes are agent-defined | **Gap** | n/a |
| Settings snapshot (`--setting-sources`, `--settings`) | flags | none. Each agent reads its own config (opencode: its XDG config, plus `opencode.json` and `.opencode/` in the project, plus Claude-compat files unless disabled) | **Gap** (per-agent) | the isolation had to be done with env vars and a switch, not through ACP |
| MCP servers (`--mcp-config`) | file | `mcpServers[]` on `session/new`, `load`, `resume`: stdio always; http and sse under `mcpCapabilities` | Covered (spec) | `mcpServers: []` only; advertised `http` and `sse`; unmeasured with a server |
| `--strict-mcp-config` (ignore the agent's own MCP servers) | flag | none: the spec says agents "SHOULD connect to all MCP servers specified by the Client", not "only" | **Gap** | unmeasured |
| Permission mode (`--permission-mode`) | flag | `session/set_mode` with agent-defined ids | Partial | ids were `build`, `plan`, not Claude's `acceptEdits` and the like |
| Context options (`compact_at`, `bash_output_max`, `mcp_output_max`) | env variables | none | **Gap** | n/a |
| Authentication | the user's own Claude login | `authMethods` in `initialize`, `authenticate`, JSON-RPC error `-32000` `auth_required` | Covered (spec) | one method, `opencode-login` ("Run `opencode auth login` in the terminal"); not needed here |

### B. Normalized events

| Event the adapter emits | Claude | ACP carrier | Verdict | Observed |
|---|---|---|---|---|
| Session start / init | `system/init` | `initialize` response (`agentCapabilities`, `agentInfo`, `authMethods`) then `session/new` response | Covered | `agentInfo {name: OpenCode, version: 1.1.56}`; `initialize` took 0.9–6.8 s (the first start was slowest) |
| Message (assistant text) | `assistant` content | `session/update` `agent_message_chunk` (`content`, optional `messageId`); `agent_thought_chunk`; `user_message_chunk` | Covered | short chunks, a word or less (`done`, `ready`); no `messageId` |
| Tool call (start) | `tool_use` | `tool_call` (`toolCallId`, `title`, `kind`, `status`, optional `name`, `rawInput`) | Covered | `kind` `edit` or `execute`; `title` was the tool name, then the path; `name` absent |
| Tool call (progress, end) | `tool_result` | `tool_call_update` (`status` `pending`, `in_progress`, `completed`, `failed`; `content`; `rawOutput`) | Covered | `completed` with a `diff` content and `rawOutput`; `failed` with an error text |
| Tool input bytes | the `tool_use` input | `rawInput` (optional) | Partial | empty `{}` in the first `tool_call`, filled in the first `tool_call_update` |
| Tool result bytes | the `tool_result` | `rawOutput` (optional), `content[]` (text, diff, terminal) | Partial | `rawOutput` for `write` was `{output, metadata}`; the written bytes were in the `diff` |
| Per-tool-call timing (start, end) | stream arrival times; telemetry for precision | no timestamps in any message. The host can time `in_progress` to the terminal status, and the permission request to its answer | Partial (host receipt times only) | `in_progress` arrived while the permission request was still unanswered (58 ms after the request, with the answer held), so it does not mean "running". Time from the answer, not from `in_progress` |
| Permission request | `control_request` `can_use_tool` | `session/request_permission { toolCall: ToolCallUpdate, options[] }` | Covered, different shape (section C) | measured |
| Usage per turn (tokens) | `result.usage` | `PromptResponse.usage` (`inputTokens`, `outputTokens`, `thoughtTokens`, `cachedReadTokens`, `cachedWriteTokens`, `totalTokens`): **unstable only** | Partial | sent anyway; last step only; zeros after cancel |
| Usage (context size) | `get_context_usage` (inferred from the binary) | `UsageUpdate { used, size, cost? }` (stable) | Partial | never sent |
| Cost per turn | `total_cost_usd` (running per process) | `UsageUpdate.cost { amount, currency }`: cumulative session cost, optional | Partial | never sent (local model) |
| Cache write split by tier (1 h, 5 min) | `result.usage` | `cachedWriteTokens` only; no tier | **Gap** | n/a |
| Compaction | `system/compact_boundary` | `compaction_update` and `compaction_summary_chunk` (stable; agent MUST send only if the client advertised `session.compaction`) | Covered when the agent implements it | advertised by the client; none emitted. `/compact` is listed as an available command, not triggered |
| Usage limit with reset time (`limited`) | `rate_limit_event`, `resetsAt` | none. No rate-limit concept, no reset time anywhere in either schema. Possible vehicles: a JSON-RPC error, a `notice` (`severity: error`, free text, live-only), or `_meta` | **Gap** | not reproducible locally; an API failure produced nothing at all (below) |
| API failure (`is_error`, `api_error_status`) | `result` fields | JSON-RPC error on `session/prompt` (agents "MUST catch" abort errors and return `cancelled`); `refusal` stop reason; `notice` | Partial; **opencode drops it** | `end_turn`, zeros, no update, no error; text on stderr only |
| Hook events (`system/hook_started`, `hook_response`) | `--include-hook-events` | none | **Gap** | n/a |
| Refusal source (hook, user, config) | `tool_result_meta` | none. Only `tool_call_update` `failed` with free text | **Gap** | Collide's own reject: `Error: The user rejected permission to use this specific tool call.` A cancel: `Tool execution aborted`. Both `failed`, no source field |
| Plan / todo | not in the normalized events | `plan` (`entries[]`), unstable `plan_update` | Covered, unused | none emitted |
| Available slash commands | not used | `available_commands_update` | Covered, unused | `init`, `review`, `compact` |
| Final report | `result.result` (last assistant message) | none. The text is the `agent_message_chunk`s since the last tool call, ended by the `session/prompt` response | Partial (reconstructed) | worked: `done`, `ready`, `collide` |
| Turn end | `result` (`subtype`, `is_error`, `terminal_reason`) | `session/prompt` response `stopReason`: `end_turn`, `max_tokens`, `max_turn_requests`, `refusal`, `cancelled` | Covered | see cancel and failure rows |
| Unmapped update | n/a | any `sessionUpdate` value the adapter does not know (the schema allows new ones and `_meta`) | needs an `Other` event | n/a |

### C. Permission requests and answers

| Aspect | Claude | ACP | Verdict | Observed |
|---|---|---|---|---|
| Request id | `request_id` (uuid, ours to echo) | JSON-RPC `id` of an agent-to-client request (number) | Covered | ids 0, 1, 2… per process |
| Join to the tool call | `tool_use_id` | `toolCall.toolCallId` | Covered | same id as the later `tool_call` |
| Tool name and input | `tool_name`, `input` (always) | `toolCall.title`, `kind`, `rawInput` (all optional) | Partial | `write`: `rawInput` had `filepath` and a unified `diff`. `bash`: `rawInput` was `{}`: no command at request time |
| Why was it asked (rule, hook, mode) | `decision_reason_type`, `decision_reason` | none | **Gap** | n/a |
| Suggested rule changes | `permission_suggestions` | none; the options are the only suggestions | n/a | n/a |
| Answer shape | `allow` with `updatedInput`, or `deny` with `message` | `outcome: selected { optionId }` or `cancelled`. Options are agent-defined: `{optionId, name, kind}` with `kind` one of `allow_once`, `allow_always`, `reject_once`, `reject_always` ("`kind` is only a UI hint") | Partial | options were `once` (allow_once), `always` (allow_always), `reject` (reject_once); no `reject_always` |
| Message to the model on deny | `message`, shown to the model verbatim | none | **Gap** | the model saw opencode's fixed text |
| Edit the tool input on allow | `updatedInput` | none | **Gap** | n/a |
| Persisting a rule | `updatedPermissions` (we never send it) | an `allow_always` option; "The Agent owns remembered decisions" and its scope is not fixed | Covered by never choosing it | `always` offered on every request |
| Answer on cancel | `interrupt` | the client MUST answer pending requests with `cancelled` after sending `session/cancel` | Covered | answered 51 ms after the cancel; the turn ended 114 ms later |
| Host goes away with a request pending | every later call denied; `result/success`, exit 0 | not in the spec | different | agent exited 0 within 200 ms; no prompt response; the file was not written |
| Agent asks the user a question (not a tool) | `AskUserQuestion` as a `can_use_tool` | `elicitation/create` (form or url), only if the client advertises it | Partial | not advertised |

### D. Control, resume, termination

| Member | Claude | ACP | Verdict | Observed |
|---|---|---|---|---|
| Send a turn | user message on stdin | `session/prompt` (content blocks) | Covered | |
| Cancel a turn | `control_request` `interrupt`; then close stdin | `session/cancel` (notification). The agent MUST answer the pending `session/prompt` with `cancelled`, after any late updates | Covered; **opencode deviates** | `cancel-run`: tool `failed` (`Tool execution aborted`) 288 ms after the cancel, prompt answered `end_turn` 317 ms after it, usage zeros, `sleep` gone, the same session answered the next turn (`ready`). `cancel-perm`: prompt answered 165 ms after the cancel |
| Resume | `--resume` | `session/load` replays the whole conversation as `session/update` before it responds. `session/resume` does not replay | Covered | `load`: 272 ms; replay was 2 `user_message_chunk`, 2 `tool_call_update` (one per tool call, final status only, no `tool_call`) and 1 whole `agent_message_chunk` for 2 turns; the session id was unchanged; the model remembered `hello.txt`'s content. `resume`: ok in 38 ms |
| List sessions | transcripts on disk | `session/list` (capability `sessionCapabilities.list`), `cwd` filter | Covered | listed the session with `cwd`, `title`, `updatedAt` |
| Stop a child | interrupt, close stdin, then 0.5 | `session/cancel`, wait for the response, close stdin, then 0.5; `session/close` exists (capability) | Covered | stdin EOF while idle: exit 0 in 57 ms. SIGTERM unmeasured |
| Outcome of a phase | `result` (and the settle rule: exit plus stdout EOF) | the `session/prompt` response only. Exit alone is not an outcome | different | exit 0 with no response happened (`exit-pending`) |
| Take-over (interactive TUI in a PTY, resume by id) | `claude --resume <id>` | none: ACP has no TUI. An agent's own CLI on the same session store | **Gap** (per-agent command) | `opencode --session <id>` exists in `--help`; not run |
| Background sessions (`background`) | `claude --bg`, not used in v0 | none | **Gap** | n/a |
| Subagents / agent teams | agent definitions | unstable `subagent_update`, `session_message` | unstable only | n/a |

### E. The seven capabilities in 0.3

| Capability (plan) | Claude | ACP | Verdict |
|---|---|---|---|
| `headless_stream` | `-p` with stream-json | ACP is always a structured stdio stream | true for every ACP agent |
| `permission_host` | `--permission-prompt-tool stdio` | `session/request_permission` is baseline for clients to implement, but an agent "MAY" ask, and may never ask | true in protocol; the answer shape is weaker (C) |
| `session_resume` | `--resume` | `loadSession` (replay), `sessionCapabilities.resume` (silent), or neither | needs three values, not a boolean |
| `background` | not used | none | false |
| `budget_cap` | `--max-budget-usd` | none | false; the host can enforce it only with cost reports |
| `agent_definitions` | `--agents` | `systemPrompt` (new, optional), modes | partial; splits |
| `usage_report` | `result.usage`, `total_cost_usd` | `PromptResponse.usage` (unstable), `UsageUpdate` (stable) | partial; needs a quality field |

## Gaps

Each gap is a trait member, normalized event or behaviour of 0.3 that ACP cannot express. "Declared" is how the proposal
makes it safe.

| # | Gap | Plan place | Declared as |
|---|---|---|---|
| G1 | Collide cannot choose the session id | 0.3 launch line (`--session-id`); 0.2 recovery by `--resume <session-id>`; 0.2 shim run directory | `SessionStart::AgentAssigned` and a `SessionBound` event. The id is recorded when it arrives |
| G2 | No budget cap | 0.3 `--max-budget-usd`, `budget_cap`, `error_max_budget_usd` | `budget_native`; host-enforced budget needs `usage_report` with cost |
| G3 | No limit signal, no reset time | 0.3 "Usage and rate limits" (`limited` event, pause, resume at reset) | `limit_signal`; a role that needs the pause requires it |
| G4 | No hook events | 0.3 `--include-hook-events`; the hook-outcome record | `hook_events` |
| G5 | No refusal source; no "why asked"; no deny message; no `updatedInput` | 0.3 "refused tool calls and hook outcomes"; Queue items showing `decision_reason`; deny answers | `refusal_source`; optional fields on the request and answer |
| G6 | Usage is weak: per-turn tokens only in the unstable schema, last step only in opencode, no cost in opencode, no cache-write tier | 0.3 "Context and cache"; 0.13 agent-stream row (per-turn totals, cache writes by tier, per-turn cost) | `usage_report` with granularity and completeness; `Option` fields |
| G7 | Compaction needs the agent to implement the (stable) update | 0.3 `compact_at`, 0.13 compactions | `compaction_events` |
| G8 | Permission request may lack the input | 0.3 permission answers; Queue items | `input: Option`, plus a rule that an unknown input cannot be auto-allowed by a pack's input-based policy |
| G9 | Agent definitions: no tool allow list, no strict config or MCP isolation, no settings snapshot | 0.3 `--agents`, `--disallowed-tools`, "The tool's config paths in a repo", the snapshot rule | `tool_allowlist`, `strict_config`; per-agent profile lists the config paths |
| G10 | Context knobs (`compact_at`, output caps) | 0.3 "Context options" | `context_options`: the set of knobs the adapter supports; a role setting another is refused |
| G11 | A model id is an opaque agent string, set after the session starts | 0.3 `model.name`, the safe-token rule (0.5) | `model_select`; `model.id` beside `model.name` |
| G12 | Cancel's stop reason cannot be trusted (opencode answers `end_turn`) | 0.3 "stopping a phase sends `interrupt`" | the session tracks `cancel_requested` and reports `Cancelled` itself |
| G13 | An API failure can end a turn as a clean empty `end_turn` | 0.3 "`result/success` does not mean the work happened" | `TurnEnd.empty`; an empty turn is a failure |
| G14 | Take-over has no ACP form; background has none | 0.3 take-over; "Agents are `claude -p` children, not `--bg`" | `take_over` (command from the profile), `background` stays false |

Not gaps, though easy to assume: per-tool-call timing (host receipt times are the same source Claude's stream gives,
see 0.13 "Timings approximate"), tool input and result bytes (the host can measure `rawInput` and `rawOutput` when the
agent sends them), working directory and worktree (`session/new.cwd`), environment (spawn env, as for Claude), MCP
servers (better than Claude's flag: per session, in the protocol), and exit semantics (a child exit is still observed by
the shim; only its meaning changes).

## Live-run observations

All from `.collide/planning/spikes/sp-0006-kq-agentadapter-acp/out/live-run.jsonl` (the final run). Times are from the driver's clock.

### What matched the spec

- **Handshake.** `initialize` with `protocolVersion: 1` returned `loadSession: true`, `mcpCapabilities {http, sse}`,
  `promptCapabilities {embeddedContext, image}`, `sessionCapabilities {fork, list, resume}`, one auth method and
  `agentInfo`. `session/new` returned a session id plus `models` and `modes`.
- **Permission request and answer.** `session/request_permission` arrived as an agent-to-client request. Answering
  `selected` with the `allow_once` option's id ran the tool; answering the `reject_once` id made the call `failed` and
  the file was not written. The model was told only opencode's fixed text.
- **Tool-call lifecycle.** `tool_call` (`pending`), `tool_call_update` (`in_progress`), then `completed` or `failed`,
  joined by `toolCallId`. `kind` (`edit`, `execute`) and `locations` were set.
- **Client file methods.** Because the client advertised `fs.writeTextFile`, opencode wrote the file by calling
  `fs/write_text_file` on the driver (after the permission answer). The driver had to validate the path itself. With
  `fs` advertised, the daemon would be in the agent's write path.
- **Cancel of a pending permission request.** After `session/cancel`, answering the request `cancelled` ended the turn
  165 ms after the cancel, with the tool call `failed` and the file absent.
- **Resume.** `session/load` into a new process: 272 ms, same session id, history replayed, context kept (the model
  answered `collide` about a file from the earlier process). `session/resume` also worked, with no replay.
- **Idle shutdown.** Closing stdin while idle: exit 0 in 57 ms. The child needs no signal.

### Where opencode differs from the spec, or the spec is silent

| # | Observed | Spec says | Effect on the adapter |
|---|---|---|---|
| O1 | `session/cancel` answered with `stopReason: "end_turn"` | MUST be `cancelled` | track `cancel_requested` in the session; never rely on `cancelled` |
| O2 | `PromptResponse.usage` is the last model step. Turn 1: step 1 = 9,414 tokens (34 in, 49 out, 9,331 cache read), step 2 = 9,433 (53 in, 2 out, 9,378 cache read); ACP reported 9,433. Zero after cancel although step 1 had run | `usage` is unstable and its scope is "a prompt turn" | token totals from ACP are lower bounds; a record must carry `quality: partial` |
| O3 | no `usage_update` and no cost in any turn | `UsageUpdate` optional; cost optional | cost is `None`, not 0; a budget cannot be enforced |
| O4 | model not found: `end_turn`, zeros, 245 ms, no update, no error. stderr: `AI_APICallError: model 'nosuch:1b' not found` | agents "MUST catch" abort errors; nothing on API errors | an empty turn must fail the phase |
| O5 | permission request for `bash` has `rawInput: {}`, `title: "bash"`; the command appears in `tool_call_update` only after the answer | `rawInput` is optional | the Queue item cannot show the command, and an input-based auto-rule cannot run |
| O6 | the permission request precedes the `tool_call` for the same id (request at 25,210 ms, `tool_call` at 25,265 ms in `cancel-perm`); `in_progress` arrives while the request is still unanswered | spec examples send `tool_call` first | the session must accept an update for a call it has not seen, and must not read `in_progress` as "running" |
| O7 | stdin closed with a request pending: exit 0, no prompt response | silent | a phase is settled by the prompt response; exit without one is `lost`/failed, never success |
| O8 | config's `model` was not used by `session/new` (`currentModelId` stayed `qwen2.5:7b` with `model` set to another); `session/set_model` worked | silent | the adapter sets the model explicitly after `session/new` and checks `currentModelId` |
| O9 | `session/new` returns `models` (draft shape), not `configOptions`; `session/set_model` exists; 1.31's stable schema has neither | stable: `configOptions` with `category: "model"` | the adapter must read both forms |
| O10 | replay on `session/load` has no `tool_call`, only each call's final `tool_call_update`, and no `usage` | MUST replay "the entire conversation" | replayed events are marked and not counted into the record |
| O11 | opencode lists its own hosted models next to the Ollama ones (`opencode/big-pickle`, …) in `availableModels` | n/a | an adapter must validate the chosen model against an allow list: a role must never reach a hosted model by accident |
| O12 | `tool_call.title` changes from the tool name to the path; `name` is never sent | `name` is the programmatic name, optional | use `kind` and `rawInput`, not `title`, for classification |

Per-tool-call timing from the host side, `allow` scenario: permission request 18,887 ms, answered 18,915 ms (28 ms
later, the driver's own delay), `fs/write_text_file` 19,008 ms, `tool_call` 19,270 ms, `completed` 19,281 ms, last
message chunk 20,458 ms, prompt response 20,576 ms. So the first `tool_call` notification came 260 ms after the write
had finished on the driver: tool timing taken from update arrival is late by the agent's own bookkeeping. For
`cancel-run`: `in_progress` 30,845 ms, cancel 34,003 ms (a `sleep 47` three seconds old), `failed` 34,291 ms,
prompt response 34,320 ms.

### What was not exercised

See "Left unmeasured".

## Proposed trait changes

All are labelled **core**: the trait and types live in `core`, which has no process, Tauri or SQLite types. Everything
below stays bytes and values.

### Shape: three options

| Option | What it is | Pros | Cons |
|---|---|---|---|
| **A (proposed)** | `AgentAdapter` (static declarations and launch) plus `AgentSession` (per-child sans-IO state machine; bytes in, events and bytes out) | core stays free of async and process types; testable with a fake child and a log file; replaying a log rebuilds the state after a daemon restart (0.2 reattach); fits both Claude (almost stateless) and ACP (request ids, pending requests) | more surface than the plan's one trait; Claude's session is trivial |
| B | Keep one stateless trait. The ACP adapter lives behind a bridge that rewrites ACP into Claude-shaped stream-json | no trait change | the bridge becomes a second, unspecified adapter that has to invent `resetsAt`, `total_cost_usd`, `is_error` and hook events; it hides the gaps instead of declaring them, which is what 0.3's "refuses to start rather than silently losing it" rules out |
| C | Use ACP's own Rust SDK as the abstraction and make Claude an ACP agent through an adapter binary | one protocol | adds a dependency in `core`'s interface; Claude's `can_use_tool`, `--max-budget-usd` and hook events are not ACP, so S1's measurements would be lost; the crate was not examined |

Option A is proposed. The record does not need B or C to proceed; C is the first thing to revisit if Claude Code ships
a first-party ACP mode (unmeasured).

### P1. Split the trait into an adapter and a per-child session. [core]

**Plan sections:** 0.3 ("`AgentAdapter` (core trait) turns a role + phase into a launch spec, parses the tool's stream
…"), 0.2 (the shim holds stdin; reattach), Stage 2 ("the traits `AgentAdapter` …").

Sketch (names are proposals, not signatures to copy):

```
trait AgentAdapter {                       // one per tool; no state
    fn capabilities(&self) -> Capabilities;
    fn launch(&self, req: &LaunchRequest) -> Result<LaunchSpec, LaunchError>;  // argv, env, cwd, strip list
    fn session(&self, req: &LaunchRequest) -> Box<dyn AgentSession>;
    fn repo_config_paths(&self) -> &[PathPattern];
    fn take_over(&self, s: &SessionRef) -> Option<CommandSpec>;
}
trait AgentSession {                       // one per child; deterministic in its input
    fn start(&mut self) -> Step;                           // ACP: initialize, session/new|load|resume, first prompt
    fn feed(&mut self, line: &[u8], at: Instant) -> Step;  // one line of the child's stdout
    fn send_turn(&mut self, text: &str) -> Step;
    fn answer(&mut self, req: RequestId, a: PermissionAnswer) -> Step;
    fn cancel(&mut self) -> Step;
    fn finish(&mut self, exit: ExitInfo) -> PhaseOutcome;
}
struct Step { events: Vec<Event>, writes: Vec<Vec<u8>> }
```

- Reason: ACP needs a handshake the host writes, and every agent request (permission, `fs/*`) needs a reply with the
  request's id. A parser alone cannot do either (G-shape, from the mapping).
- Replay: `feed` over a child's whole log must reproduce the session state, including pending request ids. S2/S3's
  recovery tails the log after a daemon restart; for ACP the pending set comes from the log (an agent request with no
  reply written yet). This needs a rule that replies are logged too (the shim writes the stdin bytes to a file).
  Unmeasured: the log of host-to-agent bytes is not in 0.2 today.
- A Claude session implements `start` as "nothing to write" (the launch line carries the first turn) and `feed` as
  the S1 parser.

### P2. Session id as an event. [core]

**Plan sections:** 0.3 launch line (`--session-id <uuid>`; resumed phases pass `--resume <uuid>`), 0.2 recovery, 0.8
data model (where a phase's session id is stored; not read for this spike).

- `LaunchRequest.session: SessionStart::{ Assign(Uuid), AgentAssigned, Resume(SessionId) }`. Claude supports `Assign`
  and `Resume`; ACP supports `AgentAssigned` and `Resume`.
- `Event::SessionBound { id }`, emitted when the id is known (Claude: at once from the launch line; ACP: on the
  `session/new` response). The store records it then.
- Window: if the daemon dies between spawn and `SessionBound`, the id is lost. Recovery lists sessions by `cwd`
  (`session/list`, if advertised) or fails the phase as `lost`. Claude has no such window.

### P3. Permission answers and requests. [core]

**Plan sections:** 0.3 (stream protocol: "permission answers (`control_response`, allow or deny)"; "a hook `ask` arrives
as an ordinary `can_use_tool` request"), 0.11 Queue items.

- `PermissionRequest { id, tool_call_id, tool: Option<String>, kind: ToolKind, input: Option<Value>, input_complete:
  bool, reason: Option<Reason>, options: Vec<PermOption> }`.
- `PermissionAnswer::{ Allow, Deny { message: Option<String> }, Cancel }`. The Claude adapter maps `Allow` to
  `behavior: allow` (no `updatedPermissions`) and `Deny` to `deny` with `message`. The ACP adapter maps `Allow` to the
  option of kind `allow_once` and `Deny` to `reject_once`. It drops `message` (capability `deny_message` false), and
  if no `allow_once` option exists it answers `Cancel` and reports a `PermissionUnanswerable` event. **It never picks
  an `allow_always` or `reject_always` option**: the agent owns the scope and storage of those (spec), and they would
  write rules into the agent's own config.
- A request whose `input` is `None` is shown in the Queue as "input not available until approved" and is never
  auto-allowed by a pack rule that looks at the input (O5).
- Elicitation is not advertised in v0, so an ACP agent that wants to ask the user falls back to its own behaviour.

### P4. Usage events with provenance. [core]

**Plan sections:** 0.3 ("Context and cache", "Usage and rate limits"), 0.13 (the "Agent stream" and "Interactive
sessions" rows), 0.5 (`usage` lines).

- `Event::Usage { scope: Turn | Step | RunningTotal, input, output, cache_read, cache_write, thought, context_used,
  context_size, cost: Option<Money { amount, currency }>, quality: Exact | Partial | Unreported }`, every number an
  `Option`.
- Claude: exact, `RunningTotal` cost in USD, cache write split by tier (kept as a Claude-only detail field).
- ACP: `Partial` from `PromptResponse.usage` (and `Unreported` after a cancel), `Exact` for context from `UsageUpdate`.
  A measured total built from partial figures is shown as "at least", never summed with exact ones.
- The budget: `budget_native` (Claude) lets 0.3 pass `--max-budget-usd`. For any adapter with `usage_report.cost`
  the core enforces the budget by sending `cancel` when the running cost passes it (it may overshoot, as Claude's cap
  does). For an adapter without cost, a role that sets a budget requires `budget_native` or `usage_report.cost` and is
  refused otherwise.

### P5. Capabilities. [core]

**Plan sections:** 0.3 (the list `headless_stream`, `permission_host`, `session_resume`, `background`, `budget_cap`,
`agent_definitions`, `usage_report`; "A role may `require` capabilities"), Stage 3 ("`AgentAdapter` capabilities per
S7"), 0.3 "A user can override a role's tool per board".

| Plan name | Proposed |
|---|---|
| `headless_stream` | keep |
| `permission_host` | keep; sub-flags `deny_message`, `input_at_request`, `request_reason` |
| `session_resume` | `session_resume: none \| replay \| silent` |
| `background` | keep (ACP false) |
| `budget_cap` | `budget_native`; a host-enforced budget derives from `usage_report.cost` |
| `agent_definitions` | split: `system_prompt: none \| append \| override`, `tool_allowlist`, `named_agent` |
| `usage_report` | keep, with `granularity` (`turn`, `step`, `running_total`), `cost`, `cache_write_tiers`, `context`, `quality` |
| (new) `limit_signal` | a structured limit with a reset time (Claude true; ACP false) |
| (new) `compaction_events` | Claude true; ACP true only if the agent sends the update |
| (new) `hook_events` | Claude true; ACP false |
| (new) `refusal_source` | Claude true; ACP false |
| (new) `strict_config` | the adapter can run with nothing from the worktree or the user's agent config (Claude: the snapshot line; ACP: per profile) |
| (new) `context_options` | the set of `compact_at`, `bash_output_max`, `mcp_output_max` the adapter supports (Claude all three) |
| (new) `model_select` | `launch` (Claude) or `session` (ACP) or `none` |
| (new) `take_over` | the adapter can name an interactive command for a session |
| (new) `mcp_servers` | `stdio`, `http`, `sse` |

An adapter reports each as `Yes`, `No` or `Unknown` (a profile for an ACP agent that has not been probed). A role that
requires a capability the adapter reports `No` or `Unknown` is refused at launch, as 0.3 already says.

### P6. Turn end and outcome. [core]

**Plan sections:** 0.3 (stream protocol: "a phase settles once the child has exited and its stdout has reached its
end"; "`result/success` does not mean the work happened"; "stopping a phase"), 0.2 (shim, `lost`).

- `Event::TurnEnd { reason: EndTurn | MaxTokens | MaxTurns | Refusal | Cancelled | Error { status }, cancel_requested:
  bool, empty: bool }`. `empty` is true when the turn produced no message, no tool call and zero or unreported usage.
- ACP: a turn the host cancelled is `Cancelled` whatever `stopReason` says (O1). An `end_turn` with `empty` is a
  failed phase with cause `empty_turn` (O4), carrying the agent's stderr tail as the reason. A turn with `refusal` or
  `max_tokens` fails the phase with that cause.
- The phase outcome of an ACP session is its prompt response. A child that exits before one is `lost` (O7). The 0.2
  settle rule (exit plus EOF) still decides when to stop reading; it no longer decides success.
- Final report: `FinalReport { text, source: ResultField | Reconstructed }`. ACP reconstructs it from the message chunks
  after the last tool call.

### P7. Model naming. [core]

**Plan sections:** 0.3 ("Each mapping also names `model.name` … Collide requires only a safe token (0.5)"), 0.5 safe
token (`[A-Za-z0-9._-]`, 1 to 64 characters).

- `ollama/qwen2.5:7b` and `opencode/big-pickle/high` fail the safe token (`/` and `:`). So `model.name` stays the safe
  alias label (used in env, paths, labels, `requires.modelName`), and `models.yaml` gains an opaque `model.id` that
  only the adapter reads and never puts in env, paths or argv. For ACP it is the agent's `modelId`, checked against the
  advertised list after `session/new` (O8, O11). For Claude it is `model.name` as today.

### P8. An ACP adapter is an engine plus a profile. [core interface; host implementation]

**Plan sections:** 0.3 "Later adapters" (in 0.3's bullet list of tools and in Deferred), Stage 2 `host` outline.

- Protocol handling (JSON-RPC, handshake, update parsing, permission mapping) is one engine in `host`.
- Everything that varies is data in a per-agent profile: argv, env to strip and to set, repo config paths and how to
  turn off the agent's reading of other tools' config, take-over command, model-id mapping, known deviations (O1, O4,
  O5). The profile for opencode is this record's table O1–O12.
- `core` holds only the interface from P1; nothing ACP-shaped enters `core` except the capability names above.

### P9. Client capabilities to advertise. [core decision, host implementation]

**Plan sections:** 0.2 ("Children never talk to the owner over pipes"; the shim holds stdin), 0.3.

| Choice | Pros | Cons |
|---|---|---|
| **Advertise no `fs`, `terminal`, `elicitation` (proposed for v0)** | the agent keeps its own file and shell tools; Collide stays out of the data path, so a daemon restart cannot stall a write; no path-validation code to get wrong | Collide does not see commands as client-run processes, so per-command rusage and env markers come only from 0.5's existing sweep |
| `fs` | the host sees every write's content | every write blocks on the daemon; path boundary checks become Collide's job (the driver had to add one); more load on the owner |
| `terminal` | commands run as children of the shim with Collide's markers and `outputByteLimit`, and the host gets exit status | changes the process tree (S3's sweep would need new rules); every shell call stalls if the daemon is down; unmeasured |

Advertise `session.notices` and `session.compaction` (observability only; an agent may ignore them) and `configOptions`.
Add `elicitation` when a Queue item type for agent questions exists (not in v0).

## Proposed plan changes

The plan was not edited. Each item names the section it affects. All are `core` unless marked.

1. **0.3 first `AgentAdapter` bullet; Stage 2 "`core` outlines".** Replace "turns a role + phase into a launch spec,
   parses the tool's stream into normalized events, answers permission requests, and declares capabilities" with the
   two-trait shape of P1, and add `SessionBound`, `TurnEnd`, `FinalReport.source`, `Usage.quality` and an `Other` event
   to the normalized events. Stage 2's line "S7's trait changes are in" then names P1–P7.
2. **0.3 capability list; Stage 3 "`AgentAdapter` capabilities per S7".** Replace the seven names with the table in
   P5. State that a capability is `Yes`, `No` or `Unknown`, and that `No` and `Unknown` refuse a role that requires it.
3. **0.3 launch line and "Who picks what".** After the Claude launch line, add: "a session starts with
   `Assign(uuid)`, `AgentAssigned` or `Resume(id)`; the id is recorded on `SessionBound`". Add `model.id` (opaque, adapter
   only) beside `model.name` (safe token), and say `model.id` never reaches env, paths or argv.
4. **0.3 stream protocol, last bullet ("stopping a phase").** Add the ACP form: `session/cancel`, wait for the prompt
   response, close stdin, then 0.5; the session reports `Cancelled` itself because agents may answer `end_turn`.
5. **0.3 "`result/success` does not mean the work happened".** Add: for an adapter whose protocol has no failure flag
   (ACP), a turn that ends with no output and no usage is a failed phase, `empty_turn`; the agent's stderr tail is the
   evidence recorded.
6. **0.2 shim.** The settle rule ("exit plus stdout EOF") decides when reading stops; the outcome of an ACP phase is
   its prompt response, and a child that exits without one is `lost`. Also: the shim logs the bytes written to stdin as
   well as stdout, because an ACP session is rebuilt by replaying both (P1).
7. **0.3 "Usage and rate limits".** Say a `limited` event exists only for adapters with `limit_signal`; for others the
   phase fails with the agent's message and a role may require `limit_signal` to get the pause-and-resume behaviour.
8. **0.3 launch line (`--max-budget-usd`) and "Cost".** The budget is native (`budget_native`) or enforced by the
   core from cost reports (P4); a role with a budget requires one of them.
9. **0.13 metrics, "Agent stream" row.** Add a row or a note for adapters with `usage.quality: partial`: tokens are a
   lower bound and are shown as "at least"; cost is `None`, not 0, when unreported.
10. **0.3 Queue/permissions (and 0.11 Queue item).** A permission request whose input is unknown shows "input not
    available until approved", and cannot be auto-allowed by an input-based rule (P3).
11. **Stage 1 → S7 and the Success Criteria.** Mark S7 done with this record; the criterion "the trait has no member
    that S7 found ACP cannot express, or each such member is marked as a capability" is met by P1–P7 once applied.
12. **Stage 2 "Contracts in full" / Stage 3 contract test.** Add a second implementation of `AgentSession` to the
    contract tests, a scripted fake ACP child that reproduces O1–O7 (agent-assigned id, no cost, partial usage,
    cancel answered `end_turn`, silent API failure, exit on stdin EOF). It keeps the trait from being shaped by Claude
    alone, and it needs no model. [host test]
13. **Deferred, "Other agent tools and model sources".** Replace "ACP adapter" with: an ACP engine plus a profile per
    agent (P8), opencode first. Record that profiles are probed by their `initialize` response and that an unprobed
    capability is `Unknown`.

## Left unmeasured

- **Other ACP agents.** Only opencode 1.1.56 with one 7B model. goose (1.49.0 is installed) and Zed's Claude adapter
  were not run (the second would need Claude calls, which this spike may not make). So every "opencode differs" row
  (O1–O12) may be that implementation's behaviour; the "Gap" rows are protocol-level and hold for every agent.
- **A real usage limit, a hook, a compaction, a plan update, `thought` tokens, a paid provider's cost.** opencode
  emitted none of them. The compaction and notice paths are advertised by the client and untested (opencode never
  sent them; `/compact` was not triggered).
- **MCP servers through `session/new`.** Only `mcpServers: []` was sent. Whether opencode ignores its own configured
  MCP servers (`strict_config` for MCP) is unmeasured.
- **`systemPrompt`, `session/set_config_option`, `session/fork`, `session/close`, `session/delete`, and the unstable
  `providers/*` methods** (which could set an Ollama base URL from the host without a config file). opencode advertised
  none of them but `fork`, `list` and `resume`.
- **`terminal/*`, `fs/read_text_file` and `elicitation/*`.** The client advertised `fs` only, and opencode called
  `fs/write_text_file` once per write and never `fs/read_text_file`. `terminal` and `elicitation` were off.
- **Signals.** SIGINT and SIGTERM on `opencode acp`; killing the daemon mid-turn; an orphaned grandchild after
  `session/cancel` (the one `sleep` case was clean). S3's sweep was not applied.
- **Reattach across a daemon restart with an ACP request pending.** P1's claim (rebuild by replaying both log
  directions) is reasoned, not run. The plan does not log host-to-agent bytes today.
- **Take-over.** `opencode --session <id>` on a session ACP created was not started (it is a TUI).
- **Many sessions per process, large outputs, truncation, a long idle process, and Windows.** One process per phase
  is assumed (the shim owns one child).
- **Model quality.** A 7B model needed explicit prompts to call the tool (a "Do not answer with text first" line); an
  earlier, discarded run saw it call `bash` without the required `description` argument and answer with text instead
  of a call. Nothing here says how well a model drives an ACP agent.
- **How the resumed session's `usage` and cost combine across `session/load`.** Claude's cost accumulates across
  `--resume` (0.3); whether an ACP agent's cumulative `UsageUpdate.cost` does is unmeasured (opencode sends none).
- **The ACP Rust SDK crate** was not examined (option C).
- **ACP's evolution.** The schema release read is 2026-11-10 and the stable set has moved recently (compaction,
  notices, `systemPrompt`, `resume` and `usage` all appear in 1.31). The unstable items used here (`usage` in
  `PromptResponse`, providers, subagents) may change.

## Reproduction

```
cd .collide/planning/spikes/sp-0006-kq-agentadapter-acp
./run.sh                 # all scenarios; about 70 s warm, longer on a cold model
./run.sh init allow      # chosen scenarios
```

Needs `~/.opencode/bin/opencode` 1.1.56, a running Ollama server with `qwen2.5:7b` pulled, and Node 24. `run.sh`
resets opencode's data under `work/`, copies `opencode.json` in, and writes `out/live-run.txt` and
`out/live-run.jsonl`. The schema release is in `work/spec/`:

```
gh release download schema-v1.31.0 -R agentclientprotocol/agent-client-protocol -p schema.json -p schema.unstable.json
node work/spec/dump.mjs schema.json SessionUpdate UsageUpdate RequestPermissionRequest
```

## Spend

None. No Claude or paid API calls; the model ran locally on Ollama.
