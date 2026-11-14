# Spike S2: interactive sessions

Throwaway reproduction for `.collide/planning/spikes/sp-0002-gm-interactive/record.md`. A Rust prototype daemon and shim, a minimal Tauri 2 app
with xterm.js, and node drivers. Nothing here is product code.

Requirements: macOS, Node >= 22 (measured on 24.18.0), pnpm, Rust (`. "$HOME/.cargo/env"`), `claude` on PATH with a
login. Everything installs locally: `pnpm install --store-dir ./.pnpm-store`, cargo into `target/`.

## Build

```
pnpm install --store-dir ./.pnpm-store
scripts/build-frontend.sh                       # copies xterm.js and addons into app/dist
cargo build -p s2lib                            # target/debug/s2d (daemon, shim, CLI)
pnpm exec tauri build --debug --bundles app     # target/debug/bundle/macos/S2Spike.app
swiftc -O scripts/windowid.swift -o work/bin/windowid   # only used to list window ids
```

## Layout

- `crates/s2lib/`: `s2d daemon` (adopts shims, supervises, relays attaches), `s2d shim <run dir>` (setsid, PTY or pipe
  child, wait4, `exit.json`, ring buffer replay), `s2d start --how setsid|launchd`, `s2d coalition <pid>...`,
  `s2d rpc '<json>'`. Frame protocol in `frame.rs`.
- `src-tauri/`: the app. Starts the daemon on demand (setsid or one-shot launchd job, from `work/state/app.json`),
  exposes `attach`/`write_input`/`resize` over `ipc::Channel`, and a `snapshot` command (WKWebView snapshot).
- `app/`: the page (`app.js` runs a scripted bench from `app.json`).
- `lib/`: node client, app driver, OTLP receiver (`otlp-receiver.mjs`), fake agent.
- `claude-clean.sh`: copy of S1's wrapper.
- `scripts/`: one script per question.

| Script | Question | Spends |
| --- | --- | --- |
| `q1-node-latency.mjs`, `q1-app.mjs [cat\|claude]`, `q1-resize-stty.mjs` | 1: latency below the webview and through the app; fidelity and resize screenshots | small (interactive haiku) |
| `q2-takeover.mjs` | 2 and 7: take-over, app and daemon restart, `/compact`, `/clear`, later headless resume | about $0.05 |
| `q3-daemon-restart.mjs`, `q3b-real-claude.mjs` | 3: daemon SIGKILL and restart, SIGKILLed shim; a real `claude -p` across a daemon restart | none; about $0.01 |
| `q4-coalition.mjs [scenario...]`, `q4-terminal.mjs` | 4: coalition ids, Cmd-Q, force-quit; Terminal close (emulated with SIGHUP) | none |
| `q5-otel.mjs`, `q5-analyze.mjs`, `q5-dump.mjs` (`Q5_NAME`, `Q5_PROMPT`, `Q5_EXTRA`, `Q5_ONLY_BASH` select variants: q5b default permission mode, q6d and q6e nested claude) | 5: OpenTelemetry against the transcript | about $0.02 |
| `q6-script-phase.mjs` | 6: a script's `claude -p` reaching the receiver | about $0.01 |
| `menubar-mem.mjs destroy\|hide` | addendum: menu bar memory with the window destroyed or hidden (`S2_MENUBAR` test mode in the app) | none |
| `spend.mjs` | total spend from the transcripts' `cost-state` records | none |

Run output goes to `out/` (screenshots in `out/shots/`), scratch state to `work/`. Both are git-ignored.

Notes:

- `screencapture` is refused in the environment this ran in (no Screen Recording permission), so the app snapshots its
  own webview with `WKWebView.takeSnapshot`, which needs none.
- The scripts launch `S2Spike.app` and quit it with `osascript`. `q4-terminal.mjs` opens one Terminal window with `open -a Terminal` and emulates the close with SIGHUP (AppleEvents to Terminal time out here).
- launchd jobs are labelled `dev.collide.spike.s2.*`, with plists under `work/`, and are booted out as soon as the daemon
  answers.
