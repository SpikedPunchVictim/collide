---
id: ts-0185-ge
kind: task
status: ready
title: "Owner look: the Field and the drawer beside the mock"
owners: []
links:
  - { type: blocked-by, to: ts-0184-hv }
---

## Goal

The owner looks at the Field and the drawer over real tasks, in the dev app, beside the board mock, before the rest
of Stage 6a builds on them. A difference found here costs one fix item now, not a change to six items later. The
owner also measures how to tell, without spending, whether `claude` is logged in, which `ts-0192-es` needs.

A build agent never runs it. It never starts the app or a daemon against the owner's data, and never runs `claude`.
Once `ts-0184-hv` is done, it writes the command lines and a short checklist under `### Owner checklist`, from the
code and from the Built sections of `ts-0179-ke` to `ts-0184-hv`. The owner runs them and records the result here as
owner judgement, with what was awkward.

## Success criteria

- The Field over a clone of the Collide repository (its plan tasks in `ep-006-wq` and others) matches the mock's
  layout, words, chips, folds and empty states for notes 1 to 7 and 14. Owner-run, in the dev app.
- The drawer over the same tasks matches notes 8, 18 and 39: its parts and order, the folds, the drag, the
  double-click and the narrow overlay. Owner-run.
- A status change made by hand in a worktree of the clone shows as a divergence chip, and the card flashes. Owner-run.
- The CSP is in force in the real webview: the hostile fixture task shows as text, and the webview's console shows
  the blocked image. Owner-run, in the dev app.
- The owner records a command that says whether `claude` is logged in without spending: its exit code and output
  when logged in, and with a fresh temp `HOME`. If no such command exists, that is the result. Owner-run, spend cap
  $0 (no prompt is sent).
- Each difference is written down as a numbered note, and becomes a fix item or a change to the mock. A line records
  the date of the look.

## Tests

None by a build agent: this check is the owner's. The checklist names the automated tests it relies on:

- `ts-0179-ke`: the frame and the fake-owner page;
- `ts-0180-ad`, `ts-0181-ft`: the wire and the Field;
- `ts-0182-ad`: `watch` and live updates;
- `ts-0183-ee`: the CSP, capabilities, markdown and the link window;
- `ts-0184-hv`: the drawer.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 6a: Field; Task drawer; the Tauri shell bullet (CSP). Part L.
  - The board mock, notes 1 to 8, 14, 18 and 39.
- What the checklist covers. The build agent fills in the exact commands once the earlier items are built.
  - **Set up.** Build the CLI, the daemon and the dev app from `<collide checkout>` (`pnpm tauri dev`, not a bundled
    build). Make `<collide clone>` of this repository in a folder made for the look. Use a fresh `HOME` and
    `COLLIDE_DAEMON_ROOT` where it can; say which steps, if any, use the real `HOME`.
  - **Field.** Open `<collide clone>`; the `ready` view and one other; columns, headers, folds, cards and chips,
    beside the mock's Field.
  - **Drawer.** A plan task with links and a scratch task; parts, folds, width.
  - **Live.** A worktree of the clone; a status edit there; the chip and the flash.
  - **Webview.** The hostile fixture task; the console; an external link's window.
  - **`claude` probe.** `claude --help`, then the status command it lists, if any, logged in and with a temp
    `HOME`. No prompt is sent.
  - **Record.** The differences as numbered notes, the probe's result, the time it took, and what was awkward.
- **Owner's choice, recommendation taken (2026-11-24): the midway look.** Recommend this item: the Field and drawer are
  the base of every later item, and the owner designs by looking. Alternative: no midway look; the differences wait for
  `ts-0197-uj`.
- **Owner's choice, recommendation taken (2026-11-24): the `claude` login probe.** Recommend the owner's measurement
  here, spend cap $0. Until a command is recorded, `ts-0192-es` shows "could not check" for logged in, and never blocks.
  Alternative: infer it from files `claude` writes under `HOME`; it reads private state and breaks when `claude`
  changes.
- Area: this item's own text. No code.
- The test rule, for anything automated this check leans on: no test touches the real home, real launchd, a real
  daemon, the keychain or the network. Each test that runs the `collide` binary clears the environment
  (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as `host/tests/common/mod.rs` `TempRoot`
  does. Keychain writes go through `MemorySecretStore`, never the login keychain. The owner's own look uses the
  dev app and a clone by choice, where the checklist says so.
- Stage 6a item (revision 25 scope, split 2026-11-24). Items of Stage 6a run one at a time, in id order, because they
  share `app/`, `core`, `host` and the generated files. The next item waits for this look.
- Nothing here runs a real `claude` for a prompt. The probe runs `claude` only for its help and status, by the owner.
- Owner decisions (2026-11-24): the owner took the recommendations at the split.

### Owner checklist

Written by the build agent once `ts-0184-hv` is done.
