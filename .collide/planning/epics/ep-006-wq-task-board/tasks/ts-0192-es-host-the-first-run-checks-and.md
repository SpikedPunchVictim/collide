---
id: ts-0192-es
kind: task
status: done
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
- Owner decisions (2026-11-24): the owner took the recommendations at the split.

### Built

- Wire. `core/src/owner/board.rs`: `FirstRunCheck` gains `copy` (the command the fix names, omitted when none) and
  `could_not_check` (omitted when false); `BoardChecksParams { root }` and `BoardChecksResult { checks }`; the docs of
  `BoardOpenParams` and `BoardOpenResult` say what `init_git` does and that only `git_repo` stops a board.
  `core/src/owner.rs` and `core/src/schema.rs`: the method `board.checks` (kind Board) and its doc row;
  `app/src/owner/methods.ts` has it. `pnpm gen` was run: new `BoardChecks{Params,Result}` files, and
  `BoardOpenParams`, `BoardOpenResult` and the index changed, in `schemas/` and `packages/pack-sdk/src/generated/`.
- The probe. `host/src/adapters/claude/auth_status.rs` (new): `Login`, the trait `LoginProbe`, `AuthStatus` (runs
  `claude auth status` with exactly the daemon's child environment), `NotChecked` (never starts `claude`),
  `probe_from_env` and `parse_status` (reads only `loggedIn`; exit 0 with `true` and exit 1 with `false` are answers,
  anything else is "could not check"; no reason holds the output). `host/src/bounded_run.rs`: `run_bounded_in_group`
  (the child leads a process group, and a run that is cut ends the group, so the runtime `claude` starts goes with it).
  `host/src/adapters/claude/version.rs` `VersionCache::version_within` and `host/src/claude_versions.rs`
  `ClaudeVersions::read_within` (the version read with the checks' own limit; the cache is shared with the daemon's
  start read).
- The checks. `host/src/first_run.rs` (new): `claude_checks` (finds `claude` on the captured `PATH` with
  `file_run::find_on_path`, now `pub(crate)`; reads its version and runs the probe side by side, each for at most
  `CHECK_TIMEOUT`), `ClaudeProbes`, `INSTALL_COMMAND`. `host/src/boards.rs`: `in_checkout` and `init_repository`
  (`git init --quiet` with the daemon's git environment). `host/src/daemon.rs`: `board_open` (now takes the session),
  `board_checks`, `absolute_root`, `make_repository`, `claude_first_run_checks`, `first_run_checks`; the options
  `claude_login` and `claude_check_timeout`. `host/src/bin/collide-daemon.rs` gives `probe_from_env`.
- Order of the checks: `git_repo`, `claude_installed`, then `claude_logged_in` (left out when `claude` is missing). A
  folder that is not a repo still gets all of them.
- How long the checks add to an open. Before the first capture of the base environment ends, an open waits for it, up
  to the capture's timeout and the 5 s margin the other calls use. After it: nothing when `claude` is missing, else at
  most `CHECK_TIMEOUT`, 5 s, because the version read (cached after the daemon's start read) and the probe run side by
  side. `claude` answers in about a second; a start that takes longer says "could not check", never "logged out".
- Tests, with what was seen red at runtime. Each group was run against a stub of the old behaviour, failed, and the
  stub was removed (the files were compared with a copy afterwards).
  - `host/tests/first_run_checks.rs` (17). All red against the old `board_open` (only `git_repo`, `init_git` refused,
    no `board.checks`): `a_missing_claude_is_named_with_its_install_command_and_the_board_opens`,
    `a_logged_out_claude_is_named_with_its_fix_and_the_board_opens`,
    `a_logged_in_claude_passes_both_checks_and_the_account_is_never_returned`,
    `the_checks_run_only_the_version_and_auth_status_with_the_daemons_own_environment`,
    `a_probe_that_hangs_is_cut_with_its_children_and_says_could_not_check`,
    `an_answer_that_does_not_parse_or_disagrees_with_its_exit_code_says_could_not_check`,
    `a_daemon_with_no_probe_recorded_says_could_not_check_and_runs_no_status_command`,
    `a_claude_that_prints_no_version_is_still_installed_and_says_so`,
    `a_folder_that_is_not_a_repo_answers_no_board_and_a_failed_required_git_repo_with_the_claude_checks`,
    `init_git_on_a_folder_that_is_not_a_repo_makes_it_one_and_opens_it`,
    `init_git_on_a_folder_in_a_checkout_does_nothing_more`, `init_git_on_a_folder_that_is_gone_makes_nothing`,
    `init_git_that_git_refuses_is_an_error_naming_git_init_and_opens_nothing`,
    `board_checks_opens_nothing_and_writes_nothing`,
    `board_checks_and_board_open_answer_the_same_checks`,
    `board_checks_on_a_folder_inside_a_checkout_answers_the_checkouts_git_repo`,
    `board_checks_refuses_a_root_that_is_not_an_absolute_path`. Also red, one stub each: with `git init` run even in a
    checkout, `init_git_on_a_folder_in_a_checkout_does_nothing_more`; with `board.checks` recording a board,
    `board_checks_opens_nothing_and_writes_nothing`; with the probe run in no group of its own,
    `a_probe_that_hangs_is_cut_with_its_children_and_says_could_not_check` (the sleeper outlived the cut).
  - `host/tests/scratch_tasks.rs`: `init_git_is_refused_as_not_built_yet` became
    `init_git_makes_the_repository_and_opens_it` (red against the refusal).
  - `host/tests/scratch_tasks.rs`:
    `a_folder_that_is_not_a_checkout_opens_no_board_and_fails_the_git_repo_check_with_the_fix` no longer expects
    `git_repo` to be the only check: it expects it first and the rest not required.
  - `host/tests/daemon_corpus.rs`: `a_run_agent_may_not_make_a_folder_a_repository_and_nothing_is_made` (red with the
    refusal removed: the agent's call made the repository).
  - `host/tests/bounded_run.rs`: `a_run_in_its_own_group_that_times_out_ends_the_children_it_started_too` (red with
    the group not ended). `a_run_in_its_own_group_that_ends_in_time_answers_as_run_bounded_does` is a guard that
    cannot be red before the function exists.
  - `host/src/adapters/claude/auth_status.rs` unit tests (7): red with the exit code ignored,
    `a_disagreeing_exit_code_is_could_not_check`, `a_reason_never_holds_the_output`; and with `parse_status` reading
    the answer as a struct, `output_that_is_not_the_json_is_could_not_check` (the form `[true]` read as logged in; it
    is why `parse_status` reads a JSON value). `a_daemon_with_no_root_override_runs_auth_status` is red with the two
    arms of `probe_from_env` swapped; `a_daemon_under_an_overridden_root_never_asks_claude` is the other half.
    `exit_0_with_logged_in_true_is_logged_in` and `exit_1_with_logged_in_false_is_logged_out` describe what must stay
    true and are not red.
  - `host/src/first_run.rs` unit tests (2): `with_no_environment_to_look_in_both_checks_say_could_not_check` and
    `an_environment_with_no_path_finds_no_claude` cover a new function and were not run against a stub.
  - `core/tests/owner_protocol.rs`: the `board.checks` fixture row (the existing round-trip tests read it) and
    `a_check_with_no_command_and_one_that_ran_writes_neither_optional_field`,
    `a_check_writes_its_command_to_copy_and_that_it_could_not_check` (red with the two fields not serialized, with
    `every_result_round_trips`), `a_check_refuses_a_field_it_does_not_have`,
    `board_checks_takes_only_a_root_and_answers_only_checks`.
- Safety of the tests. The fake `claude` is a script in the test's own `bin`, put on the captured `PATH` by the test's
  login shell, which also sets `HOME` to the test's root. Its JSON names `tester@example.invalid`; every test that
  runs a probe asserts the account is in neither the answer nor the daemon's log. `DaemonOptions::new` gives
  `NotChecked`, and `collide-daemon` gives the real probe only when `COLLIDE_DAEMON_ROOT` is not overridden, so no test
  daemon but the ones that pass `AuthStatus` explicitly runs `auth status`, and those find only the fake.
  A test daemon that finds a real `claude` on the process's `PATH` still reads its `--version`, as the daemon's start
  read did before this item.

### Decisions for the owner

- **How the page tells "could not check" and the command to copy.** The wire had `{ id, ok, required, message }`. I
  added `copy` (a command, optional) and `could_not_check` (a flag, optional), and a "could not check" check has
  `ok: true`, so it is not a failure. Alternative: the page reads the backticked command out of `message` and the
  words "could not check" from the text. Recommend the fields: the page draws a dash for the one and a Copy button for
  the other without parsing words. `ts-0194-qp` should draw `could_not_check` as neutral, like the GitHub row.
- **The probe's wait: 5 s** (`CHECK_TIMEOUT`), for the version read and the probe alike, side by side. Alternatives:
  10 s or more (a slow first start answers, an open can wait that long), or not waiting at all (the page opens, then
  asks `board.checks`). Recommend 5 s: a timeout is "could not check", never "logged out", so a slow start cannot
  tell the person something false, and Check again asks once more. The daemon's own start read of `claude --version`
  warms the binary first, which is the slow start on a machine that checks each new program.
- **The probe is the daemon's alone.** `NotChecked` is the default of `DaemonOptions`, and only the daemon's binary
  under a real root gets `AuthStatus` (the rule of `published_from_env` and `secret_store_for_root`). A developer
  running a daemon under `COLLIDE_DAEMON_ROOT` sees "could not check" for login. Alternative: always run it. Recommend
  keeping the rule: a test daemon never asks a real `claude`.
- **`board.open` with `init_git` is refused to a run's agent** (`refuse_run_agent`, as `board.locate` and
  `board.init` are), because it makes a repository. Opening without it, and `board.checks`, are open to all, as they
  were. Alternative: allow it to all callers. Recommend the refusal.
- **`init_git` on a folder that is gone or cannot be read is not an error.** It makes nothing and answers the failed
  `git_repo` check with the message of `checkout_of`. A `git init` that git itself refuses (a folder it cannot write)
  is `INTERNAL_ERROR` with git's own words. Alternative: refuse a missing folder with `INVALID_PARAMS`. Recommend as is:
  the page already shows the check.
- **The install check says ok when the version cannot be read** ("installed. Its version could not be read."): the
  binary is there. Alternative: "could not check". Recommend as is.
- **The checks wait for the base environment** (up to its capture timeout and margin) when an open comes before the
  daemon's first capture ends, as `daemon.agent_versions` does, because the `PATH` `claude` is found on is the
  captured one. Alternative: use the daemon's own `PATH` until then; it can name another `claude` than runs will use.
- **Words.** Taken from the mock's `welcomeHtml` rows, as one sentence each. Missing: "Claude Code is not installed.
  Install it, then check again: `npm install -g @anthropic-ai/claude-code`". Logged out: "Claude Code is not logged in.
  Run `claude` in a terminal and log in. You can open the board now; runs need it." The git failure is the message
  `board.open` already had ("Run `git init` in it, or open a folder that is inside a checkout"); the page's Make it
  one replaces the first half, which is for `ts-0194-qp` to word.

### Not done, by design

- No UI: the page, its rows, Copy, Check again and the Make it one window are `ts-0194-qp`'s. The fakes of the app
  (`app/test/support/loop-world.ts`, `app/browser/fake/`) do not answer `board.checks` yet; that item adds them with
  the page.
- `gh_logged_in` is not produced: it arrives with the `github` pack (`ts-0172-xc`).
- A `claude` that leaves a child holding its pipes after it exits (not a hang) is not ended: the answer is "could not
  check", and the child is the runtime's to end. Only a cut ends the group.
