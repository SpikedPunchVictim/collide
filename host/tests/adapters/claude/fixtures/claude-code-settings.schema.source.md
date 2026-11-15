# claude-code-settings.schema.json

Claude Code's published `settings.json` schema (JSON Schema draft-07), vendored as a test fixture (ts-0068-wx).

- Source: `https://json.schemastore.org/claude-code-settings.json`
- Fetched: 2026-11-15, once, by the build agent with Python's standard library (HTTP 200, `application/json`), after the owner approved that one URL.
- Size: 231,539 bytes. SHA-256: `f26f028ad94a1e2a28e9dad1f80832ab196a8e080bd3a7d7a32e6428a15e0611`.
- Kept byte for byte as it came; nothing in it is run.
- License: SchemaStore publishes its schemas under the Apache License 2.0 (`https://github.com/SchemaStore/schemastore`). This copy is unmodified.

Tests never fetch it. To update it, ask the owner to approve the fetch again, replace the file, and change the date and hash here.

The generated `settings.json` is checked against it in two steps: the Rust test `settings_golden` in `host/tests/adapters/claude/settings_schema.rs` fails when the adapter's settings differ from `generated-settings.golden.json`, and the vitest test in `packages/claude-contract` validates that golden file (and a broken one, which must fail) against this schema with `ajv`.
