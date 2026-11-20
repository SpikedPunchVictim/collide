---
id: ts-0149-kq
kind: task
status: ready
title: "CLI: short ids, epic= and task create on a board whose plan is not Collide's"
owners: []
links:
  - { type: blocked-by, to: ts-0145-cq }
---

## Goal

On the Orchard board `collide task <goal>` is refused ("`<goal>` is not an item id; use the full id (ts-0012-ua)
..."): the short-id resolver (`resolve` in `host/src/task_daemon.rs`) knows Collide's id shape only. `task create ...
epic=E-73` is refused the same way. `create` takes only the Collide plan's arguments (`title`, `epic`, `kind`, `seed`,
`root`, in `host/src/task_cli.rs`), so an Orchard task, which needs a proposal, cannot be made from the CLI. Found by
the agent run of `ts-0121-xp`'s checklist (2026-11-19), steps 2 and 8.

## Success criteria

- A bare id that is exactly the short id of one item of any source names that item; two sources with the id are
  ambiguous, as today.
- `epic=<id>` resolves among a foreign source's epics the same way.
- `task create` passes the arguments the source's `create` op declares, and refuses any other with that list.

## Tests

- Write first and see red at runtime: the three on the fixture Orchard board (today: refused).

## Notes

- Recommendation: an exact match among every source's ids before Collide's short forms; create's arguments from the op's
  declaration.
- Owner decisions: none recorded yet. The recommendations stand until the owner answers.

### Built

- `resolve` in `core/src/short_id.rs`: an item whose `id`, or whose short id (the part of its `key` after the
  source), is exactly the query is found first, whatever shape. Two or more are `Many`. Only when none fits does it
  read Collide's short forms, and only a query of no known shape is `Malformed`. `name_in_board` (and so every bare
  id, and `epic=`) uses it unchanged.
- `resolve_epic` in `host/src/task_daemon.rs`: while the planning source lists items of kind `epic`, today's rule. When
  it lists none but has collections, `epic=` must be the id of one, by key (`plan:<id>`) or by the part after the source
  name, and `create` gets that part. A value that names none is exit 3 before any write, listing the collection ids
  (20, then "and N more"). `epic_collection` holds the check.
- `TaskCreateParams.args` (`core/src/owner/task.rs`), a map of name to JSON value, left out when empty; `title`, `kind`,
  `epic`, `seed` and `root` are as they were. `pnpm gen` ran: `schemas/TaskCreateParams.*.json` and the SDK's
  `TaskCreateParams.*.ts`.
- `parse_create` in `host/src/task_cli.rs`: every `name=value` it does not know goes to `CreateOptions.args` as text.
  Usage and help say so.
- `admit_create_args` and `from_text` in `host/src/task_args.rs`, called by `task_create` in `host/src/daemon.rs` after
  the named fields are put in `args` and before the write is judged. The daemon refuses a name the `create` op does not
  declare (a field or an extra), listing the declared names, as `INVALID_PARAMS` with `bad_argument` in its data, which
  `rpc_failed` turns into exit 2. A string extra is read by the declared type: text stays text, an array of text is the
  words between commas (an empty part is refused), an integer is parsed; any other declared type given as text is
  refused naming the argument and the type.
- Fixture `host/tests/fixtures/fake_foreign_plan_provider.mjs`: a repo-pack `plan` source with ids `WORK-12-...`
  (key `plan:WORK-12`), no epic items, collections `AREA-1`..`AREA-25`, and a `create` declaring `proposal`, `title`,
  `epic`, `tags` (array of text), `points` (integer) and `flag` (boolean). It writes each create's `args` to
  `created.jsonl` in the checkout.
- Tests. `core/tests/short_ids.rs`: `a_bare_id_that_is_the_short_id_of_one_item_names_it_whatever_its_shape` (red at
  runtime: Malformed), `two_sources_with_one_short_id_are_ambiguous_and_listed_in_id_order` (red: Malformed),
  `a_bare_id_that_is_the_whole_id_of_one_item_names_it`, `an_exact_match_is_taken_before_collides_short_forms`,
  `a_short_form_still_reads_when_no_item_has_that_id_exactly`,
  `a_query_that_names_no_item_and_is_no_form_is_malformed_even_with_foreign_items` (guards, green before).
  `host/tests/task_args.rs`: `an_undeclared_name_is_refused_as_a_field_or_an_extra_and_the_declared_ones_are_listed`,
  `text_is_read_as_the_declared_type_for_text_a_list_of_text_and_an_integer_only`,
  `text_for_any_other_declared_type_is_refused_naming_the_argument_and_the_type`,
  `a_value_that_is_not_text_is_the_sources_to_check`, `a_name_that_is_a_field_and_an_extra_is_refused`,
  `an_op_that_declares_no_properties_is_not_checked_and_the_source_answers` (all red at runtime with
  `admit_create_args` stubbed to the old behavior), and `the_fields_collides_own_create_has_pass_through_unchanged`
  (guard: same args as before, green before). `host/tests/task_daemon_cli.rs`, on the foreign source:
  `a_bare_short_id_names_the_item_of_a_source_whose_ids_are_not_collides`,
  `epic_names_a_collection_of_the_planning_source_when_it_lists_no_epic`,
  `epic_may_be_the_key_of_the_collection_and_the_source_gets_the_id_without_its_name` and
  `an_epic_that_is_no_collection_is_refused_before_any_write_and_the_message_lists_them_capped` (red at runtime with the
  old `resolve` and the collection branch switched off: exit 2 "is not an item id"),
  `the_source_gets_the_arguments_it_declares_as_the_types_it_declares`,
  `an_argument_the_create_does_not_declare_is_refused_with_the_declared_ones_and_nothing_is_written`,
  `an_argument_the_daemon_cannot_read_as_its_declared_type_is_refused_by_name` (red with the check stubbed: exit 0,
  extras dropped), and the guards `a_bare_id_that_no_item_has_and_that_is_no_form_is_a_usage_error`,
  `a_missing_required_argument_is_left_to_the_source`. On Collide's own plan:
  `collides_own_create_takes_an_argument_it_declares_that_the_command_does_not_know` and
  `collides_own_create_refuses_an_argument_it_does_not_declare_and_lists_the_ones_it_does` (red with the check stubbed),
  `collides_own_create_with_its_own_arguments_makes_the_same_file_as_before` (guard). Parser:
  `task_create_sends_every_word_it_does_not_know_as_an_argument_for_the_daemon_to_check`,
  `task_create_refuses_an_argument_given_twice`; the old `speed=3` row left the parser's refusal list, since the daemon
  refuses it now.

- `host/tests/fixtures/task-cli-help.txt`: the stored usage line of `task create` has `[name=value ...]`. The two other
  `TaskCreateParams` literals in `host/tests/daemon_follow_ups.rs` and `host/tests/daemon_plan_write.rs` got `args`.

### Decisions for the owner

1. A bare id matches an item's `id` or the short id in its `key`, not the `id` alone. The brief said `id`; a source
   whose `id` is a long form (`<short id>-<slug>`) gives the short id only in its key, and the item's criterion is
   "exactly the short id". Alternative: `id` alone, which leaves such a goal refused. Aliases are not read, as today.
   Recommendation: mine.
2. When the planning source lists no epic and has no collection either, `epic=` keeps today's rule and error ("no item
   matches"). Alternative: a message that says the source has no collections. Recommendation: mine; the rule is only
   for a source that has groupings, and an empty Collide plan is unchanged.
3. `create` gets the collection id without the source name (`AREA-3`), also when `epic=plan:AREA-3` was typed.
   Alternative: the key. Recommendation: mine; the source's own `epic` takes its own id.
4. The collection list in the refusal names the ids without the `plan:` prefix, 20 at most, then "and N more".
5. An op with no `properties` in its `args` (or no `create` op at all) is not checked, and extras go through as
   text. The source then answers in its own words. Alternative: refuse every extra. Recommendation: mine; there is no
   declaration to hold them to.
6. A name given as a field and in the map is refused ("given twice"); only a caller that is not the CLI can do that.
7. Non-text values in the map pass as they are. The CLI sends text only.
8. The refusal of an undeclared argument now comes from the daemon for Collide's own plan too, with the same words the
   CLI used (`op `create` has no argument ...`). The new effect is that Collide's own `raisedBy` and `run`, which its
   `create` declares, can be given from the command line.

Proposed plan text (0.10, CLI bullet, after "A short id resolves in the daemon, and `epic=` names an epic before any
other item."): "A bare id that is exactly an item's id, or the short id in its key, names that item whatever the
source's id shape, before Collide's short forms are tried; two such items are ambiguous. For a planning source that
lists no epic, `epic=` is the id of one of its collections. `task create` passes `name=value` words the source's
`create` declares (the daemon checks each name; a list is the words between commas, a number a whole number) and
refuses any other with the declared names (exit 2)."

### Not done, by design

- `ts-0150-dj`, `ts-0151-fk`, `ts-0152-jq`, `ts-0153-kp`: none is changed or made trivial. `ts-0153-kp` records the fake
  plan CLI's `new task` envelope; the arguments Collide now sends to `create` are the ones the declaration names.
- `dependsOn` on the Orchard board is given as `dependsOn=a,b` (words between commas); a title with a comma in a list
  element cannot be given. No source needs it yet.
- The owner check `ts-0138-pn` must run the CLI forms on a real clone: not done here.

### At review

- Decisions 1 to 8 taken as recommended (owner delegation). Decision 1 (the key's short id too) is right: a long-form id
  gives its short id only in the key.
- Changed at review: a `create` argument given as text is read as an op's word is (`convert` in `task_args.rs`: a
  number, true or false, a list or an object as JSON), not as words between commas. The brief asked for commas; that
  made the CLI give a list two ways, one for `task <id> <op>` and one for `task create`. `from_text` is removed, the
  help text and four tests follow (`dependsOn=["T-1"]`, not `dependsOn=a,b`). The Decisions' proposed plan text and the
  Not done line on commas are superseded by this.
- The run-agent wall holds: the args map is merged and checked before `judged_write`, where `follow_ups.rs` replaces
  `run` and `raisedBy` for a run agent's create. A person may now give `raisedBy` and `run` from the CLI (decision 8); a
  person can write the item file anyway.
- Plan 4c CLI bullet: the bare-id, `epic=` and create-argument rules, applied at review.
- Mutations, each red then restored: the exact step by `id` alone (two core tests), the first exact match taken (one),
  named fields not checked (one), a field and an extra of one name allowed (one), text not converted (two), the
  collection path off (three CLI tests), a key not stripped (one), the daemon not checking (four CLI tests).
