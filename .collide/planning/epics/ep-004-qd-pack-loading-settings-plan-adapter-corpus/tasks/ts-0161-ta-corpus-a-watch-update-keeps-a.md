---
id: ts-0161-ta
kind: task
status: ready
title: "Corpus: a watch update keeps a stale row when another file takes an item's id"
owners: []
links: []
---

## Goal

In `ts-0138-pn` step 7 (agent-run, 2026-11-20) a copy of a task file with the same `id:` was committed while the daemon
ran. The next `collide tasks` listed two rows with the one key, as well as the `skipped` line for the file set aside.
`collide task reid <id>` (bare id) was then refused "matches 2 items", exit 3. After the daemon was ended and started,
the listing had one row and the reid passed. Right after the reid, the first listing still printed one `skipped` line
for a file that no longer clashed; the next listing printed none.

So the watcher's update path and a full listing disagree: a full listing keeps one row per key and sets the other file
aside, while a watch update adds the second file's row next to the first.

## Success criteria

- After a watch update that brings a second file with an item's id, the corpus holds one row for the key, the same one
  a full listing holds, and the other file is listed as skipped with the same reason.
- After the clash is resolved (reid, or the file removed), the next listing has no `skipped` line for it.
- A bare-id `collide task reid` on the clash works with the daemon running, with no restart.

## Tests

- Write first and see red: in `host/tests/daemon_corpus.rs` style, a running daemon, a second file with a taken id
  written and seen by the watcher: one row, one skipped entry.
- The same, then the copy removed: no skipped entry on the next listing.

## Notes

- Where the two rows come from (the plan pack's `watch` notifications or the daemon's corpus apply of them) is for the
  builder to find. The full listing's rule (the first file in scan order keeps the id) is the one to match.
- Owner decisions: none recorded yet. The recommendations stand until the owner answers.
