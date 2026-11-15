---
id: ts-0020-tt
kind: task
status: ready
title: "Planning slice: task CLI"
owners: []
links:
  - { type: blocked-by, to: ts-0015-dp }
  - { type: blocked-by, to: ts-0029-ea }
---

## Goal

`collide tasks [--ready] [--kind <kind>] [--json]`, `collide task <id> [--json]` and `collide task <id> <op> ...` (the ops the provider declares: `setStatus`, `link`, `claim`, `unclaim`). `<id>` may be the short form (`12` or `12-ua`) when it matches one item. The ready rule is a pure function in `core`: a task is ready when its status is `ready` and every `blocked-by` target is `done` or `dropped`; a `blocked-by` cycle is reported and none of its items is ready. Until the daemon exists the CLI starts the provider itself, through the built-in loader of `ts-0029-ea`. The corpus stage moves it behind the provider supervisor.

## Success criteria

- `collide tasks --json`, run in Collide's own repo, lists every hand-written item under `epics/` with no `task.invalid`. It does not read `spikes/`.
- `collide tasks --ready` equals the ready list checked by hand from the items' frontmatter.
- `collide task <id> setStatus active` changes only that item's `status` line (`git diff`).
- With two tasks numbered 12, `collide task 12` lists both and asks for the suffix.
- `collide tasks` and `collide task <id>` print each item's epic beside its id (`ep-002-va / ts-0007-ga`), and each link's target with its epic (`blocked-by ts-0009-cu (ep-002)`). The `--json` output carries the epic as a field.
- An unknown op fails with the list of ops the provider declares.

## Tests

Write first: the ready rule (blocked, unblocked by `done` and by `dropped`, a `blocked-by` cycle); short-form lookup (one match, several, none); `--kind` filtering; one end-to-end test that runs the CLI on a fixture root and checks `git diff`.

## Notes

- IMPLEMENTATION_PLAN.md, 0.10 (Collide's own build tasks: ready rule, ids) and Stage 2: Skeleton, Planning slice and its criteria.
- Area: `host` (CLI), `core` (the ready rule).
- Done when the criteria hold.
- The id never carries its epic (owner, 2026-11-14; 0.10, ids): the folder is the epic, and the CLI shows it beside the id.
