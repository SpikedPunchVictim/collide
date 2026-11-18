---
id: ts-0131-jh
kind: task
status: ready
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
