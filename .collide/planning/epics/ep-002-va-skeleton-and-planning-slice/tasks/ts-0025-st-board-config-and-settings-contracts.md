---
id: ts-0025-st
kind: task
status: done
title: "Board, config and settings contracts"
owners: []
links:
  - { type: blocked-by, to: ts-0023-ch }
---

## Goal

`.collide/board.yaml` (the packs in order, each from a `repo:`, `git:` or `dev:` source, with `dev:` refused here; the default workflow; `profiles.default` and `profiles.rules`; source configuration; the status mapping, including the Projects v2 keys that Stage 6c builds; the checkout filter); `.collide/config.yaml` and the user's `~/.config/collide/config.yaml` (the settings layers, `overridable`, `secret: true` kept out of both files); the `.collide` pointer file (`dir: <path>`); and Collide's own settings keys, including `agents` (one entry per id, with `kind`, `command`, `args`, `model`, `config`, `read`, `tools`, `timeout_s`, `budget_usd`, `env`, `secrets`) and `ask` (`agent`, and the repo's defaults `budget_usd` and `timeout_s`).

## Success criteria

- A valid `board.yaml`, `config.yaml` and pointer file load through the hardened loader and validate. An unknown key fails, naming it.
- The `agents` and `ask` keys are in the generated schema. An `agents` entry with an unknown key, or with a tool kind other than `read` or `search`, fails validation. `timeout_s` outside 5 to 1800 fails.
- The built-in `claude-haiku` entry of 0.7 validates as written there.
- A `kind: external` entry needs no `command`. A `kind: acp` entry needs one.
- The repo-file rule is data the loader can read: which keys a repo `config.yaml` may set (only `ask.budget_usd` and `ask.timeout_s` under `agents`/`ask`) is a function over a parsed file, with a test. Applying the layers is Stage 4a.

## Tests

Write first: validation tests with valid and invalid fixtures for each file; the `agents` unknown key and bad tool kind; the `timeout_s` range; the repo-file key check.

## Notes

- IMPLEMENTATION_PLAN.md, 0.7 (the Collide directory, `board.yaml`, Settings, Agent entries), 0.9, 0.12, Stage 2: Skeleton, Contracts (`board.yaml`, `config.yaml`, the `agents` and `ask` keys).
- Area: `core` (types and schemas).
- Views and the lock files are `ts-0026-es`.
- Done when the criteria hold and `collide schema` lists the new keys.

### Built

- `core/src/board.rs`: `Board` (`board.yaml`: `packs` as `[{source}]`, `workflow`, `profiles { default, rules[{when, profile}] }`, `sources.github { repos, projects[{owner, number}] }`, `status_mapping.github { status_field, categories }`, `checkout_filter { exclude }`), `check_board`, `CollidePointer` (`dir`) with `check_pointer`, `BoardKey`, `LINE_KEYS` and `line_ignored_keys` (0.12: a line overrides `packs`, `workflow` and `profiles`; the rest is board-level).
- `core/src/settings.rs`: `ConfigFile` (`agents`, `ask`, `config` for pack settings), `AgentEntry` (`kind`, `command`, `args`, `model`, `config`, `read`, `tools`, `timeout_s`, `budget_usd`, `env`, `secrets`; every key optional so a layer can change the built-in entry field by field), `AgentKind` (`claude-code | acp | external`), `ToolKind` (`read | search`, so any other kind fails to parse and names the key), `Ask` (`agent`, `budget_usd`, `timeout_s`), `builtin_agents()` (the `claude-haiku` entry of 0.7), `check_config_file`, `refused_acp_args`.
- The repo-file rule: `REPO_FILE_AGENT_ASK_KEYS` (`ask.budget_usd`, `ask.timeout_s`) and `repo_file_refusals(&ConfigFile)`, which returns one refusal per `agents.<id>.<field>` (or `agents.<id>` for an empty entry) and `ask.agent`. `COLLIDE_OVERRIDABLE_KEYS` holds `ask.timeout_s`. `secrets_in_file(&ConfigFile, secret_keys)` finds a `secret: true` pack key in either file.
- Reused: `lock.rs` `PackSource::parse` (a board refuses `dev:`, the mirror of `UserPacksError::RepoSource`), `profile.rs` `RoleConfig` for an entry's `config`, `view.rs` `MAX_CEL_SOURCE_BYTES` for `profiles.rules[].when`, `view.rs` `is_namespaced_name`, `item.rs` `Category` for the status mapping.
- Schemas: `Board`, `ConfigFile`, `CollidePointer` registered in `core/src/schema.rs`; `schemas/` and `packages/pack-sdk/src/generated/` regenerated and byte-stable. `timeout_s` carries `minimum: 5, maximum: 1800` in the schema. `host/tests/schema_command.rs` pins the new keys in `collide schema`.
- Tests: `core/tests/board_config_settings.rs`. Fixtures: `fixtures/board/`, `fixtures/config/`, `fixtures/pointer/`, each with an `invalid/` folder; `fixtures/config/builtin-claude-haiku.yaml` is the 0.7 entry as written, and a test checks it equals `builtin_agents()`.

### Decisions for the owner (the plan does not spell these out)

- `board.yaml` key names are mine where 0.7 gives none: `workflow` (the default workflow), `sources.github.{repos,projects}`, `checkout_filter.exclude`, and `status_mapping.github.{status_field,categories}`. `categories` maps each category to a list of Projects statuses; the first one is what a category write sets (0.10, `setStatus`). One status under two categories is refused. The board's `packs` entry is `{source}`, the same shape as `packs.yaml` and the lock.
- What a `checkout_filter.exclude` glob is matched against (branch name or worktree path) is left to the checkout code.
- A non-default line's `board.yaml` that sets a board-level key is not an error in this contract: `line_ignored_keys` names the keys and the caller shows a notice, since 0.12 says only "comes from the default line".
- An `agents` entry other than `claude-haiku` must name its `kind` (nothing below it supplies one). Also enforced from 0.7: a budget on `acp` or `external` is a load error; an `env` line that ends in `KEY`, `TOKEN`, `SECRET` or `PASSWORD` with a non-empty value is a load error; and `--mdns`, any `--cors` and a non-loopback `--hostname` in an ACP entry's `args` are refused (0.3, 0.7).
- Pointer `dir` must be relative to the repo root with no `..` part (not in the plan; it keeps a committed file from pointing outside the repo).
- Pack settings in `config.yaml` are `config: { <pack>: { <key>: value } }`, the shape a profile uses. "Per board" scoping in the user's file is not modelled yet.
- Other Collide keys the plan names without a key (concurrency cap, metrics level, retention, model aliases, `defaultRoot`) are not added.

### Not done, by design

- Applying the layers, the `overridable` flip, the built-in layer merge, the pack `config:` schema with `overridable` and `secret: true` (`ts-0012-ua`; `secrets_in_file` takes the secret keys as data), the keychain, whether `ask.agent` names an entry, resolving pack, workflow and profile names, and launching agents are Stage 4a and later. CEL syntax is Stage 4b.
