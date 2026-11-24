---
id: ts-0205-qh
kind: task
status: ready
title: "Owner look: an ask drawing a chip and a lens on the Field"
owners: []
links:
  - { type: blocked-by, to: ts-0204-ea }
---

## Goal

The owner looks at the ask box, the chip and the lens in the dev app, beside the board mock, before the rest of
Stage 6b builds on them. One real ask with Claude Code haiku draws a lens on S6's fixture board, so the owner sees
the real path once, early. A difference found here costs one fix item now.

A build agent never runs it. It never starts the app or a daemon against the owner's data, and never runs `claude`.
Once `ts-0204-ea` is done, it writes the command lines and a short checklist under `### Owner checklist`, from the
code and from the Built sections of `ts-0199-xz` to `ts-0204-ea`. The owner runs them and records the result here
as owner judgement, with what was awkward.

## Success criteria

- The ask box, its lines, the chip, Undo and a failure match notes 47, 49, 50, 52 and 53. Owner-run, in the dev app.
- The Field request of 0.11, asked once on S6's fixture board with the real Claude Code haiku, draws a lens whose
  columns, reasons and "Not grouped" match note 48. The owner reads the reasons of two groups against the code.
  Owner-run.
- One filter request draws a chip whose chips and CEL show in the filter bar. Owner-run.
- Save snapshot, then Snapshots, opens the same lens. Owner-run.
- The time to the first line and to the chip, the cost and the input tokens of the real ask are recorded.
- Each difference is a numbered note, and becomes a fix item or a change to the mock. A line records the date.

## Tests

None by a build agent: this check is the owner's. The checklist names the automated tests it relies on:

- `ts-0199-xz`: the listener and the token; `ts-0200-gs`: the ask line;
- `ts-0201-us`: the ask path over fake Claude; `ts-0202-ab`: the tools and the manifest;
- `ts-0203-xn`: the ask box and the chip; `ts-0204-ea`: the lens and snapshots.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 6b: The ask box; The Field's tools; Success Criteria (the owner walkthrough, first half). Part L.
  - The board mock, notes 47 to 53. S6's record, Live runs (the request and its intended groups).
- What the checklist covers. The build agent fills in the exact commands once the earlier items are built.
  - **Set up.** Build the CLI, the daemon and the dev app from `<collide checkout>` (`pnpm tauri dev`, not a bundled
    build). Open S6's fixture board (`fixtures/`, from `ts-0202-ab`) copied to a scratch folder. Use a temp
    `COLLIDE_DAEMON_ROOT`. Say that `claude` reads the real `HOME` for its login.
  - **Ask.** The Field request; the lines; the lens; two groups against the code; Undo; Save snapshot; Snapshots.
  - **Filter.** One request that `set_view` answers; the chips; Advanced; Save as View.
  - **Failure.** Cancel during an ask; close the box during a second ask and see the dot.
  - **Record.** The notes, the times, the cost, the input tokens, what was awkward, and the date.
- **Owner's choice, recommendation taken (2026-11-24): the midway look, with one real ask.** Recommend this item: the
  ask box, the chip and the lens are what the rest of 6b builds on, and the owner designs by looking. Alternative: no
  midway look, the differences waiting for `ts-0214-rz`; or a look over fake agents only, at $0.
- **Owner's choice, recommendation taken (2026-11-24): spend cap $0.25, time 30 minutes.** S6's lens asks cost under
  $0.01 each. The ask line carries `--max-budget-usd` only if the entry sets one, so the owner sets a budget of $0.05 on
  the Claude entry for this look.
- Area: this item's own text. No code.
- The test rule, for anything automated this check leans on: no test touches the real home, real launchd, a real
  daemon, the keychain or the network. Each test that runs the `collide` binary clears the environment
  (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as `host/tests/common/mod.rs` `TempRoot`
  does. Keychain writes go through `MemorySecretStore`, never the login keychain. The owner's own look uses the dev
  app and a scratch board by choice, where the checklist says so.
- Stage 6b item (revision 26 scope, split 2026-11-24). Items of Stage 6b run one at a time, in id order, because they
  share `app/`, `core`, `host` and the generated files. The next item waits for this look.
- Nothing a build agent runs here starts a model. The owner's one ask does, within the cap.
- Owner decisions (2026-11-24): the owner took the recommendations at the split.

### Owner checklist

Written by the build agent once `ts-0204-ea` is done.
