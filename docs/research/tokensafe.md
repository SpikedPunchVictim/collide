# tokensafe: what, if anything, Collide should take

Date: 2026-11-14. Read-only research. Source: shallow clone of `tokensafe-dev/tokensafe` (version 3.1.5, one commit visible, HEAD message "docs: update README and metadata with outcome-driven 10-capability..."). Read as data; nothing from it was run. Labels: **measured** (read in the code or docs), **inferred**, **unmeasured**.

Collide references: `IMPLEMENTATION_PLAN.md` sections 0.3 (context options: `compact_at`, `config`, `bash_output_max`, `mcp_output_max`, `on_cold`, user tooling that loads into phases) and 0.13 (metrics, the seven detectors, fingerprint), the Deferred list, and `docs/research/tokens-and-context.md` (the earlier optimaizr evaluation, whose structure this note follows). The plan's own experiments (0.14) are the test bed for everything below.

## 1. What tokensafe is

- **License**: MIT (file `LICENSE`). The MIT part covers only what is in the repo; see risks for the commercial layer.
- **Language and size**: Python 3 (about 20.6K lines in `src/`), a Node launcher (`bin/tokensafe.js`), published to npm and PyPI. Single runtime dependency `cryptography` (for license-key verification, per `docs/ARCHITECTURE.md`). 56 test files in `tests/`. CI is `unittest discover` on Python 3.10-3.13.
- **Maturity**: young, one maintainer, fast version churn (3.1.5), very marketing-heavy README. `SECURITY.md` lists supported versions 2.3/2.4 while the code is 3.1.5, so it is not kept current. Open-core: free "Developer Sandbox" capped at 500K tokens a month, paid team and enterprise tiers (README, "Pricing").
- **Mechanism**: a local layer between a coding agent and the repo, sold as ten capabilities (README, "10 Core Engine Capabilities"). It reaches the agent three ways:
  1. **An MCP server** (`src/mcp_server.py`) with tools `smart_read_file`, `smart_bundle`, `scoped_search`, `get_architecture_graph`, `get_api_surface`, `list_dir`, `sanitize_command`, `context_budget`, `compact_conversation`, `guard_inspect`. Backed by `src/skeletonizer.py` (folds function bodies to signatures, regex/brace lexer plus Python `ast`, optional tree-sitter in `src/treesitter_backend.py`), `src/graph_engine.py` (import graph, "God Nodes"), `src/slicer.py`, `src/schema_slicer.py` (lockfiles, OpenAPI).
  2. **Agent hooks** (`hooks/claude_code_hook.py`, `hooks/pre_tool_use.py`, `hooks.json`):
     - `SessionStart` injects `TOKENSAFE_DIRECTIVES` (`src/adapter_core.py`) as `additionalContext`. It tells the agent that grep/rg/find on source "are BLOCKED" and to use the MCP tools instead.
     - `UserPromptSubmit` injects a compacted form of the prompt (TOCP, `src/compactor.py`) as `additionalContext`, alongside the original.
     - `PreToolUse` runs `TokenSafeGuard` (`src/guard.py`): denies secret-bearing or destructive Bash commands, `git add` of `.env`, reads of secret files, data exfiltration patterns, raw code search; asks on TLS bypass. On writes it runs `guard_write` in `src/adapter_core.py` (deny a wholesale `Write` over an existing file of 40 lines or more; ask on a small delta).
     - `PostToolUse` calls `CommandSanitizer.compact_tool_output` and answers `decision: block` with a summary for successful noisy commands.
     - On the Antigravity path (`hooks/pre_tool_use.py`) it also rewrites tool input (`overwrite`): reads become skeleton files, commands are rewritten by `CommandSanitizer.sanitize` (run-once test flags such as `CI=true` and `--watch=false`, lockfile stripping), and `LoopDetector` (`src/loop_detector.py`: sha256 of tool name plus sorted args, three identical calls) forces a confirmation.
  3. **An optional local proxy** (`src/proxy.py`, `src/installer.py`): `tokensafe proxy on` sets `ANTHROPIC_BASE_URL=http://127.0.0.1:8765`. In the proxy: `ContextGovernor` (`src/governor.py`, never edits history inside an epoch, rolls over at 80% of the window), `PromptCacheNormalizer` (`src/cache_normalizer.py`, sorts tools, strips volatile ids, injects `cache_control`), `result_ledger.py` (folds aged tool results), `history_compactor.py`.
- **Other parts**: `AgentFirewall` (`src/agent_firewall.py`, SHA-256 keyed repeat-read pointer cache), `EnvironmentAuditor` (`src/environment_auditor.py`, `tokensafe doctor`, counts MCP tools and skill tax, can disable an MCP server in config), `SecretRedactor`, a SQLite metrics store (`src/metrics.py`), `team_cache.py`, `license_manager.py`, `telemetry_sync.py`, `version_checker.py`.
- **Evidence quality** (measured from `README.md`, `BENCHMARK_REPORT.md`, `METRICS.md`, `docs/BENCHMARK_REAL.md`):
  - Headline "-95%" is a simulated harness with modeled baselines (the README says so).
  - Real-repo numbers compare against strawman baselines: reading a whole file versus a skeleton, a whole-repo crawl versus a graph summary, uncached versus cached input. Claude Code reads with offset and limit, greps, and already uses prompt caching, so the real delta is unmeasured.
  - Its own clean-pytest and git-diff rows show 0-9%. 17 of the gated rows fail.
  - It does not report task outcome (the "100% accuracy" column is modeled).

## 2. Ideas worth considering

### A. Detect tokensafe-style tooling as a context-changer (and record the proxy)
- **What**: tokensafe can install, into the user's `~/.claude/settings.json`, hooks that rewrite or inject (`SessionStart` and `UserPromptSubmit` `additionalContext`, `PreToolUse` deny/ask, `PostToolUse` block-with-summary), an MCP server (about 10 tool schemas of start weight), and `env.ANTHROPIC_BASE_URL` for the proxy (`src/installer.py`).
- **Collide's side**: 0.3 "The user's own tooling loads into phases" lists hooks that rewrite tool calls, block prompts or set compaction variables, and the fingerprint records "the context options in effect". It strips inherited `CLAUDE_*` variables. **Gap**: the plan never names `ANTHROPIC_BASE_URL` (or `ANTHROPIC_*` in settings `env`). A settings-level base URL survives the env strip, sends every phase through a third-party proxy that edits requests, and breaks cost and cache comparisons between arms (and `apiKeySource` reasoning).
- **Verdict**: **adopt**, tiny. Add to the flagged list: `env.ANTHROPIC_BASE_URL` and other `ANTHROPIC_*` in `settings.json`, a hook that returns `additionalContext` on `SessionStart` or `UserPromptSubmit`, `PostToolUse` hooks that block with a summary, and the name "tokensafe" among known token tools beside rtk and optimaizr. Record the effective base URL host (not the full URL) in the fingerprint.
- **Where**: **core** (run preview and fingerprint).
- **Cheapest test**: install a throwaway `settings.json` fixture with these entries (never the user's own) and assert the preview flags each and the fingerprint records the URL host. This is a fixture test, no model call. Related to S9's blocked-prompt check.

### B. Run tokensafe as an experiment arm, not as a dependency
- **What**: the strongest claim (skeleton reads, graph-first discovery, command rewriting) is unproven on real task outcomes. Collide's 0.14 experiments exist to answer exactly this and tokensafe's own benchmarks cannot.
- **Collide's side**: 0.14 lets an arm differ by launch parameters and env, and results carry cost, tokens, cache hit rate, re-reads and the outcome (gates, CI, accepted). The pack's `experiment.prepare` can give an arm its own config.
- **Verdict**: **adapt** (as a test, not code). Do not vendor or depend on it. Once Stage 15 exists, a "tokensafe on versus off" arm on a small Orchard task set, run with `TOKENSAFE_TELEMETRY_DISABLED=1` and its own data dir, answers whether the idea works for Collide's users. Also a way to say yes or no to users who ask.
- **Where**: neither; an experiment definition in a pack or the owner's board.
- **Cheapest test**: one pair of runs on one task, reading only the token and outcome rows, before any crossover. Requires the Stage 15 machinery, so schedule it then. Before that, a manual S9-style probe: one phase with the tokensafe MCP server under `--strict-mcp-config` and an explicit `--mcp-config`, comparing start weight (its ~10 schemas against Collide's measured 14.4K floor).

### C. In-session guard pack: secrets and destructive commands
- **What**: `TokenSafeGuard` deny rules (`src/guard.py`, `src/adapter_core.py` patterns): hardcoded secret fallbacks in writes, `git add .` with an unignored `.env`, `cat .env`, `rm -rf /` or `~`, `dd of=/dev/`, curl-to-exfil, TLS bypass, SQL string interpolation.
- **Collide's side**: gates and hooks (0.4, 0.5) can deny at named points, and 0.3 records who refused a call. Section 0.9 covers untrusted input, but no shipped rule set for agent writes exists in the plan. Claude Code's own permission rules cover some of this per user.
- **Verdict**: **adapt**. The rule list is the useful part (regexes, 20-ish checks, tests in `tests/test_guard.py`); do not take the implementation (Python, telemetry wired in, agent directives). Reimplement a small deny set in TypeScript as an optional pack hook. It is a safety feature, not a token feature, so it should not ride on the token-strategy preset.
- **Where**: **pack**.
- **Cheapest test**: replay each deny pattern and 20 benign commands from S1 transcripts through the hook. Count false denies (a false deny stalls a headless phase, and 0.3 already routes a blocked call to the Queue).

### D. Run-once test runner guard
- **What**: `CommandSanitizer._optimize_test_stage` rewrites test commands to terminate: `CI=true`, `--watch=false`, `--forceExit`, bare `vitest` to `vitest run`, and it peeks at `package.json` to see which runner sits behind `npm test`.
- **Collide's side**: phases have timeouts, and a hung watch-mode test run is a real headless failure (an agent phase has no human to press `q`). The plan has no mention of run-once env. 0.13 detects "same command failing again" but not "command that never returned".
- **Verdict**: **adapt**. Take the idea only. The cheap form is a pack-set env (`CI=true`) for phases, not command rewriting (rewriting is brittle, and `CI=true` changes some tools' behavior, such as treating warnings as errors).
- **Where**: **pack** (env in the pack's agent launch options). A core detector "Bash call that hit the tool timeout" is a possible later shape-only detector.
- **Cheapest test**: grep Collide's own recorded runs and S1 transcripts for Bash calls that hit their timeout. If there are none, skip.

### E. Output-token waste detector: whole-file `Write` over an existing file
- **What**: `guard_write` denies a full rewrite of a 40-line-or-longer file because output tokens cost 5x input. tokensafe enforces it. Enforcement quality is unmeasured and risks a retry loop.
- **Collide's side**: the plan's seven detectors read shapes only (cold return, heavy start, compaction mid-task, oversized tool output, repeated failing command, hook failed open, call to a missing tool). Output tokens per tool call are in the stream, and "More detectors" (files read again, prompt size, cache churn, model fit) is deferred.
- **Verdict**: **adapt**. As an observational detector ("Write over an existing file: N calls, X output tokens"), not a block. Shape-only: tool name, whether the file existed, input bytes. It slots into the deferred "More detectors" with the same trigger.
- **Where**: **core** detector, later. A deny hook version is a **pack**.
- **Cheapest test**: count `Write` calls on existing files and their input bytes in the S1 transcripts. If under about 2% of output tokens, drop it.

### F. Loop guard thresholds
- **What**: three identical (tool, args) calls in a session trigger `force_ask`. Cheap, deterministic, shared state under a file lock.
- **Collide's side**: the detector "same command failing again unchanged" (0.13) already counts the Bash case. The token research already put in-session guards in a pack or plugin ("Not proposed").
- **Verdict**: **skip** as new work; keep as a reference point for a later pack hook. Note its key (sha256 of tool name plus sorted args, N=3) is a sensible definition if the existing detector needs one: "unchanged" should mean identical tool and canonical args, not only Bash text.
- **Where**: core (detector, already planned) or pack (enforcement).
- **Cheapest test**: run the detector's definition over S1 transcripts and compare the count at N=2, 3, 4.

### G. Metrics honesty wording: "injection is not replacement"
- **What**: `METRICS.md` rule 2: a saving only becomes "measured" when the compact form replaces the bytes the model sees. Its folded-history and prompt-compaction numbers stay "estimated" because the originals remain in context. Rule 1: a missing value renders as a dash, never a made-up number.
- **Collide's side**: 0.13 already labels measured, inferred, estimated and says "missing is not zero". The remaining gap is wording for packs: a pack or hook that reports "tokens saved" through `COLLIDE_REPORT` (0.5, "reported") must not be summed with measured tokens. 0.13 already says reported usage is shown beside measured, never added.
- **Verdict**: **adopt** as one sentence in the pack docs and the reported-metrics schema: a reported "saved" figure is an estimate unless the pack proves the original bytes never reached the model. Evidence for caution: a `PostToolUse` hook cannot remove an already-returned built-in tool result from context in Claude Code (inferred; not tested here), so tokensafe's `TOOL_OUTPUT_COMPACT` likely saves less than its record says.
- **Where**: **core** (docs and schema label).
- **Cheapest test**: a schema fixture where a pack reports `saved_tokens`, and an assertion that it renders as "reported" and never joins a measured total.

### H. Reusable details worth a look, no verdict beyond skip
- **Context budget warning at 80%**: Collide already reads `get_context_usage` and sets `compact_at` (a 100K floor measured in S9). **Skip.**
- **Tool-schema bloat heuristic** ("40+ tools in one server is CRITICAL"): Collide's run preview already shows the MCP tools breakdown from `get_context_usage`. **Skip.**
- **Pre-registered scenarios with deterministic zero-loss gates** (`src/bench_scenarios.json`, `src/benchmark_real.py`): the discipline (fix scenarios before measuring, publish failures) matches 0.14's balanced crossover and fixed verdict rule, which is stronger because it uses outcomes. **Skip**, already covered.

## 3. Already covered by Collide (do not re-raise)

- Token, cache and compaction metrics from the stream, one usage per `message.id`: 0.13 (the transcript-inflation finding is also in tokens-and-context.md).
- Measured / inferred / estimated labels, and "missing is not zero": 0.13.
- Compaction point (`compact_at`), output caps (`bash_output_max`, `mcp_output_max`), `config: user | none`, and the token strategy preset: 0.3 and 0.14. These do the job of tokensafe's governor and lockfile/output shield at the launch level, with S9 measurements.
- Cold-cache handling and a fresh-session handoff: `on_cold: resume | fresh` in 0.3 (S9 measured: $0.105 cold resume at 114K, $0.004 fresh with a note). tokensafe's proxy-level epoch rollover and cache normalizer address the same cost from the API side, which Collide does not need because it owns the launch, not the request.
- Detecting user-level hooks and plugins that rewrite calls or hold prompts: 0.3 (item A above only adds the base URL and `additionalContext` cases).
- Repeated failing command: 0.13 detector. Same-file reads in one context epoch: counted in the always-recorded metrics.
- Run history and spend visibility: 0.13 and Stage 9c (tokensafe's SQLite dashboard and Friday digest, which are also a paid feature).
- Start-weight reduction: measured in S9 (`config: none` 14.4K against 30.9K); an MCP schema "tax" doc is just a smaller version of that.

## 4. Risks

- **License**: MIT, compatible with copying patterns, and Collide's repo is public. Copying code verbatim needs the copyright notice kept. The pricing and licence-key code (`src/license_manager.py`, `src/cloud_backend.py` excluded from the npm package) shows the project is built to monetise, so the MIT part may lag behind the paid one. The free tier stops optimizing after 500K tokens a month while keeping guards on (see `hooks/pre_tool_use.py`, quota branch), so behavior depends on a counter, which would make an experiment arm non-stationary.
- **Maintenance**: one author, 3.x with fast churn, stale `SECURITY.md`. A rule list copied today would need its own upkeep.
- **Security** (read as data; confirm before ever installing):
  - Installs write into `~/.claude/settings.json`, MCP configs of 10+ agents and an optional launchd/systemd service (`src/installer.py`).
  - The proxy mode sets `ANTHROPIC_BASE_URL` and sees full prompts and keys in transit.
  - `src/installer.py` and `src/version_checker.py` can self-update (`git pull`, `npm`, `pip install --upgrade`) and spawn detached processes.
  - Hooks fork a background telemetry sync on every event (`src/telemetry_sync.py`) that posts to `api.tokensafe.dev`. The docs say counters only, but the event `details` include command prefixes (`cmd[:100]`) and file basenames, and a machine fingerprint goes up in the free "community" mode unless `TOKENSAFE_TELEMETRY_DISABLED` is set. The license key travels in the version-check URL. Unmeasured: what the sync sends after its own sanitizing step.
  - Its hooks fail open on any internal error (by design), so a broken guard silently allows.
  - Its install path (`curl | bash`, `npx -y`) is the usual supply-chain exposure.
- **Lock-in and behavior change**: its directives and `CLAUDE.md`, `AGENTS.md`, `.cursorrules`, `.clinerules` and `.windsurfrules` tell the agent to use tokensafe tools, block grep/rg on source (a large behavior change that may hurt quality), and append a "Turn Savings" footer to every reply. None of that was followed here; it is only noted. Adopting any of it into a pack would change agent behavior silently, which the owner's rule for `on_cold` default (a choice that experiments test) says not to do.
- **Prompt injection surface**: `additionalContext` from hooks is trusted instruction text. A pack that copied this pattern would need 0.9's untrusted-input treatment for any text derived from repo content.
- **Strawman benchmarks**: do not quote its percentages in Collide docs or planning.

## Net recommendation

1. **Adopt A** (flag `ANTHROPIC_BASE_URL` and `additionalContext` hooks as context-changers; record the base URL host). Small, core, and fixes a real hole in the fingerprint.
2. **Adopt G** (one sentence on estimate versus measured for pack-reported savings).
3. **Adapt B** as an experiment arm at Stage 15, never as a dependency.
4. **Adapt C, D, E** only if their cheap tests (false-deny replay; timeout grep; `Write` share of output tokens) show the problem exists. C and D are packs, E is a deferred core detector.
5. **Skip** the skeletonizer, graph, proxy, prompt compaction, ledger and cache normalizer: wrong layer for Collide (it owns the launch, not the API request), Python, unproven outcomes.
