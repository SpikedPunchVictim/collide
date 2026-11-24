---
id: ts-0189-fg
kind: task
status: ready
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
