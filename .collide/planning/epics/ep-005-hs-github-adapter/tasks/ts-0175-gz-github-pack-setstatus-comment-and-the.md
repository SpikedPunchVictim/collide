---
id: ts-0175-gz
kind: task
status: ready
title: "GitHub pack: setStatus, comment and the conflict before a write"
owners: []
links:
  - { type: blocked-by, to: ts-0174-wf }
---

## Goal

`setStatus` closes or reopens an issue and `comment` adds a comment. A write with a stale version is found before the
write and answered with `conflict`. A pull request is read-only.

What exists. `ts-0172-xc` declares the ops `setStatus` and `comment` and the statuses `open`, `closed` and
`not_planned` with their categories. `ts-0173-ku` gives the task mapping and the client. `ts-0174-wf` keeps the known
version of each issue. `core/src/provider.rs` has `ApplyParams` (`op`, `id`, `args`, `version`, `actor`) and the three
`ApplyResult`s, `ok` (the task, its version, `items`), `conflict` (the current task) and `refused`. `ts-0170-gh` makes a
global source's write skip the checkout wait and record cause `remote`. `host/src/task_args.rs` reads op arguments from
the declared schema, and the daemon's `judged_write` and `apply_and_record` call the provider and record the event.

What this adds.
- `setStatus`, by category or status name. `done` closes with `state_reason` `completed`; `dropped` closes with
  `not_planned`; `ready` reopens. The status names `closed`, `not_planned` and `open` do the same. `closed` may carry
  a `reason` argument (`completed` or `not_planned`). `backlog`, `active` and `review` are `refused` with the words
  "GitHub Issues have no <category> status". Setting the status an issue already has writes nothing and answers `ok`.
- The conflict check: before any write, `GET /repos/<repo>/issues/<n>` and compare `updated_at` with `params.version`.
  A difference answers `conflict` with the current task and writes nothing. A 404 or 410 answers `refused` ("the issue
  no longer exists"). The write is `PATCH` with `state` and `state_reason`; the answer is `ok` with the task read from
  the PATCH response, its new `updated_at` as the version, and `items` of that one task. The pack records the new
  version, so the next poll sees no change and sends nothing.
- `comment` (`body`, required): `POST /repos/<repo>/issues/<n>/comments`. It does not compare versions: a comment
  cannot overwrite anything. The answer is `ok` with the issue re-read.
- Pull requests: both ops are `refused` ("A pull request is read-only here"). So is any id whose repo is not in
  `board.repos`.
- Write errors: 401 sends `source.stale` cause `auth` and refuses with the sign-in words; 403 or 404 on the write names
  the missing permission (a classic token without `public_repo` or `repo`: `gh auth refresh -s public_repo`; a
  fine-grained token: "Issues: read and write on <repo>"); 422 and other answers are `refused` with GitHub's message,
  truncated and as plain text. The token is in no message.

## Success criteria

- `setStatus` with category `done` sends a PATCH with `state: closed` and `state_reason: completed`, and the result's
  version is the response's `updated_at`. `dropped` gives `not_planned`. `ready` reopens.
- `backlog`, `active` and `review` are refused with the reason, and the fake saw no PATCH.
- A write with a stale version answers `conflict` with the current task, and the fake saw no PATCH or POST. A write
  with the current version is applied.
- A pull request and an id of an unlisted repo are refused, and the fake saw no write.
- `comment` posts the body and answers `ok`. A deleted issue is refused.
- The poll after a write emits nothing for that issue.
- A 403 and a 401 on a write give the messages above with no token in them.

## Tests

Write these first.

- `packs/github/test/` write tests over stdio against the fake API, which counts every request by method: the category
  table, a stale and a current version, closed with each reason, an unchanged status, pull request and unlisted repo,
  comment, a deleted issue, 401, 403, 422, and the poll after a write.
- A daemon test in `host/tests/`: the real pack, `collide task <key> setStatus` over the CLI's op form
  (`host/src/task_args.rs`) closes the fake issue, and a second write with the old version prints the conflict and
  uses the conflict exit code of the task CLI; the event's cause is `remote` and its actor is the CLI's.

## Notes

- Area: `packs/github/src/` (an apply module), `packs/github/test/`, `host/tests/`.
- The test rule: no test touches the real home, real launchd, a real daemon, the keychain or the network. Each test that
  runs the `collide` binary clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp
  root, as `host/tests/common/mod.rs` `TempRoot` does. A temp repository sets `GIT_CONFIG_GLOBAL` and
  `GIT_CONFIG_SYSTEM` to empty files. No test, build or builder runs the real `gh`, reads a real token or reaches
  GitHub: `gh` is a script in the test's own temp folder, first on the `PATH` the test builds, and the API is the fake
  server of `packs/github/test/support/` on a loopback port.
- GitHub has no conditional write. The check is a read before the write, so a change between the read and the PATCH is
  not seen, and two edits in one second share a version. Plan section 0.10 states it. A test must not claim to catch it.
- Plan wording to settle (the proposal of the split): Stage 5 says categories map "through `board.yaml`'s status
  mapping". In the code, `GithubStatusMapping` in `core/src/board.rs` is for Projects, which is Stage 6c. Issues have
  three states, so this item maps them in the pack. Recommend that; the alternative is a `board.yaml` mapping for
  Issues, which has nothing to map but names.
- `comment` skips the version check (recommend); the alternative checks it and makes every comment a conflict when
  anyone else has commented since.
- Run agents: a person-only rule for issues is not set. The kinds the rule covers are proposals and questions
  (`host/src/auth.rs`), and no issue is one.
- Stage 5 items run one at a time, in id order. Nothing here runs a real `claude`. No dependency is added.
- Owner decisions: none recorded yet. The recommendations stand until the owner answers at the split.
