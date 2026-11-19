---
id: ts-0144-eh
kind: task
status: ready
title: "Ready view: a blocker named by its short id is the task it names"
owners: []
links: []
---

## Goal

On the Orchard board `collide tasks --ready` lists no task and prints 1,022 lines "blocked by T-NN-NNN, which is not an
item" for 380 targets, each a real task. `readiness` in `core/src/ready.rs` matches a `blocked-by` target against
`task.id`. An Orchard task's id is its file stem, the short id and its slug (`ts-0110-ra`), and its links name the short
id. The closure resolves the same links through the corpus names (`ts-0118-wp`), so it works. Found by the agent run of
`ts-0121-xp`'s checklist (2026-11-19).

## Success criteria

- The ready rule resolves a link's target as the closure does, with one shared resolver.
- On the fixture `basic` tree: a task whose blocker is done is ready, one with an open blocker is not, and only a target
  that names no task is reported unknown.
- Collide's own plan gives the same ready set as today.

## Tests

- Write first and see red at runtime: readiness over the fixture tree with short-id links lists the task whose blocker
  is done (today: none).
- Mutation at review: matching by `task.id` again turns the test red.

## Notes

- Recommendation: the corpus's name map, the one `follow` walks. Area: `core/src/ready.rs`, `core/src/corpus.rs`,
  `core/src/view_builtin.rs`.
- Owner decisions: none recorded yet. The recommendations stand until the owner answers.

### Built

- `core/src/ready.rs`, `readiness`: takes a resolver, `readiness(tasks, resolve)`. `resolve` gives the key of the item a
  `blocked-by` target names. The rule now names items by `key`: `ready`, `cycles` and `UnknownTarget.from` are keys. A
  resolved key that is not among the given tasks counts as no item.
- `core/src/corpus.rs`, `Corpus::readiness`: passes `Corpus::resolve_target`, the resolver that `follow` walks
  (`out_links`). It no longer qualifies ids, since the rule gives keys. `--ready` and the `ready` view both use it.
- Side find: the corpus used to report `source:<id>` as the ready key. On an Orchard board the key holds the short id
  and the id is the file stem, so even a task with no blocker was never matched to its row. Keys fix that too.
- Tests in `core/tests/corpus.rs` (arch-board shape: key `plan:T-01`, id `T-01-<slug>`, links name `T-01`):
  `a_blocker_named_by_its_short_id_is_the_task_it_names_for_the_ready_filter`,
  `a_blocker_named_by_id_alias_or_key_is_the_task_it_names_for_the_ready_rule`,
  `only_a_blocker_that_names_no_task_is_reported_unknown_with_the_holders_key`,
  `a_cycle_of_short_id_links_is_reported_by_key`, `a_blocker_name_that_fits_two_tasks_is_not_resolved`.
  In `core/tests/view_builtin.rs`: `a_blocker_named_by_its_short_id_makes_the_view_list_the_task_that_it_blocks`.
  In `core/tests/ready_rule.rs`: `a_target_is_what_the_resolver_says_it_names`,
  `a_resolved_key_that_is_not_one_of_the_tasks_names_no_item`. The old `ready_rule` and `view_builtin` tests keep their
  cases, with keys in the asserts.
- Red at runtime: against the old rule (matching a target to `task.id`, stubbed in `Corpus::readiness`) the first four
  corpus tests and the view test failed; the old code listed no task and 1 unknown per link, as the item says. The
  ambiguity test and the two `ready_rule` resolver tests passed against the old rule on purpose: they pin the new
  contract (an ambiguous name stays unresolved; the rule trusts the resolver), they do not catch the old bug.
- Collide's own plan: its ids are file stems and its links name full ids, so `resolve_target` finds the same task as
  the old id match. The ready set is unchanged (no test runs the binary on the real tree).
- No wire type changed, so no `pnpm gen`.

### Decisions for the owner

1. The rule identifies items by key, not id (item left open). Alternative: keep ids and map key to id in the corpus.
   Recommend keys: the corpus and views speak keys, and on Orchard the id is not the key.
2. Each source stays on its own (kept from the rule as it was). A name that resolves to a task of another source
   (`gh:12` from a `plan` item) is reported unknown, not followed. Alternative: let a blocker cross sources, as
   `follow` does. Recommend leaving it: a blocker's state then depends on a source that may not have started.
   Plan text to propose under 0.10, ready rule: "A `blocked-by` target is resolved as `follow` resolves a link; one
   that names no task of the holder's source is reported unknown."
3. An ambiguous name (two tasks share an alias) is unknown, as in `follow`. Alternative: list it as blocked by both.
   Recommend unknown: it is a typo-class fault the report names.

### Not done, by design

- `ts-0147-ve` (the closure's done goal and missing items) is untouched. It is not made trivial: the closure walks
  `out_links` already; this item changed only the ready rule. It may reuse `Corpus::resolve_target` as before.
- `ts-0145-cq` to `ts-0150-dj`, `ts-0151-fk`, `ts-0152-jq`: not read for change, none touched. None is changed by this.
- Unknown-blocker report text ("which is not an item") unchanged; it now names the holder's key (`plan:T-02`).

### At review

- Decisions 1 to 3 taken as recommended (owner delegation). Decision 2's rule is in 0.10's Links bullet, with the
  ambiguous case named.
- A target that names an old key now follows the rename, as `follow` does; the old id match missed it too.
- Mutations, each red then restored: the old rule (a target matched to `task.id`) fails four corpus tests and the view
  test; the unknown blocker's `from` as the id fails the unknown test; a resolved key that is not among the tasks taken
  as found fails three `ready_rule` tests; an ambiguous name resolved to its first key fails the ambiguity test. A
  resolver that only qualifies the target passes on short ids (the key is `plan:` plus the short id) and fails the alias
  test only, which is enough.
