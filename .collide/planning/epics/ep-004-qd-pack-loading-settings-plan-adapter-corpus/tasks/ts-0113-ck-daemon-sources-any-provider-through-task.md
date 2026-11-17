---
id: ts-0113-ck
kind: task
status: ready
title: "Daemon sources: any provider through task.get, task.apply and task.preview"
owners: []
links:
  - { type: blocked-by, to: ts-0112-px }
---

## Goal

Put every provider behind the daemon: the daemon starts each source of an open line from the line's snapshot, once the
line is trusted, passes it the pack's settings, and serves `task.get`, `task.apply` and `task.preview` for any of them,
the scratch source included, through one write path.

What exists. `host/src/daemon.rs` serves `task.get` and `task.apply` for the scratch source only (`task_get`,
`task_apply`); any other source answers "not built yet". `task.preview` is not served, though its types are
(`core/src/owner/task.rs`: `TaskGetParams`, `TaskApplyParams`, `TaskPreviewParams`). `host/src/scratch.rs`
`ScratchSource` is a `TaskProvider` over the store. After `ts-0108-dq`, `SupervisedProvider` starts and calls a provider
from a loaded pack. `host/src/board_load.rs` `LoadedLine` holds the line's packs and their manifests;
`host/src/trust.rs` holds the trust state of a line (4a); the settings layers resolve a pack's `config` (`ts-0098-nx`).
Nothing connects them: no daemon code starts a provider.

What this adds.
- A registry of the sources of an open line: for each pack the line has on, a source for each entry of its `providers:`,
  named by the entry's key, plus the built-in scratch source. Providers start on first use, are given the pack's
  resolved settings and the board's `root`, and stop with the line.
- A line that is not trusted starts no provider of a repo, user or `git:` pack, and the answer says the line is not
  trusted. Built-in packs and scratch need no trust (0.9).
- `task.get`, `task.apply` and `task.preview` for any source, with the checkout named or, when absent, the line's root.
  One function writes for every source, with the guards of `ts-0114-hr` still to be added to it.
- The scratch event's cause becomes `store` instead of `journal` (`ChangeCause::store`, a wire change with the generated
  files regenerated).

## Success criteria

- A fixture repo with a committed repo pack whose provider is the test fake: after the line is trusted, `task.get`
  returns that provider's task, `task.apply` returns its `ok`, `conflict` and `refused` as the provider gave them (with
  the files touched), and `task.preview` returns `preview`'s answer, or says the source cannot preview.
- Before trust, no provider of that pack starts (the fake writes a marker file when it starts, and it is absent), and
  `task.get` says the line is not trusted. After `collide trust` or `board.trust`, it starts.
- Edit the provider's script in the working copy, or on another branch: the running provider is the committed one at the
  line's ref, and a changed committed script asks for trust again before it starts (4a's hash; the stage's "an edited
  provider script on a branch is not used; a changed hash asks again").
- The pack's settings, resolved through the layers, reach `initialize` as `config`. A secret-valued setting is read
  through the secret store port and never written to a log or an event.
- The source's name is its provider's key in `providers:`. If two enabled packs name the same source, the line reports
  it and starts neither. See Notes for the built-in `plan` case.
- A source that is gone (the supervisor answers `Gone`) is an error that names the pack and the source, and the other
  sources keep answering.
- `task.apply` on a ref-scoped source with no `checkout` writes the line's root.
- A write through any source records one `task.changed` event with the connection's actor, as the scratch write does
  now, and a scratch write's cause is `store`.
- Providers stop when the daemon stops: no child outlives it.

## Tests

Write these first.

- Daemon tests through the socket (the pattern of `host/tests/daemon_socket.rs` and `scratch_tasks.rs`): a trusted
  fixture line with a fake provider pack at a ref, then `task.get`, `task.apply` (each outcome), `task.preview`.
- The trust gate, with the marker file; the changed-script case through a second commit.
- The settings reaching `initialize`, checked by a fake that echoes its `config`; a secret through a memory double of
  the secret store.
- The name clash of two packs; the gone source; the default checkout.
- A test that the real Orchard fixture adapter (`fixtures/packs/orchard/providers/plan.ts`) starts under the supervisor
  from a fixture repo, its `initialize` result passes `core::provider::check_initialize`, and `list` returns the fixture
  tasks. It needs `node`, as the built-in pack tests do.
- The protocol contract tests of the stage, now through the daemon: each method once against the fake.
- A stop test: after the daemon stops, the fake's process is gone.
- The scratch event's cause is `store`, and the schemas and generated TypeScript are current (`pnpm gen` leaves no
  diff).

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 4b: the first bullet (protocol v0 with the pack's settings passed to `initialize`) and the Success Criteria
    line "An edited provider script or view on a branch is not used. A changed hash asks again."
  - 0.10: Protocol; Identity (a task's key is `source:shortId`). 0.9: Trust. 0.7: Settings, Secrets are names. 0.4: Line
    lock.
- Carried from `ts-0075-hj`: the scratch event's cause, `ChangeCause::store`. Also carried there and not done here: the
  board-id reuse and the main-checkout root (6a).
- **Owner's choice, with a recommendation: two sources called `plan`.** The built-in `collide/plan` pack and the Orchard
  pack both declare a provider `plan`, and a board has one planning source (0.10, Planning sources). Recommend: the
  board's own enabled pack wins over the built-in `collide/plan`, which then starts no source on that board; two packs
  that are not built in with one source name make the line report a problem and start neither. Alternative: prefix keys
  with the pack (`orchard.plan:T-01-042`), which changes the keys the plan fixes (`plan:T-01-042`). The first keeps the
  plan's keys, and the Collide repo itself, which has no other planning pack, still gets `collide/plan`.
- **Decision for the item, with a recommendation: secrets in provider settings.** Resolve secret-valued keys through the
  `SecretStore` port at start, with a memory double in every test. The real keychain is not touched here (the real
  store's first user is a source with a token, Stage 5). Alternative: pass no secret until Stage 5. Recommend resolving,
  since the code is small and Stage 5 then needs no daemon change.
- **Decision for the item: lazy start.** A provider starts at its first call, not when the line opens, so a line open
  runs no repo code. The corpus (`ts-0116-wh`) is the first caller at start.
- **Decision for the item: one event for every source.** `task.changed` from a write, with the cause the line's route
  gives (`head`, `worktree`, `store`), as the scratch path does. The change from watch is `ts-0116-wh`.
- Out of scope: `task.list` and the corpus (`ts-0116-wh`), watching a source (`ts-0116-wh`), the line lock, root busy
  and the phase queue (`ts-0114-hr`), copies, collisions and `latest_run` on `task.get` (`ts-0116-wh`).
- Area: `host/src/daemon.rs` (or a `host/src/sources.rs` beside it), `host/src/scratch.rs`, `core/src/provider.rs`
  (`ChangeCause`), `schemas/` and the other outputs of `pnpm gen`, `host/tests/`.
- The test rule: no test touches the real home, real launchd or a real daemon. Each test clears the environment
  (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as `host/tests/common/mod.rs` `TempRoot`
  does. Settings files are fixtures, never the owner's own `settings.json`. Secrets use the memory double.
- Stage 4b item (revision 23 scope, split 2026-11-17). Items of Stage 4b run one at a time, in id order, because they
  share `core`, `host` and the generated files.
- Nothing here runs a real `claude`.
- Owner decisions (2026-11-17, at the split): the recommendations above, on delegation: the board's own enabled pack
  wins over the built-in `collide/plan`, and `ChangeCause::store` is added.
