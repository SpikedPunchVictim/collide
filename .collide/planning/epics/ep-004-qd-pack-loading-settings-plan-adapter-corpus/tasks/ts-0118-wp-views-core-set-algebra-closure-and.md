---
id: ts-0118-wp
kind: task
status: done
title: "Views core: set algebra, closure and error counts"
owners: []
links:
  - { type: blocked-by, to: ts-0117-gu }
---

## Goal

Evaluate a view over the corpus, as a pure `core` function: its membership as `(where ∪ include ∪ closure) − exclude`,
the closure with its steps, and the counts of what went wrong on it, so that one view's failure never hides another's.

What exists. `core/src/view.rs` has `View`, `Closure` (`goal`, `follow: depends_on`), `Layout` and `check_view`. After
`ts-0115-ug` the corpus answers what a board holds, links included. After `ts-0117-gu`, `core/src/view_expr.rs` compiles
an expression, estimates it, binds a task and runs a pass under the budgets. Nothing combines these into a view's
members, and nothing walks a closure.

What this adds, in `core/src/view_eval.rs` (or a module of that name):
- `compile_view(view)`: `check_view`, then the expression's compile and estimate, as one result for one view. A view
  that fails is an error on that view and nothing else.
- `evaluate_view(compiled, corpus, budgets)`: the members, in a fixed order, and the counts.
- The set algebra: `where` over each task, `include` and `exclude` as task keys, and the closure.
- The closure: from the goal, along `depends_on` links, through open tasks only (a task that is not `done` or
  `dropped`), with the goal itself always in. It returns the set and its steps: the closure's tasks in layers, a task
  after everything it depends on in the set.
- The counts for the view: tasks that raised an evaluation error, tasks that ran out of their step budget, whether the
  pass ran out of its budget, keys of `include` or `exclude` that match nothing, a closure goal that matches nothing,
  and the corpus's invalid items.

## Success criteria

- Membership is `(where ∪ include ∪ closure) − exclude`: a task in `exclude` is out even when `where` or the closure has
  it, and a task in `include` is in even when `where` does not match.
- A view with only `include`, only `where`, only `closure`, and all three, behave as the algebra says.
- A missing source field: `has(fields.x)` is false for a task without it and the view works. A read of `fields.x`
  without `has` excludes the task and counts one error on the view, with the reason.
- The closure of a goal walks open tasks only. A done dependency is not in the set and is not walked through, so the
  tasks that only a done task reaches are left out (R2's three stranded tasks, in a fixture). The goal is in the set
  even when it is done.
- A closure over a dependency cycle ends, has each task once, and reports the cycle: the cycle's tasks form the last
  step and are marked.
- The steps are layers: the first holds the tasks with no open dependency in the set, and each later one holds the tasks
  whose dependencies are all in earlier steps, in key order inside a step.
- A view whose `where` does not compile, is over 1 KB, names a corpus variable, or is too costly by the estimate has an
  error and no members, and every other view still evaluates.
- A runaway `where` on one hostile task is counted for that task and the other tasks are still tested. A pass that runs
  out of its budget says so and counts the tasks it did not reach.
- `include` and `exclude` keys that match nothing are counted on the view (a typo shows). The closure goal that matches
  nothing is an error on the view.
- The corpus's invalid items count on every view (see Notes).
- `"plan:E-07" in collections` returns exactly the tasks whose collections hold it, in a corpus built from fixture data
  (the stage's collection criterion, in fixture form).
- The same inputs give the same members in the same order every time.

## Tests

Write these first (the stage's CEL view tests that are about views).

- Set algebra: each part alone, all three, exclude over the others, overlaps.
- The missing source field, with and without `has`.
- The closure: a chain, a diamond, a done node in the middle (the stranded tasks), a goal that is done, a goal that
  matches nothing, a dependency on a key the corpus lacks, a cycle of two and of three.
- The steps of a diamond and of a chain, and of a closure with a cycle.
- Several views on one corpus, one broken for each reason: the others are unaffected.
- A hostile task and a pass over its budget, through the view.
- Counts: errors, budget, unresolved keys, invalid items.
- Collections: `"plan:E-07" in collections`, and `"mvp" in labels`, on fixture data.
- A fixture of the shape of the example view in `fixtures/views/edge-mvp.yaml` end to end.
- Order: shuffle the corpus's input and get the same members.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 4b: Views ("the set algebra, normalized fields, closure per the prototype board's generator, and per-view
    error counts"); Success Criteria: the MVP view's closure for T-01-042 and `"plan:E-07" in collections`; Tests: set
    algebra, a missing source field, a closure over a dependency cycle.
  - 0.10: Views (membership; fields; `closure` "is the prototype board's run order"; "walks open tasks only, and it
    handles the goal the same way"; R2: walking every path adds three stranded tasks). 0.9: the CEL limits.
- **Decision for the item, with a recommendation: what "the same steps" and "the goal the same way" mean.** The plan
  says the closure's set and steps are identical to the prototype board's generator's, and defines neither "steps" nor
  the goal's handling. Nothing in this repo shows that generator. Build from the plan's text: open-only, the goal always
  in, steps as layers. The owner compares the result with the generator's output at one commit in `ts-0121-xp`, and the
  differences, if any, become a fix. Alternative: wait for the owner to give the generator's rules. That stops the stage
  on a sentence, and a fixture of the owner's would have the same effect later.
- **Decision for the item, with a recommendation: invalid items count on every view.** An unreadable file has no fields
  to match a `where`, so no view can say it is a member or not. Recommend counting it on each view, so a broken file is
  never hidden by a narrow view. Alternative: count it only on views with no `where` and no `include`. That hides it
  from the views that matter to a person working on one epic.
- **Decision for the item: `follow`.** Only `depends_on`, the link type the Orchard adapter gives `depends_on` entries
  (`ts-0110-ra`). `blocked-by`, which the markdown provider uses, is not followed: `Follow` has one variant and 4c may
  add another.
- **Decision for the item: order.** Members in the corpus's order (source order, then key), so a board's cards do not
  jump between loads. The steps' order is in the criterion.
- **Decision for the item: keys.** `include`, `exclude` and the closure goal are task keys (`plan:T-01-042`). A bare id
  is an unmatched key and counted, not guessed.
- Out of scope: reading view files and the daemon methods (`ts-0119-tj`); `layout` defaults (`groupBy`) and anything the
  board draws (Stage 6); launching from a view (Stage 11); `queue: true` lanes (4c and 6).
- Area: `core/src/view_eval.rs`, `core/src/view.rs`, `core/src/lib.rs`, `core/tests/`, `fixtures/views/`.
- The test rule: no test touches the real home, real launchd or a real daemon. A test that runs the `collide` binary
  clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as
  `host/tests/common/mod.rs` `TempRoot` does. This item's tests are pure `core` tests and need neither. Settings files
  are fixtures, never the owner's own `settings.json`.
- Stage 4b item (revision 23 scope, split 2026-11-17). Items of Stage 4b run one at a time, in id order, because they
  share `core`, `host` and the generated files.
- Nothing here runs a real `claude`.
- Owner decisions (2026-11-17, at the split): the recommendations above, on delegation.

### Built

A pure `core` module. No I/O, no daemon, no store, no new crate or package, no `Cargo.toml` change, no generated file
(no wire type changed).

- `core/src/view_eval.rs` (new, in `core/src/lib.rs`).
  - `compile_view(view, board_tasks, budgets)`: `check_view`, then `compile` and `check_cost` of the `where`, as one
    `Result<CompiledView, ViewFailure>` for one view. `ViewFailure` is `Check(Vec<ViewError>)` (every reason) or
    `Expr(ExprError)`. `CompiledView::estimate()` gives the estimate (for the `matches` note), `None` with no `where`.
  - `evaluate_view(compiled, corpus, budgets)` gives `ViewResult { members, steps, cycle, counts }`. `members` are
    keys, in the corpus's order, each once. `ViewCounts` holds `errors` (by reason, with `error_count()`),
    `over_task_budget`, `pass_budget_exhausted`, `not_reached`, `unmatched_include`, `unmatched_exclude`,
    `goal_unmatched` and `invalid_items`.
  - The walk (`walk`) follows `Follow::DependsOn` by `view_expr::dependencies`, the function behind `deps`, through
    open tasks (not `done`, not `dropped`), from the goal. The layers (`layers`) are Kahn layers in key order.
- `core/src/view_expr.rs`: `dependencies` is public, so a view's `deps` and its closure read one rule.
- `core/src/corpus.rs`: `resolve_key`, `resolve_target` and `kind_of` are public (`kind_of` as `ts-0117-gu` asked).
- Tests: `core/tests/view_eval.rs` (49): set algebra (each part alone, all three, exclude over the others, overlaps, a
  key at two paths), missing fields with and without `has`, the closure (chain, diamond, a done node in the middle
  with R2's three stranded tasks, done and dropped dependencies, done goal, unmatched and bare-id goal, a missing
  dependency, cycles of one, two and three, a task waiting on a cycle, both link types, `blocking: false`, another
  source), the steps, several views with one broken for each reason, a hostile task, a pass over budget, the counts,
  collections and labels, the example view `fixtures/views/edge-mvp.yaml` end to end, order and shuffled listings.
- **Seen red at runtime**, against a stub of the old behavior (`compile_view` accepts every view, `evaluate_view` gives
  nothing): 42 of the 49 failed on wrong members, steps and counts. The 7 that passed on the stub are vacuous ones
  (no part, a clean corpus, a goal that is found, a finished pass, a view with no cycle, the same inputs twice, and the
  first draft of the `deps` test, which I rewrote). Against the finished code, one mutation each:
  - every task counted open: 5 failed (`a_done_dependency_is_not_in_the_set_...`, the dropped one, both done-goal
    tests, the example view).
  - `depends_on` links only: `every_dependency_link_is_followed_...` and
    `the_closure_and_the_deps_variable_read_the_same_dependencies`.
  - steps not cleared of excluded tasks: `the_closure_is_computed_before_exclude_...`.
  - every stalled task named a cycle task: `a_cycle_of_three_below_a_task_...`.
  - unmatched `include` not kept: `include_and_exclude_keys_that_match_nothing_...`.
  - invalid items not counted: `the_corpus_invalid_items_count_on_every_view_...`.
- Checks: `cargo build --workspace`, `cargo clippy --workspace --all-targets -- -D warnings` and
  `cargo fmt --all --check` clean; `cargo test --workspace --no-fail-fast` ran 4,326 tests, one failed under load
  (`a_task_apply_write_is_one_event_...` in `host/tests/daemon_corpus.rs`, not touched here) and passed alone;
  `pnpm test`, `pnpm typecheck`, `pnpm --filter @collide/app build`, `pnpm build:packs` and `pnpm check` passed.

### Decisions for the owner

- **`follow: depends_on` walks `DEPENDENCY_LINKS`, changed at the brief (2026-11-18).** `blocked-by` and `depends_on`,
  skipping a link marked `blocking: false`: the set the CEL variable `deps` holds, from the same function. `ts-0117-gu`
  landed after the split, and the plan provider writes `blocked-by`, so a `depends_on`-only walk finds nothing on
  Collide's own board, and a view's `deps` and its closure would disagree. Alternative, the item's original: only
  `depends_on`. Recommend as built. Proposed plan text for 0.10, after the `closure` bullet: "`follow: depends_on`
  walks the dependency links, `blocked-by` and `depends_on`, except a link marked `blocking: false`: the same set as
  the `deps` variable."
- **A done goal is walked from.** The goal is always in, and its open dependencies are in too. Alternative: the goal
  alone. The plan does not say; recommend as built, since a run order for a goal that is already done is rare, and the
  open work below it is what the view is for. The owner's comparison in `ts-0121-xp` settles it.
- **A goal that matches nothing is a count on the result, and the other parts still give members.** Alternative: an
  `Err`, no members. Recommend as built: one view still shows its `where` and `include`, and the error is loud on the
  view. The closure is empty then.
- **`exclude` removes members and steps, and does not cut the walk.** `A` depends on `B` depends on `C`, `exclude: B`:
  members `A`, `C`. The steps drop `B` and any step it empties. Alternative: steps untouched. Recommend as built, so
  steps are a subset of members.
- **A stalled layering ends in one step.** Every task it could not place goes in the last step: the ones on a cycle and
  the ones that wait on a cycle. `ViewResult::cycle` names the first kind. Alternative: put only the cycle in the last
  step and layer the waiting tasks after it. Recommend as built: the last step then reads "these cannot be ordered".
- **A key at two paths (a collision) is one member.** `where` is tested on each path's row, and a key matches when
  either does. Alternative: members by row. Recommend as built: the daemon says which path it shows.
- **A view with no `where`, `include` or `closure` has no members.** It is the union of nothing. Alternative: the whole
  board. Recommend as built.
- **A key matches as the corpus holds it, or as it was renamed from** (`Corpus::resolve_key`). A bare id, alias or
  short id never matches. Alternative: also accept those via `lookup`. Recommend as built (`ts-0118` item decision).
- **A dependency the corpus does not hold is skipped and not counted**, as is a name that fits several tasks.
  Alternative: a count on the view, `unresolved_dependencies`. Recommend adding it in `ts-0119-tj` if the Field should
  show it.
- **`author` is empty in a view.** The corpus keeps no provenance, so `evaluate_view` binds with none. Stage 5's GitHub
  source supplies it, as `ts-0117-gu` decided.
- **`compile_view` takes the board's task count and the budgets**, since the estimate is against the pass budget. A
  caller may evaluate with other budgets than it compiled with (a test does).
- **A task whose `where` errored is not a `where` match, and can still be a member through `include` or the closure.**
  The error is counted.

- Owner decisions (2026-11-18, at the build): all ten taken as recommended, on the owner's delegation. Decision 1 was
  changed at the brief (`follow` walks `DEPENDENCY_LINKS`, not `depends_on` alone) and stays overridable; its text is in
  plan 0.10. Decisions 2 (a done goal is walked from) and 3 (an unmatched goal is a count) are checked again by the
  owner's comparison in `ts-0121-xp`.

### Not done, by design

- View files, the daemon's `task.views` and `task.list` with a view, and the estimate run when a view loads and is
  saved: `ts-0119-tj`. It also decides the wire type of `ViewResult`.
- The comparison of the closure's set and steps with the prototype board's generator: `ts-0121-xp`.
- `layout` defaults (`groupBy`), what the board draws, and `queue: true` lanes: Stage 6 and 4c.
- A GitHub author for `author`: Stage 5. A second `follow` variant: 4c (`walk` matches `Follow` without a catch-all, so
  a new variant stops the build there).
