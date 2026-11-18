---
id: ts-0126-nf
kind: task
status: ready
title: "Plan pack: the spike kind"
owners: []
links:
  - { type: blocked-by, to: ts-0125-uh }
---

## Goal

The provider reads a spike record as an item of kind `spike`, so Collide's own records in `.collide/planning/spikes/`
are items, and never opens anything else in a spike folder.

What exists. `packs/plan/src/scan.ts` reads `epics/` only; its comment says `spikes/` and `questions/` are never
opened. `KNOWN_KINDS` is `task`, `epic` and `proposal` (`packs/plan/src/item.ts`). The ten records
`.collide/planning/spikes/sp-0001-de-*` to `sp-0010-bn-*` each have a `record.md` with frontmatter (`id`, `alias`,
`kind: spike`, `status: concluded`, `title`, `owners`, `links`), and sit beside scripts, `out/`, `work/` and
`node_modules`. A spike has no epic folder.

What this adds.
- Kind `spike`, with the statuses of 0.10: `planned` (`ready`), `running` (`active`), `results` (`review`),
  `concluded` (`done`) and `dropped` (`dropped`), declared in `initialize` as the proposal kind declares its own.
- The reader: it lists `<root>/spikes/`, and for each folder named `sp-<number>-<two letters>[-<slug>]` it reads
  `record.md` and nothing else in the folder. It never lists inside the folder, so an unreadable or huge subfolder
  costs nothing. A folder with another name is a `task.invalid`, with the rule. A spike has no `part-of`.
- The link type `informs` (out, does not block), from a spike to what it informs. The `link` op's `type` list gains it.
- `create` of kind `spike`: `spikes/<id>-<slug>/record.md`, status `planned`, a body of one heading. Ids use `sp-` and
  four digits, and `idsInPaths` and `takenIds` know the folder. `reid` handles the kind.
- `layout.paths.spike` (from `ts-0124-xm`) moves the folder; `previous` and the private root list spikes too.
- A spike in the private root works as any private item.

## Success criteria

- Over a fixture holding the frontmatter of Collide's ten records and one `results` record, each lists as kind `spike`
  with its status and category (`concluded` is `done`, `results` is `review`), its `path` and no `part-of`.
- A spike folder with `node_modules/`, `work/`, `out/`, `instances/` (holding another `record.md`) lists exactly one
  item, and a subfolder with mode `000` does not make the scan fail.
- `create` of kind `spike` writes the folder and record, the item loads, and its id is unique across every ref.
- `setStatus` moves a spike through `planned`, `running`, `results` and `concluded`, and `dropped` from any; a status
  that is not one of the five (`open`, an Orchard word) is a `task.invalid` that lists the five.
- The slice's criterion holds from here: no `task.invalid` for any hand-written item under `epics/` or `spikes/`.

## Tests

Write these first.

- Provider contract tests over fixture spikes: the ten-record shape, an unknown status, a badly named folder, a
  symbolic-link `record.md` (not followed), the S5 hazards in a record (an unquoted `no`, a hash like `1e10`, a
  duplicate key, an alias bomb, a file over the cap), the unreadable subfolder, a `record.md` in a nested folder.
- `create`, `setStatus`, `link informs` and `reid` on a spike; the numbering with a spike on another branch.
- `layout.paths.spike` and a `previous` root holding a spike.
- A read-only test against a copy of this repository's records' first lines: they list with no `task.invalid`.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 4c: Goal ("This adds the `spike` kind, so the slice reads `spikes/` from here"); Success Criteria ("Collide's
    spike records ... list ... and no item from inside a spike folder other than its record").
  - 0.10: Planning sources (layout; kinds, "A `spike`"; "Collide's own records are spikes"; the build-task rule's last
    bullet). R5.
  - `docs/research/planning.md` 7.2 (the spike lifecycle) and 4.2 (frontmatter).
- **Decision for the item, with a recommendation: `create` for a spike.** The plan does not say 4c creates spikes; the
  spike workflow is Stage 9. Recommend `create` now, because the contract tests need a writer and it is one more
  case in `createLocked`. Alternative: read only, with fixtures written by hand. Smaller, and the provider then has no
  write for a kind it declares.
- **Decision for the item: a wrongly named spike folder is invalid, not skipped.** A folder that is not a spike would
  otherwise hide a typo. Alternative: skip silently. No.
- **Decision for the item: `watch`.** The provider still declares `watch: false`. `watch` must ignore the rest of a
  spike folder when it is built (Stage 6a).
- Area: `packs/plan/src/scan.ts`, `item.ts`, `ids.ts`, `create.ts`, `write.ts`, `reid.ts`, `provider.ts`,
  `packs/plan/test/`.
- The test rule: no test touches the real home, real launchd or a real daemon. Each test that runs the `collide` binary
  clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as
  `host/tests/common/mod.rs` `TempRoot` does. Settings files are fixtures, never the owner's own `settings.json`. A temp
  repository sets `GIT_CONFIG_GLOBAL` and `GIT_CONFIG_SYSTEM` to empty files.
- Stage 4c item (revision 23 scope, split 2026-11-18). Items of Stage 4c run one at a time, in id order, because they
  share `core`, `host`, `packs/plan` and the generated files.
- Nothing here runs a real `claude`. No dependency is added.
- Owner decisions: none recorded yet. The recommendations stand until the owner answers at the split.
