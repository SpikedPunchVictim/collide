---
id: ts-0026-es
kind: task
status: ready
title: "Views, profiles and the lock files"
owners: []
links:
  - { type: blocked-by, to: ts-0023-ch }
---

## Goal

Views, profiles and the lock files, as types with schemas: a view (`name`, `where` as CEL text, `include`, `exclude`, `closure`, `workflow`, `profile`, and `layout` with `default`, `groupBy` and the new `sort`); a profile (`description`, `packs`, `workflow`, `launch`, `models`, `roles`, `config`), found in `.collide/profiles/`, in the user's profiles folder, or in a pack's `profiles:`; `.collide/packs.lock` with its `source` field, the user's `packs.yaml`, and the user's `packs.lock`. This item parses the CEL source as text only, and enforces the 1 KB source cap.

## Success criteria

- A view with `layout: { default: board, groupBy: category, sort: { by: id, dir: asc } }` validates, and `layout.sort` is in the generated schema. A sort with an unknown `dir` fails.
- A `where` over 1 KB fails validation.
- A profile validates, and a profile key a user profile may not set is representable as data for the load-time refusal (Stage 4a applies it).
- A lock entry carries `source`, commit and tree id, and the `source` field accepts a value for a package manager later without a schema change.
- An unknown key fails in every file.

## Tests

Write first: validation tests for a view, a profile and both lock files, with the invalid fixtures above.

## Notes

- IMPLEMENTATION_PLAN.md, 0.7 (`board.yaml`, `packs.lock`, Repo packs and user packs, Profiles), 0.10 (Views), 0.11 (a sort is saved as `layout.sort`), 0.9 (the 1 KB source cap), Stage 2: Skeleton, Contracts (views, profiles, `packs.lock`, the user's `packs.yaml`, `layout.sort`).
- Area: `core` (types and schemas).
- The CEL runtime and its budgets are `ts-0044-xq` and Stage 4b.
- Done when the criteria hold.
