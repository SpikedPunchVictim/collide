---
id: ts-0161-ta
kind: task
status: done
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

### Built

A write the daemon made is now held in the corpus where the source's list holds it, and it clears the skipped entry
of a file it renamed. Two rows and a stale `skipped` line had one cause. Neither came from a `watch` notification.

**The cause.** The plan pack declares `watch: false`, so it sends no notification: the daemon lists it again at each
look (`Inner::look_at`). The rows came from the daemon's own corpus apply of a write (`BoardCorpus::note_write`,
called from `apply_and_record` in `host/src/daemon.rs`). The plan pack's answer to a write names no checkout on its
task (`toTask` in `packs/plan/src/scan.ts`), so the corpus held the written copy in no checkout (`None`). A list of
the plan source is per checkout, and `Corpus::load` replaces only that checkout's copies, so the `None` copy was never
replaced. Two effects:

- When a second file sorted before the written one and took its id, the list held the key at the second file's path
  in the checkout, and the `None` copy held it at the written file's path. A key at two paths is a collision, and a
  collided key has a row for each path: two rows. The skipped line named the written file. A bare id then matched
  both rows (`name_in_board` in `host/src/task_daemon.rs`): "matches 2 items", exit 3.
- A reid renames the file it sets aside, so the written copy's path was new, and the invalid item of the old path
  (kept under the checkout) was not cleared by it. It stayed until the next look, then went.

A restart cleared both: the cache holds no `None` copy of a ref-scoped source, and the first list is complete. A file
that sorts after the written one gave one row, because both copies had one path; that is why the 0138 text's
`-zcopy.md` showed only the stale line.

**The rule.** The full listing's rule (the first file in scan order keeps the id, the other is set aside with
"is already used by <path>") lives in `scanEpics` (`packs/plan/src/scan.ts`), and in `Corpus::load` for a list that
names a key twice in one checkout. Neither is copied. The change only makes a written copy sit where a list puts it,
so the list's result is what the corpus holds.

**The change.**

- `host/src/corpus_service.rs`: `Run` keeps the source's `scope`. `Inner::written_checkout` gives a written copy the
  checkout the write went to when the source is ref-scoped and its task names none (the write's `checkout`, else the
  board's root, matched to the board's checkout set by canonical path). A task that names a checkout keeps it, and a
  global source's copy has none, as before. `BoardCorpus::note_write` takes `files` and `written_in`.
- `core/src/corpus.rs`: `Corpus::clear_invalid` drops the source's invalid items of a checkout at given paths.
  `Inner::note_write` calls it with the write's `files` before it applies the effects, so a task the write puts in
  may add its own. A write leaves each file it names valid, or gone.
- `host/src/daemon.rs`: `apply_and_record` passes `files` and the write's `checkout` to `note_write`.

**Tests.** `host/tests/daemon_plan_clash.rs`, with the real plan pack:
`a_file_that_takes_the_id_of_an_item_the_daemon_wrote_leaves_one_row_and_one_skipped_entry`,
`a_bare_id_reid_of_the_clash_works_with_the_daemon_running` (through `run_with`, exit 0), and
`the_skipped_entry_is_gone_from_the_next_listing_once_a_reid_ends_the_clash` (the look is a minute away, so only the
write can clear the entry). `core/tests/corpus.rs`:
`clearing_invalid_items_by_path_drops_only_those_of_that_checkout_and_those_paths`.

**Seen red at runtime.** The three daemon tests, run against the code before the change (it compiled): the first with
two rows for `plan:ts-0002-ra`; the second with "`ts-0002-ra` matches 2 items", exit 3; the third with the skipped
entry of the renamed file still listed. With only the checkout change, the third still failed, which showed the
second cause. The core test is for a new function and has no earlier behavior to fail.

**Why not `daemon_corpus.rs`.** Its fake source names the checkout on every task, so it cannot show the cause. The
tests use the plan pack, as the other `daemon_plan_*.rs` do.

### Decisions for the owner

- **Fix in the daemon, not the plan pack.** The pack could name the checkout on the tasks it answers a write with.
  That fixes only this pack: any ref-scoped provider that leaves it out would leave the same copy. The daemon already
  fills the checkout on a list (`list_all`), so it fills it on a write. Recommendation: the daemon, as built.
- **A write clears invalid items by the `files` it names**, not by a new list after each write. A list after a write
  is slower and lands after the answer, so "the next listing" would depend on timing. Alternative: mark the source
  for a look (`wanted`). Recommendation: as built.
- **The cache is not touched.** A cached skipped item of a renamed file stays in the store until the first list of
  the next start, which replaces it. The cache serves only until that list. Alternative: a store call that drops an
  invalid item by path. Recommendation: leave it; add it if the cache ever serves longer.
- **Item text 0138 step 7:** the note "the first listing right after the reid may still print one, and the next none"
  can go once this is in; also the `-zcopy.md` remark, since either name now gives one row. I did not edit 0138.
  No change to `IMPLEMENTATION_PLAN.md`.

### Not done, by design

- A file removed by hand while the daemon runs is seen at the next look (10 s), as before. No write is involved, and
  the list's rule already gives one row. Nothing was changed for it.
- A provider that watches (`watch: true`) and sends `task.invalid` or `task.changed` for the second file was not
  changed: `Corpus::apply_invalid` and `apply_changed` already take the notice's own checkout. The plan pack does
  not watch, so no test exercises it here.
