---
id: ts-0139-wq
kind: task
status: ready
title: "Follow-ups: raised-by, Keep and the create rule for a run's agent"
owners: []
links:
  - { type: blocked-by, to: ts-0137-pg }
---

## Goal

What a run finds that is not its task's work becomes an item a person reviews, never only a line in the run's last reply
(`IMPLEMENTATION_PLAN.md` 0.10, Follow-ups; revision 24).

What exists. `ts-0128-bb` reads the actor from the connection and puts the run id on the wire, and `judge_write` in
`host/src/daemon.rs` judges every write by it. `ts-0125-uh` made the private root (`<pack data>/planning`), written from
the main checkout only. Ids are drawn free across both roots. The op name `move` is reserved for Share (0.10), and
nothing implements it. `ts-0133-kg` adds `inverse` to link type declarations, and `ts-0135-cx` the pack's views.

What this adds.
- The `raised-by` link type in the markdown provider: from a follow-up to the item whose run raised it, blocking
  nothing, with `inverse` "raised". Before Stage 9d that item is the run's task: quick and file runs are the only runs
  with one.
- The create rule, in the daemon: a `create` whose actor is a run's agent goes to the board's private root, in the
  folder of the run's task's epic, whatever root it names. The daemon adds `raised-by` to the run's task and sets
  `fields.run` to the run id, written as a `run:` frontmatter key, as the provider writes other fields. A `raised-by` or
  `run` the agent sent is replaced, not trusted. When the run's task is not in the board's planning source, only
  `fields.run` is written.
- Keep: the markdown provider's `move` op for one private item. The item (its folder, for a kind that has one) moves to
  the shared root's folder for the same epic, under the same id, and `record` commits exactly those files when
  `--commit` is given. It is refused for a proposal, which is shared whole (Stage 12), and for an item whose links name
  a private item, naming that item. While the root is busy it waits like any write (exit 11). A run's agent is refused;
  the app and the CLI may. The Share preview, the move points and the handoff note built at Share stay in the
  second-pack stage.
- The built-in view `follow-ups`: open private items with a `raised-by` link, newest first.

## Success criteria

- A `task new` from a run's agent (a process with `COLLIDE_RUN` in its tree) lands in the private root, in its task's
  epic folder, with `raised-by` to the run's task and `run:` set, even when it asked for the shared root or sent another
  `raised-by`.
- A `task new` from a run whose task is not in the board's planning source has `run:` and no `raised-by`.
- The same `task new` from the CLI in a terminal goes where it asks, with nothing added.
- `move` from the CLI moves a private task and a private question to the shared root under the same id, with their links
  intact; with `--commit` the commit holds exactly those files.
- `move` of a proposal, or of an item linked to a private item, is refused with a message naming why, and nothing moves.
- `move` from a run's agent is refused with a message that names who may do it, and nothing moves.
- `collide tasks --view follow-ups --json` lists the open private items with a `raised-by` link, and not a dropped one
  or a shared one.

## Tests

Write these first.

- Daemon tests with a fake process tree for each actor: the create rule, the replaced fields, a run whose task is
  outside the planning source, the CLI's create unchanged.
- Provider tests for `move`: a task, a question, a spike folder; the epic folder it lands in; an id already in the
  shared root, a proposal, and an item linked to a private item (each refused); links kept; the `--commit` file list.
- The view over a fixture; `raised-by` in the link type declarations and the generated schema.
- The existing daemon, provider and CLI tests unedited.

## Notes

- IMPLEMENTATION_PLAN.md: 0.10 (Follow-ups; Share; the planning links); 0.2 (Who is asking); Stage 4c (Follow-ups, its
  success line); the revision 24 entry.
- **Decision for the item, with a recommendation: how a run's create reaches the private root.** A run's agent works in
  a worktree, and `ts-0125-uh` writes the private root from the main checkout only. Recommend that the daemon writes it
  there, since the private root is the board's, not a checkout's. Alternative: hold the create until accept, which loses
  the follow-up when the run is discarded.
- **Decision for the item, with a recommendation: `raised-by` names the run's task**, and the run id goes in
  `fields.run`. Alternative: a link to the run, which is not an item.
- **Decision for the item: Dismiss is `setStatus dropped`** (Stage 9a's review) and leaves the item private. Nothing to
  build for it here.
- If the view language cannot yet say "private" or "has a `raised-by` link" (`ts-0135-cx`), add the fields the view
  needs to its input, additively, and name them under Built.
- Plan… and Spike… runs (Stage 9d) will name a proposal or a spike as the run's item, and the view will leave out an
  item a Plan… proposal includes. Nothing for that here.
- Area: `host/src/daemon.rs` (`judge_write`, `task_apply`), `host/src/task_daemon.rs`, `packs/plan/src/` (links, a
  `move` op, views), the generated schemas and TypeScript if a wire shape changes, `host/tests/`, `packs/plan/test/`.
- The test rule: no test touches the real home, real launchd or a real daemon. Each test that runs the `collide` binary
  clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as
  `host/tests/common/mod.rs` `TempRoot` does. Settings files are fixtures, never the owner's own `settings.json`. A temp
  repository sets `GIT_CONFIG_GLOBAL` and `GIT_CONFIG_SYSTEM` to empty files.
- Stage 4c item, added by revision 24 (2026-11-18). Built after `ts-0137-pg` and before `ts-0140-gk`; `ts-0138-pn` waits
  for both.
- Nothing here runs a real `claude`. No dependency is added.
- Owner decisions: none recorded yet. The recommendations stand on the owner's delegation.
