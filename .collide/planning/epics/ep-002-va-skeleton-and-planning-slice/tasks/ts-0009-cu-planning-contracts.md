---
id: ts-0009-cu
kind: task
status: ready
title: "Planning contracts"
owners: []
links:
  - { type: blocked-by, to: ts-0008-mx }
---

## Goal

Defined first (part L): the provider schema's `kinds`, op effects, `links`, `assignee` and checklist field type; fan-out `instances:`; the run budget; the five task points with `change` and `latestRun`; `points:`, `views:` and `checklists:` in `pack.yaml`; S11's `change.diff`, `batch`, `effects`, `via`, `actor.kind: agent`, `apply` `items[]`, `preview`, the `watch` shapes and `task.invalid`.

## Success criteria

- A provider with one kind and no links validates unchanged.
- `collide schema` lists every new key and point.
- A derived point naming an unknown core point, or a gate bound to an `after_*` point, is refused at load.

## Tests

- Derived-point validation; the new keys' schemas; the `change` and `watch` shapes.

## Notes

IMPLEMENTATION_PLAN.md, 0.4, 0.5, 0.10 and Stage 2's Planning contracts.
