---
id: ts-0025-st
kind: task
status: active
title: "Board, config and settings contracts"
owners: []
links:
  - { type: blocked-by, to: ts-0023-ch }
---

## Goal

`.collide/board.yaml` (the packs in order, each from a `repo:`, `git:` or `dev:` source, with `dev:` refused here; the default workflow; `profiles.default` and `profiles.rules`; source configuration; the status mapping, including the Projects v2 keys that Stage 6c builds; the checkout filter); `.collide/config.yaml` and the user's `~/.config/collide/config.yaml` (the settings layers, `overridable`, `secret: true` kept out of both files); the `.collide` pointer file (`dir: <path>`); and Collide's own settings keys, including `agents` (one entry per id, with `kind`, `command`, `args`, `model`, `config`, `read`, `tools`, `timeout_s`, `budget_usd`, `env`, `secrets`) and `ask` (`agent`, and the repo's defaults `budget_usd` and `timeout_s`).

## Success criteria

- A valid `board.yaml`, `config.yaml` and pointer file load through the hardened loader and validate. An unknown key fails, naming it.
- The `agents` and `ask` keys are in the generated schema. An `agents` entry with an unknown key, or with a tool kind other than `read` or `search`, fails validation. `timeout_s` outside 5 to 1800 fails.
- The built-in `claude-haiku` entry of 0.7 validates as written there.
- A `kind: external` entry needs no `command`. A `kind: acp` entry needs one.
- The repo-file rule is data the loader can read: which keys a repo `config.yaml` may set (only `ask.budget_usd` and `ask.timeout_s` under `agents`/`ask`) is a function over a parsed file, with a test. Applying the layers is Stage 4a.

## Tests

Write first: validation tests with valid and invalid fixtures for each file; the `agents` unknown key and bad tool kind; the `timeout_s` range; the repo-file key check.

## Notes

- IMPLEMENTATION_PLAN.md, 0.7 (the Collide directory, `board.yaml`, Settings, Agent entries), 0.9, 0.12, Stage 2: Skeleton, Contracts (`board.yaml`, `config.yaml`, the `agents` and `ask` keys).
- Area: `core` (types and schemas).
- Views and the lock files are `ts-0026-es`.
- Done when the criteria hold and `collide schema` lists the new keys.
