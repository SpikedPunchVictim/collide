# Token saving and context management: evaluation for Collide

Date: 2026-11-13. This is input to plan revision 10. The owner decides the proposals in section 4.

Sources:

- **optimaizr**: the docs and source of optimaizr 0.13.1 (commit `10bb10b`, MIT), read as data. Its code was not
  run.
- **S1 transcripts**: the transcripts of spike S1's own phases under `~/.claude/projects/*collide-spikes-s1*`, read
  for token counts only.
- **The binary**: the strings of the `claude` 2.1.323 binary.

Every claim carries a label:

- **measured**: from an observation, which is named;
- **inferred**: derived from measured data, or read from optimaizr's docs or source;
- **unmeasured**: not checked.

## 1. Where a Collide phase spends tokens

- **Headless phases write the 1-hour cache only** (measured).
  - Across 105 S1 sessions, `cache_creation.ephemeral_5m_input_tokens` was 0 every time.
  - All cache writes were `ephemeral_1h_input_tokens`.
  - Whether an API-key login writes 5-minute cache instead is unmeasured. The binary contains
    `ENABLE_PROMPT_CACHING_1H` and `DISABLE_PROMPT_CACHING`.
- **Cache pricing** (inferred from optimaizr's dated rate cards, not checked against Anthropic's price page):
  - a 1-hour write bills at 2× the input rate;
  - a cache read bills at 10% of the input rate by default, 5% on Opus 5.5 and 2.5% on Fable 5.x.
  - So a phase whose cache has expired pays about 20–80× more to re-send its context than a warm one.
- **The user's config is the largest fixed cost** (measured, S1 transcripts; first call's input + cache read +
  cache write):
  - With `--setting-sources user`, a phase starts at a median of 29–34K tokens across the S1 directories.
  - The smallest starts were 6.9–7.7K.
  - S1 counted 108 plugin skills in context with the user's config. Every phase pays this before it does any work.
- **Transcripts over-count usage** (measured). Claude Code writes one transcript record per content block, each
  stamped with the parent response's usage.
  - In 84 of the 105 S1 sessions, at least one response had more than one usage row.
  - optimaizr measured 1.91× inflation on its author's history (inferred).
  - Any reader must keep one usage per `message.id`.
  - Whether stream-json `assistant` events repeat usage the same way is unmeasured. Until it is, the stream reader
    dedupes too.
- **Compaction is visible** (inferred; the binary and optimaizr's parser).
  - Transcripts carry `system/compact_boundary` with `compactMetadata` fields: `trigger` (auto or manual),
    `preTokens`, `postTokens` and `durationMs`.
  - optimaizr also marks a compaction mid-task when the reply before it stopped on `tool_use`.
  - Whether stream-json emits it too is unmeasured.
  - Knobs in the binary: `CLAUDE_CODE_AUTO_COMPACT_WINDOW`, `CLAUDE_AUTOCOMPACT_PCT_OVERRIDE`,
    `DISABLE_AUTO_COMPACT`, `DISABLE_COMPACT`.
  - optimaizr reports that a window of 200000 fired at 166–170K (inferred).
- **Output caps** in the binary (measured as strings; behaviour unmeasured): `BASH_MAX_OUTPUT_LENGTH`,
  `MAX_MCP_OUTPUT_TOKENS` and `CLAUDE_CODE_MAX_OUTPUT_TOKENS`.
- **Where Collide itself makes a cache go cold** (inferred from the plan):
  - **Usage-limit pause.** A phase resumes by session id when the limit resets. That can be hours away, well past
    the 1-hour TTL, so every limit resume rewrites the whole context.
  - **Queue waits inside a live phase:** a permission prompt or a stop-and-report answered after an hour.
  - **Take-over and resume after an owner restart**, when the gap passes the TTL.
  - **Sequential experiment arms:** arm B can read the prompt cache arm A wrote. 0.14 already lists the provider's
    prompt cache as an order effect. The cache write/read split per arm makes it visible.
- **User-level tooling changes phases** (measured in S1, inferred for optimaizr):
  - S1 found that a user `PreToolUse` hook rewrites Bash commands in phases. That hook is rtk's.
  - optimaizr writes `~/.claude/settings.json`:
    - `guard on` adds hooks. One is a prompt hold for an expired cache, which could block the prompt of a resumed
      headless phase once; unmeasured.
    - `apply context-compaction` adds `env.CLAUDE_CODE_AUTO_COMPACT_WINDOW`.
  - Phases load both through `--setting-sources user`.
  - The plan strips inherited `CLAUDE_*` variables from the environment, but a value set in `settings.json` `env`
    still applies. So a shell export of the compaction window is dropped and a settings value is kept.

## 2. What optimaizr does, and how each technique fits Collide

| technique | mechanism | fit for Collide |
|---|---|---|
| Where tokens go | Splits spend into re-reading the conversation (cache reads), loading it into cache (writes) and output | **core**: from the stream for Collide's runs, from transcripts for imported history |
| Usage dedupe by `message.id` | One usage per response; prefers the completed record | **core**: required for the past-session import; check the stream (S9) |
| Dated rate cards | Each call is costed at the rate in force on its day | Collide's own runs use `total_cost_usd` from claude. Imported history has no cost, so it needs cards or shows tokens only |
| Three kinds of number | measured, inferred and estimated; a model swap is never shown as measured | **core**: extends the plan's measured/reported labels with "estimated" for any projected saving |
| Cold-cache returns | A call after the cache TTL rewrites the whole context; reports the cost and who was waiting | **core, and Collide knows more**: it knows each wait's cause (Queue, limit, CI) and can act before the resume |
| Compaction tracking | Counts, trigger, mid-task share, summary wait, files re-read after a summary | **core** metric; the compaction point per role is a launch option |
| Compact earlier (`CLAUDE_CODE_AUTO_COMPACT_WINDOW`) | Replays history as if compacted at 200K and prices it; applied as a setting with undo | **lever**: a per-role launch option and an experiment arm |
| Smaller default model | Re-prices "quick" tasks (≤8 calls, ≤2 files, ≤1 failure) one tier down; `verify` replays them | **lever**: Collide already maps roles to models; experiments measure the outcome, which optimaizr cannot |
| Heavy start | Conversations that start heavy (MCP, agents, skills) | **core** metric: each phase's start weight, broken down from `system/init` |
| Detectors (15 rules) | Re-reads, oversized tool output, error loops, cache churn, prompt bloat, … | **core**, a subset, computed from shapes only (section 4, decision 1) |
| Guard (holds) | Hooks hold an expired-cache prompt, a whole read over 64KB, a third identical failing command | Holding an expired-cache resume: **core**, at the Queue. Read and retry holds are in-session behaviour: **pack or plugin**, not core |
| Handoff note | On `/clear`, a note built from the transcript with no model call: asked, changed, dropped, open, last reply | **lever**: a cold resume can start a fresh session from a handoff instead |
| Before/after results | Splits usage at a recorded change; 14 days each side; trade-off rows; verdict only past 10% on every baseline | **core**, secondary to experiments: for changes made outside an experiment |
| Live status | Cache countdown and 5-hour window | Fits the Queue and the run header |

**What Collide can do and a log reader cannot:**

- **It owns the launch.** Collide can set the compaction point, the config sources, the model and the output caps
  per phase, and pin them for an experiment.
- **It knows the outcome.** Gates, CI, review and accept show whether a cheaper run did the same work. optimaizr
  needs a replay and a scorer (`verify`) to guess that.
- **It knows why a cache went cold**, and it is the one deciding to resume.

## 3. Context management in Collide's design

The workflow model is already a context strategy:

- each phase is a fresh session (`--session-id`);
- what passes between phases is explicit: the phase report, files in the worktree and hook outputs;
- roles can use different models.

The cost of that strategy is the fixed start weight per phase, which is about 30K tokens with the user's config.
A short phase pays it as a 1-hour write.

**The open choice is resuming versus starting fresh.** A resumed phase keeps its whole context and pays a full
rewrite once the cache is cold. A fresh session with a handoff pays only the start weight plus the handoff.

## 4. Proposals (the owner decides)

### 1. Always-recorded token and context metrics (core; no pack needed)

**What is recorded.** From the stream, per response, deduped by `message.id`:

- input, cache read, cache write (1h and 5m) and output tokens;
- context size;
- per phase:
  - start weight;
  - peak context;
  - compactions;
  - cache hit rate;
  - cold returns, with each one's cause;
  - tool-result bytes per tool name;
  - same-file reads within one context epoch, as a count;
  - the same command failing again unchanged, as a count;
- the effective context settings in the fingerprint: config sources, the compaction window, output caps, and the
  names of user hooks and plugins.

**Where it goes and what it feeds:**

- It goes into 0.13's always-recorded level and into Stage 9's no-pack metrics.
- The same parser runs over the opt-in past-session import (metrics decision 4).
- With that import, a first run with no pack shows "where your tokens go" from the user's history.

**Detectors and labels:**

- Detectors are pure functions over shapes, never over content. Each finding says what happened, what it cost
  (measured) and what changing it would save (estimated).
- Every number is labelled measured, inferred or estimated.

**Options:**

- (a) **The full set above.** Pro: the no-pack value the owner asked for, and the data every lever below needs.
  Con: about a dozen detectors to build and keep correct as Claude Code's formats change.
- (b) **Tokens, cache and compactions only**, with no detectors. Pro: small. Con: shows numbers with no "what to
  do", which is the part that makes optimaizr useful.
- (c) **Leave it to optimaizr.** It already reads phase transcripts, because they are ordinary sessions. Pro:
  nothing to build. Con: it cannot see phases, runs, arms, wait causes or outcomes. Every worktree is a different
  "project" to it.

**Rec: (a).** Ship the first five detectors:

- cold return;
- start weight;
- compaction mid-task;
- oversized tool output;
- repeated failing command.

The rest are deferred.

### 2. Cold resume handled at the Queue (core)

**What the user sees and chooses:**

- A Queue item for a live phase shows its cache countdown and the rewrite cost of answering late. For example: "warm
  for 12 min, then $1.30 to resume".
- A usage-limit resume, which is always cold, shows its rewrite cost in the preview.
- A phase may declare `on_cold: resume | fresh`.
  - `fresh` starts a new session from a handoff note.
  - Collide builds the note from the transcript with no model call: the task, files changed, commands that kept
    failing, open items and the last reply. The same note can serve take-over.

**Options:**

- (a) **The countdown and cost only.** Pro: no behaviour change. Con: the user pays the rewrite anyway.
- (b) **(a) plus `on_cold`, defaulting to `resume`.** Pro: packs and experiments can choose `fresh`. Con: a
  handoff loses context, so the outcome may change. That is why it is a choice that experiments can test.
- (c) **(b) with `fresh` as the default.** Con: changes agent behaviour silently.

**Rec: (b).**

### 3. Context levers as launch options, testable as experiment arms (core)

**Per-role launch options** in the pack's model mapping, overridable per board and per arm:

- `compact_at` sets `CLAUDE_CODE_AUTO_COMPACT_WINDOW`;
- `config: user | none` chooses `--setting-sources user` or `""`;
- `tool_output_max` sets `BASH_MAX_OUTPUT_LENGTH` and `MAX_MCP_OUTPUT_TOKENS`.

Collide sets these values after it strips the inherited environment. The fingerprint records the effective values,
including any that the user's `settings.json` sets.

**Experiments:**

- 0.14 already lets an arm differ by launch parameters and env.
- Add a "token strategy" preset that pairs two values of one lever.
- Its result rows are cost, tokens, cache hit rate, compactions, re-reads, tool errors, active time and the outcome
  (gates, CI, accepted). Each saving therefore shows what it cost.
- Experiments keep the existing verdict rule.
- For changes made outside an experiment, add optimaizr-style before/after results split at a fingerprint change.
  These are labelled inferred, never a verdict.

**Options:**

- (a) **The three options plus the preset.**
- (b) **Only `config: user | none`**, which is the measured 4× start-weight lever.
- (c) **Nothing new**: arms use raw env overrides.

**Rec: (a).** The knobs are env variables Collide already sets, and the preset is UI over 0.14.

### 4. User-level tooling detected, not fought (core)

**What Collide does:**

- The run preview lists the user hooks and plugins that phases will load, and flags known context-changers. These
  are hooks that rewrite tool calls, hold prompts or set compaction or caching env.
- When a phase's prompt is blocked by a hook, the event is recorded and the run goes to the Queue with the hook's
  message. It is never a silent `result/success`.
- Experiment arms keep `--setting-sources ""`, as S1 decided.

**Rec: take it.** This is the S1 finding generalised, and it matters for any user of rtk, optimaizr or similar
tools.

### 5. Measure before building (spike S9, "tokens and context")

S9 is small: a Sonnet agent, about $3, run alongside S2. It measures:

- whether stream-json repeats usage per content block, and whether it emits `compact_boundary` with its metadata. A
  manual `/compact` sent on stdin makes the check cheap.
- the 1-hour versus 5-minute cache tier for an API-key login, if a key is available; otherwise unmeasured;
- start weight under `config: user` and `none`, with the breakdown `system/init` gives;
- the effect of `CLAUDE_CODE_AUTO_COMPACT_WINDOW` and the output caps set in the phase env;
- what the stream shows when a `UserPromptSubmit` hook blocks a resumed prompt. The hook comes from a spike-local
  `--settings` file, never the user's;
- the cost of a resume after more than an hour idle, against a fresh session with a handoff note. One background
  pair.

**Rec: take it.** Every number in proposals 1–3 depends on stream shapes that S1 did not measure.

## 5. Not proposed

- **Depending on optimaizr** (npm, Node).
  - It is a 0.x project by one maintainer, built for interactive sessions.
  - Collide's reader is Rust, over its own stream.
  - Coexistence costs nothing: optimaizr keeps reading phase transcripts.
- **A Collide-owned price table for its own runs.** `total_cost_usd` is measured.
  - Imported history needs a price.
  - **Rec:** imported history shows tokens and cache hit rate, plus cost labelled "estimated" from a small dated
    rate-card file shipped with Collide.
  - The alternative is tokens only, with no cost, for imported history.
- **In-session guards** (whole-file read, retry). These are agent behaviour, so they belong in a pack or a plugin
  hook. A pack can ship one, and an experiment can test it.
