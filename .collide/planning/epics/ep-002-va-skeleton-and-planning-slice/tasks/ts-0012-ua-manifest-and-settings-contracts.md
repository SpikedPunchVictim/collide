---
id: ts-0012-ua
kind: task
status: ready
title: "Manifest and settings contracts"
owners: []
links:
  - { type: blocked-by, to: ts-0008-mx }
---

## Goal

`pack.yaml` in full (with `config`, workflows as transition tables, and the Milestone C and D keys), `board.yaml`, `config.yaml`, views, profiles, `packs.lock` with `source`, the user's `packs.yaml`, and the `.collide` pointer file.

## Success criteria

- The built-in pack's, the reference pack's and both fixture packs' manifests validate.
- A manifest with an unknown key fails.
- YAML loads through `serde-saphyr` 2.x with the hardened options of 0.7. `no` stays a string; a duplicate key, an anchor and a merge key are errors; an unquoted `{{ }}` template is an error whose hint is "quote the template".
- A required string field has `minLength: 1`.

## Tests

- Manifest validation over all four packs plus invalid fixtures, including S5's YAML hazards: `no`, `0.10`, a duplicate key, an alias bomb, deep nesting.

## Notes

IMPLEMENTATION_PLAN.md, 0.7 and 0.9. The S5 record: `.collide/planning/spikes/sp-0005-fv-schema-expressions/record.md`.
