---
id: ts-0131-jh
kind: task
status: done
title: "Plan pack: the Decision log and the quality parts' wire shape"
owners: []
links:
  - { type: blocked-by, to: ts-0130-mv }
---

## Goal

The provider reads a proposal's Decision log, and the wire has a place for every quality part, so the next two items add
parts and not fields.

What exists. `Task` in `core/src/provider.rs` has `features` and `feature_problems` and nothing for the other parts of
revision 18. `packs/plan/src/features.ts` parses the Features part: the entry form `- **F<n> <title>**: <summary>`,
continuation lines indented by two spaces, and problems with their lines. `toTask` in `scan.ts` fills both fields for a
proposal. A Decision section written before revision 18 is plain text and nothing reads it.

What this adds.
- Wire (additive, left out when empty; `pnpm gen` regenerates `schemas/` and `packages/pack-sdk/src/generated/`):
  `Task.part_problems` (`{ part, line, reason }`), `Task.decisions`, `Task.assumptions` and `Task.findings` as typed
  lists. `coverage` comes in `ts-0133-kg`. The app's test doubles take the new keys.
- `packs/plan/src/parts.ts` (new): the entry form `- **<letter><n> <title>**: <text>` with indented continuation lines
  and labelled continuation lines (`Passed on:`), shared by every part, so Features keeps its own parser and the new
  parts do not copy it.
- The Decision log: entries `D<n>` in the `## Decision` part. Labels `Passed on:` (the alternatives not taken, each with
  its reason), `Evidence:` (item ids, a feature as `pr-0003-km#F2`, or repo paths), `Revisit if:` and, optional,
  `Confidence:` (`low`, `medium` or `high`, as the person wrote it, never derived). Numbers count from 1 and are not
  reused within the file.
- Problems in `part_problems`, with the part's name and the line: a decision with no alternative under `Passed on:`, an
  evidence id that names no item (the scan's ids), a feature fragment the proposal named does not have, a repeated
  `D<n>`, a `Confidence:` that is none of the three, an unknown label.
- A cited spike carries its status in the decision's evidence entry, from the scan.
- Text in the Decision section that is not an entry is kept as written and read as nothing: a section written before
  revision 18 gives no `decisions` and no problem.
- A problem here never refuses a move: `setStatus proposed` still works.

## Success criteria

- A fixture proposal whose Decision section has one entry with no `Passed on:` alternative and one `Evidence:` id that
  names no item lists both in `decisions`, with each bad entry's id and line in `part_problems`.
- A cited spike shows its status in the decision's evidence; a cited feature shows its proposal.
- A Decision section of plain text reads unchanged: no `decisions`, no `part_problems`, and the body is as written.
- The proposal still moves to `proposed` with these problems present, and a Features problem still refuses it.
- The wire keys are absent for an item with none, so an older reader sees the old shape; the generated schemas and types
  are current (`pnpm gen` leaves no diff).
- `collide task <id> --json` (the daemon's) carries the keys through `task.get`.

## Tests

Write these first.

- Parser fixtures for the Decision part: each label, a continuation line starting with a label, a repeated number, a
  bad `Confidence:`, plain text, an entry inside another heading, a very long entry, CRLF, the S5 hazards in the file.
- Provider tests: `toTask` over the fixtures; evidence resolution against the scan (item, missing item, feature,
  repo path).
- A serde round trip and a generated-schema check for the new `Task` keys.
- The existing provider and feature tests unedited.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 4c: Quality parts ("the provider reads the Decision log, the Assumptions and Findings parts and a spike's
    Pre-registration part (0.10), reports their problems in `partProblems`, and derives a proposal's `coverage`. The
    Decision log comes first, then the Assumption ledger"); Success Criteria (quality parts); Tests (parser fixtures for
    each part).
  - 0.8: Data model, Task (`decisions`, `assumptions`, `findings`, `partProblems`, `coverage`). 0.10: Planning sources,
    "A proposal's quality parts". Revision 18.
- **Decision for the item, with a recommendation: the heading is `## Decision`.** The plan says "the Decision section".
  Recommend that exact heading, matched in any case after trimming, as `## Features` is. Alternative: a configurable
  name. No: one more setting for no case yet.
- **Decision for the item: repo paths in `Evidence:`.** Checked for shape only (relative, no `..`), not for existence,
  because a path may exist on another branch. Alternative: check the path in the checkout. It would flag evidence that
  is simply on a branch.
- **Decision for the item: the names on the wire** are `part_problems` (snake case, as `feature_problems`), and an entry
  is typed, with the label values as strings.
- Not here: the Assumption ledger and the Findings part (`ts-0132-pm`), coverage (`ts-0133-kg`), the Pre-registration
  part (`ts-0134-ch`).
- Area: `core/src/provider.rs`, the generated schemas and TypeScript, `packs/plan/src/parts.ts` (new), `decisions.ts`
  (new), `scan.ts`, `provider.ts`, `packs/plan/test/`, `app/test/support/`, `core/tests/`.
- The test rule: no test touches the real home, real launchd or a real daemon. Each test that runs the `collide` binary
  clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as
  `host/tests/common/mod.rs` `TempRoot` does. Settings files are fixtures, never the owner's own `settings.json`. A temp
  repository sets `GIT_CONFIG_GLOBAL` and `GIT_CONFIG_SYSTEM` to empty files.
- Stage 4c item (revision 23 scope, split 2026-11-18). Items of Stage 4c run one at a time, in id order, because they
  share `core`, `host`, `packs/plan` and the generated files.
- Nothing here runs a real `claude`. No dependency is added.
- Owner decisions: none recorded yet. The recommendations stand until the owner answers at the split.

### Built

- `core/src/item.rs`: new wire types `PartProblem` (`part`, `line`, `reason`), `Evidence` (`target`, `kind`, optional
  `status` and `proposal`), `EvidenceKind` (`item`, `feature`, `path`), `Decision`, `Assumption` and `Finding`. All
  refuse unknown keys. They are registered in `core/src/schema.rs`.
- `core/src/provider.rs` `Task`: `decisions`, `assumptions`, `findings` and `part_problems` (wire `partProblems`). Each
  is `#[serde(default, skip_serializing_if = "Vec::is_empty")]`, so an older reader sees the old shape and an older
  cached row reads back with them empty. `core/src/testing/provider.rs` and `host/src/scratch.rs` take the new fields.
- `pnpm gen` was run: `schemas/` and `packages/pack-sdk/src/generated/` are current, and ten new files were added.
- `packs/plan/src/parts.ts` (new): `readPart(body, firstLine, spec)`, the entry form `- **<letter><n> <title>**: <text>`
  with indented continuation lines and labelled lines, for any part. It reports `<letter>0`, a repeated number, an
  entry-shaped line that is not an entry, and an unknown label. `features.ts` is unchanged except that `openingFence`,
  `closesFence` and `Fence` are exported, and `parseFeatures` and the feature ops behave as before.
- `packs/plan/src/decisions.ts` (new): `readDecisions(body, firstLine, lookup)` and `readEvidence`. It reads
  `Passed on:`, `Evidence:`, `Revisit if:` and `Confidence:`, and reports the problems of the item.
- `packs/plan/src/scan.ts` `toTask(found, withBody, items)` now takes the scan, so an evidence id is resolved against
  it. Every caller passes it: `provider.ts` (`list`, `get`), `write.ts`, `rename-section.ts`, `reid.ts` and `create.ts`.
- Tests: `packs/plan/test/parts.test.ts`, `decisions.test.ts` and `decision-log.test.ts` (provider level, with the
  real planning tree copied to a temp root); `core/tests/provider_protocol.rs` and `planning_contracts.rs` (serde round
  trip, keys absent when empty, older task, schema keys); `host/tests/corpus_store.rs` (older cached row, new row);
  `host/tests/task_daemon_cli.rs` (`task <id> --json` carries `decisions` and `partProblems` through `task.get`).
- Red at runtime: with `parts.ts` and `decisions.ts` stubbed to return nothing, 9 of 10 `parts.test.ts`, 22 of 25
  `decisions.test.ts` and 4 of 15 `decision-log.test.ts` failed on the empty results. The rest are guards that read
  "nothing" and pass on the old code. Mutations: reading a log from any kind failed the question and task test; a
  stray-text problem failed 9 tests (plain text, repeated numbers, entry-shaped lines). Removing `default` and
  `skip_serializing_if` from `Task.decisions` failed 12 tests in `provider_protocol.rs` (including the older-task and
  keys-absent tests) and 11 in `corpus_store.rs` (including both new ones). I did not run the daemon test against old
  code: it needs the new pack to pass at all.
- Older cached tasks: `a_task_cached_before_the_quality_parts_reads_back_with_them_empty_and_is_not_dropped` writes a
  row in the old shape (features and `featureProblems`, none of the new keys) over a real store row and reads the cache.
- The real tree: it has no proposal item, so no pre-revision-18 proposal can be read from it. `decision-log.test.ts`
  copies `.collide/planning` (the epics and each spike's `record.md`) to a temp root and checks that no item has any of
  the four keys, and that the spike `sp-0003-ye`, which has a `## Decision` section, has no `decisions`. A Decision
  section of plain text in a proposal is covered by fixtures in both `decisions.test.ts` and `decision-log.test.ts`.
  `checks/tests/real_tree.rs` runs no provider and does not read planning items, so the check is in `packs/plan`.
- App test doubles: `app/test/support/loop-world.ts` builds a `TaskOutput` and the new keys are optional there, so it
  is unchanged; `pnpm typecheck` and `pnpm test` pass over it.

### Decisions for the owner

1. **A question's `## Decision` is not a Decision log.** Only a proposal's section is read; a question's text from
   `resolve` stays its own text. Test: `a question or a task with entries in its Decision section has no decisions`.
   Alternative: read it too. Recommend no, as the item does.
2. **Wire names.** The Rust field is `part_problems`; the wire is `partProblems`, as `featureProblems` is (camel case
   for the whole `Task`). Entries are typed, with label values as strings, as the item says.
3. **`Assumption` and `Finding` are defined here and not filled**, so `ts-0132-pm` adds parts and not fields. Shapes:
   `Assumption { id, title, text, state?, evidence[], spike?, line }`; `Finding { id, section, severity, title, text,
   target?, accepted?, dismissed?, line }`. `ts-0132-pm` may change them; nothing reads them yet.
4. **Evidence is typed**: `{ target, kind, status?, proposal? }`. `status` is set for any cited item found in the scan
   (not for spikes only), which is less code. A feature gets `proposal` (its item id). Alternative: status for spikes
   only. Recommend as built: the app can show any cited item's status.
5. **How an evidence piece is read.** Pieces are split on commas. A piece whose part before any `#` is an item id is an
   item (no `#`) or a feature (with `#`, read by `parseFeatureTarget`); anything else is a path. A path is a problem
   only if it starts with `/` or has a `..` segment. So `ts-12`, a mistyped id, is read as a path and not flagged.
   Alternative: also flag a path with no `/` and no `.`. Recommend leaving it; a bare file name is a fair path.
6. **`Confidence:`** is exact lowercase `low`, `medium` or `high`. `High` is a problem and is not kept. A repeated
   `Confidence:` or `Revisit if:` is a problem and the first stays; an empty `Revisit if:` is nothing, an empty
   `Confidence:` is a problem. `Passed on:` is one alternative per line, and the lines under a label continue it.
7. **A refused entry is not listed**: `D0`, a repeated `D<n>` (numbers are shared by all `## Decision` parts of the
   file), and an entry with no title. A decision with no alternative, or with bad evidence, is listed and has a problem.
8. **A line that starts `- **D<digit>` and is not the entry form is a problem**, so a mistyped entry is not lost
   silently. Any other text in the section is plain text, read as nothing and no problem. Alternative: no problem
   for anything. Recommend as built.
9. **A label is a capitalised word run of at most three words and a colon** at the start of an indented line. So prose
   such as `  See also: x` under an entry is an unknown-label problem; `  note: x` (lowercase) is text. Alternative:
   only known labels, and no unknown-label problem. The item asks for the problem, so I kept it.
10. **Fenced blocks** are skipped wherever they are, in or out of the part. An unclosed fence hides the rest of the
    file. Features reports a fence inside its part as a problem; the log does not.
11. **`toTask` takes the scan as a required argument.** `create` passes only the new item: its body is the template's,
    which has no entries, so nothing is looked up (a comment says so). Alternative: re-scan after a create. Recommend
    as built.
12. **Plan text to propose:** in 0.10, "A proposal's quality parts", add that a question's `## Decision` is not a
    Decision log; that an evidence id with no `#` is an item, one with `#` a feature, and anything else a path whose
    shape is checked (relative, no `..`); and that `Confidence:` is lowercase.

- Owner decisions (2026-11-18, at the build): the item's three decisions as it recommends, on the owner's delegation,
  and the builder's as recommended above; the wire name `partProblems` follows `featureProblems`. The plan text was
  applied to 0.10, "A proposal's quality parts".

### Not done, by design

- The Assumptions and Findings parts, their readers and their problems: `ts-0132-pm`.
- `coverage`: `ts-0133-kg`. The Pre-registration part of a spike: `ts-0134-ch`.
- Any text of the Decision log in the app, the CLI view (`host/src/task_view.rs` still prints features only) or the
  `plan` pack's ops: later items of Stage 4c and Stage 9.
- A repo path in `Evidence:` is not checked for existence, by decision.
