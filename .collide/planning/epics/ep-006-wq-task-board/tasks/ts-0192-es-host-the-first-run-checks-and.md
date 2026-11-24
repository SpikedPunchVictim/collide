---
id: ts-0192-es
kind: task
status: ready
title: "Host: the first-run checks and Make it one"
owners: []
links:
  - { type: blocked-by, to: ts-0191-nv }
---

## Goal

`board.open` answers all of 6a's first-run checks: `claude` installed, `claude` logged in, and a git repo. Each
failing check names its fix in Collide's words, with the command to copy. Only a folder that is not a git repo
stops the board from opening, and "Make it one" runs `git init` there. No UI here; `ts-0194-qp` draws it.

What exists.
- `core/src/owner/board.rs`: `FirstRunCheck { id, ok, required, message }`, `CheckId { ClaudeInstalled,
  ClaudeLoggedIn, GitRepo, GhLoggedIn }`, `BoardOpenParams { root, init_git }`, `BoardOpenResult { board, checks }`.
- `board_open` in `host/src/daemon.rs` produces only the `git_repo` check (`boards::git_repo_ok`,
  `git_repo_failed`) and refuses `init_git` with "not built yet (Stage 6a)". `host/tests/scratch_tasks.rs`
  `init_git_is_refused_as_not_built_yet` tests the refusal.
- `host/src/claude_versions.rs` reads the installed `claude`'s version for `daemon.agent_versions`, over the
  daemon's captured `PATH` (`captured_path`).
- No command is known that says whether `claude` is logged in without spending. `ts-0185-ge` has the owner measure
  one.

What this adds.
- `claude_installed`: found on the captured `PATH`, with its version; when missing, the fix names the install
  command (`npm install -g @anthropic-ai/claude-code`) for the copy button. Not required.
- `claude_logged_in`: the probe the owner recorded in `ts-0185-ge`, run with a short timeout. Logged out names the
  fix ("Run `claude` in a terminal and log in."), with `claude` to copy. When no probe was recorded, or it times out,
  the check says "could not check" and is not a failure. Not required. Left out when `claude` is missing.
- `git_repo`: as now, required.
- `init_git: true` on a folder that is not in a checkout runs `git init` there (with the daemon's git environment),
  then opens it. On a folder already in a checkout it does nothing more. The refusal test becomes a test of this.
- `board.checks { root }`: the checks alone, for the page's "Check again", with no board opened.
- `gh_logged_in` is not produced: it arrives with the `github` pack in Stage 5 (`ts-0172-xc`).

## Success criteria

- A folder that is not a git repo answers no board and a failed, required `git_repo`. With `init_git` it becomes a
  repository and opens.
- A missing `claude` is named with its install command; the board still opens.
- A logged-out `claude` (by the recorded probe) is named with its fix; the board still opens.
- With no probe recorded, `claude_logged_in` says "could not check" and is not a failure.
- `board.checks` runs the checks with no board opened and nothing written.

## Tests

Write these first.

- `host/tests/`: a fake `claude` on the daemon's `PATH`, in three forms (missing, logged out, logged in by the
  recorded probe's exit code and output), each with a temp repo; a plain folder; `init_git` on it; `init_git` on a
  repo; a probe that hangs and is cut at the timeout.
- `host/tests/scratch_tasks.rs`: `init_git_is_refused_as_not_built_yet` becomes a test that `init_git` makes the
  repository and opens it.
- `core`: serde of the checks and of `board.checks`.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 6a: No-setup open (the checks, Make it one). Success Criteria: "A folder that is not a git repo does not
    open, and offers Make it one. A missing or logged-out `claude` is named with its fix and a copy button, and the
    board still opens." Tests: "the no-setup open and first-run checks".
  - 0.11: First run works with no pack and no tasks (revision 25). R5.
- Design: note 13. The words come from the mock's `welcomeHtml` rows.
- Carried: `ts-0075-hj` ("`git init` for a folder that is not a checkout, and the `claude` checks (Stage 6a)").
- **Owner's choice, recommendation taken (2026-11-24): the logged-in probe.** See `ts-0185-ge`. Recommend running the
  command the owner recorded, with "could not check" until one is recorded. Alternative: reading files `claude` keeps
  under `HOME`; private state that changes without notice.
- **Decision for the item, with a recommendation: Check again.** Recommend `board.checks`, so a check runs without
  opening a board. Alternative: calling `board.open` again; it opens the board as a side effect.
- **Decision for the item, with a recommendation: where the probe lives.** Recommend the Claude adapter's code
  (`host/src/adapters/` or beside `claude_versions.rs`), behind a small trait the tests replace with a fake `claude`
  script on `PATH`. Alternative: a free function in `daemon.rs`; harder to fake.
- Area: `core/src/owner/board.rs`, `host/src/daemon.rs` (`board_open`), `host/src/boards.rs` (or where the checks
  live), `host/src/claude_versions.rs`, `schemas/` and the generated files, `host/tests/`.
- Dependencies: none.
- The test rule: no test touches the real home, real launchd, a real daemon, the keychain or the network. Each test
  that runs the `collide` binary clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to
  a temp root, as `host/tests/common/mod.rs` `TempRoot` does. A temp repository sets `GIT_CONFIG_GLOBAL` and
  `GIT_CONFIG_SYSTEM` to empty files. Keychain writes go through `MemorySecretStore` (`host/src/secret_store.rs`),
  never the login keychain. The `claude` the checks find is a fake script on the test's `PATH`; the real one is
  never found, because `PATH` is the test's.
- Stage 6a item (revision 25 scope, split 2026-11-24). Items of Stage 6a run one at a time, in id order, because they
  share `app/`, `core`, `host` and the generated files. No builder runs `tauri build`.
- Nothing here runs a real `claude`.
- Owner decisions: none recorded yet. The recommendations stand until the owner answers at the split.
