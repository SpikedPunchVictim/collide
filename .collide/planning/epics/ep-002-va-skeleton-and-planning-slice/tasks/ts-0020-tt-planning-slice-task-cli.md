---
id: ts-0020-tt
kind: task
status: done
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
- Code. `core`: `ready.rs` (`readiness(&[Task])`, pure: `ready` ids in input order, `cycles`, `unknown_targets`), `short_id.rs` (`resolve`, `epic_short_form`), and `Task::epic()` in `provider.rs` (the `part-of` target). `host`: `provider_client.rs` (sequential NDJSON JSON-RPC client; notifications skipped; an error reply surfaces with its code and message; the provider's stderr is passed through, so its `plan: ...` count line shows on every command), `checkout.rs`, `pack_location.rs`, `task_args.rs`, `task_view.rs`, `task_cli.rs`, and `bin/collide.rs` (`tasks`, `task`, `help`). Tests: `core/tests/ready_rule.rs` (16), `core/tests/short_ids.rs` (10), `host/tests/task_args.rs` (14), `pack_location.rs` (9), `checkout.rs` (4), `task_cli.rs` (32, the binary over a fixture repo copied to a temp folder, plus a stand-in provider in `host/tests/fixtures/fake-provider/` for conflict, error replies, an exiting provider and a bad answer). `host/tests/common/mod.rs` now holds the bundle helpers that `builtin_packs.rs` had.
- Root: `git rev-parse --path-format=absolute --git-common-dir`, then its parent, so a linked worktree writes into the main checkout (a test checks it). A git directory not named `.git` (bare repository, separate git dir) is an error. No `--checkout` and no `--commit`: `ts-0031-xu`.
- Finding the pack (`pack_location.rs`), in order: `COLLIDE_BUILTIN_PACKS` (a bundle directory; when set it is the only place looked at, so a wrong value fails and never falls back); `app/src-tauri/resources/builtin-packs/` in any folder above the executable (what `pnpm build:packs` stages, found from `target/<profile>/collide` in a checkout or a worktree); `builtin-packs/` beside the executable; `../Resources/builtin-packs/` (the macOS app bundle). A directory is the bundle when it holds `plan/pack.yaml`. The staged folder comes before the copies because the copy Tauri puts in `target/<profile>/` can be older. The error lists every place looked at and names `pnpm build:packs` and the variable. `node` comes from `PATH` through the loader. For `ts-0021`: from a checkout, `pnpm build:packs` once, then `cargo run -p collide-host --bin collide -- tasks --ready` needs nothing else.
- Ready rule, edge cases (decided here): only `blocked-by` blocks, unless the link says `blocking: false`. The rule runs over every item, epics too, so an epic whose status is `ready` is listed by `--ready` (use `--kind task`), and it is applied before `--kind` filters, so a filter never hides a blocker. A `blocked-by` target that is no item makes its item not ready and is reported (`unknownTargets`; a line on stderr). A cycle (or an item that blocks itself) is found whatever the members' statuses: none of its members is ready even when one is `done`; an item that waits on a cycle member is just blocked, not part of the cycle. Cycles and unknown targets, and items the provider could not read (`invalid[]`), go to stderr in both output modes and do not change the exit code.
- Short ids: a full id first, then a short form `N`, `N-xx`, `ts-N` or `ts-N-xx`; the number is compared as a number (so `12`, `012`, `0012` agree) and a prefix or suffix, when given, must equal the item's. Exactly one match resolves; several (listed with their epics, sorted by id) exit 3 and ask for the suffix or prefix; none exits 3 naming what was tried; text that is no id form exits 2. Epics and tasks share numbers (`ep-002-va` and `ts-0002-bj`), so a bare `2` matches both and is ambiguous: use `ts-2` or `ep-2`. Not changed, since the criterion says "exactly one item matches"; a bare number could default to tasks if that is wanted.
- Output. `tasks`: one line per item, `<epic> / <id>  <status>  <kind>  <title>` in padded columns (an epic shows just its id); a `ready`-status item the rule holds back shows `ready (blocked)`. `task <id>`: heading, `title`, `kind`, `status`, `ready` with the reason (`no (waiting on ts-0002-bb (ready))`, `no (in a blocked-by cycle: ...)`, `no (status is done)`), `owners`, `path`, `links` (each target as `blocked-by ts-0009-cu (ep-002)`, `(not found)` for an unknown one, no bracket for an epic; the `part-of` link is not repeated, as the heading shows the epic), then the body. `--json`: `tasks --json` is `{tasks, invalid, cycles, unknownTargets}`, each task being the provider's task plus `epic` (id or null) and `ready`; `task <id> --json` is that one task.
- Ops: `collide task <id> <op> [<value>] [name=value ...] [--json]`. A bare value sets the op's only required property, else the only required property of the first `oneOf` alternative (`setStatus active` sets `status`), else its only property; `array` and `object` values are JSON; the declared `type` decides the rest. The CLI checks unknown names, required names, `oneOf`, `enum` and primitive types against the declared schema (no JSON Schema crate was added); lengths and patterns are the provider's, and it answers `refused`. An unknown op exits 2 and lists the declared ops with their words. The op is applied with the `version` that `get` just returned. Output: `ok` prints what changed (status, owners, links, compared before and after) and the file, or "nothing to change"; `conflict` prints the current item on stderr and says nothing was written; `refused` prints the provider's message on stderr; with `--json` the provider's result goes to stdout instead, and the exit code is the same.
- Exit codes: 0 ok; 1 failed (no repository, no bundle, no `node`, the provider did not start, answered badly or exited); 2 wrong command line (option, op, argument, unknown kind, malformed id); 3 an id names no item or several; 4 conflict; 5 refused. Documented in `collide help`.
- Not done, by design: a timeout on the provider (a hung provider hangs the CLI until the supervisor exists); `--checkout`, `--commit` (`ts-0031-xu`); `create` and `reid` (`ts-0030-qw`); the full command tree (`ts-0041-xf`).
- Hand check against this repo (read-only: `collide tasks --json` and `--ready`): 51 items listed (6 epics and 45 tasks, which is every `epic.md` and task file under `epics/`), `invalid` empty, no cycle, no unknown target. `collide tasks --ready` gave 13 items, equal to the set computed independently from the frontmatter by a script that applies the rule to each file's `status` and `blocked-by` lines: `ts-0013-nz`, `ts-0016-ka`, `ts-0024-uv`, `ts-0025-st`, `ts-0026-es`, `ts-0032-pp`, `ts-0036-xt`, `ts-0037-vg`, `ts-0044-xq` and the epics `ep-003-da` to `ep-006-wq`.
