---
id: ts-0172-xc
kind: task
status: ready
title: "GitHub pack: the pack, the gh token, the API client and the scope check"
owners: []
links:
  - { type: blocked-by, to: ts-0171-ds }
---

## Goal

The built-in pack `collide/github` exists, starts on every board, reads the person's `gh` token, talks to the REST API
through a small client, and checks at `initialize` that the token can do what Issues needs, naming the fix when it
cannot. It lists nothing yet.

What exists. Nothing of the pack: `packs/` holds only `plan`, and the built-in list is fixed in
`host/src/builtin_packs.rs` (`BUILTIN`, `names()` returns `[&str; 2]`) and in `scripts/build-packs.ts`. The pieces to
build on do exist: `@collide/pack-sdk` (`packages/pack-sdk/src/provider.ts`, `context.ts`) serves the protocol over
stdio, and `packs/plan` (`pack.yaml`, `src/main.ts`, `src/provider.ts`, `test/provider-stdio.test.ts`) is the model of
a pack, its op declarations and its stdio test. A provider gets `initialize { config, root, board }` (`ts-0168-tq`)
with the pack settings through the layers, `COLLIDE_PACK_DATA` as its data folder and the login shell's `PATH`
(`host/src/provider_launch.rs`, `host/tests/base_env.rs`). `ts-0169-mn` gives a per-user setting.
`ts-0171-ds` turns a `source.stale` with a cause into a Queue item.

What this adds.
- `packs/github/`: `pack.yaml` (name `collide/github`, one provider, `config:` keys `account` (string, `perUser`),
  `apiUrl` (string, `perUser`, default `https://api.github.com`), `pollSeconds` (default 30, minimum 1),
  `reconcileSeconds` (default 3600, minimum 5), `closedDays` (default 30, minimum 1); the last three are
  `overridable`), `package.json` (workspace package `@collide/pack-github`, the same dependencies and scripts as
  `@collide/pack-plan`, no new one), `src/main.ts`, and the `tsconfig` and `vitest` files of `packs/plan`.
- Built-in: added to `BUILTIN` in `builtin_packs.rs` (so `names()` returns three), to `scripts/build-packs.ts` and to
  wherever `plan`'s built bundle is listed for the app (`app/src-tauri/tauri.conf.json`). Its tests that count the
  built-ins are updated.
- `initialize` declares scope `global`, id pattern `<owner>/<repo>#<number>`, kinds `issue` and `pull-request`, statuses
  `open`, `closed` and `not_planned` with `categoryStatus` (ready is `open`, done is `closed`, dropped is
  `not_planned`), the fields `repo`, `number`, `url`, `state_reason`, `launchable` and `launch_reason`, the ops
  `setStatus` and `comment` (declared; `ts-0175-gz` implements them), and `capabilities.watch` true, with no `create`,
  `collections` or `preview`. `list` answers an empty list and `get` answers none until `ts-0173-ku`.
- A board with no repos and no GitHub `origin`: the provider answers an empty list, runs no `gh`, makes no request and
  is never stale. Opening such a board costs nothing and shows no error (0.11, no-setup open).
- No-setup open's GitHub source (moved from 6a; revision 25): a board whose `board.yaml` names no repos takes its
  `origin` repository as its source when `origin` is on GitHub and `gh` is logged in, and shows a hint otherwise. The
  first-run page gains the optional `gh` check (`CheckId::GhLoggedIn` in `core/src/owner/board.rs`).
- The token: `src/gh.ts` runs `gh auth token --user <account>` with `node:child_process` `execFile` (no shell), with
  the provider's own `PATH`. An unset `account` runs `gh auth token` for the account `gh` has active. The token stays
  in memory, never in a log, an error, the pack data or a task. A failure gives cause `auth` and the words "run `gh
  auth login`" (with `--user <account>` when set).
- The client: `src/api.ts` is a thin wrapper on Node's `fetch` with `Authorization`, `Accept:
  application/vnd.github+json`, `X-GitHub-Api-Version`, a User-Agent of `collide-github`, a timeout, `Link` next-page
  following, ETag and `If-None-Match` support (used by `ts-0174-wf`) and rate-limit headers read on every response.
  It classifies failures: network, 401 as `auth`, 403 with `x-ratelimit-remaining: 0` as `rate_limit` (with the reset
  time), 404 as `missing` (a private repository the token cannot see, a renamed or deleted one), other as `other`.
  Every error text is scrubbed of the token.
- `apiUrl` may only be `https://api.github.com` or a loopback address (`http://127.0.0.1:<port>` or
  `http://localhost:<port>`); anything else makes `initialize` fail with the reason. The token is never sent to another
  host. A repo's files cannot set it (per user).
- The scope check at `initialize`, once for each repo: `GET /repos/<repo>`. A classic token's `X-OAuth-Scopes` must
  hold `repo` for a private repo; for a public one nothing is needed to read. A fine-grained token has no scopes
  header, so it is judged by the answer to `GET /repos/<repo>/issues?per_page=1` (404 or 403 names "the token needs
  Issues: read and write on <repo>"). A failure sends `source.stale` with cause `auth` (or `missing`), names the fix
  (`gh auth refresh -s repo`) and answers requests with an error. The write scope is judged at the write, in
  `ts-0175-gz`.
- Test support in `packs/github/test/support/`: a fake `gh` (a script written into the test's own temp folder that
  prints a fixed fake token for `auth token [--user <name>]`, exits 1 for an unknown account or in an "expired"
  mode, and logs each call) and a fake GitHub API on a loopback `node:http` server (serves the
  fixtures, counts requests, speaks ETag, `Link`, `X-OAuth-Scopes` and the rate-limit headers, and keeps mutable
  state for the later items).
- Fixtures in `packs/github/test/fixtures/`, written by hand from GitHub's documented REST shapes (a repo, an issue
  list page, an issue, a pull request in the issues list, an issue event, the error bodies), trimmed, with invented
  logins (`octocat`) and repositories (`acme/edge`). Each file names the REST page it follows in its first key.

## Success criteria

- `collide/github` is a built-in: `names()` has three, the bundle is built, and a board with no `sources.github`
  block and no GitHub `origin` lists it with no tasks, no stale mark and no `gh` call (the fake `gh`'s log is empty).
- A board with no repos whose `origin` is on GitHub lists the origin's issues with the fake `gh` logged in; with it
  logged out, it shows the hint, not a stale source. The first-run page's `gh` check passes and fails with the fake.
- `initialize` returns the declared contract, which the protocol's schema accepts, and `collide config get
  collide/github.account` shows it as a per-user setting.
- With a fake `gh`, the token is read with `--user <account>`; with no account, without it. A failing `gh` gives a stale
  source with cause `auth` and the sign-in words, and the token appears in no output of the test.
- A classic token without `repo` on a private fixture repo gives cause `auth` and the fix `gh auth refresh -s repo`. One
  with it passes. A fine-grained token with a 404 answer names the permission it needs. A repo that is gone gives
  `missing`.
- `apiUrl` of `https://example.com` is refused; the fake server's loopback address is accepted.
- The client follows a three-page `Link`, sends `If-None-Match` after an ETag, treats a 304 as "no change", reads the
  rate-limit headers, and classifies network, 401, rate-limited 403 and 404.

## Tests

Write these first.

- `packs/github/test/` unit tests for `gh.ts` (account, no account, failure, scrubbing) and `api.ts` (each failure
  class, paging, ETag, timeout) against the fake server.
- `packs/github/test/provider-stdio.test.ts` in the shape of `packs/plan/test/provider-stdio.test.ts`: the pack over
  stdio through `initialize`, no repos, a scope pass and each scope failure.
- `host/tests/builtin_packs.rs` extended for the third built-in; a `host/tests/` case that opens a temp board with no
  github block and runs `collide tasks`, with a fake `gh` first on the `PATH` the login shell gives (`.zprofile`) and
  its log asserted empty.
- The schema and settings checks of `pack.yaml`.

## Notes

- Area: `packs/github/`, `host/src/builtin_packs.rs`, `scripts/build-packs.ts`, `app/src-tauri/tauri.conf.json`,
  `pnpm-workspace.yaml` (`packs/*` already covers it), `host/tests/`.
- The test rule: no test touches the real home, real launchd, a real daemon, the keychain or the network. Each test that
  runs the `collide` binary clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp
  root, as `host/tests/common/mod.rs` `TempRoot` does. A temp repository sets `GIT_CONFIG_GLOBAL` and
  `GIT_CONFIG_SYSTEM` to empty files. No test, build or builder runs the real `gh`, reads a real token or reaches
  GitHub: `gh` is a script in the test's own temp folder, first on the `PATH` the test builds, and the API is the fake
  server of `packs/github/test/support/` on a loopback port. There is no environment variable that points the pack at
  a fake; the seams are `apiUrl` and `PATH`.
- Dependencies: none. Node's `fetch`, `node:child_process` and `node:http`, and the `vitest` and `typescript` the repo
  has. The alternative, `@octokit/rest` (the builder checks the version), would bring a retry and paging layer that
  the pack must still wrap and test; it is an Owner's choice only if the owner wants it. Test mock libraries (`nock`,
  `msw`) are not used.
- The pack name is `collide/github`, as `collide/plan` is; the plan's 0.10 table says `github`. The source and key
  prefix are `github`. A text change for 0.10 is in the proposal of the split.
- The fixtures are hand-written from the documented shapes, because no builder may reach GitHub. `ts-0177-hd` compares
  them with real responses and each difference becomes a fix item.
- Stage 5 items run one at a time, in id order. Nothing here runs a real `claude`.
- Owner decisions: none recorded yet. The recommendations stand until the owner answers at the split.
