# S3 process ownership: reproduction scripts

Throwaway spike code. The decision record is `record.md`.

Run everything from this directory on macOS with `/usr/bin/python3` (3.9, Command Line Tools). Scripts write
their state under `work/`, which they create. Each script prints its measurement and cleans up the processes and
containers it started. It finds them by recorded pid, by its own `COLLIDE_RUN=s3-*` marker, or by the
`collide.spike=1` container label. Nothing is signalled by name.

| Script | Measures | Prints | Needs |
|---|---|---|---|
| `m1_m2_owner_ab.py [a\|b bare\|shim]` | M1 and M2 for options (a) and (b): app quit and relaunch, owner restart, and whether the next phase starts with the app closed | Per phase: exit status and its source (wait4 or shim), CPU and max RSS, log lines and gaps, whether the app's mirrored view equals the log, and lines written while the app or owner was down. Per hand-off: the delay. Saves `work/m1_m2_owner_ab.json` | about 80 s for all four runs |
| `m3_stop_tree.py [--grace 2]` | M3: stop a hostile tree plus a Docker container | Which roles the ppid walk, the `ps -E` marker scan and the process group each found; survivors after SIGTERM, after SIGKILL, after the cleanup hook, and after a safety-net kill by recorded pid; scan costs | Docker running; pulls `alpine` if absent |
| `m3b_ps_env_visibility.py` | Which binaries `ps -E` can see the environment of | For each binary: whether it is an Apple platform binary, whether the marker scan found it, and the argc and env-string count that `KERN_PROCARGS2` returns | — |
| `m4_rusage.py` | M4: which exit totals survive | E1 direct child, E2 child that waited for a grandchild, E3 child that left an orphan, E4 reattach (wait4, kqueue `NOTE_EXITSTATUS`, `proc_pid_rusage`), E5 shim after an owner SIGKILL | Run `m1_m2_owner_ab.py` first for E5 |
| `m5_claude_bg.py [1 2 3 4]` | M5 and option (c): real `claude --bg` sessions | `claude agents --json` over time; the session's process chain and `COLLIDE_RUN`; transcript growth while the owner is down; the hand-off delay; `claude stop` on a hostile tree; `--max-budget-usd` under `--bg`; whose environment a session gets | Logged-in `claude`; a directory claude already trusts (`S3_BG_CWD`, default `~/projects`); about $0.05 |
| `m6_app_quit.py` | Do children survive when an app launched by LaunchServices exits? | Each child's survival, ppid and pgid after the app exits | — |
| `rust-reap/` | How Rust `std` and tokio reap a setsid'd child, and whether rusage survives | One line per test, T1–T5 | `cd rust-reap && CARGO_TARGET_DIR=$PWD/target ~/.cargo/bin/cargo run --offline` (needs `libc 0.2.189` and `tokio 1.53.1` in the local cargo cache; drop `--offline` to fetch them) |
| `check_clean.py` | Final survivor check | Processes with `COLLIDE_RUN=s3-*`, containers labelled `collide.spike=1`, spike `--bg` sessions still listed, and the claude daemon's state | — |

Supporting code:

- `owner.py` is the stand-in owner: a small phase engine whose state lives in `state.json`, plus a socket server
  with a token. It runs as a detached daemon (option a) or inside the app (option b). Children are spawned bare
  (the host forks and reaps them) or under the shim.
- `app.py` is the stand-in app. `client` mode talks to the daemon. `host` mode hosts the engine itself. Both mirror
  phase logs from saved byte offsets.
- `lib/s3.py` holds the shared helpers: ps table, ppid walk, marker scan, signalling scoped to pids, docker label
  queries, and `run_claude`, which runs `claude-clean.sh` with output on temp files.
- `claude-clean.sh` runs `claude` with every `CLAUDE*` variable unset.
- `fixtures/` holds the test programs:
  - `fake_agent.py` writes numbered log lines;
  - `shim.py` is the per-child parent that writes `exit.json`;
  - `tree.py` is the hostile hook;
  - `cleanup.py` is the cleanup hook;
  - `spawner.py` is the rusage parent;
  - `envdump.py` records the environment;
  - `S3Quit.app` is a minimal app bundle.
