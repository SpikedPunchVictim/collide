---
id: ts-0176-ds
kind: task
status: ready
title: "GitHub with plan: the Stage 5 criteria through the daemon on fakes"
owners: []
links:
  - { type: blocked-by, to: ts-0175-gz }
---

## Goal

The Stage 5 success criteria that a fake can show, shown end to end: the real daemon, the real `collide/github` pack and
the `plan` pack on one temp board, with a fake `gh` and a fake API. It also records what the builders learned in the
pack's own Built sections as one honest account of Stage 5.

What exists. Every part: `ts-0168-tq` to `ts-0175-gz` build the contract fields, the per-user setting, the global
write path, the "could not check" Queue item and the whole pack, each with its own tests at the level of its own
code. `host/tests/daemon_corpus.rs`, `daemon_sources.rs` and `daemon_plan_views.rs` run a daemon over a temp board with
fake providers or the `plan` pack. The views and CEL of Stage 4c (`core/src/view_expr.rs`, `view_eval.rs`) list tasks
of any source. No test yet has both packs on one board or the daemon's start with a GitHub cache.

What this adds. Tests only, and fixes for what they find (each fix is small or becomes a follow-up item).
- A temp board whose `board.yaml` names `sources.github.repos`, whose checkout has an `origin` remote (added with
  `git remote add`, never fetched), with plan items in `.collide/planning/`, a view over both sources, a login-shell
  `.zprofile` that puts the fake `gh` first on `PATH`, and the fake API on loopback with `apiUrl` set in the temp
  user's settings.
- The scenarios below, run through the daemon's socket and the `collide` binary (`collide tasks`, `collide task <key>`,
  the CLI's op form).

## Success criteria

- A view over plan and GitHub tasks lists both (Stage 5, fifth criterion), with the GitHub rows' trust levels from
  their association.
- An issue edited on the fake API shows in `task.list` within a few seconds at `pollSeconds: 1` (the first criterion's
  mechanism; the 60 s measurement is `ts-0177-hd`'s).
- `setStatus` with category `done` through the CLI closes the fake issue, and the same with an old version is a
  conflict (the third criterion).
- An issue deleted on the fake API leaves the corpus within one `reconcileSeconds` (the fourth criterion), and another
  deleted while the daemon was stopped is gone after the next start (revision 22's start-time reconcile).
- With the fake `gh` failing (an expired token) at start, the cached issues stay, none is removed, `task.list` shows the
  stale source with cause `auth`, and `queue.list` has one item. Fixing the fake and answering `Retry` clears it.
- A pull request is listed and read-only, an issue of a repo that is not the origin is listed and not launchable with
  its reason, and a scope failure names the fix.
- A board with no GitHub block and no GitHub `origin` runs no `gh`. A clone whose `origin` is on GitHub, with no
  `.collide/` and the fake `gh` logged in, opens with its origin's issues; logged out, with a hint (revision 25).
- A fake issue whose body holds raw HTML, a `javascript:` link, a `file:` link and an image that fetches a remote URL
  is shown in the drawer as text: nothing runs, nothing is fetched, and the CSP blocks the image (6a's check, in its
  GitHub form; revision 25).
- The test of the settings: `collide config set collide/github.account <name>` reaches the fake `gh` as `--user <name>`
  and the provider restarts once.

## Tests

Write these first.

- `host/tests/daemon_github.rs` with the scenarios above, using the support code of `packs/github/test/support/` (the
  fake `gh` and the fake API) and the `TempRoot` of `host/tests/common/mod.rs`.
- A view file in the temp board and `collide tasks --view` over both sources, in `--json` and in the text form.
- Seen red: each scenario is run once with its code removed (the poll, the reconcile, the conflict check, the stale
  rule) and shown to fail, then restored.

## Notes

- Area: `host/tests/daemon_github.rs`, `host/tests/common/`, `packs/github/test/support/`, small fixes where a test
  shows a fault.
- The test rule: no test touches the real home, real launchd, a real daemon, the keychain or the network. Each test that
  runs the `collide` binary clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp
  root, as `host/tests/common/mod.rs` `TempRoot` does. A temp repository sets `GIT_CONFIG_GLOBAL` and
  `GIT_CONFIG_SYSTEM` to empty files. No test, build or builder runs the real `gh`, reads a real token or reaches
  GitHub: `gh` is a script in the test's own temp folder, first on the `PATH` the test builds, and the API is the fake
  server on a loopback port. A guard test, like the one in `checks/tests/real_tree.rs`, fails if a test of the pack
  resolves `gh` outside its temp folder.
- The `stub-tags.json` list and `FILLED_TAGS` in `checks/tests/real_tree.rs` have no Stage 5 tag today, so no stub lands
  here. If a builder adds one for a deferred piece, it is added to the list in the same commit.
- Stage 5 items run one at a time, in id order. Nothing here runs a real `claude`. No dependency is added.
- Owner decisions: none recorded yet. The recommendations stand until the owner answers at the split.
