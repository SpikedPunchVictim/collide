---
id: ts-0193-gk
kind: task
status: ready
title: "App: Settings, its layers, packs and secrets"
owners: []
links:
  - { type: blocked-by, to: ts-0192-es }
---

## Goal

The Settings button opens one page for the board (note 12). General is one table: each key with its value in use and
its layer as a badge (Default, You, This repo), and a value the repo overrides shown beside it, struck out. Packs
lists the board's packs and adds one. Secrets shows Set or Not set, never the value, and sets one into the keychain.
The Agents page is Stage 6b's.

What exists.
- `config.get` (`core/src/owner/config.rs` `ConfigGetResult { entries, notices }`; `host/src/config_get.rs`):
  each key's effective value and its `layer` (`SettingLayer { Default, User, Repo, Profile }`), and `secret`. It does
  not say what a higher layer overrode. Problems are in the error message (`ts-0098-nx` recommended a `problems`
  field "when the app shows them (Stage 6)").
- `config.set` (`ConfigSetParams { board, key, value, layer, secret }`) writes the user file or a secret, through
  `SecretStore` (`host/src/secret_store.rs`), which is `MemorySecretStore` under a test root.
- `config.pack_list`, `config.pack_add`, `config.pack_link`, `config.pack_enable`, `config.pack_update`, and
  `board.trust_review` (`pack_review`, `ts-0097-kw`).
- From `ts-0179-ke`: the Settings button.

What this adds.
- Wire: `ConfigEntry.overridden`: the value and layer the effective one replaced, when a higher layer set the key
  (never for a secret). `ConfigGetResult.problems`: each problem with its key, layer and words, so the page shows
  them in place.
- The page, "Settings for <board>", with Back to the board, and four tabs:
  - General: the table (Setting with its key beneath, Value in use, Set in). An overridden value shows struck out
    with "yours, overridden by this repo". Problems show on their rows. Details for engineers lists each layer's file
    as the daemon reports it.
  - Packs: each pack of the board with its source, where it reads, and its status count. Add a pack… (see the Owner's
    choice).
  - Secrets: each secret the board's settings and packs ask for, with Set or Not set and Set… or Replace…. The
    window takes the value in a password box, writes it with `config.set` (`secret: true`), and never shows it again.
  - Agents: "This page arrives with the in-app agent, in Stage 6b."
- The page's words come from the mock, with two changes (decisions below).

## Success criteria

- A key set in both settings files shows the repo's value, labelled as coming from the repo, with the user's value
  struck out beside it (Stage 6a).
- A secret set from the page is reported Set by `config.get`, and its value appears nowhere in the page, the
  owner protocol's answers or the logs.
- A settings problem shows on its row with its words.
- The Packs tab lists the board's packs with their status counts.

## Tests

Write these first.

- `host/tests/`: `overridden` for a key in both files; none for a secret; `problems` for a bad value in each layer.
- `app/test/`: the table's badges and the struck value; problems in place; the secrets list.
- `app/browser/settings.spec.ts` (bridge; the daemon on a temp root, so secrets go to `MemorySecretStore`): both
  files; a secret set and then reported Set; the value absent from the page's text and from the world's logs.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 6a: Settings (note 12). Success Criteria: "A key set in both settings files shows the repo's value,
    labelled as coming from the repo." Tests: "the settings view".
  - 0.7: settings layers and secrets. 0.9: secrets never leave the keychain.
- Design: note 12. Mock functions: `settingsHtml`, `secretModal`, `topRightHtml` (the button).
- Carried: `ts-0098-nx` (`problems` on `ConfigGetResult`); `ts-0099-bw` (setting a secret from the Settings page).
- **Owner's choice, recommendation taken (2026-11-24): Add a pack.** The mock's button shows "(Not designed yet.)".
  Recommend a small window: a source box (a path or a git URL), then `config.pack_add`'s review shown as the trust
  window shows it, and Add. Alternative: the window shows the `collide pack add <source>` command with Copy, and the
  page lists the result.
- **Decision for the item, with a recommendation: the files in Details.** The mock names the user file under
  Application Support; the code and the plan keep it at `~/.config/collide/config.yaml`. Recommend showing the paths
  the daemon reports. No plan change.
- **Decision for the item, with a recommendation: a pack's layer.** The mock says "yours wins over a pack's"; no
  settings layer is a pack's (`SettingLayer`). Recommend dropping that clause. No plan change.
- `questions.keep` (the mock's row) is Stage 9a's; the Agents page is 6b's.
- Area: `core/src/owner/config.rs`, `host/src/config_get.rs`, `schemas/` and the generated files, `app/src/` (a
  `settings/` folder), `app/src/owner/methods.ts`, `host/tests/`, `app/test/`, `app/browser/`.
- Dependencies: none.
- The test rule: no test touches the real home, real launchd, a real daemon, the keychain or the network. Each test
  that runs the `collide` binary clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to
  a temp root, as `host/tests/common/mod.rs` `TempRoot` does. A temp repository sets `GIT_CONFIG_GLOBAL` and
  `GIT_CONFIG_SYSTEM` to empty files. Keychain writes go through `MemorySecretStore` (`host/src/secret_store.rs`,
  chosen by `secret_store_for_root` whenever the daemon root is a test root), never the login keychain. Browser
  tests run the installed Chrome with a temp profile (`app/browser/support/chrome.ts`), and the bridge's daemon on a
  temp root (`app/browser/support/world.ts`). A real keychain write is the owner's, in `ts-0197-uj`.
- Stage 6a item (revision 25 scope, split 2026-11-24). Items of Stage 6a run one at a time, in id order, because they
  share `app/`, `core`, `host` and the generated files. No builder runs `tauri build`.
- Nothing here runs a real `claude`.
- Owner decisions: none recorded yet. The recommendations stand until the owner answers at the split.
