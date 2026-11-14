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

The pipeline S5 chose, wired into the build, so every contract is written once in Rust.

## Success criteria

- Generated TypeScript matches the schema.
- A manifest with an unknown key fails validation.
- Serde round-trips pass for a sample of each kind of type (tagged enum, optional, map).

## Tests

- Schema generation test; round-trip tests.
