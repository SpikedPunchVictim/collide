---
id: ts-0068-wx
kind: task
status: active
title: "Claude version, smoke check, published version and canary"
owners: []
links:
  - { type: blocked-by, to: ts-0067-kw }
---

## Goal

Claude's version and settings checks (0.3; R5; revision 16), in `host/src/adapters/claude/version.rs` and the daemon:

- **The version in use.** `parse_version` and `read_version` check it against `TESTED`. It is read when the daemon starts and before each phase, cached by `BinaryKey` (path and modification time).
- **The smoke check.** A version outside the range gets a one-turn smoke check: a prompt that must come back as a `result` with the pinned stream shape.
  - A failure refuses the phase through the Queue, naming the version and the range.
  - A pass marks the in-use chip and raises no Queue item.
- **The published version.** It is the `latest` dist-tag of `@anthropic-ai/claude-code` on the npm registry, with `stable` kept for the tooltip.
  - The daemon fetches it at start and once a day, through the system proxy, with a 5 s timeout and a request that carries no user data.
  - A failed fetch keeps the last value and its age, or "unknown".
  - `claude.check_published: false` turns the fetch off.
- **Both versions for clients.** The daemon gives both versions to its clients, so `collide run` (`ts-0069-bn`) can print them.
- **The version in the fingerprint** that `ts-0067-kw` stores.
- **The settings canary.** `canary_started` looks for the hook that `ts-0061-bg` put in the snapshot (`CANARY_MARKER`). If no `hook_started` for it arrives in the first turn, the phase fails with `CANARY_FAILURE`, and the Queue says so.
- **The offline half of the contract test.** The generated `settings.json` is validated against Claude's published settings schema.

Replaces the stubs tagged `claude: version check`, `claude: smoke check` and `claude: settings canary`.

## Success criteria

- The version, smoke-check, published-version, schema and canary case below (Stage 3b criterion; R5):
  - A `claude` outside the tested range gets the smoke check: a pass marks the in-use version and raises no Queue item, and a failure refuses the phase.
  - The published-version fetch runs against a recorded registry response, with offline, timeout and `claude.check_published: false` cases (no network in CI).
  - A generated settings file that fails Claude's schema fails the contract test.
  - A phase whose canary never appears fails with the message that names it.
- `read_version` runs `claude --version` once per `BinaryKey`, and a changed modification time reads it again.
- The `claude` version is in the fingerprint.
- No stub tagged `claude: version check`, `claude: smoke check` or `claude: settings canary` remains. The three tags move to `FILLED_TAGS`.

## Tests

Write these first:

- the version range and the canary;
- version parsing;
- the cache by path and modification time, with a fake Claude that prints a version;
- a smoke check that passes and one that fails, with a fake Claude outside the range;
- the published-version fetch against a recorded registry response, served locally: offline, a timeout, and `check_published: false`;
- the generated settings against the vendored schema, and a broken settings file that fails it.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 3b: Claude's version and settings.
  - 0.3: A tested version range; Two version chips (revision 16); Settings that fail validation are ignored silently under `-p`.
  - 0.13: the fingerprint.
  - R5.
- **The schema.** Claude's published settings schema is vendored as a fixture, with its source and the date it was fetched. Fetching it needs the network, so the build agent asks the owner first and fetches one URL, or the owner supplies the file. Tests never fetch it.
- **Decision for the item: the HTTP client.** The published-version fetch needs one. The choice is a new crate or the system's `curl`, and it is recorded with the alternative and how each honours the system proxy. A new dependency is named for the owner before it lands.
- No test touches the network. The registry is a recorded response served on loopback. A test that would reach a non-local host fails.
- The real binary's smoke check runs in `ts-0070-mk`, with the owner's OK.
- Area: `host` (`adapters/claude/version.rs`, `daemon.rs`), plus `core` for the Queue kind and the RPC that carries the versions.
- The test rule: no test touches the real home, real launchd or a real daemon. Each test clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as `host/tests/common/mod.rs` `TempRoot` does. Settings files are fixtures, never the owner's own `settings.json`.
- Stage 3b item (revision 18 scope, split 2026-11-15). Items of Stage 3b run one at a time, in id order, because they share `host` and the generated files.
