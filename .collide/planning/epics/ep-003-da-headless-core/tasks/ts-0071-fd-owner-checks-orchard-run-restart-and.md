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
    - Optional: restart the Mac mid-run. The phase is `lost` and the run `failed`; `--resume` shows the record and exits 7 (ts-0069-bn decision 7: a hand resume of a lost phase is Stage 4). The daemon's record shows host shutdown, not a crash.
  - **The sleep test.**
    - Start a run whose task asks the agent to run `sleep 180` in Bash, and allow the call.
    - While the call runs, start `sleep-check.sh`, sleep the Mac for about 60 s, and wake it.
    - The run's trace should show a sleep wait about as long as `sleep-check.sh` reports, and active time without it.
    - Check the 0.2 table: uptime for work timeouts, continuous time for elapsed time and the cache countdown, wall time for a limit reset.
- Area: this item's own text. No code.
- The test rule, for anything automated these checks lean on: no test touches the real home, real launchd or a real daemon. Each test clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as `host/tests/common/mod.rs` `TempRoot` does. Settings files are fixtures, never the owner's own `settings.json`. The owner's own runs here use the owner's real setup by choice.
- Stage 3b item (revision 18 scope, split 2026-11-15). Items of Stage 3b run one at a time, in id order, because they share `host` and the generated files.

### Owner checklist

Checked against the code at f4dad8a on 2026-11-17: no command, path, table or message drifted; the run now loads the
worktree's `CLAUDE.md` and skills (a note under Check 1).

Written from the code and `collide --help` of the build at the time of writing. Every command below is the owner's to
run; nothing here was run by the build agent.

**Setup, once.**

1. Build the binaries. `collide run` starts the `collide-daemon` that sits beside it, so use both from one folder.

   ```sh
   cd <collide checkout>
   cargo build -p collide-host
   export COLLIDE_BIN=<collide checkout>/target/debug
   "$COLLIDE_BIN/collide" run        # prints the usage line and exits 2; it starts nothing
   ```

2. Map the builder's alias. `collide run` uses the alias `fast` unless `--alias <name>` is given. The file is
   `~/.config/collide/models.yaml`; if it exists, add the alias under `tools.claude` and keep the rest.

   ```yaml
   tools:
     claude:
       fast: { name: haiku }
   ```

   A missing mapping is refused with exit 5, and the message prints this snippet.

3. Do not set `COLLIDE_DAEMON_ROOT`. With it set, the daemon starts without the launchd job and keeps its files in
   another folder. Without it, the files are in `~/Library/Application Support/Collide/`:

   | what | path |
   |---|---|
   | daemon log | `daemon.log` |
   | store (SQLite) | `store/store.db` |
   | run folders | `runs/<run id>/build/` (journal, `stdout.log`, `exit.json`, `shim.log`) |
   | worktrees | `workspaces/<run id>/` |

4. Write a small task file in a folder that is not inside any git repository (so no stray file shows up in Orchard's
   status). The file's text is sent to the agent as the first turn. `collide run` makes the worktree from the repository
   that holds the task file, else from the one the command is run in, so run it from `<orchard checkout>`.

   ```sh
   mkdir -p <scratch folder>
   $EDITOR <scratch folder>/<task file>.md
   ```

   For check 1 pick a task that touches one or two files and ends with a commit. For checks 2 and 3 see the task text
   given in each.

Exit codes of `collide run`: 0 the phase ended, 1 could not do its work, 2 wrong command line, 3 no such task file or
run, 5 the daemon refused (unmapped alias, and so on), 7 the run failed, 8 the run was stopped, 9 waiting for a person.

**Check 1: a real run on the Orchard repo in a fresh worktree.**

Model: haiku, through the `fast` mapping above. Budget: suggested `--budget 1.00` (US dollars; the phase stops when its
cost passes it). The owner chooses both.

Steps:

1. Note the state of Orchard before the run.

   ```sh
   git -C <orchard checkout> status --short
   git -C <orchard checkout> worktree list
   git -C <orchard checkout> branch --list 'collide/*'
   ```

2. Start the run. It starts the daemon when none answers, so the first start may take several seconds (it waits for the
   login shell's environment).

   ```sh
   cd <orchard checkout>
   "$COLLIDE_BIN/collide" run <scratch folder>/<task file>.md --budget 1.00
   ```

3. Answer each permission prompt: `y` allows that one call, `n` denies it. There is no "always".
4. When it ends, note the run id and the `worktree:` line of the record. Then repeat the three commands of step 1, and:

   ```sh
   git -C <worktree path> status --short
   git -C <worktree path> log --oneline -5
   sqlite3 -readonly "$HOME/Library/Application Support/Collide/store/store.db" \
     "SELECT run, phase, state FROM phase_ends WHERE run='<run id>';"
   ```

Look for:

- Before the run, two lines: `claude in use: <version> (tested range <from> through <through>)` and
  `claude published: latest <x>, stable <y>`. A version outside the range reads `(outside the tested range ...; its
  smoke check passed|failed ...)`.
- `run <run id> started on <task file>`, then `run <run id>: running`.
- A permission request reads `The agent asks to use <tool> (<kind>).`, with `reason:` and `input:` lines, or
  `input not available until approved`. The prompt is `Allow this once? [y/N]` and offers nothing else.
- Since `ts-0104-gx`, a run reads the worktree's own `CLAUDE.md` (and the files it imports) and its `.claude/skills`,
  as the commit the worktree is made from has them (`repo_instructions: worktree`, the default of the built-in
  builder: `host/src/adapters/claude/instructions.rs`). That adds input tokens to turn 1, and a `Skill` call asks like
  any tool: `The agent asks to use Skill (<kind>).`. If an instruction file is changed in the worktree before a later
  attempt, the record shows `warning: phase build started after instruction files changed in the worktree: <files>`
  under the `worktree:` line (`record_text` in `host/src/run_cli.rs`). With one attempt, as here, none is expected.
- The record ends with `run <run id>: ended`, `worktree: <path> (branch collide/<run id>)`, `report:` with the agent's
  words, then `model calls:` (one `turn N: wall ..., model time ..., input ..., output ..., cache read ...,
  cache write ..., cost $...` line per turn; `not reported` for a figure the agent did not give, never `0`) and a
  `total:` line. The exit code is 0.
- A fresh worktree: `git worktree list` gains one entry (the path under `workspaces/<run id>`), and the branch list
  gains `collide/<run id>`. The worktree's log shows the agent's commit on top of Orchard's `HEAD`.
- One phase: the `phase_ends` query returns exactly one row, with phase `build`.
- Nothing changed outside the worktree: `git status --short` of `<orchard checkout>` is the same as before, and no
  branch other than `collide/<run id>` was added. The worktree is left in place; nothing removes it.
- The cost in the record is under the budget.

Automated tests it relies on: `ts-0069-bn` (`collide run`: `host/tests/adapters/claude/run_command.rs`).

- Owner result:

**Check 2: a daemon restart in the middle of a run, then `collide run --resume`.**

Model and budget: haiku, `--budget 0.50` suggested. Best point to restart: while a permission request is pending, so the
pending request is also checked. Task text for this check:

> Run `echo restart-check > restart-check.txt` with the Bash tool, then `cat restart-check.txt`, then give your report.

Steps:

1. Start the run from `<orchard checkout>`, as in check 1, and when the permission prompt appears, do not answer it.

   ```sh
   cd <orchard checkout>
   "$COLLIDE_BIN/collide" run <scratch folder>/<restart task file>.md --budget 0.50
   ```

2. In a second terminal, find the daemon's pid and stop it. The daemon logs its pid at each start.

   ```sh
   grep 'starting, pid' "$HOME/Library/Application Support/Collide/daemon.log" | tail -1
   kill -TERM <daemon pid>
   ps -p <daemon pid>                        # gone
   pgrep -fl 'collide shim'                  # the run's shim is still there
   ```

   `SIGTERM` is the daemon's clean stop. Optional second round, a crash: run the check again and use
   `kill -KILL <daemon pid>` in place of `kill -TERM`.
3. Close the first terminal (or press Ctrl-C). Its command may already have ended with a `collide run:` error and
   exit 1, since its daemon is gone.
4. Come back to the run. This starts a new daemon, which adopts the shim before it answers.

   ```sh
   cd <orchard checkout>
   "$COLLIDE_BIN/collide" run --resume <run id>
   ```

5. Answer the pending permission prompt with `y`, and let the run end.
6. Check the log and the spans.

   ```sh
   tail -40 "$HOME/Library/Application Support/Collide/daemon.log"
   sqlite3 -readonly -header -column "$HOME/Library/Application Support/Collide/store/store.db" \
     "SELECT id, kind, wait, phase, round((ended_ns - started_ns) / 1e9, 1) AS secs
        FROM spans WHERE run='<run id>' ORDER BY id;"
   ```

Look for:

- `--resume` prints the two version lines, then `run <run id>: running`, and asks the same permission request again
  (the request is stored, not lost). It prints no `started` line: it is the same run.
- The run then ends as in check 1: `run <run id>: ended`, the record, exit code 0.
- In `daemon.log`, from the new daemon: `daemon <version> starting, pid <new pid>`, then one of
  `the last daemon was stopped` (SIGTERM) or `the last daemon ended with no word (a crash, or it was killed)` (SIGKILL),
  then `adopting <n> shim directories`, then `listening on ...`.
- The spans: one `run` row and one `phase` row (phase `build`), not two of each. The restart did not start the phase
  again, and the permission `wait` row is closed at the time of your answer.
- The agent's own file exists in the worktree: `cat <worktree path>/restart-check.txt` prints `restart-check`.

Optional, restart the Mac mid-run (the login session; 0.2). Start a run as above, restart the Mac with the run going,
log in again, and run `"$COLLIDE_BIN/collide" run --resume <run id>`. Look for:

- The shim and the agent are gone with the machine, so the phase is `lost` and the run is `failed`. The command prints
  the record and then `Nothing resumes this run: a phase that failed or was lost has no Queue item to answer, so start
  the task again.` It exits 7. Resuming a lost phase by session id is not built in 3b (ts-0069-bn, decision 7).
- In `daemon.log`, the line for the daemon that started after the restart says how the last one ended:
  `the last daemon ended at logout (host shutdown, not a crash)` when macOS logged out cleanly, and `the last daemon
  ended with no word (a crash, or it was killed)` when it did not. Write down which one you saw.

Automated tests it relies on:

- `ts-0066-zb`: `a_pending_request_survives_a_daemon_restart_and_can_be_answered` and
  `a_request_that_was_answered_is_not_asked_again_after_a_restart` (`host/tests/adapters/claude/pause.rs`).
- `ts-0069-bn`: `resume_reattaches_to_a_live_run_started_elsewhere_and_follows_it_to_its_end`
  (`host/tests/adapters/claude/run_command.rs`).
- `ts-0070-mk`: `a_phase_outlives_a_daemon_that_is_killed_and_the_next_daemon_adopts_it`
  (`host/tests/adapters/claude/launchd_run.rs`, opt-in with `COLLIDE_REAL_LAUNCHD=1`).

- Owner result:

**Check 3: the sleep test with `sleep-check.sh`.**

Model and budget: haiku, `--budget 0.50` suggested. Task text for this check (the Bash tool's own default timeout is
shorter than 180 s, so the task sets one):

> Run `sleep 180` with the Bash tool, with its timeout set to 300000 ms. Then say "done" as your report.

Steps:

1. Start the run from `<orchard checkout>`, as in check 1.

   ```sh
   cd <orchard checkout>
   "$COLLIDE_BIN/collide" run <scratch folder>/<sleep task file>.md --budget 0.50
   ```

2. Answer the permission prompt with `y` at once. The permission wait counts as waiting, so a quick answer keeps it
   short and apart from the sleep. The `sleep 180` call now runs.
3. In a second terminal, start the watcher (about two minutes; it gives up after 15 minutes, `MAX_S=<seconds>` changes
   that). It prints `watching`.

   ```sh
   <collide checkout>/.collide/planning/spikes/sp-0010-bn-metrics-sources/sleep-check.sh
   ```

4. About 30 s into the call, put the Mac to sleep by hand (Apple menu, Sleep). Note the time. Wait about 60 s and wake
   it.
5. Read what the watcher prints, then let the run end. Run the spans query with the run id:

   ```sh
   sqlite3 -readonly -header -column "$HOME/Library/Application Support/Collide/store/store.db" \
     "SELECT id, kind, wait, phase,
             round((ended_ns - started_ns) / 1e9, 1) AS secs,
             datetime(started_wall_ms / 1000, 'unixepoch', 'localtime') AS started,
             datetime(ended_wall_ms / 1000, 'unixepoch', 'localtime') AS ended
        FROM spans WHERE run='<run id>' ORDER BY id;"
   sqlite3 -readonly "$HOME/Library/Application Support/Collide/store/store.db" \
     "SELECT COUNT(*) FROM cold_returns WHERE run='<run id>';"
   ```

   The rows are on the continuous clock (`secs`), with wall time beside them. A span's active time is its `secs` less
   the `secs` of the `wait` spans inside it (overlaps counted once). There is no command that prints it, so subtract
   by hand.

Look for:

- The watcher prints `gap detected: slept <N> s by (continuous - uptime); uptime advanced ..., continuous ..., wall ...`
  and `kern.sleeptime to kern.waketime says <M> s`. `N` and `M` agree within about 1 to 2 s, and both match your
  stopwatch. The watcher also writes the same text to `out/sleep-check.txt` beside the script.
- The spans query has a row with `kind` `wait` and `wait` `sleep` under phase `build`, and its `secs` is about `N`.
  Its two wall times are about `N` apart.
- The `phase` row's `secs` is the whole length of the phase on the continuous clock, so it includes the sleep. The
  active time (the phase's `secs` less the sleep wait, and less the permission wait) is shorter than the phase by
  about `N`.
- The 0.2 table, as far as a run shows it: continuous time gives spans as elapsed (the phase `secs` grew by the sleep);
  wall time gives the display times (they jumped by the sleep, as the watcher's `wall advanced` says). The work-timeout
  clock (uptime) has no timeout a hand check can see in 3b, so it rests on the tests below (see Gaps).
- The sleep counts as waiting: it is a `wait` span and the active time excludes it. A sleep of about 60 s is far under
  the one-hour cache time, so `cold_returns` has no row for the run (a sleep alone is not a cold return).
- The run ends `ended` with exit code 0. The `sleep 180` call may finish later than 180 s of wall time after the wake;
  that is the call's own timing and not what is checked.

Automated tests it relies on:

- `ts-0067-kw`: `the_machine_sleeping_is_waiting_time_and_not_active_time` and
  `a_sleep_inside_a_permission_wait_is_counted_once` (`host/tests/metrics.rs`), and
  `a_machine_sleep_the_sampler_finds_is_a_sleep_wait_and_not_active_time`
  (`host/tests/adapters/claude/metrics.rs`).
- `host/tests/clock.rs` for the three clocks.

- Owner result:

**Gaps.**

- No command prints a run's spans, waits or active time. The checks read the store with `sqlite3 -readonly` on
  `store/store.db` (table `spans`, columns `kind`, `wait`, `phase`, `started_ns`, `ended_ns`, `started_wall_ms`,
  `ended_wall_ms`), and the owner subtracts by hand. The record that `collide run` prints has no span, wait or active
  time. A viewer is Stage 6 or later.
- The 0.2 table's uptime row (work timeouts) cannot be seen from a run in 3b: the uptime clock times the login
  shell capture at daemon start and the waits of a stop, and neither shows in a run's output. Only the continuous and
  wall rows are checked by hand.
- After a Mac restart, `--resume` does not start a lost phase again (ts-0069-bn, decision 7): it shows the record and
  exits 7. The Notes were corrected to say so (review).
- There is no `collide-daemon stop` or `collide daemon` command. The daemon is stopped with `kill` on the pid in
  `daemon.log`, and it is started by the next `collide run` (or by `"$COLLIDE_BIN/collide-daemon" start`, which prints
  `started` or `already running`).
- Not verifiable from the code or `--help`: `pgrep`, `ps`, `sqlite3` and `kill` behave as on any Mac; whether the
  agent keeps to the task text above (the exact `sleep 180` and its timeout); and whether the Bash tool of the
  installed `claude` accepts a 300000 ms timeout.
