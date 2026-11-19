---
id: ts-0146-se
kind: task
status: ready
title: "Board root mid-rebase: the line is the branch git is rebasing"
owners: []
links: []
---

## Goal

During a `git rebase` at the board root `HEAD` is detached. `checkout_of` in `host/src/boards.rs` then names the line
`HEAD`, which is not trusted, so the source `plan` is not started and a write is refused at once (exit 1, "the line
`HEAD` is not trusted, so the source `plan` ... is not started") instead of waiting. The Stage 4b criterion "A write to
the board root during a rebase waits, and the CLI says why. It applies once the rebase ends" fails for a rebase. It
passes for a merge. Found by the agent run of `ts-0121-xp`'s checklist (2026-11-19), step 5b.

## Success criteria

- During a rebase at the root, the line is the branch being rebased (git's `rebase-merge/head-name` or
  `rebase-apply/head-name`); the source stays up and its trust is the line's.
- A write to the root then waits with "a rebase is in progress" and applies after `--abort` or `--continue`.
- What is read for the line during the rebase is the branch's own commit, not the rebase's `HEAD`.
- A detached `HEAD` with no rebase is as today.

## Tests

- Write first and see red at runtime: a daemon test that stops a rebase on a conflict at the root, then writes (today:
  refused), aborts, and sees the write applied.

## Notes

- Recommendation: read `head-name` in `checkout_of` when a rebase is in progress, and read the line's snapshot from the
  branch ref. A bisect detaches too (`BISECT_START` holds the branch); take it in the same change if it is the same few
  lines.
- Owner decisions: none recorded yet. The recommendations stand until the owner answers.
