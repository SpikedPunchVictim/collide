---
id: ts-0162-yh
kind: task
status: done
title: "Orchard fixtures: the common section headings"
owners: []
links: []
---

## Goal

In `ts-0138-pn` step 8b (agent-run, 2026-11-20) the frontmatter keys of the fixture ADRs, spikes and questions in
`fixtures/orchard-arch/kinds/arch/` matched the real files, but the fixtures have no `##` headings at all. The real
files share a few: an ADR has a context, a decision, rejected alternatives and consequences; a spike and a question
most often have a question section. The provider reads sections only from epic files today, so nothing behaves
differently. The fixtures should still look like the real files, so a later reader of sections is tested on them.

## Success criteria

- Each fixture ADR has the four common headings, and most fixture spikes and questions have the question heading, with
  made-up text. Generic heading words only: no text of a real file.
- Step 8b's heading comparison in `ts-0138-pn` shows only headings that are rare in the real tree.

## Tests

- The existing `fixtures/arch-plan-tests/test/kinds.test.ts` still passes; no new test, since nothing reads the
  sections.

## Notes

- Low priority. No code change.
- Owner decisions: none recorded yet. The recommendations stand until the owner answers.

### Built

Fixture files only. No code, no test, no golden and no snapshot changed. Each file keeps its frontmatter and its
existing text; the new sections are appended after it.

- The step 8b heading part (`files_of` and `heads_of`) ran on the owner's Orchard copy and on the fixtures. A count of
  the files each `##` heading appears in came from the same `files_of` lists. The lists stay in a scratch folder.
- Threshold: a heading that appears in at least half of the real files of its kind.
  - ADR, 31 real files, so 16 or more: `Context` 31, `Decision` 31, `Rejected alternatives` 30, `Consequences` 30,
    `Sources` 21, `Verification` 17, `Invariants` 16.
  - Spike (`PLAN.md`), 108 real files, so 54 or more: `Question` 77.
  - Question, 175 real files, so 88 or more: `Question` 159.
- Added, under `fixtures/orchard-arch/kinds/arch/`:
  - The six ADRs (`001-...` to `006-...`): the seven ADR headings above, in that order, each with one or two made-up
    sentences about the fixture's own subject (quays, berth prices, the ledger, sunken quays, bells, lamp colours).
  - The four spike `PLAN.md` files: `## Question`, in the fixture's own words.
  - The five question files: `## Question`, in the fixture's own words.
  - Not touched: the two spike `RESULTS.md` files and the task fixture. Step 8b does not list them.
- No common heading was left out: every one above is plain generic English, none carries a project word.
- Step 8b heading part, run again after the edit (counts only):
  - ADR: 7 headings shared, 113 real-only, each in at most 14 of 31 files, none in the fixtures only.
  - Spike: 1 shared, 233 real-only, each in at most 9 of 108 files, none in the fixtures only.
  - Question: 1 shared, 84 real-only, each in at most 19 of 175 files, none in the fixtures only.
  - So every difference left is a heading below the threshold. The key comparison of step 8b was not changed.
- Readers of the fixtures, found by a search for `orchard-arch` and the `-arch/kinds/arch` path:
  - `fixtures/arch-plan-tests/test/*.test.ts`, all 16 files (`kinds`, `list`, `get-from-read`, `queue`, `watch`,
    `paging`, `hazards`, `checkouts`, `initialize`, `provider-stdio`, `fake-plan`, `contract`, `shape` and others).
    They copy the tree to a temp root. Run after the last edit: 519 tests pass, including `kinds.test.ts`. The
    `get` test still finds `Made-up body for ADR-002`, because that text is unchanged.
  - `host/tests/daemon_reference_kinds.rs` copies `kinds/arch` into a temp checkout. Run after the last edit: 15
    tests pass.
  - `host/tests/daemon_sources.rs` reads only `basic/arch`, which is unchanged.
- No test was written, as the item says, so there is no red to report.

### Decisions for the owner

- **Three ADR headings beyond the four the item names.** `Sources`, `Verification` and `Invariants` are in at least
  half of the real ADRs (21, 17 and 16 of 31) and are plain English, so the fixtures have them. Alternative: only the
  four the item names. Recommendation: as built, since the rule is "half of the real files" and a later reader of
  sections is tested on the real shape.
- **The threshold is half of the real files of the kind.** A heading in 14 of 31 ADRs (the highest below it) is
  left out. Alternative: a lower cut such as a third. Recommendation: half; the rest are too rare to matter.
- **Spike and question files get `## Question` only.** No other heading is in half of their real files. Alternative:
  add the numbered spike headings. Recommendation: no, each is in fewer than a tenth of the real spikes.
- **`RESULTS.md` and the task fixture get no headings.** Alternative: add some. Recommendation: leave them until a
  later item reads those files.
- **The text is new.** Each section is one or two invented sentences about the fixture's subject. No sentence,
  phrase or title of a real file was copied.

### Not done, by design

- A provider that reads the sections of these kinds, and a test of it on these headings: no item yet. Nothing reads
  sections of an ADR, spike or question today.
- The rare real-only headings stay out of the fixtures: the item asks only for the common ones.
