---
id: ts-0031-xu
kind: task
status: ready
title: "Planning slice: checkout and commit options"
owners: []
links:
  - { type: blocked-by, to: ts-0015-dp }
  - { type: blocked-by, to: ts-0020-tt }
---

## Goal

Where a write lands and when it is committed. Writes go to the main checkout's shared root by default, even when the CLI runs inside a linked worktree. `--checkout <path>` names another checkout (a `git worktree list` entry), passed to `apply` as `checkout`. Without `--commit` a write leaves an uncommitted file. `--commit` makes a one-file commit of exactly the files `apply` reported, as a stopgap until the `record` hook exists (Stage 8).

## Success criteria

- Run from a linked worktree, `collide task <id> setStatus active` changes the file in the main checkout and leaves the worktree untouched.
- `--checkout <path>` writes into that checkout. A path that is not a checkout of the repo is refused.
- `--commit` commits exactly the files `apply` reported, and an unrelated dirty file in the same root stays uncommitted (`git show --stat`).
- Without `--commit`, `git status` in the checkout shows the file as modified and nothing is committed.
- A `claim` that is refused because an owner is shown writes nothing and commits nothing.

## Tests

Write first: tests over a fixture repo with a linked worktree: default root, `--checkout`, `--commit` beside an unrelated dirty file, no `--commit`, and a refused claim with `--commit`.

## Notes

- IMPLEMENTATION_PLAN.md, 0.10 (the planning slice checks out the main checkout's shared root by default; Committing) and Stage 2: Skeleton, Planning slice.
- Area: `host` (CLI flags, git commit), `packs/plan` (the `checkout` argument).
- Done when the criteria hold.
