---
id: ts-0082-fp
kind: task
status: ready
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
