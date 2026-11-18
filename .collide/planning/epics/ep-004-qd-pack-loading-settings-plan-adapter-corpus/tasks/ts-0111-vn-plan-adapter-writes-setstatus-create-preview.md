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

### Built

- The `plan` adapter writes. `initialize` declares ops `setStatus` and `create` (each with an `args` schema) and
  capabilities `{ watch: false, create: true, collections: true, preview: true }`. `apply` and `preview` are in
  `fixtures/packs/orchard/providers/plan/write.ts`, and nothing in them commits.
  - `run-plan.ts` runs `./plan --root <checkout>/arch ... --format json` with the root as cwd, a time limit and an
    output cap, and kills the child on either.
  - `tree-hash.ts` hashes every file under `arch/` before and after. The files touched are the paths whose hash
    changed.
  - `scan.ts` gains `findTask`, and `files.ts` gains `inBatches` (moved from `scan.ts`).
  - The provider takes `env` and `timeoutMs` options, so a test controls the CLI's environment.
- The fake plan CLI is `fixtures/orchard-arch/plan` (Node, no dependencies, executable). It follows the owner's local
  copy of the Orchard repo as read read-only on 2026-11-16: the flags, the JSON envelope, the exit codes, the
  `--dry-run` report and the epic table refresh. Only shapes were taken. Every message, title and name is made up.
- Tests (all in `fixtures/arch-plan-tests/test/`): `fake-plan.test.ts` (74, the fake itself), `write.test.ts` (65:
  setStatus, create, a misbehaving CLI, preview, the generated schemas, writes in sequence), plus new cases in
  `initialize.test.ts` and `provider-stdio.test.ts` (one end-to-end test with a real node child). `repo.ts` makes a
  temp git repository with an empty global and system git config and a temp home. 213 tests in the folder, all green.
- Seen red at runtime, not at compile time. The adapter side was run against the old `provider.ts`, `scan.ts`,
  `files.ts`, `declaration.ts` and `ids.ts` from HEAD: 63 of 213 failed (44 in `write.test.ts`, 18 in
  `initialize.test.ts`, 1 in `provider-stdio.test.ts`). The bad-call tests and the schema negative check passed there
  because they pass for any provider that errors. The fake was written before its tests, so its red check is a stub
  that exits 0 and prints nothing: 68 of the 74 tests in `fake-plan.test.ts` failed.
- Could not be confirmed in the Orchard copy (it has no installed dependencies, so the CLI was never run, and I did
  not run it): the exact text of every message; whether `set` with the status a task already has is a no-op or an
  error (the fake makes it a no-op that lists no file); the full set of write-gate rules (the fake has three:
  status is declared, the epic exists, `depends_on` resolves); the exit code for a signal-killed run in practice;
  whether `--dry-run` lists a file whose edits change nothing (the fake lists it, and the adapter leaves it out);
  and the real transition table (the fake's is one step at a time, with a reopen and a drop).
- Not run here: the full gates beyond the nine checks of the brief. Results: `cargo build`, `cargo fmt --check`,
  `cargo clippy -D warnings`, `cargo test --workspace`, `pnpm test`, `pnpm typecheck`, `pnpm --filter @collide/app
  build`, `pnpm build:packs` and `pnpm check` all passed. No wire type changed, so `pnpm gen` was not needed.

### Decisions for the owner

1. **Exit codes (differs from the item's text).** The plan CLI exits 2 for an illegal transition and for "done needs
   a review on record", where the item says exit 2 is a provider error. Recommendation: exit 1 and exit 2 are both
   `refused`, with the CLI's message (the first error of the envelope for exit 1, stderr for exit 2). The adapter
   built the call well, and the CLI declined it. Any other exit, a CLI that cannot be run, a timeout, and any failure
   that changed a file are provider errors. Plan text for 0.10 or the Stage 4b bullet: "an exit 1 or 2 of the plan
   CLI with nothing written is `refused`; any other failure is an error reply".
2. **Preview cannot know the version after the write.** A dry run writes nothing to hash, so a previewed item has
   the task's current version (and `""` for a create). The "same `items[]`" criterion holds for key, `from` and `to`,
   not for `version`. Alternative: run the CLI against a scratch copy of `arch/` and hash the result; it is exact but
   copies the tree (about 1,600 files in Orchard) on every preview. Recommendation: keep the current version, and
   take the scratch copy only if a caller needs the written version.
3. **A category is written as the one status that has it.** `categoryStatus` is declared (active writes `doing`,
   review writes `in_review`, and so on), and `setStatus` takes `status` or `category`, not both. A category with no
   status is refused. Recommendation: keep it.
4. **Preview leaves out an edited file whose edits change nothing**, such as `set` to the status a task already has,
   so that the preview lists what the write touches. Recommendation: keep it.
5. **Title rules.** `create` rejects (as an error reply, from the args schema) a title that is empty, over 200
   characters, has a control character or starts with a dash (the CLI would read it as a flag). Recommendation: keep
   the 200 and revisit if a real title is longer.
6. **Write limit.** The CLI may run 120 seconds (`WRITE_TIMEOUT_MS`) and print 8 MiB. Recommendation: keep both; the
   timeout is an option for tests.

- Owner decisions (2026-11-17, at the build): decisions 2 to 6 as recommended, taken on delegation. Decision 1 as
  amended at review: exit 2 is `refused` with stderr (the plan CLI's UsageError, which an illegal transition is, read in
  the copy); exit 1 is `refused` only when its envelope carries the write gate's errors, and otherwise an error reply,
  since the plan CLI's own failure (`Internal error: ...`) also exits 1, with no envelope. Also at review: the command
  comes first and the flags after it (`./plan set <id> status=<s> --root <checkout>/arch --format json`), because the
  plan CLI is oclif 4 and reads its first argument as the command; the fake now refuses a flag before the command with
  exit 2, and with it 27 adapter tests were red until the adapter was fixed. No plan text: the plan states no exit rule
  for the adapter.

### Not done, by design

- `watch`, renames and collisions (`ts-0112-px`); the line lock, root busy and the phase queue (`ts-0114-hr`);
  recording a write in a commit (Stage 7). The SDK answers one request at a time, so writes in this process are
  already in order, and a test pins it with eight calls at once.
- No test reads or runs the Orchard copy or its CLI. The real CLI was never run.
- No IMPLEMENTATION_PLAN.md edit: the text for decision 1 is above.
