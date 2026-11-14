# herdr and herdr-core: process and task management lessons for Collide

Research input for plan revision 10, 2026-11-13. Both repos were read only. Nothing from them was built, run or
installed.

**Sources**
- herdr-core at commit d9e6826, version 0.1.21. The clone is shallow (one commit), so fixes come from CHANGELOG
  entries and tests, not diffs.
- herdr at commit ee5bfb5, version 0.9.5.

**Labels**
- **[read `loc`]** means the claim was seen in the source at the file and symbol named (or heading,
  or changelog version).
- **[inferred]** means it is reasoning that the source does not state.

**Location prefixes**
- `core/` is `herdr-core/crates/herdr-core/src/`.
- `core-tests/` is `herdr-core/crates/herdr-core/tests/`.
- `core-repo/` is the herdr-core repo root.
- `herdr/` is the herdr repo root.
- `core-CL` and `herdr-CL` mean that repo's `CHANGELOG.md`, cited by version entry (for example `core-CL 0.1.20`).

## Two designs, one project

**herdr 0.9.5** is a TUI multiplexer.
- Its server owns every PTY itself, through portable-pty 0.9.0 with four Windows patches
  [read `herdr/Cargo.toml`, `[dependencies].portable-pty`, `[patch.crates-io]`, `herdr/vendor/portable-pty.patches.md`].
- A server upgrade is "live handoff", which is experimental and opt-in. It passes the PTY master fds over SCM_RIGHTS
  in batches of 64 [read `herdr/src/server/handoff.rs`, `HANDOFF_VERSION`, `READY_TIMEOUT`, `OWNED_ACK_TIMEOUT`,
  `FDS_PER_MESSAGE`, herdr-CL 0.6.3].

**herdr-core 0.1.21** has been public since 2026-10-23 (core-CL 0.1.10).
- It is a headless core: "Core draws nothing" [read `core-repo/README.md`, the intro under the title].
- A separate **holder** process owns the PTYs, so the core can crash, restart or upgrade without touching programs
  [read `core/holder.rs`, module doc comment].
- herdr does not depend on herdr-core [read `herdr/Cargo.toml`, no entry].
- herdr-core is the newer design and the closer match to `collide-daemon` [inferred].

## Summary

| | Question | Verdict | Affects |
|---|---|---|---|
| A | Holder design | **adapt**: keep the per-child shim, copy the protocol rules | decision 1, plan 0.2 |
| B | Environment | **adopt** pieces; S4's design stands | S4, plan 0.2, Stage 3 test (plan: the headless-core stage) |
| C | Starting the server | **adapt**: on macOS, the on-demand spawn must escape the app's coalition in v0 | **decision 2**, plan 0.2 |
| D | Stopping | **adapt**: descendant closure, stamped marker, in-process scan | decision 3, plan 0.5 Stopping |
| E | Exit totals, durability | Collide is ahead on totals; **adopt** the durability rules | decision 1 (`exit.json`), plan 0.13 |
| F | Agent state, resume | **ignore** screen detection; **adopt** hook hygiene | decision 3, decision 4, plan 0.3 |
| G | Cross-OS | **adopt** the platform seam and stop intents | plan 0.2 crates (`core` vs `host`) |
| H | Testing | **adopt** the test patterns | Stage 1 and Stage 3 tests, S3's unmeasured list |
| I | herdr-core as a dependency | **do not depend**; the reason in plan 0.3 needs correcting | plan 0.3, "herdr is out" |

---

## A. Holder design

### What herdr-core does

- **One holder per build and login session, not one per terminal.**
  - `State.terms: HashMap<u32, Term>` holds every terminal [read `core/holder.rs`, `State`].
  - This corrects the brief, which assumed one holder per terminal.
- **What the holder does:**
  - It is every program's parent.
  - It always reads output.
  - It keeps about 1 MiB of tail per terminal, so a crashed core can rebuild screens.
  - It restarts a crashed core.
  
  [read `core/holder.rs`, module doc comment, `TAIL_BYTES`, `EXIT_DRAIN`, `BACKLOG_BYTES`, `Holder::supervise()`]
- **The core-to-holder protocol** uses binary frames `[kind u8][terminal u32 LE][len u32 LE][payload]`, with VERSION 1
  and payloads of at most 16 MiB [read `core/holder_proto.rs`].
  - Commands: HELLO, SPAWN, INPUT, RESIZE, SIGNAL, RELEASE, DRAIN, GOODBYE{Handover|Shutdown}.
  - Replies: WELCOME, TAIL, OUTPUT, SPAWNED, SPAWN_FAILED, EXITED{code, signal}, BYE.
  - "VERSION is checked in Hello/Welcome and is separate from ownership (the newest Hello owns)."
    [read `core/holder_proto.rs`, module doc comment]
- **WELCOME is a full inventory** [read `core/holder_proto.rs`, `core/holder.rs`, `Holder::serve()`]:
  - an instance id that is random per process;
  - pid, build and login session;
  - a `draining` flag;
  - `previous: none | handover | crash`;
  - every held terminal, with its pid, size and exit (if it has already exited).
- **Restart is a handover:**
  - On SIGTERM or `server restart`, the core saves its screens, sends GOODBYE Handover and exits
    [read `core/server.rs`, `serve()`, `core/runtime.rs`, `Core::handover()`].
  - The new core sends HELLO and adopts from WELCOME and the tails [read `core/runtime.rs`, `Core::take_over()`,
    `Core::adopt()`].
  - After a crash, the core rebuilds screens from the tail and marks them inexact [read `core/runtime.rs`,
    `Core::adopt()`].
- **Builds:** a newer build starts its own holder. Older holders get DRAIN and exit when their last terminal ends
  [read `core/runtime.rs`, `State.holders`, `core/holder.rs`, `Holder.draining`, `Holder::exit_if_drained()`,
  `Holder::commands()`].
- **Rules worth copying:**
  - "Never signal after reaping: the pid may belong to someone else now." [read `core/holder.rs`, `Holder::commands()`]
  - Output is always read. When the core falls 8 MiB behind, the holder applies backpressure on the core link and
    does not block the child [read `core/holder.rs`, `BACKLOG_BYTES`, `Holder::pump()`].
  - EXITED is sent only after the output has drained (200 ms) [read `core/holder.rs`, `EXIT_DRAIN`, `Holder::reap()`].
  - The record is saved before SPAWN, and the next core start kills any spawn it has no record of
    [read `core/api/run.rs`, `start()`, `core/runtime.rs`, `Core::adopt()`].
  - A spawn that times out (10 s) stays claimed as `spawn_pending`. The core "does not … start another program on a
    guess" [read `core/api/run.rs`, `start()`, `core-repo/schema/README.md`, "Events"].
- **Losing the holder:**
  - The PTY masters close, so the programs get SIGHUP.
  - The core records `holder_lost` and sweeps each terminal's cleanup token.
  
  [read `core/runtime.rs`, `Core::read_holder()`, `core/sweep.rs`, module doc comment]
- **No long-lived headless pipe.** `machine.exec` is one-shot:
  - stdin is buffered up front, at most 8 MiB;
  - output is captured;
  - the default timeout is 60 s.
  
  [read `core/exec.rs`, module header]

### Proven across OSes (all [read])

- core-CL 0.1.16 (#47): "Terminals no longer freeze when the core falls behind or loses touch with its holder".
- core-CL 0.1.20: a command run during a core restart no longer fails with "core did not start".
- Upgrades keep programs running:
  - CI builds the last release tag and upgrades from it (Linux) [read `core-repo/.github/workflows/ci.yml`,
    `jobs.upgrade`, `core-repo/scripts/upgrade-test.sh`];
  - macOS handover is tested in `core-tests/macos.rs`,
    `launchd_core_and_program_survive_the_launcher_and_leave_no_job()`.
- The design is young: 12 releases in three weeks.

### Recommendation: adapt (decision 1, plan 0.2)

1. **Keep the per-child shim.**
   - A single herdr-core holder takes every terminal down when it dies
     (`core-tests/ghosts.rs`, `a_lost_holder_takes_its_programs_with_it`).
   - A per-child shim limits a crash to one child [inferred].
2. **Copy into the shim-to-daemon protocol** [inferred from the rules above]:
   - a versioned HELLO/WELCOME carrying an instance id, build id and `previous` (clean or crash);
   - exit reported only after the output files are flushed;
   - never signal a reaped pid;
   - write the record before spawning, and kill unrecorded spawns when the daemon starts;
   - `spawn_pending` instead of a retry.
3. **Version the shim protocol separately from the daemon build**, and have the daemon accept the previous shim
   version. After a daemon upgrade, it meets shims started by the old build, just as herdr-core's new core meets old
   holders [inferred from `core/runtime.rs`, `Core::take_over()`].
4. **A shim that dies is `lost`, not exited.**
   - Holding stdin makes this sharper than in herdr. S1 found that when stdin closes, pending permission requests are
     silently denied and `claude` then reports `result/success` with exit 0.
   - So if a shim dies without writing `exit.json`, the daemon must ignore the agent's own "success", mark the phase
     lost and sweep its marker.
   
   [inferred, using `.collide/planning/spikes/sp-0001-de-claude-headless/record.md`]
5. **Let shims restart the daemon.**
   - herdr-core's holder restarts a crashed core after a grace period, with backoff [read `core/holder.rs`,
     `Holder::supervise()`].
   - A shim whose child exits while the daemon socket is dead could start the daemon, under the CLI's lock. Then a
     phase boundary is not missed while the app is closed.
   - Plan 0.2 says "The workflow engine needs a live owner between phases" [inferred].

---

## B. Environment

### What herdr-core does

- **macOS: no login-shell capture.**
  - The core inherits its launcher's environment.
  - Terminals run `$SHELL -l`, so `.zprofile` and Homebrew load [read `core/platform/unix.rs`, `own_shell()`,
    core-CL 0.1.15].
- **Login capture exists only for Linux cores started over SSH** [read `core/platform/linux_login.rs`]:
  - It runs `$SHELL -l -c`; csh and tcsh are run with argv0 `-tcsh`.
  - The script is `printf '\0HC_LOGIN_ENV\0'; /usr/bin/env -0; printf '\0HC_LOGIN_END\0'`. The markers are fixed and
    NUL-wrapped; there is no nonce.
  - stdin is `/dev/null`, the shell runs in its own session (setsid), reads are non-blocking, output is capped at
    1 MiB and the timeout is 5 s.
  - Afterwards it runs `killpg(-pid, SIGKILL)`, because "the shell may exit before its background jobs close the
    capture pipes" [read `capture()`].
  - On failure it falls back to the inherited environment and logs the shell and the reason.
  - There is **no `-i`**.
- **Child environment** [read `core/api/run.rs`, `terminal_env()`, `child_env()`]:
  - It strips identity variables, `HC_SCOPE` and `HC_RUNTIME`.
  - It sets `TERM=xterm-256color`, `COLORTERM` and `TERM_PROGRAM`.
  - It sets `LANG` when none is set, because "macOS gives apps started from Finder no LANG" [read `core/locale.rs`,
    module doc comment].
  - It adds a fresh `HC_SCOPE` token and `PWD`.
- **Restart environment** [read `core/restart.rs`]:
  - An explicit restart saves the environment to `restart-env.json` as a private file, fsynced and renamed into
    place [`save()`].
  - The file is valid for 3 minutes, and only on the same machine and the same boot [`ENVIRONMENT`, `VALID_MS`,
    `load()`].
  - It is restored before any threads start and before holders are adopted [`restore()`].
  - SIGTERM discards it [`on_sigterm()`].
  - `--fresh` replaces it, and the replacement must include `PATH` [`replacement_environment()`].
- **What the restored environment keeps and drops:**
  - It takes local paths from the new process: `HOME`, `XDG_*`, the session and socket variables, and a stale
    `SSH_AUTH_SOCK`.
  - It drops `HC_SCOPE`, `HC_RUNTIME`, the launchd variables and `XPC_FLAGS` [read `core/platform/mod.rs`,
    `restored_environment()`].
- **Program environment per OS** [read `core/platform/mod.rs`, `program_env()`]:
  - macOS drops `XPC_FLAGS`;
  - Linux takes the systemd user manager's environment;
  - Windows rereads the registry.
- **herdr strips agent markers from new panes.**
  - It removes `CODEX_THREAD_ID`, `OMPCODE`, `CLAUDECODE`, `CLAUDE_CODE_CHILD_SESSION`, `CLAUDE_CODE_SESSION_ID` and
    `CLAUDE_CODE_MESSAGING_TOKEN`.
  - The reason given: "A new pane is not a child agent of the process that started the server"
    [read `herdr/src/pane.rs`, `PaneLaunchEnv`, `apply_pane_launch_env()`].

### Proven across OSes (all [read])

- core-CL 0.1.15: macOS terminals start as login shells.
- core-CL 0.1.20 (#205): `TERM=dumb` was inherited "like an agent's" and broke tmux, vim and htop.
- core-CL 0.1.21: Linux cores started over SSH get the login profile's environment (herdr-app#873).
- Windows:
  - core-CL 0.1.21: when PowerShell 7 was not on `PATH`, every terminal failed;
  - core-CL 0.1.19: system `PATH` entries were missing (herdr-app#631);
  - core-CL 0.1.12: environment refresh.
- herdr-CL 0.9.2 (#4104): new panes inherited Claude Code session markers and iTerm2 session ids from the terminal
  that started the server.

### Recommendation: adopt pieces (S4, plan 0.2, Stage 3 test, plan: the headless-core stage)

1. **Keep S4's capture** (`$SHELL -l -i -c` with nonce markers and `env -0`). herdr-core's Linux capture has the same
   shape: markers, `env -0`, stdin from `/dev/null`, a timeout and a fallback. That is a second implementation of the
   same idea [inferred].
2. **Add from `linux_login.rs`:**
   - run the capture in its own session and `killpg` it after reading, since an interactive zsh may leave background
     jobs holding the pipe;
   - cap the output at 1 MiB;
   - log the shell and the reason on fallback;
   - handle csh-family shells.
   
   [read `core/platform/linux_login.rs`, `capture()`]
3. **Strip the full marker list, not only `CLAUDECODE`.** Strip:
   - `CLAUDE_CODE_CHILD_SESSION`, `CLAUDE_CODE_SESSION_ID`, `CLAUDE_CODE_MESSAGING_TOKEN` and `CODEX_THREAD_ID`;
   - Collide's own `COLLIDE_*` variables when a daemon starts;
   - terminal session ids (`TERM_SESSION_ID`, `ITERM_SESSION_ID`);
   - `XPC_SERVICE_NAME` and `XPC_FLAGS`.
   
   Extend the Stage 3 test (plan: the headless-core stage, the `CLAUDECODE=1` criterion) to cover all of them [read
   `herdr/src/pane.rs`, `apply_pane_launch_env()`, `core/platform/mod.rs`, `restored_environment()`,
   `program_env()`; the variable choice is inferred].
4. **Fill gaps:**
   - set `LANG` when it is missing (a Finder launch has none);
   - set `TERM=xterm-256color` for take-over PTYs.
   
   [read `core/locale.rs`, module doc comment, core-CL 0.1.20]
5. **If the captured environment is reused across a daemon restart**, bound it as herdr-core does: a short window,
   the same boot and the same machine. Otherwise capture again [inferred from `core/restart.rs`, `load()`].

---

## C. Starting and supervising the server

### What herdr-core does

- **On-demand start from any client** [read `core/client.rs`, `connect_or_start()`, `connect_or_start_explicit()`,
  `connect_starting()`, `require_existing()`, `listening_elsewhere()`, `START_WAIT`, `wait_for_socket()`]:
  1. connect;
  2. if refused, read `core.lock` to see whether a core is listening elsewhere;
  3. if a closing core still holds the lock, wait for it;
  4. spawn a core;
  5. wait up to 15 s for its socket.
- **Single instance** [read `core/server.rs`, `serve()`, `core/platform/unix.rs`, `try_lock()`]:
  - `flock(LOCK_EX|LOCK_NB)` on `core.lock`;
  - the winner writes its socket path into the lock file;
  - it adopts the holders **before** binding its socket, so no client ever sees an empty core.
- **No self-sweep.** A spawned core gets `HC_RUNTIME=1` and loses `HC_SCOPE`. A core started from inside a terminal
  is therefore never swept along with that terminal [read `core/client.rs`, `spawn_detached()`, `core/sweep.rs`,
  `RUNTIME`].
- **Linux: a transient systemd user service.**
  - It is started with `systemd-run --user`, `KillMode=process` and `OOMPolicy=continue`.
  - The stated reason: "a desktop crash, a logout or an ssh login ending can't take them along"
    [read `core/platform/systemd.rs`, module doc comment, `unit_args()`].
- **macOS: a one-shot launchd job in `gui/<uid>`.**
  - Sequence: `launchctl bootstrap`, then kickstart, then bootout.
  - Job settings: `RunAtLoad false`, `KeepAlive false`, `AbandonProcessGroup true`, `ProcessType Interactive`.
  - The job forks the core and exits. The environment and rlimits are passed over a private socket.
  - The stated reason: "setsid alone leaves it in the app's coalition, which macOS can kill when the app quits."
  
  [read `core/platform/macos_launchd.rs`, module doc comment, `launch()`, `detach()`, `plist()`]
  - If launchd fails, the core starts directly and the failure is logged once [read `fallback()`,
    `core-tests/macos.rs`, `launchd_failure_logs_once_and_preserves_direct_start()`].
  - A core started over SSH skips launchd [read
    `launchd_preserves_environment_bytes_and_hard_file_limit_without_its_own_variables()`].
- **macOS extras:**
  - **TCC responsibility.** The core and holder disclaim it through the private
    `responsibility_spawnattrs_setdisclaim` SPI [read `core/platform/macos_responsibility.rs`].
  - **Coalition upkeep.** The holder checks in as an accessory `NSApplication` "to keep its coalition represented"
    [read `core/platform/macos_appkit.rs`].
  - **Login sessions.** A core from an earlier GUI login is replaced by a launcher running in the new login
    [read `core/platform/macos_session.rs`].
  - **Shutdown.** loginwindow is watched, because "loginwindow can end agents well before launchd sends the core
    SIGTERM" [read `core/platform/macos_shutdown.rs`].
- **Windows:** the core is spawned with `DETACHED_PROCESS | CREATE_BREAKAWAY_FROM_JOB` and an explicit handle list
  [read `core/platform/windows.rs`, `spawn_detached()`].
- **fd limits.** "Daemons must not keep launchd's 256-fd or systemd's 1024-fd soft limit", so the soft limit is raised
  to min(hard, 8192) [read `core/platform/mod.rs`, `raise_nofile_limit()`, `core/platform/unix.rs`,
  `nofile_soft_limit()`].
- **Socket paths.**
  - The limit comes from `sockaddr_un`: 103 bytes on macOS, 107 on Linux.
  - The longest path is checked up front, failing with "the socket path … is too long here (N bytes; this system
    allows M)".
  - Holder sockets use an 8-hex id to stay short.
  
  [read `core/platform/unix.rs`, `MAX_SOCKET_PATH`, `core/paths.rs`, `Paths::from_env()`, `core/runtime.rs`,
  `Core::ensure_holder()`, `Core::start_holder()`]
- **Versions.**
  - The build id is the ELF build-id on Linux. Elsewhere it is dev:ino:mtime:size of the executable
    [read `core/client.rs`, `build_id()`, `BUILD`].
  - Installing stages the new binary, checks that it runs, then renames it into place
    [read `core/platform/unix.rs`, `swap_in()`].
- **herdr does the same on macOS.** "A server started from Ghostty died with every pane on Cmd-Q (herdr#4920)"
  [read `herdr/src/platform/macos/launchd_spawn.rs`, module doc comment].

### Proven across OSes (all [read])

**macOS coalition.** Both projects shipped this bug and fixed it several times:
- core-CL 0.1.17 (herdr-app#294): "quitting Herdr no longer stops your agents… The core now runs apart from the
  app".
- core-CL 0.1.20: after a login or reboot the core "often ended up running inside the app then, so force
  quitting Herdr ended those terminals".
- core-CL 0.1.20 (#140): quitting left a "running in background" entry, and stopping it ended every terminal.
- herdr-CL 0.9.4 (#4920, #4965): closing the shell that started the server stopped it.

**Logout:**
- core-CL 0.1.20 (#210): terminals opened after a logout and login could not find the user or use ssh.
- herdr-CL 0.9.1 (#4100): "Newly started macOS servers retain access to user lookup and DNS across logout… live
  handoff cannot repair their inherited service context."

**fd limits:**
- core-CL 0.1.18 (#110): the limit was 256.
- herdr-CL 0.6.4 (#327): failures started "around 40 live panes".

**Linux:**
- core-CL 0.1.19 (#135): systemd 239.
- core-CL 0.1.14 (#18): Ubuntu 22.04 and Debian 11.
- core-CL 0.1.17 (#62): a 25 s delay without polkit.

**Start timing:**
- core-CL 0.1.19: a slow core gets 15 s.
- core-CL 0.1.11: `hc update` restarts an older running core even when the installed binary is already current.

**Tests:**
- `core-tests/macos.rs`, `launchd_core_and_program_survive_the_launcher_and_leave_no_job()` checks that:
  - the core's coalition differs from the launcher's;
  - the one-shot job is gone afterwards;
  - programs share the core's coalition and survive a restart.
- `core-tests/nofile.rs`, `low_nofile_startup_can_hold_one_hundred_terminals()` holds 100 terminals under a low fd
  limit.

### Recommendation: adapt (decision 2, plan 0.2)

1. **Keep on-demand start and copy its lock protocol** [inferred]:
   - flock on a lock file;
   - the socket path written into the lock file;
   - wait for a closing owner;
   - a 15 s start wait;
   - adopt shims before binding the socket.
2. **On macOS, the on-demand spawn must leave the launcher's coalition in v0.**
   - Use a one-shot launchd job, as herdr-core does, or SMAppService or a LaunchAgent.
   - A plain setsid spawn from a Tauri app, or from Terminal or Ghostty, is the bug both projects shipped and fixed.
   - S3 measured only a shell-script `.app` that exited normally. Cmd-Q and force-quit of a Tauri app are [U]
     [read `.collide/planning/spikes/sp-0003-ye-process-ownership/record.md`, "M1 and M2 detail (fake agents)", "Unmeasured"].
   - [inferred] Collide would hit the same failure.
3. **Measure it before freezing rev 10.** It is cheap:
   - start the daemon from a real Tauri app and from Terminal;
   - compare coalition ids with `proc_pidinfo(pid, 20, …)`, as `core-tests/macos.rs`, `coalition()` does;
   - then Cmd-Q and force-quit the app.
   
   [inferred]
4. **Raise the daemon's soft fd limit.** It tails at least three files per child [inferred].
5. **Handle login sessions on macOS:**
   - Record the daemon's login session. A daemon from an earlier login loses user lookup and DNS, which agents need,
     so restart it.
   - Record `host_shutdown`, not a crash, when loginwindow ends the session.
   
   [read `core/platform/macos_session.rs`, `macos_shutdown.rs`; the application to Collide is inferred]
6. **Socket paths.** Fail fast with the computed length limit instead of S3's chdir and relative-bind workaround.
   `chdir` is process-wide in a multi-threaded daemon [inferred].
7. **Linux, later:** a systemd user service with `KillMode=process` [read `core/platform/systemd.rs`, `unit_args()`].
8. **Ignore for v0:** the TCC responsibility disclaim, a private SPI. Revisit if privacy prompts from agents name
   Collide.app [inferred].

---

## D. Stopping

### What herdr-core does

- **A normal terminal kill** [read `core/runtime.rs`, `Core::kill()`, `core/platform/unix.rs`, `Pty::foreground()`,
  `Pty::signal()`, `Pty::hang_up()`]:
  - sends HUP, waits 500 ms; TERM, 500 ms; then KILL, 2000 ms;
  - uses `killpg` on the program's group and on the foreground group.
- **Detached descendants are spared on purpose,** like tmux: nohup, setsid and daemons survive
  [read core-CL 0.1.19, `core-repo/README.md`, "Agents run agents", `core-tests/ghosts.rs`,
  `detached_programs_survive_a_normal_exit()`].
- **Crash cleanup sweeps by `HC_SCOPE=<token>`.** This covers a lost holder, the end of an exec, and a core starting
  after a crash [read `core/sweep.rs`]:
  - **Token.** It is `<start-clock stamp>-<random>`. Processes that started before the stamp are never read, which
    makes the sweep cheaper and keeps it off reused pids [`token()`, `oldest()`].
    - On macOS the clock is `mach_absolute_time`, the same clock process start times use.
    - On Linux it is min(MONOTONIC, BOOTTIME), in ticks [`now()` (Linux and macOS versions)].
  - **Members.** The sweep takes every token carrier, plus all of its descendants by ppid, because "macOS hides the
    environment of its own binaries (an agent's /bin/bash tool shell), but not their parents" [`members()`].
  - **Exclusions.** Processes marked `HC_RUNTIME=1` (cores and holders) and their descendants are never swept
    [`RUNTIME`, `members()`].
  - **Enumeration on macOS is in-process** [`processes()` (macOS), `environ()` (macOS)]:
    - `proc_listallpids`;
    - `proc_pidinfo(PROC_PIDTBSDINFO)` for the ppid and zombie state;
    - `proc_pid_rusage` for `ri_proc_start_abstime`;
    - `KERN_PROCARGS2` for the environment.
  - **Enumeration on Linux** reads `/proc/<pid>/stat` field 22 and `/proc/<pid>/environ` [`processes()` (Linux),
    `environ()` (Linux)].
  - **Escalation:** HUP 500 ms, TERM 500 ms, KILL 2000 ms, scanning again before each step [`sweep()`].
  - **Crash recovery:** the token is kept in the state directory, so a core that crashed sweeps it at its next start
    [read `core/exec.rs` header, `core/runtime.rs`, `Core::attach_all()`].
- **Pgid reuse guard.** `waitid(WEXITED|WNOWAIT)` checks for an exit but leaves the process unreaped: "until it is
  reaped its pid, and so its group id, can't go to another process" [read `core/platform/unix.rs`, `exited()`].
- **Windows:** each terminal has a job object with `KILL_ON_JOB_CLOSE`, so there is "nothing to hunt"
  [read `core/sweep.rs`, `sweep()` and `members()` (Windows versions)].
- **herdr's TUI is different** [read `herdr/src/pane.rs`, `shutdown_pane_processes()`]:
  - it takes one pid list from the child's session;
  - it sends HUP, TERM, then KILL, 250 ms apart;
  - it does not scan again between steps.

### Proven across OSes (all [read])

- core-CL 0.1.19 (#136): detached programs survive a terminal close.
- `core-tests/ghosts.rs` (helpers `python()`, `stubborn()`, `pid_in()`; tests
  `kill_spares_programs_that_ignore_hangup()` to `a_run_while_stopping_is_refused()`) cover:
  - processes that ignore HUP and TERM;
  - setsid'd tool shells;
  - a SIGKILLed holder;
  - a holder SIGSTOPped during a stop;
  - a terminal that starts another core.
- `core/sweep.rs`, `a_fresh_token_finds_its_own_in_a_shifted_time_namespace()` runs a sweep inside a Linux time
  namespace.
- The limit is stated outright: a process "that leaves the session and drops its environment escapes"
  [read `core/sweep.rs`, module doc comment].

### Recommendation: adapt (decision 3, plan 0.5 Stopping, plan 0.2 marker scan)

1. **This confirms decision 3.** The token scan plus descendant closure still misses a double-forked Apple platform
   binary whose parent has died [inferred from `core/sweep.rs`, `members()`]. Keep the hook-recorded pids,
   the cleanup hook and the survivor report.
2. **Add the descendant closure** from marker carriers. It catches the common case that a pure `ps -E` scan misses:
   claude's `/bin/bash` tool shell and its `/bin/sleep` [read `core/sweep.rs`, `members()` doc comment].
3. **Stamp `COLLIDE_RUN` with the start clock** and skip processes older than the stamp. This replaces the separate
   start-time guard and removes the pid-reuse question at its source [read `core/sweep.rs`, `token()`, `oldest()`].
4. **Enumerate in-process with libproc** instead of parsing `ps -E` [read `core/sweep.rs`, `processes()` (macOS),
   `environ()` (macOS)]. [inferred] It is faster and has no truncation or locale parsing.
5. **Use a runtime mark.**
   - Set `COLLIDE_RUNTIME=1` on the daemon and the shims, and exclude them from sweeps.
   - The shim must stay out of the kill set so it can reap its child and write `exit.json` [inferred].
   - When the CLI starts a daemon from inside an agent, strip `COLLIDE_RUN` and `COLLIDE_SPAN`. Otherwise that run's
     own stop kills the daemon [read `core/client.rs`, `spawn_detached()`, `core/sweep.rs`, `members()`].
6. **Scan again before each escalation step,** as herdr-core does, rather than reuse one list as herdr does
   [read `core/sweep.rs`, `sweep()`, `herdr/src/pane.rs`, `shutdown_pane_processes()`].
7. **In the shim, check with `waitid(WNOWAIT)` before `killpg`,** so the group id cannot be reused between the check
   and the signal [read `core/platform/unix.rs`, `exited()`].
8. **Keep Collide's grace periods.** herdr's 0.5 s suits interactive shells. Keep 60 s and 30 s for hooks, and S1's
   order for agents [inferred].

---

## E. Exit status, resource totals, durability

### What herdr-core does

- **Exit status:**
  - It is `{code, signal}` only, from `std::process::Child::wait`. There is no rusage anywhere
    [read `core/platform/unix.rs`, `wait()`, `core/holder_proto.rs`].
  - Windows has an exit code and no signal [read `core/platform/windows.rs`, `Pty::wait()`].
- **Finish reasons:** `exited`, `killed`, `server_stop`, `host_shutdown`, `holder_lost` and `stopped_by_restart`.
  Kept terminals can also be `signaled` [read `core/runtime.rs`, `finish_reason()`, `core-repo/schema/README.md`,
  "Events"].
- **Atomic writes.** `write_atomic` writes `<name>.tmp`, fsyncs and renames it. There is no directory fsync, and the
  tmp name is fixed [read `core/paths.rs`, `write_atomic()`].
- **A dedicated writer thread** keeps only the newest snapshot and writes off the state lock. `flush` waits only for
  the current version [read `core/runtime/persistence.rs`, `RegistryWriter`].
  - This fixed core-CL 0.1.17: "The core no longer freezes every terminal while it saves to disk".
- **Failed writes do not stop persistence.**
  - A failed write is logged with `let _ = writeln!(io::stderr(), …)` and the thread carries on
    [read `core/runtime/persistence.rs`, `RegistryWriter::start()`].
  - A Linux test points `core.log` and the tmp file at `/dev/full`, then checks that every terminal survives a
    restart [read `core-tests/persistence.rs`].
  - [inferred] The cause was `eprintln!`, which panics when stderr fails and killed the writer.
  - Some `eprintln!` calls remain [read `core/restart.rs`, `prepare()`, `restore()`].
- **Corrupt state is never silently reset.**
  - A missing file means the default. A broken file is an error [read `core/paths.rs`, `read_state()`].
  - A broken `docs.json` is renamed to `.bad`, and the core starts with empty documents
    [read `core-repo/schema/README.md`, "Documents"].
  - herdr (#4125): "Saved sessions that cannot be loaded are preserved before replacement" [read herdr-CL 0.9.1].
- **Clone detection:** a state directory copied to another machine is detected by its machine id
  [read `core/server.rs`, `CLONE_FORGETS`, `load_identity()`].

### Recommendation: Collide is ahead on exit totals; adopt the durability rules (decision 1, plan 0.13)

1. **Keep `wait4` rusage in the shim,** as plan 0.13 has it. herdr has nothing equivalent.
2. **Write `exit.json`** as tmp, fsync, rename, then **fsync the directory**. herdr-core skips the directory fsync
   [inferred].
3. **Logging must never panic.**
   - Use no `eprintln!` in daemon or shim code; use `let _ = writeln!` or a logger that swallows I/O errors.
   - Copy the `/dev/full` test [read `core/runtime/persistence.rs`, `RegistryWriter::start()`,
     `core-tests/persistence.rs`].
4. **Hold the record if the disk is full.** If `exit.json` cannot be written (ENOSPC), the shim keeps the record in
   memory and serves it over its socket until the daemon acknowledges it [inferred].
5. **Quarantine and report** a corrupt store or `exit.json`; never replace it silently
   [read `core/paths.rs`, `read_state()`, herdr-CL 0.9.1].
6. **Add finish reasons for "shim lost" and "host shutdown"**, so recovery can tell them apart from a real exit
   [inferred from `core/runtime.rs`, `finish_reason()`].

---

## F. Agent state and resume

### What herdr does

- **Claude Code's state comes from the screen.**
  - "Agent state detection via terminal tail pattern matching", with a manifest per agent.
  - PTY activity is the authority for `working` [read `herdr/src/detect/mod.rs`, module doc comment, `AgentState`,
    `AgentDetection`].
- **Hooks report session identity only, since 0.6.7.** "Claude Code, Codex, … integrations now report session
  identity only. Native state for those agents comes from Herdr's screen detection" [read herdr-CL 0.6.7].
  - The installer removes the old state hooks: UserPromptSubmit, PreToolUse, PostToolUse, Stop, PermissionRequest,
    SubagentStop and SessionEnd.
  - It keeps one `SessionStart` hook, with matcher `^(startup|resume|clear|compact|fork)$`
    [read `herdr/src/integration/claude_settings.rs`, `SESSION_START_MATCHER`, `HookRemoval`, `HOOK_REMOVALS`].
- **Why state moved off hooks** (all [read]):
  - subagent completion hooks revived idle panes (herdr-CL 0.6.0, #198; herdr-CL 0.5.2);
  - post-tool hooks reported `working` (herdr-CL 0.5.12);
  - `working` went stale after a cancellation (herdr-CL 0.6.1, #249);
  - `blocked` went stale after a permission prompt (herdr-CL 0.6.6, #349);
  - reports arrived out of order (herdr-CL 0.5.8).
  - herdr dropped Codex hooks again in 0.9.5 (herdr-CL 0.9.5).
- **The Claude hook script never fails Claude**
  [read `herdr/src/integration/assets/claude/herdr-agent-state.sh`]:
  - every error exits 0;
  - the socket timeout is 0.5 s;
  - it ignores events that carry `agent_id` (subagents);
  - it ignores Cursor's Claude-compatible events;
  - it stamps a `time_ns` sequence so the server rejects stale reports.
- **Nested sessions.** Since #620, session restore accepts real `/clear`, `/resume` and compaction identity changes
  "while still ignoring nested `claude -p` startup sessions that inherit the pane environment" [read herdr-CL 0.7.0].
  - Acceptance depends on which agent the pane's foreground process is
    [read `herdr/src/terminal/state.rs`, `set_agent_session_ref_for_session_start`].
- **Resume:**
  - It is opt-in, through `[session] resume_agents_on_restore` [read herdr-CL 0.6.3].
  - After a server restart, herdr types `claude --resume <id>` into the restored pane's fresh shell.
  - The argv must start with a bare command name.
  - The dedupe key includes the working directory [read `herdr/src/agent_resume.rs`, `ReportedAgentResume`,
    `ReportedAgentResume::plan()`].
- **herdr-core:**
  - Status comes only from OSC 7501, which a program emits about itself. "hc never infers status from the screen"
    [read `core-repo/schema/README.md`, "Status", `core/status.rs`, module doc comment].
  - `--keep` starts a fresh shell in the last folder, and "the original command is never re-run"
    [read `core-repo/schema/README.md`, "Events"].

### Recommendation: ignore screen detection; adopt hook hygiene (plan 0.3, 0.5, decisions 3 and 4)

1. **Ignore screen detection and OSC 7501.** Headless phases get their state from stream-json and `control_request`
   (decision 4).
2. **Do not derive phase state from Claude hooks.** herdr tried it and backed out [read herdr-CL 0.6.7].
3. **Filter hook events by session.**
   - Collide's hooks (decision 3 records detached pids) fire in every `claude` that inherits the phase environment.
     That includes a nested `claude -p` an agent starts, and its subagents.
   - Accept events only when `session_id` matches the phase's `--session-id`. Drop events that carry `agent_id`, or
     attribute them to the nested session.
   
   [read herdr-CL 0.7.0 and the hook script; the application to Collide is inferred]
4. **Hooks always exit 0, time out fast and carry a monotonic sequence number** [read the hook script].
5. **Track session id changes after a take-over.**
   - The session id changes on `/clear`, `/compact` and `fork` [read `herdr/src/integration/claude_settings.rs`,
     `SESSION_START_MATCHER`].
   - After an interactive take-over, capture the newest id (from SessionStart with those sources) before resuming
     headless by id [inferred].
6. **Resume only by explicit session id; never rerun the task.** This matches herdr-core's rule and plan 0.2
   [read `core-repo/schema/README.md`, "Events"].

---

## G. Cross-OS strategy

### What herdr-core does

- **One small platform seam:** "connections, terminals, process trees, the console, files. Everything above it is
  shared." [read `core/platform/mod.rs`, module doc comment]
  - The modules are `unix.rs` (Linux and macOS), `macos_*.rs`, `systemd.rs` and `linux_login.rs`, and `windows*.rs`.
- **Windows** [read `core/platform/windows.rs`]:
  - **IPC:** named pipes whose DACL allows the user and SYSTEM, with an integrity label. Pipe names include the user
    [`pipe_security()`, `MAX_SOCKET_PATH`, `endpoints()`].
  - **ConPTY:** a bundled `conpty.dll`, because "Windows' own drops input escape sequences agents answer with"
    [`conpty()`].
  - **Job objects:** one per terminal, with `KILL_ON_JOB_CLOSE | BREAKAWAY_OK`. The program is created suspended,
    assigned to the job, then resumed, so nothing escapes before assignment [`spawn_pty()`]. The result of the
    assignment is not checked.
  - **Signals:** interrupt writes `\x03`; anything else is `TerminateJobObject`. Only the exit code comes back
    [`Pty::foreground()`, `Pty::signal()`, `Pty::hang_up()`, `Pty::wait()`].
  - **`.cmd` shims** (npm-installed agents) are quoted per BatBadBut [`command_line()`, `batch_line()`].
  - **No SIGTERM handover** [`termination()`, `boot_id()`].
- **herdr carries four Windows patches on portable-pty 0.9.0:** ConPTY loading, raw command tails, malformed
  environments, and directories on `PATH` [read `herdr/vendor/portable-pty.patches.md`].

### Proven across OSes (all [read])

- Windows fixes:
  - core-CL 0.1.21: when PowerShell 7 was not on `PATH`, every terminal failed;
  - core-CL 0.1.19: system `PATH` entries;
  - core-CL 0.1.12: environment refresh;
  - herdr-CL 0.8.0 (#2008): detached servers now survive logout from the OpenSSH session;
  - herdr-CL 0.7.1 (#761): system ConPTY;
  - herdr-CL 0.8.0: an app-local ConPTY.
- CI runs the full test suite on Linux (musl), macOS 15 (arm64) and Windows 2025
  [read `core-repo/.github/workflows/ci.yml`, `jobs.test.strategy.matrix.include`].

### Recommendation: adopt the seam shape (plan 0.2 crates; "core must not preclude Linux or Windows")

1. **Shape the `core` types for every OS** [inferred from `core/holder_proto.rs`, `core/platform/windows.rs`,
   `Pty::foreground()`, `Pty::signal()`, `Pty::hang_up()`, `Pty::wait()`]:
   - process identity is (pid, start stamp);
   - stop is a set of intents (interrupt, terminate, kill), not signal numbers;
   - exit is {code, signal?, rusage?}, with signal and rusage optional because Windows has neither.
2. **Windows, later:**
   - a job object per child, created suspended (it replaces the marker scan there);
   - interrupt via `\x03` or CTRL_BREAK;
   - correct `claude.cmd` quoting.
3. **Linux, later:**
   - a `/proc` scan and a systemd user service;
   - [inferred] a cgroup per run could replace the marker scan.
4. **Expect to pin or vendor portable-pty for Windows** [read `herdr/Cargo.toml`, `[dependencies].portable-pty`,
   `[patch.crates-io]`].

---

## H. Testing

### What herdr-core does

- **Each integration test drives the real binary in an isolated session** [read `core-tests/common/mod.rs`,
  `test_tmp()`, `Session::new()`]:
  - it gets its own `HC_SESSION` and XDG directories;
  - these live under `/var/tmp`, "whose short path keeps socket paths under the macOS limit";
  - a 120 s watchdog fails a hung test.
- **Fixtures are python3, not `/bin/sleep`,** because "macOS hides the environment of its own binaries"
  [read `core-tests/ghosts.rs`, `python()`, `core/sweep.rs`, `tests::finds_only_processes_carrying_the_token()`].
  - `members_settled` waits up to 5 s, because macOS's python3 is a stub that starts the real one
    [read `core/sweep.rs`, `tests::members_settled()`].
- **Hostile cases** [read `core-tests/ghosts.rs`, the tests from `kill_spares_programs_that_ignore_hangup()` to
  `a_run_while_stopping_is_refused()`]:
  - processes that ignore HUP and TERM;
  - setsid'd children;
  - a SIGKILLed holder;
  - a holder SIGSTOPped during a stop;
  - a terminal that starts another core;
  - a program that starts after its run gave up.
- **macOS:** coalition and responsibility are checked through `proc_pidinfo(…, 20, …)` and
  `responsibility_get_pid_responsible_for_pid`. The tests skip when there is no GUI launchd domain
  [read `core-tests/macos.rs`, `coalition()`, `responsible()`, `gui()`, `independent()`].
- **Environment:**
  - restart snapshots: stale or invalid ones, an environment that cannot be saved, and SIGTERM leaving none behind
    [read `core-tests/environment.rs`];
  - login capture: bounded, with a timeout, and cleaned up [read `core-tests/login_environment.rs`].
- **Resources:** a full disk (`core-tests/persistence.rs`) and 100 terminals under a low fd limit
  (`core-tests/nofile.rs`, `low_nofile_startup_can_hold_one_hundred_terminals()`).
- **CI** [read `core-repo/.github/workflows/ci.yml`, `jobs.lint`, `jobs.test`, `jobs.upgrade`]:
  - lint: fmt, plus clippy for Windows run from Linux;
  - tests on three OSes;
  - an upgrade job from the last release tag.
- **API compatibility.** `released.openrpc.json` is pinned, and a test fails on any breaking change
  [read `core-repo/schema/README.md`, the intro under the title].

### Recommendation: adopt (Stage 1 and Stage 3 tests, plan: the headless-core stage, S3's unmeasured list)

1. **Copy the ghost tests as Collide's stop tests:**
   - SIGKILL a shim;
   - SIGSTOP a shim during a stop;
   - children that ignore HUP and TERM;
   - setsid'd grandchildren;
   - a daemon started from inside a run.
   
   These cover S3's "Shim failure" and "Hostile hooks" items.
2. **Use python3 or a Collide test binary to carry the marker,** not `/bin/sleep`. Prove decision 3's platform-binary
   escape in a separate test [read `core-tests/ghosts.rs`, `python()`].
3. **Add a coalition test** for a daemon started by the Tauri app and by the CLI. This covers S3's "Tauri" item.
4. **Use short test roots** for socket paths, and add a watchdog.
5. **Generate the socket protocol schema from the Rust types,** and pin a released copy.
6. **Add an upgrade test:** start an old daemon with running shims, then check that the new daemon adopts them.

---

## I. herdr-core as a dependency

### Facts

- **Licence:** Apache-2.0 [read `core-repo/LICENSE`, `README.md`, the "License:" line under "Build from source"].
- **Stability:**
  - It is not published: `publish = false`.
  - It is a binary crate whose `lib.rs` exposes internals (`pub mod runtime`, `holder`, …) with no stability promise
    [read `core-repo/Cargo.toml`, `core/lib.rs`].
  - Version 0.1.21. The first public release was 2026-10-23, and there have been 12 releases since, with process-model
    changes on macOS in 0.1.17, 0.1.18 and 0.1.20 [read core-CL].
- **Build:** needs Zig 0.16 and a vendored ghostty submodule for `ghostty-vt`
  [read `core-repo/.github/workflows/ci.yml`, `jobs.test` step `mlugg/setup-zig`, `.gitmodules`, `core/Cargo.toml`].
- **API:**
  - JSON-RPC 2.0 over a Unix socket or named pipe, described by an OpenRPC schema;
  - the API "only grows";
  - status: preview, "protocol generation 1, version 0.x" [read `core-repo/schema/README.md`,
    the intro under the title].
- **It streams, which corrects R1** [read `core-repo/schema/README.md`, "Attach streams", "Events"]:
  - `terminal.attach` sends raw PTY output bytes with credit-based flow control.
  - `events.subscribe` returns a snapshot plus events numbered by `seq`, with heartbeats and `missed` resyncs. Events
    include `finished{reason, exit_code, signal}` and `status`.
  - R1's "snapshot only" (herdr 0.9.5's `pane.read`) does not hold for herdr-core.
- **No headless stdin pipes.**
  - Programs run in PTYs. `machine.exec` is one-shot, with stdin buffered at start [read `core/exec.rs` header].
  - `claude -p --input-format stream-json` needs a long-lived stdin pipe and separate stdout and stderr (decision 1,
    S1).
  - [inferred] In a PTY, stderr is merged, input is echoed and line discipline applies.
- **No exit totals:** there is no rusage [read `core/platform/unix.rs`, `wait()`].
- **Markers and stopping:**
  - It uses its own `HC_SCOPE` token, not `COLLIDE_RUN`. Extra environment variables pass through `merge_env`
    [read `core/api/run.rs`, `terminal_env()`, `child_env()`].
  - A normal close spares detached programs by design [read core-CL 0.1.19]. Collide's stop must not.
- **Security:** there is no per-client token, and any process running as the user can connect
  [read `core-repo/docs/security.md`, "Who can connect"]. Plan 0.2 requires a per-client token.

### Pros and cons

**Pros:**
- PTY ownership that survives crashes and upgrades, proven across OSes;
- streaming attach;
- events with resync;
- Windows ConPTY and job objects already done.

**Cons:**
- no headless pipes, which is Collide's main path;
- no rusage;
- no per-client token;
- unpublished and three weeks old;
- a Zig toolchain;
- a second process owner beside `collide-daemon`. Collide's run and phase model would be a client of it, and both
  owners would need stop, environment and marker logic [inferred].

### Verdict: do not depend; adapt its patterns (plan 0.3)

- Revisit only for interactive take-over, and only once a stable crate is published [inferred].
- Correct plan 0.3's "herdr is out" note. The reasons are now:
  - no long-lived stdin pipes;
  - no rusage;
  - no per-client token;
  - no stability promise.
  
  The old reason, snapshot-only reads, applies to herdr 0.9.5 but not to herdr-core.

---

## Bugs herdr hit that Collide should avoid

All entries are [read] unless marked.

| # | Bug | Evidence |
|---|---|---|
| 1 | The daemon stays in the app's or terminal's coalition and dies on quit, Cmd-Q or force-quit, despite setsid | core-CL 0.1.17, 0.1.20; herdr-CL 0.9.4 (#4920); `core/platform/macos_launchd.rs`, module doc comment |
| 2 | A daemon from an earlier login loses user lookup, DNS and the ssh agent after logout | core-CL 0.1.20 (#210); herdr-CL 0.9.1 (#4100) |
| 3 | launchd's soft fd limit of 256 caused failures around 40 panes | core-CL 0.1.18 (#110); herdr-CL 0.6.4 (#327) |
| 4 | Saving state while holding the state lock froze every terminal | core-CL 0.1.17 |
| 5 | On a full disk, logging panicked and killed the persistence writer | `core-tests/persistence.rs`; cause [inferred] |
| 6 | Saved state that could not be loaded was silently overwritten | herdr-CL 0.9.1 (#4125) |
| 7 | Agent markers were inherited: Claude Code session markers and terminal session ids (#4104), and `TERM=dumb` from an agent's shell (#205) | herdr-CL 0.9.2; core-CL 0.1.20 |
| 8 | A nested `claude -p` and subagents reported into the parent's hooks | herdr-CL 0.7.0 (#620), 0.6.0 (#198), 0.5.2 |
| 9 | Hooks used as the state authority went stale or arrived out of order | herdr-CL 0.6.7, 0.6.6, 0.6.1, 0.5.8; 0.9.5|
| 10 | Other tools that import Claude-compatible hooks (Cursor, Grok) fired them | herdr-CL 0.9.0 (#2832), 0.9.1 |
| 11 | Process discovery scanned an ever-growing tree, or unrelated host processes | herdr-CL 0.9.1, 0.7.4 (#1399) |
| 12 | Unreaped children became zombies; stuck audio players piled up | herdr-CL 0.7.4 (#1360), 0.7.5 (#1622) |
| 13 | A command that raced a restart got "core did not start"; slow starts need about 15 s | core-CL 0.1.20, 0.1.19 |
| 14 | The core spun on a kept terminal that had lost its program | core-CL 0.1.20 (#124) |
| 15 | Repeated waits grew the core's memory | core-CL 0.1.21 (#230) |
| 16 | Socket paths in macOS temp dirs were too long | herdr-CL 0.5.8 (#103) |
| 17 | Windows: PowerShell 7 off `PATH` broke every terminal, system `PATH` entries were missing, and OpenSSH logout killed detached servers | core-CL 0.1.21, 0.1.19; herdr-CL 0.8.0 (#2008) |
| 18 | Linux: older systemd versions, and no polkit, gave a 25 s start delay | core-CL 0.1.19, 0.1.14, 0.1.17 |
| 19 | An interrupted update left an empty install directory; the fix is an atomic directory swap | herdr-CL 0.8.2 |

## Against owner decisions 1-4

- **Decision 1 (per-child shim): consistent.**
  - herdr-core's single holder is the weaker design.
  - One addition: when a shim dies, its held stdin closes, which can turn pending permission requests into a silent
    `result/success`. The daemon must treat a missing `exit.json` as `lost` (A.4) [inferred].
- **Decision 2 (on demand, launchd later): contradicted on the spawn method, not on "on demand".**
  - On macOS, a daemon spawned with setsid from the app or from a terminal stays in the launcher's coalition. macOS
    can kill it when that app quits.
  - Both herdr and herdr-core shipped this bug and fixed it with a one-shot launchd job.
  - So v0 needs launchd (or SMAppService) for the spawn itself. KeepAlive supervision can still wait.
  - Not yet measured for Collide; S3 lists it as [U]. See C.2 and C.3.
- **Decision 3 (platform-binary orphans escape): confirmed.**
  - The module doc comment of `core/sweep.rs` and the doc comment of `members()` state the same limit,
    and herdr-core's tests avoid `/bin/sleep` for that reason.
  - Refine the scan with the descendant closure and the stamped marker (D.2, D.3).
- **Decision 4 (raw `claude -p` stream-json): consistent.**
  - herdr has no headless path.
  - Its experience argues against deriving state from hooks, and stream-json avoids that.
