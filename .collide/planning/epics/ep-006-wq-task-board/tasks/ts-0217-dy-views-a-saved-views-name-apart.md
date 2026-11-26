---
id: ts-0217-dy
kind: task
status: done
title: "Views: a saved view's name apart from its file name"
owners: []
links:
  - { type: blocked-by, to: ts-0215-gt }
---

## Goal

A person saves a view under the words they type, spaces included: "Slow spots" is saved, listed, picked and named on
the command line as "Slow spots". The file it is saved in gets a name Collide makes from those words,
`slow-spots.yaml`. The plan's view example has always worked this way (`edge-mvp.yaml` holds `name: MVP`); the save
did not.

What exists.
- `host/src/views.rs` `save_view` refuses a name that is not a safe token (`collide_core::resolve::is_safe_token`:
  1 to 64 letters, digits, `.`, `_` or `-`) and writes `<name>.yaml`. `ts-0196-uw` found the refusal: S6's "Slow
  spots" cannot be saved, and `app/browser/s6.spec.ts` saves "Slow-spots" instead.
- Loading already takes a view's name from its YAML `name:`, not from its file name (`personal_candidates`,
  `repo_candidates`). A second view of a name already taken is listed with an error (`assemble`, `shadowed`).
- Share with team (`host/src/board_init.rs`) copies the person's view files by file name, with their bytes.
- The CLI resolves `--view <name>` in the daemon, by the whole name or by the part after its last `/` (Stage 4,
  Built-in and pack views).

What this adds.
- **The name.** The words typed, with spaces at the ends trimmed: 1 to 64 characters, no control characters and no
  `/`, which marks a pack's view (`collide/plan/spikes`). A name a built-in view has (`ready`, `bottlenecks`) is
  refused, as is the picker's All tasks, matched without regard to case. Each refusal says what to change.
- **The file name**, made from the name: lowercased; each run of characters other than `a-z`, `0-9`, `.` and `_` made
  one `-`; `-` and `.` trimmed from the ends; cut to 64 characters. A name that leaves nothing (one in another
  script) gets `view-` and a short hash of the name. The file name is always a safe token.
- **Where it is saved.** A file of the folder that holds a view of that exact name is that view's file: it is
  replaced only with `overwrite`, as now, whatever its file name. Otherwise the made file name is used. If that file
  is there and holds another name, or does not parse, the save is refused and asks for another name ("Slow Spots" and
  "slow spots" are two names and one file). The link checks and the atomic write stay as they are.
- **The command line** takes the name quoted: `collide tasks --view "Slow spots"`. A name no view has lists the
  names, each quoted.
- **S6's spec** saves "Slow spots" and detects it by that exact name. Its refusal check and the `Slow-spots`
  workaround go.

## Success criteria

- "Slow spots" saves as `slow-spots.yaml` with `name: Slow spots`, and `task.views` and `task.list` with the view
  name it "Slow spots".
- Saving "Slow spots" again is refused unless with overwrite, which replaces the same file.
- A hand-written `perf.yaml` holding `name: Slow spots` is the file an overwrite of "Slow spots" replaces; no second
  file appears.
- "Slow Spots" beside a saved "Slow spots" is refused and asks for another name; no file changes.
- A name with `/`, a control character, more than 64 characters, only spaces, or a built-in's name is refused, and
  says what to change.
- A view saved before this item (name equal to its file name) loads, lists and saves as before.
- `browser/s6.spec.ts` saves "Slow spots" and detects; no step in it refuses a space.

## Tests

- `host/tests/` (a temp root): the cases above, each against the folder on disk; the hash fallback; a link at the made
  file name still refused; Share with team copies the file under its made name.
- The CLI over a temp root: `collide tasks --view "Slow spots"` lists the view's tasks; an unknown name lists the
  names quoted.
- `app/browser/s6.spec.ts` (bridge): saves "Slow spots" and detects it.
- A unit test of the file-name rule: spaces, case, runs of other characters, the trims, the 64 cut, the hash
  fallback, and that each result passes `is_safe_token`.

## Notes

- IMPLEMENTATION_PLAN.md: Stage 4 (Views; Built-in and pack views); Stage 6a (Save as View); revision 31.
- **An item decision: names that differ only in case.** Recommend refusing the second, since one file name holds
  both and two views a person cannot tell apart help no one. Alternative: a numbered file name, `slow-spots-2.yaml`.
- **An item decision: the hash.** Use a hash crate the workspace already has. If none fits, stop and name it to the
  owner; no new dependency without them.
- Leave `is_safe_token` as it is: other names (packs, board ids) use it.
- The test rule: no test touches the real home, real launchd, a real daemon, the keychain or the network. Each test
  that runs the `collide` binary clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a
  temp root, as `host/tests/common/mod.rs` `TempRoot` does. A test never stubs or bypasses a safety gate (the trust
  check, `env_clear`, the link checks) to make a result appear.
- Nothing here runs a real `claude`. New dependencies: none expected.
- Stage 6a item (revision 31, 2026-11-26), from `ts-0196-uw`'s finding. It runs after `ts-0215-gt` and before
  `ts-0197-uj`, whose S6 walkthrough saves "Slow spots".
- Owner decision (2026-11-26): a view keeps the name typed, and Collide makes its file name (chosen over allowing
  spaces in file names, and over keeping the rule and stating it).

### Built

- `host/src/views.rs`:
  - `check_view_name` (new, public): the words typed, trimmed; 1 to 64 characters, no control character, no `/`, not
    a built-in's name (`ready`, `bottlenecks`) and not `All tasks`, the last two without regard to case. Each refusal
    says what to change.
  - `view_file_stem` (new, public): the file name made from a name, as the item says: lowercased; each run of
    characters other than `a-z`, `0-9`, `.`, `_` one `-`; `-` and `.` trimmed from the ends; cut to 64; `view-` and
    eight hex digits when nothing is left. A run of dots is made one dot (Decisions).
  - `save_view`: checks and trims the name, makes the file name, and finds the file of the folder that holds a view
    of that exact name (`file_holding`, in file name order, regular files that parse). That file is replaced, only with
    `overwrite`, whatever its file name. Otherwise the made name is used, and a file there that holds another name or
    does not parse refuses the save (`clash`), with or without `overwrite`. The link checks run for the made name and
    again for the file found. The atomic write is as it was.
  - `Lookup::message`: the names are each in quotes (`quoted`).
- `host/src/daemon.rs` `task_save_view`: `check_view_name` runs first, so a name's refusal is the first thing said.
  The wire shape is unchanged, so no `pnpm gen`.
- `host/src/git_fetch.rs`: `fnv1a` is `pub(crate)`; the hash fallback uses it (Decisions).
- `host/src/task_cli.rs`: the `--view` help says to quote a name with a space and that the names are listed quoted.
  `--view` itself already took a name with a space as one argument.
- `host/tests/fixtures/task-cli-help.txt`: follows the `--view` help (the test
  `help_and_the_usage_text_hold_the_stored_lines_of_the_new_forms` holds it).
- `host/src/board_init.rs`: no change. It copies by file name, and the file names are the made ones.
- `app/browser/s6.spec.ts`: saves "Slow spots" and detects the view by that exact name; the refusal step, the
  `Slow-spots` name and the hyphen stand-in are gone.

Tests, with the ones seen red at runtime (against the old rule, behind stubs of `check_view_name` and
`view_file_stem`) marked R.
- `host/tests/views_files.rs`:
  R `the_file_name_is_made_from_the_name_lowercased_with_runs_of_other_characters_as_one_dash`,
  R `the_file_name_loses_dashes_and_dots_at_its_ends`, R `the_file_name_is_cut_at_64_characters`,
  R `a_name_that_leaves_nothing_gets_view_and_a_short_hash_of_the_name`, R `every_file_name_is_a_safe_token`,
  R `slow_spots_is_saved_as_slow_spots_yaml_and_loads_back_under_the_name_typed`,
  R `spaces_at_the_ends_of_the_name_are_not_part_of_it`,
  R `saving_the_same_name_again_is_refused_unless_with_overwrite_which_replaces_the_same_file`,
  R `a_hand_written_file_that_holds_the_name_is_the_file_an_overwrite_replaces`,
  R `a_name_that_differs_only_in_case_is_refused_and_no_file_changes`,
  R `a_file_at_the_made_name_that_holds_another_name_or_does_not_parse_is_refused`,
  R `a_link_at_the_made_file_name_is_refused_even_with_overwrite_and_its_target_is_not_changed` (red because the
  name was refused as invalid, not blocked by the link),
  R `a_name_the_file_system_cannot_hold_is_a_file_named_for_it_and_two_such_names_do_not_clash`,
  R `a_name_that_cannot_be_a_view_is_refused_with_what_to_change_and_nothing_is_written`,
  R `a_name_of_64_characters_with_spaces_inside_is_a_view`. Green on the old rule, kept as guards:
  `a_view_saved_before_this_item_with_its_name_equal_to_its_file_name_saves_as_before`.
- `host/tests/daemon_corpus.rs` (a daemon over a temp root): R
  `a_view_saved_as_slow_spots_is_listed_and_listed_through_under_that_name_and_its_file_is_slow_spots_yaml`,
  R `a_view_saved_to_the_repo_as_slow_spots_is_the_view_of_that_name_once_committed`,
  R `save_view_refuses_slow_spots_again_and_slow_spots_in_another_case_and_overwrite_replaces_the_same_file`,
  R `save_view_says_what_to_change_for_a_name_with_a_slash_or_a_built_ins_name_or_only_spaces`, and the old
  refusal test, renamed `save_view_refuses_an_invalid_view_a_name_that_cannot_be_a_view_and_an_overwrite_with_no_flag`
  (`has space` removed from its refused names; `has/slash`, `Ready` and spaces only added), R.
  The Slow spots tests were first red because the save was refused; I then gave their fixture a task and a `where`
  (they first had an `include` that no task matched) and they passed.
- `host/tests/board_init.rs`: `a_view_saved_as_slow_spots_is_shared_under_its_made_file_name_with_its_name_kept`.
  It passes on the old code too: Share with team needed no change, so it is a guard.
- `host/tests/task_daemon_cli.rs`:
  R `a_view_named_with_a_space_is_listed_by_its_quoted_name_and_an_unknown_name_lists_the_names_quoted`
  (the listing half already passed; the quoted names were red).
- `app/browser/s6.spec.ts` (bridge): the S6 scenario 2 test saves and detects "Slow spots". Not run red: it needs the
  new host to pass at all.

### Decisions for the owner

- **Names that differ only in case:** took the item's recommendation, refuse the second. The check is the made file:
  a file at the made name that holds another name refuses the save. Alternative: a numbered file name. It also means
  a hand-written `perf.yaml` named `Slow spots` and a new `slow spots` are two views, since no file clashes.
  Alternative: refuse any name that matches a view of the folder without regard to case. Recommend the smaller rule.
- **The hash:** the workspace has no hash crate it declares. `sha2`, `siphasher`, `crc32fast` and `fnv` are in
  `Cargo.lock` only as other crates' dependencies, and naming one in a `Cargo.toml` is a new dependency. I added none.
  `host/src/git_fetch.rs` already has `fnv1a` (32 bits, no seed, for the mirror folder's name), and I reuse it:
  `view-` and eight hex digits. Alternative: declare `sha2`. Recommend `fnv1a`: it is stable, and a clash needs two
  other-script names whose hashes agree, in one folder, and the clash check then refuses the second.
- **Dots:** the item's rule leaves `a..b` as is, which is not a safe token, and the item says the result is always
  one. A run of dots becomes one dot. Alternative: make the second dot a `-`. Recommend the one dot.
- **The 64 cut:** a made name is at most 64 characters before `.yaml`, so the file is up to 69. This is the old
  size. The item's "file name is always a safe token" holds for the name without `.yaml`.
- **A file that holds the name but is a link, or does not parse:** not "the file that holds the name". A link at the
  made name is refused by the link check; one that does not parse refuses the save, as a file holding another name does.
- **Where the name is checked:** in the daemon before the view is compiled, and again in `save_view`, so `name: ' '`
  gets the new words and not "`name:` is empty".
- **`All tasks`:** the host has its own copy of the picker's text (`ALL_TASKS` in `host/src/views.rs`; the app's is in
  `app/src/field/ViewPicker.tsx`). Alternative: one copy in core, sent over the wire. Recommend leaving two.
- **Plan text to propose** for Stage 4's "A view's name and its file": add "a name whose made file name holds
  another view is refused, whatever the case of either name".

- Review (2026-11-26): the recommendations above are taken. Reusing `fnv1a` adds no dependency, so it needs no
  owner call. Fixed in review: a 64 cut that ended on a `-` or a `.` kept it (`aaa…a-.yaml`); the stem is trimmed
  again after the cut (`views_files.rs` `a_cut_that_ends_on_a_dash_or_a_dot_trims_it`).

### Not done, by design

- A view saved by hand under a name that breaks the new rule (a `/` in it, a built-in's name) still loads as it did;
  the rule is for saving only.
- Renaming a saved view is not here (the app's Save as a view makes a new name, and overwrite replaces).
- The S6 walkthrough that saves "Slow spots" by hand is `ts-0197-uj`.
