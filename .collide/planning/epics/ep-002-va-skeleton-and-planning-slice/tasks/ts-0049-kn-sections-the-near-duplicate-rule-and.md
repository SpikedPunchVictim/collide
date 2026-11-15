---
id: ts-0049-kn
kind: task
status: active
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
- Built: `core/src/section.rs` holds the pure section rules. `same_section` (trim, any case), `near_duplicate_key` (lowercase, spaces, hyphens and underscores out, one final `s` dropped) and `near_duplicate`; `section_uses(&[ProposalFeatures])` counts the features and the proposals of each section, gives each section the spelling most of its features use (a tie goes to the lowest proposal id, a further tie to the spelling that sorts first), sorts by feature count then name with the empty section last, and groups the sections that share a key. Its result, `Sections { sections, nearDuplicates }` (with `SectionUse` and `NearDuplicateGroup`, camelCase, `deny_unknown_fields`), is the documented `--json` shape and is in the schema registry, so `pnpm gen` wrote `schemas/` and `packages/pack-sdk/src/generated/` for the three (byte-stable on a second run). `host` gains `collide sections [--json]` (`parse_sections` and `run_sections` in `task_cli.rs`, the text in the new `section_view.rs`, the word in `bin/collide.rs`): it lists the items the way `tasks` does, keeps the kind `proposal` items whose category is not `dropped` (the provider's mapping of `rejected` and `superseded`), and hands their features to core. An invalid item is reported on stderr as `tasks` does. The help states both rules, their false matches and that nothing is merged. Tests: `core/tests/sections.rs` (matching, the key, one letter, `s` alone, `Bus` with `Bu`, spelling and tie rules, sort, groups, the JSON shape and its schema) and `host/tests/task_cli.rs` (text and `--json` over fixtures, dropped proposals, no proposals, a draft with a problem, usage, help).
- Decisions for the owner: (1) The rule on an edge the plan does not fix: a name with only separators in it (`-`, `_`) has no near-duplicate key and is near nothing, and an `s` that is all that is left is kept, so `s` alone is not an empty name; `s` and `ss` still group, like `a` and `as`, `Bus` and `Bu`. (2) `Sections.sections` lists the unsorted features as one entry with the empty `name`, last, as `Feature.section` does; the text print calls it Unsorted, and it is left out when there are none. (3) `--json` gives `featureCount`, `proposalCount` and the ids in `proposals`, and a group gives the shown names only. (4) The text is one line per section with its counts, then `near-duplicates (a suggestion; nothing is merged)` and one line per group. (5) A draft whose Features part has problems still counts the entries that parsed; its problems stay with `collide task`. (6) Proposals count in every status but `rejected` and `superseded`, `deferred` and `draft` included. (7) The `Sections` types are registered in `core/src/schema.rs`, as the plan wants the view to read the same shape, although `tasks --json`'s own wrapper document is not registered.
- Not done by design: merging and `renameSection` (Stages 4c and 9d); GitHub's `area:` label and Projects field (Stages 5 and 6c); the view's flag and merge button (Stages 6a and 9d); Plan…'s prompt (Stage 9d). Collide's own planning tree has no proposal yet, so `collide sections` there prints `no sections: no proposal that is not dropped has a feature`.
