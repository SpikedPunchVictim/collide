---
id: ts-0189-fg
kind: task
status: done
title: "App: Changes, the diff window and the id peek"
owners: []
links:
  - { type: blocked-by, to: ts-0188-kb }
---

## Goal

The drawer gets its Changes part: each run that worked on the task, where and how, and its files with their counts,
updating while the agent edits; and the commits that name the task. A file opens the diff window. Any item id in a
body, a note or a result peeks: a small window with the item's state, title, blockers and body.

What exists.
- From `ts-0188-kb`: `task.changes` and `task.diff`, and the heads on `run.get`.
- From `ts-0184-hv`: the drawer and its folds. From `ts-0183-ee`: `Markdown`. From `ts-0180-ad`: each source's
  `id_pattern` and `short_id_pattern`.
- From `ts-0182-ad`: the Field's live events.

What this adds.
- Changes (note 17), placed in 0.11's order: for each run, "Run R-n · <state>", where it ran and how (a worktree, the
  person's checkout, a line), and its files with +N and −N. A live run's part says "Live, as the agent edits" and
  follows the live events. Then the hand commits, each with its subject and files. "No changes yet." when empty. A
  run's entry links to Tracks once Stage 9b ships; until then it names the run only.
- The diff window (note 29): opened from a file. "Changes" shows the patch; "Whole file" shows the file after the
  changes. Lines are numbered, added and removed lines marked, and the window scrolls to the first change. Esc or
  Close closes it. A cut patch says so.
- The id peek (note 34): in text shown through `Markdown` (a body, a note, a result), a word that matches one of the
  board's sources' id patterns and resolves through `task.get` becomes a link. Hovering shows a small window: the
  item's state, title, what it waits on, and its body, which scrolls. A click keeps it open. An id inside the peek
  opens in the same window, with Back. Esc or a click away closes it. An id that does not resolve stays text.

## Success criteria

- **Changes** (note 17) in the app: a CLI run's files show with their counts while the run is active and after,
  and a hand commit naming the id shows too.
- A file opens its diff, on its changes or on the whole file after them, numbered, marked, and scrolled to the
  first change (note 29).
- An id in a body peeks on hover, stays on click, opens an inner id with Back, and closes on Esc and on a click
  away (note 34). An unknown id is plain text.

## Tests

Write these first.

- `app/test/`: Changes from a `FakeOwner` world (a live run, a done run, a commit, nothing); the diff window's
  numbering and marks; which words become id links.
- `app/browser/fake-peek.spec.ts` (Stage 6a test "the id peek in a browser"): hover, click, inner id and Back, Esc,
  click away.
- `app/browser/fake-diff.spec.ts`: open, switch to whole file, scrolled to the first change, Esc.
- `app/browser/changes.spec.ts` (bridge): a file run with the world's fake `claude` on a task; its files in Changes
  while active and after; a hand commit naming the id.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 6a: Task drawer (Changes; a file opens its diff; An id peeks). Success Criteria: Changes. Tests: "the id
    peek in a browser".
  - 0.11: Task drawer, Changes; the id peek.
- Design: notes 17, 29 and 34. Mock functions: `changesHtml`, `diffModal`, `diffOf`, `wholeOf`, `idLinks`,
  `peekHtml`, `showPeek`, `hidePeek`, `placePeek`, `drawPeek`.
- **Decision for the item, with a recommendation: matching ids.** Recommend matching only the patterns the board's
  sources declare, and resolving each through `task.get` once per id while shown. Alternative: matching any
  `xx-0000-xx` shape; it links words no source knows.
- Out of scope: the link to Tracks (Stage 9b); a run's own result page (9b).
- Area: `app/src/` (the drawer's Changes, a diff window, the peek, `Markdown`'s id links), `app/src/owner/methods.ts`,
  `app/test/`, `app/browser/`.
- Dependencies: none.
- The test rule: no test touches the real home, real launchd, a real daemon, the keychain or the network. Each test
  that runs the `collide` binary clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to
  a temp root, as `host/tests/common/mod.rs` `TempRoot` does. A temp repository sets `GIT_CONFIG_GLOBAL` and
  `GIT_CONFIG_SYSTEM` to empty files. Keychain writes go through `MemorySecretStore` (`host/src/secret_store.rs`),
  never the login keychain. Browser tests run the installed Chrome with a temp profile
  (`app/browser/support/chrome.ts`), and the bridge's daemon on a temp root (`app/browser/support/world.ts`).
- Stage 6a item (revision 25 scope, split 2026-11-24). Items of Stage 6a run one at a time, in id order, because they
  share `app/`, `core`, `host` and the generated files. No builder runs `tauri build`.
- Nothing here runs a real `claude`: the bridge spec's run uses the fake `claude` of `app/browser/support/world.ts`.
- Owner decisions (2026-11-24): the owner took the recommendations at the split.


### Built

- Changes part, `app/src/drawer/`. `changes.ts` holds the words as plain functions (`runHeading`, `whereWords`,
  `filesWords`, `sizeWords`, `shortCommit`, `isLive`, `hasLiveRun`, `isEmpty`, `NOTHING_YET`, `LIVE_WORDS`) over the
  generated `TaskChangesResult`. `Changes.tsx` (`ChangesPart`) draws each run ("Run r-n · state", where it ran and how
  in the wire's own words, "Live, as the agent edits" for a live run, its files with +N and −N, a binary file as
  "binary file", the folders and files left out), then the commits (short id, subject, files). "No changes yet." when
  empty; the wire's note as it comes ("no changes recorded"). `model.ts`: `PartId` has `changes`, and `partsOf` puts it
  after Part of and before Fields from the source (0.11), for any task the daemon names (not for a collided key).
  `DrawerView.tsx` takes `changes` and `onOpenFile`; `Drawer.tsx` reads them and holds the diff window's state.
- Live: `changes-feed.ts` (`ChangesFeed`, `isChangesEvent`, `LIVE_REFRESH_MS`) reads `task.changes` once for a burst of
  run, phase, workspace, gate, permission and task events and when the stream comes back (`Burst` and `Clock` from
  `field/live.ts`), and every 5 s while a run is live, because an edit in a worktree sends no event. A failed read
  keeps what was read and says why. `useChanges.ts` is the hook, keyed by board and task, off while the part is folded.
- Diff window, `DiffWindow.tsx` and `diff.ts` (`patchLines`, `wholeLines`, `firstChange`): a modal `<dialog>` (so the
  drawer's Escape, which already stands back for an open dialog, leaves the key to it), "Changes" and "Whole file",
  numbered lines, `+` and `−` marks, scrolled to the first change, a cut patch or file says so, a binary file and a
  note from the wire are shown. Esc and Close shut it.
- Id peek, `app/src/peek/`: `ids.ts` (`compilePatterns`, `candidateWords`, `splitIds`, `MAX_LOOKUPS`) takes only the
  patterns the sources declare; `book.ts` (`IdBook`) calls `task.get` once per word and keeps the answer; `remarkIds.ts`
  turns matching words into `collide-id` elements; `IdRef.tsx` is the button; `Peek.tsx` provides the book and the
  window (hover after 350 ms, goes 250 ms after the pointer leaves, a click keeps it, Back, Esc in the capture phase,
  a click away); `peek.ts` is the reducer. `Markdown.tsx` asks the book and links the words that were found. `App.tsx`
  wraps the window in `Peek`. `index.css` has `.md .id-ref`.
- Test support: `FieldWorld` (`test/support/field-world.ts`) answers `task.changes` and `task.diff` (`changes`,
  `fileDiffs`, `refuseChanges`, `announceRun`) and takes a short id in `task.get`; `test/support/changes-world.ts` has
  the `field-changes` and `field-peek` worlds (registered in `worlds.ts`); `browser/fake/main.tsx` has `setChanges`
  and `announceRun`; `browser/support/recordings.ts` has `heldPhase`.
- Tests, vitest (`app/test/`): `diff.test.ts`, `changes.test.ts`, `changes-feed.test.ts`, `peek-state.test.ts`,
  `id-links.test.tsx`, `drawer-changes.test.tsx`; `drawer.test.tsx` now lists `changes` in its part orders.
- Tests, browser: `fake-peek.spec.ts` (7), `fake-diff.spec.ts` (11), bridge `changes.spec.ts` (3); the heading list
  of `fake-drawer.spec.ts` has Changes.
- Red at runtime before the code: against stubs that did nothing, 96 tests failed by assertion in the six new
  vitest files (`diff`, `changes`, `changes-feed`, `peek-state`, `id-links`, `drawer-changes`); the tests that say
  something is absent passed against the stubs, as they should.
- Red at runtime with Changes and the id patterns switched off: with `partsOf` leaving out `changes` and
  `compilePatterns` returning no pattern, all 21 browser tests of `fake-peek.spec.ts`, `fake-diff.spec.ts` and
  `changes.spec.ts` failed at runtime (an expect that timed out on a missing element or text); the two edits were
  then put back.
- Checks: `cargo build --workspace`, `cargo clippy --workspace --all-targets -- -D warnings` and
  `cargo fmt --all --check` pass (no Rust changed, so `cargo test` was not run);
  `pnpm --filter @collide/app exec vitest run` 740 passed in 29 files; `pnpm typecheck`,
  `pnpm --filter @collide/app build`, `pnpm build:packs --out <temp>/packs` and `pnpm check` exit 0;
  `pnpm exec playwright test` in `app/` 96 passed (bridge and fake-owner specs).

### Decisions for the owner

- **A run on a task is a prompt run, and only a scratch task runs.** The item says "a file run". `run.start` refuses a
  plan task ("only scratch tasks run so far"), and a scratch task lists no commits, so `changes.spec.ts` has two
  tests: a prompt run on a scratch task (held at a permission request with a file written, then allowed), and a hand
  commit naming a plan task's id. Alternative: wait for runs on plan tasks. Recommend as built.
- **Live is events plus a 5 s refresh while a run is running.** Alternative: events only; a file written in a worktree
  would show at the next event, which can be minutes away. Recommend as built; the cost is one `task.changes` per
  5 s for a task whose Changes is open and has a live run.
- **Run events are not filtered by run id.** Any run or phase event re-reads the open task's changes. Alternative:
  filter by the task's runs; the cost is knowing a new run's id before its event. Recommend as built.
- **One number column.** A patch line has the number it has after the changes; a removed line has none. The whole-file
  view cannot show removed lines because `task.diff` gives none. Alternative: old and new columns in the patch view.
  Recommend as built.
- **The window opens on Changes; the choice is not kept; no previous or next file.** Alternative: keep the choice.
- **Ids.** Only a word that matches a source's `id_pattern` or `short_id_pattern` is asked of `task.get`, once per id
  for as long as the board's reading (its stamp) is the same; a new reading makes a new book, so a peek never shows an
  old task (a link can blink out for the moment it takes to ask again). An ambiguous or failed `task.get` stays text.
  A word after `/`, `.` or `#` is not an id. An id in a backtick span links only when the span is that one id. An id in
  link text or a fenced block is not linked. At most 200 words of one text are asked (a pattern for plain numbers
  matches every number); with 40 an id at the end of a long body was not linked, which a browser test showed.
  Alternative: no cap. Recommend as built.
- **Left out of the mock:** "Open in the drawer" and "Copy id" in the peek. Alternative: add them with the Tracks
  link in Stage 9b.
- **Changes is left out for a collided key** (the daemon names no task), as Waits on is.
- **"Run R-n" is drawn as "Run r-12"**, the run's own id.
- Proposed plan text for 0.11 (no edit made): "Changes is drawn for every task the daemon names; a scratch task shows
  its runs, a plan task its runs and the commits that name it. While a run is live it is read again every 5 s."

### Not done, by design

- The link from a run's entry to its place in Tracks, and a run's own result page: Stage 9b.
- Runs on plan tasks, so a plan task has runs and commits together in the bridge test: when the loop runs them.
- A run in the person's checkout is drawn in the wire's words and tested from the fake owner only: no run works in
  the checkout yet.
- Previous and next file in the diff window; removed lines in the whole-file view (the wire has none).
