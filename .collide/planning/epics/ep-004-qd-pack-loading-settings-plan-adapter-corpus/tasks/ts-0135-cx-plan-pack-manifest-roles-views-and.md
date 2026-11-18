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
- Owner decisions: none recorded yet. The recommendations stand until the owner answers at the split.
