---
id: ts-0049-kn
kind: task
status: ready
title: "Sections: the near-duplicate rule and collide sections"
owners: []
links:
  - { type: blocked-by, to: ts-0047-fb }
---

## Goal

Sections are named by their headings and matched across proposals (revision 14). `core` gains the section rules, so the CLI now and the Planning view later agree. Two names are one section when they match after trimming, in any case. Two names are near-duplicates when they match once spaces, hyphens and underscores are taken out and one final `s` is dropped. Near-duplicate names are gathered into groups. The CLI gains `collide sections [--json]`, which lists every section in use in proposals that are not dropped. Each section shows its feature count and its proposal count. The empty section is listed last as Unsorted, with its count, and the near-duplicate groups follow.

## Success criteria

- Over proposal fixtures, `Packs` used in two proposals is one section that names both. `packs` and `Packs` are one section, shown in the spelling most of its features use, and on a tie the spelling from the lowest proposal id.
- `Pack` and `packs` are listed as one near-duplicate group, and so are `Task view` and `task-view`. `Daemon` and `Tasks` are not grouped.
- Features of `rejected` and `superseded` proposals are not counted.
- Sections are sorted by feature count, highest first, then by name. Unsorted comes last.
- `--json` gives the same content in one documented shape, and the help text states the matching and near-duplicate rules.

## Tests

Write first: `core` tests for one-section matching and the near-duplicate rule (the pairs above, a name of one letter, `s` alone, `Bus` with `Bu`, which the rule groups and which is accepted as a known false match), and for the grouping; a CLI test over fixtures for the text and `--json` output.

## Notes

- IMPLEMENTATION_PLAN.md, 0.10 ("A proposal's features": sections matched by name, near-duplicates, `collide sections`), 0.11 (Planning view, Features; Plan…'s naming rule, which calls `collide sections`), Stage 2: Skeleton (Planning contracts, "From revision 14"; the criteria's "features (revision 14)"; Tests, "Features"), Stage 6a (the view's sections match `collide sections --json`).
- Area: `core` (the section rules), `host` (`collide sections`).
- A near-duplicate group is a suggestion for a person, shown so they can merge (Stage 9d). It never merges anything itself.
- Not in this item: merging and `renameSection` (Stages 4c and 9d); GitHub's `area:` label and Projects field (Stages 5 and 6c).
- Done when the criteria hold.
