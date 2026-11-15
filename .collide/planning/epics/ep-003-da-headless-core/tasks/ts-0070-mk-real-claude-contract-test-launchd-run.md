---
id: ts-0070-mk
kind: task
status: done
title: "Real claude: contract test, launchd run and dogfood"
owners: []
links:
  - { type: blocked-by, to: ts-0069-bn }
---

## Goal

Every Stage 3b step that runs the real `claude` (except the owner's own checks in `ts-0071-fd`), kept together at the end of the stage. **Each one needs the owner's OK before it runs.** Each uses haiku and a stated budget cap, and each is opt-in through an env var, never part of the default `cargo test`.

- **The contract test's live half** (`COLLIDE_REAL_CLAUDE=1`). It records each case from the real, pinned `claude`. It checks the stream shapes that the fixtures of `ts-0062-rj` to `ts-0064-hw` rely on, so format drift fails a test:
  - the launch line's flags, including `--permission-prompt-tool stdio`;
  - the permission round trip;
  - a hook event under `--include-hook-events`;
  - the `result` fields.

  It also runs `ts-0068-wx`'s smoke check against the real binary, and compares `--autocompact` with `CLAUDE_CODE_AUTO_COMPACT_WINDOW` (0.3). It is re-run on every `claude` upgrade.
- **A real `claude -p` phase under the daemon that the one-shot launchd job started** (`COLLIDE_REAL_LAUNCHD=1` and `COLLIDE_REAL_CLAUDE=1`). Carried from `ts-0058-ad`: `USER` is set, and `claude` must not fail with `api_error`.
- **The dogfood run** (part L): one of Collide's own build tasks from `.collide/planning/` goes through `collide run`, in a fresh worktree of Collide.

One more step uses fake Claude and `COLLIDE_REAL_LAUNCHD=1` only. A shim and its child survive a real SIGKILL of the launchd-started daemon, and the next daemon adopts them (carried from `ts-0058-ad`, decision 3).

## Success criteria

- One of Collide's own build tasks runs through `collide run` in a fresh worktree of Collide (Stage 3b criterion; part L). Needs the owner's OK.
- A real `claude -p` phase runs under the daemon that the one-shot launchd job started (Stage 3b criterion). Needs the owner's OK.
- The contract test passes on the pinned `claude`, and fails when a recorded shape changes. Without `COLLIDE_REAL_CLAUDE=1` it is skipped, with the reason printed. Recording it needs the owner's OK.
- The shim and its child survive a real kill of the launchd-started daemon, and are adopted. After the launchd tests, no `dev.collide` job is loaded, `~/Library/LaunchAgents` is unchanged, and no test process is left (checked as for `ts-0058-ad`).
- The item reports the spend of each real run, and the total stays under the cap.

## Tests

The real-claude contract test (opt-in in CI, `COLLIDE_REAL_CLAUDE=1`). The launchd tests run only with `COLLIDE_REAL_LAUNCHD=1`, on macOS with a GUI session. Elsewhere they are skipped, with the reason printed.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 3b: Contract test; Dogfood (part L); the Success Criteria on launchd and dogfood.
  - 0.3: A tested version range; `--permission-prompt-tool stdio` pinned by the contract test; the `--autocompact` comparison.
  - 0.2: Starting the daemon (the launchd job).
  - R5. The risk "stream-json is not a versioned contract".
- **Asking first.** Before each real run, the build agent stops and asks the owner, listing the command lines, the model and the caps. Nothing here runs in CI's default job.
- **Budget.** Haiku throughout, through a test `models.yaml` that maps the alias to haiku.
  - `--max-budget-usd 0.05` for each contract case and for the smoke check.
  - `0.10` for the launchd phase.
  - `0.50` for the dogfood phase.
  - At most $1.50 for the whole item, re-runs included.
- **The owner's login.** The `claude` child needs the owner's login, which lives under the real home.
  - Collide's own files stay in a temp root (`COLLIDE_DAEMON_ROOT`).
  - The phase runs with `config: none` (`--setting-sources ""`), so the owner's own `settings.json`, hooks, CLAUDE.md and plugins do not load. Auto memory is off.
  - The login is the only use of the real home.
  - How the child gets the real `HOME` is recorded as a decision.
- **The dogfood run.** The owner picks a small item, or approves one the agent proposes. Its worktree and branch are left for the owner to review or remove; nothing is merged or pushed.
- A build agent never touches the Orchard repo.
- Recordings made here are redacted before anything is committed: no raw session ids, paths outside the temp root, or prompt text beyond the fixture's own.
- Area: `host` (`host/tests/adapters/claude/`, and the launchd test beside `host/tests/launchd_start.rs`).
- The test rule: the default tests touch neither the real home, real launchd nor a real daemon. They clear the environment (`env_clear()`) and set `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as `host/tests/common/mod.rs` `TempRoot` does. The opt-in tests here are the only exceptions, each named above. Settings files are fixtures, never the owner's own `settings.json`.
- Stage 3b item (revision 18 scope, split 2026-11-15). Items of Stage 3b run one at a time, in id order, because they share `host` and the generated files.

### Real-binary findings (claude 2.1.325, haiku, 2026-11-15)

Recorded by the live half of the contract test; the redacted streams are in `host/tests/adapters/claude/fixtures/real-recordings/` (`real-recordings.source.md` next to it holds the redaction rule).

- **Launch line.** The adapter's own line (`FIXED_ARGV` plus the role's flags) is accepted as it stands, with `--permission-prompt-tool stdio` and `--permission-prompts host`. Exit code 0 in every case.
- **Permission round trip.** A write command (`touch allowed.txt`) gives one `control_request` (`can_use_tool`) with `tool_use_id`, the input and a `description`; allow runs the tool, deny keeps it from running and the call is refused with cause `host`. A Bash `echo` needs no permission under this config, so an "allow" case must use a write.
- **Hook events.** `hook_started` and `hook_response` carry `hook_event`, `hook_name`, `hook_id`, `exit_code`, `outcome`, `stdout`, `stderr`. They carry no call id (see decision 4).
- **Canary.** The settings canary is on the real `SessionStart` `hook_response` (stderr and `output` hold `collide-settings-canary`), on a fresh session and on `--resume`.
- **`result` fields.** `subtype`, `is_error`, `duration_ms`, `duration_api_ms`, `num_turns`, `result`, `stop_reason`, `terminal_reason`, `session_id`, `total_cost_usd`, `usage`, `modelUsage`, `permission_denials`, plus timing fields (`ttft_ms`, `time_to_request_ms`, `first_content_frame_ms`). `api_error_status` is present and null.
- **Smoke check (ts-0068-wx).** `smoke_check` on the real binary: `Passed`, 2.1 s, $0.0016. The daemon runs it only for a version outside `TESTED`, so no daemon run exercised it (2.1.325 is inside the range); the contract test calls it directly through a logging wrapper. `--no-session-persistence` is accepted (the smoke check and the free control requests use it).
- **SessionStart on `--resume`.** It fires, with `hook_name` `SessionStart:resume`, and the canary comes with it.
- **A real resume's first turn.** It gives a normal turn usage with exact quality and the right cost (the cumulative `total_cost_usd` is differenced), but `model_time_ms` is `None`: see decision 6.
- **`duration_api_ms` across `--resume`.** It continues from the first run (820 ms, then 1567 ms in one measured pair), and `total_cost_usd` continues too (0.00093, then 0.00187). The second result's figures include the first run's.
- **`missing_tool`.** `init.tools` lists `Read` and `Edit` (and `Write`, `Bash`, ...). A failing `Read` or `Edit` (file absent) is an `is_error` result with no `tool_result_meta`, which the adapter used to read as `missing_tool`; see decision 3. A genuinely missing tool cannot be provoked: the model can only call tools its list holds.
- **Hook-to-call join.** Two parallel Bash calls with a `PreToolUse` hook that exits 1: each call gets its own `HookFailedOpen` (see decision 4).
- **`background`.** A real phase ran detached under a shim, under a launchd-started daemon, and ended (see the launchd result below); a fake-Claude phase survives a real SIGKILL of the daemon and is adopted. That is the evidence the capability row waits for; see decision 8.
- **Event rate.** At most 10 lines in one second (a two-call hook turn); a plain turn peaks at 3. Far under 200/s.
- **`--autocompact` and `CLAUDE_CODE_AUTO_COMPACT_WINDOW`.** Measured with the free `get_context_usage` control request (no model call). For 200000 and 500000 the two give the same window (`maxTokens`), the flag reporting `autocompactSource` `settings` and the variable `env`; the default is 1,000,000 (`model-default`). Set together, the variable wins. The flag errors out of its range (100k to 1M) where the variable clamps or is ignored. They agree, so `--autocompact` can replace the variable (decision 5).
- **New lines.** `system/thinking_tokens` and `rate_limit_event` with status `allowed_warning` are now in the recordings; both read as `Other`.
- **`rate_limit_info.resetsAt`.** In seconds (Unix epoch), seen on an `allowed_warning` event. No limit was hit, and none was provoked, so the 30 s floor was not exercised.
- **Login.** A temp `HOME` is "not logged in"; the real `HOME` is logged in. Decision 1.

Launchd results (macOS, GUI session):

- Fake Claude, real launchd: the shim's child survived `SIGKILL` of the launchd-started daemon (`AbandonProcessGroup`), a second launchd job started the next daemon, and `collide run --resume` followed the phase to `ended` with its report.
- Real `claude -p` under a launchd-started daemon: exit 0, `USER` set, no `api_error`; report `ok`; $0.00099 against a $0.10 cap. The line the daemon built had `--setting-sources ""` (through the wrapper), `--permission-prompt-tool stdio` and `--max-budget-usd 0.1`.

Dogfood (ts-0072-sk, `collide run <file> --budget 0.50`, haiku): ended, 22 turns in the agent's loop, wall 491 s, model time 73.8 s, cost $0.018 against the $0.50 cap. Result: six doc-comment fixes in five files, one commit on the run's branch; `cargo doc --workspace --no-deps` prints no warning (checked by me on the branch), `cargo fmt --all --check` passes, and the diff holds only `//!` and `///` lines. See the answer list in Built. Two findings: tests run inside a run see Collide's own run variables (decision 9), and the worktree grew to 6.7 GB because the agent built and tested in it.

Files `claude` wrote under the real home (left in place; none deleted):

- `~/.claude/projects/-private-tmp-cdt-*-work/*.jsonl` and `~/.claude/projects/-private-tmp-cdt-*-data-workspaces-run-*/*.jsonl`: session transcripts of the contract cases and the launchd phases (22 folders, each with the prompt text of its case);
- `~/.claude/projects/-private-tmp-ts70d-*-data-workspaces-run-*/*.jsonl`: the dogfood's transcript;
- `~/.claude.json` (updated on every run) and `~/.claude/backups/.claude.json.backup.*` (five, made by `claude`);
- `~/.claude/cache/model-catalog/*.json`;
- `~/.cargo/.global-cache` (touched by `cargo` inside the dogfood).

Nothing under `~/.collide` or `~/Library/Application Support/Collide`.

Proposed plan text:

- Stage 3b, contract test: "The live half is `host/tests/adapters/claude/real_claude.rs` (`COLLIDE_REAL_CLAUDE=1`, haiku, `--max-budget-usd 0.05` a case, a spend ledger in `COLLIDE_REAL_LEDGER` that refuses a run that could pass $1.50). It checks every live line against the shapes of the redacted recordings in `fixtures/real-recordings/` (the paths all recorded lines of a kind had, and their JSON types): a lost path or a new type fails. It covers the launch line, the permission round trip, hook events and their join to calls, the canary, `--resume`, a failing tool, the smoke check and `--autocompact`."
- 0.3, tested version range: "Recorded against 2.1.325: a hook line carries no call id, so a failing `PreToolUse` hook is joined to the latest call of its tool; a failing tool the `init` lists is not `missing_tool`."
- Stage 3b, launchd: "`COLLIDE_REAL_LAUNCHD=1` runs a fake-Claude phase through a SIGKILL of the launchd-started daemon, and with `COLLIDE_REAL_CLAUDE=1` a real `claude -p` phase under it (`host/tests/adapters/claude/launchd_run.rs`)."

### Built

- **Live harness** (`host/tests/adapters/claude/real.rs`): runs the real, pinned `claude` on the adapter's own launch line (`ClaudeAdapter.launch`) with `env_clear()` and an environment of `PATH`, `LANG`, `HOME`, `USER` and `DISABLE_AUTOUPDATER`; config none; haiku; a cap on every line; a snapshot from `snapshot_files`; and the Claude session driving the permission answers and turns. The spend ledger (`COLLIDE_REAL_LEDGER`) refuses a run when spent plus its cap would pass $1.50. `Redactor` is the redaction rule (its doc comment is the rule, the guard refuses a line that still names `/Users/`, `/private/`, `/var/folders`, `/tmp/`, a key, the user name or the home). `Shapes` is the shape check. `context_usage` is the free `get_context_usage` request.
- **Contract test** (`host/tests/adapters/claude/real_claude.rs`): seven live tests (a plain turn, the permission round trip, a failing Read and Edit, a failed pre-hook joined to its own call, `--resume`, `--autocompact` against the variable, the smoke check through a logging wrapper) and ten offline ones (the redaction rule, the recordings holding nothing that names the machine or account, replay of the recordings through `ClaudeSession`, shape drift on a made-up line, the skip reason). Live tests are skipped with the reason printed unless `COLLIDE_REAL_CLAUDE=1`.
- **Recordings** (`host/tests/adapters/claude/fixtures/real-recordings/`, with `real-recordings.source.md`): seven redacted streams. They are in their own folder so the existing recording count test (`every_recording_is_valid_json_per_line_and_says_something`, 22) is untouched.
- **Launchd tests** (`host/tests/adapters/claude/launchd_run.rs`, `COLLIDE_REAL_LAUNCHD=1`; the second also `COLLIDE_REAL_CLAUDE=1`): the fake-Claude kill-and-adopt test and the real `claude -p` phase under the launchd-started daemon, behind a wrapper `claude` script in the test's temp root. `run_process.rs` gained `pub(super)` on `Proc` and its helpers for them; no test changed.
- **Two adapter fixes found by the real binary** (`host/src/adapters/claude/session.rs`): the `init.tools` check for `missing_tool` and the `PreToolUse` hook-to-call join (decisions 3 and 4), with three offline tests in `hooks.rs` and an edit of the `disallowed-tool.jsonl` fixture's `init` tools.
- **Dogfood** of ts-0072-sk, left in place for review:
  - worktree: `/tmp/ts70d.K2fM1z/data/workspaces/run-1794781495163-0`; branch `collide/run-1794781495163-0` (a branch of the Collide repo, one commit, `f136b6f`, on top of `5c510bc`); temp root `/tmp/ts70d.K2fM1z` (holds the daemon files, the wrapper and its logs). The daemon I started for it is stopped.
  - diff: `core/src/owner/task.rs`, `core/src/replay.rs` (two fixes), `core/src/yaml.rs`, `host/src/launch.rs`, `host/src/run_cli.rs`; 7 insertions, 8 deletions, all `//!` and `///` lines. The `task.rs` module doc now has one long line (the link target added to `PreviewResult`).
  - **Permission requests and my answers (17):**
    1. Bash `cargo doc --workspace --no-deps | grep | head`: allowed.
    2. Edit `core/src/owner/task.rs` (in the worktree): allowed.
    3. Edit `core/src/replay.rs`, redundant link target: allowed.
    4. Edit `core/src/replay.rs`, link to `crate::testing`: allowed.
    5. Edit `core/src/yaml.rs`: allowed.
    6. Edit `host/src/launch.rs`: allowed.
    7. Edit `host/src/run_cli.rs`: allowed.
    8. Bash `cargo doc` twice, `cargo fmt --all --check`, `git diff` with filters: allowed.
    9. Bash `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace`: allowed.
    10. Bash `cargo test -p collide-host --test builtin_packs | grep`: allowed.
    11. Bash `cargo test --workspace --no-fail-fast > /tmp/ts72-test.log ...`: **denied** (a write outside the worktree).
    12. Bash the same run with `tee >(grep | awk)` and no file: allowed.
    13. Bash `git add` (five doc files) and `git commit`: allowed (on the run's branch; no push).
    14. Bash `git checkout 5c510bc && cargo test ... && git checkout <branch>`: **denied** (`git checkout` is not on the allowed git list).
    15. Bash `git rev-parse`, `git log --oneline -1`, `git status --short`, `command -v claude`: allowed (read-only; `rev-parse` and `log` are not on my list but only read).
    16. Bash `cargo test --workspace --no-fail-fast | grep | sed | sort`: allowed.
    17. Bash `env | grep -i collide | sed (values masked)` and a `grep` in the worktree: allowed (read-only; names only).
  - After each denial the agent chose another command inside the rules (request 12 after 11, 15 after 14), and every path it wrote was in the worktree.

### Decisions for the owner

1. **The `claude` child's login.** Chosen: the child gets the real `HOME` and `USER` and nothing else of the owner's environment, in a wrapper `claude` script (for `collide run` phases) or `env_clear()` plus `child_env()` (for the contract cases); Collide's own files stay in the temp root. Measured: a temp `HOME` is "not logged in". Alternative: a temp `HOME` with a copied credential file (copies a secret into /tmp; refused here). Recommendation: keep. The cost is that `claude` writes transcripts and state under the real home (listed above).
2. **Config none for a `collide run` phase.** Chosen: a wrapper script rewrites the line's `--setting-sources user` to `""`, because the built-in `collide/builder` role has `config` unset. Alternative: a `config: none` key on the role or a `collide run --config none` option, so no wrapper is needed. Recommendation: add the option in a later item; it is the way to run Collide's own tasks without the owner's user settings.
3. **A failing tool the `init` lists is not `missing_tool`.** Chosen: `ClaudeSession` keeps `init.tools`; an `is_error` result with no metadata is `missing_tool` only when the tool is not listed (it had been read as `missing_tool` for every failing `Read` or `Edit`). The `disallowed-tool.jsonl` fixture's `init` now lists the tools, so it still shows a tool that is not there. Alternative: drop the cause and report `unknown`. Recommendation: keep.
4. **A `PreToolUse` hook line has no call id; join by order.** Chosen: a failed `PreToolUse` hook joins the latest call of its tool; `PostToolUse` stays joined by name. The real two-call test gives each call its own `HookFailedOpen`. Alternative: join only when one call of the tool is open, and leave the rest unjoined. Recommendation: keep; the live test fails the day a hook line gains a call id, so the join can then go.
5. **`CLAUDE_CODE_AUTO_COMPACT_WINDOW` against `--autocompact`.** Chosen: they agree (200000 and 500000), so nothing changes now; the variable stays. Alternative: switch `compact_at` to `--autocompact`. Recommendation: switch later: the flag is on the visible line and errors out of range (100k to 1M) instead of silently clamping; keep the contract test's comparison.
6. **Seed the resume anchors.** After `--resume`, `total_cost_usd` and `duration_api_ms` continue from the first run, so the first turn has cost (differenced) but no `model_time_ms`. Chosen: nothing changed; the live test records the fact. Alternative: seed the session's anchors from the first run's last result, so the first turn after a resume has model time. Recommendation: do it with the engine's resume (Stage 4); the resumed turn's model time is "not reported" until then.
7. **New stream lines.** `system/thinking_tokens` and `rate_limit_event` (`allowed_warning`) are `Other` events. Chosen: keep as `Other`. Alternative: map `allowed_warning` to a usage-pressure signal for the Queue. Recommendation: keep until a use for it exists; the account was at about 92% weekly use during this item.
8. **`background` capability.** Chosen: left at `Unknown` (the generated capability table and its refusal tests pin it). The evidence it waited for is in (a real phase under a shim under a launchd daemon, and survival and adoption after a SIGKILL). Alternative: set `Support::Yes` now. Recommendation: set it to `Yes` in the next item that touches the table; it changes the capability schema's golden output.
9. **Tests inside a run.** In the dogfood the agent's own `cargo test --workspace` gave 2143 passed and 183 failed: about 85 for `node` not being on the daemon's `PATH`, the rest because the daemon refused runs ("may not start a run": the agent's environment carries `COLLIDE_RUN`, `COLLIDE_RUN_START` and `CLAUDE_CODE_AGENT`). Chosen: nothing changed (out of scope; the task's `cargo test` criterion was checked by me outside the run). Alternative: tests that start runs clear those variables. Recommendation: do it, so a Collide run can build Collide.
10. **The ledger and its variable.** Chosen: `COLLIDE_REAL_LEDGER` (a file; no file, no total) with a $1.50 limit in code. Alternative: a limit variable. Recommendation: keep; the limit is the item's, and a re-run adds to the same file.
11. **The npm registry fixture stays hand-written.** Chosen: `registry-dist-tags.json` is the shape the plan recorded; no network was used, and the owner's approval did not include the registry. Alternative: record it once from the registry. Recommendation: leave it unless the chip breaks.
12. **Dogfood left in place.** Chosen: worktree, branch and temp root stay (the worktree is 6.7 GB with its `target/`). Alternative: remove the worktree. Recommendation: review the one commit, then `git worktree remove --force` the path and delete the branch; remove `/tmp/ts70d.K2fM1z`.

### Spend

Every run that could cost money. Caps are `--max-budget-usd`. "Cost" is `total_cost_usd` as the run reported it (the free control requests make no model call and cost 0). The first block is what the harness's ledger recorded (`COLLIDE_REAL_LEDGER`); rows repeat when a case ran again.

| # | Run | Cap (USD) | Cost (USD) | Running total (USD) |
|---|-----|-----------|------------|---------------------|
| 1 | hello | 0.05 | 0.002707 | 0.002707 |
| 2 | hello | 0.05 | 0.000930 | 0.003638 |
| 3 | hooks | 0.05 | 0.001329 | 0.004966 |
| 4 | tools-missing | 0.05 | 0.001622 | 0.006588 |
| 5 | permission-allow | 0.05 | 0.001234 | 0.007822 |
| 6 | permission-deny | 0.05 | 0.001248 | 0.009071 |
| 7 | resume-first | 0.05 | 0.001022 | 0.010093 |
| 8 | resume-second | 0.05 | 0.002070 | 0.012163 |
| 9 | hooks | 0.05 | 0.001188 | 0.013350 |
| 10 | tools-missing | 0.05 | 0.001561 | 0.014911 |
| 11 | permission-allow | 0.05 | 0.001260 | 0.016172 |
| 12 | permission-deny | 0.05 | 0.001155 | 0.017327 |
| 13 | hello | 0.05 | 0.000930 | 0.018257 |
| 14 | resume-first | 0.05 | 0.000931 | 0.019188 |
| 15 | resume-second | 0.05 | 0.001875 | 0.021063 |
| 16 | context-usage | 0.05 | 0.000000 | 0.021063 |
| 17 | context-usage | 0.05 | 0.000000 | 0.021063 |
| 18 | context-usage | 0.05 | 0.000000 | 0.021063 |
| 19 | context-usage | 0.05 | 0.000000 | 0.021063 |
| 20 | context-usage | 0.05 | 0.000000 | 0.021063 |
| 21 | smoke-check | 0.05 | 0.001597 | 0.022660 |
| 22 | hooks | 0.05 | 0.001188 | 0.023848 |
| 23 | tools-missing | 0.05 | 0.001851 | 0.025699 |
| 24 | permission-allow | 0.05 | 0.001163 | 0.026862 |
| 25 | permission-deny | 0.05 | 0.001154 | 0.028016 |
| 26 | hello | 0.05 | 0.000930 | 0.028946 |
| 27 | resume-first | 0.05 | 0.000931 | 0.029877 |
| 28 | resume-second | 0.05 | 0.001875 | 0.031752 |
| 29 | context-usage | 0.05 | 0.000000 | 0.031752 |
| 30 | context-usage | 0.05 | 0.000000 | 0.031752 |
| 31 | context-usage | 0.05 | 0.000000 | 0.031752 |
| 32 | context-usage | 0.05 | 0.000000 | 0.031752 |
| 33 | context-usage | 0.05 | 0.000000 | 0.031752 |
| 34 | smoke-check | 0.05 | 0.001597 | 0.033349 |
| 35 | launchd-smoke | 0.05 | 0.000000 | 0.033349 |
| 36 | launchd-phase | 0.1 | 0.000985 | 0.034334 |
| 37 | launchd-phase | 0.1 | 0.000985 | 0.035319 |
| 38 | dogfood | 0.50 | 0.017985 | 0.053303 |
| 39 | hooks | 0.05 | 0.001227 | 0.054530 |
| 40 | tools-missing | 0.05 | 0.001739 | 0.056269 |
| 41 | permission-allow | 0.05 | 0.001166 | 0.057435 |
| 42 | permission-deny | 0.05 | 0.001155 | 0.058590 |
| 43 | hello | 0.05 | 0.000930 | 0.059520 |
| 44 | resume-first | 0.05 | 0.000930 | 0.060451 |
| 45 | resume-second | 0.05 | 0.001874 | 0.062325 |
| 46 | context-usage | 0.05 | 0.000000 | 0.062325 |
| 47 | context-usage | 0.05 | 0.000000 | 0.062325 |
| 48 | context-usage | 0.05 | 0.000000 | 0.062325 |
| 49 | context-usage | 0.05 | 0.000000 | 0.062325 |
| 50 | context-usage | 0.05 | 0.000000 | 0.062325 |
| 51 | smoke-check | 0.05 | 0.001597 | 0.063922 |

Total: $0.0639 of the $1.50 limit (re-runs included). Rows 38 (dogfood: cap $0.50, one phase of 22 turns) and 36 and 37 (the launchd phase, run twice: cap $0.10 each) are the three that are not contract cases; row 35 is the launchd test's smoke check, which did not run because 2.1.325 is inside the tested range. Rows 1 to 8 are the probe runs made while the harness was written (the first `hello` was a probe that was redacted wrongly and re-run). Each cap was within the item's caps; no run came near its cap.

- Owner decisions (2026-11-15): the twelve recommendations are taken. Plan text applied to 0.3 (the tested range's 2.1.325 findings) and Stage 3b (the contract test's live half and the launchd tests). Spend $0.0639 of the $1.50 cap. The dogfood branch `collide/run-1794781495163-0` (commit f136b6f, ts-0072-sk) is left for the owner, who decides whether it lands. `claude` wrote its own files under the real home (transcripts in `~/.claude/projects`, `~/.claude.json` and its backups, a model cache): reported to the owner, not removed. Follow-ups: `ts-0074-cu` makes Collide's tests pass inside a Collide run (they inherit `COLLIDE_RUN`, `COLLIDE_RUN_START` and `CLAUDE_CODE_AGENT`) (decision 9); `ts-0071-fd` checks that a real daemon's phase has `node` on its `PATH` (the dogfood's daemon, started by a build agent, did not); a role or `collide run` option for `config: none` comes with pack roles in Stage 4a (decision 2); resume anchors are seeded with the Stage 4 engine's resume (decision 6); the capability table's `background` row becomes Yes in the next item that touches it (decision 8).

### Not done, by design

- A usage limit was not triggered, so `resetsAt`'s unit is known (seconds, from a warning event) but the 30 s floor and a `rejected` event are not exercised.
- No network beyond `claude`'s own: the npm registry fixture is hand-written (decision 11); the published-version chip was not run against the real registry.
- A genuinely missing tool (`missing_tool` for a tool not in `init.tools`) was not provoked: the model cannot call a tool it does not have. The rule is covered by offline tests.
- `background` stays `Unknown` (decision 8); the resume anchors are not seeded (decision 6); `--autocompact` does not replace the variable (decision 5).
- The smoke check did not run inside the daemon (version in range); it ran against the real binary directly.
- The dogfood's `cargo test` criterion was not met inside the run (decision 9); nothing was merged, pushed or removed.
- `ts-0071-fd`'s owner-run checks.
- `claude`'s files under the real home are left (listed in Notes); none were deleted.
