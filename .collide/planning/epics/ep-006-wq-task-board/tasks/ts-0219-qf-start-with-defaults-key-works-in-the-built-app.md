---
id: ts-0219-qf
kind: task
status: ready
title: "Start with defaults: its key works in the built app"
owners: []
links:
  - { type: blocked-by, to: ts-0197-uj }
---

## Goal

The key of "Start with Defaults…" works in the built app, as its menu item does. A person whose page is broken can
press it without finding the menu.

What exists.
- `app/src-tauri/src/menu.rs`: `START_WITH_DEFAULTS_KEY` is `Ctrl+Alt+Shift+Escape`. `app/src-tauri/src/lib.rs` builds
  the menu item with `.accelerator(START_WITH_DEFAULTS_KEY)`. The item's handler reloads the window with the defaults
  flag (`Reloader`).
- `ts-0197-uj`, the owner's run (2026-11-27), steps 26 and 28: in the debug build, with the page blanked, Control-
  Option-Shift-Escape did nothing; the menu item worked and opened on the right board. Whether the menu showed the
  key beside the item was not recorded.

What this adds.
- Find why the key does nothing: whether the menu registers it (the item shows ⌃⌥⇧⎋), whether macOS or the webview
  takes the chord first, and whether Escape with modifiers can be a menu key at all in the menu crate Tauri uses.
- Fix it. If Escape cannot carry it, pick a chord that works, that macOS does not reserve and that the app binds to
  nothing else, and change `START_WITH_DEFAULTS_KEY`, the plan's text and the checklist steps that name it.

## Success criteria

- In the built debug app (`tauri build --debug --no-bundle`), with the page blanked as in `ts-0197-uj` step 26, the
  key reloads the window into Start with defaults. The menu item shows the key.
- The key does nothing else in the app: no page binding uses the same chord.

## Tests

- `app/src-tauri/` (cargo): the item carries the accelerator the constant names, and the constant parses as an
  accelerator (the existing menu tests, extended).
- The key press in a built app is the owner's to check: one line in `ts-0214-rz`'s walkthrough, or in the owner's
  first look after this item, says so.

## Notes

- IMPLEMENTATION_PLAN.md: Stage 6a (Start with defaults); revision 33.
- **An item decision: the chord, if Escape cannot carry it.** Recommend one with Command, since macOS menus match those
  most reliably (for example Command-Option-Shift-D), checked against macOS's reserved list and the app's own keys.
- The builder cannot press keys in the app's window. Read the menu crate's source and its macOS key mapping; a script
  may read the built menu's items (for example by Accessibility) only if it needs no permission change. Never change
  system or security settings.
- The test rule: no test touches the real home, real launchd, a real daemon, the keychain or the network. Never
  `fs::write` over an executable that has run (macOS 26.3 kills it on its next run): use `common::write_executable`.
- Nothing here runs a real `claude`. New dependencies: none expected.
- Stage 6a item (revision 33, 2026-11-27), from `ts-0197-uj`'s owner run. It runs after `ts-0197-uj`, before
  `ts-0220-oc`.
