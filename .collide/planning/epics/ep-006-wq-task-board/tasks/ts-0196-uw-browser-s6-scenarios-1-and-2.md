---
id: ts-0196-uw
kind: task
status: done
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
- Owner decisions (2026-11-24): the owner took the recommendations at the split.

### Built

- `app/browser/s6.spec.ts` (new, three tests, bridge, real daemon, installed Chrome). Its header table maps each S6
  control to the app's.
  - Scenario 1, up to the board: the world's repository (no `.collide/`) and a fresh daemon root; type the folder,
    Check this folder; `git_repo` and `claude_installed` are `ok` and no row is `no` (the login stays "could not
    check", never a failure; no way to run the probe was added); Open the board; the empty board shows once the board
    settles; Start with a task with "Add a --quiet flag to the CLI"; the end state is the card on the board and
    `task.list` holding one `scratch` task with that title.
  - Scenario 2: the board comes from the `s6-board` fixture pack (below), copied into the temp repo's
    `.collide/packs/`, named in `.collide/board.yaml`, committed, opened and trusted (`board.trust_review`, then
    `board.trust`, as `octo-board.ts` does; the daemon's rule is unchanged). The start state is asserted first: the
    CEL for epic E-07, ready and perf lists exactly the five of `S2_TARGET`. Then the epic's funnel, the Ready chip,
    the perf chip, Save as a view…. The card's name "Slow spots" is tried first and refused (Decisions); then
    "Slow-spots". The end state is `slowSpotsDetected`: `task.views` names, and for each name that reads as Slow
    spots, `task.list` with that view's name gives exactly the five.
  - Wrong filter: only the epic, picked through the Epic box this time, saved as "Slow-spots": the view is there, holds
    all 13 tasks of E-07, and `slowSpotsDetected` is false.
  - Times: `report` prints each scenario's time from its first action to its detected end beside its advisory limit
    (S1 150 s, S2 60 s) and adds it to the test's annotations. Nothing asserts a time.
  - Full-suite run on this machine: S1 3.5 s (limit 150 s); S2 2.1 s (limit 60 s), with the refused name in it.
- `fixtures/packs/s6-board/` (new): `pack.yaml` and `providers/s6.mjs`, a fixed source of 17 tasks in three epics
  (E-04 Cache layer, E-06 Config and CLI, E-07 Performance) with labels. E-07 has S6's five targets and its decoys
  (a ready task with no perf label, perf tasks that are backlog, active or done, and ready perf tasks in E-04 and
  E-06). Each task names its epic in `collections`, and the provider lists the three epics as collections, so the
  strip and the Epic box have epics. It has no logic beyond that table, so it has no test of its own: the start-state
  check in scenario 2 is its test.
- `core/tests/pack_manifest.rs` and `host/tests/layout.rs`: the counts of pack directories and manifests go from five
  to six, with the names (`six`, `the_six_manifests_*`, `there_are_six_pack_directories`), because the fixture folder
  gained a pack. The new pack's manifest passes the same checks as the others.
- Red at runtime, seen before the code went green:
  - `there_are_six_pack_directories` and `the_six_manifests_*` (and `the_reference_manifest_holds_every_key_of_the_
    plans_example`, which uses the same list) failed with 5 against 6 before the pack existed.
  - Scenario 1: with the Add click left out, it failed at "the empty board is gone".
  - Scenario 2: with the perf chip left out, it failed waiting for "5 tasks".
  - Wrong filter: with Ready and perf added, it failed on "13 tasks" (the view held the five).
  - The three browser tests could not fail on the code before the change, as the item adds no app code; the runs above
    change the test's own steps, and no guard was touched.

### Decisions for the owner

- **"Slow spots" cannot be a view's name in the app.** `host/src/views.rs` `save_view` takes the name as the file name
  (letters, digits, `.`, `_`, `-`, 1 to 64, not starting with `-` or `.`), so the card's own words are refused with
  "the view name ... cannot be a file name". A person following S6's card meets this on the first try. The test types
  "Slow spots", checks the refusal and that nothing was saved, then saves "Slow-spots"; the detector reads a hyphen
  for the space. Alternatives: (a) as built, which records the finding and changes nothing; (b) let a view have a
  display name apart from its file name (a `name` with spaces, and a slug for the file); (c) keep the rule and say it
  beside the field before the person types. Recommend (b) or (c), as a plan change for a later item; a person's first
  view name often has a space. This item does not touch the rule.
- "Stage E-07" is the epic's funnel on its card in the strip (one click). The Epic box is the other control for the
  same choice; the wrong-filter test uses it, so both are driven. Alternative: use the box in both. Recommend as built.
- The fixture pack is named `s6-board` with the source `s6`, so keys read `s6:T-07-001`; it is a provider of fixed
  data and not the corpus provider of the host's tests, which has no epics and reads task files from the checkout.
  Alternative: a `labels:` field in the plan pack's frontmatter reading. Recommend as built; no 6a criterion asks for
  the pack change.
- A fourth fixture pack means every list of "the five packs" says six. Alternative: keep the pack outside
  `fixtures/packs/`, for example under `app/browser/support/`. Recommend as built, as the item names `fixtures/packs/`.
- The S1 end state also reads the daemon (one `scratch` task with the card's title), not only the page. Alternative:
  the page alone. Recommend as built: S6's detectors read state, not the screen.

### Not done, by design

- Launching the scratch task's run and reviewing it, the rest of S6 scenario 1: Stage 9a.
- The bundled window and the owner's walkthrough with its times: `ts-0197-uj`.
- Allowing a view name with a space: needs a plan decision (see Decisions); no item yet.
