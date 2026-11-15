---
id: ts-0026-es
kind: task
status: done
title: "Views, profiles and the lock files"
owners: []
links:
  - { type: blocked-by, to: ts-0023-ch }
---

## Goal

Views, profiles and the lock files, as types with schemas: a view (`name`, `where` as CEL text, `include`, `exclude`, `closure`, `workflow`, `profile`, and `layout` with `default`, `groupBy` and the new `sort`); a profile (`description`, `packs`, `workflow`, `launch`, `models`, `roles`, `config`), found in `.collide/profiles/`, in the user's profiles folder, or in a pack's `profiles:`; `.collide/packs.lock` with its `source` field, the user's `packs.yaml`, and the user's `packs.lock`. This item parses the CEL source as text only, and enforces the 1 KB source cap.

## Success criteria

- A view with `layout: { default: board, groupBy: category, sort: { by: id, dir: asc } }` validates, and `layout.sort` is in the generated schema. A sort with an unknown `dir` fails.
- A `where` over 1 KB fails validation.
- A profile validates, and a profile key a user profile may not set is representable as data for the load-time refusal (Stage 4a applies it).
- A lock entry carries `source`, commit and tree id, and the `source` field accepts a value for a package manager later without a schema change.
- An unknown key fails in every file.

## Tests

Write first: validation tests for a view, a profile and both lock files, with the invalid fixtures above.

## Notes

- IMPLEMENTATION_PLAN.md, 0.7 (`board.yaml`, `packs.lock`, Repo packs and user packs, Profiles), 0.10 (Views), 0.11 (a sort is saved as `layout.sort`), 0.9 (the 1 KB source cap), Stage 2: Skeleton, Contracts (views, profiles, `packs.lock`, the user's `packs.yaml`, `layout.sort`).
- Area: `core` (types and schemas).
- The CEL runtime and its budgets are `ts-0044-xq` and Stage 4b.
- Done when the criteria hold.

### Built

- `core/src/view.rs`: `View` (`name`, `where`, `include`, `exclude`, `closure`, `workflow`, `profile`, `layout` with `default`, `groupBy`, `sort {by, dir}`, and `queue`), `MAX_CEL_SOURCE_BYTES` (1024), `check_view` (pure; every refusal in one pass).
- `core/src/profile.rs`: `Profile`, `check_profile`, `USER_PROFILE_KEYS`, `Profile::keys_set` and `user_profile_refusals`, which returns one `UserProfileRefusal { key }` per key a user profile may not set (`workflow`, `launch`, `models`, `roles`), Display "a user profile may not set `workflow`". `MIN_COMPACT_AT` (100,000) is checked per role.
- `core/src/lock.rs`: `PackLock` (used for both `.collide/packs.lock` and the user's `packs.lock`: one schema, `packs: [{source, commit, tree}]`), `UserPacks` (`packs: [{source}]`), `PackSource::parse`, `check_lock`, `check_user_packs`.
- Schemas: `View`, `Profile`, `PackLock`, `UserPacks` registered in `core/src/schema.rs`; `schemas/` and `packages/pack-sdk/src/generated/` regenerated and byte-stable.
- `PackManifest` gained `views:` and `profiles:` (maps of `View` and `Profile`), the minimum for a pack's `profiles:` and the manifest's `views:` that the plan names; ts-0012 owns the rest. Three existing key-set assertions (`core/tests/planning_contracts.rs`, `host/tests/schema_command.rs`) list the two new keys.
- Tests: `core/tests/views_profiles_locks.rs`. Fixtures: `fixtures/views/`, `fixtures/profiles/`, `fixtures/locks/`, each with an `invalid/` folder (the plan's example view and profile, plus failure cases).

### How `source` stays open

A source is text, `<scheme>:<rest>`. The schema fixes only the shape (`^[a-z][a-z0-9+.-]*:.+$`) and names no scheme, so `npm:...` later needs no schema change. `PackSource` splits it into `Repo`, `Git`, `Dev` or `Other { scheme, rest }`; an unknown scheme is `Other` and passes the lock check. A lock refuses `repo:` and `dev:` (neither is pinned by an entry); `packs.yaml` refuses `repo:`. `commit` and `tree` are required for every source (0.7: "every source resolves to a git tree id"), 40 or 64 lowercase hex.

### Decisions for the owner (the plan does not spell these out)

- Lock and `packs.yaml` shape: a `packs:` list, not a map by pack name, because a lock entry is keyed by its `source` (duplicates refused) and the pack's name is not known without fetching it. ts-0025 may want the board's pack list to use the same `{source}` entry shape.
- `profile.packs` is `{ on, off }`; the plan shows only `off`. `on` is added so a profile can switch a pack on that is off by default.
- `layout.default` is `board | run-order` (0.10 says "a board or a run order"; the table layout is deferred). `closure.follow` is the single value `depends_on`. `layout.groupBy` and `layout.sort.by` stay free text: the Field and Tracks know their own keys.
- `queue: true` is on the view (0.10, Planning sources), though the item's list did not name it.
- A user profile may set `description`, `packs` and `config` only; whether those `packs` are user packs and those `config` keys `overridable` needs the loaded packs, which is Stage 4a.
- `compact_at` under 100,000 is refused in a profile's `roles` (0.3), so Stage 3b can reuse the constant.
- A pack's `views:` is a map by name, and a `View` also requires its own `name:`, so the two can disagree. ts-0012 (the manifest checks) decides: refuse a mismatch, or treat the key as the name.
- Types and checks share one module each (`view.rs`, `profile.rs`, `lock.rs`), not a `_check` module beside each, because each is small.

### Not done, by design

- CEL parsing, the cost estimate and evaluation (`ts-0044-xq`, Stage 4b); the `git:` locator grammar (`<url>//<path>@<ref>`), fetching, and comparing a pin with a fetched tree (Stage 4a); the pack-profile scope rule ("only that pack's own settings"); `board.yaml`'s `profiles.default` and `profiles.rules` (ts-0025).
