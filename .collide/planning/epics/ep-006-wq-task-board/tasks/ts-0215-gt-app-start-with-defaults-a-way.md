---
id: ts-0215-gt
kind: task
status: done
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

### Built

- Wire (`core/src/owner.rs`, `core/src/owner/config.rs`, `core/src/schema.rs`; `pnpm gen` ran, the generated files
  are its output): `OwnerInitializeParams.defaults` (absent means a normal connection); `config.reset` with
  `ConfigResetParams { board }` and `ConfigResetResult { folder, moved }`.
- Daemon, per connection (`host/src/auth.rs` `Session.defaults`; `host/src/owner_client.rs` `connect_as`):
  - `host/src/daemon.rs` `defaults_serves` is the list of methods a defaults connection may call. Any other method
    gets `NOT_PERMITTED` from `refused_in_defaults`: "this window started with defaults, so `<method>` is refused:
    your settings, views and packs are set aside for it. Start normally to change them". The gate is the first thing
    `call` does, so no write can forget it. The refused ones include `config.set`, `task.save_view`,
    `config.pack_add`, `config.pack_link`, `config.pack_enable` and `board.init`, and also `board.trust`,
    `board.locate`, `task.apply` and every run, queue and daemon write.
  - Reads: `config.get` (`host/src/config_get.rs` `get_defaults`, over `board_load.rs` `load_defaults`: the Default
    layer and the built-in packs, no file read); `config.pack_list` (`pack_list_defaults`); `board.list`,
    `board.open`, `board.trust_review` (`board_lines.rs` `for_defaults`, `summary_defaults`, `review_defaults`: an
    empty line, no problems, no trust, no review); `task.views` and `task.list` with a view (`daemon.rs`
    `loaded_views` with the flag: the built-in views and the built-in packs' views, no personal or repo view);
    `task.list` and `task.get` show only the scratch source and the built-in packs' sources (`defaults_allows`,
    `defaults_only`, `BoardCorpus::packs` in `host/src/corpus_service.rs`).
  - The trust record is never read or written by a defaults connection, and the trust check is unchanged.
- Reset (`host/src/set_aside.rs`, `daemon.rs` `config_reset`): moves `config.yaml`, `packs.yaml`, `packs.lock` and
  `boards/<id>` out of `~/.config/collide/` into `set-aside/<YYYY-MM-DD-HHMMSS>/`, keeping their paths, by `rename`.
  A folder that exists gets `-2`, `-3`, so two resets in a second make two folders. The folder is made `0700`. Nothing
  is deleted, a file that is not there is skipped, a failure names the file and what had moved, and the set-aside
  folder is never in the list. A defaults connection may call it: it is the way out.
- Shell (`app/src-tauri/src/shell.rs`, `menu.rs`, `lib.rs`):
  - `shell.set_defaults { defaults }` (`SET_DEFAULTS`, `set_defaults`, `DaemonLink::set_defaults`,
    `DaemonAccess::connect_as`, `SocketLink`): the window cannot send `initialize`, so it tells the shell, and the
    shell drops its connection and makes the next with the flag. `app/src/owner/methods.ts` `SHELL_METHODS` has it.
  - `menu.rs`: the id, label and key of the item, the `Reloader` trait, `on_menu_event` and `reload_script`.
    `lib.rs` `build_menu` makes the app menu (About, the new item, Hide, Quit; Edit; Window) and `WebviewReloader`
    runs the script in the main window. The script sets or clears the `defaults` key of the fragment and keeps the
    other keys, then reloads.
- UI (`app/src/defaults/mode.ts`, `DefaultsBanner.tsx`, `App.tsx`):
  - `App` reads `defaults` from the fragment. `DefaultsMode` tells the shell first and renders nothing until it has
    answered, so no call goes out on a normal connection; a shell that refuses is said, with Start normally.
  - The window's stored state goes to a memory store in defaults mode (`StateStoreProvider`, `useKeptStore`, used by
    `FieldPage`, `Drawer` and `EpicDrawer`), so nothing stored is read and nothing is written.
  - `DefaultsBanner` (the item's words, Start normally, Reset…), `ResetWindow` (lists what it moves and leaves),
    `ResetNotice` (names the folder after the next start). A confirmed Reset calls `config.reset`, runs
    `clearStoredState`, tells the shell `false` and reloads without the flag and with `reset=<folder>` on the fragment.
- Where the UI keeps its state, all in the webview's `localStorage` through the `FoldStore` seam:
  `collide.drawer.width`, `collide.drawer.folds`, `collide.field.folds:<board>:<view>`,
  `collide.field.layout:<board>:<view>`, `collide.field.filters.folded:<board>` and
  `collide.field.epics.folded:<board>:<view>`. The last tab (`App` `useState`) and the last view (`useField`) are in
  memory only, so they need no clearing. `clearStoredState` removes every key that starts with `collide.`.
- The key. Ctrl-Option-Shift-Escape. Checked: macOS reserves Option-Command-Escape (Force Quit) and Option-Command-
  Shift-Escape (force quit the front app, held); this chord is neither. In `app/src/` the Escape handlers
  (`Drawer.tsx`, `Peek.tsx`, and the dialogs) take plain Escape, and `Drawer.tsx` and the other key handlers
  (`Frame.tsx`, `FieldPage.tsx`, `filters/keys.ts`) return when Control, Option or Command is down. `frame/actions.ts`
  and `ui/actions.ts` bind no chord. A menu accelerator is taken by the system menu before the page sees the key.
- Tests, with those seen red at runtime (a stub of my own function, never a guard):
  - `host/tests/daemon_defaults.rs` (11): the first run had 10 of 11 red, the defaults flag ignored. Names:
    `the_broken_files_break_a_normal_connection`, `defaults_open_the_board_with_the_built_in_views_and_the_fixtures_
    tasks`, `defaults_show_every_setting_at_default_and_read_no_settings_file`, `defaults_list_the_built_in_packs_and_
    no_other`, `defaults_show_no_problems_no_trust_and_no_review`, `a_normal_connection_on_the_same_daemon_still_sees_
    the_persons_files_and_problems`, `every_write_is_refused_in_defaults_mode_with_words_that_name_it_and_changes_no_
    file`, `a_trusted_repos_views_and_packs_do_not_load_and_its_trust_record_is_unchanged`,
    `reset_moves_the_listed_files_aside_and_deletes_nothing`, `a_reset_of_an_unknown_board_is_refused_and_moves_
    nothing`, `after_a_reset_a_normal_connection_reads_the_defaults_too`. The first test passes before the change by
    design: it proves the fixture breaks a normal connection. The secret store is a `MemorySecretStore` and is
    checked unchanged in the reset test.
  - `host/tests/set_aside.rs` (6): five red with `reset` stubbed to move nothing; the stamp test is new code.
  - `app/src-tauri/tests/menu.rs` (5): two red with the handler asking for the wrong mode.
    `app/src-tauri/tests/rpc.rs` `shell_set_defaults_*` (2): the first red with the link not told.
    `app/src-tauri/tests/daemon_link.rs` `a_link_told_defaults_makes_a_defaults_connection_*`: red with the link
    ignoring the mode.
  - `app/test/defaults.test.tsx` (16): three red with the fragment flag, the store and the clearing stubbed.
  - `app/browser/fake-defaults.spec.ts` (6, fake owner): five red with the fragment flag stubbed.
    `app/browser/defaults.spec.ts` (3, bridge, real daemon on a temp root with the broken fixture): all three red with
    the shell ignoring the mode.
- No new crate or package. `is_safe_token` and `save_view`'s name rule are unchanged.

### Decisions for the owner

- **The key.** Ctrl-Option-Shift-Escape (checks above). Alternative: no key, the menu item alone. Recommend the key:
  it is a chord nothing else uses, and a person with a blank page cannot reach a button.
- **How the window enters the mode.** The shell reloads with the `defaults` key on the fragment, as the item says. The
  item's "the UI passes `defaults: true` on `initialize`" cannot be literal: the window never sends `initialize`
  (`Shell::rpc` refuses it, and the token is the shell's). Instead the UI sends `shell.set_defaults` first, and the
  shell's connection carries the flag on `initialize`, so the flag still rides the connection and no method can
  forget it. Alternative: the menu handler sets the shell's mode itself, and the page asks nothing. Recommend what is
  built: it also works for the bridge page, which has no menu, and the fragment stays the page's one source.
  Known gap: the shell's mode is set by the page, so a page that loads without the flag while the shell is still in
  defaults mode reads with the flag. Only the page's own exits (Start normally, a Reset) drop the flag, and each
  tells the shell first.
- **Writes.** Refused, with words that name defaults mode, by one gate before dispatch that allows a list of reads
  and Reset. Alternative: allow writes to the person's files. Recommend refusing: nothing saves into files Reset
  would move. A new method is refused until someone adds it to `defaults_serves`, which fails safe.
- **Reset's scope.** The global files plus this board's folder, as recommended. Alternative: this board's folder
  only. A Reset with no board open moves the global files alone.
- **The event stream.** The shell's event pump keeps its own, normal connection. It carries notices only, and the
  window asks for data over the defaults connection. Alternative: the pump also reconnects with the flag. Recommend
  leaving it: a second reconnect path for no read the window makes.
- **Where the flag lives on the page.** A fragment key beside the others (`#token=` of the bridge, `#world=` of the
  fake). Alternative: a query string, which the bridge and the app's own server would see. Recommend the fragment.
- **A notice after Reset.** The folder reaches the next start in the fragment (`reset=<folder>`), so the page keeps no
  stored state for it; Dismiss takes it off. Alternative: `sessionStorage`.
- **Plan text to propose** (Stage 6a, Start with defaults, after "Writes are refused"): "A defaults connection may
  list and read tasks of the scratch source and of the built-in packs, the built-in views, the Default layer and the
  built-in packs; it may call `config.reset`. The shell makes its connection with the flag after the window tells it
  (`shell.set_defaults`), because the window cannot send `initialize`."

- Review (2026-11-26): `set_aside.rs` `GLOBAL_FILES` names `PACKS_FILE` and `LOCK_FILE` from `user_packs.rs`
  instead of spelling them again. Checked: the gate is first in `call` and fails closed; Reset renames, never
  deletes; the page tells the shell before its first call, and Start normally tells it `false` before the reload.
  Full gates passed but for `browser/settings.spec.ts`, whose reload can land while a poll's answer is read and whose
  refused-secret case times out under load; both predate this item and are fixed in their own commit.

### Not done, by design

- Counts of a built-in view still cover every task of the board's corpus, including tasks from repo packs that the
  window does not list. Defaults mode filters the tasks it returns, not the counts the corpus keeps. A later item can
  give a defaults connection its own corpus; the cost is a second corpus for one window.
- A built-in pack's tasks come from the board's shared corpus, which reads its sources through the normal line. With a
  user pack folder gone the normal line fails, so a corpus started after the files broke may list only the scratch
  source. The bridge test's plan tasks were read before the files were broken, and the host test's tasks are scratch
  ones, so a corpus that starts broken is not covered. A later item can read the built-in sources for a defaults
  connection through its own line.
- The owner check of the menu item and the key in the bundled app is `ts-0197-uj`'s. No `tauri build` ran, and the app
  was never launched; the menu code compiles and its handler is unit-tested.
- A way to move a set-aside folder back is not built. The folder keeps the paths, so a person can copy it by hand.
- The Reset window cannot yet list the board's views by name; it names the board and says "your views".
