---
id: ts-0197-uj
kind: task
status: ready
title: "Owner check: the board and S6's walkthrough"
owners: []
links:
  - { type: blocked-by, to: ts-0217-dy }
---

## Goal

The Stage 6a checks that only the owner runs: the walkthrough of S6's scenario 2, and scenario 1 up to the board, in
the bundled app, timed beside S6's advisory limits; and the checks that only the bundled app shows (the CSP and
capabilities in the real webview, the folder picker, external links through the OS, the copy buttons, a secret in
the real keychain).

A build agent never runs it. It never runs `tauri build`, never starts the app or a daemon against the owner's data,
and never runs `claude`. It writes the command lines and a short checklist under `### Owner checklist`, from the
code and from the Built sections of `ts-0179-ke` to `ts-0196-uw`. The owner runs them and records the result here
as owner judgement, with what was awkward.

## Success criteria

- The owner's walkthrough of S6 scenario 2, and scenario 1 up to the board, is recorded: the time per scenario
  beside S6's advisory limit (S1 150 s, S2 60 s), the errors, and what was awkward (R5). Owner-run, in the bundled
  app. S6's thresholds are advisory (Stage 1).
- The bundled app's webview runs under the CSP, and a page script cannot call any command but `rpc`. Owner-run.
- An external link asks first, then opens in the default browser. Owner-run.
- "Open a folder…" and "Locate the folder…" open the system folder picker. Owner-run.
- A Copy button on the first-run page puts its command on the clipboard. Owner-run.
- A secret set from Settings lands in the login keychain under Collide's service, and Settings shows it Set, never
  its value. Owner-run, with a throwaway value the owner removes afterwards.
- The differences from the mock that `ts-0185-ge` recorded are each closed or carried. Owner-run.
- The checklist holds the command lines, with `<collide checkout>` and `<scratch repo>` as placeholders, what to look
  at, how to clean up, and a line for the owner's result and one for what was awkward.
- A line records the date Stage 6a is judged done by the owner.

## Tests

None by a build agent: this check is the owner's. The checklist names the automated tests it relies on:

- `ts-0179-ke`: the frame and the fake-owner page;
- `ts-0180-ad` to `ts-0182-ad`: the wire, the Field and live updates;
- `ts-0183-ee`, `ts-0184-hv`: the webview rules and the drawer;
- `ts-0186-ch`, `ts-0187-hb`: `unblocks`, `bottlenecks`, run order and graph;
- `ts-0188-kb`, `ts-0189-fg`: Changes, the diff window and the id peek;
- `ts-0190-tz`, `ts-0191-nv`: the filter bar and views; boards, missing and locate;
- `ts-0192-es` to `ts-0194-qp`: the first-run checks, Settings, the empty board, Share with team and trust;
- `ts-0195-ju`: Planning;
- `ts-0196-uw`: the S6 smoke run.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 6a: Success Criteria (S6's scenarios and the owner's walkthrough; the external link; the CSP); the Tauri
    shell bullet.
  - R5; Stage 1 (S6's thresholds are advisory). Part L.
- What the checklist covers. The build agent fills in the exact commands once the earlier items are built.
  - **Set up.** The owner builds the bundled app (`tauri build`, about 40 minutes) from `<collide checkout>`. Make
    `<scratch repo>` in a folder made for the check. Say which steps use the real `HOME` (the bundled app does), and
    how to remove what they leave.
  - **Scenario 1.** Open `<scratch repo>` as a fresh folder; the checks; the empty board; Start with a task. Time
    it.
  - **Scenario 2.** The S2 fixture board of `ts-0196-uw`; the chips; Save as a view "Slow spots". Time it.
  - **Bundled-app checks.** The CSP (the webview's console with the hostile fixture task); the capability list; an
    external link; the folder picker; Copy; a secret set and removed; Start with defaults from the app menu and from
    its key with the page broken by a fixture, then Reset… (`ts-0215-gt`, revision 29).
  - **Record.** The times beside the limits, the errors, what was awkward, and the date.
- **Owner's choice: the S2 board for the walkthrough.** Recommend the fixture board of `ts-0196-uw`, so the five
  targets are known. Alternative: a copy of a real board with labels added by hand.
- Area: this item's own text. No code.
- The test rule, for anything automated this check leans on: no test touches the real home, real launchd, a real
  daemon, the keychain or the network. Each test that runs the `collide` binary clears the environment
  (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as `host/tests/common/mod.rs` `TempRoot`
  does. Keychain writes in tests go through `MemorySecretStore`, never the login keychain. The owner's own run here
  uses the bundled app, the real `HOME` and the login keychain by choice, where the checklist says so.
- Stage 6a item (revision 25 scope, split 2026-11-24). Items of Stage 6a run one at a time, in id order, because they
  share `app/`, `core`, `host` and the generated files.
- Nothing here runs a real `claude`. Scenario 1 stops at the board, before any run.
- Owner decisions (2026-11-24): the owner took the recommendations at the split.

### Owner checklist

Written by the build agent once `ts-0196-uw` is done.
