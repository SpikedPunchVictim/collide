---
id: ts-0169-mn
kind: task
status: ready
title: "Settings: a per-user pack key, for the gh account"
owners: []
links:
  - { type: blocked-by, to: ts-0168-tq }
---

## Goal

A pack can declare a setting that is per user, so the `gh` account and the API address of the GitHub pack live only in
the person's own settings, never in the repo's. Plan section 0.7 says source accounts are per user by nature.

What exists. `core/src/settings_registry.rs` marks some of Collide's own keys `per_user`, and
`core/src/settings_layers.rs` refuses a per-user key in the repo's file (`repo_file_refusals`) and in a repo profile
(`check_profile_key`, `ProfileSource::Repo`). A pack's setting under `config:` carries `secret`, `overridable` and
`repoOnly`, read in `key_infos` and checked in `core/src/pack_check.rs` (each a boolean, `repoOnly` excludes the other
two). `key_infos` sets `per_user: false` for every pack key, and `read_file` skips the per-user refusal for pack keys.
There is no `perUser` flag and no account key anywhere. `collide config get`, `set` and the layers' origin report are
built (`host/tests/config_get.rs`, `config_set.rs`).

What this adds.
- `perUser: true` beside the schema keywords of a pack setting. `pack_check.rs` reads it as a boolean, a non-boolean is
  a problem, and `perUser` with `repoOnly` is a problem naming both flags (`secret` may go with `perUser`).
  `core/src/pack.rs` `SchemaObject`'s description lists it, and the schema of `pack.yaml` is regenerated.
- `key_infos` sets `per_user` from the flag. A pack's per-user key in the repo's settings file or in a repo profile is
  refused with a problem that names the key and says it is per user, the same words as a Collide key's. The user's own
  file sets it, and `collide config get <pack>.<key>` reports its value and the layer it came from.
- No GitHub key is declared here. `ts-0172-xc` declares `account` and `apiUrl` as per-user keys of `collide/github`.

## Success criteria

- A pack with `perUser: true` on a setting passes `check_pack`. With `perUser: "yes"`, or with `repoOnly: true` beside
  it, it fails with a problem that names the setting and the flags.
- That setting in the repo's settings file, or in a profile of the repo, is a settings problem naming the key; the
  value is not applied. The same value in the user's file is applied and `collide config get` shows it and its origin.
- Every setting of `collide/plan` is unchanged, and a key without the flag behaves as before.
- The generated schema of `pack.yaml` accepts the flag and still refuses an unknown one.

## Tests

Write these first.

- `core/tests/` pack-check cases beside the existing flag cases: accepted, non-boolean, with `repoOnly`, with
  `secret`.
- Settings-layer tests in `core/tests/` with a fixture pack that has a per-user key: repo file refused, repo profile
  refused, user file applied, an unset key absent.
- `host/tests/config_get.rs` and `config_set.rs` extended with that fixture pack: the origin report names the user's
  file; `config set` writes only the user's file.
- The schema drift check that already guards `pack.yaml`'s generated schema.

## Notes

- Area: `core/src/pack.rs`, `core/src/pack_check.rs`, `core/src/settings_layers.rs`, the generated `pack.yaml` schema,
  `host/tests/`, `core/tests/`.
- The test rule: no test touches the real home, real launchd, a real daemon, the keychain or the network. Each test that
  runs the `collide` binary clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp
  root, as `host/tests/common/mod.rs` `TempRoot` does. Settings files are fixtures, never the owner's own
  `settings.json`. A temp repository sets `GIT_CONFIG_GLOBAL` and `GIT_CONFIG_SYSTEM` to empty files.
- A user profile still needs `overridable` to set a pack key (`check_profile_key`, `ProfileSource::User`); that is not
  changed here.
- Stage 5 items run one at a time, in id order. Nothing here runs a real `claude`. No dependency is added.
- Owner decisions: none recorded yet. The recommendations stand until the owner answers at the split.
