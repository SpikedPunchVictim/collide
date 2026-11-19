---
id: ts-0139-wq
kind: task
status: ready
title: "Follow-ups: raised-by, Keep and the create rule for a run's agent"
owners: []
links:
  - { type: blocked-by, to: ts-0137-pg }
---

## Goal

What a run finds that is not its task's work becomes an item a person reviews, never only a line in the run's last reply
(`IMPLEMENTATION_PLAN.md` 0.10, Follow-ups; revision 24).

What exists. `ts-0128-bb` reads the actor from the connection and puts the run id on the wire, and `judge_write` in
`host/src/daemon.rs` judges every write by it. `ts-0125-uh` made the private root (`<pack data>/planning`), written from
the main checkout only. Ids are drawn free across both roots. The op name `move` is reserved for Share (0.10), and
nothing implements it. `ts-0133-kg` adds `inverse` to link type declarations, and `ts-0135-cx` the pack's views.

What this adds.
- The `raised-by` link type in the markdown provider: from a follow-up to the item whose run raised it, blocking
  nothing, with `inverse` "raised". Before Stage 9d that item is the run's task: quick and file runs are the only runs
  with one.
- The create rule, in the daemon: a `create` whose actor is a run's agent goes to the board's private root, in the
  folder of the run's task's epic, whatever root it names. The daemon adds `raised-by` to the run's task and sets
  `fields.run` to the run id, written as a `run:` frontmatter key, as the provider writes other fields. A `raised-by` or
  `run` the agent sent is replaced, not trusted. When the run's task is not in the board's planning source, only
  `fields.run` is written.
- Keep: the markdown provider's `move` op for one private item. The item (its folder, for a kind that has one) moves to
  the shared root's folder for the same epic, under the same id, and `record` commits exactly those files when
  `--commit` is given. It is refused for a proposal, which is shared whole (Stage 12), and for an item whose links name
  a private item, naming that item. While the root is busy it waits like any write (exit 11). A run's agent is refused;
  the app and the CLI may. The Share preview, the move points and the handoff note built at Share stay in the
  second-pack stage.
- The built-in view `follow-ups`: open private items with a `raised-by` link, newest first.

## Success criteria

- A `task new` from a run's agent (a process with `COLLIDE_RUN` in its tree) lands in the private root, in its task's
  epic folder, with `raised-by` to the run's task and `run:` set, even when it asked for the shared root or sent another
  `raised-by`.
- A `task new` from a run whose task is not in the board's planning source has `run:` and no `raised-by`.
- The same `task new` from the CLI in a terminal goes where it asks, with nothing added.
- `move` from the CLI moves a private task and a private question to the shared root under the same id, with their links
  intact; with `--commit` the commit holds exactly those files.
- `move` of a proposal, or of an item linked to a private item, is refused with a message naming why, and nothing moves.
- `move` from a run's agent is refused with a message that names who may do it, and nothing moves.
- `collide tasks --view follow-ups --json` lists the open private items with a `raised-by` link, and not a dropped one
  or a shared one.

## Tests

Write these first.

- Daemon tests with a fake process tree for each actor: the create rule, the replaced fields, a run whose task is
  outside the planning source, the CLI's create unchanged.
- Provider tests for `move`: a task, a question, a spike folder; the epic folder it lands in; an id already in the
  shared root, a proposal, and an item linked to a private item (each refused); links kept; the `--commit` file list.
- The view over a fixture; `raised-by` in the link type declarations and the generated schema.
- The existing daemon, provider and CLI tests unedited.

## Notes

- IMPLEMENTATION_PLAN.md: 0.10 (Follow-ups; Share; the planning links); 0.2 (Who is asking); Stage 4c (Follow-ups, its
  success line); the revision 24 entry.
- **Decision for the item, with a recommendation: how a run's create reaches the private root.** A run's agent works in
  a worktree, and `ts-0125-uh` writes the private root from the main checkout only. Recommend that the daemon writes it
  there, since the private root is the board's, not a checkout's. Alternative: hold the create until accept, which loses
  the follow-up when the run is discarded.
- **Decision for the item, with a recommendation: `raised-by` names the run's task**, and the run id goes in
  `fields.run`. Alternative: a link to the run, which is not an item.
- **Decision for the item: Dismiss is `setStatus dropped`** (Stage 9a's review) and leaves the item private. Nothing to
  build for it here.
- If the view language cannot yet say "private" or "has a `raised-by` link" (`ts-0135-cx`), add the fields the view
  needs to its input, additively, and name them under Built.
- Plan… and Spike… runs (Stage 9d) will name a proposal or a spike as the run's item, and the view will leave out an
  item a Plan… proposal includes. Nothing for that here.
- Area: `host/src/daemon.rs` (`judge_write`, `task_apply`), `host/src/task_daemon.rs`, `packs/plan/src/` (links, a
  `move` op, views), the generated schemas and TypeScript if a wire shape changes, `host/tests/`, `packs/plan/test/`.
- The test rule: no test touches the real home, real launchd or a real daemon. Each test that runs the `collide` binary
  clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as
  `host/tests/common/mod.rs` `TempRoot` does. Settings files are fixtures, never the owner's own `settings.json`. A temp
  repository sets `GIT_CONFIG_GLOBAL` and `GIT_CONFIG_SYSTEM` to empty files.
- Stage 4c item, added by revision 24 (2026-11-18). Built after `ts-0137-pg` and before `ts-0140-gk`; `ts-0138-pn` waits
  for both.
- Nothing here runs a real `claude`. No dependency is added.
- Owner decisions: none recorded yet. The recommendations stand on the owner's delegation.

### Built

- **`raised-by` link type.** `packs/plan/src/item.ts` `LINK_TYPES` and `RAISED_BY`: out, not blocking, inverse "raised".
  It is not in `STORED_LINKS` of `write.ts`, so the `link` op still refuses it and only a create writes it.
- **Create args `raisedBy` and `run`.** `packs/plan/src/create.ts` `checkCreate` (the id pattern; 1 to 200 characters of
  text with no control character) and `fileOf` (a `run: "<id>"` line right after `status:`, and one `raised-by` link).
  One atomic file write carries both. `provider.ts` declares them in the `create` args.
- **Keep, the `move` op.** New `packs/plan/src/move.ts` (`moveLocked`, `moveWall`, `checkMove`), reached from
  `write.ts` `applyOp`. It locks the shared root and then the private one, copies the file (a spike: every file of its
  folder) to the shared epic folder under the same id, reads it back, then removes the private copy. The reply lists the
  shared paths, which is what `--commit` adds, and one `items[]` entry with `from` equal to `to`. Refused: not private
  or in both roots; a proposal; an epic; a link to a private item (named); an id that a ref or the working tree has; an
  epic with no shared folder; a destination that is there; a spike folder with a link in it. An agent actor is refused
  by name (the second wall).
- **View `follow-ups`.** `packs/plan/pack.yaml`: open private items with a `raised-by` link, sorted by `id`, descending.
- **View input, additive.** `core/src/view_expr.rs`: `link_types` (the type of each link, once, blocking or not) in
  `TASK_NAMES`, `TaskVars`, `check_task_caps` and `link_types()`. It is not a wire type; `pnpm gen` changed nothing.
- **`move` is declarable.** `core/src/provider.rs` `RESERVED_OPS` is now empty. See decision 1.
- **The create rule and the move wall.** New `host/src/follow_ups.rs` (`shape_create`, pure, 4 unit tests). In
  `host/src/daemon.rs`: `shape_run_agent_create` and `run_task_of` (called first in `judged_write`, so `task.create`
  and `task.apply` both pass it, and a queued write keeps the shaped args), and a refusal in `judge_write` for a run's
  agent and op `move` on the planning source (`NOT_PERMITTED`, naming the run and "the app or the CLI").
- **Tests.**
  - `packs/plan/test/follow-ups.test.ts` (20): the create args, the `link` op refusing `raised-by`, and `move` for a
    task, a question, a spike folder, the epic folder it lands in, each refusal, links kept, the file list, the agent
    wall, version and args.
  - `host/tests/daemon_follow_ups.rs` (7): an agent create via `task.apply` and via `task.create`, a run whose task is
    scratch, missing or unknown, the CLI create unchanged, Keep refused to an agent, Keep from the CLI with `--commit`
    holding exactly one file (an unrelated file stays untracked), and the view.
  - `core/tests/view_expr.rs` (+3), `core/tests/plan_roles_and_views.rs` (the view), `core/tests/provider_protocol.rs`.
- **Existing tests edited, all because a declared list grew.** The link declarations (`raised-by`) in
  `packs/plan/test/link-types.test.ts`, `feature-links.test.ts` and `host/tests/daemon_plan_feature_links.rs`; the op
  lists (`move`) in `packs/plan/test/write.test.ts` and `feature-ops.test.ts`; the view lists in
  `core/tests/plan_roles_and_views.rs`; and the `move` reservation in `core/tests/provider_protocol.rs` and
  `host/tests/provider_supervisor.rs` with its fixture `fake_provider.py` (mode `bad-initialize` is now
  `declares-move`). No test was removed without a replacement. The supervisor's refusal of a reserved op has no
  test now, since no name is reserved.
- **Seen red at runtime.** All 16 failing provider tests failed on the old code with `unknown op "move"` or an unknown
  argument. Six of the seven daemon tests failed on the old daemon (create stayed shared, no `run`, no `raised-by`,
  Keep reached the provider's wall, not the daemon's); the CLI create passed, as it should. The `view_expr` tests
  failed with `link_types` stubbed empty. Mutations, each red and restored: the daemon `raisedBy` drop (1), the
  `root` rewrite (6), the move refusal in `judge_write` (1).

### Decisions for the owner

1. **`move` was reserved, and the reservation is lifted.** `check_initialize` refused any provider that declares `move`,
   so the provider could not start. Recommend: `RESERVED_OPS` empty, the name now being Keep's (and later Share's) op,
   as the plan says ("Keep uses it for one item from Stage 4c"). This edited the tests that pinned the refusal (see
   Built). Alternative: name Keep `keep`, with no core change and no test edit; the CLI would read
   `collide task <id> keep`, and Share's later `move` would be a second op.
2. **`raisedBy` and `run` are create args**, not a create followed by a link and a field write. One atomic write. A
   person's own `task.apply` create may send them (the rule shapes only a run's agent). Alternative: a daemon-only path.
3. **`move` takes one item at a time**, and refuses a proposal ("shared whole") and an epic. Alternative: move an
   epic's `epic.md` alone, or a proposal with its folder.
4. **Copy, read back, then delete**, since the pack data folder and the repository may be on different filesystems. A
   spike's `out/` folder is copied with the rest if it is there. Stage 9d decides what a spike keeps.
5. **The epic of a run's follow-up is the epic of its task**, whatever it asked; with no task found it keeps its own.
6. **The run's task is the first task its launch recorded**, when it is a planning-source key. Today only quick runs
   record one, and theirs are scratch tasks, so a quick run's follow-ups carry `run:` and no `raised-by`.
7. **The view sorts by `id` descending**, since the sort `by` names a surface field. `collide tasks --view` lists in key
   order and applies no layout sort, so "newest first" is the app's. Recommend the CLI honour `layout.sort` later.
8. **Cap on `link_types`.** A task with more than 1,000 distinct link types, or a type name over 4,096 bytes, is now an
   invalid item. No real task is near it.

Plan text to propose:
- 0.10, Fields: add `link_types` ("the type of each link the task has, once each, blocking or not") to the names a view
  may use.
- 0.10 and the revision 24 entry: "`move` is declarable by a provider; Keep is its first use." Drop "reserved".
- 0.2, Follow-ups, `ts-0140-gk`'s guide and the workflow prompts: the agent files a follow-up with `collide task create`
  (the planning-source create); `collide task new <prompt>` makes a scratch task and the rule does not touch it.

### Not done, by design

- `task.preview` is not shaped for a run's agent; the plan provider declares no `preview`.
- Keep of a proposal, an epic, or an item that links a private item: the person keeps the linked item first (Stage 9a's
  review section), and a proposal is shared whole by Share (Stage 9d).
- Follow-ups from a Plan… or Spike… run name that run's proposal or spike (Stage 9d); here it is the run's task.
- A CLI that honours the view's sort: no item yet.
- `ts-0140-gk` must say `task create`, not `task new` (see the last proposal above). `ts-0141-sn`, `ts-0142-ua`,
  `ts-0144-eh` to `ts-0150-dj` and `ts-0151-fk` are not touched, and none gets trivial.

### At review

- The success criteria's `task new` is read as `task create`: `task new` makes a scratch task in Collide's store,
  outside the planning source. Noted in `ts-0140-gk`.
- Changed: the daemon refuses `move` to a run's agent on every source, not only the planning source, since core no
  longer reserves the name and any provider may declare it. No test for a second source: this rig loads the planning
  source only; the planning test covers the refusal.
- Decision 1 (`move` unreserved) taken; plan 0.10's op-name line rewritten to match.
- Mutations, each red then restored: the forged `raisedBy` kept (a unit test and a daemon test), the private-link
  refusal (vitest), the proposal refusal (vitest), the view's private condition (the daemon view test), the daemon's
  `move` refusal (the daemon Keep test).
- Not checked: a private spike folder with files the repository ignores (an `out/`); `move` copies the whole folder and
  `--commit` names every file. Spike runs (Stage 9d) make such folders.
