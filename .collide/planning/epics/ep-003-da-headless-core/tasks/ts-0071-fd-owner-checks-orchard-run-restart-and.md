---
id: ts-0071-fd
kind: task
status: active
title: "Owner checks: Orchard run, restart and sleep test"
owners: []
links:
  - { type: blocked-by, to: ts-0070-mk }
  - { type: blocked-by, to: ts-0073-ac }
---

## Goal

Three Stage 3b checks that only the owner runs:

- a real run on the Orchard repo, in a fresh worktree;
- a restart in the middle of a run;
- the sleep test (`sleep-check.sh`).

A build agent never runs them and never touches the Orchard repo. It writes into this item, under `### Owner checklist`, the command lines and a short checklist for each check. The owner runs them and records the results here as owner judgement.

## Success criteria

- A real run on the Orchard repo in a fresh worktree (Stage 3b criterion). Owner-run.
- The owner restarts mid-run, and the run continues or resumes (Stage 3b criterion). Owner-run.
- The owner's sleep test (Stage 3b criterion; R5; 0.13). Owner-run, and recorded as owner judgement in this item:
  - `sleep-check.sh` runs on a real run that sleeps;
  - the table in 0.2 holds;
  - the sleep counts as waiting.
- For each check, the item holds:
  - the command lines, with `<orchard checkout>` and the run id as placeholders;
  - the model and budget, chosen by the owner (haiku and a small `--budget` suggested);
  - what to look at;
  - a line for the owner's result.

## Tests

None by a build agent: these checks are the owner's. The checklist names the automated tests each check relies on:

- `ts-0066-zb`: resume, and the pending request after a daemon restart;
- `ts-0067-kw`: sleep accounting;
- `ts-0069-bn`: `--resume`.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 3b: the Orchard run, restart and sleep-test criteria.
  - 0.2: the clock table; Starting the daemon (the login session).
  - 0.13: Sleep is not work.
  - Part L.
  - S8 record: `sleep-check.sh`, in `.collide/planning/spikes/sp-0010-bn-metrics-sources/`.
- What each checklist covers. The build agent fills in the exact commands once `collide run` exists.
  - **The Orchard run.**
    - Set up a `models.yaml` mapping for the builder's alias.
    - Run `collide run <task file>` against `<orchard checkout>`.
    - Look for: a fresh worktree, one phase, the record printed, nothing changed outside the worktree, and both version lines.
  - **The restart.**
    - Start a run. In the middle of it, stop the daemon process and close the terminal.
    - Run `collide run --resume <run>`. The run should have continued, through shim adoption.
    - Optional: restart the Mac mid-run. The phase is `lost`, then `--resume` starts it again by session id, and the daemon's record shows host shutdown, not a crash.
  - **The sleep test.**
    - Start a run whose task asks the agent to run `sleep 180` in Bash, and allow the call.
    - While the call runs, start `sleep-check.sh`, sleep the Mac for about 60 s, and wake it.
    - The run's trace should show a sleep wait about as long as `sleep-check.sh` reports, and active time without it.
    - Check the 0.2 table: uptime for work timeouts, continuous time for elapsed time and the cache countdown, wall time for a limit reset.
- Area: this item's own text. No code.
- The test rule, for anything automated these checks lean on: no test touches the real home, real launchd or a real daemon. Each test clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as `host/tests/common/mod.rs` `TempRoot` does. Settings files are fixtures, never the owner's own `settings.json`. The owner's own runs here use the owner's real setup by choice.
- Stage 3b item (revision 18 scope, split 2026-11-15). Items of Stage 3b run one at a time, in id order, because they share `host` and the generated files.
