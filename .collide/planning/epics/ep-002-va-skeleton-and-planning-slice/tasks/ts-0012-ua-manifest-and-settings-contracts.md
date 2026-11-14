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

## Tests

- Manifest validation over all four packs plus invalid fixtures.

## Notes

IMPLEMENTATION_PLAN.md, 0.7.
