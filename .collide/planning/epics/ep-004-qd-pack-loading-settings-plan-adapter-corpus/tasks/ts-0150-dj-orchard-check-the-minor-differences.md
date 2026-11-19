---
id: ts-0150-dj
kind: task
status: ready
title: "Orchard check: the minor differences"
owners: []
links: []
---

## Goal

The small differences the agent run of `ts-0121-xp` found (2026-11-19). Each is fixed, or dropped with a reason in
Built.

## Success criteria

- The Orchard plan provider declares no kinds, so `--kind widget` exits 0 with an empty list, and the text of `task
  <key>` has a blank `kind` line where the listing shows `-`. It declares `task`, and the text prints `-`.
- The fake's illegal-move message has other words and one line; the real CLI's names the transition and the legal moves
  on two lines. Both exit 2 with nothing on standard output. The fake follows the real shape, in made-up words.
- The first listing right after a re-trust warns "source `plan` may be out of date: the line ... is not trusted". It
  does not.
- `collide run` printed no `claude published:` line in a temp `HOME` (`published_chip` in `host/src/run_cli.rs`:
  `agent.published` was `None`). Find why, and fix it or say so in the run's text.
- After a daemon restart the first `collide tasks --checkout` of 1,659 tasks took 5.5 s, as long as the cold one (5.7
  s); later ones 0.5 s. Find what the first listing waits for while the store's cache (`ts-0116-wh`) is there.
- A worktree Collide makes has no build of a repo's own tools when they are not committed: the Orchard plan CLI's
  packages and `dist/`. A write queued into a run's worktree then fails (`./plan exited with 1`, a node error). Until
  setup hooks (Stage 7), the failure's Queue item says the repo's plan CLI could not run there; Stage 7 gets a note.

## Tests

- One per fix, red at runtime first. The timing one is a measurement in Built, not a test.

## Notes

- No item for: the real `depends_on` block lists (the fixture has both shapes), the `ID-MAP.md` fourth column's name
  (the adapter reads the first two cells), and the extra folders under `arch/` (ignored).
- Owner decisions: none recorded yet. The recommendations stand until the owner answers.
