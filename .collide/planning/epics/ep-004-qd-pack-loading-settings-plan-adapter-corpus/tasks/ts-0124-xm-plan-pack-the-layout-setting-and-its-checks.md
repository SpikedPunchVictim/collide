---
id: ts-0124-xm
kind: task
status: done
title: "Plan pack: the layout setting and its checks"
owners: []
links:
  - { type: blocked-by, to: ts-0123-yw }
---

## Goal

The markdown provider reads its shared root from the `layout` setting of `collide/plan`, and refuses a path it must not
use. The root is no longer a constant.

What exists. `packs/plan/src/scan.ts` holds `SHARED_ROOT = '.collide/planning'`; `scanEpics`, `readEpicFiles`,
`packs/plan/src/ids.ts` (`idsInPaths`, `takenIds`), `create.ts`, `write.ts` and `reid.ts` all build their paths from it.
`packs/plan/pack.yaml` declares no `config:`, and `planHandlers.initialize` in `packs/plan/src/provider.ts` reads only
`root`. The settings layers are built (`core/src/settings_layers.rs`: `repoOnly`, `overridable`; the JSON Schema subset
of `core/src/setting_value.rs`), the daemon passes a pack's settings in `initialize` (`host/src/sources.rs`), and
`.collide/config.yaml` is in the trust hash (`core/src/domain.rs`, `host/src/board_load.rs`). The direct CLI
(`host/src/task_cli.rs`) sends an empty `config`, so it cannot see `layout` until `ts-0136-rr`.

What this adds.
- `packs/plan/pack.yaml` gains `config.layout`, an object with `root`, `paths`, `code` and `previous`, marked
  `repoOnly: true` and not `overridable`. The default `root` is `.collide/planning`. The schema stays inside the
  supported subset (no `$ref`, no `oneOf`).
- `packs/plan/src/layout.ts` (new) reads `config.layout` at `initialize` and checks it. Every path is repo-relative,
  with no `..` part, not absolute and not empty; it is not `.git` or under it; and it is not a protected file or over
  one (`board.yaml`, `packs.lock`, `config.yaml`, `views/`, `profiles/` of the Collide directory; 0.9). A path whose
  existing part is a symbolic link that leaves the repository is outside it. A refusal names the rule and, for a
  protected file, the file the path would have covered. `initialize` then fails with that reason, which the daemon shows
  as it shows any source problem.
- Scan, ids, create, write and reid use the resolved root. New items go to the root; ids stay unique across every ref
  and across the old and new root.
- `layout.previous`: each entry is checked like `root`, and items found under it are listed. A write to such an item
  stays at its path. Move, and the "old path" chip, are not here (see Notes).
- `layout.paths.spike` and `layout.code` are accepted by the schema; `ts-0126-nf` gives `paths.spike` its meaning.

## Success criteria

- A `root` of `../outside`, `/abs/path`, `.git/planning`, `.collide/config.yaml`, `.collide/views` or a path through a
  symbolic link that leaves the repository is refused at `initialize`, and each message names the rule. The protected
  cases name the file.
- With `layout.root: docs/plan` over a fixture repository, the provider lists the items under `docs/plan/epics/`,
  creates new items there, and writes nothing under `.collide/planning/`.
- With no `layout`, nothing changes: every existing `packs/plan` test passes unedited.
- `layout` in the user's file is refused with the key named, and in the repo's file is accepted: the manifest's schema
  carries `repoOnly: true`, checked through the real layers with `packs/plan/pack.yaml`.
- A change of `layout.root` in the repo's `.collide/config.yaml` changes the line's trust hash.
- Items under a `previous` entry are listed, with the id numbering and the collision rules unchanged.

## Tests

Write these first.

- Provider tests over temp repositories: each refusal above, a symbolic link out of the repository, a root with a
  different depth, `previous` with items in two roots, ids unique across both, and the default path with no setting.
- A `core` test that loads `packs/plan/pack.yaml` through `check_manifest` and `check_setting_schema`, and runs the
  layers with the key in the user's file and in the repo's file.
- A trust-hash test with a fixture `config.yaml` before and after a `layout` change (the existing hash function, no
  daemon).
- The daemon path, once, in `host/tests/daemon_sources.rs`'s style: a temp root, a repository with `layout.root` set,
  `task.list` shows the item from the new root.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 4c: Planning ("the private root and the layout setting"); Success Criteria ("a layout path outside the repo
    or under `.git/` is refused with the reason"); Tests ("layout checks").
  - 0.10: Planning sources (two roots; "the path is set in `.collide/config.yaml` and is part of the trust hash"; "after
    a change, new items go to the new path"). 0.7: Keys added for planning; Settings (`repoOnly`). 0.9: Trust and
    protected files. R5.
  - `docs/research/planning.md` 2.4, "The path setting" (the checks and the `previous` table).
- **Owner's choice, with a recommendation: what `paths.<kind>` and `code` mean.** The plan lists the keys and no more.
  `docs/research/planning.md` shows `paths.spike: docs/decisions` and `code: spikes`, but the plan has since put spikes
  in the shared root. Recommend: in 4c only `paths.spike` can be set (where the spike folders live, default
  `<root>/spikes`); another kind in `paths` is refused with "only spike can be placed: the other kinds live in their
  epic's folder". `code` is accepted and has no effect until Stage 9's spike workflow. Alternative: `paths.<kind>` for
  every kind, as a flat folder. More flexible, but the epic then cannot come from the folder, which breaks 0.10's
  "the folder is the epic membership". This sets what a team can configure.
- **Decision for the item, with a recommendation: where the check lives.** In the provider (`layout.ts`), because
  `layout` is this pack's own key and core stays generic. Alternative: a pack-specific validator hook in core. No: a
  new mechanism for one key.
- **Decision for the item: `previous`.** Read and list, never write elsewhere. Move (`git mv` in one commit) is Stage 12
  and the "old path" chip is Stage 6a. The entry's removal rule ("only when nothing remains there") is Stage 12's with
  Move.
- Area: `packs/plan/pack.yaml`, `packs/plan/src/layout.ts` (new), `scan.ts`, `ids.ts`, `create.ts`, `write.ts`,
  `reid.ts`, `provider.ts`, `packs/plan/test/`, `core/tests/` (the manifest and layers test), `host/tests/`.
- The test rule: no test touches the real home, real launchd or a real daemon. Each test that runs the `collide` binary
  clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as
  `host/tests/common/mod.rs` `TempRoot` does. Settings files are fixtures, never the owner's own `settings.json`. A temp
  repository sets `GIT_CONFIG_GLOBAL` and `GIT_CONFIG_SYSTEM` to empty files.
- Stage 4c item (revision 23 scope, split 2026-11-18). Items of Stage 4c run one at a time, in id order, because they
  share `core`, `host`, `packs/plan` and the generated files.
- Nothing here runs a real `claude`. No dependency is added.
- Owner decisions: none recorded yet. The recommendations stand until the owner answers at the split.

### Built

The root is a setting now. `SHARED_ROOT` is gone; scan, ids, create, write and reid take a `Layout`, and with none
given they use the default one, so no existing `packs/plan` test changed.

- `packs/plan/pack.yaml`: `config.layout`, an object with `root`, `previous`, `paths` and `code`, `repoOnly: true`, no
  `overridable`, default `{ root: .collide/planning }`, `additionalProperties: false`. Inside the supported subset.
- `packs/plan/src/layout.ts` (new): `resolveLayout(repoRoot, config.layout)` returns a `Layout`
  (`root`, `previous`, `spike`, `code`) or throws `LayoutError`. `checkPath` runs on every path: text, no control
  character, no backslash, not absolute, no `..` part, not empty, not `.git` or under it, not a protected file or under
  one (`.collide/board.yaml`, `packs.lock`, `config.yaml`, `views/`, `profiles/`; the message ends "it would cover
  <file>"). Then `resolveLinks` walks the path from the real repository root, one name at a time, with `lstat`: a link
  that leaves the repository or points at nothing is refused, and a link that stays inside is resolved, so the root is
  the path git knows. The rules run again on the resolved path (a link to `.git` is refused as `.git`). Case is ignored
  for `.git` and the protected names. `layout.paths` takes only `spike`; another kind is refused with "only spike can be
  placed: the other kinds live in their epic's folder". `code` is checked as a path and used for nothing. An unknown key
  of `layout`, or a value of the wrong type, is refused naming the key.
- `provider.ts`: `initialize` resolves the layout before it keeps anything, so a refused layout fails the call with the
  reason (the daemon shows it as it shows any start failure) and leaves the provider as it was. `list`, `get` and
  `apply` pass the layout on.
- `scan.ts`: `readEpicFiles` and `scanEpics` read the root, then each `previous` root. `ids.ts`: `idsInPaths`,
  `takenIds` (refs and working tree) and the `look` hook take the layout's epics folders. `create.ts`: new items go
  under `layout.root`; an epic that only a previous root holds is refused with where it is. `write.ts`: the lock file is
  in the root, a write to an item stays at its path, and a root that is not there yet is made when a previous root has
  items. `reid.ts`: reads every root and renames a file where it is.
- `core`: no code change. `host/tests/config_command.rs`: `with_no_key_every_setting_is_listed_one_to_a_line_in_order`
  lists `collide/plan.layout`, because the built-in pack now has a setting.

Tests (names are the `describe` and `test` titles; each green now):
- `packs/plan/test/layout.test.ts` (56 tests): each refusal with its rule and file; symbolic links (out of the
  repository, a chain, a dangling one, one to `.git`, one to a protected folder, one that stays inside); `previous`,
  `paths` and `code` checks; no layout changes nothing; `layout.root: docs/plan` (list, get, create, write, reid, other
  depths, ids across refs); `previous` (both roots listed, numbering, an id on an old-root branch, the same id in both
  roots, a write stays in place, a write while the new root is missing, create into an old-root epic refused).
- `core/tests/plan_layout_setting.rs` (9): the real `packs/plan/pack.yaml` through `check_manifest` and
  `check_setting_schema`, and through `settings_layers`: refused in the user's file and per-board file naming the key,
  accepted in the repo's file, default says default.
- `host/tests/trust.rs`: `a_committed_change_of_the_plan_layout_root_in_the_repos_config_changes_the_hash`. Green at
  once: the trust hash already covered `config.yaml`; the test holds that.
- `host/tests/daemon_plan_layout.rs`: `task_list_shows_the_items_of_the_root_the_repos_layout_names` and
  `a_layout_the_provider_refuses_is_an_error_on_the_source_naming_the_rule_and_the_file`, with the real built-in plan
  pack from a built bundle.

Red at runtime: before the change `layout.test.ts` had 50 of 56 failing (the other 6 are the no-layout and not-refused
cases) and `plan_layout_setting.rs` 7 of 9. After the code compiled, `resolveLayout(given, config?.layout)` in
`initialize` was stubbed to `resolveLayout(given, undefined)` (the old behaviour): 48 of 56 failed, then all passed with
it put back. The `daemon_plan_layout.rs` tests were written against the built bundle and not run against the old code:
the old provider ignores `config`, so the first lists the default root and the second does not fail.

### Decisions for the owner

1. `paths` and `code` (taken as the item recommends, on the owner's delegation). Only `paths.spike` can be set. Another
   kind is refused with the reason. `code` is accepted and does nothing until Stage 9. Both are checked like `root`.
   Alternative for `code`: accept any text. Recommend as built: a bad path is better found now than at Stage 9.
2. `paths.<kind>` other than `spike` is refused by the provider, not by the schema. The schema lets `paths` hold any
   text values so the person reads the reason above, and not "is not a property the schema lists". The direct CLI sends
   no `config`, so the provider checks anyway. Alternative: `additionalProperties: false` in the schema. Recommend as
   built.
3. A root of `.` or an empty text is refused ("empty"): it would put `epics/` at the repository root. Alternative:
   accept `.`. Recommend as built.
4. A symbolic link that stays inside the repository is accepted and resolved; the root is then the real path. Git does
   not follow links, so refs would otherwise be read at the wrong path and an id on another branch could be reused.
   Alternative: refuse any link in the path. Recommend as built.
5. A `previous` entry equal to the root (after resolving) is dropped, so an item is not read twice. Alternative: refuse
   it. Recommend as built: Stage 12 owns when an entry leaves.
6. `create` into an epic whose folder is only in a previous root is refused, naming the folder. A new item goes to the
   root, and an epic's folder is its membership, so the item cannot go to the old folder or to a new folder of the same
   epic. Move (Stage 12) fixes it. Alternative: put the task in the old epic folder. Recommend as built.
7. Ids are made unique across the root and the `previous` roots only. A branch forked before the change that still has
   items at `.collide/planning`, with no `previous` entry for it, is not seen. Alternative: always read the default root
   too. Recommend as built: `previous` is the plan's mechanism, and `doctor` (Stage 12) reports the rest.
8. The lock file is always in the root. A write to an old-root item with no root folder yet makes the empty root folder
   for the lock. Alternative: lock in the item's own root, which would not exclude a `create` in the new root. Recommend
   as built.
9. Backslashes in a path are refused, not read as `/`. A name with a backslash is legal on a POSIX file system but is a
   separator on Windows, so it would mean two things.
10. Plan text to propose (do not edit the plan from here): in 0.10, Planning sources, after "is refused, and the message
    names the file it would have covered (R5)", add "A symbolic link in the path is followed to its target, which must
    be inside the repository; the root is then the target. `layout.paths` takes only `spike` and `code` is accepted
    until Stage 9."

- Owner decisions (2026-11-18, at the build): `paths.<kind>` and `code` as the item recommends, on the owner's
  delegation (only `paths.spike`, `code` inert until Stage 9); the rest as built. Decision 10's plan text applied to
  0.10, Planning sources.

- Found at review: the builder's report said `cargo test --workspace` passed, but `host/tests/config_get.rs`
  `a_packs_keys_show_as_pack_dot_key_and_its_defaults_say_default` failed on the new `collide/plan.layout` key; its
  expected list now names it, as `config_command.rs` does.

### Not done, by design

- `layout.paths.spike` has no meaning yet: `ts-0126-nf` gives it one. `code` waits for Stage 9.
- Move, the "old path" chip and the rule for removing a `previous` entry: Stage 12 and Stage 6a, as the Notes say.
- The direct CLI (`host/src/task_cli.rs`) sends an empty `config`, so it uses the default root and cannot see `layout`
  until `ts-0136-rr`. Until then `collide task create` in a repository with a `layout` writes to `.collide/planning`.
- A named checkout is checked for links against the main checkout's real path only; the links are committed, so the
  checkouts agree.
- The write lock (`.write-lock`) and a killed write's `.collide-tmp` file are git-ignored only under .collide/planning,
  by this repository's `.gitignore`. Under another root they show in `git status` while a write runs or after one is
  killed. No step writes ignore lines into a repository yet; found at review.
