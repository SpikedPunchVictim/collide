---
id: ts-0123-yw
kind: task
status: ready
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
