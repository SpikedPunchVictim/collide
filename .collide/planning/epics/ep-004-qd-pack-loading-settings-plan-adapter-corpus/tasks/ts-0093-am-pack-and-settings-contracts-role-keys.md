---
id: ts-0093-am
kind: task
status: ready
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
