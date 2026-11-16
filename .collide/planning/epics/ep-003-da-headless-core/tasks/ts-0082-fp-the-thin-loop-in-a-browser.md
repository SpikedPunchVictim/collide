---
id: ts-0082-fp
kind: task
status: done
title: "The thin loop in a browser, against fake Claude"
owners: []
links:
  - { type: blocked-by, to: ts-0081-wm }
---

## Goal

Stage 3c's criteria, end to end, in a real browser: the page of `ts-0081-wm`, over the generic `rpc` command, to a
temp-root daemon whose `claude` is fake Claude.

- **A test-only bridge.** A small program that:
  - starts a daemon in a temp root, with fake Claude first on its `PATH` and a fixture repository;
  - hosts the shell's `Shell` (`app/src-tauri/src/shell.rs`), so a call takes the same path as from the webview,
    with `SocketLink` to the daemon;
  - serves the built page, the `rpc` call and the event channel on loopback HTTP.
  It is never part of the app or its bundle.
- **A browser client** for the bridge, beside `app/src/owner/tauri.ts`, used only by the test build.
- **A browser runner**, per the decision below.
- **The scenarios** of the Stage 3c criteria.

## Success criteria

- In a browser, a prompt creates a scratch task and starts a `collide/quick` run in a fresh worktree. The page shows
  "ready to review" with the run's diff (Stage 3c criterion).
- Accept merges the branch into the fixture checkout (`git log`) and removes the worktree. The scratch task is
  `done`.
- Re-prompt continues the same session, and a second review follows.
- Discard removes the worktree and leaves the checkout unchanged (`git status`, `git log`).
- A permission request from the run shows on the page. Answering it lets the run continue (Stage 3c criterion).
- Reloading the page mid-run, and again at review, shows the same run and the same review item (Stage 3c
  criterion: closing the window and reopening it).
- The bridge answers only on loopback. It refuses a foreign `Host` or `Origin`, and a call without the run's
  random token.
- The tests run in the default test command, with no network and no download, and leave no process behind.

## Tests

Write these first, one browser test for each:

- the loop: prompt, review, accept;
- re-prompt, then accept;
- discard;
- a permission request, allowed;
- a reload mid-run, and a reload at review;
- the bridge's refusals (a foreign `Host`, a foreign `Origin`, no token).

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 3c: Success Criteria and Tests (the loop over `rpc` in a browser against the fake Claude).
  - 0.2: The UI speaks the owner's protocol (UI tests run in a browser).
  - Stage 6a: The UI's tests run in a browser against a fake owner, with no Tauri; one Playwright smoke run.
  - S6 record (`.collide/planning/spikes/sp-0009-bb-interface-prototype/record.md`): the loopback endpoint's checks
    (a foreign `Host` and `Origin` refused, a JSON content type).
- **Decision for the owner, with a recommendation: the browser runner.** The repository has no browser runner yet,
  and `app/test` says click handling needs a browser. Recommend `@playwright/test` as a dev dependency, driving the
  installed Chrome (`channel: 'chrome'`), so no browser is downloaded and Stage 6a's Playwright smoke run reuses it.
  The alternatives are vitest's browser mode (Playwright underneath), or a Chromium that Playwright downloads to a
  folder in the repository.
- **Decision for the item, with a recommendation: where the bridge lives.** Recommend a binary of `app/src-tauri`
  behind a cargo feature that the app build never turns on, so it can use `Shell` and the host's daemon. It starts
  the daemon in its own process, as `host/tests/adapters/claude/run_command.rs` does.
- The Stage 3c criterion says a browser over a fake Claude. 0.2 and Stage 6a say a browser over a fake owner. The
  bridge gives a real daemon behind the browser; the page's own tests over `FakeOwner` are `ts-0081-wm`'s.
- The bridge puts fake Claude on the daemon's `PATH`, with a recording for each scenario. Code outside the adapter
  must not spell the Claude variables (the names check).
- Area: `app/src-tauri` (the bridge), `app/src/owner` (the bridge client), a browser test folder in `app`,
  `app/package.json`.
- The test rule: no test touches the real home, real launchd or a real daemon. Each test clears the environment
  (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as `host/tests/common/mod.rs` `TempRoot`
  does. Settings files are fixtures, never the owner's own `settings.json`. The browser runs with a temp profile.
- Stage 3c item (revision 22 scope, split 2026-11-15). Items of Stage 3c run one at a time, in id order, because
  they share `host`, the app and the generated files.
- Owner decisions (2026-11-15, at the split): the browser runner (`@playwright/test`, a new dev dependency, driving
  the installed Chrome). The recommendation, taken under the owner's delegation; the owner may change it before the
  item is built.

### Built

- **Browser runner.** `@playwright/test` 1.64.1 (published 2026-10-14), pinned exact in `app/package.json`, installed
  with `PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD=1`; no browser is downloaded. `app/playwright.config.ts`: one worker, no
  retries, `forbidOnly`, global setup. `app` script `test` is now `vitest run && playwright test` (so `pnpm test`
  runs the browser tests); `test:unit` and `test:browser` run each half.
- **The bridge.** A binary `collide-bridge` of `app/src-tauri` (`src/bin/collide-bridge/`), with
  `required-features = ["test-bridge"]`; the app build never turns the feature on, and nothing is added to Cargo
  (the bridge uses `std` and the workspace's own crates).
  - `main.rs`: arguments, token (16 random bytes as hex), loopback listener on a system-picked port, `Hub`
    (`EventChannel` to server-sent events), `Bridge::route` (static files, `GET /events`, `POST /rpc`,
    `POST /shutdown`), `Bridge::finish` (stops live runs with `run.stop`, then the daemon, then exits), a parent-pid
    watchdog. It hosts `Shell`, `SocketLink`, `DaemonAccess` and `pump_daemon_events` from `shell.rs` unchanged.
  - `http.rs`: `read_request`, `Policy` (`check_origin`: Host must be the bridge's own loopback name and port, Origin
    absent or the bridge's own; `check_token`: bearer, constant-time compare), `check_json`, `file_for`
    (no `..`, `%`, `\`). Seven unit tests.
  - `daemon.rs`: `ChildDaemon` (`DaemonStarter`): `collide-daemon` as a child with `env_clear()` and `PATH`, `HOME`,
    `COLLIDE_DAEMON_ROOT` (root, both), `SHELL`; ended with `SIGTERM`, then a kill.
- **The client.** `app/src/owner/bridge.ts` (`createBridgeOwner`, `readEvents`), beside `tauri.ts`; entry
  `app/src/bridge-main.tsx` and `app/bridge.html`, built by `app/vite.bridge.config.ts` into a temp folder, so the
  app's `index.html` build never reads them. `app/test/bridge-owner.test.ts` (5 client tests, and one that only
  `bridge-main.tsx` reads the client and `index.html` does not name it).
- **The runner side.** `app/browser/support/`: `world.ts` (`World`: temp root `/tmp/cdb-*`, fake `claude` script
  and login shell, fixture repo, bridge launched detached so no test process is among its parents, `stop()` that
  sweeps for anything naming the root), `chrome.ts` (`Chrome.launch`: the installed Chrome, `channel: 'chrome'`,
  headless, a temp profile, loopback-only host rules; fails with a message that Chrome must be installed, never
  skips), `fixtures.ts` (teardown on pass and fail; a left process fails the test), `recordings.ts` (fake Claude's
  recordings, derived from the host's). `global-setup.ts` builds the binaries and the bridge page.
- **Browser tests** (`app/browser/`), all seen red first (the first run failed in global setup, because the cargo
  feature and the bridge did not exist; a later red was the page, below):
  - `loop.spec.ts`: "a prompt runs in a fresh worktree and ends in a review that accept merges"; "a re-prompt
    continues the same session and a second review follows"; "discard removes the worktree and leaves the checkout
    as it was"; "a permission request shows what the agent would use, and allowing it lets the run go on"; "a page
    loaded again mid-run and at review shows the same run and the same item".
  - `bridge.spec.ts`: "the bridge refuses a foreign Host, a foreign Origin and a call without the token"; "a page of
    another origin cannot call the bridge".
  - Mutation checks on the bridge, each seen red and then restored: the Host check off (expected 403, got 200), the
    token check off (expected 401, got 200), the Origin check off (expected 403, got 200).
- **A bug the first browser run found in the page (ts-0081-wm).** The page passes `loop.openFolder` and the other
  methods to components as callbacks, and the controller's methods lost `this`, so "Open folder" did nothing in a
  real browser (the vitest tests call the methods on the object). Fixed in the `LoopController` constructor in
  `app/src/loop/loop.ts` (each prototype method is bound); test "a method handed to a component as a callback still
  works, as the page hands them" in `app/test/loop-page.test.ts`, seen red without the fix.
- The criteria: accept merges (git log has the commits) and the worktree is gone and the task is `done`; a re-prompt
  launches with `--resume` of the first session; discard leaves `main` and the checkout as they were; a reload shows
  the same run and item; the bridge refuses a foreign Host or Origin and a call without the token. The tests need
  no network, download or process left behind (checked in each teardown).

### Decisions for the owner

- **The `test-bridge` feature is not in the default gates.** `cargo build/test/clippy --workspace` skip the bridge
  (`required-features`). It is built by `app/browser/global-setup.ts`, so `pnpm test` covers it, and I ran
  `cargo test` and `cargo clippy -D warnings` with the feature by hand. Alternative: add the feature to the
  workspace gates. Recommend keeping it out, and adding `--features collide-app/test-bridge` to the clippy gate
  when CI is written.
- **Std-only HTTP** in the bridge, in about 300 lines, with no HTTP crate. Alternative: a crate such as `tiny_http`
  (a new dependency, which the item forbids). Recommend std.
- **A child daemon** (`ChildDaemon`) in place of the app's `LaunchStarter`: it never uses launchd and never
  outlives the test. Alternative: the launchd path in a temp root (real launchd, forbidden).
- **Shutdown** is `POST /shutdown` plus a parent-pid watchdog, because the bridge has no libc for signals.
  Alternative: add `libc` or `ctrlc` (a dependency). Recommend as is.
- **The token is in the URL fragment** (`#token=...`), which no request carries; the page sends it as a bearer.
  Alternative: a cookie (then the Origin check is the only guard). Recommend the fragment.
- **Events over `fetch` and server-sent events**, with the subscribe promise resolving when headers arrive.
  Alternative: `EventSource` (cannot send the bearer). Recommend as is.
- **A separate vite config and entry** for the bridge page, so no bridge code reaches the app bundle. Alternative: a
  mode flag in the one config. Recommend separate.
- **`--host-resolver-rules` in Chrome** map everything but loopback to nothing, so a stray request cannot leave the
  machine. Recommend keeping it.
- **One worker.** Each test starts a daemon, and the suite takes about 15 s. Alternative: more workers (each world is
  isolated, so it works). Recommend one until the suite is slower.
- **Pin.** 1.65.0 was under two weeks old on 2026-11-15, so 1.64.1 is pinned. Revisit at the next upgrade.
- **The page and the "Sent back" notice.** After a re-prompt the daemon makes a new run, the page follows it, and
  a run change clears the notice. With fake Claude's fast second phase the person would never see "Sent back".
  The test asserts the second review, not the notice. Alternative: keep a notice across the change in ts-0081's
  state (`afterRunChange` in `app/src/loop/state.ts`). Recommend a small item for ts-0081's owner.
- **Plan text, proposed** (Stage 3c, tests): "A browser test drives the page against a real daemon through the
  test-only bridge (a binary behind the cargo feature `test-bridge`); the page's own tests stay over `FakeOwner`."
  Not applied: `IMPLEMENTATION_PLAN.md` is not edited here.

- Owner decisions (2026-11-15, at the build): decisions taken under the owner's delegation, as recommended. The plan
  text applied, reworded, to Stage 3c's Tests. Review: the main session's gate list now also runs clippy and the
  tests with `--features test-bridge`. The callback fix in `LoopController`'s constructor (a page bug from
  ts-0081-wm) was changed in review from a loop over the prototype to one explicit `bind` per action. The "Sent back"
  notice that a run change clears is a follow-up item of its own. The builder's scratch folder under `/tmp` was left:
  a safety check blocked its removal, and it is the owner's to delete.

### Not done, by design

- No Linux or Windows run; the bridge's tests use `/usr/bin/python3` and Chrome on macOS only.
- No real `claude`, real launchd, real home, or the person's Chrome profile is touched, and none is needed.
- The bridge serves the page and the owner's `rpc` only; it has no UI-tools endpoint (its manifest is empty, as the
  app's is until Stage 6a).
- Stage 6a's Playwright smoke run over `FakeOwner` is not built; this item's runner is what it will reuse.
- No other npm or cargo package was added.
