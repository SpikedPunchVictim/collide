---
id: ts-0130-mv
kind: task
status: ready
title: "Plan pack: feature ops addFeature, moveFeature and renameSection"
owners: []
links:
  - { type: blocked-by, to: ts-0129-uv }
---

## Goal

The provider writes a proposal's Features part: add a feature, move one to a section, and rename a section across every
proposal that uses it.

What exists. `packs/plan/src/features.ts` parses the part (`parseFeatures`, `SECTION_MAX`) and reports problems with
their lines; `feature-target.ts` and `links.ts` handle `pr-0003-km#F2`; `frontmatter-edit.ts` and `atomic.ts` edit a
file and write it by temp and rename under the root's lock (`lock.ts`); `core/src/section.rs` holds the near-duplicate
rule and `collide sections` lists names. `proposedRefusal` in `write.ts` refuses `proposed` while the part has problems.
No op writes a feature: `OPS` in `write.ts` has `setStatus`, `claim`, `unclaim`, `link`, `create` and `reid`.

What this adds.
- `addFeature { section, title, summary }` on a proposal. `section` is required and may be empty. It takes the next
  `F<n>` (one past the highest in the part) and writes the entry under that section's heading, matched after trimming
  and in any case, or under `### Unsorted`, which is made at the end of the part when it is missing. An empty section is
  `Unsorted`.
- `moveFeature { feature, section }`: moves the entry, with its continuation lines and its number, under the section's
  heading, and makes the heading when it is missing.
- `renameSection { from, to }`, which has no `id`: it edits every proposal whose Features part has a section named
  `from`, renames the heading, and merges the entries into the heading that already has the name `to` when there is
  one. It returns each file in `items[]` and `files`.
- Each op reads the part again after its edit and refuses to write when the result has a problem (the text the problem
  names is in the message). A part that has problems before the op is refused with them, so a write never leaves a
  part worse.
- Titles and summaries are checked (one line for the title, no `**`, a summary of one line or continuation lines), and
  the op declarations carry argument schemas.

## Success criteria

- `addFeature` with an empty section writes under `### Unsorted` with the next number, and creates the heading when it
  is missing.
- `addFeature` into an existing section appends under it; `F` numbers are never reused within the file.
- `moveFeature` keeps the number and the continuation lines and leaves the part without problems.
- `renameSection` of `Pack` to `packs` over three proposals changes the three files, returns them in `items[]`, merges
  into an existing `packs` heading, and leaves each part without problems. A fourth proposal without the section is not
  touched.
- A proposal with a problem in its Features part is refused with the problem and its line, and not written.
- A stale version on any file makes the op return `conflict` with that file's current task, and write none.

## Tests

Write these first.

- Provider tests over proposal fixtures: every case above, a section with a `#` or over 40 characters, an `Unsorted`
  in another case, a part with no heading at all, CRLF line ends, a feature with continuation lines.
- `renameSection` over three, then a mid-way failure injected between files, with each file whole afterwards.
- The existing `features.test.ts` and `feature-links.test.ts` unedited.
- `collide sections --json` before and after a rename (the direct CLI, until `ts-0136-rr`): the near-duplicate group is
  gone.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 4c: Feature ops ("`addFeature {section, title, summary}` ... `moveFeature {feature, section}`; and
    `renameSection {from, to}` ... Each leaves the part without problems"); Success Criteria (`addFeature` with an empty
    section; `renameSection` of `Pack` to `packs` over three proposals).
  - Revision 14 (2026-11-14): features in proposals. 0.10: "A proposal's features" (the part, sections by name,
    near-duplicates). `docs/tasks-and-docs.md` lists the ops with the features.
- **Decision for the item, with a recommendation: a number is "one past the highest in the file".** 0.10 says a removed
  feature's number is not reused. A file cannot show a number that was removed from it. Recommend the highest present
  plus one, and write that limit in the op's description. Alternative: also read the proposal in every ref, as ids do,
  to find the highest ever. Heavier, and still blind to a number removed before any ref held it.
- **Decision for the item: what `renameSection` touches.** Every proposal under the root, whatever its status, so a
  rename never leaves a name half changed. Alternative: only proposals that are not dropped, as `collide sections`
  lists. Then a rejected proposal keeps the old name.
- **Decision for the item: all files or none.** The op reads every file and checks every version first, then writes each
  by temp and rename. A crash between renames leaves each file whole but the rename partly done; the result lists what
  changed. A rename is idempotent, so running it again finishes it.
- Out of scope: the planning view's Sort and merge buttons (Stage 9d), the read-only Features view (Stage 6a).
- Area: `packs/plan/src/features.ts`, `feature-ops.ts` (new), `write.ts`, `frontmatter-edit.ts`, `provider.ts`,
  `packs/plan/test/`.
- The test rule: no test touches the real home, real launchd or a real daemon. Each test that runs the `collide` binary
  clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as
  `host/tests/common/mod.rs` `TempRoot` does. Settings files are fixtures, never the owner's own `settings.json`. A temp
  repository sets `GIT_CONFIG_GLOBAL` and `GIT_CONFIG_SYSTEM` to empty files.
- Stage 4c item (revision 23 scope, split 2026-11-18). Items of Stage 4c run one at a time, in id order, because they
  share `core`, `host`, `packs/plan` and the generated files.
- Nothing here runs a real `claude`. No dependency is added.
- Owner decisions: none recorded yet. The recommendations stand until the owner answers at the split.
