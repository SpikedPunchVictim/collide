---
id: ts-0023-ch
kind: task
status: ready
title: "Hardened YAML loader"
owners: []
links:
  - { type: blocked-by, to: ts-0008-mx }
---

## Goal

The one YAML entry point in `core`: `serde-saphyr` 2.x with the hardened option set of 0.7, never its defaults (`strict_booleans`, `no_schema`, merge keys as errors, no anchors or aliases, at most 100,000 nodes and depth 32, duplicate keys an error), with the file size capped before parsing. Typed `String` fields keep `no`, `0.10` and `1e10` as strings, and an ambiguous unquoted scalar in a string field is an error. An unquoted `{{ }}` template is an error reported at its column with the hint "quote the template". Every manifest and settings item loads through this function.

## Success criteria

- A test pins the options: `no` stays a string, a duplicate key (also inside a map) is an error, an anchor, an alias and a merge key are errors whose text says so, and an unquoted `{{ }}` is an error with the hint "quote the template".
- `0.10` and `1e10` stay strings in a string field.
- An alias bomb, a document of 100,000-deep nesting and a file over the size cap each fail fast with a message, and the process neither hangs nor overflows its stack.
- An unknown key is an error with the path to it.

## Tests

Write first: one test per hazard above (`no`, `0.10`, `1e10`, duplicate key, alias bomb, 100,000-deep nesting, the `{{ }}` hint, an oversized file), each observed failing against the library's defaults.

## Notes

- IMPLEMENTATION_PLAN.md, 0.7 (YAML is read with `serde-saphyr` 2.x and a hardened option set), 0.9 (YAML and frontmatter are untrusted input), 0.15 (Chosen crates), Stage 2: Skeleton, Contracts and Success Criteria. The S5 record: `.collide/planning/spikes/sp-0005-fv-schema-expressions/record.md`.
- Area: `core` (`serde-saphyr` is pure Rust, so `core` may use it).
- Done when the criteria hold.
