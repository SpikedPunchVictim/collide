---
id: ts-0031-xu
kind: task
status: done
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
- Code. `host`: `checkout.rs` (`resolve_checkout`: a path, relative to the working directory, canonicalized and compared with the canonicalized entries of `git worktree list --porcelain`; the errors `NotACheckout`, which lists the real checkouts, and `UnreadableCheckout`), `commit.rs` (`commit_files`), `task_cli.rs` (`--checkout <path>`, `--checkout=<path>` and `--commit` on `task <id> <op>`, `task create` and `task reid`; `Session` carries the resolved checkout), `provider_client.rs` (`list_all` and `get` take the checkout). `packs/plan`: `src/checkout.ts` (`resolveCheckout`, same rule), `write.ts` (`applyOp` writes in the named checkout), `provider.ts` (`list` and `get` read it). Tests: `packs/plan/test/checkout.test.ts` (11), 17 new in `host/tests/task_cli.rs` (the binary over a fixture repo with a linked worktree built in `target/tmp`), 3 in `host/tests/checkout.rs`. Two old tests changed: the provider's refusal of any `checkout` is now a refusal of one that is not a checkout (message contains `checkout`), and the old usage test used `--commit` as its example of an unknown option. No schema or generated file changed (`pnpm gen` left `schemas/` and `packages/pack-sdk/src/generated` byte-identical).
- Where the `--checkout` check lives: both. The host checks first, so a bad path exits 2 with a message that lists the real checkouts, before the provider starts and before anything is read or written. The provider checks again, because it is the one that writes and any protocol client can send `checkout`; its refusal is `refused` (an error reply for `list` and `get`). A relative path is the host's convenience only: the provider takes an absolute one. The main checkout is itself a valid `--checkout`.
- `list`/`get` with a checkout read that checkout (decision beyond "keep reading the main checkout"): the CLI looks the item up and takes its `version` from the checkout it will write to. Reading the main checkout would make every write conflict as soon as the two files differ, and an item that exists only in the worktree could not be found. Without `--checkout` nothing changes: `list` and `get` read the main checkout. `create` resolves `epic=` in the same checkout. `create` and `reid` in another checkout take ids from every ref (shared by all checkouts) and that checkout's working tree; another checkout's uncommitted files are not seen.
- Each checkout has its own lock file (`<checkout>/.collide/planning/.write-lock`) and the same temp, fsync, rename writes; a test holds the main checkout's lock and writes in the worktree.
- `--commit`: after an `ok` that reported files, in the checkout written to: `git --literal-pathspecs add -- <files>` only for files git does not know (new) or that are gone but still in the index (removed), then `git commit -m <message> -- <files>` (a commit with paths takes the working-tree content of those paths and leaves the rest of the index alone). A test has one unrelated file modified, one staged and one untracked: the commit holds only the reported file, and afterwards the first is still ` M`, the second still `M ` and the third `??`. A reported path that was never in git and is no longer on disk (a created file that `reid` then renamed) is dropped from the commit. Hooks run; there is no `--no-verify`. Message: `task <id> <op>: <title>`, `task <id> create: <title>` and `task <new id> reid (was <old id>): <title>`; the text output ends with `  commit  <short hash>  <message>`, `--json` output stays the provider's result alone. `ok` with no files, `conflict` and `refused` commit nothing.
- Exit codes (the ts-0020 table, two meanings added): a `--checkout` that is no checkout of the repository exits 2 (a command-line fault; nothing is written); a failed commit exits 1, after the `ok` text, with git's message (stderr, then stdout, so a hook's output shows) on stderr after `collide: <id> was written, but git commit failed`. The write stands; what `add` staged is unstaged again, so the file is as without `--commit` (a test uses a failing `pre-commit` hook). `--commit` or `--checkout` on `collide task <id>` with no op, or `--checkout` with no path or twice, is exit 2.
- Help: new `--checkout <path>` and `--commit` in the usage lines and a paragraph on where a write goes; the `create` and `reid` paragraphs now say "without --commit".
- Not done, by design: `collide tasks --checkout` (the criteria need only writes); the `record` hook (Stage 8); a commit that adds a Co-Authored-By trailer or takes a `-m` of the user's; committing several writes into one commit; amending; refusing a write while a phase runs in the checkout (0.10, later stage); a lock around `git commit` beyond git's own `index.lock`.
