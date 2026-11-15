---
id: ts-0012-ua
kind: task
status: active
title: "pack.yaml contract"
owners: []
links:
  - { type: blocked-by, to: ts-0023-ch }
  - { type: blocked-by, to: ts-0024-uv }
  - { type: blocked-by, to: ts-0026-es }
---

## Goal

`pack.yaml` in full, as one type with its schema: `apiVersion`, `name`, `version`, `requires` (`sdk`, `tools`, `agents`, `modelName`), `config` (one JSON Schema per key, `secret`, `overridable`), roles, prompts and definitions, hooks and implementations (`workspace`, `integrate`, `sync`, with their sub-hooks and timeouts), gates, bindings with `when:` and `timeout:`, `points:`, `views:` and `profiles:` (the View and Profile types of `ts-0026-es`), `checklists:`, observers with `events:`, `record` paths, `protectedPaths`, `metrics` and `experiment` (the keys used only in Milestones C and D), and `workflows` from `ts-0024-uv`. Replace the placeholder manifests of the five pack directories with real ones.

## Success criteria

- The two built-in packs' manifests (`packs/collide`, `packs/plan`), the reference pack's fixture copy and both fixture packs' manifests validate against the generated schema.
- A manifest with an unknown key fails, at any depth.
- A required string field has `minLength: 1`.
- A gate bound to an `after_*` point is refused at load, with a message that names the point.
- A `script`, `wait` or hook path is relative to the pack directory, and `repo:` marks a path in the line's snapshot.

## Tests

Write first: manifest validation over all five manifests plus invalid fixtures, including S5's hazards read through the loader of `ts-0023-ch` (`no`, `0.10`, a duplicate key, an alias bomb, deep nesting); a gate on an `after_*` point.

## Notes

- IMPLEMENTATION_PLAN.md, 0.4 (Contribute, named points, gates), 0.7 (the manifest, its example, paths), 0.9. The S5 record: `.collide/planning/spikes/sp-0005-fv-schema-expressions/record.md`.
- Area: `core` (manifest type, load-time checks), the five `pack.yaml` files.
- `board.yaml`, `config.yaml`, views and the lock files are `ts-0025-st` and `ts-0026-es`.
- Done when the criteria hold and `collide schema` lists the manifest keys.
- `ts-0029-ea` started `PackManifest` with `apiVersion`, `name`, `requires.tools` and `providers.<name>.script`, all optional so the built-in packs load. Make `apiVersion` and `name` required here, with the rest of the manifest check.

### Built

- `core/src/pack.rs`: `PackManifest` in full, extended in place. `apiVersion` and `name` are now required (no `Default`); every other key is optional. New types: `SchemaObject` (a JSON Schema as data), `HookEntry` (`script`, `timeout`, `grace`, `outputs`), `WorkspaceImpl` (`create` required, then `sandbox`, `line`, `resume`, `doctor`, `remove`, `discard`, `cleanup`), `Record`, `ProtectedPaths` (a list, or `{ paths, at: phase-end }`), `Metrics`, `Experiment`, `Role`, `Binding`, `ObserverEntry`; `Requires` gained `sdk`, `agents`, `modelName`. Keys: `apiVersion`, `name`, `version`, `requires`, `launch`, `config`, `workspaces`, `integrations`, `syncs`, `record`, `protectedPaths`, `metrics`, `experiment`, `roles`, `prompts`, `gates`, `bindings`, `observers`, `providers`, `points`, `checklists`, `workflows`, `views`, `profiles`. Every struct is closed; `ProtectedPaths` reads a list or a mapping itself so that an unknown key in the mapping is named.
- `minLength: 1` (`#[schemars(length(min = 1))]`) on every required string of the manifest: its own types, and the required strings of `workflow.rs` (`role`, `script`, `wait`, `human`, `end`, `artifact`, `op`), `View.name` and `DerivedPoint.on`. The check refuses the same strings when empty, so the schema and the check agree.
- `core/src/pack_check.rs`: `check_manifest` (every refusal in one pass; `ManifestError` with a message that says where), `parse_pack_path` (`PackPath::InPack` or `InRepo`; refuses empty, absolute, `..`, and another scheme such as `dev:`). It runs `workflow_check::validate` over each `workflows:` entry, `check_view`, `check_profile` and `check_derived_points`, and refuses: a gate bound to an `after_*` point (the message names the point and the gate), a binding with no point, no target, or both, `script`/`wait`/hook/prompt/definition paths and `record.paths`/`protectedPaths` globs that are not relative, a zero hook timeout, `secret`/`overridable` that are not booleans, `compact_at` under the floor of `profile::MIN_COMPACT_AT`, a name that is not one word, an observer with no events, and `when:` over 1 KB on a binding, a derived point or a checklist item. `points::side` became `pub(crate)` to read a point's side.
- `host/src/builtin_packs.rs`: the built-in loader now runs `check_manifest` and returns `BuiltinPackError::InvalidManifest` with every refusal; `WrongName.found` is a `String`.
- Manifests: `packs/plan` and the orchard fixture copy needed no change (the fixture is the plan's example, and it now loads as a full `PackManifest`). `packs/collide` keeps its workflow, with a comment that says what it declares. `fixtures/packs/loop` and `fixtures/packs/pr-ci` gained the keys their workflows name (requires, launch, config with both flags, integrations, roles, prompts, a gate, a binding, an observer).
- Tests: `core/tests/pack_manifest.rs` (44 tests), with `core/tests/schema_validate/` (a small JSON Schema validator for tests, since the workspace has no validator crate: it reads only the keywords `schemars` writes and fails on one it does not know). It validates all five manifests against `PackManifest.input.json`, round-trips them and validates the output side, pins an unknown key at 28 depths (serde and the schema both refuse), `minLength` in both schemas, gates on every `after_*` point, paths, and each invalid fixture. Fixtures: `fixtures/manifests/minimal.yaml` and `fixtures/manifests/invalid/` (33 files, each in a table with where it fails and a word its message holds; a test fails for a file not in the table). The S5 hazards are there as files: duplicate key, anchor and alias, merge key, unquoted `0.10`, unquoted `{{ }}` (hint "quote the template"), 40-deep nesting; `no` and a quoted `"0.10"` stay strings. Updated: the key-list pins in `core/tests/planning_contracts.rs` and `host/tests/schema_command.rs`, `views_profiles_locks.rs` (the manifests there now carry the identity keys), `packages/pack-sdk/test/planning-types.test.ts` (the manifest type needs `apiVersion` and `name`). `host/tests/builtin_packs.rs` has a test for `InvalidManifest`. Schemas and the SDK types were regenerated and a second `pnpm gen` changed nothing.

### Decisions for the owner

- **The `views:` key is the view's name.** `check_manifest` refuses a view whose `name:` differs from its key and names both. The other rule (key is the name, `name:` optional) would need `View.name` optional, which the loose view files do not want.
- **`bindings:` is a decision.** The plan describes bindings (0.4: `task.before_status: review`, `when:`, `timeout:`, derived points) and gives no manifest key. A list of `{ point, match?, gate | script, when?, timeout? }`: a list because chain handlers run in the order written; `gate:` names a pack gate and may bind only to `before_*`; `script:` is a chain handler for any point. `point` is a core point, a key of this pack's `points:`, or `pack/point` (resolved when a board loads). Pack names and gates named in a binding are not resolved here.
- **`syncs:` is a decision.** 0.4 names `sync` implementations beside `workspace` and `integrate`; the example has no key. `syncs:` mirrors `integrations:`.
- **`workspaces.<name>.create` is required** (a workspace cannot be made without it); the other sub-hooks are optional.
- **A pack profile covers only the pack:** `packs:` is refused, `workflow:` must be `<this pack>/<name>`, and `config:` may hold only this pack's name. This is the rule `ts-0026-es` left open ("only that pack's own settings"). `launch`, `models` and `roles` in a pack profile are not narrowed.
- **A path with another scheme** (`dev:`, `git:`, `C:`) is refused, because only `repo:` marks a path in the snapshot and `dev:x` would otherwise read as a file called that.
- **`apiVersion` is any non-empty string.** Refusing everything but `collide/v0` is one line when `collide/v1` exists.

### Not done, by design

- Running hooks, gates, bindings and observers, resolving `repo:` paths on disk, checking that a script file exists, and the pack-directory rule for a `node_modules` or `package.json` (0.6; the materialization stage).
- Resolving names across the manifest or across packs: a phase's `role`, `prompt`, `gates`, `workspace` and `integrate` against `roles`, `prompts`, `gates`, `workspaces` and `integrations`, and a binding's `gate` against `gates`. `packs/collide` names a role, prompts, `merge-local` and a decision script that its directory does not hold yet, so the load cannot resolve them before Stage 3.
- Settings layering, which keys are `secret` or `overridable` per layer (the settings stage; `settings::secrets_in_file` takes the secret keys), checking a `config` value or a `launch` value against its schema, and whether `config`, `launch` and `outputs` entries are valid JSON Schema (only "is a mapping" and the two flags are checked).
- CEL parsing of `when:` (only the 1 KB cap), a regex check of `requires.modelName`, version ranges in `requires`, and matching an observer's `events:` against the event catalogue.
