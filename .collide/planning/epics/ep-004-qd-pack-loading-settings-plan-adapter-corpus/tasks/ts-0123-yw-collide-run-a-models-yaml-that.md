---
id: ts-0123-yw
kind: task
status: done
title: "collide run: a models.yaml that fails to load is named, not told it lacks the alias"
owners: []
links: []
---

## Goal

When the daemon refuses `collide run` and `~/.config/collide/models.yaml` exists but does not load, the refusal branch
(`host/src/run_cli.rs`, the `RunStartResult::Refused` arm) treats the load error as "not mapped" and prints
`models_hint`: "<path> does not map the alias `fast`". That sends the person to add a mapping that is already there.
Seen at `ts-0105-jv` step 6, where a pasted block made the file fail with `unknown field agents`.

## Success criteria

- A file that does not load prints its path and the load error, once, and no mapping hint.
- A missing file and an alias the file does not map print as they do now.

## Tests

- Write first and see red at runtime, in the run CLI tests with a temp `HOME`: an unloadable `models.yaml` prints the
  error and not "does not map".
- No file, and a file without the alias, keep their hints.

## Notes

- Check whether the daemon's refusal already names the load error; if it does, add nothing after it, so the error
  shows once.
- Found at `ts-0105-jv` (2026-11-18).

### Built

The daemon's refusal already names the load error. `check_agent` in `host/src/file_run.rs` pushes the
`load_user_models` error as a problem, and the CLI prints every problem as `collide run: refused: <message>`. The
message holds the path, the place and the error. The fault was only in the CLI, which printed `models_hint` after it.

- `host/src/run_cli.rs`: new `unmapped_alias_hint(home, alias)`. It returns `None` when `models.yaml` does not load,
  and when the file loads and maps the alias. It returns `models_hint` for a missing file and for a file without the
  alias. The `RunStartResult::Refused` arm in `run_in` prints what it returns. `models_hint` is unchanged.
- `host/tests/adapters/claude/run_command.rs` (temp `HOME` in the fixture, in-process CLI):
  - `a_models_file_that_does_not_load_is_named_once_and_no_mapping_is_offered`: a file with an `agents` key. The error
    and the path print once, with no "does not map" and no mapping block. Seen red at runtime.
  - `a_models_path_that_cannot_be_read_is_named_and_no_mapping_is_offered`: `models.yaml` is a directory. Seen red at
    runtime (it printed "There is no models.yaml").
  - Kept as they were, and green: `an_alias_with_no_mapping_is_refused_and_the_message_shows_the_mapping_to_add`,
    `with_no_models_file_the_message_says_where_it_goes_and_maps_fast`, and in `run_process.rs`
    `an_alias_with_no_mapping_exits_5_and_shows_the_mapping_to_add`.
- Red at runtime was seen by putting the old logic back inside `unmapped_alias_hint` (a load error counted as
  not mapped). Both new tests failed, then passed with the fix.

### Decisions for the owner

1. A file that does not load gets no hint at all, not even a shortened one. The alternative is a line such as "Fix the
   file, then run again". The refusal line already says where the error is. Recommend none: the item says print nothing
   more, and one line is enough.
2. An unreadable path (a directory, no permission) is treated like a parse error: named by the daemon, no hint. The
   alternative is to keep the "no models.yaml" hint for it. Recommend as built: the hint would be wrong, as the path
   exists.
3. The CLI still loads the file itself to decide on the hint. The alternative is to have the daemon mark the problem
   kind on the wire, so the CLI need not load. Recommend leaving it: that changes a wire type for a small fix.

- Owner decisions (2026-11-18, at the build): all taken as recommended, on the owner's delegation: the daemon's refusal
  already names the load error, so the CLI adds no hint after it; an unreadable path is treated as a file that does not
  load; no wire change.

### Not done, by design

- Nothing carried to a later item.
