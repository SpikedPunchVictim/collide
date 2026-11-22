---
id: ts-0170-gh
kind: task
status: ready
title: "Daemon: a global source writes outside a checkout and restarts with its block"
owners: []
links:
  - { type: blocked-by, to: ts-0169-mn }
---

## Goal

A source whose tasks are global (GitHub) writes without waiting for a checkout, records its change as remote, and
restarts when the repos or settings it was started with change.

What exists. `host/src/sources.rs` `Source::writes_checkout()` is true for every pack, so a write waits for a busy
checkout and for the line lock (`host/src/write_guard.rs`, `ts-0114-hr`), and `Source::write_cause()` is `Worktree`
for every pack. `Source::declared()` gives the `InitializeResult`, and its `scope` is `global` or `ref`. A `Slot`'s
identity is its pack, provider and folder only (`Identity` in `sources.rs`): `Sources::refresh` keeps a running
provider when the identity matches, and `Source::start` reads `config` once, so a change to the board's repos or to a
pack setting does not reach a running provider. `ts-0168-tq` gives a provider `board.repos` and `board.origin` in
`initialize`. The corpus already holds a global source's tasks with no checkout (`ApplyParams.checkout` is optional).

What this adds.
- For a source that declared `scope: global`: `writes_checkout()` is false and `write_cause()` is `Remote`. A write
  goes straight to the provider with no `checkout`. It does not wait for a busy checkout or the line lock, and a queued
  write is not made for it. A `ref` source (`plan`) is unchanged.
- The slot's identity also holds a digest of what `start` sends: the board block and the pack's settings as the layers
  give them. `plan` computes it. A change makes `refresh` stop the old provider and the next use start a new one with
  the new `initialize`; the source is relisted, so the corpus follows (tasks of a repo that left the block are removed
  by the full list). An unchanged digest keeps the process.

## Success criteria

- A write to a global source is applied while the checkout the board sits on is busy with a run, and while the line
  lock is held. A write to the `plan` source in the same state still waits.
- The change event of a global write has cause `remote`, and the provider got no `checkout`.
- Editing `sources.github.repos` in the line's `board.yaml` restarts the provider once, and the next list shows the new
  repo's tasks and none of the removed repo's.
- Changing a pack setting through `collide config set` restarts the provider once. A recheck with nothing changed does
  not restart it.

## Tests

Write these first.

- A fake global provider in `host/tests/fixtures/` (scope `global`, counts its `initialize` calls, returns its params
  and a task or two for each repo it was given), run through the daemon as `host/tests/daemon_sources.rs` does.
- The busy-checkout case of `ts-0114-hr` repeated for a global source (applied) and for `plan` (waits).
- Event cause: the recorded event of a global write is `remote`.
- Restart: edit the board file, assert one more `initialize` with the new repos; edit a setting, assert the same; a
  recheck, assert none.

## Notes

- Area: `host/src/sources.rs` (`Source`, `Identity`, `refresh`, `plan`, `start`), `host/src/daemon.rs` (`write`,
  `judged_write`, `apply_and_record`), `host/src/write_guard.rs`, `host/tests/`.
- The test rule: no test touches the real home, real launchd, a real daemon, the keychain or the network. Each test that
  runs the `collide` binary clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp
  root, as `host/tests/common/mod.rs` `TempRoot` does. Settings files are fixtures. A temp repository sets
  `GIT_CONFIG_GLOBAL` and `GIT_CONFIG_SYSTEM` to empty files.
- Stage 5 items run one at a time, in id order. Nothing here runs a real `claude`. No dependency is added.
- The cause `remote` for Collide's own write is a recommendation: the change lives on the remote side, and the next poll
  sees the same version and emits nothing. The alternative is a new cause, which widens the contract for no reader.
- Owner decisions: none recorded yet. The recommendations stand until the owner answers at the split.
