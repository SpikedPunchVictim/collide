---
id: ts-0219-qf
kind: task
status: done
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

### Built

- Cause, from code and docs read offline (no key pressed, no app launched). How sure: medium to high. The menu side
  is certain. The delivery side rests on AppKit's documented key path and on what WebKit does with Escape, which is
  from memory and not seen in source here.
  - The menu registers the key. muda 0.20.0 `src/accelerator/mod.rs` parses `Ctrl`, `Alt`, `Shift` and `Escape`;
    `src/accelerator/macos.rs` maps Escape to `"\u{001b}"` and the modifiers to `NSEventModifierFlags`; and
    `src/platform_impl/macos/mod.rs` sets `setKeyEquivalent` and `setKeyEquivalentModifierMask` on the item. A parse
    failure would have failed `MenuItemBuilder::build`, and the item built and worked from the menu. So the item
    carries the key (it should show as Control-Option-Shift-Escape).
  - Nothing in the shell routes a key to the menu. The window is the config window, so tauri-runtime-wry builds its
    web view with `build` (`WebviewKind::WindowContent`), not `build_as_child`. wry 0.57.2 `WryWebView` overrides
    only `performKeyEquivalent:`, and passes it to `super` when the view is not a child. The one `keyDown:` that
    calls `NSApp.mainMenu.performKeyEquivalent` is `WryWebViewParent`, used only by child web views. tao 0.37.2
    `TaoApp.sendEvent:` only repairs Command key-ups. tao's `TaoView` returns true from `_wantsKeyDownForEvent:` and
    forwards `cancelOperation:` as Command-period, but the web view is the focused view, not `TaoView`.
  - So the path is AppKit's own. `NSApplication` offers a key-down to the main menu before the responder chain only
    when Command is down. Without Command, the key goes to the first responder (the `WKWebView`), and the menu sees
    it only if the web view leaves it unhandled. WebKit handles Escape itself (`cancelOperation:`), so the menu never
    sees it. This confirms the hypothesis in the brief. What is not confirmed: that nothing else takes this chord
    (no macOS shortcut for it is known to me).
- Fix: `START_WITH_DEFAULTS_KEY` in `app/src-tauri/src/menu.rs` is `CmdOrCtrl+Alt+Shift+S`, and its doc comment says
  why Command is there and which macOS chords it avoids. muda reads `CmdOrCtrl` as Command on macOS and `S` as
  `KeyS`, so the item shows Command-Option-Shift-S. `lib.rs` is unchanged: it builds the item from the constant.
- Reserved keys checked, from what I know of macOS: Option-Command-Escape (Force Quit; with Shift held, force quit
  of the front app), Option-Command-D (Dock), Option-Command-H (Hide Others, which this app's own menu has),
  Option-Command-M, Option-Command-W, Option-Command-Shift-Q (log out), Command-Shift-3 to 6 (screenshots) and
  Option-Command-Shift-V (paste and match style). Option-Command-Shift-S is none of them.
- The page binds nothing with this chord. In `app/src`, every key handler (`Frame.tsx`, `Drawer.tsx`,
  `FieldPage.tsx`, `filters/keys.ts`) returns when Command, Control or Option is down; `FilterBar.tsx` and `Peek.tsx`
  take no chord. The app menu's own items (`lib.rs` `build_menu`) bind no Option-Command-Shift-S.
- Tests, `app/src-tauri/tests/menu.rs`. All three were red at runtime on the old constant, from plain assertions (it
  compiles unchanged), and are green after the change:
  - `the_accelerator_is_command_option_shift_s` (replaces `the_accelerator_is_control_option_shift_escape`)
  - `the_accelerator_holds_command_so_appkit_offers_it_to_the_menu_before_the_web_view`
  - `the_accelerator_is_not_a_key_macos_keeps_for_force_quit_log_out_or_the_dock`
- Live text naming the key: only the constant, its doc comment and the test did. No other live file names it.
  `ts-0215-gt` and `ts-0197-uj` are history and are untouched. The plan names no chord (it says "its key").
- The owner's check, in the built debug app (`tauri build --debug --no-bundle`, not run here). Blank the page as in
  `ts-0197-uj` step 26 (`document.body.innerHTML = ''` in the inspector's console). First open the "Collide" menu:
  "Start with Defaults..." should show Command-Option-Shift-S beside it. Click the page, then press Command-Option-
  Shift-S. The window should reload into Start with defaults, with the banner "Started with defaults...". The same key
  with the page working should do the same. If it does nothing, record whether the menu shows the key.

### Decisions for the owner

- **The chord.** Command-Option-Shift-S (S for Start). Alternatives: Command-Option-Shift-D (matches the item's
  example, but one Shift away from the Dock toggle); Command-Option-Shift-R (R for reset, but Reset… is a different
  action). Recommend S: no known macOS or app binding, and a letter that names the action.
- **`CmdOrCtrl`, not `Cmd`.** Same chord on macOS. On Windows or Linux it becomes Control-Alt-Shift-S, so the string
  needs no change when those ports come. Alternative: `Cmd`, which parses as the Windows or Super key elsewhere.
- **No handler to keep Escape.** A page or shell handler that sees Control-Option-Shift-Escape before the web view
  would need a `keyDown:` or `performKeyEquivalent:` override of wry's web view class, or a global shortcut, which
  the item rules out. Recommend the Command chord: it uses the menu as AppKit means it.
- **The test checks the string, not the parse.** A real parse needs `muda` as a direct dependency, or a menu built on
  the main thread, which a Rust test cannot give on macOS. Alternative: add `muda` as a dev-dependency (it is already
  in the lock). Recommend not: the owner's key press is the real check.
- **Plan text to propose** (Stage 6a, Start with defaults, after "its key"): "The key is Command-Option-Shift-S. It
  holds Command because AppKit offers a key to the menu before the web view only when Command is down; a chord
  without Command does not reach a menu item while the page has focus."
- Review (2026-11-27): the recommendations above are taken. The plan's Start with defaults now names the key and
  why it holds Command. Noted: Shift-Option-Command-S is "Save As…" in some Apple apps; Collide has no Save As, so
  nothing clashes. The key press in the built app stays the owner's.

### Not done, by design

- The key press in the built app: the owner's, per the check above (and `ts-0214-rz`'s walkthrough or the owner's
  first look after this item).
- `ts-0197-uj` step 26 still names Control-Option-Shift-Escape. It is a done item's history; its step 28 (the menu)
  is the one that worked.
