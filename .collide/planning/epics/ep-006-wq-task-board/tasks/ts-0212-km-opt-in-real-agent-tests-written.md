---
id: ts-0212-km
kind: task
status: ready
title: "Opt-in real-agent tests, written and gated"
owners: []
links:
  - { type: blocked-by, to: ts-0211-xg }
---

## Goal

The four opt-in checks of Stage 6b are written and gated by `COLLIDE_REAL_AGENT=1`, in the style of Stage 3's
real-claude contract test. The builder writes them and proves they skip without the opt-in. It never runs them;
`ts-0213-as` does, by the owner.

What exists.
- Stage 3's live harness (`host/tests/adapters/claude/real.rs`, `ts-0070-mk`): `COLLIDE_REAL_CLAUDE=1`; the child's
  environment cleared, with `PATH`, `LANG` and the owner's `HOME` and `USER` for the login; config none; a budget
  on every line; the spend ledger (`LEDGER_ENV`); recordings through a redactor.
- S6's fixture board in `fixtures/` (`ts-0202-ab`); the whole ask path (`ts-0199-xz` to `ts-0211-xg`).

What this adds.
- **The real-agent contract test.** The Field request of 0.11, three times, through the real ask path with Claude
  Code haiku, against S6's fixture board. A run passes when the groups hold the intended sets (quantities 001 and
  002; expiry 004, 005 and 006; reports 008, 009 and 010; data file 011 and 012) with no set split or mixed, 003
  and 007 in either area or left out, and no "no files were read" flag. The test passes on two of three. A file
  with a known marker is planted outside every readable folder, and the marker must not reach any answer. The
  test also checks that the line it ran equals the pinned line. It prints each run's pass, time, cost and input
  tokens.
- **The Claude snippet check.** The Agents page's Claude snippet connects a real `claude -p` that calls `query`
  once.
- **`opencode mcp list`** shows the endpoint connected with its snippet; skipped, saying so, when opencode is not
  installed. opencode runs with a temp `HOME` and `XDG_*` folders.
- **The strict line's token comparison.** The two example asks under `config: none` and under `config: user`; it
  prints the input tokens of each.
- Each check skips with its reason without `COLLIDE_REAL_AGENT=1`, and each goes through the spend ledger.

## Success criteria

- Without `COLLIDE_REAL_AGENT=1`, each of the four checks is skipped and says why; the default suites stay
  green and start no model.
- Each check reuses the live harness's rules: the cleared environment, config none where the check says so, a
  budget on each line, and the ledger. None bypasses them.
- The contract test's pass rule, the planted marker and the pinned-line comparison are code a reviewer can read.
- `cargo test --workspace` and `pnpm test` pass, with the four skipped.

## Tests

Write these first.
- The four checks, gated.
- A default-suite test that each check is gated: run without the variable, each reports skipped and starts no
  process.
- The contract test's pass rule as a unit test over recorded group sets: pass, a split set, a mixed set, 003 in
  either area, the flag.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 6b: Success Criteria (Real-agent contract test, and the three more opt-in checks); The token cost of the
    strict line is measured here; Tests. 0.3: the contract test plants a file outside the folders. Part L.
- **Decision for the item, with a recommendation: where the window runs.** Recommend a Playwright spec over the
  bridge, so the real page answers the calls, gated by the variable. The bridge's daemon runs on a temp root, and
  only the ask child gets the owner's `HOME` and `USER`, as `real.rs` does. Alternative: a Rust test with a window
  stand-in that runs the TS tools under node; a second window to keep in step.
- Never run here: no builder sets `COLLIDE_REAL_AGENT=1`, stubs the opt-in, or loosens the ledger to see a check
  go red. A check's own logic is tested on recorded data instead.
- Area: `app/browser/`, `host/tests/adapters/claude/`, `fixtures/`.
- Dependencies: none new.
- The test rule: no test touches the real home, real launchd, a real daemon, the keychain or the network. Each test
  that runs the `collide` binary clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to
  a temp root, as `host/tests/common/mod.rs` `TempRoot` does. A temp repository sets `GIT_CONFIG_GLOBAL` and
  `GIT_CONFIG_SYSTEM` to empty files. Keychain writes go through `MemorySecretStore` (`host/src/secret_store.rs`),
  never the login keychain. Browser tests run the installed Chrome with a temp profile (`app/browser/support/
  chrome.ts`), and the bridge's daemon on a temp root (`app/browser/support/world.ts`), with fake agents only. A test
  never stubs or bypasses a safety gate: the real-agent opt-in, the confinement probe, `env_clear`.
- Stage 6b item (revision 26 scope, split 2026-11-24). Items of Stage 6b run one at a time, in id order, because they
  share `app/`, `core`, `host` and the generated files. No builder runs `tauri build`.
- Nothing here runs a real `claude`, `opencode` or any model, or spends money.
- Owner decisions (2026-11-24): the owner took the recommendations at the split.
