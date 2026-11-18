---
id: ts-0132-pm
kind: task
status: ready
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
