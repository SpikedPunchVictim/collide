---
id: ts-0204-ea
kind: task
status: ready
title: "App: the lens on the Field, Save snapshot and Snapshots"
owners: []
links:
  - { type: blocked-by, to: ts-0203-xn }
---

## Goal

An agent's `show_groups` draws a lens on the Field: its groups as columns, each with its reason, and the rest dimmed
or hidden. A lens is saved only as a snapshot, a personal file, and the view menu lists snapshots to open again.

What exists.
- The reducer's lens (`app/src/ui/state.ts` `Lens`, `LensGroup`; `ts-0042-vn`) and `LensBanner.tsx`.
- The ask box and the chip of a `show_groups` change (`ts-0203-xn`); the Field and its view menu (`ts-0181-ft`).
- Personal views in the person's board folder (`UserScope::board_dir`; `task.save_view`). No snapshot store.
- The mock: `lensHtml`, `snapsHtml`, `saveSnap`, `snapModal`, `openSnap`; notes 48 and 51.

What this adds.
- The lens on the Field (note 48): one column per group, its name and its reason under it, and a "Not grouped"
  column, dimmed for `rest: dim` and absent for `rest: hide`. The lens is stamped with its time, agent and model, and
  does not follow the corpus: a task that changes after it was made stays where it was put.
- A group can be selected as a whole; selecting launches nothing.
- Save snapshot on the chip: a name, then a personal file. Snapshots in the view menu (note 51): the list with
  each one's time and agent; opening one draws its lens again, marked with its time; one can be deleted.
- The daemon's snapshot methods: save, list, read and delete, over files in the person's board folder.

## Success criteria

- Over a fake owner: the columns, the reasons, "Not grouped" dimmed or hidden, and the stamp.
- A task changed after the lens was made stays in its group.
- Save snapshot writes one file under the person's board folder; Snapshots lists it; opening it draws the same
  groups; it survives a daemon restart.
- A snapshot name with a path separator, and text over 0.9's caps, are refused with plain text.
- A lens is never saved as a View: the chip of a `show_groups` change offers Save snapshot only.
- `pnpm test` and `cargo test --workspace` pass.

## Tests

Write these first.
- `app/test/`: the lens board, `rest` dim and hide, the stamp, a changed task, the selection, Save snapshot, the
  Snapshots menu.
- `host/tests/`: the four methods over a temp root; the file's place and shape; the refusals.
- `app/browser/`: a lens saved, the bridge's daemon restarted, the snapshot opened.
- The S6 ports: `agent.test.ts` "lens" (the Save snapshot half; "selectIds adds a group to the selection and does
  not launch anything"); `agent-ui.test.tsx` "lens on the Field" (two tests).

## Notes

- IMPLEMENTATION_PLAN.md:
  - 0.11: A view and a lens are two things (a snapshot is "a personal file under the board's data directory").
  - Stage 6b: The Field's tools (Save snapshot); Success Criteria (a lens saved only as a snapshot). Part L.
- Builds on `ts-0181-ft` (the Field and the view menu), and names it.
- **Decision for the item, with a recommendation: where snapshots live.** Recommend `snapshots/<name>.json` in the
  person's board folder (`UserScope::board_dir`), beside personal views; it is never in a repo. Alternative: the
  daemon's data folder per board, beside pack data; it is not where the person's own board files are.
- **Decision for the item, with a recommendation: the methods.** Recommend `task.save_snapshot`, `task.snapshots`,
  `task.snapshot` and `task.delete_snapshot` in `core/src/owner/task.rs`, beside `task.save_view`. Alternative: a
  generic personal-file method; wider than needed.
- Area: `app/src/components/field/`, `app/src/ui/`, `core/src/owner/task.rs`, `host/src/` (a new snapshot module),
  the generated files, `app/test/`, `app/browser/`.
- Dependencies: none new.
- The test rule: no test touches the real home, real launchd, a real daemon, the keychain or the network. Each test
  that runs the `collide` binary clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to
  a temp root, as `host/tests/common/mod.rs` `TempRoot` does. A temp repository sets `GIT_CONFIG_GLOBAL` and
  `GIT_CONFIG_SYSTEM` to empty files. Keychain writes go through `MemorySecretStore` (`host/src/secret_store.rs`),
  never the login keychain. Browser tests run the installed Chrome with a temp profile (`app/browser/support/
  chrome.ts`), and the bridge's daemon on a temp root (`app/browser/support/world.ts`), with fake agents only. A test
  never stubs or bypasses a safety gate: the real-agent opt-in, the confinement probe, `env_clear`.
- Stage 6b item (revision 26 scope, split 2026-11-24). Items of Stage 6b run one at a time, in id order, because they
  share `app/`, `core`, `host` and the generated files. No builder runs `tauri build`.
- Nothing here runs a real `claude`, `opencode` or any model, or spends money.
- Owner decisions (2026-11-24): the owner took the recommendations at the split.
