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

### Built

- `host/src/adapters/claude/version.rs`: `parse_version`, `read_version` (`claude --version`, 15 s limit), `VersionCache` (by `BinaryKey`, successes only: a changed modification time reads again), `check_range` against `TESTED`, and the smoke check (`FIXED_ARGV` plus `--model haiku --max-budget-usd 0.05 --setting-sources "" --no-session-persistence`, a one-word prompt, 45 s limit). The reply must come back as a `result` with the pinned stream shape.
- `host/src/bounded_run.rs` (new): `run_bounded(command, stdin, timeout)`, one deadline for the child and its pipes, so a grandchild that holds a pipe cannot hang a caller.
- `host/src/http_get.rs` (new): the `HttpGet` trait and `CurlGet`, which runs `/usr/bin/curl -q --silent --show-error --fail --globoff --proto =https` with an explicit argv, `--max-time` and `--connect-timeout` of 5 s, `--max-filesize` 1 MiB, no redirects, an empty environment and no user data (user agent `collide`). `SystemProxy` picks the proxy: `HTTPS_PROXY`, then `ALL_PROXY` from the base env, then `scutil --proxy` (HTTPS, then SOCKS); `NO_PROXY` is passed on. `=http` is allowed only for a loopback URL (tests).
- `host/src/claude_versions.rs` (new): `ClaudeVersions`. `gate` reads the version (cached), runs the smoke check outside the range (a pass is cached per binary, a failure is tried again at each phase) and returns `Ready`, `Refused` or `Unreadable`. A refresh thread fetches the `latest` and `stable` dist-tags at start and then every 24 h; a failed fetch keeps the last value and its age, or "unknown". `claude.check_published: false` (in `~/.config/collide/config.yaml`, read at each refresh) turns it off. `status` is what the RPC returns.
- RPC `daemon.agent_versions` (`core/src/owner/daemon.rs`, `owner.rs`, `schema.rs`, `app/src/owner/methods.ts`, `app/src/components/words.ts`, generated schemas and pack SDK types): in use (with the range check and the smoke result), published `latest` and `stable`, the age of the fetch and its error.
- `QueueKind::AgentSetup` (answer: Dismiss). It is raised when the gate refuses a phase (the message names the version and the range) and when the canary fails.
- `host/src/file_run.rs`: the gate runs before each phase; a refusal fails the run through the Queue. The version goes in the fingerprint as the `agent_version` part (agent, version, in tested range, smoke passed).
- The canary: the snapshot's SessionStart hook is `echo collide-settings-canary >&2`. `session.rs` sees it on the `hook_response` line (hook event `SessionStart`, outcome `success`, the marker in stderr, stdout or output). A session that expects it and ends its first turn without it gives `PhaseFailure::SettingsIgnored` (`core/src/adapter.rs`); `agent_feed.rs` turns that into `CANARY_FAILURE` ("Claude ignored Collide's settings") in `phase_records.failure`, and the Queue gets an `AgentSetup` item with the same text, pushed before the run is recorded as failed.
- The schema: `host/tests/adapters/claude/fixtures/claude-code-settings.schema.json` (231,539 bytes, from `https://json.schemastore.org/claude-code-settings.json`, 2026-11-15, SHA-256 in the `.source.md` beside it). Offline check in two steps: the Rust test `settings_golden` fails when the adapter's settings differ from `generated-settings.golden.json`; `packages/claude-contract/test/settings-schema.test.ts` validates that golden file against the schema with ajv, and a broken file must fail.
- The validator: `ajv` 8.20.0, a devDependency of the new `packages/claude-contract` (with vitest, typescript and `@types/node`, versions already in the workspace). ajv has 4 dependencies (`fast-deep-equal`, `fast-uri`, `json-schema-traverse`, `require-from-string`), so the lockfile gained 5 packages. It is test-only and ships nowhere. `pnpm-lock.yaml` was updated offline.
- Tests added, about 120 in all: `host/src/http_get.rs` 8, `host/src/claude_versions.rs` 3, `host/src/adapters/claude/version.rs` 13 (unit); `host/tests/adapters/claude/` `version.rs` 24, `published.rs` 16 (loopback registry, loopback proxy, offline, timeout, `check_published: false`), `agent_setup.rs` 11, `canary.rs` 9, `settings_schema.rs` 2, `fake_claude.rs` 3 new. The fake Claude gained `--emulate-hooks` (fires the SessionStart hook) and the daemon tests' wrappers answer `--version`. `checks/tests/real_tree.rs`: `FILLED_TAGS` is 37 (the three tags are under ts-0068-wx); `HOST_ADAPTER_EXECUTOR_TAGS` is 14.
- Network safety: the daemon fetches only when `COLLIDE_DAEMON_ROOT` is not set (`published_from_env`). Every test sets it, so the default under test is no fetch; the tests that fetch point the source at a loopback server.

### Decisions for the owner

1. The HTTP client. Chosen: `/usr/bin/curl` with an explicit argv, no new crate. It reads no `~/.curlrc` (`-q`), follows no redirect, sends nothing but a GET with a fixed user agent, and honours the system proxy because the daemon passes it in: a proxy variable from the base env, else `scutil --proxy`. Alternative: a Rust HTTP crate (`ureq` with `rustls`, or `reqwest`): no child process and testable in-process, but it adds a TLS stack (a rough guess: tens of crates), and it reads proxy variables only, so the system proxy would need `scutil` anyway, or a second crate. Recommend curl: the fetch is one GET a day.
2. The schema validator. Chosen: ajv in a TS devDependency plus a Rust golden test. Alternative: the `jsonschema` crate as a Rust dev-dependency: one test language, but many more crates (a rough guess), and the schema is draft-07 with large `anyOf`s that ajv is the reference for. Recommend as built; the golden test keeps Rust the one place the settings are generated.
3. The canary is seen on `hook_response` with the marker in stderr, not as a missing `hook_started`. `hook_started` carries no command, so a user's own SessionStart hook cannot be told from ours. Alternative: a distinctive hook `matcher` or a file the hook writes. Recommend as built. Proposed plan text below.
4. A `claude --version` that cannot be read or run refuses the phase (`Unreadable`), with an `AgentSetup` item. Alternative: run on with an unknown version. Recommend refusing: the fingerprint and the tested range are meaningless without it.
5. A smoke pass is cached per `BinaryKey`; a failure is tried again at each phase, so a fixed install recovers without a restart. Alternative: cache failures for a time. Recommend as built (a smoke turn costs under $0.05 and runs only outside the range).
6. `claude.check_published` is read by a minimal reader of `~/.config/collide/config.yaml` at each refresh. A malformed file leaves the fetch on and logs it. Alternative: wait for the config item to give one reader. Recommend replacing it when that exists.
7. The registry fixture (`registry-dist-tags.json`) is written by hand in the registry's shape, not recorded, because recording needs the network. Its `.source.md` says so. Alternative: record it once with the owner's OK. Recommend recording it with the real-binary run in ts-0070-mk.
8. `--no-session-persistence` in the smoke argv is not yet verified against the real binary. ts-0070-mk checks it with the owner's OK; if the flag is refused, drop it.
9. The smoke check runs inside `start_file` with a 45 s limit, so a phase start can wait that long once per binary. Alternative: run it at daemon start. Recommend as built until the real run shows the time.
10. The system proxy's exceptions list from `scutil --proxy` is not read; `NO_PROXY` from the base env is. Alternative: parse `ExceptionsList`. Recommend as built: the only host is `registry.npmjs.org`.
11. A resumed session is assumed to fire SessionStart, so the canary is expected on a resume too. If the real binary does not, ts-0070-mk will show it as a CANARY_FAILURE on resume; the fix is to expect it only on a fresh session.

Proposed plan text:

- 0.3, Settings that fail validation are ignored silently under `-p`: "Collide's SessionStart hook writes `collide-settings-canary` to stderr. The adapter looks for it on the hook's `hook_response` line in the first turn. A turn that ends without it fails the phase with 'Claude ignored Collide's settings', and the Queue says so." (replaces the 'no `hook_started` event' wording.)
- 0.3, Two version chips: "`claude.check_published` (default true) lives in `~/.config/collide/config.yaml`."

- Owner decisions (2026-11-15): all eleven recommendations taken, curl kept for the daemon's fetch (the owner offered Python or JS; `/usr/bin/python3` can be a Command Line Tools placeholder that prompts, and node is not guaranteed). The fetch runs only when `COLLIDE_DAEMON_ROOT` is unset, as `launchctl_from_env` does for launchd. The vendored schema's `.source.md` names its Apache-2.0 license. `ajv` 8.20.0 is a test-only devDependency of the new private `packages/claude-contract`. Follow-ups to ts-0070-mk: the real `hook_response` carries the canary; SessionStart fires on `--resume` (else expect the canary only on a fresh session); `--no-session-persistence` is accepted; the real smoke check and its time; record `registry-dist-tags.json` from the real fetch.

### Not done, by design

- No test touches the network, and the real `claude` was never run. The real binary's smoke check, the real registry fetch and `--no-session-persistence` belong to `ts-0070-mk`, with the owner's OK.
- The app does not show the two version chips yet; the RPC carries what they need. The chip is for a later app item.
- `collide run` printing both versions is `ts-0069-bn`; it reads `daemon.agent_versions`.
- The system proxy's exceptions list is not read (decision 10).
