---
id: ts-0098-nx
kind: task
status: active
title: "Settings layers and collide config get"
owners: []
links:
  - { type: blocked-by, to: ts-0097-kw }
---

## Goal

The four layers of 0.7 applied, so that `collide config get <key>` shows a value and the layer it came from. Agent
entries have their own field-by-field rules and are `ts-0099-bw`; secrets, the keychain and `config set` are there too.

**The merge** is a pure function in `core` (`settings_layers`). Inputs: the registry and the loaded packs' `config:`
schemas (`ts-0093-am`), the user's file and the user's per-board file, the repo's file (`LoadedLine`), and an optional
profile. Output: for each key its effective value, the layer it came from (`SettingLayer`: default, user, repo,
profile), whether it is secret, and every problem found, each naming the key.

- **Layers, lowest first**: the schema's defaults; the user's file (`~/.config/collide/config.yaml`, and under it the
  board's own user file, which wins over the global one inside the user layer); the repo's `.collide/config.yaml`,
  which wins except for a key marked `overridable`, where the user's value wins; the run's profile, applied last.
- **Per-user keys in the repo file** are refused naming the key (`repo_file_refusals`, which reads the registry now).
- **`repoOnly` keys** (the pack's `repoOnly: true`) are refused in the user's file and in a user profile, naming the
  key.
- **Secrets** (`secret: true`) in either file are a load error naming the key (`secrets_in_file`, given the loaded
  packs' secret keys). The value of a secret is never read here; its entry has no `value`.
- **Values are checked against the pack's schema.** A hand-written checker in `core` (`setting_value`) covers the
  keywords a setting uses: `type`, `enum`, `const`, `minimum`, `maximum`, `minLength`, `maxLength`, `pattern`, `items`,
  `properties`, `required`, `additionalProperties`, and `default`, `description`, `title` (not checked). A schema with a
  keyword it does not support fails the pack's load naming the keyword (fail closed), as does a `default` that does not
  meet its own schema. A value that fails names the key, the constraint and the value's type, never the secret.
- **Profiles.** Files from the snapshot's `profiles/`, `~/.config/collide/profiles/` and the packs' `profiles:` (a
  pack's profile may set only that pack's own settings: the scope rule carried from `ts-0026-es`). `--profile <name>`
  finds the name in those places and applies it as the top layer. A user profile that sets a key not marked
  `overridable` is refused, naming the key (`profile::user_profile_refusals` for the profile's own keys, and the
  `config:` keys here). A name found in two places is an error naming both.
- **Keys.** Collide's own: `ask.agent`, `ask.timeout_s`, `ask.budget_usd`, `disk.floor_gib`, `claude.check_published`,
  and `agents.<id>.<field>` (`ts-0099-bw`). A pack's: `<pack>.<key>`, as the file nests them (`ferry.lanePool`). A pack
  named like a root of Collide's own keys (`agents`, `ask`, `disk`, `claude`) is refused at load, naming both, by the
  loader's name check.

**Reading and wiring.** `host` reads the user's files with `load_yaml_file` (a missing file is empty, a bad one is an
error that names it), asks the loader for the line's repo layer, and serves `config.get` (`ConfigGetParams`: `board`,
`key`, `profile`; result `ConfigEntry`). `collide config get [<key>] [--profile <name>]` prints each entry as `key =
value  (layer)`, a secret as `key = <secret, in the keychain>  (user)`, and every problem found, with the nonzero exit
the other commands use. With no board it shows the user's layers only. The `cli: config` stub tag moves from stage 4b to
4a in `stub-tags.json` and `commands.rs`; the stub is filled when `ts-0099-bw` adds `set`.

## Success criteria

- A key set in both the user's file and `.collide/config.yaml` resolves to the repo's value, and `collide config get`
  names the repo as its layer. A key set only in the user's file says `user`, and a key set in neither says `default`.
- An `overridable` key set in both resolves to the user's value and says `user`. `disk.floor_gib` and `ask.timeout_s`
  do.
- A user profile that sets any key not marked `overridable` is refused, naming the key. One that sets an overridable key
  loads, and `config get <key> --profile <name>` gives its value with layer `profile`.
- A `repoOnly` key in the user's file is refused, naming the key. The same key in the repo's file loads. In a user
  profile it is refused.
- A per-user key (`claude.check_published`) in the repo's file is refused, naming the key.
- A secret key in the user's file, or in the repo's, fails the load and names the key; the value appears in no message.
- A pack setting with a wrong type, under `minimum`, or outside its `enum` fails naming the key and the constraint. An
  unsupported schema keyword (`$ref`, `oneOf`, `format`) fails the pack's load naming it. A default that fails its own
  schema fails naming the key.
- A pack's defaults show as layer `default`; its keys show as `<pack>.<key>`.
- A user's per-board file value wins over the user's global value, and both say `user`.
- A pack named `ask` or `disk` fails the load naming both.
- A profile named in two places fails naming both places. An unknown profile fails naming the name and where it looked.
- In a fixture repo whose `.collide` file says `dir: tools/collide`, `config get` shows the settings from
  `tools/collide/config.yaml`.
- Through the daemon: `collide config get disk.floor_gib` over a fixture repo prints the value and `(repo)` or `(user)`
  by where it is set; `collide config get` outside a repo prints the user's layers.
- `config.get` is the only owner method served here for `config`; the others still answer "not served yet".
- No new crate.

## Tests

Write these first:

- `core`, as a table over small in-memory layers: every rule above, one case each, with the layer asserted;
- `setting_value`: each keyword that passes and fails, each unsupported keyword refused, `default` checked, a deep
  object, and a value that must not appear in the message (a secret-shaped string);
- profiles: the three sources, a name in two, a user profile with an overridable and with another key, a pack profile
  that sets another pack's key, `--profile` as the top layer;
- the loader-side checks of `ts-0096-ph` that this item extends: the pack name against the roots;
- `host`: files read from a temp `HOME` (missing, empty, malformed, duplicate key, over the cap), the per-board file;
- the CLI and the daemon, with `COLLIDE_DAEMON_ROOT` in a temp root and a fixture repository: `config get` with a key,
  with none, with `--profile`, a refusal (exit code and message), a secret entry's line;
- `pnpm gen` (wire types are unchanged, so nothing regenerates) and `pnpm check` with the retagged stub.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 4a: Settings (the four layers; profile files; per-user-only and repo-only keys; secrets; Collide's own
    settings; `collide config get`); the tests ("layering, per-user-only keys, the secret rule", "`overridable` keys,
    profile layering").
  - 0.7: Settings; Profiles. 0.9: YAML is untrusted input.
- Carried from `ts-0025-st`: applying the layers, the `overridable` flip, the built-in layer merge, whether a value
  meets the pack's schema, and resolving profile names are this item's. The keychain and `ask.agent` naming an entry
  are `ts-0099-bw`.
- Carried from `ts-0026-es`: whether a user profile's `packs` are user packs and its `config` keys `overridable` (needs
  the loaded packs: here), and the pack-profile scope rule.
- **Decision for the owner, with a recommendation: how a setting's value is checked.** The plan says "one JSON Schema
  per key". The workspace has no validator crate (`core/tests/schema_validate` is a test-only one).
  - Hand-written subset (this item): no crate, about 200 lines, the keywords above, and a keyword outside the subset is
    refused at pack load, so a pack never relies on a check that does not run. Con: a pack author who wants `oneOf`
    must wait.
  - `jsonschema` 0.30 (MIT): the full drafts. Con: a large tree (`regex`, `url`, `fancy-regex`, `ahash` and more),
    `$ref` resolution that can reach the network or files unless switched off, and a new surface for untrusted pack
    input.
  - Recommend hand-written, with the fail-closed keyword check. Revisit when a real pack needs more.
- **Decision for the owner, with a recommendation: where the user's per-board settings live.** 0.7 says the user's file
  is "per board and per pack" and 0.7 puts personal views under `~/.config/collide/boards/<board-id>/`. Recommend
  `~/.config/collide/boards/<board-id>/config.yaml`, the same shape as the global file, applied after it, with the
  board's name from the daemon's store (the `board-N` that `board.list` gives). It is per machine, as the store is. The
  same folder holds the user's pack switches (`ts-0101-bd`). Alternative: key the folder by a hash of the root path,
  which survives a store reset. Recommend the store's name: it is what the wire and the views path already use.
- **Decision for the item:** the key for a pack's setting is `<pack>.<key>`. The plan names no form. It follows the
  file (`config: ferry: lanePool`) and needs the pack-name rule above so it cannot be confused with Collide's own keys.
- **Decision for the item:** `claude.check_published` keeps its minimal reader in `host/src/claude_versions.rs`, which
  leaves the fetch on when the file is malformed (`ts-0068-wx`). It is not moved onto the layers here; `config get`
  shows the key from the typed file. Recommend leaving it: its failure behavior is deliberate.
- **Decision for the item:** the profile's `config:` for a repo profile may set any key a repo file may set; a pack
  profile only its pack's.
- Area: `core/src/settings_layers.rs` and `setting_value.rs` (new, pure), `core/src/settings.rs`, `profile.rs`,
  `host/src/settings_files.rs` (new), `host/src/board_load.rs` (the name check), `host/src/daemon.rs`
  (`config.get`), `host/src/commands.rs`, `stub-tags.json`, `host/tests`, `core/tests`.
- The test rule: no test touches the real home, real launchd or a real daemon. Each test clears the environment
  (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as `host/tests/common/mod.rs` `TempRoot`
  does. Settings files are fixtures, never the owner's own `settings.json`.
- Stage 4a item (revision 23 scope, split 2026-11-16). Items of Stage 4a run one at a time, in id order, because
  they share `core`, `host` and the generated files.
- Owner decisions (2026-11-16, at the split): the recommendations above are taken under the owner's delegation; they
  stay overridable until the item is built.
