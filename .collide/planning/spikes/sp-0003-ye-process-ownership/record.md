---
id: sp-0003-ye
alias: S3
kind: spike
status: concluded
title: "S3: process ownership"
owners: []
links: []
---
# S3: process ownership

**Date:** 2026-11-13. **Status:** proposal; it does not edit `IMPLEMENTATION_PLAN.md` (revision 9).
**Spike code:** `.collide/planning/spikes/sp-0003-ye-process-ownership/` (its README lists the scripts and what each one prints). Raw outputs
were saved under that directory's `work/`.

**Versions:**

- macOS 26.3.1 (25D771280a), arm64;
- `/usr/bin/python3` 3.9.6 (Command Line Tools);
- Docker 29.9.2, context `desktop-linux`;
- `claude` 2.1.323 (claude.ai login);
- node v24.18.0;
- rustc 1.98.0, cargo 1.98.0, tokio 1.53.1, libc 0.2.189.

**Labels:**

- **[M: name]** means measured, and names the script and observation.
- **[I]** means inferred from measurements or documentation.
- **[U]** means unmeasured.

## Decision

**Recommended owner: (a), a `collide-daemon` that owns detached children, with every child started under a
per-child shim.** The shim is a small parent process, for example a `collide shim` mode of the same binary. It:

- calls setsid;
- starts the child with stdout and stderr going to log files;
- reaps the child with `wait4`;
- writes `exit.json` atomically, with exit status, signal, CPU time and max RSS.

The daemon reattaches by pid plus start time and reads exit totals from `exit.json`, so a daemon restart loses
nothing.

**Agents should run as `claude -p` children under the shim, not as `claude --bg` sessions.** S1 decides the exact
launch line.

**Do runs continue between phases with the app closed?** Yes, while the daemon lives:

- The next phase started 0.25 s and 0.37 s after the previous one ended, with the app closed
  [M: m1_m2 a/bare, a/shim].
- When the daemon itself dies, children keep running and logs keep being written. No phase advances until the
  daemon restarts. The restarted daemon reattached and advanced in 0.18–0.34 s [M: m1_m2, daemon SIGKILL and
  restart 2 s later].
- Who restarts a crashed daemon (a launchd agent, or the app and CLI on demand) is [U].

**Why not (b) or (c):**

- (b) fails measurement 2. Runs pause until the app returns: the next phase started 4.62 s and 4.84 s after the
  previous phase ended, which is exactly how long the app stayed closed [M: m1_m2 b/bare, b/shim]. Everything else
  in (b) matches (a), given the shim.
- (c) passes measurement 2, but it breaks several things the plan relies on:
  - stream-json output;
  - budget caps;
  - the `COLLIDE_RUN` and `COLLIDE_SPAN` markers;
  - environment stripping;
  - exit totals;
  - stopping.

  The per-measurement table and M5 below give the evidence.

## Options against the measurements

M1 to M5 are the brief's measurements.

**(a) daemon, bare spawn** (the daemon forks and reaps):

- **M1, app quit and relaunch: passes.**
  - 48 log lines were written while the app was closed.
  - There were 0 gaps, and the app's view equalled the log [M: m1_m2 a/bare].
- **M1, owner restart: logs pass, but the exit is lost.**
  - 20 lines were written while the daemon was dead.
  - On reattach, the pid's start time matched.
  - The phase that ended while the daemon was down has no exit status and no rusage: "pid vanished; not our child
    (wait4 → No child processes)" [M].
- **M2, next phase with the app closed: passes.** The hand-off took 0.25 s [M].
- **M3, stop the tree:** the stop code is the same for (a) and (b); see M3.
- **M4, exit totals:** exact only while the daemon is the parent. They are lost after a reattach [M].

**(a) daemon + shim:**

- **M1, app quit and relaunch: passes.** 50 lines were written while the app was closed, with 0 gaps [M: m1_m2
  a/shim].
- **M1, owner restart: passes in full.** The phase that ended while the daemon was dead reported exit 0, CPU 0.379 s
  and 61.0 MB from `exit.json` [M].
- **M2: passes.** The hand-off took 0.37 s [M].
- **M3:** same stop code as (b).
- **M4: totals survive an owner SIGKILL** [M: m1_m2 a/shim, m4 E5].

**(b) app owner, bare spawn:**

- **M1:** for (b), quitting the app is also the owner restart.
  - Logs pass: 50 lines were written while the app was down, with 0 gaps.
  - Phase 1's exit status was lost [M: m1_m2 b/bare].
- **M2: fails, so runs visibly pause.** Phase 2 started 4.62 s after phase 1 ended, when the app came back [M].
- **M3:** same stop code as (a).
- **M4:** lost for any phase that ends while the app is closed [M].

**(b) app owner + shim:**

- **M1:** logs pass, and the exit is kept (exit 3, CPU 0.598 s) [M: m1_m2 b/shim].
- **M2: fails, so runs pause.** The hand-off took 4.84 s [M].
- **M3:** same stop code as (a).
- **M4: totals survive** [M].

**(c) `claude --bg` agents, owned by (a):**

- **M1: passes.**
  - The session runs under claude's own daemon.
  - Its transcript grew from 289,637 to 441,058 bytes while the Collide daemon was dead.
  - The restarted daemon reattached by bg id [M: m5 part 1].
- **M2: passes.** The hand-off took 0.75 s, and the app was never open [M: m5 part 1].
- **M3: `claude stop` left all 9 roles and the container alive.**
  - Collide's own scans were still needed.
  - Its marker matched 7 claude processes outside the tree [M: m5 part 2].
- **M4: none.** The session is not Collide's child. kqueue saw exit code 143, which is the SIGTERM sent by
  `claude stop` [M: m5 part 1].

**(c) owned by (b):** same as (b) for M2, so runs pause. The owner must be alive to poll `claude agents` [I].

**M3 is the same for every option, and every option fails it on one role.**

- The union of the ppid walk, the marker scan and the process group found 7 of the 9 roles.
- The cleanup hook reached `clean_env_orphan` through the pid it recorded.
- No discovery method reached `platform_orphan`, a double-forked `/bin/sleep`. It survived until the spike's
  safety net killed it by recorded pid [M: m3, 3 runs].
- The record of zero survivors below depends on that safety net.

## M1 and M2 detail (fake agents)

**Setup:**

- Phase p1: 6 s of output, 0.5 s of CPU burn, 100 MB allocated, exit 3.
- Phase p2: 6 s, 0.3 s of CPU, 50 MB.
- Phase p3: 2 s.
- Children write `seq=N t=T` lines to log files and never to pipes.
- The app mirrors each log from saved byte offsets.

**Option (a):**

- The app is SIGKILLed at 1.5 s and relaunched after p2 starts.
- The daemon is SIGKILLed during p2 and restarted 2 s later.

**Option (b):** the app is SIGKILLed during p1 and relaunched 4 s after p1 ends.

**Results:**

- **Log continuity** [M: m1_m2, all four runs]:
  - The logs had 0 sequence gaps.
  - The app's mirrored view equalled the log byte for byte.
  - No marked process survived.
- **Reattach** [M]:
  - The new owner checks that the pid is alive and that its `ps lstart` matches the recorded start time.
  - The check matched for (a).
  - For (b) the pid was already gone, so the shim's `exit.json` was the only source.
- **Socket:** the longest AF_UNIX path that binds is 103 bytes [M: `sun_path_limit()` in m1_m2]. The stand-in avoids
  the limit by calling `chdir` to the state directory and binding the relative name `owner.sock`.
- **App exit** [M: m6]: an app launched by LaunchServices has ppid 1 and its own process group. When it exited
  normally, all three of its children survived:
  - a plain child in the app's group;
  - a setsid'd child;
  - a double-forked child.

  The test app was a minimal shell-script `.app`, not Tauri. Tauri's `plugin-shell` cleanup on exit and Cmd-Q
  through AppKit are [U].

## M3 detail: stopping a hostile tree

`fixtures/tree.py` acts as a hook running in its own session. It starts these roles, each with `COLLIDE_RUN=<id>`
unless noted:

- a plain child;
- a setsid'd grandchild under a living parent;
- a double-forked orphan that ignores SIGTERM;
- a double-forked orphan that deletes `COLLIDE_RUN` in-process;
- a double-forked `/bin/sleep`;
- a child and a double-forked orphan, each exec'd with only `PATH` and `HOME`;
- a container started with `docker run -d --label collide.spike=1 --label collide.run=<id> alpine sleep 600`.

The hook records the cleaned-environment orphan's pid and the container in `lane.json`, the way Orchard records its
servers. The stop follows 0.5:

1. Discover, using the walk, the marker scan and the process group.
2. Send SIGTERM to the group and to every pid found, then wait 2 s.
3. Rescan, then SIGKILL whatever is left.
4. Run the cleanup hook.
5. Check with `ps` and `docker ps`.

Each result below is [M: m3, 3 runs, consistent; about 600 processes on the machine].

| Role | ppid walk | marker scan (`ps -E`) | process group | Reached by |
|---|---|---|---|---|
| `hook`, `child`, `setsid_parent` | yes | yes | yes | stop |
| `setsid_grandchild` | yes | yes | **no** | stop |
| `orphan` (ignores SIGTERM) | no | yes | no | stop; needed the SIGKILL after the rescan |
| `unsetenv_orphan` | no | **yes** | no | stop |
| `clean_env_child` | yes, while its parent lives | no | yes | stop |
| `clean_env_orphan` | no | no | no | cleanup hook only, through the pid in `lane.json` |
| `platform_orphan` (`/bin/sleep`) | no | **no** | no | **nothing**; the spike's safety net killed it |
| container | — | — | — | cleanup hook (`docker rm -f` took 0.17–0.37 s) |

**Timings:** with a 2 s grace, the stop took 2.17–2.51 s. A full ps table took 49–91 ms, and a marker scan took
61–289 ms [M: m3].

**Why `/bin/sleep` escapes** [M: m3b]:

- For Apple platform binaries, `KERN_PROCARGS2` (the sysctl that `ps -E` reads) returns argv and no environment:
  `env_strings=0`.
- These binaries were invisible to the marker scan:
  - `/bin/sleep`, `/bin/sh`, `/bin/bash`, `/bin/zsh`;
  - `/usr/bin/tail`, `/usr/bin/caffeinate`, `/usr/bin/perl`;
  - `/usr/bin/env sleep`.
- Visible were Command Line Tools Python (87 env strings) and Homebrew node (84).
- R3 measured node and claude, which is why this was not seen earlier.
- [I] A backgrounded shell loop in a hook (`while …; done &`) or a `nohup sleep` becomes an orphan the marker scan
  cannot see.

**Clearing the environment:**

- A cleaned environment escapes the marker scan, as the plan's Risks section says [M: m3, `clean_env_*`].
- Deleting the variable in-process does not escape it, because the scan reads the environment from exec time
  [M: m3 `unsetenv_orphan`; m3b `os.environ.pop`].

**Telling our orphans apart:** listing every process with ppid 1, the same uid, a start after the run, and an
unreadable environment found 5–8 suspects per run, only one of them ours [M: m3]. That is too noisy to kill on, so
use it for reporting only.

## M4 detail: exit-time totals

| Case | What the owner got | Ground truth |
|---|---|---|
| E1: direct child, reaped with `wait4` | CPU 1.112 s, 211.0 MB | the child's own report: 1.101 s, 211.0 MB [M: m4] |
| E2: the child waited for a heavy grandchild | CPU 1.151 s, which **includes** the grandchild's 1.073 s; max RSS 303.6 MB is the grandchild's, the largest single process and not a sum | [M: m4] |
| E3: the child double-forked a heavy orphan | CPU 0.082 s, 11.1 MB; the orphan's 1.067 s and 303.4 MB are not counted anywhere | [M: m4] |
| E4: reattach, where the owner is not the parent | `wait4` returns ECHILD. A kqueue `EVFILT_PROC` with `NOTE_EXIT \| NOTE_EXITSTATUS` registered on a non-child **works**: data 1792 decodes to exit 7. `proc_pid_rusage` on the live non-child: the last sample was 1.096 s of CPU, and peak footprint was 157 MB against a max RSS of 161 MB. After exit it returns nothing. | the child's own report: 1.096 s [M: m4] |
| E5: shim, after an owner SIGKILL | exit 0, CPU 0.379 s, 61.0 MB from `exit.json` | [M: m1_m2 a/shim] |
| `claude --bg` | nothing; the only exit code is 143, from `claude stop`'s SIGTERM, seen by kqueue | [M: m5 part 1] |

`proc_pid_rusage` reports CPU time in **mach absolute-time units, not nanoseconds**. On this machine the timebase is
125/3. Read as nanoseconds, the E4 sample showed 0.026 s instead of 1.096 s [M: m4].

**Rust behaviour** [M: rust-reap]:

- **T1:** `std::process::Command` with `pre_exec(setsid)` gives `getsid(child) == child`. `libc::wait4` returns
  exit 3, CPU 0.581 s and 111.2 MB.
- **T2:** std's `Child::wait()` returns an `ExitStatus` with no rusage, and it consumes the zombie, so a later
  `wait4` gets ECHILD.
- **T3:** tokio's `Child::wait()` behaves the same as T2.
- **T4:** a dropped tokio `Child` is reaped by tokio's orphan reaper on a later tokio wait, so even the exit status is
  lost.
- **T5:** keeping the tokio `Child` and calling `wait4` first works (exit 3, CPU 0.559 s), after which tokio's
  `try_wait()` gets ECHILD.

## M5 detail: option (c), `claude --bg`

**Spend:** every call went through `claude-clean.sh` (no `CLAUDE*` variables) with `--model haiku`. The run used 10
sessions and spent **$0.0524** in total, per each transcript's `cost-state.totalCostUSD`, on claude.ai login.

**Launching:**

- `--bg` cannot be combined with `-p`; it fails with "--bg and --print conflict" [M].
- A launch returns in about 1.4 s and prints `backgrounded · <id>` [M].

**Process tree:**

- The first `--bg` starts a transient `claude daemon run` with ppid 1 and a prewarmed `bg-spare` [M: m5 part 1].
- The session process is usually that spare: its command line says `claude bg-spare` with no session id.
- Its chain of parents is spare → `bg-pty-host` → claude daemon → launchd.
- So sessions run with no Collide parent [M].
- The claude daemon exits after being idle for 5 s with no clients, and logs `bg adopt` when it starts
  [M: `~/.claude/daemon.log`].

**What `claude agents --json --all` reports:**

- Always: `id`, `cwd`, `kind`, `startedAt`, `sessionId`, `name`, and `state` (`working`, `done`, `blocked`,
  `stopped`).
- Only while the session process lives: `pid` and `status` (`busy` or `idle`).
- Each poll took 0.16–0.20 s [M].

**After a session finishes:**

- A finished (`done`) session's process **stays alive and idle until `claude stop`** [M].
- `stop` takes about 0.7 s. Afterwards the row keeps saying `done` and loses its pid, and `claude logs <id>` returns
  rc 1 with no output [M].

**Transcript:**

- `~/.claude/projects/<slug>/<sessionId>.jsonl` can be read while the session runs [M].
- Assistant records carry usage tokens [M].
- `cost-state` records carry `totalCostUSD` and `modelUsage` [M].
- There is no stream-json file [M: `--bg` conflicts with `--print`].

**Budget:**

- `--max-budget-usd 0.0001` is **not enforced** under `--bg`. The session made 3 tool calls, replied, and spent
  $0.0073 [M: m5 part 3].
- `claude --help` says the flag "only works with --print".

**Environment:**

- A session's environment is the claude daemon's, not the launcher's.
- Sessions A and B, launched with different markers, both carried the marker of whichever launcher had started the
  daemon. So did B's Bash-tool child [M: m5 part 4].
- [I] It follows that:
  - `COLLIDE_RUN` and `COLLIDE_SPAN` are wrong for every session except the one whose launcher started the daemon;
  - Stage 3's "a shell's `CLAUDECODE=1` never reaches the phase" cannot be guaranteed.

**Stopping:**

- `claude stop` killed the session process but left the tool's tree alive, all 9 roles plus the container
  [M: m5 part 2].
- The marker of the run whose launcher started the daemon also matched 7 shared claude processes: the daemon,
  pty-hosts and spares [M].
- [I] A marker-based stop of that run would kill claude infrastructure serving other sessions.

**Workspace trust:** `--bg` refused with "Workspace not trusted. Run claude in <dir> once…" in new subdirectories,
and it worked only in a directory already trusted [M].

- [I] Fresh worktrees would need a trust step.
- `claude --help` says `-p` skips the trust dialog. That is documented, and [U] in this spike.

## Proposed plan changes

Each item names the plan section it affects. These are proposals; the plan is unchanged.

1. **0.2 Process model: owner.**
   - Replace "S3 decides" with option (a) plus a per-child shim.
   - The daemon reattaches by pid plus start time and reads exit totals from the shim's `exit.json`.
   - If the shim itself is gone and `exit.json` is missing, the fallback is a kqueue
     `NOTE_EXIT | NOTE_EXITSTATUS` registration made at reattach. It works on a non-child and gives the exit code,
     but no rusage. [M: m4 E4]
   - The shim being killed is itself [U].
2. **0.2 Process model: "needs a live owner between phases".**
   - Add: runs continue with the app closed while the daemon lives [M].
   - Add: if the daemon dies, children and logs continue, phases do not advance, and a restart reattaches and
     advances [M].
   - Decide in Stage 3 who restarts the daemon (a launchd agent, or the app and CLI starting it on demand) [U].
3. **0.2 Process model: (c).**
   - Drop `claude --bg` as the agent mechanism and run agents as `claude -p` children under the shim.
   - The reasons, each from M5: no stream-json; `--max-budget-usd` not enforced; the environment comes from claude's
     daemon (wrong markers, and the marker can match shared claude processes); `stop` leaves the tool's tree;
     finished sessions idle until stopped; the trust gate; no exit totals.
   - Keep the measured `agents --json` fields as a note for a possible later "adopt a user's bg session" feature.
4. **0.2 Process model: marker scan.**
   - Sharpen "`ps -E` reads a same-user process's environment" to: "…except for Apple platform binaries (`sh`,
     `bash`, `zsh`, `sleep`, `tail`, `perl`, `caffeinate`, …), for which it returns argv only" [M: m3b].
   - "Also finds a double-forked orphan that launchd has adopted" then holds only when the orphan's final image is
     not a platform binary.
5. **0.2 Process model: socket.**
   - AF_UNIX paths are limited to 103 bytes [M].
   - Put the socket at a short fixed path, or bind it relative after `chdir`. Fail fast with a message naming the
     path and the limit.
6. **0.5 Hook protocol: Stopping.**
   - Keep the three discovery methods. The process group alone misses a setsid'd grandchild [M].
   - Spell out the order: SIGTERM to everything found, then the grace period, then a **rescan**, then SIGKILL, then
     the cleanup hook, then a final rescan with a survivor report [M: the orphan that ignores SIGTERM needed the
     SIGKILL after the rescan].
   - Add a sentence that orphans whose final image is a platform binary escape all three scans, so only the cleanup
     hook reaches them. A detached shell loop is one example.
   - Choose a mitigation for v0; the options are below.
7. **0.13 Metrics: "Process exit totals" row.**
   - Change "exact only for processes Collide parents and reaps. There are none after a reattach" to: exact for
     every child started through the shim, including across an owner restart [M]; none for orphans [M]; none under
     `--bg` [M].
   - Add that a child's total includes descendants it waited for [M: E2].
   - Keep "peak memory is the largest child's" [M: E2 confirms it is the largest single process].
8. **0.13 Metrics: "Process sampling" row, and S8.**
   - `proc_pid_rusage` CPU fields are mach absolute-time units. Convert them with `mach_timebase_info`, which is
     125/3 on this machine [M].
   - The last sample of a non-child approximates its CPU total to within one interval. The stand-in got 1.096 s
     exactly, sampling every 0.1 s [M: E4].
9. **Stage 3: Headless core.**
   - The shim is synchronous: `std::process::Command` with `pre_exec(setsid)`, then `libc::wait4` on the pid [M: T1].
   - Never take an agent's exit from std's or tokio's `wait`, and never drop a tokio `Child` [M: T2–T4].
   - The daemon may reap shims with tokio, because it reads totals from `exit.json`.
   - Add success criteria:
     - "a phase that ends while the owner is down keeps its exit code and totals";
     - "the next phase starts with the app closed".
   - `app/src-tauri` spawns no children. Whether `plugin-shell` kills children on exit is [U].
10. **Stage 3 success criterion: "Stopping leaves zero processes, including … a double-forked orphan".**
    - That holds only when the orphan is not a platform binary [M: m3].
    - Either narrow the criterion, or add a mitigation and test it with a `/bin/sleep` orphan.
11. **Risks: "Orphaned processes".**
    - Add the platform-binary blind spot [M].
    - Note that clearing the variable in-process does not escape the scan; only exec with a cleaned environment
      does [M].
12. **Risks: "Pipes die with the daemon…".**
    - This can be marked resolved. File logs survived a daemon SIGKILL with no gaps, and the shim keeps exit totals
      [M].

### Mitigation options for platform-binary orphans (item 6)

**A. Report only.**

- The survivor report lists suspects: ppid 1, same uid, started after the run, environment unreadable.
- Pros: cheap; it never kills a wrong process.
- Cons: noisy (5–8 suspects per run, one ours [M]); it reports and does not stop.

**B. Hooks record what they detach.**

- Hooks record everything they start in the background, in a lane-file contract for the cleanup hook, as Orchard
  already does for its servers.
- Pros: precise, and already Orchard's pattern.
- Cons: relies on the hook's author.

**C. Remember every pid in the run's tree.**

- The owner remembers every pid and start time it ever sees in the run's tree, by walking during the run or through
  S8's sampler, and kills the survivors at stop.
- Pros: automatic.
- Cons: a double fork's middle process lives for milliseconds, so it is likely missed [I]; untested [U].

**D. kqueue `NOTE_FORK` on every descendant.**

- Cons: racy, because the owner must register each new pid after the fork [I]. `NOTE_TRACK` is not available on
  macOS [I]. Untested [U].

**Suggested v0 position: B plus A.** State in 0.5 that such orphans need the cleanup hook. Measure C in Stage 3 only
if a real pack hits the gap.

## Unmeasured

- **Daemon supervision:** a launchd `KeepAlive` agent, reboot, and logout. Whether children survive logout is [U];
  [I] they do not.
- **Shim failure:** a shim SIGKILLed mid-run (`exit.json` missing; the child reparents to launchd), and the shim's
  per-child overhead.
- **claude's daemon:** killing or restarting it mid-session under `--bg` (`--keep-workers`, `bg adopt`), and
  `--bg --resume`.
- **Tauri:** `plugin-shell`'s behaviour on exit, and Cmd-Q of a real Tauri app through AppKit (only a shell-script
  `.app` exiting normally was measured).
- **Hostile hooks:** a hook that respawns, or that races its forks against the stop.
- **Pid reuse:** the start-time guard matched, but a pid-reuse collision was not provoked.
- **Containers:** whether container processes are visible to host `ps`. [I] They run inside Docker Desktop's VM.
- **Trust gate:**
  - why one probe directory passed the trust gate once and later failed;
  - `-p` skipping the gate is documented, not measured here.
- **Scale:** many concurrent runs.
- **Linux:** `ru_maxrss` is in KiB there, `/proc/<pid>/environ` replaces `ps -E`, and [I] `PR_SET_CHILD_SUBREAPER`
  would let the daemon adopt orphans. macOS has no equivalent [I].
- **Rust/tokio:** Rust std and tokio reaping were measured (T1–T5). A full tokio-based daemon was not.

## Cleanup record

Run at 2026-11-13 09:18 PST, after the last script [M]:

- `python3 check_clean.py` found:
  - 0 processes with `COLLIDE_RUN=s3-*`;
  - no containers labelled `collide.spike=1`;
  - 10 spike `--bg` sessions known, 0 still listed, 0 running;
  - the claude daemon not running.
- `docker ps -a --filter label=collide.spike` printed the header only.
- `ps -axE -ww | grep 'COLLIDE_RUN=[s]3-'` printed nothing.

All kills were scoped to recorded pids, the spike's markers, and the `collide.spike=1` label. One mid-run accident is
recorded:

- A `| head` pipe SIGPIPE'd `m3_stop_tree.py` and left its tree running.
- That tree was cleaned the same way: 9 pids killed and container `1436f2bf` removed.
- The script was then rerun with its output going to a file.
