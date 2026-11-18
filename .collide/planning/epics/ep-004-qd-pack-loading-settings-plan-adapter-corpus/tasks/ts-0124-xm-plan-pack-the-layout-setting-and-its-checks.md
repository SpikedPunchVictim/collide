---
id: ts-0124-xm
kind: task
status: ready
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
