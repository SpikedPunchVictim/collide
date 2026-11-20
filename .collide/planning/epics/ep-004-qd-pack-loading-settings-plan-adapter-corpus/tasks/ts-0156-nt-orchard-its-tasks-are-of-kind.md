---
id: ts-0156-nt
kind: task
status: done
title: "Orchard: its tasks are of kind task"
owners: []
links:
  - { type: blocked-by, to: ts-0151-fk }
---

## Goal

An Orchard task has no kind today (`ts-0137-pg`): the provider declares `adr`, `spike` and `question` only. So `collide
tasks --kind task` is refused on a board whose only source is Orchard's, and on a mixed board it lists Collide's tasks
and not Orchard's (`ts-0150-dj`, Decision 1). The owner wants Orchard's tasks to count as tasks.

## Success criteria

- The Orchard plan provider declares the kind `task`, with the statuses it declares for tasks today, and every task it
  lists or gets carries `kind: task`. ADRs, spikes and questions are unchanged.
- `collide tasks --kind task` on a board whose only source is Orchard's lists its tasks and exits 0; on a board with
  Collide's plan too, it lists the tasks of both.
- The listing and the text of `task <key>` show `task` for an Orchard task.
- What reads a task's kind still works: the `ready` view, `decisions-needed`, the roles check (`ts-0135-cx`), `create`,
  `setStatus` by status and by category, and the transitions of a task.

## Tests

- Write first and see red at runtime: the provider test that a listed task has kind `task`, and a daemon test that
  `--kind task` lists an Orchard task on an Orchard-only board.

## Notes

- Owner 2026-11-19: yes, Orchard's tasks count as tasks (answer to `ts-0150-dj` Decision 1).
- Recommendation: the provider declares the kind, since only the source knows what its items are. Alternative: Collide
  reads an item with no kind as a task; rejected, since it says that of every kind-less item of every source.
- The plan's 4c text says of the Orchard provider "(a task has no kind)"; it changes at review.
- Built after `ts-0151-fk`, which changes the same provider.
- Owner decisions: the owner's answer above. The recommendation on how stands until the owner answers.

### What reads a kind, and whether it changes

Found before the build, by reading every use of a task's kind and of a kind-less item.

- Orchard pack:
  - `INITIALIZE_RESULT` in `declaration.ts` declares the kinds: changes (`task` first, with the source's statuses).
  - `toTask` in `task.ts` builds every listed and gotten task: changes (`kind: task`).
  - `setStatus` in `write.ts` refused any item that had a kind, since only a task had none: changes (it refuses any
    kind but `task`). `create` has no kind to read.
  - `kinds.ts` gains `TASK` and exports `declareKind`; `scan.ts` uses `TASK` for its candidate kind, a rename only.
  - `diffOf` in `changes.ts`, `watcher.ts`, `epic.ts`, `collections.ts`, `queue.ts`, `ids.ts`: no kind read, or only
    `question`; unchanged. `pack.yaml`: the `spikes` view tests `spike`, `decisions-needed` tests a collection, and the
    `roles` default names no `task`; unchanged.
- Core:
  - `SourceState::category_of` finds the task's kind and reads its statuses, which are the source's own: same answer.
  - `SourceState::kind_of` (list filter, the views' `kind`, the proposal check) now meets a kind it can name; its
    sole-kind rule never applied to Orchard (three kinds before, four now).
  - `declared_kinds` now includes `task`; the `--kind` refusal and its text follow it with no code change.
  - `diff_of` names `kind`: see the watcher paragraph below.
  - `check_roles` (`roles.rs`), `view_expr.rs`, `planning_kinds`, `validate` of the reserved ops in `provider.rs`: no
    change. A kind's `transitions` are read nowhere in core or host (only the declaration carries them).
- Daemon and CLI:
  - `task_list` (the unknown-kind refusal): unchanged code, now accepts `task`. The listing's kind column
    (`task_daemon.rs`) and `show_text` (`task_view.rs`) print the task's own kind, `-` before and `task` now.
  - `judge_write` (`daemon.rs`) and `person_only_request` (`auth.rs`) act on `proposal` and `question` only: unchanged.
  - `epic=` in `task create` (`task_daemon.rs`) says "which is a task, not an epic" for an Orchard task; it said "an
    item". Wording only.
- Generated pack SDK types: `KindDecl` and `TaskInput.kind` are strings already; no wire change, so no `pnpm gen`.

### The watcher

The first `watch` sends nothing, for any kind: "what is there now is not news; core lists at start", and the existing
`answers with an empty result, sends nothing for what is already there` test covers it, now over tasks that carry a
kind. `diffOf` does not name `kind`, and a file's kind is its folder, so a later change never needs it: a kind change
is not a `diff` entry, and `ts-0157-hd`'s fetch is not set off by it.

One thing does send events. A daemon restarted with a corpus cache written before this change holds Orchard tasks with
no kind. Its startup reconcile (`reconcile_by_source` in core) sees each differ and records one `task.changed`, cause
`startup`, with `kind` in the diff, once per task. That is the event log's one-time record of the change, and wanted: it
is what `ts-0116-wh` does for any task that differs from the cache.

## Built

- `fixtures/packs/orchard/providers/plan/kinds.ts`: `TASK`; `declare` is exported as `declareKind`.
- `declaration.ts`: `INITIALIZE_RESULT.kinds` is `declareKind(TASK, STATUSES…)` then the three others.
- `task.ts`: `toTask` sets `kind: TASK`. `write.ts`: the `setStatus` guard is `kind !== TASK`. `scan.ts`: `TASK`.
- Tests (all changed or added; the existing ones that said "no kind" now say `task`):
  - `kinds.test.ts`: `task, adr, spike and question, each with its own statuses and their categories`; `every item
    names its kind, and a task names task`; `the tasks of the basic tree list as they did, each of kind task`; `get`
    over `T-03-001` carries `task`.
  - `write.test.ts`: the task of the `setStatus` result and of the `create` result carry `kind: task`.
  - `watch.test.ts`: a new file's diff does not name `kind`.
  - `host/tests/daemon_reference_kinds.rs`: `kind_task_lists_the_packs_task_on_a_board_whose_only_source_is_the_pack`,
    `a_task_gets_with_the_kind_task_and_the_other_kinds_are_still_listed_by_their_own_name`.
- Red at runtime, against the code before the change (no source edit yet): the four `kinds.test.ts` tests above, the
  two `write.test.ts` results, and both daemon tests (`unknown kind task; the board's sources declare: adr, spike,
  question`; and the task's kind `None`). The watch assertion is green before and after, by design: it pins that the
  diff stays free of `kind`.
- Mutation, red then restored: the old `setStatus` guard (`kind !== undefined`) turns 35 `write.test.ts` tests red,
  every write on a task refused as "a task: the plan CLI changes these".

### Decisions for the owner

1. The task's kind is declared in `declaration.ts`, not in `kinds.ts`, since its statuses are the source's `STATUSES`
   and `kinds.ts` is imported by `declaration.ts` (a cycle otherwise). Alternative: a second copy of the six statuses
   in `kinds.ts`. Recommendation: as built; one list of task statuses.
2. `task` is the first kind declared, ahead of `adr`. Order shows in `declared_kinds` and so in the `--kind` refusal
   text. Alternative: last. Recommendation: as built; the common kind first.
3. The task's kind declaration has empty `ops`, `transitions` and `roles`, as the other three do: it has the source's
   `setStatus` and `create`. Alternative: put the two ops under the kind. Recommendation: as built; nothing reads them.
4. The watcher does not add `kind` to `diffOf`. Alternative: add it, so a kind change names itself. Recommendation: as
   built; a file cannot change its kind, and adding it would make the daemon fetch a task for nothing.
5. Proposed plan text (Stage 4c, Planning bullet; not applied). Replace "(a task has no kind)" so the sentence reads:
   "The Orchard pack's plan provider reads tasks, ADRs, spikes and questions, as the kinds `task`, `adr`, `spike` and
   `question`, and puts the questions that `plan status --format json` queues in the collection `plan:queue`, which
   its `decisions-needed` view lists; a write on the last three is refused, since the plan CLI changes them
   (`ts-0137-pg`, `ts-0156-nt`)."

### Not done, by design

- `ts-0155-wb` (restart listing) and `ts-0158-ow` (a `get` rescans the checkout): not changed and not made trivial.
  This item changes what a listed task carries, not when it is listed or fetched.
- The Orchard tasks' kind in a plan CLI envelope (`kind: task` in `data`) is the CLI's own and is held by
  `ts-0153-kp`; the shape files are untouched.

### At review

- Reviewer mutations, each red then restored: `toTask` without `kind` (39 TS tests and both daemon tests); no `task`
  kind declared (the declaration test and the `--kind task` daemon test); the task kind with no statuses and the task
  kind last (the declaration test each).
- The mixed-board half of the criteria (Collide's plan and Orchard's on one board, `--kind task` lists both) has no
  daemon test: no rig puts both sources on one board. It rests on `kind_of` reading each item's own kind; the owner
  check `ts-0138-pn` sees it on a real board.
- Decisions 1 to 4 taken as recommended (owner delegation). Decision 5's text applied to the plan's 4c Planning bullet.
