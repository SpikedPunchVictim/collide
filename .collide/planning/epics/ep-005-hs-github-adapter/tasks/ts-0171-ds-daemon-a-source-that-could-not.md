---
id: ts-0171-ds
kind: task
status: ready
title: "Daemon: a source that could not check has one Queue item"
owners: []
links:
  - { type: blocked-by, to: ts-0170-gh }
---

## Goal

A source that "could not check" (an expired token, no network, a repository that moved) keeps its tasks, removes
nothing, and raises one Queue item that names the cause and the fix and lets the person retry. Plan section 0.2 ("A
check that cannot finish") describes it.

What exists. The corpus half is built: `Corpus::mark_stale` keeps a source's last tasks, `reconcile_by_source` skips a
stale source so a full list never removes on its account, and `task.list` lists `stale_sources` with the reason. The
daemon marks a source stale on `source.stale` and when a start fails (`failed_to_start`, `mark_source_stale` in
`host/src/corpus_service.rs`), and rechecks a stale source with a backoff (`relist`). `ts-0168-tq` gives the stale mark
a cause. There is no Queue item for it: `QueueKind` (`core/src/owner/queue.rs`) has `Conflict`, `Confirmation` and
others, and `grep` finds no "could not check" text. `AnswerKind` has `Retry`, `Proceed` and `Dismiss`. The app renders
a kind in `app/src/loop/Queue.tsx`.

What this adds.
- `QueueKind::SourceCheck`: one item for each stale source of a board. It is derived from the corpus, not stored: it
  exists while the source is stale and is gone when the source is fresh. The title is Collide's own words for the cause
  (`auth`: "Sign in again to check <source>"; `network`: "Could not reach <source>"; `rate_limit`: "<source> is rate
  limited"; `missing`: "Locate the repository of <source>"; `other`: "<source> could not be checked"). The reason the
  source gave is the detail, as plain text; it already names the fix (for GitHub, `ts-0172-xc` writes it). No task text
  is in either.
- Answers `Retry` (relist that source now; a success makes it fresh and removes the item, a failure leaves one item)
  and `Dismiss` (hides it until the source is fresh and stale again; held in memory, so a restart shows it again).
- A source that fails to start (its program is missing) raises the same item with cause `other`.
- `Queue.tsx` renders the kind with its title, detail and the two answers; the kind's words go in
  `app/src/components/words.ts` and `app/src/loop/words.ts`.
- "Continue anyway" (`Proceed`) is not added. Nothing is held yet (no run, Up next or hold exists before Stage 9), so
  there is nothing to release. It is carried to Stage 9.

## Success criteria

- A fake provider's `source.stale` with cause `auth` gives exactly one Queue item in `queue.list`, with the title above
  and the reason as detail. A second notification does not add another.
- While stale, a full `list` that returns nothing removes no task, and `task.list` still shows the cached ones.
- `Retry` relists. With the provider healthy, the source is fresh and the item is gone. With it still failing, there is
  still one item.
- `Dismiss` hides the item. After the source is fresh and stale again, it is back.
- A source that cannot start has one item with cause `other`, and the board's other sources are unaffected.

## Tests

Write these first.

- Socket tests in `host/tests/` with a fake provider (`host/tests/fixtures/`) that goes stale, recovers and fails
  again on command, the shape of `daemon_corpus.rs`: item count, title, detail, Retry, Dismiss, a restart, a source
  that does not start.
- A `core` serde test of `QueueKind::SourceCheck`, and the schema and generated-type checks.
- `Queue.tsx` component test for the kind, as `app/test/loop-view.test.tsx` does for confirmations.

## Notes

- Area: `core/src/owner/queue.rs`, `host/src/corpus_service.rs`, `host/src/daemon.rs` (`queue_list`, `queue_answer`),
  `app/src/loop/Queue.tsx`, `app/src/components/words.ts`, `app/src/loop/words.ts`, `app/test/`, `host/tests/`.
- The test rule: no test touches the real home, real launchd, a real daemon, the keychain or the network. Each test that
  runs the `collide` binary clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp
  root, as `host/tests/common/mod.rs` `TempRoot` does. A temp repository sets `GIT_CONFIG_GLOBAL` and
  `GIT_CONFIG_SYSTEM` to empty files.
- Stage 5 items run one at a time, in id order. Nothing here runs a real `claude`. No dependency is added.
- A start failure now raises a Queue item where before it only marked the source stale and logged it. The
  recommendation is yes, for one wording of one problem; the alternative is the item for a typed cause only.
- `ts-0113-ck` has a note about a keychain check of `gh`. The code has no such check (`CheckId::GhLoggedIn` in
  `core/src/owner/board.rs` is declared and never produced), so this item does not use it.
- Owner decisions: none recorded yet. The recommendations stand until the owner answers at the split.
