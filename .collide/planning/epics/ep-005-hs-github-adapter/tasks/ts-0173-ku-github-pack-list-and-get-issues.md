---
id: ts-0173-ku
kind: task
status: ready
title: "GitHub pack: list and get issues and pull requests"
owners: []
links:
  - { type: blocked-by, to: ts-0172-xc }
---

## Goal

The pack reads the issues and pull requests of the board's repositories into tasks: a full `list`, a `get` by id, the
status mapping, the provenance, and the reason a task cannot be launched.

What exists. `ts-0172-xc` gives the pack, the token, the client (paging, ETag, failure classes), the declared
contract, the fake `gh` and fake API, and the fixtures; `list` answers empty and `get` answers none. `ts-0168-tq`
gives `board.repos`, `board.origin` and `Task.provenance`. `core/src/provider.rs` has the `Task` and `ItemState`
shapes. The corpus reconciles a full `list` against its cache and removes what is absent, for a source that is not
stale (`reconcile_by_source`).

What this adds.
- `list`: for each repo in `board.repos`, two listings with `per_page=100`: every open issue
  (`state=open&sort=updated&direction=desc`) and the issues closed in the last `closedDays` days
  (`state=closed&since=<now - closedDays>`). A closed issue older than the window is not in the corpus. The issues
  endpoint also returns pull requests (an entry with a `pull_request` object), and they are kept.
- The task for an issue: key `github:<owner>/<repo>#<n>`, id `<owner>/<repo>#<n>`, `title`, `body` (null reads as
  none), `version` the issue's `updated_at`, `labels` the label names, `owners` the assignees' logins, `links` none,
  `kind` `issue` or `pull-request`, and `fields` `repo`, `number`, `url` (the `html_url`), `state_reason`,
  `launchable` and `launch_reason`. Status: open is `open` (category ready); closed with `state_reason` `not_planned`
  is `not_planned` (dropped); any other closed is `closed` (done). Milestones are not mapped to collections.
- `provenance`: `author` is the issue's `user.login`, `association` its `author_association`.
- Launchability: an issue is launchable only when its repo is `board.origin`. Any other repo's issue has
  `launchable: false` and a `launch_reason` ("No board here for <repo>: launch from a board in that repository"), and
  so has every pull request ("A pull request is read-only here"). With no origin the reason says the checkout has no
  GitHub origin. Core reads neither field yet.
- `get(id)`: `GET /repos/<repo>/issues/<n>`; it returns the same task as `list`. A 404 or 410 (deleted) and a 301
  (transferred) answer "no such task". An id of a repo that is not in `board.repos` answers none.
- Limits: a listing that would pass 1000 issues for one repo stops with a `source.stale` of cause `other` whose reason
  says to lower `closedDays`; nothing partial is returned.
- A repo that cannot be read (404, 401, 403, network) fails the whole `list` with the cause classes of `ts-0172-xc`, so
  the corpus never reconciles against a partial list. The rate-limit headers are read; at zero remaining the call
  fails with `rate_limit` and the reset time.

## Success criteria

- `list` over two fixture repos returns each issue with the right key, id, title, labels, assignees, version, `fields`
  and status: open, closed as completed, and closed as not planned map to `open`, `closed` and `not_planned`, and so to
  ready, done and dropped.
- A pull request in the issues list is a `pull-request` task, `launchable` false, with its reason. An issue of the
  origin repo is launchable; an issue of another repo is not, and says why. A null body gives no body.
- The eight `author_association` values of the fixtures reach `provenance.association`; the daemon's view shows the
  trust level of `ts-0168-tq` for them.
- Paging over three pages returns all issues once. A closed issue outside the window is absent. A listing over the cap
  is a stale source with a reason, not a short list.
- `get` returns the task `list` returned, and answers none for a deleted, transferred or unknown id.
- With one repo unreadable the whole `list` fails with the cause, and a core reconcile against it removes nothing.

## Tests

Write these first.

- `packs/github/test/` mapping tests from fixture issues to tasks (a table over status, state reason, null body, labels,
  assignees, association, pull request, origin and other repo, no origin).
- Provider tests over stdio against the fake API: `list` with paging, the closed window (the fake checks `since`),
  the cap, an unreadable repo, `get` for found, 404, 410 and 301.
- A daemon test in `host/tests/` with the real pack on a temp board, the fake `gh` on the login shell's `PATH` and the
  fake API on loopback: `collide tasks` lists the issues, and `task.list` carries the provenance trust.

## Notes

- Area: `packs/github/src/`, `packs/github/test/`, `host/tests/`.
- The test rule: no test touches the real home, real launchd, a real daemon, the keychain or the network. Each test that
  runs the `collide` binary clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp
  root, as `host/tests/common/mod.rs` `TempRoot` does. A temp repository sets `GIT_CONFIG_GLOBAL` and
  `GIT_CONFIG_SYSTEM` to empty files. No test, build or builder runs the real `gh`, reads a real token or reaches
  GitHub: `gh` is a script in the test's own temp folder, first on the `PATH` the test builds, and the API is the fake
  server of `packs/github/test/support/` on a loopback port.
- Owner's choices in the proposal of the split: the closed window (recommend 30 days; the alternative is every closed
  issue, which costs many pages on an old repo), and "launchable means the board's own repository" (recommend; the
  alternative is every repo named in `board.yaml`, which Stage 9a cannot launch across boards anyway).
- Decisions: all-or-nothing `list` on an unreadable repo (recommend; the alternative, one stale mark for each repo, has
  no place in the protocol). A hand-made `fields.launchable` is the recommendation over a new core field, because
  cross-board launch is Deferred in the plan.
- Stage 5 items run one at a time, in id order. Nothing here runs a real `claude`. No dependency is added.
- Owner decisions: none recorded yet. The recommendations stand until the owner answers at the split.
