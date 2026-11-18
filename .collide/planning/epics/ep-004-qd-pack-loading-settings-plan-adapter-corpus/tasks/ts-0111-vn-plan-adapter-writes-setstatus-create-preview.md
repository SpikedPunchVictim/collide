---
id: ts-0111-vn
kind: task
status: ready
title: "Plan adapter writes: setStatus, create, preview and the fake plan CLI"
owners: []
links:
  - { type: blocked-by, to: ts-0110-ra }
---

## Goal

The write side of the `plan` adapter: `setStatus` (by status or by category) and `create`, run through the plan CLI,
with the files each write touched, and `preview` through the CLI's `--dry-run`. The plan CLI is the owner's and lives in
the Orchard repo, so the tests run a fake one that keeps the same rules.

What exists. After `ts-0110-ra`, `fixtures/packs/orchard/providers/plan.ts` reads one checkout (`initialize`, `list`,
`get`) and declares no ops. `packs/plan/src/{write,lock,atomic,op-util}.ts` show how the markdown provider writes (an
in-root lock, temp file and rename, `refused` and `conflict` answers); it writes files itself, and this adapter does
not. The protocol's `apply` and `preview` outcomes are in `core/src/provider.rs` (`ApplyResult`, `PreviewResult`,
`Effect`). There is no fake plan CLI.

What this adds.
- A fake plan CLI for tests: one executable `plan`, in Node, that takes `--root <dir>` (an `arch/` folder), `set`, `new
  task`, `--dry-run` and `--format json`. It keeps plan's rules that matter: a legal-transition table that refuses with
  a message and exit 1, a usage error with exit 2, and a regenerated epic table in the epic file after a write. It is a
  test fixture and not the plan CLI.
- `initialize` gains the ops (each with its argument schema), `capabilities.create` and `capabilities.preview`.
- `apply`: `setStatus` with `status` or `category` (a category goes through `categoryStatus`), `create` with the
  arguments the fixture's `new task` takes. Each runs `./plan --root <checkout>/arch ...` with the cwd the `root` of
  `initialize`, one at a time for each checkout.
- The files a write touched, as paths under the checkout, found by hashing the checkout's `arch/` before and after.
- `refused` with the CLI's own message, `conflict` with the current task when the caller's `version` is old.
- `preview`: the same arguments with `--dry-run`, nothing written, the same three outcomes.

## Success criteria

- A legal `setStatus` returns `ok` and reports exactly two files as touched: the task's file and its epic file (the
  regenerated table). Nothing else under `arch/` is listed, and an unrelated uncommitted edit of another file there is
  left as it was and is not listed.
- `setStatus` with `category: active` writes `doing`. Each category writes the status `categoryStatus` names, and a
  category with no status to write is refused with a message.
- `setStatus` with an illegal transition returns `refused` with the plan CLI's message, and writes nothing.
- A usage error of the CLI (exit 2) is a provider error, not a `refused`: it means the adapter built a bad call.
- A write with an old `version` returns `conflict` with the current task and writes nothing.
- `create` returns `ok` with the new task, the id the CLI allocated, and the files touched (the new file and its epic).
- Nothing commits: `HEAD` is the same after every write, and the files are left uncommitted.
- `preview` returns the same outcome and the same `items[]` and files as the write would, and the checkout is
  byte-identical afterwards.
- Two writes to one checkout at once run one after the other, and each answer is true to what is on disk.
- The result of `apply` and `preview` validates against the generated schemas.

## Tests

Write these first. The fixture is a temp git repository made by a helper: the fixture `arch/` tree, the fake `plan` at
its root, one commit.

- Legal `setStatus` by status and by each category; the files touched; the epic table regenerated; `HEAD` unchanged.
- An unrelated edit under `arch/` before the write: not touched, not listed, still there after.
- An illegal transition refused with the CLI's message; the tree unchanged.
- A usage error (a call the fake rejects with exit 2) is an RPC error.
- A stale `version`: `conflict`, and the current task in the answer.
- `create`: the new file, the allocated id, the regenerated table, the files listed.
- `preview` against the same cases as the writes, with a content hash of the tree before and after.
- Concurrency: eight `setStatus` calls on one checkout at once, all answers consistent with the final file.
- One end-to-end test with a real `node` child on stdio: `initialize`, `apply`, `preview`.
- The fake CLI's own tests: its table of legal moves, its table regeneration, its exit codes.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 4b: the `plan` adapter's operations (`setStatus` by status or category and `create`; run through a line root's
    `./plan --root <checkout>/arch`; return the files they touched; queue while a phase runs; never commit) and "The
    plan adapter declares `preview`, using the plan CLI's `--dry-run` (S11)".
  - 0.10: the `plan` row of Sources; Protocol (`apply`, `preview`); Writes are operations; Ref-scoped sources
    (committing; writes leave uncommitted changes; the planning slice). `docs/research/planning.md` and S11
    (`sp-0008-bk-derived-point-coverage`): `--dry-run` works on every mutating command.
- "Queue while a phase runs in that checkout" is in this list in the plan, and the adapter cannot do it: it does not
  know what phases run. It is the host's, in `ts-0114-hr`. This item answers the call it is given.
- **Owner's choice, with a recommendation: how the fake CLI learns the real one.** The fake must match the real plan CLI
  in what the adapter reads: its flags, its JSON envelope (`schema_version: 1`), its exit codes (exit 2 is a usage
  error, R1) and what `--dry-run` prints. If the owner gave the sample asked for in `ts-0110-ra`, build the fake from
  it. If not, the fake follows the plan's text, and `ts-0121-xp` is where a mismatch shows. Alternative: have the owner
  run the adapter against a copy of the Orchard repo's plan CLI now. Not recommended: this build should not touch that
  repo.
- **Decision for the item, with a recommendation: how files touched are found.** Hash every file under the checkout's
  `arch/` before and after, and report the paths whose hash changed. It does not trust what the CLI says it wrote, and
  an unrelated edit is never listed. Alternative: read the CLI's own list from its JSON. Use it only to cross-check, and
  fail a write whose two lists differ.
- **Decision for the item: exit codes.** Exit 2 is a usage error and so a provider error. Any other non-zero exit with
  nothing written is `refused`, with the message from the JSON envelope's error, else the first line of stderr.
- **Decision for the item: the working directory.** `./plan` runs with `root` as its cwd (0.4: repo code runs from a
  line root, never a workspace), and `--root` names the checkout. A non-default line's own root is a lines question
  (Stage 6a); the adapter takes whatever `root` it is started with.
- **Decision for the item: the dry-run's files.** If `--dry-run` does not list files, `preview` reports the task's file
  and its epic's file, which is what a write touches. Say so in a comment, with the sample as the test.
- Out of scope: renames, collisions, `watch` (`ts-0112-px`); the line lock, root busy and the phase queue
  (`ts-0114-hr`); recording a write in a commit (Stage 7, `record`).
- Area: `fixtures/packs/orchard/providers/`, `fixtures/orchard-arch/plan` (the fake CLI) and the fixture-repo helper,
  `fixtures/arch-plan-tests/`.
- The test rule: no test touches the real home, real launchd or a real daemon. A test that runs the `collide` binary
  clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as
  `host/tests/common/mod.rs` `TempRoot` does. This item's tests are Node: each makes its own temp git repository, sets
  `GIT_CONFIG_GLOBAL` and `GIT_CONFIG_SYSTEM` to empty files and a temp `HOME`, and runs no daemon. Settings files are
  fixtures, never the owner's own `settings.json`. No test reads or runs the Orchard repo's plan CLI.
- Stage 4b item (revision 23 scope, split 2026-11-17). Items of Stage 4b run one at a time, in id order, because they
  share `core`, `host` and the generated files.
- Nothing here runs a real `claude`.
- Owner decisions (2026-11-17, at the split): the fake plan CLI follows the real one as read from the owner's local copy
  of the Orchard repo, under the rules of `ts-0110-ra`. The rest: the recommendations above.
