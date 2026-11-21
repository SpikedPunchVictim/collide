---
id: ts-0162-yh
kind: task
status: ready
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
