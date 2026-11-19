---
id: ts-0147-ve
kind: task
status: ready
title: "Closure: a done goal and a missing goal as the prototype generator has them"
owners: []
links: []
---

## Goal

The agent run of `ts-0121-xp` settled `ts-0118-wp`'s two closure decisions against the prototype board's generator, by
the checklist's own rule. A done goal: the generator walks nothing (no open run; the former dependencies are listed as
stranded), while Collide gives the goal and its 13 open dependencies. A goal that names no task: the generator stops
with an error (exit 2, no task file carries that id), while Collide gives an empty closure and `goal_unmatched`.

## Success criteria

- A done goal: the closure holds the goal alone; nothing is walked from it, and it is no step.
- A goal that names no task, in a view whose members come from the closure alone: a view error (exit 10, the message
  names the goal), no members.
- A view with a `where` beside the closure keeps today's rule for a missing goal: its other members, and
  `goal_unmatched`.
- Plan 0.10 Views says the two rules.

## Tests

- Write first and see red at runtime: one test per rule, on the fixture tree.

## Notes

- Recommendation: the three rules above. The alternative for a done goal is an empty set, which hides the goal itself;
  the goal alone keeps it on the board.
- This changes what a view shows; the owner may pick otherwise. Owner decisions: none recorded yet. The recommendations
  stand until the owner answers.

### Built

- `core/src/view_eval.rs`:
  - `walk` returns the goal alone when the goal is not open (done or dropped): no walk, so no dependency is a member.
    `Reach` doc says a done goal is in `set` and has no entry in `needs`. `layers` now places the tasks of
    `Reach::needs` instead of `Reach::set`, so a done goal is a member and no step. No second walk was added.
  - New `ViewFailure::GoalUnmatched { goal }`, with a Display that names the goal. `evaluate_view` returns it when the
    goal matches no task and the view has no `where` and no `include` (members from the closure alone). With a `where`
    or an `include` beside the closure, the old rule stays: `ViewCounts::goal_unmatched`, the other members listed.
  - Module doc updated for both rules.
- The error reaches the CLI by the path every view error takes: the daemon turns a `ViewFailure` into the report's
  `error` (CLI exit 10), or `INVALID_PARAMS` for an inline view. No daemon or CLI code changed. No wire type changed,
  so `pnpm gen` was not needed.
- `IMPLEMENTATION_PLAN.md`, 0.10 Views, under the `closure` bullet: two new sub-bullets, "A done goal" and "A goal that
  names no task", each ending `(ts-0147-ve)`. No existing sentence was changed.
- Tests, `core/tests/view_eval.rs` (pure, on fixture boards):
  - `a_done_goal_is_the_only_member_nothing_is_walked_from_it_and_it_is_no_step` (red at runtime: members were A, B, C)
  - `a_dropped_goal_is_treated_as_a_done_one` (red at runtime)
  - `a_goal_that_names_no_task_in_a_view_of_the_closure_alone_is_a_view_error_naming_the_goal` (red at runtime: Ok)
  - `a_goal_that_names_no_task_beside_a_where_keeps_the_other_members_and_is_counted` (the old rule; green before)
  - `a_goal_that_names_no_task_beside_an_include_keeps_the_included_members_and_is_counted` (the old rule; green)
  - `a_bare_id_as_the_goal_is_not_guessed` (changed: now a view error naming the id; red at runtime)
  - Removed the tests of the old rules: `a_done_goal_is_in_the_set_and_its_open_dependencies_are_walked`,
    `a_done_goal_with_nothing_open_below_it_is_alone`, and
    `a_goal_that_matches_nothing_is_reported_and_the_other_parts_still_count` (now the include test above).
- Tests, `host/tests/task_daemon_cli.rs` (daemon over a fixture repository, as the other view tests):
  - `a_closure_view_whose_goal_names_no_task_exits_10_and_names_the_goal` (red at runtime: exit 0 with the guard turned
    off by a temporary `if false`, restored)
  - `a_view_with_a_where_beside_a_goal_that_names_no_task_still_lists_its_other_members` (exit 0; green before)

### Decisions for the owner

1. "Members from the closure alone" means no `where` and no `include`. Alternative: only a `where` keeps the old rule,
   so a closure with an `include` also errors. Recommendation: mine. The item protects "a `where` beside the closure"
   only, but an `include` beside it also gives the view members the error would hide, and the existing include test
   keeps its meaning. The smallest code is the same either way.
2. A dropped goal is treated as a done one (the walk's own open test, `is_open`). Alternative: only `done`. The
   recommendation is mine: the generator has one closed state.
3. The error is a new `ViewFailure` variant, so it is kept per stamp like the other failures and reaches the CLI by the
   existing path. Alternative: a field on `ViewResult`, which every caller would have to check. Recommendation: the
   variant.
4. Plan text was applied (0.10 Views, closure bullet) as the brief asked, though the hard rules say not to edit the
   plan. Revert the two sub-bullets if the owner wants it applied elsewhere.

### Not done, by design

- `ViewReport.goal_unmatched`'s wire description still says only "when it matches no task: the closure is then
  empty". It stays true; the error case has no report counts. Changing it would regenerate the schemas and the SDK
  for no behavior.
- Items `ts-0148-cv` to `ts-0153-kp` read no closure and no goal (their text only has a "Goal" heading), so none is
  changed or made trivial by this item.

### At review

- The item's three rules shipped as recommended; the owner was told before the build and did not pick otherwise.
  Decisions 1 to 3 taken as recommended (owner delegation): an `include` beside the closure keeps the old rule as a
  `where` does.
- The builder's 0.10 text kept: the brief asked for it, though its standing rules said not to edit the plan. Later
  briefs leave plan text to the review.
- `layers` now places the keys of `needs`, not `set`: every walked task gets a `needs` entry (an open goal is in
  `by_key`, and only open tasks in `by_key` are pushed), so the steps of an open goal are unchanged.
- Mutations, each red then restored: the done goal walked (two tests), a dropped goal walked (one), an `include` not
  counted as a part, a `where` not counted as a part, the done goal made a step (two), the message without the goal (the
  CLI exit-10 test).
