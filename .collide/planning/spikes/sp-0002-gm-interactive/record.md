---
id: sp-0002-gm
alias: S2
kind: spike
status: concluded
title: "S2: interactive sessions"
owners: []
links: []
---
# S2: interactive sessions

- **Date:** 2026-11-13. Plan revision 10 (`IMPLEMENTATION_PLAN.md`, 0.2, 0.3, 0.11, 0.13, Stage 1 → S2). The plan is
  not edited here; the last section proposes changes.
- **Versions:** `claude` 2.1.323 (claude.ai login), node v24.18.0, macOS 26.3.1 arm64 (Darwin 25.3.0), rustc via
  `~/.cargo/env`, Tauri 2.12.5 (`@tauri-apps/cli` 2.12.4), portable-pty 0.9.0, xterm.js 6.0.0 with addon-fit 0.11.0 and
  addon-webgl 0.19.0, objc2-web-kit 0.3.2. Haiku (`claude-haiku-5-5`) for every model call.
- **Reproduction:** `.collide/planning/spikes/sp-0002-gm-interactive/`. `README.md` has the build steps and a script table; each script prints
  its own measurement and `out/<name>.txt` holds the run that this record quotes. Screenshots are in `out/shots/`.
  The build needs `pnpm install --store-dir ./.pnpm-store`, `cargo build -p s2lib` and
  `pnpm exec tauri build --debug --bundles app`; everything stays inside the spike directory.
- **Method:**
  - A throwaway Rust prototype of the plan's process model: `s2d daemon` (flock lock file, adopts shims before binding
    the socket, 100 ms supervisor), `s2d shim` (setsid, PTY via portable-pty or pipe mode, `libc::wait4`, atomic
    `exit.json`, a 1 MiB ring buffer replayed on attach, atomically with subscribing) and a small frame protocol
    (1 byte type, u32 length, payload). The daemon relays shim frames to attached clients.
  - A minimal Tauri 2 `.app` (debug bundle) with xterm.js. Output reaches the page through `ipc::Channel` with a raw
    body; input goes back through a raw-body command. A scripted bench inside the page types, waits, measures and
    snapshots.
  - Every `claude` ran through a copy of `claude-clean.sh` with `--model haiku`; headless calls carried
    `--max-budget-usd 0.25`. The wrapper drops every `CLAUDE*` variable, and `S1_PASS=CLAUDE_CODE_ENABLE_TELEMETRY`
    keeps the one the telemetry tests need. Interactive sessions cannot take a budget (Q5).
  - The scratch repo `work/repo` is a trusted git repo; `work/repo-sibling` is an untrusted one.
  - launchd jobs were labelled `dev.collide.spike.s2.*`, with plists under `work/`, booted out and removed as soon as
    the daemon answered. None remain (checked with `launchctl list` at the end).
- **Labels:** **measured** [M: script], **inferred** [I], **unmeasured** [U]. A measured claim names the script and
  quotes what it printed.
- **Screenshots.** `screencapture -l <windowid>` and a full-display `screencapture` both fail here with `could not
  create image from window` / `from display`: the environment has no Screen Recording permission (measured, three
  tries, `scripts/windowid.swift` found the window id). So the app snapshots its own webview with
  `WKWebView.takeSnapshot`, which needs no permission. That shows the page, not the window chrome. `screencapture -l`
  itself stays unmeasured.

## Answers

### Q1. portable-pty → `ipc::Channel` → xterm.js with Claude's TUI

**Latency, keystroke to echo.** Keystrokes were injected with `term.input()`, so the DOM key-event path is not in these
numbers. WebKit's `performance.now()` has 1 ms resolution. "Arrival" is when the echo chunk reaches the webview's
channel callback; "rAF" is the next animation frame (not photon time).

| path | n | p50 | p90 | p95 | max |
|---|---|---|---|---|---|
| node client → daemon → shim, `cat` | 72 | 0.41 ms | 1.08 | 1.65 | 3.33 |
| node client → shim directly, `cat` | 72 | 0.33 ms | 1.03 | 1.96 | 3.31 |
| node client → daemon → shim, claude TUI | 72 | 8.34 ms | 29.3 | 39.2 | 69.7 |
| node client → shim directly, claude TUI | 72 | 7.91 ms | 14.4 | 22.2 | 58.3 |
| app, `cat`, WebGL, arrival | 200 | 2 ms | 3 | 4 | 38 |
| app, `cat`, WebGL, to rAF | 200 | 3 ms | 4 | 5 | 39 |
| app, `cat`, DOM renderer, arrival | 200 | 2 ms | | 3 | 12 |
| app, claude, WebGL, arrival | 100 | 13 ms | 40 | 59 | 762 |
| app, claude, WebGL, to rAF | 100 | 15 ms | 43 | 60 | 764 |
| app, claude, DOM renderer, arrival | 100 | 11 ms | 35 | 53 | 116 |
| app, claude, DOM renderer, to rAF | 100 | 13 ms | 37 | 55 | 121 |

[M: `q1-node-latency.mjs`, `q1-app.mjs cat`, `q1-app.mjs claude`; `out/q1-node-latency.txt`, `out/q1-app-cat.txt`,
`out/q1-app-claude.txt`.]

- The plumbing is cheap: for `cat` the whole path to the page is 2 to 3 ms at p50, and the daemon relay adds about
  0.1 ms (0.41 against 0.33 ms) [M].
- Claude's own response to a keystroke dominates: p50 8 ms on a node client, 13 ms in the app, with a long tail (p95
  59 ms, one 762 ms outlier in the WebGL run) [M]. The tail is Claude's redraw work, not the pipe: the direct-to-shim
  node run has the same shape [I].
- WebGL against DOM: no benefit from WebGL in this test. The DOM renderer was as fast or faster (claude p50 11 against
  13 ms) [M]. This was a quiet window; a long scrollback under load is unmeasured [U].
- The echo is the TUI's own redraw of the input box, so it is the latency a typist would feel apart from the key
  event itself and the display [I].

**Rendering fidelity** [M: `q1-app.mjs claude`, screenshots `out/shots/claude-welcome-*.png`, `claude-tool-webgl.png`,
`claude-answer-webgl.png`, `claude-resize-*.png`, `q2-*.png`; I looked at each].
- The logo, colours, statusline, the bordered markdown table, bullets, the "Allowed by auto mode" line and box-drawing
  characters all render correctly, with WebGL and with DOM.
- Two cosmetic defects: a small empty band under the last row (the remainder `fit` leaves), and a scrollbar when the
  window is shrunk.
- The user's own settings load in the PTY (auto mode, statusline, hooks, about 51k tokens of starting context), so
  the take-over terminal looks like the user's normal Claude [M].

**Resize** [M: `q1-app.mjs claude`, `q1-resize-stty.mjs`].
- Claude reflows correctly. 130x42 → 90x30: xterm resized after 34 ms, first redraw output after 170 ms, quiet after
  235 ms (6 chunks). 90x30 → 168x53: 22 ms, 123 ms, 130 ms. 168x53 → 117x36: 9 ms, 153 ms, 210 ms.
- Under `/bin/sh` in the shim's PTY, `stty size` after each window resize matched xterm four times out of four
  (29x90, 49x157, 36x117, 42x130); xterm resized in 7 to 28 ms (`out/q1-resize-stty.txt`).

**Answer:** the path works and is fast enough; the only latency worth worrying about is Claude's. Either renderer is
fine; DOM is the simpler default [I].

### Q2. Take-over with `claude --resume <id>` in a shim-held PTY: app restart, daemon restart

[M: `q2-takeover.mjs`, `out/q2-run.txt`.]
- A headless phase ran under a shim in pipe mode (session `744a1583-…`, "noted.", $0.0039, exit 0) and ended.
- `claude --resume 744a1583-…` started in a PTY under a shim, driven by the real Tauri app. Asked for the secret word
  from the headless turn, it answered `PLUM-731`: the take-over has the headless context.
- **App quit** (AppKit quit event): the shim and claude stayed alive.
- **Daemon SIGKILL**: the shim and claude stayed alive. A new daemon, started as a launchd one-shot, came up in
  614 ms and adopted the run as `running`.
- **App relaunched:** attaching replayed the earlier conversation from the ring buffer (`q2-3-reattached.png` shows
  it faithfully), the same claude pid answered a new prompt, and the screen was correct.
- **So a take-over survives an app restart and a daemon restart,** because the PTY master lives in the shim, which
  nothing but a SIGKILL of the shim itself ends (Q3).
- **Trust dialog** [M: `q2-trust-gate.txt`, `trust-repo.mjs`]: in a directory Claude has not trusted, interactive
  claude stops at a workspace-trust dialog before anything else, and the highlighted default is "No, exit": plain
  Enter exits claude with code 1. Down then Enter trusts. A subdirectory of a trusted directory shows no dialog; a
  sibling directory does. A fresh worktree is a sibling, not a subdirectory, so a New session or take-over in one will
  meet it (inferred from the sibling result [I]). Whether it can be pre-accepted is unmeasured [U].
- Not done: the take-over of a headless process that is still running. The headless phase had already exited
  [U].
- Interactive claude has no `--max-budget-usd` (print mode only; `--help`). A take-over's spend is not capped by the
  flag [I, from `--help`; Q5 spent without one].
- **Closing the terminal view does not end the run** in this prototype: detach closes only the client connection
  [M: Q2, the app quit left claude running]. Whether the run ends is a policy the app must implement (see the
  proposals).

### Q3. The shim holds stdin and the PTY master across a daemon restart; a killed shim is `lost`

[M: `q3-daemon-restart.mjs`, `out/q3.txt`; `q3b-real-claude.mjs`, `out/q3b.txt`.]
- **Daemon SIGKILL, shims and children untouched.** `pipe1` kept writing to `out.log` (464 bytes grew while the
  daemon was dead). `pipe2` and `pty2` exited while the daemon was dead; the shim alone wrote `exit.json` (code 7,
  CPU 0.079 s and 0.084 s, max RSS 47.9 and 48.4 MB). After the restart the daemon reported both as `exited` with
  those totals.
- **Restart:** the new daemon answered 28 ms after start; adopting five runs took 1.1 ms.
- **Nothing lost.** The fake agent printed a numbered line every 100 ms: the PTY run's ring replay plus live output
  showed seq 0 to 27, 28 lines, 0 gaps; the pipe run's `out.log` has 28 lines, 0 gaps. Input sent before, while the
  daemon was dead (held by the shim) and after the restart arrived in order: `before-0 … while-dead-0 … after-restart-1`
  (8 of 8). Pipe-mode stdin stayed open because the shim holds the write end.
- **A real claude, same test** (`q3b`): `claude -p --input-format stream-json` under a pipe shim took turn 1 ("Remember
  KIWI-884", answered `stored.`). The daemon was SIGKILLed (shim and claude alive), a new daemon adopted the run
  (`running`), and turn 2 sent through the new daemon answered `KIWI-884` in the same session. Closing stdin
  through the new daemon ended claude with exit 0, CPU 8.10 s, max RSS 421.3 MB, written by the shim. Cost
  $0.0075 for both turns.
- **A shim killed with SIGKILL makes the phase `lost`,** 22 to 26 ms after the kill (supervisor poll plus pid check),
  with no `exit.json`. What happens to the child depends on the child:
  - pipe mode: the dead shim's stdin write end closed, the fake agent saw EOF and exited (`stdin-closed` in its
    log);
  - PTY mode: the child was gone within 600 ms. The PTY master closed with the shim, which is a hangup; SIGHUP is the
    probable cause [I];
  - a child that ignores stdin EOF (`sleep`): it stayed alive as an orphan, phase `lost` [M].
- The daemon's soft nofile limit was 1048576 when started from a shell, and the prototype set it to 8192
  (`up` event `nofile_before`/`nofile_after`). The limit under launchd was not read [U].
- Spawn cost 13 to 35 ms per run [M].

### Q4. One-shot launchd job against a plain setsid, from a real `.app` and from Terminal

[M: `q4-coalition.mjs`, `q4-terminal.mjs`; `out/q4-first.txt`, `out/q4-rest.txt`, `out/q4-terminal.txt`.]
`proc_pidinfo(pid, 20)` returns two coalition ids (resource and jetsam). The table is the resource coalition id.

| launcher | how the daemon started | launcher | daemon | shims and children | daemon ppid |
|---|---|---|---|---|---|
| S2Spike.app | setsid | 89073 | 89073 | 89073 | the app, then 1 |
| S2Spike.app | launchd one-shot | 89087 | 89089 | 89089 | 1 |
| Terminal (our shell) | setsid | 89105 (Terminal.app is 89105) | 89105 | 89105 | 1 |
| Terminal (our shell) | launchd one-shot | 89105 | 89147 | 89147 | 1 |

(The ids in the table come from different scenarios; within each scenario the launcher and the daemon were compared.)

- **A setsid daemon stays in its launcher's coalition; the one-shot launchd job puts the daemon in its own.**
  Shims and children follow the daemon, either way [M].
- The one-shot job: the plist (`RunAtLoad`, `KeepAlive` false, `AbandonProcessGroup` true, `ProcessType Interactive`,
  `PATH`/`HOME` in `EnvironmentVariables`) ran `s2d detach`, which forks the daemon with setsid and exits. The daemon
  answered 71.2 and 71.8 ms later (78.5 ms from Terminal); a first-ever launch took about 500 to 550 ms. Afterwards
  `launchctl bootout` succeeded, `job_gone` was true and the plist was removed, in every run
  (`bootout_ok`, `job_gone`, `plist_removed` all true) [M].
- **Cmd-Q (AppleEvent quit) and force-quit (SIGKILL of the app) did not take any daemon down.** In all four app
  scenarios, setsid included, the daemon, both shims and both children were alive 5 s later and the daemon still
  answered [M]. So this test does not reproduce herdr's coalition kill; it only shows that the coalitions differ.
- **Terminal window close** was emulated: AppleEvents to Terminal time out here (`-1712`, probably an Automation
  prompt that nobody can click), so one window was opened with `open -a Terminal term.command` and the close was
  replaced by SIGHUP to our shell and its child (what a closing window delivers). Terminal stayed alive; both daemons,
  all shims and all children survived, for setsid and for launchd [M]. A real window close and a real Cmd-Q of
  Terminal are unmeasured [U].
- Not measured: the real Force Quit dialog, logout, sleep, reboot, or anything that makes macOS end a coalition
  [U]. Whether macOS ends a coalition's members when its app quits is herdr's claim and stays [I] here.
- Environment: a launchd job's environment is clean by construction (launchd supplies `USER`, `LOGNAME`, `HOME`); a
  setsid daemon inherits the launcher's environment [I]. `claude -p` needs `USER` set: `env -i PATH HOME` gave
  `terminal_reason: api_error` with cost 0 three times out of three, and adding `USER` completed three times out of
  three [M: `probe-claude.mjs`].
- Socket path: a path through `src-tauri/../work/…` made the shim socket 106 bytes and the shim failed at once with
  `socket path … is 106 bytes; AF_UNIX allows 103`. Canonicalizing the path fixed it [M]. The failing fast is the
  right behaviour; the cause is a path that was never made absolute.
- Launchd's working directory is `/`, so a relative state path failed ("daemon did not answer within 15 s"). The
  prototype makes it absolute [M].

**Answer:** the launchd one-shot works from both launchers, costs about 70 ms, cleans up after itself and does give
the daemon its own coalition. The benefit it is meant to give (surviving Cmd-Q and force-quit) is not demonstrated,
because the setsid control also survived [M].

### Q5. Metrics for interactive sessions: OpenTelemetry against the transcript

[M: `q5-otel.mjs`, `q5-analyze.mjs`, `q5-dump.mjs`; `out/q5-run.txt`, `out/q5-analysis.txt`, `out/q5/`, `out/q5b/`.]
Setup: `CLAUDE_CODE_ENABLE_TELEMETRY=1`, `OTEL_METRICS_EXPORTER=otlp`, `OTEL_LOGS_EXPORTER=otlp`,
`OTEL_EXPORTER_OTLP_PROTOCOL=http/json` to `lib/otlp-receiver.mjs` (dependency-free node, one JSONL line per POST),
export intervals 1 s (logs) and 2 s (metrics), `OTEL_RESOURCE_ATTRIBUTES=collide.run=…`, the three `OTEL_LOG_*` flags
unset. One interactive haiku session, three prompts (a timed Bash command, a Read, a no-tool question).

**Tokens.**
- Main thread: 5 OTel `api_request` events against 5 unique `message.id`s in the transcript, identical: input 10,
  output 241, cacheRead 237141, cacheWrite 20417.
- OTel also reports calls the transcript does not contain: `generate_session_title` (1 call: in 1230, out 18) and
  `prompt_suggestion` (3 calls: in 1518, out 125, cacheRead 154709, cacheWrite 140).
- All four sources together: in 2758, out 384, cacheRead 391850, cacheWrite 20557, $0.0084977. That is exactly the
  transcript's `cost-state.modelUsage`, which is written **once, at exit** (a killed session may not have it).
- The metric `claude_code.token.usage` has delta temporality; summed over all exports it gives the same four totals,
  and `claude_code.cost.usage` sums to $0.008498.
- Only OTel has per-call `cost_usd`, `ttft_ms`, `duration_ms`, `request_id`, the 5-minute against 1-hour cache write
  split and `query_source`.
- **Verdict:** tokens are exact from both; OTel is complete (it sees the background calls that cost money) and live;
  the transcript is the only source for message content.

**Per-tool timings.** A Bash command whose own clock read 2011.7 ms:

| source | auto mode | `--permission-mode default` (`out/q5b`) |
|---|---|---|
| OTel `tool_result.duration_ms` | 4396 | 3310 |
| transcript `tool_use` to `tool_result` timestamps | 4791 | 3622 |

A Read: OTel 16 ms, transcript 100 ms. The transcript's `cost-state.totalToolDuration` (4412) equals the OTel sum
(4396 + 16).
- OTel's `duration_ms` is the tool call's wall time as Claude sees it, including shell spawn and, in auto mode, the
  permission classifier. It is not the command's own time: it was 1.3 s longer in default mode and 2.4 s longer in
  auto mode. The cause is inferred (shell startup, plus the classifier in auto mode; the PreToolUse hooks had
  finished before the duration started) [I]. Why exactly is unmeasured [U].
- Transcript timestamps run another 0.3 to 0.4 s later still (model stream and write latency), and Read shows 100 ms
  against 16 ms, so they are coarser for fast tools [M].
- So neither source gives a command's own time; OTel is the closer one and is a per-call number rather than a
  difference of two timestamps. The command's own time needs the tool's own reporting.

**Delivery.** Receiver clock minus event timestamp: logs p50 978 ms, max 1473 ms with a 1000 ms log interval;
metrics 2 to 114 ms with a 2000 ms interval (the point's timestamp is the interval end, so that is by construction).
The defaults are 5 s for logs and 60 s for metrics, so a live view needs the interval set [M for the 1 s runs; the
defaults' lag is [I]].

**What is in the stream with the three `OTEL_LOG_*` flags off** [M: `q5-analyze.mjs`]: `user_prompt` has
`prompt_length` and `prompt = "<REDACTED>"`. The prompt text, the Bash command text and the file name (`README.md`)
do not appear anywhere in the received bytes (five needles, five times false). Event names: `user_prompt`, `api_request`,
`tool_decision`, `tool_result`, `assistant_response`, `hook_registered`, `hook_execution_start`,
`hook_execution_complete`, `plugin_loaded`, `mcp_server_connection`, `managed_settings_resolved`. Metrics:
`session.count`, `token.usage`, `cost.usage`, `active_time.total`.
- Identity fields ride on every record (`user.id`, `user.email`, `user.account_uuid`, `user.account_id`,
  `organization.id`). The receiver should drop them on arrival; this record does not quote them.
- Attribution: `session.id` is on every record and `OTEL_RESOURCE_ATTRIBUTES` (`collide.run`, `collide.span`) comes
  through as resource attributes on every record [M].
- `--max-budget-usd` is print-mode only, so an interactive session's spend is bounded only by watching the stream
  (`--help` [I]).

**Answer:** use OTel for interactive sessions. It is exact where the transcript is, adds the calls the transcript
lacks, has per-call cost, ttft and tool duration, and arrives live; it needs the interval set and an identity filter
[M]. Keep the transcript for content and for the final `cost-state`.

### Q6. Does a script's own `claude -p` reach the receiver?

[M: `q6-script-phase.mjs`, `out/q6-run.txt`; `q5-otel.mjs` runs with `Q5_NAME=q6d` and `q6e`, `out/q6d`, `out/q6e`.]
- **Yes.** A plain node script spawned `claude-clean.sh -p … --output-format json` with `S1_PASS=
  CLAUDE_CODE_ENABLE_TELEMETRY`, the OTel variables and `OTEL_RESOURCE_ATTRIBUTES=collide.run=run-script-A`. The
  receiver got three requests (two log, one metrics) carrying that run id; one `api_request` event; token totals from
  the metric equal claude's own JSON usage exactly (in 2, out 4, cacheRead 16569, cacheCreation 15979).
- **A short `-p` flushes at exit.** With default intervals nothing was exported for 8.9 s and the process exited at
  19.8 s, yet logs (at 8.9 s and 17.8 s) and metrics (at 18.3 s) all arrived before it exited. With 1 s intervals,
  seven requests arrived (`interval-1s`).
- **Control:** the same call without the variables: 0 requests (`no-env`).
- The process needs `USER` in its environment (see Q4).
- **Inheritance does not work from inside an interactive session.** Claude's Bash tool environment keeps
  `CLAUDE_CODE_ENABLE_TELEMETRY` but has every `OTEL_*` variable stripped. A `claude -p` started from the Bash tool of
  a telemetry-enabled interactive session did not report a second session: the receiver saw exactly one `session.id`
  (the parent's) in both `q6d` and `q6e` [M]. A script (or agent) that wants its claude measured must set the OTel
  variables itself.
- Passing the variables through a container boundary is unmeasured [U].

**Answer:** `agentWork` script phases can be measured, if the phase's environment carries the receiver settings, the
enable flag and a run label. The tokens are exact.

### Q7. Session id after `/compact` and `/clear` in a take-over

[M: `q2-takeover.mjs`, `out/q2-run.txt`, `out/q2/hook-events.json`, `out/q2/hooks.jsonl`.] A `SessionStart` and
`SessionEnd` hook was added with `--settings`.

| event | hook | session id |
|---|---|---|
| take-over starts | `SessionStart` source `resume` | `744a1583-…` (the headless id) |
| `/compact` | `SessionStart` source `compact` | `744a1583-…` (same) |
| `/clear` | `SessionEnd` reason `clear`, then `SessionStart` source `clear` | `744a1583-…` ends, `d6c51a2a-…` starts |
| `/exit` | `SessionEnd` reason `prompt_input_exit` | `d6c51a2a-…` |

- `/compact` keeps the id; the word `PLUM-731` was still known afterwards [M].
- `/clear` starts a new session id and a new transcript file (`d6c51a2a-…`). OTel's `session.id` changed the same way
  (the receiver saw exactly those two ids) [M].
- A later headless `--resume`: with the original id, "What is the secret word?" returned `PLUM-731` ($0.0214, a cold
  cache). With the post-clear id it returned `unknown` ($0.0037): the new session holds only what happened after
  `/clear` [M].
- So the id to resume is the latest `SessionStart` id, and a `/clear` in a take-over makes the pre-clear work
  unreachable from the new id. The adapter must follow the id, from a `SessionStart` hook (which also gives `source`
  and `transcript_path`) or from OTel's `session.id`.

## Addendum: menu bar memory (2026-11-13, after the spike)

Added for the owner's menu bar decision (option (a): a menu bar icon inside the main app; closing the window keeps the
app running). Same versions as above; debug, unsigned build.

- **Method.** `S2_MENUBAR=destroy|hide` puts the app in a test mode (`src-tauri/src/lib.rs`, module `menubar`): a
  tray icon with a Quit item, no daemon, the idle xterm page, then a scripted sequence of window open 20 s, close
  (destroy the window, or hide it) and switch to the accessory activation policy, 45 s closed, reopen (a new window,
  or show), 20 s, exit. `scripts/menubar-mem.mjs <mode>` launches the app with `open -n --env`, follows the app's
  marks and samples `footprint` (phys_footprint) and RSS of the app plus every WebKit process in the app's resource
  coalition (`s2d coalition`). Output: `out/menubar/menubar-<mode>.json`.
- **Results** [M: `menubar-mem.mjs`], total footprint, MiB:

  | State | destroy on close | hide on close |
  | --- | --- | --- |
  | Window open (idle xterm page) | 138.2 | 137.5 |
  | Closed +5 s | 57.2 | 146.3 |
  | Closed +35 s | 53.0 (app 33.1, WebKit GPU 14.0, Networking 5.9) | 143.0 |
  | Reopened | 138.6 | 132.5 |
  | Reopen time | 267 ms to page loaded | returns at once |

  - Destroying the window ends the WebContent process (76 MiB here); the GPU and Networking processes stay while the
    app runs. Hiding it keeps everything. RSS undercounts WebContent (20 MiB RSS against 76 MiB footprint).
  - Both runs exited cleanly, and no WebKit process of the app's coalition was alive 3 s after exit.
- **Inferred** [I]: a real Collide page is heavier than the idle xterm page, so the open figure and the reopen time
  grow with it; the closed figure does not, because the page's process is gone. A separate tray-only helper with no
  WebKit would save roughly the 20 MiB of GPU and Networking processes plus part of the app's 33 MiB, not measured.
- **Unmeasured** [U]: a release build; time to visible pixels after reopen; whether the GPU and Networking processes
  can be released while the app keeps running; memory over hours.

## Proposed plan changes

Plan text cited by section. None applied.

1. **0.2, Starting the daemon (coalition).**
   - Keep the one-shot launchd job; it is measured to work from an `.app` and from Terminal in about 70 ms, with the
     daemon in its own coalition (Q4).
   - But the plan's reason ("macOS can end it when the app quits") is not reproduced: a setsid daemon survived an
     AppleEvent quit and a SIGKILL of the app (Q4). Reword to "puts the daemon in its own coalition; the
     failure it prevents was not reproduced by quit or force-quit, so the job is insurance" and keep the Force Quit,
     logout and sleep cases on the unmeasured list.
   - Add: the plist's working directory is `/`, so every path passed to the job must be absolute and canonical; a
     state path that makes the socket exceed 103 bytes must fail at start with that message (Q4: 106 bytes failed
     fast).
   - Add: the job and the daemon get `USER` (and `HOME`, `PATH`) explicitly. `claude` fails with `api_error` without
     `USER` (Q4).
   - The nofile line stays; the measured limit was 1048576 under a shell, so the raise matters only where the limit is
     low. The launchd limit was not read (unmeasured).
2. **0.2, the shim.**
   - Note the three outcomes of a killed shim: pipe child gets EOF; PTY child gets a hangup; a child that ignores
     both stays as an orphan under phase `lost`. The daemon should sweep the orphan (the plan says "sweeps the run's
     marker"; add the child's pid) (Q3).
   - A PTY shim keeps a ring buffer (1 MiB here) and replays it on attach, atomically with subscribing. Say so, since
     a take-over or New session attach after an app restart depends on it (Q2).
3. **0.11 and Stage 9 (Take over; New session).**
   - "A take-over ends when the terminal closes" (0.11) and "Closing the terminal ends the phase" (Stage 9, Take
     over): in the prototype, closing the view only detaches and the run goes on, which is what makes a take-over
     survive an app restart (Q2). Decide it: either the app ends the run on an explicit close action and not on view
     close or app quit, or the plan says the run outlives the view. Recommended: detach on close, end on an explicit
     action, so an app restart does not end the owner's session.
   - A fresh worktree is a sibling of the trusted checkout, so interactive Claude opens at a workspace-trust dialog
     whose default is "No, exit" (Q2). New session and take-over need either the trust pre-accepted (unmeasured how)
     or the terminal showing the dialog and the app not sending a blind Enter.
   - A take-over has no `--max-budget-usd`; its spend is only observable (Q5). The phase's budget cannot be enforced
     there.
   - Follow the session id: `SessionStart` hook or OTel `session.id`; `/compact` keeps it, `/clear` replaces it (Q7).
     After a `/clear`, the earlier work is not in the new id, so the phase's resume id must be updated, and the
     "interactive take-over, then headless resume" path should resume the newest id.
   - xterm renderer: DOM is as fast as WebGL here and simpler (Q1); the plan need not require WebGL.
4. **0.13, Interactive sessions row.**
   - Confirm OpenTelemetry as the source (Q5): exact tokens equal to the transcript's, plus the title and
     prompt-suggestion calls the transcript lacks, per-call cost, ttft and tool `duration_ms`.
   - State what the tool duration is: wall time of the tool call including shell spawn (and the classifier in auto
     mode), not the command's own time (Q5).
   - Add the intervals: set `OTEL_LOGS_EXPORT_INTERVAL` and `OTEL_METRIC_EXPORT_INTERVAL` (defaults 5 s and 60 s) to
     get a live view (Q5).
   - Attribute by `session.id` and by `OTEL_RESOURCE_ATTRIBUTES=collide.run=…` (Q5). The receiver drops the
     identity attributes (`user.email` and the ids) on arrival.
   - "Inherited `OTEL_*` variables are stripped": measured to be the case from the other side too: claude strips
     them from its own tools' environments, so nested `claude` processes do not report (Q6).
   - Spend: the transcript's `cost-state` appears only at exit, so keep OTel's `cost.usage` as the live figure.
5. **0.3, `agentWork` script phases (and 0.13 Reported row).** Q6 confirms measurement works for a script's own
   `claude -p` when the phase's environment has the receiver's settings and the enable flag, and that a short run
   flushes before it exits. Scripts in containers stay unmeasured (the pack's job). The adapter must set the
   variables itself because the Bash tool strips them.
6. **Stage 1, S2 bullets.** Mark the questions measured as above; list `screencapture -l`, Force Quit UI, Terminal
   quit and a real window close, containers and logout or reboot as still unmeasured. Keep `S2` open only for those
   if the owner cares.

## Not measured

- `screencapture -l <windowid>`: refused for lack of Screen Recording permission; WKWebView snapshots stand in.
- The DOM key-event path (the bench injects through `term.input()`), and the time from rAF to photons.
- Renderer comparison on a real display under load, or with a long scrollback.
- A real Force Quit dialog, Cmd-Q by keystroke, a real Terminal window close and a Terminal quit (AppleEvents to
  Terminal time out; SIGHUP stands in). Logout, sleep and reboot.
- Whether macOS ever ends a coalition's members when its app quits (herdr's claim).
- The nofile limit under launchd.
- A take-over of a headless process that is still running (the headless run had ended).
- Pre-accepting the workspace-trust dialog.
- OpenTelemetry from inside a container.
- Why OTel's tool duration exceeds the command's own time (cause is inferred).
- A shim whose child exits while the daemon is down starting the daemon (plan 0.2); not built.

## Spend

Notional `total_cost_usd` (claude.ai subscription), the last `cost-state` per transcript under
`~/.claude/projects/*s2-interactive*` (`scripts/spend.mjs`): **$0.0938 over 28 transcripts** (the cap was $2).
Interactive sessions ended with `/exit` (or were stopped) so that their `cost-state` was written. Telemetry and
background calls are included in the figure (Q5). Nothing was spent on the Q3, Q4 or resize checks that do not start
claude.

## Left behind

- Two Terminal windows from `open -a Terminal` (`ping.command` and `term.command`) may remain open, with their shell
  processes ended; AppleEvents to Terminal time out, so they could not be closed from here.
- Everything else I started (daemons for `work/state*`, shims, children, receivers, app instances) was killed, no
  `dev.collide` launchd job is loaded and no plist remains under `work/` (checked at the end).
