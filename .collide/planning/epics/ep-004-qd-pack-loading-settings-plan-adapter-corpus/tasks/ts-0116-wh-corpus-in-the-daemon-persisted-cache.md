---
id: ts-0116-wh
kind: task
status: ready
title: "Corpus in the daemon: persisted cache, startup reconcile and task events"
owners: []
links:
  - { type: blocked-by, to: ts-0115-ug }
---

## Goal

Run the corpus in the daemon: list every source of a trusted board across its checkouts, keep the answer in the store,
reconcile it at start, follow the providers' notifications into events, and serve `task.list` and a full `task.get`.

What exists. After `ts-0113-ck` the daemon starts any source of a line and serves `task.get`, `task.apply` and
`task.preview` for one. After `ts-0115-ug`, `core/src/corpus.rs` holds the corpus as a pure structure with `reconcile`.
After `ts-0108-dq`, the supervisor hands a provider's notifications over as a stream. The store (`host/src/store.rs`, 8
migrations) keeps the event log, runs (`host/src/run_store.rs`, with `run_tasks` by task key) and Queue items;
`core/src/events.rs` has the events `task.changed`, `task.renamed`, `task.removed`, `task.invalid` and `task.written`.
`host/src/checkout.rs` lists `git worktree list`, and the board's `checkout_filter` is in `core/src/board.rs`.
`task.list` is not served; `task.get` answers one copy, with `copies`, `collisions` and `latest_run` empty.

What this adds.
- A migration for the corpus cache (tasks and invalid items, by board, source and checkout).
- A corpus service for each board: the checkouts from `git worktree list` that pass the filter; a `list` of every source
  for each of them, in pages; `watch` on the sources that have it; a recheck of the checkout set every 10 s; a relist of
  a source that has no `watch`.
- The startup: load the cache, list, `reconcile`, record one `task.changed` with `cause: startup` for each item that
  differs, and replace the cache. The first listing runs in the background and does not hold up the daemon's start.
- The notification pump: each notification updates the corpus and the cache and records its event (`task.changed`,
  `task.renamed`, `task.removed`, `task.invalid`), tagged with its source. `source.stale` and a provider that is gone
  mark the source stale.
- `task.list` (no `view` yet) for `ready`, `kind` and `checkout`, with paging, and `task.get` with its copies,
  collisions, runs, `latest_run` and `ambiguous`.

## Success criteria

- A trusted fixture board with two checkouts and a provider (the fake, and also the Orchard fixture adapter over its
  `arch/` tree) lists through `task.list`: the tasks, the collections and the invalid items, paged by `limit` and
  `cursor`, each task once.
- Stop the daemon, change two tasks and add one in the checkouts, delete one, start it: the log holds one `task.changed`
  with `cause: startup` for each changed or new task and one `task.removed` for the deleted one, and none for the
  unchanged. A second start with no change records none.
- A change to a file while the daemon runs reaches the log as one `task.changed` with the provider's cause and checkout.
  A write made through `task.apply` and the watcher's notice of the same write are one event, with the connection's
  actor.
- An unparseable file gives `task.invalid` in the log and in `task.list`'s invalid items, and fixing it clears it. The
  count is the invalid items of the board (the view's count comes in `ts-0118-wp`).
- A rename in a checkout is `task.renamed`, the old key resolves to the new one, and the task's runs follow the key.
- `task.get` returns the copy at each checkout, the divergence, the collisions, the ids of the task's runs newest first,
  and the latest finished run, for a task that has them.
- A scratch task is listed for its own board only: two boards, each with a scratch task, each lists only its own (the
  stage's last criterion). A scratch write shows up in `task.list` at once.
- A source whose provider is gone is marked stale, its last tasks stay listed, the other sources keep answering, and a
  restart brings it back.
- The cache is the same after a clean stop and after a kill, and a corrupt row is dropped with a log line, not a refusal
  to start.
- No provider of an untrusted line is listed or started.

## Tests

Write these first. Daemon tests as in `host/tests/daemon_socket.rs`, with a fake provider and, for the end-to-end ones,
the Orchard fixture adapter (needs `node`, as the built-in pack tests do).

- The migration, with the pattern of the store's tests for the older versions.
- List and paging; the invalid items; two checkouts made with `git worktree add`.
- The startup reconcile: stop, change, start; counts and shapes of the events; a second start with none.
- A watched change and a write through `task.apply`: one event.
- Unparseable and repaired; rename; a deleted file.
- `task.get` with copies, a collision (the same short id at two paths) and runs (a run row for the key).
- Scratch for two boards.
- A source that dies: stale, and the others answer.
- The cache after a kill (the store is opened by a second process); a corrupt row.
- An untrusted line starts nothing (the marker file of `ts-0113-ck`).
- The time to list a generated tree of 1,600 tasks on the fake, recorded in the item and not asserted.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 4b: Corpus ("The corpus cache persists in the store. On start, core reconciles each source against it and
    emits one `task.changed` per item with `cause: startup` (0.10; S11)"; "An unparseable item emits `task.invalid` and
    counts on its view"); Success Criteria: "A scratch task created with `collide task new` is listed for that board
    only."
  - 0.10: Protocol (the notifications), Ref-scoped sources (the checkout filter), Collections. 0.11: Task drawer. S11
    (`sp-0008-bk-derived-point-coverage`): G7, G8.
- Carried from `ts-0018-dd`: the events `task.changed`, `task.renamed`, `task.removed` and `task.invalid` are in the
  catalogue, and 4b is where a source emits them.
- **Owner's choice, with a recommendation: whether `task.invalid` makes a Queue item.** 0.10 says core turns it into a
  Queue item, and S11 G8 asked for one. `QueueKind` has no kind for it, and the stage text says only that it emits the
  event and counts on the view. Recommend the event and the count here, and a Queue item when the Queue page exists
  (Stage 6), with a new `QueueKind` then, so no item is a dead end today. Alternative: add the kind now and a Queue item
  for each invalid path. It puts items in a Queue nothing shows yet. This decides what the owner sees about a broken
  task file.
- **Decision for the item, with a recommendation: wire fields for the list.** `TaskListResult` gains `divergent` (keys),
  `collisions`, `stale_sources` and `refreshing`, all additive, so a board can show the chips and a first list that is
  still running. Alternative: only `task.get` carries them. The board then cannot mark a card.
- **Decision for the item, with a recommendation: one event for a write.** `task.apply` records the write's event with
  the actor, and remembers (key, checkout, version); a notification with the same triple is dropped. Alternative: record
  both and let readers merge. Recommend the first: observers see one change.
- **Decision for the item: `latest_run`.** The newest run of the key whose state is final: `id`, `workflow` and
  `outcome` from its `RunState`. `state` holds the run's frozen outputs where the store keeps them, and is empty until
  the engine (Stage 8) writes them. Check `host/src/run_store.rs` and the phase records first.
- **Decision for the item: rechecks.** The checkout set and any source with no `watch` are rechecked every 10 s, the
  plan's own interval for the plan source. A scratch source needs none, since only the daemon writes it.
- **Decision for the item: the cache is not the truth.** It is read at start only. A source's `list` replaces it.
  Nothing else reads it.
- **Decision for the item: collisions have no event.** Core's collision list comes from the lists; the provider's
  `task.collision` notification is used as a hint to relist. The event catalogue gets no new event.
- Out of scope: views (`ts-0118-wp`, `ts-0119-tj`); the app's chips (Stage 6); experiment-prefix branches excluded from
  checkouts (0.14, Stage 14); the adopted-worktree reading and its minute poll (6a); `task.sections` (4c).
- Area: `host/src/store.rs` (migration), a new `host/src/corpus_store.rs` and `host/src/corpus_service.rs`,
  `host/src/daemon.rs`, `host/src/run_store.rs` (a read), `core/src/owner/task.rs`, `schemas/`, `host/tests/`.
- The test rule: no test touches the real home, real launchd or a real daemon. Each test clears the environment
  (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as `host/tests/common/mod.rs` `TempRoot`
  does. Settings files are fixtures, never the owner's own `settings.json`. Every git repository is a temp one with
  `GIT_CONFIG_GLOBAL` and `GIT_CONFIG_SYSTEM` set to empty files.
- Stage 4b item (revision 23 scope, split 2026-11-17). Items of Stage 4b run one at a time, in id order, because they
  share `core`, `host` and the generated files.
- Nothing here runs a real `claude`.
- Carried from `ts-0112-px` (2026-11-17): `source.stale` carries only `reason`, and the `plan` adapter names the
  checkout in that text. If the daemon acts per checkout (marks one checkout's tasks stale and not the others), add a
  `checkout` field to `SourceStale` in `core/src/provider.rs` here, run `pnpm gen`, and have the adapter's `watcher.ts`
  send it.
- Carried from `ts-0113-ck` (2026-11-17): a provider reads its settings and secrets at start, so a `config set` of one
  of its pack's keys reaches it only at its next start. `config.set` runs in the daemon: have it stop that pack's
  providers (every board's, for a user-scope key), so the next call or watch starts them with the new settings, and test
  it.
- Owner decisions (2026-11-17, at the split): the recommendations above, on delegation: `task.invalid` is an event and a
  view count now, and a Queue item comes with Stage 6's Queue page.

### Built

- Store: migration 10 (`host/src/store.rs`) adds `corpus_tasks`, `corpus_invalid` and `corpus_sources`, keyed by
  board, source and checkout. `host/src/corpus_store.rs` reads and replaces them. A row that does not read is
  dropped with a log line and the start goes on. The declaration of each source (`corpus_sources`) is kept too, so a
  restart can serve the cached list before any provider is up.
- `host/src/corpus_service.rs`: one service per board. Checkouts come from `git worktree list` through
  `checkout_filter`; `list` is paged per source and checkout; `watch` is sent to sources that have it; the checkout
  set is rechecked every 10 s (`DaemonOptions::corpus_recheck`); no-watch sources are listed again at each look. The
  cache is read and served in `BoardCorpus::start`, before the thread runs, so the first `task.list` after a restart
  is the cached one with `refreshing: true`. The startup reconcile records `task.changed` (cause `startup`),
  `task.removed` and `task.invalid`, then replaces the cache. A pump tags each notification with its source.
  `source.stale` or a gone provider marks the source stale, its last tasks stay listed, a restart brings it back.
  A rename fetches the renamed copy first and applies rename and task together, so a list never shows the new key
  with the old id. An untrusted line's providers are never listed or started. Scratch is listed for its own board
  and a scratch write shows in `task.list` at once.
- `task.apply` records its event with the connection's actor and remembers (key, checkout, version); a watcher
  notification with the same triple is dropped. `task.invalid` is an event and a count, with no Queue item.
  Collisions have no event; the provider's `task.collision` only triggers a relist.
- Wire (`core/src/owner/task.rs`, `core/src/provider.rs`, `core/src/corpus.rs`): `TaskListResult` gains `divergent`,
  `collisions`, `stale_sources`, `refreshing`; `task.get` returns copies, collisions, runs (newest first),
  `latest_run` (newest final-state run, empty `state`) and `ambiguous`. `pnpm gen` was run.
- ts-0112-px: `SourceStale` has `checkout`; `fixtures/packs/orchard/providers/plan/watcher.ts` sends it, with a test
  in `fixtures/arch-plan-tests/test/watch.test.ts`.
- ts-0113-ck: `config.set` calls `Sources::stop_pack`, for every board on a user-scope key and the board's own on a
  repo key; test `a_config_set_of_a_packs_key_stops_that_packs_providers_...` (it also shows another pack's
  provider keeps running).
- Tests: `host/tests/daemon_corpus.rs` (25), `host/tests/corpus_store.rs` (11), `host/tests/corpus_checkouts.rs`
  (6), additions to `core/tests/corpus.rs`, `core/tests/provider_protocol.rs` and
  `host/tests/provider_supervisor.rs`. The provider the daemon tests start is
  `host/tests/fixtures/corpus_pack_provider.mjs`.
- Existing test changed: `host/tests/write_guard.rs` `Rig::changed()` now leaves out `cause: startup` events, since
  the board's plan pack tasks are now recorded at startup and the six tests counted them as writes.
- Seen red at runtime against stubs of the pre-change behaviour (stub in the daemon, tests run, stub removed):
  `stop_pack` doing nothing (the `config.set` test red); the same-triple drop removed (the `task.apply` event test
  red); the cache write skipped (four tests red: removed-and-cache, restart-records, bad cache row, restart-serves);
  `mark_stale` doing nothing (the gone-provider test red). Not seen red by a stub, written after the code: paging,
  filters, divergence, collision, runs order, scratch, untrusted line, new worktree, no-watch relist, and the
  store and core tests. The first-start race of the cached list was seen red as a real failure before its fix.
- Time to list a generated 1,600-task tree through the daemon on the fake (16 folders of 100, page size 100): 3.8 s
  from `board.open` to a complete list with 1,600 startup events. Not asserted.
- Checks: `cargo build --workspace`, `cargo test --workspace --no-fail-fast` (149 suites, none failed), `cargo clippy
  --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`, `pnpm test` (the `packs/plan` suite passed),
  `pnpm typecheck`, `pnpm --filter @collide/app build`, `pnpm build:packs`, `pnpm check` all pass.

### Decisions for the owner

- A user-scope pack setting is stored per board (`config set` needs a board, and its user file is that board's). The
  brief asks that a user-scope key stop that pack's providers on every board, so it does, though only the named
  board's value changes. Recommend keeping it: it is cheap and safe, and a global pack setting may come later. Option:
  stop only the named board's providers (fewer restarts, but wrong if a global layer arrives).
- Startup records one `task.changed` per task on every start where the task is new or changed since the cache, and
  nothing for an unchanged one. A first start of a board with N tasks makes N events. Recommend keeping it (the
  event log is the history); option is one summary event, which loses the per-task rows the views need.
- Plan text proposed for 0.10: "A rename gives `task.renamed`; the old key resolves to the new one and the runs
  follow the key. The corpus cache is not the truth: it is served at start, marked refreshing, and replaced by the
  first listing."
- Checkouts read from the cache before git is asked are served as `Other` until the first listing knows their kind,
  so `divergent` is empty for the first moments of a restart. Recommend accepting.

- Owner decisions (2026-11-18, at the build): every recommendation above, on delegation. The cache half of the proposed
  0.10 text is applied to the Stage 4b Corpus cache bullet; the rename half was already in 0.10 (`task.renamed`, runs
  and `latestRun` follow the key). At review, two mutations were caught: a startup that ignores the cache
  (`a_restart_records_what_changed_while_the_daemon_was_stopped_and_the_next_restart_records_nothing` failed) and a bad
  cache row left in place (`a_row_that_does_not_read_is_dropped_and_named_and_the_others_are_kept` failed). The
  `write_guard.rs` rig now leaves out `cause: startup` events, which are not writes.

### Not done, by design

- Views (`ts-0118-wp`, `ts-0119-tj`), the CLI (`ts-0120-wm`), a Queue item for `task.invalid` (Stage 6), the adopted
  worktree reading and its minute poll (6a), `task.sections` (4c).
- The 1,600-task time is recorded, not asserted.
