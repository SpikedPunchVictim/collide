---
id: ts-0135-cx
kind: task
status: ready
title: "Plan pack manifest: roles, views and bare view names"
owners: []
links:
  - { type: blocked-by, to: ts-0134-ch }
---

## Goal

`collide/plan` ships its views (`spikes`, `decisions-needed`, and the slice's `--ready` as a built-in view) and its
`roles` setting, and a view can be named by its bare name.

What exists. A pack may declare `views:`, which a board adopts by name: `core/src/pack.rs` `views`,
`host/src/views.rs` `pack_candidates` (named `pack/name`, and a pack that is not built in gives none to an untrusted
line), `task.views` and `task.list` with a named view (`host/src/daemon.rs` `task_list`, `loaded_views`), which matches
the whole name. `View.queue` exists and nothing reads it. `packs/plan/pack.yaml` declares only `providers`. The
slice's `--ready` is `core/src/ready.rs` `readiness` applied by `host/src/task_cli.rs`, and a CEL view cannot say it,
because a view sees one task and its blockers' states are other tasks'. `KindDecl.roles` (`core/src/item.rs`) is a tag
core never reads, and no setting maps kinds to roles. Provider role tags as defaults are Stage 12.

What this adds.
- `packs/plan/pack.yaml` `views:`: `spikes` (`kind == 'spike'`), and `decisions-needed` with `queue: true` (an open
  question, or a proposal that is `proposed`: `(kind == 'question' && category == 'ready') || (kind == 'proposal' &&
  status == 'proposed')`).
- A built-in `ready` view, in `core` (`view_builtin.rs`, new): the members are the tasks `readiness` calls ready, in the
  corpus's order, with a cycle and an unknown blocker reported in the view's report. `ViewSource` gains `Builtin {}`
  (additive), and `task.views` lists it. It does not hold a `where`.
- A bare view name in `task.list` (and so `collide tasks --view spikes`): the whole name first, then the part after the
  last `/` when exactly one view has it, else an error that lists the full names. If two packs declare the same key, the
  one that supplies the board's planning source wins (the precedence sources use); with neither, the error.
- `roles` in `packs/plan/pack.yaml` `config:`: an object from a role (`proposal`, `question`, `spike`, `decision`,
  `deferred`) to a list of kind names, with the markdown provider's own mapping as the default (`proposal`:
  `proposal`; `question`: `question`; `spike`: `spike`; `decision`: `proposal`, `question`; `deferred`: none). Not
  `overridable` (the repo's value wins) and not `repoOnly`. Hooks read it as `config.roles`.
- A check when a line loads: each kind a role names must be a kind of the board's planning source, else the line reports
  a problem that names the role and the kind; a role with no kind is "off", with the reason. The source still works.

## Success criteria

- `task.views` lists `collide/spikes`, `collide/decisions-needed` (with `queue: true`) and `ready`, each with its
  source; over a fixture corpus `spikes` has the spikes and no other kind.
- `ready` equals `readiness` over the fixtures of `core/src/ready.rs`: blocked, unblocked by `done` and by `dropped`, a
  `blocked-by` cycle (none ready, the cycle in the report), an unknown blocker.
- `decisions-needed` lists the open questions and the proposals that are `proposed`, and nothing else.
- `collide tasks --view spikes` (through `task.list`) is the same as `--view collide/spikes`; two views with one key and
  no tie-break give the error with both names; a name no view has lists the names.
- The default `roles` read as above; a mapping that names `adr` on the markdown source is a problem naming role and
  kind, and tasks still list.
- The manifest validates against the generated schema, and the `roles` schema passes `check_setting_schema`.

## Tests

Write these first.

- `core` tests for the built-in view over the readiness fixtures, and the report's cycle and unknown-blocker fields.
- Daemon tests (`daemon_corpus.rs` style) for `task.views` and `task.list` by full name, bare name, a tie, a miss, with
  the real built-in pack over fixture items, in a temp root.
- A `core` test for the `roles` default, the layers, and the check against a source's declared kinds.
- The manifest tests of the five packs and the invalid fixtures, with the new keys.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 4c: Planning ("`collide/plan`'s ... `roles` mapping; pack views, with the slice's `--ready` as a built-in
    view"); Success Criteria ("Collide's spike records ... list in `collide tasks --view spikes --json`").
  - 0.7: Keys added for planning (`views:`, `roles`). 0.10: Planning sources ("A board has one planning source. A role
    that source lacks either maps onto a kind it has, or is off with a reason"), Views; 0.11: Queue (a lane fed by a
    view). `docs/research/planning.md` 2.5 and 2.6 (role tags and mappings).
  - Carried from `ts-0118-wp`: `queue: true` lanes (here, and Stage 6 for the lane).
- **Decision for the item, with a recommendation: `ready` is a rule in core, not CEL.** A view's expression sees one
  task and its blockers' ids, not their states (no corpus variable; `ts-0117-gu`). Recommend a built-in view that
  applies the existing rule, so `--ready` and the view agree by construction. Alternative: a CEL variable for "open
  blockers", which needs the corpus in every pass and breaks the one-task rule. Not recommended.
- **Decision for the item, with a recommendation: bare names.** Resolve in the daemon, so the app and the CLI agree.
  The tie-break follows the sources' precedence (the board's own pack over the built-in). Alternative: only the CLI
  resolves. Then the app needs its own rule.
- **Decision for the item: `roles` is `overridable: false`, not `repoOnly`.** 0.10 says only `layout` is repo-only. A
  user may map roles on a repository with no `.collide/config.yaml`, and the repo's file wins when it has one.
- **Decision for the item: `Follow`.** `ts-0118-wp` left a second variant to 4c. No view here needs it (`ready` is
  built in), so none is added.
- Area: `packs/plan/pack.yaml`, `core/src/view_builtin.rs` (new), `core/src/view_eval.rs`, `core/src/owner/task.rs`
  (`ViewSource`), `host/src/views.rs`, `host/src/daemon.rs` (`task_list`), `host/src/board_load.rs` or
  `source_problems` (the roles check), the generated schemas and TypeScript, `core/tests/`, `host/tests/`.
- The test rule: no test touches the real home, real launchd or a real daemon. Each test that runs the `collide` binary
  clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as
  `host/tests/common/mod.rs` `TempRoot` does. Settings files are fixtures, never the owner's own `settings.json`. A temp
  repository sets `GIT_CONFIG_GLOBAL` and `GIT_CONFIG_SYSTEM` to empty files.
- Stage 4c item (revision 23 scope, split 2026-11-18). Items of Stage 4c run one at a time, in id order, because they
  share `core`, `host`, `packs/plan` and the generated files.
- Nothing here runs a real `claude`. No dependency is added.
- Owner decisions (2026-11-18, at the build): the item's choices as it recommends, and the builder's decisions 1 to 7 as
  built, on the owner's delegation. At review: decision 6 as written says a role whose kinds are all missing is not a
  problem, but the code and the `adr` test make each missing kind a problem and the role off as well, which is what the
  item's criterion asks; the plan text says so. The views are `collide/plan/spikes` and `collide/plan/decisions-needed`
  (the pack's name is `collide/plan`), not `collide/spikes` as the item's criterion wrote; `pack.yaml`'s comment is
  fixed. The builder's plan text (decision 8) is applied to 0.7 and 0.10. Mutations caught at review: `decisions-needed`
  taking every proposal, the all-missing role left on, and a miss listing no names.

### Built

- `core/src/view_builtin.rs` (new): `READY`, `ready_view`, `evaluate_ready`. `core/src/corpus.rs`: `CorpusReadiness`
  and `Corpus::readiness` (ready keys, cycles, unknown blockers, per source); `ready_ids` now reads it, so `--ready` and
  the view agree by construction. `core/src/ready.rs`: `UnknownTarget` is a wire type. `core/src/view_eval.rs`:
  `CompiledView::ready`, `is_builtin`; `evaluate_view` hands a built-in to `evaluate_ready`; `ViewResult` and
  `view_report` carry `unknown_blockers`. `core/src/owner/task.rs`: `ViewSource::Builtin {}` and
  `ViewReport.unknown_blockers` (left out when empty). The cycle uses `ViewReport.cycle`.
- `core/src/roles.rs` (new): `Roles`, `read_roles`, `check_roles`, `RoleProblem`, `RoleCheck`.
- `packs/plan/pack.yaml`: views `spikes` and `decisions-needed` (`queue: true`), and the `roles` setting (object of
  five lists, `additionalProperties: false`, default the plan pack's own mapping, `overridable: false`, not
  `repoOnly`). `decisions-needed` is `(kind == 'question' && category == 'ready') || (kind == 'proposal' && status ==
  'proposed')`. `packs/plan/src/item.ts` maps the question status `open` to category `ready`, so the item's
  expression is what the code says.
- `host/src/views.rs`: `builtin_candidates`, the built-in first in precedence (a file or pack view named `ready` is
  listed with an error), a cache key for it, `Lookup` and `find_view` (whole name, then the part after the last `/`;
  a tie goes to the one pack that supplies a source of the board). `host/src/daemon.rs`: `task_list` uses `find_view`
  (a miss is `NOT_FOUND`, a tie is `INVALID_PARAMS`, both list names); `BoardViews` carries the supplying packs from
  `sources::plan`; `with_role_problems` adds the roles problem to `board.list` and `board.open`.
  `host/src/sources.rs`: `role_problems`, run at the planning source's start, where kinds are known.
- `pnpm gen` rewrote `schemas/Task{List,Views}Result.*.json` and the matching `pack-sdk` generated files. The app test
  doubles under `app/test/support/` need no change: the new keys are optional on the wire.
- Tests. `core/tests/view_builtin.rs` (10), `core/tests/plan_roles_and_views.rs` (17), `host/tests/views_find.rs` (9),
  `host/tests/daemon_plan_views.rs` (10, through the built plan pack), `host/tests/task_daemon_cli.rs` (3:
  `a_bare_view_name_gives_the_same_list_as_the_full_name_and_the_same_json`,
  `a_view_name_no_view_has_exits_3_and_lists_the_full_names`,
  `the_built_in_ready_view_lists_what_the_ready_flag_lists`). The CLI tests run the daemon over a temp root as the
  file's other tests do.
- Seen red at runtime, by stubbing and then restoring: `evaluate_ready` returning an empty result failed 8 of 10 in
  `view_builtin` and `ready_is_what_the_ready_filter_lists_...` and
  `a_view_file_named_ready_cannot_replace_the_built_in_one`. Breaking the bare-name step of `find_view` failed 4 in
  `views_find`, and 2 in `daemon_plan_views` and 1 CLI test. Breaking the tie-break failed 1 in `views_find` and 1 in
  `daemon_plan_views`. Breaking the missing-kind check in `check_roles` failed 2 in `plan_roles_and_views` and 1 in
  `daemon_plan_views`. `pack.yaml` at its old text failed 10 in `plan_roles_and_views` and 7 in `daemon_plan_views`.
  The CLI test for a miss and the default-roles test pass against the old code by nature (a miss was already an
  error; the default has no problem).

### Decisions for the owner

1. **The built-in `ready` is first in precedence and cannot be shadowed.** A file or pack view of that name is listed
   with an error. Alternative: let a personal view replace it. Recommend as built: `--ready` and the view must agree.
2. **A bare-name tie goes to the pack that supplies a source of the board** (`sources::plan` pack names), else it is an
   error naming both views. Alternative: the CLI alone breaks ties. Recommend as built: the app then agrees.
3. **A tie is `INVALID_PARAMS` (CLI exit 1); a miss is `NOT_FOUND` (exit 3).** Alternative: exit 10 for both, as a
   view error. Recommend as built: the request is wrong, not the view.
4. **The report is named by the full name found**, so a bare name and a full name give one report.
5. **The roles check runs when the planning source starts** and shows in the `board.list` and `board.open` line
   problems, not in `trust_review`. The tasks still list. Alternative: refuse the line. Recommend as built.
6. **A role with no kind, or whose kinds are all missing, is off with a reason in `RoleCheck.off`, and is not a
   problem.** A kind missing from a role that has others is a problem. `roles` replaces the default as a whole, so a
   role left out of a user's map is off.
7. **`save_view` does not refuse the name `ready`;** the saved file is listed with the shadow error. Alternative:
   refuse at save. Recommend a later item if wanted.
8. **Plan text to propose.** 0.10 Views: the built-in `ready` view, `ViewSource` `builtin`, `unknown_blockers` in the
   report, bare-name resolution and its tie-break. 0.10 Planning sources: the roles check and where it shows.
   0.7: `roles` is `overridable: false`, not `repoOnly`.

### Not done, by design

- CLI stderr lines for the `ready` cycle and unknown blockers: `ts-0136-rr`. Their display in the app: Stage 6.
- Queue lanes that read `queue: true`: Stage 6.
- Provider role tags as defaults: Stage 12.
- A `Follow` variant: none needed, as the item decided.
