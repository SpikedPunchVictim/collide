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

### Built

The provider reads, lists, creates, changes and renames a spike. All in `packs/plan/src/`:

- `item.ts`: `SPIKE`, `SPIKE_STATUSES` (the five of 0.10), `INFORMS` and its entry in `LINK_TYPES` (out, not blocking),
  `SPIKE` in `KNOWN_KINDS`, `hasOwnStatuses`, and `statusesOf` for a spike. The status refusal of `readItem` lists the
  five "for a spike".
- `layout.ts`: `spikeDirOf` (`paths.spike`, or `<root>/spikes`) and `spikesDirsOf` (where new spikes go, then each
  root's `spikes/`). The `Layout.spike` comment is true now.
- `scan.ts`: `readEpicFiles` also reads spikes. It lists each spikes folder one level down. A folder named for a spike
  id has its `record.md` read, after one `lstat`, and nothing else: it never lists inside the folder. Another name is a
  `task.invalid` with the rule. The private root's `spikes/` is read as the shared ones are. A spike has no epic, so it
  has no `part-of`.
- `ids.ts`: `KIND_IDS.spike` (`sp`, four digits). `idsInPaths` takes the spikes folders as a third argument, and
  `takenIds` reads them in every ref (`ls-tree` one level down), in the working tree and in the private root.
- `create.ts`: `create` of kind `spike` writes `<spikes folder>/<id>-<slug>/record.md`, status `planned`, one heading
  (the title). It takes no epic. The reply's `files` is the record.
- `write.ts`: `setStatus` knows the spike's words and refuses a category for a spike, as for a proposal. `link` takes
  `informs`. A spike under `paths.spike` is locked and written even when the planning root is not there.
- `reid.ts`: a spike's folder is renamed whole, and only its record is reported in `files`. Nothing inside is listed.
- `provider.ts`: `initialize` declares the kind, with its own `setStatus` (one helper now builds both), the link type,
  and `spike` in `create`'s kinds and `informs` in `link`'s types.

Tests: `packs/plan/test/spikes.test.ts` (58 tests) and the fixture `fixtures/collide-spikes/` (the frontmatter of the
ten records and one `results`). Existing tests that list kinds, link types, ids or the `basic` fixture's items were
updated: `provider.test.ts`, `provider-stdio.test.ts`, `create.test.ts`, `feature-links.test.ts`, `write.test.ts`,
`host/tests/builtin_packs.rs` and `host/tests/task_cli.rs` (its unknown kind is `widget` now, as `spike` is declared).
The `basic` fixture's spike record was a decoy with a bad frontmatter. It is a valid record now, and `work/` beside it
is the decoy. The real-tree test in `provider.test.ts` lists `epics/` only, and one level of `spikes/`, then checks
that every spike folder lists as one item with no `task.invalid`.

Red at runtime: before any change to `src/`, 56 of the 58 tests failed on an assertion or on the reply they read (no
spike kind, no `sp-` ids, a refused `create`). Two passed: the no-`record.md` and symbolic-link case (nothing lists, as
before) and the task-status guard. Then, with the code in, each of these was broken in turn and the named tests went
red, and each was put back: `reid` listing inside the folder (1), `takenIds` ignoring spikes in refs (1), `write.ts`
not locking the spikes folders (1), `spikesDirsOf` without the old place and the previous roots (3).

Checks, each in a run after my last edit: `cargo build`, `cargo test --workspace` (4427 passed, 0 failed), `cargo
clippy -D warnings`, `cargo fmt --check`, `pnpm test`, `pnpm typecheck`, `pnpm --filter @collide/app build`,
`pnpm build:packs` and `pnpm check` all pass.

### Decisions for the owner

1. `create` for a spike, a wrongly named folder as `task.invalid`, and `watch` unchanged: taken as the item
   recommends.
2. A spike folder with no `record.md`, or with a `record.md` that is a symbolic link, is no item and no
   `task.invalid`, as an epic folder with no `epic.md` is none. Alternative: invalid. Recommend as built: a branch
   switch leaves folders of ignored files behind, and they would show as errors for good. A record that cannot be
   opened (a folder with mode `000`) is a `task.invalid`, and the scan goes on.
3. `paths.spike` is the folder that holds the spike folders, in place of `<root>/spikes`. The `spikes/` folder of the
   current root and of each `previous` root stays read, so a spike is not lost when the setting is added. New spikes go
   to `paths.spike`. Alternative: read only `paths.spike`. Recommend as built. Plan text to propose for 0.10:
   "`paths.spike` is the folder that holds spike folders; the `spikes/` folder of every root is still read."
4. A private spike is `<pack data>/planning/spikes/<id>-<slug>/record.md`. Private epic folders are named by id alone
   because their tasks look the epic up by id. A spike has no epic, so it keeps its slug.
5. `informs` has no rule on the kind it starts from. Its target must be an item that exists, not itself and not a
   feature. Alternative: spikes only. Recommend as built: 0.10 and Stage 9 also link a question to a spike and a spike
   to a task.
6. `setStatus` on a spike takes `status` only and refuses `category`, as a proposal does. Each category has one spike
   status, so accepting it would work. Recommend as built: one rule for the kinds with their own words.
7. A new spike's body is `# <title>`, the first line of 4.3. Alternative: `## Pre-registration`, which Stage 9 owns.
8. The folder rule is `sp-<three or more digits>-<two letters>[-<slug>]`, as `ID_PATTERN` is. `create` writes four.
9. The refusal of a `setStatus` with a word no kind has now lists the words kind by kind, so a spike's five show.
   The message changed. The CLI checks the argument against the declared schema first, so people see that one.
10. Not checked, as for epics: that a record's `kind` is `spike`, or that its id equals the folder's.
11. `readEpicFiles` and `scanEpics` keep their names although they read spikes. Alternative: rename. Recommend a rename
    in a later item that touches both.

- Owner decisions (2026-11-18, at the build): the item's decisions as it recommends, on the owner's delegation; the rest
  as built. Decision 3's plan text applied to 0.10, Planning sources.

### Not done, by design

- `watch` ignoring the rest of a spike folder: Stage 6a, when `watch` is built (the provider still declares
  `watch: false`).
- `collide tasks --ready` over the real tree: it starts the real daemon, which a build must not do. The provider's list
  over the real tree (`provider.test.ts`, "Collide's own items") shows the ten records load with no `task.invalid`.
- The spike workflow (pre-registration, `running` and `results` set by a workflow), `supersedes`, and a spike's
  `instances`: Stage 9.
- The Move offer for spikes after a layout change: with the Move item of Stage 12.
- A `reid` of a spike carries its ignored files with the folder, because the folder is renamed. Nothing is copied.
