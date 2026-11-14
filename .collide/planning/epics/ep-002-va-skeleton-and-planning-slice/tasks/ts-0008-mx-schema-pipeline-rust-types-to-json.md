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

The pipeline S5 chose, wired into the build, so every contract is written once in Rust: `schemars` to JSON Schema to TypeScript with `json-schema-to-typescript`, a Node build step. Not specta, not ts-rs.

## Success criteria

- Generated TypeScript matches the schema.
- A manifest with an unknown key fails validation, at any depth and in a tagged variant. Every struct and variant has `additionalProperties: false`.
- A tagged variant with no fields is `Wait {}`, never a unit variant.
- Input and output types are generated separately, and integer fields use the `uint*` formats.
- The Rust side checks a document against the schema with serde's `deny_unknown_fields`, or with a JSON Schema validator this item picks. S5 measured only serde and `ajv`, so the choice is recorded here.
- Serde round-trips pass for a sample of each kind of type (tagged enum, optional, map).

## Tests

- Schema generation test; round-trip tests; a test that pins the YAML loader's hardened `serde-saphyr` options (`no` stays a string, duplicate keys, anchors and merge keys are errors).
