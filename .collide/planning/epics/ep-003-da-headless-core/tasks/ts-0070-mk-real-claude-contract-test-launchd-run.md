---
id: ts-0070-mk
kind: task
status: ready
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
