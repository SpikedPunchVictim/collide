# Spike S1: Claude Code headless

Throwaway reproduction scripts for spike S1 of `IMPLEMENTATION_PLAN.md` (Stage 1). The decision record is
`.collide/planning/spikes/sp-0001-de-claude-headless/record.md`.

## Prerequisites

- macOS or Linux with `sh`, `git`, `jq`, `node` (measured with v24.18.0), `python3` and `uuidgen`.
- `claude` on `PATH`, logged in (measured with 2.1.323, claude.ai login).
- Every measured call runs through `claude-clean.sh`, which removes the calling shell's `CLAUDE*` variables, so the
  scripts can be started from inside a Claude Code session. Only `09-env.sh` variant `a` deliberately keeps them.
- Every call uses `--model haiku` (except `03` variant `b`, where the model is the question) and
  `--max-budget-usd 0.25`. Override with `S1_MODEL` / `S1_BUDGET`.

## Running

```sh
./run-all.sh          # deletes out/ and work/, runs 00..09 in order, writes out/<script>.txt, prints spend
./07-snapshot-settings.sh   # or any one script; each prints its measurement on stdout
```

Run scripts one at a time: `00-fixture.sh` deletes and rebuilds `work/`, which the others use. Scripts
`02`..`07` and `09` build the fixture first if `work/.fixture-ok` is missing. `08` reads the logs of `01`.

## Scripts, in order

| script | question | prints |
|---|---|---|
| `00-fixture.sh` | (setup) | builds `work/`: scratch git repo `repo` (main, tag BASE, untracked MAIN-LOCAL files), worktree `wt` (builder edits, tag WT / WT-LOCAL), `snapshot` (tag SNAP: settings, MCP config, agents JSON, a plugin with a skill), `plain` (no git) |
| `01-multiturn.sh` | Q1 | exit and stderr without `--verbose`; the event sequence of a two-turn stream-json session; the session id and cost per turn |
| `02-permission-host.sh` | Q2 | `--permission-prompts host` alone / with `initialize` / with `--permission-prompt-tool stdio`, the hidden flag alone, and a host that closes stdin with a request pending: the `can_use_tool` request, the allow and deny answers, and what each produces |
| `03-agents.sh` | Q3 | `--agents <file>` + `--agent`: the model that ran (frontmatter vs `--model`), and which definition wins a name clash with the worktree's `.claude/agents/` |
| `04-budget.sh` | Q4 | a tiny `--max-budget-usd`: the result event, whether the process and a second turn survive, exit code; a mid-turn stop; whether a resumed process's budget counts the session's earlier cost |
| `05-report-resume-cancel.sh` | Q5 | `result.result` vs the assistant messages; `--resume <id>` in a new process; cancel by interrupt control request, SIGINT and SIGTERM (events, exit code, surviving tool processes, resume after) |
| `06-worktree-loading.sh` | Q6 | plain `-p` in the worktree: which hooks (by tag), MCP servers, skills, CLAUDE.md files, plugins, agents and memory paths load |
| `07-snapshot-settings.sh` | Q7 | the same with snapshot settings (`--setting-sources user`, `--settings`, `--mcp-config`, `--strict-mcp-config`, `--plugin-dir`), plus variants: `--agents`, `--disable-slash-commands`, `claudeMdExcludes`, `--safe-mode`, `--setting-sources ""`, an auto-memory canary, `autoMemoryEnabled:false`, `CLAUDE_CODE_DISABLE_AUTO_MEMORY=1`, `--restricted` |
| `08-limits.sh` | Q8 | read only, no spend: the rate-limit schemas from the installed binary, the live `rate_limit_event` from `01`, and the masked shape of limit messages in local transcripts |
| `09-env.sh` | Q9 | nested (CLAUDE* left set) vs clean launch: refusals, session id, and the CLAUDE*/ANTHROPIC_* variable names seen by a hook, an MCP server and a Bash tool call |

Helpers: `driver.mjs` (the stream-json host: turns, permission answers, cancels, JSONL log of both directions),
`summarize.mjs` (event sequence and per-process cost), `lib.sh` (shared flags and the loading report),
`make-settings.mjs` (settings whose hooks write tagged marks), `mcp-canary.mjs` (a one-tool MCP server that writes
a mark when it starts), `build-agents.mjs` (`.md` agent definitions to the `--agents` JSON), `claude-clean.sh`
(copied from S3; adds an `S1_PASS` keep-list).

## Side effects outside this directory

- Every `claude` run writes its transcript under `~/.claude/projects/<key of the cwd>/`, as any session does.
- `07` variant `g` writes one file, `MEMORY.md`, into the scratch repo's auto-memory directory
  `~/.claude/projects/<…-s1-claude-headless-work-repo>/memory/`, runs `g` to `j`, then deletes it. It
  refuses to run if that file already exists, and skips if `init.memory_paths.auto` is not that directory.
- `08` reads the installed `claude` binary and scans `~/.claude/projects/**/*.jsonl` for limit messages; it prints
  only keys, types and masked text.
- No user settings, agents, plugins or MCP config are changed. The runs do load the user's own hooks, plugins and
  MCP servers where the question is whether they load.

## Spend

Notional `total_cost_usd` as reported by `claude` (the account is a claude.ai subscription). `./spend.sh` prints it
from the logs in `out/`: per process, what it added to its session's `total_cost_usd` (a `--resume`d process
starts from the session's earlier total). One clean pass on 2026-11-13 (`run-all.sh`, then `02`, `04` and `07`
rerun after their last variants were added):

| script | spend |
|---|---|
| `01-multiturn.sh` | $0.0105 |
| `02-permission-host.sh` | $0.0136 |
| `03-agents.sh` | $0.0048 |
| `04-budget.sh` | $0.0114 |
| `05-report-resume-cancel.sh` | $0.0146 |
| `06-worktree-loading.sh` | $0.0043 |
| `07-snapshot-settings.sh` | $0.0169 |
| `08-limits.sh` | $0 (no calls) |
| `09-env.sh` | $0.0054 |
| total | $0.0815 |

Prompt caching makes reruns cheaper than first runs, so the same script can cost half as much on a second pass. The SIGTERM variant of `05` reports no cost because the process is killed before its result. Costs vary
with the model's wording from run to run.
