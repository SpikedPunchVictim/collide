---
id: ts-0118-wp
kind: task
status: ready
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
