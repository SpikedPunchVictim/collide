---
id: ts-0196-uw
kind: task
status: ready
title: "Browser: S6 scenarios 1 and 2 ported, and the smoke run"
owners: []
links:
  - { type: blocked-by, to: ts-0195-ju }
---

## Goal

S6's scenario 2, and scenario 1 up to the board, pass as Playwright tests ported from S6's pilots, against the real
daemon through the bridge. Each completes and detects its end state, as S6's detectors do. Together they are Stage
6a's one Playwright smoke run.

What exists.
- S6's prototype (`.collide/planning/spikes/sp-0009-bb-interface-prototype/`): the scenario cards in
  `SESSION-PROTOCOL.md` ("Scenario 1. Fresh repo to a reviewed run"; "Scenario 2. Build a view and save it"), the
  start states and end-state detectors in `src/scenarios.ts` (`S1`, `S2`, `S2_TARGET`), and the pilots in
  `test/pilot.test.ts` and `test/ui.test.tsx`. The advisory limits are in `record.md` (S1 150 s, S2 60 s; owner,
  2026-11-14).
- The bridge's Playwright runner (`app/playwright.config.ts`, `app/browser/support/`), with the installed Chrome.
- From the earlier 6a items: the first-run page, the empty board, Start with a task (`ts-0194-qp`); the filter bar
  with its Labels group and Save as a view (`ts-0190-tz`).

What this adds.
- `app/browser/s6.spec.ts`, one file, both scenarios:
  - Scenario 1, up to the board: a fresh temp repo with no `.collide/` and a fresh daemon root; open the folder; the
    checks pass with the world's fake `claude`; the board opens with no setup; the empty board shows; Start with a
    task with the card's words ("Add a --quiet flag to the CLI"); the end state: the board shows that scratch task.
    Launching it and reviewing the run are Stage 9a's.
  - Scenario 2: a board whose tasks match S6's start state for S2 (epic E-07, statuses, the label perf; the five
    targets of `S2_TARGET`), from a fixture pack whose tasks carry labels; the chips Stage E-07, Where it stands
    Ready, Labels perf; Save as a view named "Slow spots"; the end state, detected as S6's `S2.detect` does: a saved
    view named "Slow spots" whose members are exactly the five targets, read back with `task.list`.
  - S6's wrong-filter case: saving with only Stage E-07 does not count as done.
- Each scenario's time from its first action to its detected end, printed beside its advisory limit. The times are
  information only: a slow machine never fails the test.

## Success criteria

- S6 scenario 2, and scenario 1 up to the board, pass as UI tests ported from S6's pilots against the real app,
  each completing and detecting its end state (Stage 6a).
- The smoke run needs no network, no `github` pack and no real `claude`.
- The wrong-filter case does not detect.

## Tests

Write these first.

- `app/browser/s6.spec.ts` as above (Stage 6a test "one Playwright smoke run of scenarios 1 and 2").
- `fixtures/packs/`: the S2 fixture pack's own small test, if it has logic.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 6a: Success Criteria ("S6 scenario 2, and scenario 1 up to the board, pass as UI tests ported from S6's
    pilots against the real app"); Tests ("one Playwright smoke run of scenarios 1 and 2").
  - Stage 1 and R5: S6's thresholds are advisory.
- Design: the scenario cards. The mock is not the reference here; the built app is.
- **Decision for the item, with a recommendation: "against the real app".** Recommend the bridge page over a real
  daemon, which runs the same React app and owner protocol as the window. The bundled window is the owner's, in
  `ts-0197-uj`. Alternative: driving the Tauri window with WebDriver; it needs `tauri build` and a new driver.
- **Decision for the item, with a recommendation: the S2 board.** Recommend a small fixture pack in `fixtures/packs/`
  with S6's E-07 tasks and labels, since no built-in source gives labels. Alternative: `collide/plan` reading a
  `labels:` list from frontmatter; a pack change no 6a criterion asks for.
- Carried: `ts-0082-fp` (the bridge runner and the installed Chrome are what "Stage 6a's Playwright smoke run
  reuses").
- The owner's walkthrough and its times are `ts-0197-uj`'s.
- Area: `app/browser/s6.spec.ts`, `app/browser/support/`, `fixtures/packs/` (the S2 fixture).
- Dependencies: none.
- The test rule: no test touches the real home, real launchd, a real daemon, the keychain or the network. Each test
  that runs the `collide` binary clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to
  a temp root, as `host/tests/common/mod.rs` `TempRoot` does. A temp repository sets `GIT_CONFIG_GLOBAL` and
  `GIT_CONFIG_SYSTEM` to empty files. Keychain writes go through `MemorySecretStore` (`host/src/secret_store.rs`),
  never the login keychain. Browser tests run the installed Chrome with a temp profile
  (`app/browser/support/chrome.ts`), and the bridge's daemon on a temp root (`app/browser/support/world.ts`). The
  spike's `out/` folder is never read.
- Stage 6a item (revision 25 scope, split 2026-11-24). Items of Stage 6a run one at a time, in id order, because they
  share `app/`, `core`, `host` and the generated files. No builder runs `tauri build`.
- Nothing here runs a real `claude`: the checks find the world's fake `claude`.
- Owner decisions: none recorded yet. The recommendations stand until the owner answers at the split.
