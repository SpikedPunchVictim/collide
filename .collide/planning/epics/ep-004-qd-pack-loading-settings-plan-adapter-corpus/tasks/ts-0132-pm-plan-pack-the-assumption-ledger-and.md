---
id: ts-0132-pm
kind: task
status: done
title: "Plan pack: the Assumption ledger and the Findings part"
owners: []
links:
  - { type: blocked-by, to: ts-0131-jh }
---

## Goal

The provider reads a proposal's Assumptions and Findings parts, with their problems, so the planning view can show what
a plan leans on and what a review said about it.

What exists. From `ts-0131-jh`: `Task.assumptions`, `Task.findings` and `Task.part_problems` on the wire (empty), the
shared entry parser `packs/plan/src/parts.ts`, and the Decision log, whose ids a `Target:` can name.

What this adds.
- The Assumption ledger: a `## Assumptions` part with entries `A<n>`. Labels: `State:` (`unverified`, `verified`,
  `contradicted` or `accepted risk`; `unverified` when the label is missing), `Evidence:` (as in the Decision log) and
  `Spike:` (the spike that would settle it, which must be an item of kind `spike`, shown with its status).
- Findings: a `## Findings` part with `### Red-team` and `### Pre-mortem` headings and entries `R<n> <severity> <title>`
  (`high`, `medium` or `low`). Labels: `Target:` (an `F<n>`, `D<n>` or `A<n>` of this proposal), and then `Accepted:`
  (the change it made to the plan) or `Dismissed:` (the reason). A finding with neither is open, and the typed entry
  says so.
- Problems in `part_problems`, with part and line: an assumption with no `State:`, a `State:` that is not one of the
  four, a `Spike:` that is no spike, an `Evidence:` that names no item, a severity that is not one of three, a `Target:`
  that names nothing in this proposal, a finding with both `Accepted:` and `Dismissed:`, an entry outside the two
  headings, a repeated number.
- Nothing refuses a move: only the Features part gates `proposed`.
- `accepted risk` is read as written. That the agent writes it only on the person's word is a rule for the planning
  workflow and the phase-end check (Stage 8), not a thing a reader can see.

## Success criteria

- A fixture proposal with a decision that names no alternative, an evidence id that names no item, an assumption with no
  `State:` and an open high finding lists them in `decisions`, `assumptions` and `findings`, with each bad id and line
  in `part_problems`, and still moves to `proposed` (the stage's criterion, with `ts-0131-jh`).
- An assumption with no `State:` reads as `unverified`, and the missing label is a problem.
- A finding with a `Target:` of `D2` in a proposal that has no `D2` is a problem; one with `Accepted:` is not open;
  one with neither is open, whatever its severity.
- Each of the four states and each severity reads; a heading that is neither `Red-team` nor `Pre-mortem` is a problem
  naming the line.
- A proposal with no Assumptions or Findings part has none of the keys.

## Tests

Write these first.

- Parser fixtures for each part: every label and value, a missing `State:`, a continuation line that starts with a
  label, an open and a closed finding of each severity, a repeated number, a part with only prose, an entry under the
  wrong heading, CRLF, the S5 hazards in the file.
- Provider tests over the stage's mixed fixture, with `setStatus proposed` at the end.
- The cross-part references: `Target:` to a decision, an assumption and a feature, and to a missing one.
- A round trip of the typed entries, and the existing tests unedited.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 4c: Quality parts (the Assumptions and Findings parts); Success Criteria (quality parts); Tests (parser
    fixtures for each part).
  - 0.10: Planning sources, "A proposal's quality parts" (Assumption ledger; Findings). 0.8: Task. Revision 18 (planning
    quality checks from the Field mockup A).
- **Decision for the item, with a recommendation: what counts as a problem.** A malformed or dangling thing is a
  problem. An open high finding is not: it is a state the planning view shows. Alternative: also flag an open `high`
  finding. That would make a plan that is simply unfinished look broken. The stage's text says the open finding is
  listed, which is what this does.
- **Decision for the item: an entry outside the two headings** in Findings is a problem, and is kept as written.
- Area: `packs/plan/src/assumptions.ts` (new), `findings.ts` (new), `parts.ts`, `scan.ts`, `provider.ts`,
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

- `core/src/item.rs`: `Assumption` and `Finding` were defined by `ts-0131-jh` and not filled, so their shape changes.
  `Assumption`: `state` is now required (it always reads as one of the four, `unverified` by default), and
  `spikeStatus` is new (the status of the spike, when `spike` names one); the type is now camelCase on the wire.
  `Finding`: `open` is new and required, and `section` is optional (left out for an entry under neither heading). Doc
  comments are one short line so the generated schema lines stay under 120 columns. `pnpm gen` was run, so `schemas/`
  and `packages/pack-sdk/src/generated/` are current.
- `packs/plan/src/parts.ts`: `PartSpec.sections` and `PartEntry.section`. A part that names its `### ` headings gets
  each entry's heading (matched in any case, spelled as the part spells it), and a heading that is none of them is a
  problem naming the line. A part without `sections` reads exactly as before.
- `packs/plan/src/assumptions.ts` (new): `readAssumptions(body, firstLine, lookup)`. Reads `State:`, `Evidence:` and
  `Spike:`. A missing `State:` is `unverified` and no problem; a `State:` that is not one of the four is a problem and
  reads `unverified`. A `Spike:` must be an item of kind `spike`, and then carries `spikeStatus`.
- `packs/plan/src/findings.ts` (new): `readFindings(body, firstLine, own)`. Reads the severity (first word of the
  title), `Target:`, `Accepted:` and `Dismissed:`, and sets `open`. `own` is the ids of the proposal's features,
  decisions and assumptions.
- `packs/plan/src/decisions.ts`: `ItemLookup` may say the item's `kind`; new `readEvidenceList` splits an `Evidence:`
  value and calls `readEvidence` on each piece. `readDecisions` uses it, and so does the ledger. Nothing else changed.
- `packs/plan/src/scan.ts`: `qualityPartsOf` reads the three parts of a proposal; `toTask` carries `decisions`,
  `assumptions`, `findings` and one `partProblems` list in line order. The lookup now carries `kind`.
- Tests, all new: `test/assumptions.test.ts` (27), `test/findings.test.ts` (30), `test/part-sections.test.ts` (6),
  `test/quality-parts.test.ts` (19, provider level, the mixed fixture and `setStatus proposed`); in Rust,
  `core/tests/provider_protocol.rs` (two new tests, and the quality-part fixture gains `spikeStatus` and `open`) and
  `core/tests/planning_contracts.rs` (the key sets). No existing test was edited except those two Rust fixtures.
- Red at runtime: with `assumptions.ts` and `findings.ts` stubbed to return nothing and `readPart` unchanged, 62 of
  the 81 TS tests then written failed on empty results; the 19 that passed are the ones that expect nothing. With
  `item.rs` restored to the old shape, `the_quality_part_keys_are_in_the_schema` failed at runtime; the two new
  `provider_protocol.rs` tests and the new assertions there did not compile (red by compile error only). Mutations,
  each caught: a problem for a missing `State:` (7 tests), an open high finding that is not open (5), an open high
  finding that is a problem (6), no `Target:` check (7), a spike status for a non-spike (2), both-labels check removed
  (1), assumptions not sent (3), assumption ids left out of the targets (3), heading section not reset at `## ` (2),
  heading matched case-sensitively (2). Unsorted `partProblems` was first not caught, so `partProblems are in line
  order, whichever order the parts are in` was added and then failed under the mutation.
- Hazards of the file: the body cap and a duplicate frontmatter key leave the proposal invalid and unread; an entry of
  5000 lines and one line of 200 000 characters read in full; a title of 100 000 asterisks does not hang; CRLF reads as
  LF; fenced blocks and prose-only parts read as nothing.

### Decisions for the owner

1. **A missing `State:` is not a problem.** The item's problem list says it is; 0.10 and the item's Goal say it reads
   as `unverified`. Followed the plan. A `State:` that is not one of the four (including an empty one, and `Verified`)
   is a problem and reads `unverified`, so a typo is never a pass. Alternative: flag both. Recommend as built. The
   Stage 4c criterion in the plan says "with the bad id in `partProblems`" for the whole fixture; that now holds for
   the other bad entries, and the assumption with no `State:` is in `assumptions` only. Propose text for 0.10: "a
   missing `State:` reads as `unverified` and is not a problem".
2. **`Assumption.state` is required on the wire and `Finding.open` is a field**, not derived by the reader. The app and
   CLI need not repeat the rule. Alternative: leave `state` optional and derive `open` from the two labels. Recommend
   as built.
3. **The spike status** is `spikeStatus`, a sibling of `spike`, set only when `spike` names an item of kind `spike`.
   Alternative: `spike` as an `Evidence`-like object. Recommend as built (smaller, and additive).
4. **`Spike:` and `Evidence:` that say nothing** are no value and no problem; an empty `State:`, `Accepted:`,
   `Dismissed:` or `Target:` is a problem (a closed or typed set, where silence is a mistake). An empty `Accepted:` or
   `Dismissed:` does not close the finding. Alternative: ignore all empty labels. Recommend as built.
5. **The severity is the first word of the entry's title** and is kept as written, even when it is not one of the
   three, so the entry is listed and the problem names it. A severity with no title is a problem and the title is
   empty. Alternative: refuse such an entry, as `D0` is. Recommend as built: a mistyped finding is not lost.
6. **An entry outside the two headings** is a problem, is listed, has no `section` and is kept as written (the item's
   decision). An entry under an unknown `###` heading is both: the heading is a problem and so is the entry.
   Alternative: report only the heading. Recommend as built, so the entry's own line is flagged.
7. **`Target:`** must be `F<n>`, `D<n>` or `A<n>`. A well-formed id the proposal has not (`D2`) is a problem and is kept
   as written, so the app can show it as missing. A target of another shape is a problem and is not kept. Only
   entries that were listed count: a refused `D1` (a repeat) does not make a target. Alternative: keep neither.
   Recommend as built.
8. **A finding with both `Accepted:` and `Dismissed:`** is a problem on the line of the second label. It keeps both as
   written and is not open. Alternative: open. Recommend as built: someone answered it.
9. **A repeated `State:`, `Spike:`, `Target:`, `Accepted:` or `Dismissed:`** is a problem and the first stays, as
   `Confidence:` is; a repeated `Evidence:` adds.
10. **`partProblems` of the three parts are merged and sorted by line**, not grouped by part. A reader can still
    tell them apart by `part`. Alternative: group by part. Recommend as built.
11. **`ItemLookup.kind` is optional**, so `decisions.test.ts` stays unedited; a lookup that leaves it out names no
    spike. The scan always gives it.
12. **Plan text to propose**, in 0.10, "A proposal's quality parts": the missing `State:` reading above; that an
    assumption's `Spike:` must be an item of kind `spike` and is shown with its status; that a finding's severity is
    the first word of its title and is lowercase; and that an entry outside the two headings is a problem and is kept.
    Also: the Stage 4c success text (quality parts) should say the assumption with no `State:` is listed as
    `unverified`, not that it has a problem.

- Owner decisions (2026-11-18, at the build): the item's two decisions as it recommends, on the owner's delegation, and
  the builder's as recommended above. Its problem list's "an assumption with no `State:`" gave way to 0.10, which reads
  a missing `State:` as `unverified`; Stage 4c's success text was reworded to match.

### Not done, by design

- `coverage`: `ts-0133-kg`. The Pre-registration part of a spike: `ts-0134-ch`.
- Any display of the ledger or the findings in the app, the CLI view or the plan pack's ops (for example an op that
  writes `Accepted:`): later items of Stage 4c and Stage 9. Nothing here writes a part.
- That the agent writes `accepted risk` only on the person's word: the planning workflow and the phase-end check
  (Stage 8), not the reader.
- A `Target:` in a Findings part is not re-checked when a `reid` or a `renameSection` changes the proposal; no op in
  this pack renumbers a decision, an assumption or a feature, so none can break one.
