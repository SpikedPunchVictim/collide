---
id: ts-0215-gt
kind: task
status: ready
title: "App: start with defaults, a way back from broken settings and views"
owners: []
links:
  - { type: blocked-by, to: ts-0196-uw }
---

## Goal

The person can always get back to a working board. An item in the app menu, Start with defaults, with its key,
reloads the window with the built-in packs, views and settings, and none of the person's stored UI state. It works
when the page is blank or throwing, because the shell owns it, not the React app. A banner offers Start normally and
Reset…, which moves the person's Collide files into a dated folder after listing them, and deletes nothing.
Woolshed's reset key is the model (`docs/research/woolshed.md`, B).

What exists.
- The person's files under `~/.config/collide/` (`host/src/settings_files.rs` `user_config_dir`): the user settings
  file, `packs.yaml` and `packs.lock` (`host/src/user_packs.rs` `PACKS_FILE`, `LOCK_FILE`), and each board's folder
  `boards/<board id>/` (`UserScope::board_dir`), which holds the board's packs and personal views.
- The repo's `.collide/` is read only while the line is trusted (`host/src/views.rs` `repo_candidates`; the trust
  prompt is `ts-0194-qp`'s).
- The owner's `initialize` (`OwnerInitializeParams { protocol, token, client }`, `host/src/owner_client.rs`).
- From 6a: the frame and top bar (`ts-0179-ke`), Settings and its `problems` (`ts-0193-gk`), first run and trust
  (`ts-0194-qp`), and the UI state the earlier items keep (folds, the drawer's width, the filter fold, the last tab
  and view), wherever they keep it.

What this adds.
- **The menu item and key.** "Start with defaults…" in the app menu, built by the shell with Tauri's menu API, with
  an accelerator (decision below). It reloads the webview in defaults mode. No new capability: the webview still has
  `rpc` and one event channel.
- **Defaults mode, per connection.** The app's `initialize` carries `defaults: true` (decision below). For that
  connection the owner reads only the Default layer, the built-in packs and the built-in views. It does not read the
  person's files or the repo's `.collide/`, as if the line were not trusted, and changes no trust record. Other
  clients (the CLI, other windows) are unchanged.
- **No writes in defaults mode.** `config.set`, `task.save_view`, `config.pack_add`, `config.pack_link`,
  `config.pack_enable` and Share with team are refused for that connection, with words that name defaults mode.
  Settings shows the defaults, read-only.
- **No stored UI state.** In defaults mode the UI reads none of its stored state and writes none; changes live in
  memory until the window reloads.
- **The banner**, across the top: "Started with defaults. Your settings, views and packs are set aside for this
  window." with **Start normally** (reload without the flag) and **Reset…**.
- **Reset…** lists what it will move: the user settings file, `packs.yaml`, `packs.lock`, this board's folder, and
  the UI's stored state. It says what it leaves: other boards' folders, the repo, and secrets in the keychain. On
  confirm, a new owner method moves the files into `~/.config/collide/set-aside/<date-time>/`, keeping their paths
  under `~/.config/collide/`, and returns the folder. The UI clears its stored state. The window then starts normally
  and names the folder.

## Success criteria

- With a user settings file that does not parse, a personal view that does not compile and a user pack whose folder
  is gone, Start with defaults opens the board with the built-in packs and views, the banner and the fixture's tasks
  (Stage 6a).
- In defaults mode Settings shows every key at Default, and no value from the person's file or the repo.
- A CLI client connected at the same time still sees the person's settings and their problems (Stage 6a).
- Saving a view, setting a key and adding a pack are refused in defaults mode, with words that name it, and change no
  file.
- A trusted repo's `.collide/` views and packs do not load in defaults mode, and its trust record is unchanged after.
- A stored fold and drawer width are ignored in defaults mode, and still there after Start normally.
- Reset… moves exactly the files it listed and deletes none (Stage 6a): each moved file is byte-identical in the
  set-aside folder, no repo file changed, and the secret store is unchanged. The board then starts normally with the
  defaults in effect.
- Two resets in the same second make two folders.
- The menu item and its key work with the page blank: the owner's check, in the bundled app, in `ts-0197-uj`.

## Tests

Write these first.

- `host/tests/` (a temp root): the broken fixture (bad settings file, failing view, missing pack folder); a defaults
  connection's `config.get`, `config.pack_list` and `task.list` with views, beside a normal connection's on the same
  daemon; each write refused in defaults mode with no file changed; a trusted repo's views absent and its trust record
  unchanged; Reset's move (paths kept, bytes equal, nothing deleted, the set-aside folder itself never moved, two
  resets in one second); `MemorySecretStore` unchanged by Reset.
- `app/test/` (the fake owner): the banner and its buttons; Reset's list; stored state ignored in defaults mode and
  kept after Start normally.
- `app/browser/defaults.spec.ts` (bridge; a real daemon on a temp root with the broken fixture): open the window in
  defaults mode by the same entry the menu calls (the bridge cannot press a native menu); the Field shows the
  fixture's tasks; Reset… and then a normal start.
- `app/src-tauri/` unit test: the menu item's handler asks for a reload in defaults mode. No `tauri build`.

## Notes

- IMPLEMENTATION_PLAN.md: Stage 6a (Start with defaults; Settings; No-setup open; the Tauri shell bullet); 0.7
  (layers); 0.9 (trust); revision 29. `docs/research/woolshed.md`, B.
- **An item decision: the key.** Recommend Ctrl-Option-Shift-Escape as the menu item's accelerator, Woolshed's
  chord. Check it against macOS's reserved keys (Option-Command-Escape is Force Quit) and the keys 6a binds.
  Alternative: the menu item alone, with no key.
- **An item decision: how the window enters the mode.** Recommend the shell reloads the webview with `#defaults` on
  its URL, and the UI passes `defaults: true` on `initialize` when it sees it. A blank page still reloads, and the
  flag rides the connection, so no method can forget it. Alternative: a flag on each call, which every method would
  have to carry.
- **An item decision: writes.** Recommend refusing them, so a person never saves into files Reset is about to move.
  Alternative: allow writes to the person's files in defaults mode.
- **An item decision: Reset's scope.** Recommend the global files plus this board's folder, since a broken global
  settings file breaks every board. Other boards' folders stay. Alternative: this board's folder only.
- **An item decision: where the UI keeps its state.** If the earlier items keep it in the webview's storage, Reset
  clears those keys; if in the board's folder, Reset's move covers it. Name what was found in the item's record.
- New dependencies: none expected. Tauri's menu API is part of `tauri`. If one is needed, stop and name it to the
  owner.
- The test rule: no test touches the real home, real launchd, a real daemon, the keychain or the network. Each test
  that runs the `collide` binary clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a
  temp root, as `host/tests/common/mod.rs` `TempRoot` does. Keychain writes in tests go through `MemorySecretStore`,
  never the login keychain. A test never stubs or bypasses a safety gate (the trust check, `env_clear`) to make a
  result appear.
- Nothing here runs a real `claude`.
- Stage 6a item (revision 29, 2026-11-24), from the Woolshed study. It runs after `ts-0196-uw` and before
  `ts-0197-uj`, whose owner check covers the menu item and key in the bundled app.
- Owner decisions (2026-11-24): the owner asked for this item. The item decisions above are recommendations for the
  builder.
