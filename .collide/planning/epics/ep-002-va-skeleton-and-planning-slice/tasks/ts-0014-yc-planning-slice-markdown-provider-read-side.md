---
id: ts-0014-yc
kind: task
status: done
title: "Planning slice: markdown provider, read side"
owners: []
links:
  - { type: blocked-by, to: ts-0009-cu }
  - { type: blocked-by, to: ts-0010-yb }
  - { type: blocked-by, to: ts-0028-hq }
---

## Goal

The markdown provider's read side, in TypeScript in `packs/plan`, on the SDK's working `defineProvider`. It reads the shared root at its default path, `.collide/planning/`, and in this stage only under `epics/`: `epics/<epic>/epic.md` and the `.md` files directly in `epics/<epic>/tasks/`. It never walks anywhere else, and it does not read `spikes/`, `proposals/` or `questions/` yet (Stage 4c adds them). Kinds `task` and `epic`; the epic comes from the folder (`part-of`, never from frontmatter); `blocked-by` links; ids of the form `ts-0012-ua`, always strings; an optional `alias` is shown and never linked to; task statuses are the five categories. Frontmatter is parsed with `yaml` in core mode (never `gray-matter`, never 1.1 mode), with duplicate keys and the alias limit on, every field's JavaScript type checked and no coercion or `Date`. The frontmatter block is capped before parsing (S5 suggests 64 KiB), not the file, and the body is never parsed as YAML. The body has its own cap, chosen here and recorded in the item's Notes. Protocol v0 `initialize`, `list` and `get`.

## Success criteria

- `list` returns every hand-written item under `epics/` in Collide's own `.collide/planning/`, with no `task.invalid`. It does not read `spikes/`.
- An item with an unknown kind or a broken link is counted, not dropped.
- A task file moved into another epic's folder is listed under that epic, with its links unchanged.
- Two files with one id are a `task.invalid`.
- An id that does not match `^[a-z]{2}-[0-9]{3,}-[a-z]{2}$` is a `task.invalid` that says so.
- Frontmatter with an unquoted hash such as `1e10`, a duplicate key, an alias bomb, or a frontmatter block over its cap is a `task.invalid`, and a quoted hash stays a string.
- A long body under the body cap is valid. A body over it is a `task.invalid`.
- `initialize` declares the kinds `task` and `epic`, the five categories as the task statuses, and `blocked-by` as the one blocking link.

## Tests

Write first: provider contract tests over fixture items (run the provider over stdio with the SDK's fake client); epic membership from the folder; a moved file; a duplicate id; a bad id; the frontmatter hazards; a frontmatter block over its cap beside a long body; `spikes/` present and ignored.

## Notes

- IMPLEMENTATION_PLAN.md, 0.10 (the markdown provider's layout, kinds and ids, frontmatter, Collide's own build tasks), 0.9 (YAML and frontmatter are untrusted input), Stage 2: Skeleton, Planning slice. Spike S5's record for the parser findings.
- Area: `packs/plan` (the provider, TypeScript, run by `node`).
- Done when the criteria hold, including `list` over this repository's own `epics/`.
- Body cap: 256 KiB (262,144 bytes), five times the largest record in this repo (51 KB) and small enough that one `get` reply stays reasonable. The frontmatter cap is 64 KiB, as S5 suggests. Both are checked from the file's head and size, so a file over either cap is never read whole. `list` leaves `body` out (every item comes in one reply), and `get` returns it.
- Choices the plan leaves open: a task's key is `plan:<id>`; scope is `ref`; an epic has the same five statuses as a task; frontmatter keys other than the seven named ones are kept in `fields` and must be a string, a boolean or a list of strings (a number is refused, so an unquoted hash is caught); of two files with one id the first in path order is the item; an epic folder must start with an epic id; symbolic links are not followed.
- For `ts-0029-ea` (bundling): `yaml` 2.9.1 is a runtime dependency of the pack, and so is `@collide/pack-sdk`, which resolves through its built `dist/`. `packs/plan`'s `test` and `typecheck` scripts build the SDK first (`pnpm sdk`).
- Owner decisions (owner, 2026-11-14): `initialize` receives `root`, the main checkout's absolute path, and the provider reads `<root>/.collide/planning` from it, never from its working directory (`list` and `get` fail until `initialize` has run); `list` returns `invalid[]` (`{path, checkout, reason}`, the `task.invalid` shape) beside tasks and collections; `get` returns no task for an unknown id, as an answer and not an error. Core: `InitializeParams.root` is required, `ListResult.invalid` defaults to empty like `collections`, `GetResult.task` is optional. The provider is `packs/plan/src/main.ts`, run by `node`; `checkout` is ignored for now and written back as null on `invalid`; unknown kinds and broken links are listed as they are and counted on stderr.
