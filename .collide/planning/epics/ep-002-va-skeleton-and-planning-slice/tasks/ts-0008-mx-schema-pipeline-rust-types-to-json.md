---
id: ts-0008-mx
kind: task
status: ready
title: "Schema pipeline: Rust types to JSON Schema to TypeScript"
owners: []
links:
  - { type: blocked-by, to: ts-0007-ga }
---

## Goal

The pipeline S5 chose, wired into the build, so every contract is written once in Rust: `schemars` to JSON Schema to TypeScript with `json-schema-to-typescript`, a Node build step that is never shipped. The generated TypeScript goes into `packages/pack-sdk`. Not specta, not ts-rs. Every later contract item adds its types to this pipeline.

## Success criteria

- Generated TypeScript matches the schema.
- A manifest with an unknown key fails validation, at any depth and in a tagged variant. Every struct and variant has `additionalProperties: false`.
- A tagged variant with no fields is `Wait {}`, never a unit variant.
- Input and output types are generated separately, and integer fields use the `uint*` formats.
- The Rust side checks a document against the schema with serde's `deny_unknown_fields`, or with a JSON Schema validator this item picks. S5 measured only serde and `ajv`, so the choice is recorded here, in the item's Notes.
- Serde round-trips pass for a sample of each kind of type (tagged enum, optional, map).

## Tests

Write first: schema generation test; round-trip tests; an unknown-key test at depth 3 and inside a tagged variant; a `Wait {}` test.

## Notes

- IMPLEMENTATION_PLAN.md, 0.6 (Types), 0.15 (Chosen crates), Stage 2: Skeleton, Contracts. The S5 record: `.collide/planning/spikes/sp-0005-fv-schema-expressions/record.md`.
- Area: `core` (the sample types, `schemars`), the Node build step, `packages/pack-sdk` (generated output).
- The hardened YAML loader is `ts-0023-ch`, not this item.
- Done when the criteria hold and the generator runs inside `cargo build` or one documented build command.
