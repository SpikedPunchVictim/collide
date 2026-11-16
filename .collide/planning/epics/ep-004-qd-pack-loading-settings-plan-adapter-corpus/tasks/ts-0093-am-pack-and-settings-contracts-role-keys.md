---
id: ts-0093-am
kind: task
status: active
title: "Pack and settings contracts: role keys, repoOnly, the settings registry"
owners: []
links: []
---

## Goal

The contracts that Stage 4a's loader and settings layers read, made before either exists, so the later items build on
types that are right. No file is read and no process runs: this is `core`, the generated schemas and a YAML writer.

**Role keys** (0.3). `pack::Role` has no key for three things the plan gives a role, and two carried follow-ups wait
for them:

- `require`: a list of capability names. Each is a name of `agent::CAPABILITY_NAMES`, or a name with a sub-flag, as
  `permission_host.deny_message`. `check_manifest` refuses a name the table does not have, naming the role and the
  entry. The run-time check is `adapter::refuse_unmet_capabilities`, wired in `ts-0104-gx`.
- `allow_tools`: the tools the role asks for among the ones Collide disallows by default (0.3, Disallowed tools). A
  list of plain tool names; `check_manifest` refuses an empty name or one with a space or a quote. Which names an
  adapter disallows is the adapter's: the Claude adapter's `DISALLOWED_TOOLS` stays in `host`, and `core` does not
  name Claude's tools.
- `repo_instructions`: `worktree` or `none`, absent meaning `worktree` (0.3, Context options; revision 21). It is also
  a key of a profile's `roles:` options (`profile::RoleOptions`), as `config` is.

`requires.tools` also gets its install hint (0.7: "a missing tool fails the load with the pack's install `hint:`").
`ToolRequirement` has `name` and `version` only. Add an optional `hint` string.

**`repoOnly`** (0.7, Settings; revision 20). A pack's `config:` setting may carry `repoOnly: true` beside `secret` and
`overridable`, as `collide/plan` does for `layout`. The setting check in `check_manifest` covers only the other two.
Check that `repoOnly` is a boolean, and refuse it together with `overridable` or with `secret`, naming the key and the
two flags: a key the repo owns cannot be the user's to override, and a secret is never in a repo file.

**Collide's own settings.** Their keys are scattered today: `COLLIDE_OVERRIDABLE_KEYS`, `REPO_FILE_AGENT_ASK_KEYS`, and
a separate lenient reader for `claude.check_published` (`host/src/claude_versions.rs`). `ConfigFile` has no `disk` or
`claude` section and refuses unknown keys, so a user's file that sets `disk.floor_gib` does not load. Add:

- a registry in `core` of Collide's own settings: the key path, its JSON type and range, its default, and the flags
  `overridable`, `repo_only` and `per_user`. The keys now: `ask.agent` (per user), `ask.timeout_s` (5 to 1800,
  overridable, no default of its own: each agent has one), `ask.budget_usd` (none by default), `disk.floor_gib`
  (integer from 1, default 5, overridable) and `claude.check_published` (boolean, default true, per user). The settings
  of later stages (concurrency cap, metrics level, retention, default aliases) are added by the stage that builds them;
- the `disk` and `claude` sections of `ConfigFile` for those two keys;
- `repo_file_refusals` reads the registry: `claude.check_published` in a repo file is refused naming it, and
  `disk.floor_gib` is allowed. `COLLIDE_OVERRIDABLE_KEYS` is derived from the registry or replaced by it, and the
  existing tests stay green.

**A YAML writer.** Collide writes five files that the loader only reads today: `board.yaml` (`collide init`),
`trust.yaml`, `packs.yaml`, the two `packs.lock` files and the user's `config.yaml` (`collide config set`). The
hardened loader (`yaml.rs`) is read only: `serde-saphyr` is built without `serialize`. Add `core/src/yaml_out.rs`: one
function that turns a `Serialize` value into YAML text, with a leading comment block as an argument. Strings are always
written double-quoted with JSON's escapes (valid YAML 1.2), so no string is ever read back as a number, a boolean or
`null`, and no value needs a quoting rule. Keys are written plain when they are `[A-Za-z0-9_-]+` and quoted otherwise.
Maps and lists are in block style, and empty ones are `{}` and `[]`.

Re-run `pnpm gen`: the schemas and `pack-sdk`'s types change.

## Success criteria

- A role with `require: [mcp_servers.http, permission_host.deny_message]` loads. `require: [teleport]` fails, naming
  the role and `teleport`.
- A role with `allow_tools: [CronCreate]` loads. `allow_tools: [""]` and `allow_tools: ["a b"]` fail, naming the role.
- `repo_instructions: none` and `worktree` load and anything else fails. A profile's `roles:` entry takes the key too.
- `requires.tools: [{ name: pnpm, hint: "run corepack enable" }]` loads and round-trips. `hint: 3` fails.
- A setting `{ type: string, repoOnly: true }` loads. `repoOnly: "yes"` fails. `repoOnly` with `overridable: true`
  fails naming both flags, and so does `repoOnly` with `secret: true`.
- The registry holds the five keys, each with its default. A test lists the keys of each flag, so a change is
  deliberate: `ask.timeout_s` and `disk.floor_gib` are overridable, `ask.agent` and `claude.check_published` are per
  user.
- A `config.yaml` with `disk: { floor_gib: 8 }` and `claude: { check_published: false }` loads. `floor_gib: 0` fails,
  and `disk: { other: 1 }` fails naming `other`.
- In a repo file `claude.check_published` is refused naming the key, and `disk.floor_gib` is allowed. Every refusal
  `repo_file_refusals` made before is unchanged.
- `yaml_out` writes every contract type that Collide writes (`Board`, `PackLock`, `UserPacks`, `ConfigFile`), and the
  hardened loader reads each back equal. A string such as `"yes"`, `"1"`, `"null"`, `"a: b"`, `"# x"`, `"- x"`, one
  with a quote, a newline, a tab and a non-ASCII letter all come back as the same string.
- The output is deterministic: the same value gives the same text twice.
- `pnpm gen` leaves the tree unchanged after its first run, and `pnpm check` passes.
- No new crate.

## Tests

Write these first:

- `check_manifest` over fixture manifests: each `require`, `allow_tools`, `repo_instructions` and `hint` case above, and
  `repoOnly` with each flag; each failing fixture in `core/tests/pack_manifest.rs`'s way, its message naming the key;
- a profile with `roles: { collide/builder: { repo_instructions: none } }`, and one with a bad value
  (`core/tests/views_profiles_locks.rs`);
- the registry: the five keys with their defaults, the list of keys for each flag, and a test that every key of
  `ConfigFile` that Collide owns is in the registry (so a key added to the file type without the registry fails);
- `ConfigFile` with the `disk` and `claude` sections, the bad ones, and `repo_file_refusals` for both keys
  (`core/tests/board_config_settings.rs`);
- `yaml_out`: the round trip of each type through `load_yaml`, the string table above, the empty map and list, a
  key that needs quoting (`collide/builder`), determinism, and a leading comment block that does not change what is
  read;
- the generated-schema check (`schemas/` is up to date), as the earlier contract items do it.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 4a: Settings; Minimal pack loading (`collide init`, trust file).
  - 0.3: Capabilities (`require`); Disallowed tools; Context options (`repo_instructions`); The repository's own
    instructions.
  - 0.7: Settings (layers, `overridable`, `repoOnly`, per-user keys); Layers and sources; `requires.tools`.
  - Revision 20 and revision 21.
- Carried from `ts-0061-bg` (review): the role key for the capabilities a role may `require`, and the key for asking
  for a disallowed tool. `launch_spec` keeps passing none until `ts-0104-gx`.
- Carried from `ts-0025-st`: the pack `config:` schema with `overridable` and `secret: true` was done in `ts-0012-ua`;
  `repoOnly` is the missing flag. Applying the layers is `ts-0098-nx`.
- **Decision for the item:** the role key that asks for a disallowed tool is `allow_tools`. The plan names none.
  Recommend it: it reads as the opposite of the launch line's `--disallowed-tools`. Alternative: `disallowed_ok`.
- **Decision for the item:** `repoOnly` with `overridable` or `secret` is refused at load. A key cannot be both owned by
  the repo and the user's to override, and a secret never sits in a repo file. Recommend refusing. Alternative: let the
  layers resolve it (repo wins), which hides a mistake in a pack.
- **Decision for the owner, with a recommendation: how Collide writes YAML.** Writing needs a writer, and none exists.
  - Hand-written, in `core` (this item): no crate. About 80 lines, a closed set of types, every string quoted. Con: a
    file a person edits loses its comments when Collide rewrites it (see `ts-0099-bw`, which handles that).
  - `serde-saphyr`'s `serialize` feature (2.1.1, MIT OR Apache-2.0): the crate we already use. It pulls
    `nohash-hasher` 0.2.0 (Apache-2.0 OR MIT), which is not in `Cargo.lock`; `zmij` and `base64` are. Con: output style
    is the library's, one more crate, and the same comment loss.
  - Recommend hand-written. The files are small and closed, and the round-trip test holds the loader to the writer.
- Run `pnpm gen`. `schemas/` and the pack SDK's generated types change.
- Area: `core/src/pack.rs`, `pack_check.rs`, `profile.rs`, `settings.rs`, a new settings registry and `yaml_out.rs`,
  `core/tests`, `fixtures/manifests`, `fixtures/config`, `schemas/`.
- The test rule: no test touches the real home, real launchd or a real daemon. Each test clears the environment
  (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as `host/tests/common/mod.rs` `TempRoot`
  does. Settings files are fixtures, never the owner's own `settings.json`.
- Stage 4a item (revision 23 scope, split 2026-11-16). Items of Stage 4a run one at a time, in id order, because
  they share `core`, `host` and the generated files.
- Owner decisions (2026-11-16, at the split): the recommendations above are taken under the owner's delegation; they
  stay overridable until the item is built.

### Built

The role keys, the tool `hint`, `repoOnly`, the settings registry, the `disk` and `claude` sections and the YAML
writer. No file is read at run time and no process runs. No crate was added. `pnpm gen` was run and its output kept
(`schemas/` and `packages/pack-sdk/src/generated/`); a second run changes nothing.

- `core/src/pack.rs`: `Role` gains `require` and `allow_tools` (lists of text, skipped when empty) and
  `repo_instructions` (`Option<RepoInstructions>`). `ToolRequirement` gains an optional `hint`. The docs of
  `SchemaObject` and `PackManifest::config` name `repoOnly`.
- `core/src/profile.rs`: `RepoInstructions` (`worktree` or `none`, as `RoleConfig` is written) and
  `RoleOptions::repo_instructions`. Absent stays `None`; the default `worktree` is the launch's (`ts-0104-gx`).
- `core/src/pack_check.rs`: three `ManifestError` variants. `UnknownCapability { role, name }` for a `require` entry
  that `Capabilities::support_of` does not know (it is the table the run-time check reads, so the two cannot drift).
  `BadAllowedTool { role, name }` for an `allow_tools` entry that is empty or has white space, `"` or `'`.
  `RepoOnlyConflict { at, flag }` for `repoOnly: true` with `overridable: true` or `secret: true`, naming the key and
  both flags. `Check::setting` also checks that `repoOnly` is a boolean (`NotABoolean`). `Check::role_lists` is new.
  `requires.tools[i].hint` is refused when empty, as `version` is.
- `core/src/settings_registry.rs` (new): `COLLIDE_SETTINGS` (a slice of `CollideSetting`: `key`, `value_type`, `min`,
  `max`, `default`, `overridable`, `repo_only`, `per_user`), `ValueType`, `SettingDefault`, `collide_setting(key)`,
  `overridable_keys()`, `repo_file_keys()` and `MIN_FLOOR_GIB`. The five keys, as the item lists them.
- `core/src/settings.rs`: `DiskSettings` and `ClaudeSettings` and the `disk` and `claude` fields of `ConfigFile`
  (closed; skipped when empty). `ConfigFile::collide_keys_set()`. `SettingsError::BelowMinimum` and its check in
  `check_config_file` (`disk.floor_gib: 0`). `repo_file_refusals` now refuses every set key the registry marks per
  user, so `claude.check_published` is refused and `disk.floor_gib` is allowed. `REPO_FILE_AGENT_ASK_KEYS` and
  `COLLIDE_OVERRIDABLE_KEYS` are gone, replaced by `repo_file_keys()` and `overridable_keys()`.
- `core/src/yaml_out.rs` (new): `to_yaml(&value, header) -> Result<String, serde_json::Error>`. See the rules in the
  module doc: every string double-quoted, plain word keys only, block style, `{}` and `[]`, floats keep `.0`, fields in
  declaration order. The value is written as JSON text and read into an ordered tree, so no `Serializer` is written
  and no map is sorted. `core/src/lib.rs` lists both modules.
- Fixtures: `fixtures/manifests/invalid/` (eight: `role-require-unknown-capability`, `role-allow-tools-empty`,
  `role-allow-tools-space`, `role-repo-instructions-bad`, `tool-hint-not-a-string`, `setting-repo-only-not-boolean`,
  `setting-repo-only-overridable`, `setting-repo-only-secret`), `fixtures/config/disk-and-claude.yaml` and four under
  `fixtures/config/invalid/` (`disk-floor-zero`, `disk-unknown-key`, `claude-unknown-key`,
  `repo-sets-check-published`).
- Two existing tests lists changed with the schema: `core/tests/planning_contracts.rs` (`ToolRequirement` keys) and
  `host/tests/schema_command.rs` (`ToolRequirement` and `ConfigFile` keys). `board_config_settings.rs`'s test of the
  two removed constants became `the_keys_a_repo_file_may_set_are_data_read_from_the_registry`.

**Tests.** None starts a process, reads a settings file of the owner's or touches a home: they are pure functions
over fixtures and strings, so no `env_clear()` is needed.

- `core/tests/pack_manifest.rs` (16 new, and 8 rows of the `INVALID` table):
  `a_role_that_requires_known_capabilities_and_sub_flags_loads`,
  `a_role_that_requires_a_capability_the_table_lacks_is_refused_naming_the_role_and_the_entry`,
  `a_sub_flag_the_table_lacks_and_an_empty_entry_are_refused_as_unknown_capabilities`,
  `every_capability_name_of_the_table_is_a_valid_require_entry`,
  `a_role_with_no_require_has_an_empty_list_and_writes_none`, `a_role_that_allows_a_tool_loads`,
  `an_allowed_tool_that_is_empty_or_has_a_space_or_a_quote_is_refused_naming_the_role`,
  `repo_instructions_is_worktree_or_none_and_absent_is_none_set`,
  `repo_instructions_and_the_role_keys_are_in_the_generated_role_schema`,
  `a_tool_requirement_has_an_optional_hint_that_round_trips`,
  `a_hint_that_is_not_a_string_fails_at_load_and_an_empty_one_is_refused`,
  `a_repo_only_setting_loads_and_the_flag_is_false_by_default`, `a_repo_only_flag_must_be_a_boolean`,
  `a_repo_only_setting_that_is_also_overridable_or_secret_is_refused_naming_the_key_and_both_flags`,
  `a_repo_only_setting_with_the_other_flags_false_is_fine`, `a_profile_role_of_a_pack_takes_repo_instructions`.
- `core/tests/views_profiles_locks.rs` (2): `a_profile_role_takes_repo_instructions_worktree_or_none`,
  `a_profile_role_with_a_bad_repo_instructions_fails_naming_the_key`.
- `core/tests/settings_registry.rs` (new, 13): `the_registry_holds_the_five_keys_in_order`,
  `each_key_has_its_type_range_and_default`, `a_default_is_written_as_the_json_value_a_file_would_hold`,
  `a_value_type_is_named_as_json_schema_names_it`, `the_keys_of_each_flag_are_listed_so_a_change_is_deliberate`,
  `the_helpers_read_the_flags`, `a_key_is_found_by_its_path_and_an_unknown_one_is_not`, `no_key_contradicts_itself`,
  `keys_are_unique`, `the_timeout_range_is_the_one_the_file_check_uses`,
  `every_key_collide_owns_in_config_file_is_in_the_registry_and_the_other_way` (reads `ConfigFile`'s generated schema),
  `a_file_that_sets_every_key_reports_exactly_the_registry_keys`, `the_registry_ranges_are_in_the_generated_schema`.
- `core/tests/board_config_settings.rs` (10 new, 1 rewritten): `a_file_with_a_disk_and_a_claude_section_loads`,
  `a_file_with_no_disk_or_claude_section_sets_neither_and_writes_neither`,
  `a_floor_of_zero_fails_naming_the_key_and_the_minimum`, `a_floor_of_one_is_valid`,
  `an_unknown_key_in_disk_or_claude_fails_naming_it`, `a_value_of_the_wrong_type_in_disk_or_claude_fails`,
  `the_disk_and_claude_sections_are_in_the_generated_schema_and_closed`,
  `a_repo_file_that_sets_a_per_user_key_is_refused_naming_it_and_a_team_key_is_allowed`,
  `a_repo_file_may_set_disk_floor_gib`; the schema-keys test now lists `claude` and `disk`.
- `core/tests/yaml_out.rs` (new, 25): the round trip of `Board`, the smallest and the empty `Board`, `PackLock` (order
  of fields, empty, exact text with a header), `UserPacks`, `ConfigFile` (three fixtures and a full one), `Profile`;
  `every_awkward_string_comes_back_as_the_same_string` (the item's table and about 50 more: control
   characters, `\u{85}`,
  `\u{2028}`, a BOM, emoji, `.inf`, `~`, `---`), the same strings as map values and as untyped values, a root string,
  the key rule, keys in nested maps, empty map and list, nesting, a list of maps, scalars and floats, untyped values,
  determinism, the final newline, a header that changes nothing read, a header that cannot break out, an empty
  header, and an error for a map with a key that is not text.
- **Seen red.** The skeleton (types, variants and fields with `todo!()` bodies) was compiled first, so the tests
  compiled. Red on that run: in `pack_manifest` 7 (`a_repo_only_flag_must_be_a_boolean`,
  `a_hint_that_is_not_a_string_fails_at_load_and_an_empty_one_is_refused`, the two `repo_only`/conflict tests, the
  three `require` and `allow_tools` refusal tests and
  `every_invalid_fixture_fails_where_the_table_says_and_names_what_is_wrong`);
  in `board_config_settings` 5 (floor of zero, the disk and claude section load, the per-user refusal, the wrong
  type, the registry keys); in `settings_registry` 10 of 13; in `yaml_out` all 25. Not red, because the key or type
  was part of the skeleton and has no behaviour to fail: the two `views_profiles_locks` tests and the loading cases
  of `pack_manifest` (`require`, `allow_tools`, `repo_instructions` and `hint` load and round-trip).
  `every_key_collide_owns_in_config_file_is_in_the_registry_and_the_other_way` was red against the empty registry.
- Checks: `cargo build --workspace`; `cargo test --workspace --no-fail-fast` (2994 passed, 0 failed; no flaky test
  failed this time); `cargo clippy --workspace --all-targets -- -D warnings`; `cargo fmt --all --check`; `pnpm test`;
  `pnpm typecheck`; `pnpm --filter @collide/app build`; `pnpm build:packs`; `pnpm check` all pass. A first full test
  run failed two `schema_command` tests in `host` that list the generated keys; they were updated (above).

### Decisions for the owner

1. **The registry's name and shape.** `core/src/settings_registry.rs`, a `const` slice of plain data with three small
   functions. The item said "derived from the registry or replaced by it": the two constants are replaced by
   `repo_file_keys()` and `overridable_keys()`, because a `const` array cannot be filtered at compile time. Anything
   that read the constants (only one test did) reads the functions. Alternative: keep both constants and add a test
   that they equal the registry. Recommend what is built: one list, no copy to drift.
2. **How the writer is built.** Value to JSON text to an ordered tree to YAML, about 200 lines with the docs. The
   item said about 80. A `serde::Serializer` is longer; going through `serde_json::Value` would sort every key
   (`packs.lock` would read `commit`, `source`, `tree`). Going through text keeps the order of the type's fields.
   Cost: a value is written twice in memory, which no file here makes matter. Recommend keeping.
3. **A plain key is stricter than the item's rule.** The item says plain when `[A-Za-z0-9_-]+`. Written so, the keys
   `true`, `null`, `1` and `-` could be read as a boolean, `null`, a number and a list marker (not measured for keys:
   the rule is cautious, and the tests only hold that quoted keys read back). So a key is plain only when it
   starts with a letter or `_`, and is not `true`, `false`, `null`, `yes`, `no`, `on`, `off`, `y` or `n` in
   any case. Any other key is quoted. Recommend the stricter rule; the output differs only for those keys.
4. **`to_yaml` returns `serde_json::Error`.** Its only failures are a map key that is not text and a float that is
   not finite, both bugs in the caller's type. Alternative: an error type of our own. Recommend keeping until a
   caller needs to tell the cases apart.
5. **The refusal text of `repo_file_refusals` changed.** The refused keys and their order are as before. The message
   now lists the allowed keys from the registry (`ask.timeout_s`, `ask.budget_usd`, `disk.floor_gib`) and says a
   per-user key is one person's, since `claude.check_published` is not "an agent". Every test that read the message
   still passes. Alternative: keep the old sentence and let it be wrong for per-user keys. Recommend the change.
6. **`disk.floor_gib: 0` fails in `check_config_file`, not at load.** As `timeout_s` does: serde has no range, so the
   file loads and the check refuses it (`SettingsError::BelowMinimum`, naming the key). The schema has `minimum: 1`.
   A test holds the schema's and the registry's ranges equal. `floor_gib: 1.5` and `-1` fail at load.
7. **Section types are `DiskSettings` and `ClaudeSettings`.** `Ask` is the older, shorter name. Recommend these names
   so a `Claude` type in `core` is not read as the adapter. They are in the schemas and the SDK's types.
8. **`require` accepts what `Capabilities::support_of` accepts.** That includes the aliases it reads
   (`permission_host.support`, `usage_report.support`, `context_options.support`, `mcp_servers.support`). A sub-flag
   the table lacks (`permission_host.telepathy`) and an empty entry are refused. Alternative: a separate list of
   names in `pack_check`. Recommend the single source.
9. **`allow_tools` checks the form only.** White space of any kind, `"`, `'` and the empty name are refused; a name
   twice is not. Whether a name is one the adapter disallows is `ts-0104-gx`. Alternative: refuse a repeat.
   Recommend leaving repeats be: they change nothing at launch.
10. **`repoOnly: false` beside `overridable: true` is fine.** Only `true` is a conflict. Recommend.
11. **`claude.check_published` is read twice for now.** `host/src/claude_versions.rs` keeps its own reader (as
    `ts-0098-nx` decides), and a user file with a `claude:` section now also loads as a `ConfigFile`. Both read the
    same key and agree.
12. **Plan text, proposed:** none for the plan's design. If the owner wants the writer's rules in the plan, one line
    under 0.7, Layers and sources: "Collide writes its own YAML files with every string double-quoted and keys quoted
    unless they are plain words."

- Owner decisions (2026-11-16, at the build): decisions 1 to 11 taken as recommended, under the owner's delegation.
  Decision 12's plan line is not applied: the writer's rules are the code's, and the plan names none. The registry's
  `ask.budget_usd` is not `overridable`, so the repo's value wins, as the owner chose at the split.

### Not done, by design

- Nothing reads `require`, `allow_tools` or `repo_instructions` at launch: `launch_spec` and `check_agent` still pass
  none (`ts-0104-gx`). A role's `allow_tools` names are not checked against any adapter's list.
- No layers, and no refusal of a `repoOnly` key in the user's file or a user profile (`ts-0098-nx`). This item only
  makes the flag loadable and refuses its bad pairs.
- `packs/plan/pack.yaml` has no `layout` setting yet, so no built-in pack uses `repoOnly`; adding it belongs with the
  plan pack's settings.
- No `agents.<id>.<field>` entries in the registry (`ts-0099-bw`), and none of the later stages' keys.
- `to_yaml` writes no file. The atomic write, the mode, and what to do about a comment in a file a person edited are
  the callers' (`ts-0097-kw`, `ts-0099-bw`, `ts-0100-pu`, `ts-0101-bd`).
- `host/src/claude_versions.rs` is not changed.
