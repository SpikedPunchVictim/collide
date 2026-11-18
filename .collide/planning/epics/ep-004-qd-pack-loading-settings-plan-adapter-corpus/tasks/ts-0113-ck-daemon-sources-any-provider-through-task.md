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

### Built

- `core/src/provider.rs`: `ChangeCause::Store` (a source's `journal` stays; `store` is Collide's own store, where
  scratch keeps its tasks). `pnpm gen` ran; `schemas/` and `packages/pack-sdk/src/generated/` are in step and a second
  run leaves no diff.
- `host/src/sources.rs` (new). `plan(&LoadedLine) -> Plan` names the sources of a line (`Planned`, `Clash`);
  `source_problems` feeds the line's problems. `Sources::source(board, name, tools) -> Source` is the one door: scratch
  needs no line; any other name loads the line from its commit snapshot, plans it, refuses a clash, refuses a pack that
  is not built in on a line that is not trusted (`SourceError::Refused`, "is not trusted"), then starts the provider
  once, on its first call, with `provider_launch`, the captured base environment, `COLLIDE_PACK_DATA`, the pack's
  resolved settings and the board's root. `Source::{list, get, apply, preview, write_cause}`; `changed_event` builds the
  `task.changed` body. `Sources::stop` ends every provider and starts none after. A slot is kept while its identity
  (pack, provider, cache folder of the snapshot) and trust hold, so a committed script change stops the old child.
- `host/src/config_get.rs`: `ConfigService::pack_config` resolves one pack's keys through the layers. A secret is read
  through the `SecretStore` port. It is never logged or put in an error.
- `host/src/board_lines.rs`: `Lines::for_sources` and `SourceLine`. A clash shows in `line_of` and `review` problems.
- `host/src/daemon.rs`: `Shared.sources`; `DaemonOptions.supervisor`; `task_get`, `task_apply` and the new
  `task_preview` all go through `source_of`; `write` is the one function every source's write goes through (for
  `ts-0114-hr`); `Drop for Daemon` calls `Sources::stop`.
- `host/src/scratch.rs` loses its `changed_event` (now `sources::changed_event`); `host/src/file_run/quick_run.rs` uses
  it with cause `store`.
- Tests: `host/tests/daemon_sources.rs` (19 tests) and `host/tests/fixtures/fake_pack_provider.mjs` (a Node fake that
  runs as a repo pack's provider; `get` ids `boom`, `missing`, `die`, `echo`, `linger`). Updated:
  `core/tests/provider_protocol.rs` (cause list), `host/tests/scratch_tasks.rs`.
- Seen red at runtime. Scratch cause: `each_write_records_a_task_changed_event_with_the_actor_of_the_connection` and the
  cause-list test (journal against store). The new daemon tests were run against five mutations of the code, each put
  back after: `Sources::source` answering `NoSource` for all but scratch (17 of the 18 tests then written failed:
  get, apply, preview, trust, settings, default checkout, event, reference adapter); trust gate off
  (`before_trust_no_provider_starts_and_after_trust_it_does`,
  `a_committed_change_to_the_script_asks_for_trust_again_and_stops_the_running_provider`); `pack_config` result
  dropped (the settings test, the secret test, `a_secret_with_no_item_in_the_store_is_left_out_of_config`); clash
  rule off (the clash test); board pack no longer winning (the `plan` test and the reference adapter test); slot
  identity ignored
  (`a_change_to_the_script_that_is_trusted_before_any_call_replaces_the_running_provider`).
- Not seen red: `no_provider_outlives_the_daemon`. Dropping `Sources::stop` from `Drop for Daemon` leaves it green,
  because dropping a `ProviderClient` also stops its child (`impl Drop`). The explicit stop is the second net.
  `task_preview_of_a_source_that_cannot_preview_says_so` and `a_provider_starts_at_its_first_call...` were not mutated.

### Decisions for the owner

- **Settings and secrets are read when a provider starts.** A changed setting reaches a running provider at its next
  start (a crash, a changed script, a daemon restart). Alternative: restart on a settings change. Recommend as built;
  `ts-0116-wh` can restart on `config set`.
- **Every request reloads the line and its trust** (a few git reads), so a branch change or a trust change is seen at
  once. Alternative: cache per commit. Recommend as built until the corpus item measures it.
- **A `Gone` source stays gone until the line changes or the daemon restarts**; the error names the pack, the source and
  the last failure. Alternative: call `retry` on each request. Recommend as built: the supervisor already backed off.
- **The pack's data folder is `<daemon data>/pack-data/<board>/<pack>`.** Plan text for 0.7 or 0.10: "a provider's
  `COLLIDE_PACK_DATA` is `pack-data/<board id>/<pack name>` under the daemon's data folder".
- **`commit: true` on `task.apply` is refused for every source** (scratch keeps its own text, "never committed") until
  `ts-0120-wm`. Recommend.
- **Preview on a source with no preview is `INVALID_PARAMS` naming the source**, not a `refused` answer, since a refused
  answer means the provider judged the change. Get on an unknown source gives `task: None`; apply and preview on one
  give `INVALID_PARAMS`.
- **Error codes:** not trusted and a name clash are `CONFLICT`; a gone source is `UNAVAILABLE`; a provider failure is
  `INTERNAL_ERROR`. Alternative for not trusted: `NOT_PERMITTED`. Recommend `CONFLICT`: trusting the line fixes it.
- **A write's event cause is `worktree` for a pack source and `store` for scratch.** The daemon cannot tell `head` from
  `worktree` yet; `ts-0114-hr` and `ts-0116-wh` know the route. Recommend.
- **A pack that declares a source named `scratch` is a clash** with the built-in one. Recommend.
- **`DaemonOptions.supervisor`** carries the `SupervisorPolicy`, so a test can use a zero backoff. Recommend.
- **`source.stale` with a `checkout` field** (asked by `ts-0112-px`) is not added here: nothing in this item emits
  `source.stale`. Recommend adding it in `ts-0116-wh`, which watches sources.
- Provider processes share one lock per source (`SupervisedProvider`), and starting one holds only that source's slot,
  so a slow start of one source does not block the others.

- Owner decisions (2026-11-17, at the build): every recommendation above, on delegation. The pack data path needs no
  plan text: 0.7 already says a data directory per pack per board under Collide's data directory, and the subfolder is
  the code's. A changed setting restarting the pack's providers is carried to `ts-0116-wh`. At review: the trust check
  in `refresh` survived a mutation, since every test changed the pack's own folder, so
  `a_committed_change_elsewhere_in_the_line_asks_for_trust_again_and_stops_the_running_provider` was added and seen red
  under that mutation; the gone message now says a changed pack on the line also starts it again. The adapter test
  builds the reference pack's name at run time, as `checks/` does: no product code names it (the rule's intent, 0.1).

### Not done, by design

- `task.list`, the corpus, watching, copies, collisions and `latest_run` on `task.get`: `ts-0115-ug` and `ts-0116-wh`.
  `Source::list` exists for them and is used here only by the reference adapter test.
- The line lock, root busy, the phase queue and the guards on `write`: `ts-0114-hr`.
- `commit: true`: `ts-0120-wm`.
- The real keychain behind `SecretStore`: Stage 5. Board-id reuse and the main-checkout root: 6a.
