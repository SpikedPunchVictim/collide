---
id: ts-0008-mx
kind: task
status: done
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
- Rust-side document check (decided in this item): serde's `deny_unknown_fields`, with no JSON Schema validator crate. S5 found one gap, the unit variant of an internally tagged enum, and `Wait {}` closes it, so a validator would add a dependency and check nothing serde does not. The tests check each unknown-key case (top level, depth 3, tagged variant, `Wait {}`) against serde and check the schemas for `additionalProperties: false`.
- Running it: `pnpm gen` runs `cargo run --example gen_schemas` in `core` (the real registry to `schemas/`, the sample types in `core/tests/fixtures/` to `core/tests/fixtures/schemas/`), then `packages/pack-sdk/scripts/gen.ts` (to `packages/pack-sdk/src/generated/` and `packages/pack-sdk/test/fixtures/generated/`). To add a contract, derive `Serialize`, `Deserialize` and `JsonSchema` with `deny_unknown_fields` and add one `registry.add::<T>()` line to `registry()` in `core/src/schema.rs`, then run `pnpm gen` and commit the output. The real registry starts empty: contracts are `ts-0012-ua`.
