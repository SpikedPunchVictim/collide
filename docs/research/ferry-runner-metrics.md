# Ferry's runner, Ferry and its platform: how they capture Claude Code metrics

Research input for plan revision 10, 2026-11-13. All sources were read only. Nothing was built, installed, run or
tested. No docker or `claude` call was made.

This note does not repeat `docs/research/ferry.md`. That note covers gaps C1-C3, `agentWork` and report kinds. It is
referenced where relevant.

## 1. Sources and labels

**Sources**
- Ferry's runner: the container entrypoint that runs one `claude -p` per call.
- Ferry: the orchestrator that drives Ferry's runner and writes the build report.
- The platform's own `claude -p` execution service: a long-lived API service that spawns `claude -p` per call.
- Collide's own measurements: S1 (headless), S2, S9 (tokens and context), claude 2.1.323.
- Collide's plan, `IMPLEMENTATION_PLAN.md`, revision 10, including part F (plan 0.13).

**Claude versions each source targeted**
- Ferry's runner and the platform's parser both say claude **2.1.129** in comments [read Ferry's runner and the
  platform's execution service, the top-of-file comment in each].
- Ferry's runner image installs claude unpinned, so 2.1.129 is a comment, not a pin [read Ferry's runner image build,
  the install line].
- Ferry's runner results record no claude version [read Ferry's runner].
- Collide measured 2.1.323. Where a difference below could be drift between 2.1.129 and 2.1.323, it is marked
  **version drift?**.

**Labels**
- **[read source]** means the claim was seen in the named source (or plan section).
- **[inferred]** means it is reasoning the source does not state.
- Each recommendation is tagged **core** (generic, belongs in Collide) or **Ferry example pack** (only for the Ferry
  pack). Orchard and Ferry are example solutions. Collide must not special-case them.

**Location prefixes**
- Claims about Ferry's runner, Ferry and its platform cite the source by role, not by file path.
- `plan` is `collide/IMPLEMENTATION_PLAN.md`, cited by section and bold lead-in, or by stage name.
- `S1` and `S9` are `collide/.collide/planning/spikes/sp-0001-de-claude-headless/record.md` and
  `sp-0007-zf-tokens-context/record.md`.
- `spk/` is `collide/.collide/planning/spikes/sp-0007-zf-tokens-context/out/`.

## 2. What each source captures, and how

### 2.1 Process and stream handling

| | Ferry's runner | Ferry (host side) | platform service |
|---|---|---|---|
| Process model | One container per call, `claude -p` inside via `execFileSync` [read Ferry's runner] | `docker run` per call [read Ferry's source] | One `spawn` per call, `shell:false`, no `detached` [read the platform's execution service] |
| Output mode | `json` for structured; `stream-json --verbose` only when tool telemetry is on [read Ferry's runner] | n/a | `stream-json --verbose --include-partial-messages --input-format stream-json --no-session-persistence --bare --system-prompt-file` [read the platform's execution service] |
| Buffering | stdout fully buffered, `maxBuffer` 256MB, no timeout [read Ferry's runner] | `maxBuffer` 100MB [read Ferry's source]; no deadline unless `--task-timeout` [read Ferry's source] | Line-buffered across chunks with a final flush [read the platform's execution service]; events collected, published only after exit [read the platform's execution service] |
| Stdin | not used | n/a | JSON array `[{role:'user',content:prompt}]`, then `end()` [read the platform's execution service] |
| Stop | none; on abort the host runs `docker kill` [read Ferry's source] | same | `SIGTERM` to the child, `SIGKILL` after 200 ms grace [read the platform's execution service] |
| Failure | Non-zero exit throws an error with the status and the first 4000 characters of stderr or stdout [read Ferry's runner] | Non-zero exit becomes a throw [read Ferry's source] | Timeout rejects with a typed timeout error; abort resolves; crash has its own path [read the platform's execution service] |
| Concurrency | wave concurrency lives in Ferry | per-task attribution is by budget delta [read Ferry's source] | Per-org semaphore, default 3, throws an over-capacity error, in memory [read the platform's execution service] |

Launch flags:
- Ferry's runner passes `-p`, `--mcp-config`, `--allowedTools`, `--add-dir /workspace`, `--output-format`, `--model`
  [read Ferry's runner].
- It passes no `--max-budget-usd`, `--max-turns`, `--tools`, `--setting-sources` or `--strict-mcp-config`
  [read Ferry's runner].
- The platform passes no `--max-budget-usd` either. It passes `--allowed-tools` only for one role
  [read the platform's execution service].
- The base URL and model reach claude as environment variables. Ferry forwards every config key as `docker run -e
  KEY=value` [read Ferry's source]. Ferry's runner was validated on an Ollama model (`kimi-k2.5:cloud`) through
  `host.docker.internal` [read Ferry's runner README, "Status"].
- Ferry's config files hold live-looking tokens [read Ferry's config files]. They are not copied here.

### 2.2 Fields captured

| Metric | Ferry's runner / Ferry | platform service |
|---|---|---|
| Input tokens | `usage.input_tokens`, `?? 0` [read Ferry's runner] | `result.usage.input_tokens`, absent gives 0 [read the platform's execution service] |
| Output tokens | `usage.output_tokens`, `?? 0` | `result.usage.output_tokens` |
| Cache | One number: cache read plus cache creation [read Ferry's runner] | Read and creation kept separate [read the platform's execution service]; stored in separate read and creation columns [read the platform's database schema] |
| Cache tier split (5m/1h) | not read | not read |
| Cost (`total_cost_usd`) | not read. Ferry omits cost on purpose [read Ferry's source] | declared in the zod schema, never used [read the platform's execution service] |
| `modelUsage` | not read | not read |
| `duration_ms`, `duration_api_ms`, `num_turns` | read in the timing parser; each field a number or `null`; `undefined` if all missing [read Ferry's runner]. Ferry maps the API duration to its API time and the total duration to its runner time, dropping nulls [read Ferry's source] | not read. Duration is wall-clock around the supervisor, started after guard preflight and prompt load [read the platform's execution service] |
| Tool calls | Count `tool_use` blocks by name from `assistant` events, opt-in, code mode only [read Ferry's runner] | `user` events map `tool_use_id` to tool name [read the platform's execution service]; no per-name counts |
| Result status | Never checks `is_error` or `subtype` [read Ferry's runner] | Maps `result/success` to completed. Subtype union is `success`, `error_max_turns`, `error_during_execution` [read the platform's execution service] |
| `permission_denials` | not read | not read |
| Changed files | `git` diff against a baseline commit, filtered by exclude lists of directories and files [read Ferry's runner] | n/a |
| Cache hit ratio | `cached / (input + cached)`, creation counted as a hit [read Ferry's source] | `cacheRead / (input + cacheRead)`, creation excluded from the denominator [read the platform's catalog service] |
| Token total | `input + output` [read Ferry's source] | per row, four columns |

Ferry report shape [read Ferry's source]:
- Per phase: tokens in, tokens out, cached tokens, optional tool calls, API time, runner time, turns, and start and
  end times.
- Per task: tokens and, in the schema, turns, which nothing populates.
- Gates and gate rounds carry durations.
- Per-task telemetry is appended live to `task-telemetry.jsonl` [read Ferry's source].
- Tool time and host time are derived at report time from the run timing record [read Ferry's source].
  Tool time is runner time minus API time [inferred from the field names].

Parsing details:
- The code-mode parser takes usage and timing from the terminal `result` only. It throws if there is no `result`.
  It skips malformed lines and blocks [read Ferry's runner].
- Ferry's runner fails soft in code mode. A parse failure gives zero usage and a stderr warning
  [read Ferry's runner].
- The platform uses `result.usage` only. It never reads per-row usage [read the platform's execution service].
- The platform's schema for `assistant` content allows only text and tool-use blocks. A `thinking` block fails
  validation and is skipped with a warning [read the platform's execution service]. Unknown envelope types are rejected
  by the discriminated union. This is harmless where each row holds one block (see 3.4) [inferred].
- The platform's token count counts `text_delta` events, which are not tokens [read the platform's execution service].
- An error `result` yields only a failure event, with no usage [read the platform's execution service].
- If claude exits 0 with no `result`, the platform adds a synthetic zero-usage result
  [read the platform's execution service].

### 2.3 Hooks, read budget and tool telemetry (Ferry's runner)

- The runner's Claude Code settings have three hooks: PreToolUse `Bash` runs a command-rewrite script, PreToolUse `Read`
  runs a read-budget script, PostToolUse `Edit|Write` runs an incremental code-index update [read Ferry's runner].
- The read-budget hook denies `Read` after N reads (default 12) with a deny permission decision, redirecting to
  code-index tools or a context-gap declaration [read Ferry's runner].
- The counter is a read-increment-write file, shared per workspace, not atomic [read Ferry's runner].
- It fails open on any error [read Ferry's runner].
- It counts hook calls, so denied reads count too [read Ferry's runner].
- The 12-read default and the "70k+ token whole-file Read" claim have no source in the docs read
  [read Ferry's archived feedback doc]. The token half of that task was deferred [read
  same file, "Implementation status (updated 2026-07-09)"].
- No measured effect of the read budget or of tool counts was found in any file read.

### 2.4 What Ferry measured, and what it shows

These numbers come from `result.usage` sums, so cache zeros on Ollama are ambiguous (see 3.2).
- url-shortener on kimi: 31,638,866 input tokens, no cache, against 7,075,206 for v1
  [read Ferry's archived token-usage comparison].
- Worst task: 7.33M. Refine: 5.74M, 21% [read same].
- Usage is reported only at completion, so a runaway call is paid in full [read same].
- Run 5: 86.0M input, 0 cached tokens across 68 phases [read Ferry's roadmap, failure mode 2].
- Sonnet comparison: 25k fresh and 7.1M cached against kimi's 5.8M fresh and 0 cached
  [read Ferry's archived feedback doc, implementation status].
- The roadmap asks "dead or unreported?" about the zero cache field [read Ferry's roadmap, the convention-gates stage].
  That is the zero-versus-missing confusion.

## 3. Checked against S1, S2 and S9

Verdicts: **right**, **wrong**, **version-dependent**.

### 3.1 Usage from `result.usage` only: right, with a known gap
- S9 found the stream repeats one `usage` per content block (same `message.id`) and per-row `output_tokens` is
  partial. Per-turn totals come from `result.usage`. Ferry's runner and the platform both read `result.usage` only
  [read Ferry's runner and the platform's execution service]. **Right.** Neither double counts.
- S9 also found `result.usage` excludes a compaction's own call, while `model_usage_since_last_result` and
  `total_cost_usd` include it. The real platform fixture shows the same shape for a side model: `total_cost_usd`
  0.10988 equals the sum of `modelUsage`, and that sum includes a haiku call (344 in, 11 out) that is not in
  `result.usage` (5 in, 6 out) [read the platform's test fixtures].
- Consequence: both sources undercount. The error is at most one summarizer call per compaction (S9: about 22K in /
  2.5K out against 4 / 464 reported) plus small side calls. That is small against 0.8-7M tasks. Whether Ferry sessions
  compacted is **unmeasured**.
- Ferry's belief that there is no mid-session compaction [read Ferry's architecture doc, the token-economy stage] does
  not match claude's auto-compaction in S9. **Contradicts S9.**

### 3.2 Zero versus missing: wrong for usage, right for timing
- Ferry's runner applies null-not-zero to timing only [read Ferry's runner]. Usage is zero-defaulted in the runner's
  usage reader and in Ferry's runner protocol normalizer. Ferry's timing ledger sums with a 0 default [read Ferry's
  runner and Ferry's source].
- Result: an endpoint that returns no cache fields becomes a cached-token count of 0. That produced the "dead or
  unreported?" question above.
- Covered by plan 0.13 part F "Missing is not zero" [read plan 0.13, Missing is not zero, Token and context metrics;
  acceptance test: the headless-core stage, Success Criteria; review log: Ferry's runner (owner, part F)]. **Not
  proposed again.** Section 4 item 2 only extends it to error results.
- Ferry omits a zero timing delta from a phase [read Ferry's source]. That is the same rule applied to one place.

### 3.3 Timing: right for one process per call, with one derived-field risk
- Part F is measured in S9's recordings [read plan 0.13, Missing is not zero, Levels, the Agent stream row]:
  - `duration_ms` is per turn.
  - `duration_api_ms` and `total_cost_usd` are running totals over the process.
  - A `num_turns: 0` result reports `duration_api_ms` 0.
- My scan of the S9 spike logs agrees [read `spk/**/run.jsonl`, 109 result events with `duration_api_ms`]. In
  `04-env/A-150000.jsonl` the api values are 1927, 3474, 5075, 6606, 7133 against per-turn `duration_ms` 2219, 1577,
  1633, 1574, 548. In 29 of the 109, `duration_api_ms` exceeds `duration_ms`.
- Ferry's runner is unaffected. One `-p` per container means each field is that call's total
  [read Ferry's runner]. **Right.**
- **The platform is unaffected for the same reason.** One process per call, stdin ended after one prompt,
  `--no-session-persistence` [read the platform's execution service]. There is no multi-turn process and no resume. The
  service ignores `duration_ms`, `duration_api_ms`, `num_turns` and `total_cost_usd` [read the platform's execution
  service]. It would break only if the platform moved to a persistent or resumed session [inferred].
- Risk in Ferry: tool time is runner time minus API time [inferred]. `duration_api_ms` can exceed `duration_ms`
  (parallel side calls; the real platform fixture has 2659 against 1892 [read the platform's test fixtures]). The
  derived tool time can then be negative. Show it as "not derivable" instead.
- Version drift?: both 2.1.129 and 2.1.323 data show api > total, so this is not drift.

### 3.4 Tool-call counting: right on 2.1.323 data
- Each `assistant` row in S9's logs holds one content block. In three message ids from `spk/01-usage-compact/run.jsonl`
  no block repeats across rows. So counting `tool_use` blocks by name is correct [read `spk/01-usage-compact/run.jsonl`;
  small sample].
- Gaps: it is opt-in, code mode only, and records no tool errors and no hook denials [read Ferry's runner]. Structured
  calls record no tool calls [read Ferry's source].
- How a PreToolUse hook deny appears in the stream is **unmeasured**.

### 3.5 Cache: the runner's merge is wrong for Collide
- Merging read and creation into one number loses the split [read Ferry's runner]. Plan 0.5's `usage` line carries
  `cacheRead` and `cacheWrite` separately [read plan 0.5, Reporting]. A merged number cannot fill both. **Contradicts
  the plan's split.**
- Ferry's runner counts creation as a hit. The platform excludes it from the denominator. The two formulas disagree. The
  plan defines no formula (see 4.3).
- S9 Q2: the cache is 1-hour tier only on a claude.ai login. A compaction call wrote 181 tokens at the 5-minute tier.
  Neither source reads `cache_creation.ephemeral_5m/1h_input_tokens`. The real platform fixture shows 1h = 16137, 5m = 0
  [read the platform's test fixtures]. On an API key the tier may differ [inferred].

### 3.6 Budgets and cost: token caps do not track cost
- Ferry budgets use `input + output` [read Ferry's source]. On a caching endpoint,
  fresh input is tiny (4.1k against 7.1M cached) so the cap barely moves.
- Neither source passes `--max-budget-usd`. S1 measured it: `error_max_budget_usd` is a result subtype with
  `is_error` true and exit 1, and it counts only the current process [read `S1`, "Q4. A tiny `--max-budget-usd`"]. Plan
  0.3 already uses it [read plan 0.3, v0 ships one adapter, Claude].
- The platform's subtype union omits `error_max_budget_usd` [read the platform's execution service]. Moot while it
  passes no budget.
- Cost for a non-Anthropic model through claude is a price-table guess in claude, unmeasured here. Ferry's decision
  to omit cost [read Ferry's source] is defensible for that case. It
  is not defensible for Anthropic models, where `total_cost_usd` is measured.

### 3.7 Spend lost on failure: wrong
- A failed, timed-out or killed call records no usage [read Ferry's source and Ferry's runner].
- The stdout JSON of an error result holds usage and cost. Ferry's runner only echoes it as text, so the spend is lost
  [read Ferry's runner].
- The platform persists zero usage for failed runs [read the platform's catalog service]. That is zero,
  not null.
- S1: a SIGTERM'd process emits no `result` [read `S1`, "Q5. Final message as the report; resume; cancel"]. Its cost
  shows only on a later resume, because `total_cost_usd` is cumulative per session [read `S1`,
  "Q1. Multi-turn stream-json; is `--verbose` needed?"].
- The platform maps `result/success` to completed regardless of `is_error` [inferred from the platform's execution
  service]. S1 shows `result/success` with `is_error: true` and `api_error_status: 429` on rate limits [read `S1`, "Q8.
  Usage and rate limits"]. A rate-limited call would be recorded as completed [inferred].
- Neither source reads `permission_denials`. S1 says `result/success` does not mean the work happened.

### 3.8 Start weight: unmeasured for Ferry's runner
- `--allowedTools` pre-approves tools. It does not remove them from context [inferred from S9 Q3]. S9 measured
  `config: user` at 30.9-32.6K start weight, and `--tools ""` at about 3.2K.
- Ferry's runner uses neither `--tools`, `--setting-sources` nor `--strict-mcp-config`, and loads two MCP servers
  (context7 and a code-index server) [read Ferry's runner]. Its start weight is likely large [inferred]. Not measured.

### 3.9 Fixtures and versions: verified claims are weaker than they read
- the runner's tests say the envelopes are "verified against real claude v2.1.129", but the fixtures are hand-built
  literals [read Ferry's runner].
- Of the platform's five fixtures, only one is real. It is single-turn, with no tool use and no compaction
  [read the platform's test fixtures].
- That real fixture was captured with `echo ... | claude -p --verbose --output-format stream-json
  --no-session-persistence --include-partial-messages`. That is not the production arg set (`--input-format
  stream-json`, `--bare`, `--system-prompt-file`) [read the platform's execution service, top-of-file comment].
- The array-shaped stdin differs from S1's measured user-turn shape and was never fixture-tested
  [read the platform's execution service; inferred]. Whether `--input-format stream-json`
  accepts it is **unverified**.
- Version drift?: The platform's `thinking`-block validation failure and the absence of `modelUsage` use in either
  source could reflect drift. 2.1.323 data has `thinking` rows. Whether 2.1.129 emitted them is unknown.

### 3.10 Supervisor behaviour compared with S1 and Stage 3
- `child.kill('SIGTERM')` hits the child only, not the process group. Tools and MCP servers can orphan
  [read the platform's execution service; inferred].
- It settles on `exit`, not `close`, so the last stdout lines may be unflushed [read the platform's execution service;
  inferred]. That could explain a lost `result`.
- The 200 ms grace is short [read the platform's execution service].
- Stderr tail is 4096 characters, not bytes [read the platform's execution service].
- Events are published only after exit, so the bus is not live [read the platform's execution service]. The event bus
  has no replay, buffer or backpressure, and its publish loop has no try/catch [read the platform's execution service].
- S1's order is `interrupt`, then close stdin, then SIGTERM [read `S1`,
  "Q5. Final message as the report; resume; cancel"]. The platform has no interrupt step.

## 4. Ideas Collide lacks or under-specifies

Items already approved and in plan 0.13 part F ("Model time per turn from `duration_api_ms`" and "Missing is not
zero") are cited as covered and are not proposed again.

### 4.1 Cache and usage kept separate, tiers kept apart
- Evidence: Ferry's runner merges read and creation [read Ferry's runner]. The platform keeps them apart but drops tiers
  [read the platform's execution service].
- Plan: 0.5 `usage` line has `cacheRead` and `cacheWrite` [read plan 0.5, Reporting]. 0.13 has token
  metrics. The 5m/1h split is not specified there [inferred; plan 0.13 not fully re-read for this term].
- Recommendation: **core.** Keep read, write-5m and write-1h as separate fields. Never merge them. **Ferry example
  pack:** send `cacheRead` and `cacheWrite` in its result file, not one merged cached-tokens number.

### 4.2 Spend from failed, killed and error results
- Evidence: 3.7.
- Plan: part F covers missing-is-not-zero for timing and sources. It does not say error results carry usage and cost.
  [inferred]
- Recommendation: **core.** Read `usage`, `total_cost_usd` and `modelUsage` from error results. When a process is
  killed with no `result`, record spend as "not reported" with whatever partial rows were seen. Do not record 0.

### 4.3 Cache hit ratio formula
- Evidence: Ferry's runner and the platform use different formulas (2.2).
- Plan: no formula in 0.13 [inferred].
- Recommendation: **core.** Define `hit rate = read / (input + read + write)`. Show it as "not reported" when the
  source reports no cache fields. Say in the label that Ollama-class endpoints may report none.

### 4.4 Other model calls
- Evidence: side-model calls appear in `modelUsage` and `total_cost_usd`, not in `result.usage` (3.1).
- Plan: S9 records the compaction exclusion. 0.13 sources table covers it for compaction [inferred].
- Recommendation: **core.** Report "other model calls" as `modelUsage` total minus `result.usage`, per model. Label it
  as a cost and token line, not part of the turn.

### 4.5 Hook denials and per-tool counts
- Evidence: tool counts are opt-in and lack denials (3.4). The read budget is a fail-open hook with a racy counter
  (2.3).
- Plan: 0.14 has a token-strategy preset that includes an "added plugin such as an output-condensing hook" arm
  [read plan 0.14, Token strategy preset]. A "files read again" detector is deferred [read
  plan: Deferred, More detectors].
- Recommendation: **core.** Record per-tool-name counts and hook denials for every session, not opt-in. Add a spike
  for how a PreToolUse deny appears in stream-json. **Ferry example pack:** the read-budget hook itself, as one 0.14
  arm, with its effect measured. It is not core.

### 4.6 Tool list as a start-weight lever
- Evidence: 3.8.
- Plan: 0.3 launch line and 0.14 cover tool lists [read plan 0.3, v0 ships one adapter, Claude].
- Recommendation: **core.** Record `--tools` against `--allowedTools` in the session fingerprint and in token-strategy
  arm labels, because they differ in start weight.

### 4.7 Exclude dependency and build directories from changed-file lists
- Evidence: a pnpm store made `JSON.stringify` overflow V8's 512M-character limit [read Ferry's runner].
- Plan: unclear whether range and diff views exclude dependency directories [inferred].
- Recommendation: **core:** the mechanism, a configurable exclude list on changed-file and diff views. **Ferry
  example pack:** the list data (`node_modules`, `.pnpm-store`, `dist`, `coverage`, lock files, `.tsbuildinfo`).

### 4.8 Token budget in USD, not tokens
- Evidence: 3.6.
- Plan: 0.3 uses `--max-budget-usd` [read plan 0.3, v0 ships one adapter, Claude]. Covered.
- Recommendation: **core.** Keep it as is. Do not add a token cap as a cost proxy.

### 4.9 Attribution under concurrency
- Evidence: Ferry attributes per-task spend by budget delta, exact only at concurrency 1 [read Ferry's source].
- Plan: Collide records per session, so it does not use deltas [inferred].
- Recommendation: **core.** Attribute by session id, never by a shared counter delta.

### 4.10 Record the claude version with each result
- Evidence: Ferry's runner results record none (header, 1).
- Plan: the fingerprint already records the claude version [inferred from plan 0.13; not re-verified].
- Recommendation: **core.** Keep it. Mark parsed fields with the version so version drift is visible.

## 5. Things to avoid

- Merging cache read and creation into one number (**core**).
- Defaulting missing usage fields to 0 (**core**; part F covers the rule).
- Throwing away stdout on a non-zero exit. The JSON may hold usage and cost (**core**).
- Settling a child on `exit` instead of `close`, and killing only the child, not its process group (**core**,
  Stage 3).
- A fixed 200 ms kill grace (**core**).
- Parsing only a hand-built fixture and calling it "verified". Capture with the real launch line (**core**).
- A read counter that is a shared file with read-increment-write (**Ferry example pack**; also a hook that fails open
  with no record that it did).
- Token caps as a cost control on caching endpoints (**core**).
- `docker -e KEY=value` for tokens. Values show in `ps`, which plan 0.5's `ps -E` rule forbids [inferred from
  plan 0.5; `docs/research/ferry.md` makes the same point for per-alias sources]. Tokens in plaintext config files
  [read Ferry's config files] (**Ferry example pack**).
- `curl | sh` from master and an unpinned `npm install -g` in a harness image [read Ferry's runner image build, the
  install lines] (**Ferry example pack**).
- Counting `text_delta` events as tokens [read the platform's execution service] (**core**).
- Treating `result/success` as "the work happened" (**core**).

## 6. Proposed plan changes (proposals only, not applied)

Each is exact text and the section it goes under. None of these edits the plan.

**P1. Plan 0.13, token metrics.** Add:
> Cache fields are kept separate and never merged: `cacheRead`, `cacheWrite5m`, `cacheWrite1h`. The cache hit rate
> is `cacheRead / (input + cacheRead + cacheWrite)`. It is "not reported" when the source reports no cache fields.

**P2. Plan 0.13, sources table (new row).** Add:
> Error and killed results. An error `result` carries `usage`, `total_cost_usd` and `modelUsage`. Read them. A process
> killed with no `result` records spend as "not reported" with the partial rows seen. It never records 0. This
> extends "Missing is not zero" to usage and cost.

**P3. Plan 0.13, sources table (new row).** Add:
> Other model calls. Side-model calls (compaction summaries, small helper models) are in `modelUsage` and
> `total_cost_usd`, not in `result.usage`. Report them as "other model calls": `modelUsage` minus `result.usage`,
> per model.

Spot-check correction (lead, after the note): in a multi-turn session `modelUsage` is a running total over the
process, like `total_cost_usd`; `model_usage_since_last_result` is the per-result value (measured: S9's
`04-env/A-100000.jsonl`, where `modelUsage` input grows 4 → 8 → 22,165 while `model_usage_since_last_result` gives
4, 4, 22,157). So per turn, "other model calls" are `model_usage_since_last_result` minus `result.usage`. The
single-turn platform fixture cannot show the difference.

**P4. Plan 0.13, tool metrics.** Add:
> Per-tool-name call counts and hook denials are recorded for every session. A `result` with `subtype: success` does
> not mean the work happened: also record `is_error`, `api_error_status` and `permission_denials`.

**P5. Plan 0.14, token-strategy preset.** Add:
> A read budget (deny `Read` after N calls) is one arm, supplied by the pack. Its effect is measured against the
> baseline with per-tool counts and denial counts. It is not a core default.

**P6. Plan 0.3, launch line.** Add:
> The session fingerprint records whether tools are limited by `--tools` or only pre-approved by `--allowedTools`,
> because `--allowedTools` leaves tool definitions in context.

**P7. Plan 0.3 or Stage 3 adapter.** Add:
> Process handling: settle on `close`, not `exit`. Stop in order `interrupt`, close stdin, SIGTERM to the process
> group, SIGKILL after a configurable grace. Stream-json test fixtures are captured with the real launch line,
> never written by hand.

**P8. Plan 0.13, changed-file and diff views.** Add:
> Changed-file lists exclude dependency and build directories by a configurable list. The Ferry pack ships its
> list as data.

**P9. Spike (S-new, small).** Add to the spike list:
> Record how a PreToolUse deny, a hook error and a permission denial appear in `stream-json` on the current claude
> version.

**P10. Plan 0.5, `usage` line.** Add:
> A pack that reports usage sends `cacheRead` and `cacheWrite` separately. A merged "cached" number is rejected.

## 7. Pointers outside metrics

**The platform's guard** (a layered prompt-injection guard). A pointer for plan 0.9 Untrusted input. No claim here that
it works.
- Three layers: a synchronous regex classifier with severity scoring and a trigger threshold of 3 (from OWASP LLM01
  and garak), an LLM judge with a bypass symbol to prevent recursion, and structured-output schema enforcement
  [read the platform's guard module; summary of the layers].
- A pre-flight check runs before the semaphore and spawn. A post-flight check runs after. It emits one audit row per
  turn and publishes a halt event [read the platform's execution service].
- Plan 0.9 may borrow the shape, a cheap local check before spawn and an audit row per turn [inferred].
- Regex classifiers are easy to bypass. A judge call costs tokens and needs its own metrics line [inferred].
- Guard time is outside the platform's duration (the clock starts after the pre-flight check) [read the platform's
  execution service].

**Other**
- Token hygiene: tokens in argv and in plaintext config files (section 5). Relevant to 0.5.
- Concurrency: the read counter race and budget-delta attribution (2.3, 4.9).
- The semaphore rejects when full instead of queueing, and is per process. Collide's scheduler should queue
  [inferred].
- Ferry's runner and the platform both hold one process per call. Neither tests a persistent session, which is what
  Collide does. Their handling of cumulative fields is therefore not evidence for or against Collide's part F.

## 8. What I couldn't read or verify

- Nothing was run. No claude, docker, build or test call. All behaviour is from reading source and recorded logs.
- The claude version inside the Ferry's runner image is unknown (unpinned install).
- Parts of Ferry's runner, its code-index tool and its source were not read in full.
- No "B-timing" section exists in Ferry's feedback doc. It appears only in code comments.
- No measured effect of the read budget or of tool-call counts was found.
- How a PreToolUse deny, hook error or permission denial appears in `stream-json` is unmeasured (P9).
- Whether `--input-format stream-json` accepts the platform's array stdin is unverified.
- Whether Ferry sessions compacted is unmeasured, so the size of the compaction undercount there is unknown.
- kimi cost, cache and `modelUsage` behaviour through claude are unmeasured. Cache zeros on Ollama may be "not
  reported" rather than zero.
- The tool-count check (3.4) uses three message ids. It is a small sample.
- Line numbers in `plan` for 0.5 and 0.14 and the deferred detector (near 2691) are from
  earlier reading and were not re-checked after the plan changed to revision 10.
- The guard summary is from a quick read and is out of scope for this note.
