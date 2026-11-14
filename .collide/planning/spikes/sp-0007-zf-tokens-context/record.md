---
id: sp-0007-zf
alias: S9
kind: spike
status: concluded
title: "S9: Tokens and context"
owners: []
links: []
---
# S9: Tokens and context

- **Date:** 2026-11-13. Plan revision 10 (`IMPLEMENTATION_PLAN.md`, 0.3 "Context and cache", 0.7 `on_cold` and context
  options, 0.13 "Agent stream" and "Token and context metrics", Stage 1 → S9). The plan is not edited here; the last
  section proposes changes.
- **Versions:** `claude` 2.1.323 (claude.ai login, `apiKeySource: "none"`), node v24.18.0, macOS 26.3.1 arm64. `haiku`
  resolved to `claude-haiku-5-5` (context window 1,000,000 as the model default). Every call used haiku.
- **Reproduction:** `.collide/planning/spikes/sp-0007-zf-tokens-context/`. `README.md` lists the scripts in order; `run-all.sh` runs them from a
  clean state and writes `out/<script>.txt`. Question 6 needs more than an hour of idle time, so a full run takes a
  little over an hour.
- **Method:** every call ran through `claude-clean.sh` (S1's copy; its keep list is now `S9_PASS`) with
  `--max-budget-usd` on every call (0.25 unless a script says more). `driver.mjs` and `summarize.mjs` are S1's.
  Scratch repos are `work/idle` (ledger1-4.js), `work/q4` (ledger1-8.js) and `work/empty`; each ledger file is 120
  generated functions, about 17 KB and 11K tokens. `usage-compare.mjs`, `usage-aggregate.mjs` and
  `result-usage-check.mjs` read this spike's own transcripts under `~/.claude/projects/*collide-spikes-s9*`,
  read-only. The hook for Q5 is `hooks/block-prompt.sh`, loaded only through the spike's own `--settings` file with
  `--setting-sources ""`.
- **Labels:** **measured** (script and observed output), **inferred** (reasoned from measurements, `--help` or the
  binary's strings), **unmeasured**.

## Summary

| # | question | answer |
|---|---|---|
| 1 | usage per content block; `compact_boundary` | the stream repeats one `usage` per content block, as transcripts do; assistant rows carry a **partial `output_tokens`**, so take per-turn totals from `result.usage`. `system/compact_boundary` is emitted for manual and auto compaction, with `compact_metadata` |
| 2 | cache tier | API-key tier **unmeasured** (no key in the environment). claude.ai login: 1-hour writes only in the main loop; the compaction call writes 181 tokens at the 5-minute tier |
| 3 | start weight | `config: user` 30.9-32.6K; `--setting-sources ""` 18.8K; the cleanest usable line 14.4K; no tools 3.2K |
| 4 | env variables | all three change the stream. Auto-compact window: minimum 100K, fires at window minus 33K. Bash and MCP caps change what enters the context |
| 5 | blocked prompt | `system/informational` with `prompt_submit_outcome`, then `result/success` with `num_turns: 0`, `is_error: false` |
| 6 | idle resume vs handoff | after 62 min idle, a cold resume wrote 63.5K tokens ($0.013) at 73K context and 103.9K ($0.105) at 114K; a fresh session with a handoff note cost $0.004 both times |

## Answers

### Q1. Usage per content block; `compact_boundary`

**Usage repeats per content block** (measured, `01-usage-compact.sh`, `usage-aggregate.mjs`):

- One model response with several content blocks (parallel `tool_use` blocks, or `thinking` then `text`) arrives as
  several `assistant` events with the same `message.id` and the same `usage`. In `01`, three ids came as 2 + 2 + 2
  rows. Summing every row gives double the cache tokens: `cache_read_input_tokens` 121,420 against 60,710 once per id.
- Across all 38 sessions of this spike (46 driver logs, each compared with its own transcript): 105 distinct ids; 66
  appear on more than one stream row (at most 4 rows); on all 66, every row has identical usage. The transcript has
  the same 66 repeats, also identical.
- **The input and cache fields are exact; `output_tokens` is not.** For the same id, `input_tokens`,
  `cache_read_input_tokens` and `cache_creation_input_tokens` equal the transcript's on 105 of 105 ids. `output_tokens`
  in the stream equals the transcript's on 11 ids and is lower on 94 (`01`: stream 21, 5, 4 against transcript 179, 90,
  174). The stream's assistant rows carry the output count from the start of the response; the transcript has the
  final one (inferred from the values; the mechanism is unmeasured).
- **`result.usage` is right per turn.** On all 71 results checked (`result-usage-check.mjs`), `result.usage` equals the
  per-id sums: input and cache from the stream, `output_tokens` from the transcript. So the adapter takes per-turn
  token totals from `result.usage` (or `model_usage_since_last_result`) and per-response context from the stream's
  deduped input and cache fields. Per-response output comes only from the transcript.
- **A compaction's own model call is not in `result.usage`.** In the auto-compaction turn of `04` (`A-100000`),
  `result.usage` shows input 4 and output 464, while `model_usage_since_last_result` shows input 22,157 and output 2,508
  (it includes the summarizer call). The cost (`total_cost_usd`) includes it. A manual `/compact` result has
  `usage` all zero and its whole spend (in 22,498, out 1,518, $0.0034) only in `model_usage_since_last_result`.

**`compact_boundary`** (measured, `01` for manual, `04` for auto):

- A manual compaction works from stdin: a user turn whose text is `/compact`. Event order: `system/status`
  `{status:"compacting"}`, the SessionStart hooks (`hook_started`/`hook_response` ×3 for this user), `system/status`
  `{status:null, compact_result:"success"}`, `system/init`, **`system/compact_boundary`**, a synthetic `user` event
  (`isSynthetic: true`, the summary text, "This session is being continued from a previous conversation..."), a `user`
  event `<local-command-stdout>Compacted </local-command-stdout>`, then `result/success` with `result: ""`,
  `num_turns: 0`, `local_command: "compact"`.
- The metadata, manual: `{"trigger":"manual","pre_tokens":53376,"post_tokens":10422,"cumulative_dropped_tokens":42954,
  "duration_ms":10314,"preserved_segment":{head_uuid,anchor_uuid,tail_uuid},"preserved_messages":{anchor_uuid,uuids,
  all_uuids}}`, plus `logical_parent_uuid` on the event. An auto compaction has the same keys with
  `"trigger":"auto"` (`04`, `A-100000`: pre 67,810, post 15,029, 8,194 ms).
- **An auto compaction can land mid-task.** In `04` the order was: an `assistant` response with `tool_use`,
  `system/status compacting`, `compact_boundary`, then the next response (timeline in Q4). The tool call in flight
  is kept in the preserved segment.
- **`post_tokens` is not the next response's context.** Manual: `post_tokens` 10,422, next response context 33,042
  (start weight under `config: user` about 32.6K). Auto, `config: none` line (start weight 14.4K): `post_tokens` 15,029,
  next response context 38,462. `pre_tokens` is close to the last response's context (manual: 53,376 against 53,274,
  an estimate). The metric "tokens after" should be the next response's context. Why `post_tokens` runs low is
  unmeasured.
- The per-turn `system/init` repeats on every turn (S1 saw this); a compaction turn has one more.

### Q2. Cache tier

- **API-key login: unmeasured.** No `ANTHROPIC_*` variable is set in this environment (`cache-tier.sh` lists the
  names), and every `init` says `apiKeySource: "none"`. Per the brief I did not look for a key. How to measure: with a
  key in the environment, run `01-usage-compact.sh` and `cache-tier.sh`, and compare
  `cache_creation.ephemeral_1h_input_tokens` with `..._5m_...`.
- **claude.ai login** (measured, `cache-tier.sh` over every log in `out/`): across 105 distinct assistant messages, 1-hour
  writes total 928,672 tokens and 5-minute writes 0; no message has a 5-minute write. This repeats S1's finding.
- **One exception, in the compaction call** (measured, `cache-tier.sh`): every compaction (one manual, three auto)
  reports `cacheCreationEphemeral5mInputTokens: 181` in `model_usage_since_last_result`, always 181. The summarizer
  request writes a tiny prefix at the 5-minute tier. The plan's "headless phases write the 1-hour cache only" holds
  for the main loop and is off by 181 tokens per compaction.

### Q3. Start weight and the `system/init` breakdown

**Start weight** is the context of the first response (input + cache read + cache write), for the same one-word prompt
in an empty repo (`03-start-weight.sh`, measured):

| line | first-response context | `init` shows |
|---|---|---|
| `--setting-sources user` (**`config: user`**) | **30,902-30,942 (twice) or 32,583 (twice)**; 31,868 in one variant | 32 tools, 5 MCP servers, 11-12 plugins, 6 agents, 138 skills, 193 slash commands |
| `--setting-sources ""` (**`config: none`**) | **18,847** (three identical runs) | 24 tools, 0 MCP servers, 4 plugins (all `@builtin`), 6 agents, 19 skills, 55 slash commands |
| `""` + `--strict-mcp-config` | 18,847 (no change; `""` already drops MCP) | same |
| `""` + `autoMemoryEnabled:false` | 18,132 (-715) | 24 tools |
| `""` + `--disable-slash-commands` | **21,982 (+3,135)**; skills 0, tools 23 | see below |
| `""` + `autoMemoryEnabled:false` + `--disallowedTools` (the 0.3 list: CronCreate, ScheduleWakeup, RemoteTrigger, PushNotification, EnterWorktree, ExitWorktree, Workflow, SendMessage) | **14,352**, the cleanest line that still has working tools | 16 tools |
| the same plus `--tools ""` | 3,151, a floor; no tools | 0 tools |

- **Cleanest flags: `--setting-sources "" --settings '{"autoMemoryEnabled":false}'` plus `--disallowedTools` for the
  disallowed list.** `--strict-mcp-config` is redundant with `--setting-sources ""` but harmless. (Spot-check
  correction, after the spike: not redundant. Run `h-b-again`, `--setting-sources ""` alone, listed the four
  claude.ai connectors (Claude Docs `pending`, three `needs-auth`) in `init`; 1 of the 5 runs without
  `--strict-mcp-config` did, 0 of the 5 with it. None added tools by the first response, so the token figures stand.
  Keep `--strict-mcp-config` on every line, as the plan's launch line already does.) `--bare` needs an
  API key (S1). `--disable-slash-commands` makes the start **heavier** (measured, repeated in `c` and `f`). I did not
  find out why; `get_context_usage` shows that tool deferral differs between lines (see below), which is a
  hypothesis only (inferred).
- **The user config adds 12-14K over `config: none`**, and the user-config figure moves by 1.7K between runs (30.9 and
  32.6K both recur; cause unmeasured). Attribution (`03` variants `k`-`n`, measured): MCP servers -2.1K (5 servers; only
  the connected one adds tools), SessionStart hooks -1.0K, auto memory -0.7K; with all three off the line is 28,418,
  which leaves 9.6K for the user's CLAUDE.md, `RTK.md` and plugin skills.
- **`system/init` gives names and counts, not tokens** (measured; the `init` keys are `tools`, `mcp_servers`
  (name and status), `plugins` (name, path, source), `agents`, `skills`, `slash_commands`, `terminal_slash_commands`,
  `capabilities`, `memory_paths`, `model`, `permissionMode`, `apiKeySource` and others). `config: user` listed
  `forgetful=pending`, `claude.ai Claude Docs=connected`, and three claude.ai servers as `needs-auth`; skills were 138 (user plugins and built-ins) against 19 built-in.
- **The token breakdown comes from a control request.** The binary's strings contain `get_context_usage`; sending
  `{"type":"control_request","request_id":"x","request":{"subtype":"get_context_usage"}}` after a turn returns
  `categories` with tokens, `totalTokens`, `maxTokens`, `autoCompactThreshold`, `autocompactSource`, `memoryFiles`,
  `skills`, `mcpTools` and a `messageBreakdown` (`context-usage.mjs`, `03b-context-usage.sh`, measured):

| category (tokens) | `config: user` | `config: none` | none, clean (14.4K line) |
|---|---|---|---|
| system prompt | 3,393 | 3,393 | 2,671 |
| system tools | 757 | 8,400 | 4,663 |
| MCP server instructions | 717 | 717 | 717 |
| memory files | 1,770 (CLAUDE.md 1,619, RTK.md 151) | 0 | 0 |
| skills | 9,953 | 2,310 | 2,310 |
| messages (attachments) | 15,948 | 3,982 | 3,946 |
| **total** | **32,538** | **18,802** | **14,307** |
| deferred (not in total): system tools / MCP tools | 13,846 / 2,234 | 13,846 / 2,234 | 7,058 / 2,234 |
| window / auto-compact threshold | 300,000 / 267,000 (source `settings`) | 1,000,000 / 967,000 (`model-default`) | same |

- The `messageBreakdown.attachmentsByType` for `config: user`: `prompt_snapshot` 17,199, `skill_listing` 9,509,
  `hook_success` 1,995, `instructions` 1,311, `agent_listing_delta` 941, `deferred_tools_delta` 826,
  `mcp_instructions_delta` 516; for `none`: `prompt_snapshot` 17,199, `skill_listing` 1,898, `agent_listing_delta` 941,
  `deferred_tools_delta` 136. So the largest single user-config cost is the skill listing from plugins (+7.6K), then
  hook output (2.0K) and CLAUDE.md (1.3K).
- **The user's own `settings.json` sets a compaction window** (measured: `source: "settings"`, window 300,000). Under
  `config: user` that applies unless the phase's environment sets the variable (Q4).
- The totals from `get_context_usage` (32,538; 18,802; 14,307) are within 0.2% of the measured first-response contexts
  of the same lines (30.9-32.6K; 18,847; 14,352).

### Q4. `CLAUDE_CODE_AUTO_COMPACT_WINDOW`, `BASH_MAX_OUTPUT_LENGTH`, `MAX_MCP_OUTPUT_TOKENS`

All runs: the `config: none` clean line, variables passed with `driver.mjs --env` and, for the `CLAUDE_*` one, kept
with `S9_PASS` (a check of the wrapper with `env` showed the variable arrives). `04-env.sh` (measured unless marked).

**A. `CLAUDE_CODE_AUTO_COMPACT_WINDOW`: changes when compaction fires, and the stream shows it.** Five turns that read
eight files (context grows from 14K to about 100K). Timeline: context size per response, `R` = result:

| window | timeline |
|---|---|
| unset | 14392 36653 R 36862 57109 R 57242 77489 R 77621 97868 R 97981 R |
| 30000 | 14392 36653 R 36870 57117 R 57236 **[compacting] [compact_boundary auto pre=67369 post=15108]** 38594 R 39220 39939 60264 R 60483 R |
| 100000 | 14392 35017 R 37227 57550 R 57674 **[compacting] [compact_boundary auto pre=67810 post=15029]** 38462 R 39064 39475 59840 R 60087 R |
| 120000 | 14392 35017 R 37203 57527 R 57651 77898 R 78022 **[compacting] [compact_boundary auto pre=88158 post=15111]** 38518 R 39112 R |
| 150000 | no compaction; context reaches 98,108 |

- **Minimum 100,000.** 30,000 and 100,000 behave the same (pre 67,369 and 67,810). `get_context_usage` states it
  (`04b-window-clamp.sh`): a window of 30000 gives `maxTokens` 100,000; 200k (suffix accepted) gives 200,000; 500000
  gives 500,000; 2,000,000 is capped to the model's 1,000,000; `abc` is ignored (`source: model-default`).
  The binary says the same ("treats it as the minimum").
- **Threshold = window - 33,000.** `autoCompactThreshold` is 67,000 for 100K, 167,000 for 200K and 267,000 for 300K;
  the first compaction fired at a pre-compaction estimate of 67.4-67.8K (windows 30K and 100K, both 67,000) and 88.2K (window 120K, threshold 87,000).
- **The variable beats the user's settings.** Under `config: user` the window is 300,000 from settings; with the
  variable at 150000 it is 150,000, `source: "env"` (`04b`).
- The stream signals: `system/status compacting`, `system/status` with `compact_result: "success"`, then
  `compact_boundary` (Q1). **No warning appears in the stream or stderr** when a value is clamped or ignored (measured;
  the binary has a headless warning path, but nothing was emitted).
- The compaction turn costs more: the 100K-window run cost $0.023 against $0.018 with no compaction (the summarizer
  call; the main run reads about 58K from cache either way).

**B. `BASH_MAX_OUTPUT_LENGTH`: changes what enters the context.** The Bash tool inlines output up to a limit and
otherwise stores it in a file and returns a short preview:

| case | output | tool result seen by the model | context after the call |
|---|---|---|---|
| default, `seq 1 9000` | 43,893 chars | `<persisted-output> Output too large (42.9KB). Full output saved to <path>` plus a preview, 2,294 chars; `tool_use_result.stdout` 30,000 chars | 17,753 |
| env 5000, same | 43,893 | same wrapper; `tool_use_result.stdout` **5,000** chars | 17,756 |
| default, `seq 1 3000` | 13,893 | the **whole output inline**, 13,892 chars | **24,554** |
| env 5000, same | 13,893 | persisted wrapper (13.6KB), 2,294 chars | **17,754** |
| `bashOutputMaxChars: 5000` in `--settings` | 13,893 | identical to the env case | 17,754 |

- The default inline limit is 30,000 chars (binary: "default 30000; values clamp to 4000-128000"); the variable moves it,
  and the `bashOutputMaxChars` setting has the same effect.
- The stream shows it in the tool result: `is_error: false`, content starting `<persisted-output>`, and
  `tool_use_result` keys `persistedOutputPath` and `persistedOutputSize`. Those keys exist only when output was cut
  (the inline case has `persistedOutputSize: null`). **A detector for "oversized tool output" can read them.**

**C. `MAX_MCP_OUTPUT_TOKENS`: changes what enters the context.** A spike MCP server (`mcp-big.mjs`) returns N chars:

| case | tool result | context after |
|---|---|---|
| default, 150,000 chars | `Error: result (150,000 characters across 1 line) exceeds maximum allowed tokens. Output has been saved to <path>...`, 1,458 chars | 15,489 |
| default, 20,000 chars | the text inline, 20,027 chars | **27,864** |
| `MAX_MCP_OUTPUT_TOKENS=1000`, 20,000 chars | the same error text (20,000 chars exceed it), 1,456 chars | **15,466** |

- The default cap is 25,000 tokens (binary). **The over-limit tool result has `is_error: null`**, not true, though
  its text starts `Error:` (measured). A detector must match that text, or the `tool-results/*.txt` path in it.
- The model is told to read the saved file in spans, so a cap moves the cost to later calls; whether that is a
  saving was not measured.

### Q5. A `UserPromptSubmit` hook blocks a resumed prompt

`05-hook-block.sh`, hook from `work/hook-settings.json` (spike-local `--settings`; `--setting-sources ""`). Process 1
starts a session with a prompt the hook passes; process 2 `--resume`s it with a prompt that contains `BLOCKME`. Three
block styles, a control, and a block on turn 2 of a live process. Measured.

- **Stream, all three styles:** `system/init`, then **`system/informational`**, then `result/success`. **No
  `assistant` event.** No `hook_started` or `hook_response` events for this hook (they appear for SessionStart hooks
  only). The process exits 0.
- **`system/informational`** carries: `content` (the text below), `level: "warning"`, `prevent_continuation: true`
  and `prompt_submit_outcome: {kind, prompt_kept, reason}`:
  - JSON `{"decision":"block","reason":"..."}`: `kind: "blocked_by_hook"`, `prompt_kept: false`, `reason` is the hook's
    reason;
  - exit 2 with stderr: the same `kind`, `reason` is the stderr text; `content` prefixes the hook's script path;
  - `{"continue":false,"stopReason":"..."}`: `kind: "stopped_by_hook"`, `prompt_kept: true`, `content`
    `Operation stopped by hook: <stopReason>`.
- **`result`:** `subtype: "success"`, `is_error: false`, `num_turns: 0`, `stop_reason: null`, `terminal_reason: null`,
  usage zero, no `permission_denials`. `result.result` is the same text as `content`, for example
  `UserPromptSubmit operation blocked by hook:\nS9-HOOK-REASON: ...\n\nOriginal prompt: Reply with exactly: TWO BLOCKME`.
  A normal result has `stop_reason: "end_turn"`, `terminal_reason: "completed"`, `num_turns: 1`.
- **So a blocked prompt is a `result/success` that a naive adapter would take as the phase report.** The signals to
  use: the preceding `system/informational` with `prompt_submit_outcome` (`kind` and `reason` are structured, no text
  parsing), or `num_turns == 0` with `terminal_reason: null`. A `/compact` result also has `num_turns: 0`; it has
  `local_command: "compact"`.
- A block costs nothing (usage zero; the cost shown is the session's earlier total). The same on a live process: the
  blocked second turn gives the same two events, and a third turn proceeds normally (measured, `live`).
- **Control:** a resume with a prompt the hook passes runs normally (`assistant`, `result` with `stop_reason: "end_turn"`).
  The hook ran twice per case (once per process), so it ran on the resumed prompt.

### Q6. Idle resume against a fresh session with a handoff note

Two pairs, run by `06a`/`06b` (a session with four files read, 73K tokens of context) and `06c`/`06d` (eight files
read, 114K tokens). Each idle session had finished its last turn 3,717 s (62 min) and 3,703 s before the pair ran,
so its 1-hour cache had expired. Both use `--setting-sources user` (`config: user`), haiku, and the same follow-up
question. A: `--resume` of the idle session. B: a **fresh session** whose first prompt is a four-sentence handoff note
(written by hand, listing the files read and the answers given, about 120 tokens) plus the question. C: the idle
session resumed once more straight after A (a warm resume, as a reference). Measured: `06b-idle-resume.sh`,
`06d-big-pair.sh`, from `model_usage_since_last_result`.

| pair | run | first-response context | cache read | cache write (all 1h) | cost (notional USD) |
|---|---|---|---|---|---|
| 73K session | A cold resume | 74,885 | 11,372 | **63,511** | **$0.0129** |
| | B fresh + handoff | 31,063 | 14,060 | 17,001 | **$0.0036** |
| | C warm resume | 75,165 | 74,883 | 280 | $0.0008 |
| 114K session | A cold resume | 115,256 | 11,372 | **103,882** | **$0.1047** |
| | B fresh + handoff | 32,780 | 14,060 | 18,718 | **$0.0040** |
| | C warm resume | 117,646 | 115,254 | 2,390 | $0.0084 |

- **A cold resume costs 3.6 times the fresh start at 73K and 26 times at 114K.** Only the 11,372 tokens of the
  shared static prefix (the same figure in both pairs, and the same as the first response of an unrelated first
  call) were read; the rest of the context was written again at the 1-hour rate. Cost grows with the session,
  while a fresh start costs the same 31-33K start weight each time.
- Fitting the two warm and cold costs gives about $1.00 per million tokens for a 1-hour write and $0.05 for a cache
  read on this model (inferred from the fit; `costBasis: "list"`), so a cold token costs about 20 times a warm one.
- **Both fresh sessions answered correctly** from the note alone (the question was one the note could answer; this
  checks the handoff shape, not that a note can replace a context for real work). A cold resume also answered
  correctly.
- A fresh session's own start is mostly written, not read (17-19K written, 14K read): its first response, too, pays
  for the user-config part of the prompt at the write rate, because that part was not in the cache. It was still far
  cheaper than rewriting a 73-114K session.
- Caveats: one pair of each size, no repeat; the notes were written by hand, where the plan builds them from the
  transcript with no model call; the answer tasks were trivial; the 11,372-token shared prefix was warm because other
  runs (mine, and S2 on the same account) keep it so, which is also true in Collide's use. The idle times were 3,717 s and 3,703 s. A resume after 5-60 minutes (cache warm) was not measured apart
  from C, which is seconds after A.
- **Reading for `on_cold`** (inferred): a cold resume costs $0.009 more than a fresh start at 73K and $0.10 more at 114K; the break-even is where the written context exceeds the fresh start's
  roughly 18K write, which is any session past about 30K tokens of context. Whether the fresh session does the same
  work is not what this measures; Collide's gates and review are the measure of that (0.14).

## Addendum: hook and permission outcomes in the stream (after the spike, 2026-11-13)

Question: how do a `PreToolUse` hook deny, a hook error and a permission denial appear in stream-json? Script
`07-hook-tool-deny.sh` (`07-summarize.mjs` prints the shapes; output `out/07-hook-tool-deny.txt`, logs and the per-case
settings files in `out/07-hook-tool-deny/`). All measured unless labelled.

- **Method:** 25 haiku calls (`claude-haiku-5-5`) through `claude-clean.sh`, each with `--setting-sources ""`,
  `--strict-mcp-config`, a spike-local `--settings` file (one Bash-matched command hook, `hooks/tool-hook.sh`, behaviour
  from `S9_HOOK_MODE`) and `--max-budget-usd 0.05`. Prompt: run `touch s9-x.txt` with Bash in `work/p9` (a write that
  needs approval; the file shows whether the tool ran). All cases but `c6b` carry `--permission-prompt-tool stdio` and
  the driver answers `can_use_tool` (allow, or deny with `S9-HOSTDENY ...`). **The installed `claude` is 2.1.325**
  (`claude --version` and `system/init`; the body of this record says 2.1.323, the CLI has updated since).
- **Spend:** the recorded 25 calls cost $0.0135 (sum of `total_cost_usd`; `spend.sh` row `07-hook-tool-deny`). Two
  earlier runs of the script, whose logs the final run overwrote, cost about $0.0155 more (a one-case check, $0.0018,
  and a first run of the 14 `c*` cases, $0.0137). Total about $0.029 of the $0.25 cap.
- **Common to all 25 calls:** `result/success`, `is_error: false`, `stop_reason: "end_turn"`, `terminal_reason:
  "completed"`, `num_turns: 2`, exit 0. **None of the result fields tells the cases apart**, except that
  `permission_denials` is non-empty for some.

### Shapes, side by side

"Window" = events between the assistant's `tool_use` and the next assistant message. Without `--include-hook-events`
(the default) a `PreToolUse` or `PostToolUse` hook emits **no** `hook_started` or `hook_response` (the control `c0`
ran the hook once, per its marks file, and the stream has only `system/init`).

| case (log) | window besides the `user` tool_result | `tool_result` | `tool_result_meta[0]` | `permission_denials` | tool ran |
|---|---|---|---|---|---|
| allowed control (`c0`) | `control_request` `can_use_tool` (keys `blocked_path`, `permission_suggestions`, ...) | `is_error: false`, `(Bash completed with no output)` | `permission_decision {accept, user_temporary}` | `[]` | yes |
| 1. hook deny, JSON (`c1`) | none: no control request, no system event | `is_error: true`, `PreToolUse:Bash hook error: S9-DENY-JSON ...` | `non_execution_kind: "permission-rule"`, `permission_decision {reject, source: "hook", reason_type: "hook"}` | 1 entry | no |
| 2. hook deny, exit 2 (`c2`) | none | `is_error: true`, `PreToolUse:Bash hook error: [<script path>]: S9-DENY-EXIT2 ...\n` | identical to case 1 | 1 entry | no |
| 3. hook `ask`, host allows (`c3a`) | `control_request` whose request adds `decision_reason` (the hook's text), `decision_reason_type: "hook"`, `decision_reason_hook_event: "PreToolUse"`, `decision_reason_hook_source: "settings"` | normal success | `accept, user_temporary` | `[]` | yes |
| 3. hook `ask`, host denies (`c3b`) | the same `control_request` | the host's message verbatim, `is_error: true` | as case 6 | 1 entry | no |
| 4a. hook exit 1 (`c4a`) | none | normal success | `accept, user_temporary` | `[]` | **yes** (fails open) |
| 4b. hook prints invalid JSON, exit 0 (`c4b`) | none | normal success | same | `[]` | **yes** |
| 4c. hook sleeps past `timeout: 1` (`c4c`) | **`system/informational`**: `level: "warning"`, `tag: "hook_no_verdict"`, `hook_no_verdict {hook_event, tool_use_id, timed_out: true}`, `content` `A hook didn't answer in time, so it didn't check this action.`, before the `can_use_tool` request | normal success | same | `[]` | **yes** (fails open) |
| 5. `PostToolUse` `{"decision":"block"}` (`c5a`) or exit 2 (`c5b`) | none | **unchanged**: `is_error: false`, `(Bash completed with no output)` | `accept, user_temporary` | `[]` | yes (it had run) |
| 6. host deny (`c6a`) | `control_request` | `is_error: true`, **the host's message verbatim**; no `<tool_use_error>` wrapper | `non_execution_kind: "permission-rule"`, `permission_decision {reject, source: "user_reject"}` | 1 entry | no |
| 6'. no prompt tool, auto-deny (`c6b`) | **`system/permission_denied`** (keys `tool_name`, `tool_use_id`, `message`) | `is_error: true`, `touch in '...' needs approval. ...` | `non_execution_kind: "user-rejected"`, `{reject, source: "config"}` | 1 entry | no |
| 7a. `--disallowedTools Bash` (`c7a`) | none; **no hook call** | `is_error: true`, `<tool_use_error>Error: No such tool available: Bash. Bash is disabled ...</tool_use_error>` | **absent** (`null`); `tool_use_result` is a string | **`[]`** | no |
| 7b. `--disallowedTools 'Bash(touch:*)'` (`c7b`) | `system/permission_denied` with `decision_reason_type: "subcommandResults"`; no `can_use_tool` | `is_error: true`, `Permission to use Bash with command touch s9-x.txt has been denied.` | `permission-rule`, `{reject, source: "config", reason_type: "subcommandResults"}` | 1 entry | no |

- **A hook deny is in `result.permission_denials`** (cases 1 and 2), as the same `{tool_name, tool_use_id,
  tool_input}` entry a host deny, an auto-deny and a rule deny produce. A disallowed tool that is not in the tool list
  (7a) is not. A hook deny does not reach the host: **no `can_use_tool` request** (`c1`, `c2`: 0 requests).
- **7a:** the model did attempt `Bash` although `Bash` is absent from `system/init.tools` (its `tool_use` input has no
  `description`, unlike the other cases). So a `tool_use` of a name that `init.tools` lacks is a shape signal, not
  only an error string.
- **Exit codes:** every process exited 0, and every turn ended `success`. A deny or a hook failure never ends the
  process.

### Does the reason reach the model?

- **Hook deny (both styles), host deny, rule deny, auto-deny:** yes. The model's text repeated the reason in each case
  (`S9-DENY-JSON`, `S9-DENY-EXIT2`, `S9-HOSTDENY`, "denied by the permission settings").
- **PostToolUse block (both styles): yes, but not through the stream.** The `tool_result` is unchanged and no event
  carries the reason (`grep S9-POST` over the logs finds it only in the model's own text and the `result`). The model
  said the tool ran, then that a PostToolUse hook "returned a blocking error ("S9-POST ...")". The reason travels in a
  channel the stream does not show.
  > **Spot-check correction (lead, 2026-11-13):** only the exit-2 block's reason reached the model. In `c5b` the
  > model quoted `S9-POST-EXIT2`. In `c5a` (JSON `{"decision":"block"}`) it said "a PostToolUse hook returned a
  > blocking error that I can't explain", and `S9-` appears nowhere in `c5a-post-block.jsonl`. So a JSON block tells
  > the model that a block happened, but not why (one run each).
- **Hook error exit 1, invalid JSON, timeout:** nothing reaches the stream, and the model did not mention it. Asked to
  quote every hook or system message it received (`c4a2`, `c4b2`), it answered `NONE` for exit 1 and gave no message for
  invalid JSON. (Weak evidence: it shows the model did not volunteer one; I did not read the on-disk transcript.)

### With `--include-hook-events` (`h*` logs; the flag is in `claude --help`)

Every hook now emits `system/hook_started` and `system/hook_response`, in the window after the `tool_use`
(`PreToolUse`) or after the `can_use_tool` request and before the `user` event (`PostToolUse`). Keys of
`hook_started`: `hook_id`, `hook_name` (`PreToolUse:Bash`), `hook_event`. Keys of `hook_response`: the same, plus
`output`, `stdout`, `stderr`, `exit_code`, `outcome`.

| hook | `outcome` | `exit_code` | note |
|---|---|---|---|
| passes silently | `success` | 0 | |
| JSON deny | `success` | 0 | `stdout` holds the JSON; the deny is visible only in `tool_result_meta` and `permission_denials` |
| exit-2 deny | `error` | 2 | `stderr` holds the reason |
| `ask` | `success` | 0 | |
| exit 1 | `error` | 1 | the tool still ran |
| invalid JSON, exit 0 | `success` | 0 | **indistinguishable from a good hook** without parsing `stdout` |
| timeout | `cancelled` | 1 | followed by the `hook_no_verdict` informational event |
| PostToolUse JSON block | `success` | 0 | the block is only in `stdout` |
| PostToolUse exit 2 | `error` | 2 | |

The flag changes nothing else: the same `tool_result`, `permission_denials` and `result` as the runs without it.

### What tells the outcomes apart without reading tool content (measured)

- **Cause of a refused tool call:** `user` event `tool_result_meta[].permission_decision.source` (join on `id` =
  `tool_use_id`): `hook` (with `reason_type: "hook"`) is a hook deny; `user_reject` is the host's deny;
  `config` without `reason_type` plus `non_execution_kind: "user-rejected"` is the auto-deny when no prompt tool is set;
  `config` with `reason_type` (`subcommandResults`) is a deny rule. No `tool_result_meta` at all, with `is_error: true`
  and a `tool_use_result` that is a string, is a disallowed or unknown tool.
- Side evidence: a `can_use_tool` request for that `tool_use_id` (host deny, hook `ask`); `system/permission_denied`
  (auto-deny and rule deny only; hook and host denies emit none).
- **Hook failure:** invisible by default. With `--include-hook-events`, `outcome: "error"` and `exit_code` 1 versus 2
  (2 is the deliberate block); timeout is `outcome: "cancelled"` or, without the flag, the `hook_no_verdict`
  informational event. Invalid JSON is not detectable from shapes.
- **The text is a trap for content detectors:** a deliberate hook deny reads `PreToolUse:Bash hook error: ...`.
- **`result` is useless for this:** identical fields in every case; `permission_denials` mixes four causes.

### Unmeasured

Tools other than Bash (Read, Edit, MCP), and `PreToolUse` on a tool that needs no approval; a hook `allow` decision,
`updatedInput` and `continue: false` from a tool hook; several hooks on one call; `PostToolUse` on a failing tool;
whether `permissionDecision: "ask"` without a prompt tool is auto-denied; hook outcomes inside subagents; the on-disk
transcript's record of a hook error (not read); Sonnet and Opus (haiku only); whether `hook_no_verdict` appears with
the default timeout (only a 1 s timeout was run).

### Proposed plan changes (addendum)

1. **0.3 stream protocol, launch flags:** add `--include-hook-events` to the Claude launch line, or state why not. It is
   the only source of hook failures (exit 1, timeout `cancelled`, exit 2) that are otherwise invisible; cost is two
   small events per hook call, with hook `stdout`/`stderr` inside them (treat as untrusted text).
2. **0.3 stream protocol, event table:** add `system/hook_started`, `system/hook_response` (with `outcome`, `exit_code`)
   and `system/informational` with `tag: "hook_no_verdict"`; note that without the flag only the last exists, and a
   `PreToolUse`/`PostToolUse` hook is silent.
3. **0.3, tool outcome classification:** classify a refused tool call from `user.tool_result_meta[].permission_decision`
   (`source`, `reason_type`) and `non_execution_kind`, joined on `tool_use_id`; not from the `tool_result` text.
   No meta plus `is_error: true` is a disallowed tool.
4. **0.3 / 0.13, `result.permission_denials`:** document that it lists hook denies, host denies, auto-denies and rule
   denies alike (not disallowed-tool calls); never present it as "permission prompts the user refused". Take the cause
   from item 3.
5. **0.13 detectors:** "hook denied a tool" (meta `source: "hook"`); "hook failed open" (`hook_response`
   `outcome: "error"` with `exit_code: 1`, or `hook_no_verdict`, tool still ran); "PostToolUse blocked" is
   visible only as `hook_response` `exit_code: 2` (a JSON `decision: "block"` shows only in `stdout`, outcome `success`);
   "tool not in `init.tools`" (a `tool_use` name the session did not list).
6. **0.13 tool metrics:** count a tool call as executed only if `tool_result_meta` lacks `non_execution_kind`; a
   hook-denied call has `is_error: true` and costs a model turn (the model explains it), so "tool error rate" should
   split refused calls (`non_execution_kind` set) from tool failures.
7. **0.3 permission requests:** a hook `ask` arrives as an ordinary `can_use_tool` request; its `decision_reason_*`
   keys say the hook asked, and `decision_reason` is the hook's text. The adapter shows that text in the Queue item and
   answers as for any request.

## Proposed plan changes

1. **0.13 "Agent stream" row ("tokens exact, one usage per `message.id`") and "Token and context metrics".** Say how
   usage is read:
   - dedupe assistant rows by `message.id` (66 of 105 ids repeat; summing every row doubles cache tokens);
   - the input and cache fields of an assistant row are exact; its `output_tokens` is a partial count (lower than the
     final on 94 of 105 ids). Take per-turn totals from `result.usage` or `model_usage_since_last_result`, per-response
     context from the deduped input and cache fields, and per-response output from the transcript if it is needed;
   - `result.usage` leaves out a compaction's own model call; `model_usage_since_last_result` and `total_cost_usd`
     include it. The metric "compaction cost" reads `model_usage_since_last_result`.
2. **0.13 compaction metrics.** "Trigger, tokens before and after, duration" come from `system/compact_boundary`
   `compact_metadata` (`trigger`, `pre_tokens`, `post_tokens`, `duration_ms`, `cumulative_dropped_tokens`) for manual
   and auto compactions, preceded by `system/status compacting`. Take "tokens after" from the next response's
   context, not `post_tokens` (10,422 against 33,042 measured). "Landed mid-task" is whether the preceding assistant
   event holds an open `tool_use`.
3. **0.3 context options.**
   - `compact_at` sets `CLAUDE_CODE_AUTO_COMPACT_WINDOW`: the minimum is 100,000 (a smaller value behaves as 100,000), the
     maximum is the model window, compaction fires at **window minus 33,000**, `200k` suffixes work, and an unusable
     value is ignored with no warning in the stream. Collide validates the value itself and refuses one under 100,000.
     The variable overrides the user's own `settings.json` window (measured: the user's is 300,000), so `config: user`
     phases get the role's value when `compact_at` is set. The fingerprint's "effective window" can be read from
     `get_context_usage` (`maxTokens`, `autoCompactThreshold`, `autocompactSource`).
   - `tool_output_max` sets two limits in different units: `BASH_MAX_OUTPUT_LENGTH` in characters (default 30,000; the
     binary clamps to 4,000-128,000), and `MAX_MCP_OUTPUT_TOKENS` in tokens (default 25,000). Both change what enters
     the context (Q4). The `bashOutputMaxChars` setting has the same effect as the Bash variable, so a snapshot can use
     either. The plan should name two fields, or define one with two conversions.
4. **0.3 "The user's config is the largest fixed cost".** Replace "29-34K" and "6.9-7.7K" with the S9 numbers:
   `config: user` 30.9-32.6K; `config: none` (`--setting-sources ""`) 18.8K; none with auto memory off and the 0.3
   disallowed tools 14.4K. `--setting-sources ""` alone keeps all built-in tools and 19 built-in skills, so a role that
   restricts its tools starts lighter (no tools at all: 3.2K), and the role's `tools` list is a start-weight lever.
   `--disable-slash-commands` raises the start weight and should not be used. The run preview can show the breakdown
   by calling `get_context_usage` (skills from plugins 9.5K, hook output 2.0K, CLAUDE.md 1.3K under this user's
   config); `system/init` has names and counts only.
5. **0.3 and 0.13, hook-blocked prompts.** The adapter treats a prompt blocked by a hook as: a `system/informational`
   event with `prompt_submit_outcome` (`kind` `blocked_by_hook` or `stopped_by_hook`, `reason`, `prompt_kept`) and
   `prevent_continuation: true`, followed by `result/success` with `is_error: false`, `num_turns: 0`,
   `stop_reason: null`, `terminal_reason: null` and zero usage. The Queue item carries `reason`. The adapter must not
   read that `result.result` as the phase report; a `/compact` result also has `num_turns: 0`, with
   `local_command: "compact"`. UserPromptSubmit hooks emit no `hook_started` or `hook_response` events, so the
   run preview's list of user hooks (from settings) is the only warning before the block.
6. **0.3 "Context and cache".** Keep "the 1-hour cache only" for the main loop and add that a compaction call writes
   181 tokens at the 5-minute tier. State that the API-key tier is unmeasured. The `on_cold` text can say what a cold
   resume costs: the whole context is rewritten at about 20 times the read price (73K: $0.013 against $0.004 fresh;
   114K: $0.105 against $0.004). A cold resume read only the 11K shared prefix, so the plan's "writes its whole
   context again" is right.
7. **0.14 token-strategy experiments.** The three context options all change the stream, so an experiment arm can
   vary them and read the result from `compact_boundary`, the persisted-output keys and the MCP error text (Q4).
   Add an "oversized tool output" detector input: `tool_use_result.persistedOutputPath` for Bash, and the text
   `exceeds maximum allowed tokens` (with `is_error: null`) for MCP.

## Not measured

- **The cache tier under an API-key login.** No key is in the environment and I did not look for one.
- A resume after 5-60 minutes idle, and any idle time between 1 and 62 minutes; a pair with a repeat or with real work
  rather than a one-line question.
- Whether a fresh session with a handoff note does the same work as the resumed one (the question was answerable
  from the note alone), and handoff notes built from a transcript by Collide rather than by hand.
- Why `--disable-slash-commands` raises the start weight, why the `config: user` start weight alternates between 30.9K
  and 32.6K, and why `post_tokens` is so far below the next response's context.
- Output limits of a PowerShell tool, and `MAX_MCP_OUTPUT_TOKENS` against a real server's structured content or images;
  whether capping tool output saves tokens overall (the model is told to read the saved file in spans).
- Sonnet and Opus: all calls used haiku, so token counts for the system prompt and tools are haiku's, and the
  prices fitted in Q6 are haiku's.
- Compaction on a resumed session. That the variable overrides the user's window is measured through
  `get_context_usage` only, not by watching a compaction fire under `config: user`.
- `get_context_usage` is a control-request subtype read from the binary's strings; its stability across versions and
  whether `-p` hosts other than this driver can send it are unmeasured.

## Spend

Notional `total_cost_usd` (claude.ai subscription), per process, from `spend.sh`:

| script | spend |
|---|---|
| `01-usage-compact` | $0.0158 |
| `03-start-weight` | $0.0209 |
| `04-env` | $0.1362 |
| `05-hook-block` | $0.0043 |
| `06a-idle-start`, `06b-idle-resume` | $0.0131 + $0.0173 |
| `06c-big-idle-start`, `06d-big-pair` | $0.0584 + $0.1171 |
| `spend.sh` total | $0.3831 |
| `03b` and `04b` (about 16 control-request runs, no driver log) | about $0.05 (estimated: each run reads about 32K from cache and writes little) |

The runs recorded in `out/` cost about **$0.43** ($0.38 plus the estimate). Development reruns of `03` and `04` (about
four extra runs of each, overwritten in `out/`) and the first draft runs add roughly $0.4, so the total is about
**$0.85**, against the $4 cap and the plan's estimate of $3 (estimated, not summed: overwritten logs carry no cost).
The cost is small because every call used haiku and most context came from cache.
