---
id: ts-0194-qp
kind: task
status: done
title: "App: first run, the empty board, Share with team and trust"
owners: []
links:
  - { type: blocked-by, to: ts-0193-gk }
---

## Goal

Opening a folder shows one page of checks (note 13), and any git repo opens with no setup step. A board with nothing
on it shows the empty board's choices. Until the board is shared, a banner says its settings are the person's only,
with Share with team, which writes `.collide/` for the person to commit. A clone whose `.collide/` brings packs,
views or settings asks for trust before they apply.

What exists.
- From `ts-0192-es`: `board.open`'s checks with their fixes, `board.checks`, and `init_git`.
- From `ts-0191-nv`: the folder picker and the board menu.
- `board.init { board, dry_run }` (`host/src/board_init.rs`): writes `.collide/board.yaml` (`packs: []`) and
  refuses when `.collide/` exists; `dry_run` lists the files. Personal views live in the person's board folder
  (`UserScope::board_dir`). No setting is per board and per person: the user file is machine-wide.
- `board.trust_review` (`BoardTrustReviewResult { trust, problems, changes, packs }`, from `pack_review`) and
  `board.trust`. Every line is untrusted until trusted (`host/src/trust.rs` `trust_state`). The hash covers packs,
  the lock, `board.yaml`, `config.yaml`, views and `protectedPaths`. Built-in packs run either way; repo packs, repo
  views and the repo's `config.yaml` apply only when trusted.
- `task.create` makes a scratch task (`ts-0075-hj`).

What this adds.
- The "Open a project" page (the mock's `welcomeHtml`), shown for a folder opened from the menu and at first launch:
  the folder and Choose another…; the checks (git, `claude` installed, `claude` logged in), each failing one with its
  fix and a Copy button (`navigator.clipboard`); the row "GitHub: checked when GitHub sources arrive, in Stage 5";
  "Found in this folder" (what the board will read); Open the board, disabled for a folder that is not a git repo;
  Check again (`board.checks`). A folder that is not a git repo offers Make it one…: a window that says what `git
  init` does, then `board.open` with `init_git`.
- The empty board (the mock's `emptyHtml`), shown when the board has no task: Plan it first, first, with its "9d"
  tag and its button saying it arrives in Stage 9d; Start with a task, which makes a scratch task and shows it; the
  sources Collide found; Add a pack (the window of `ts-0193-gk`), with "GitHub arrives in Stage 5".
- The banner "This board's settings are yours only." with Share with team, while the board has no `.collide/`. The
  window (the mock's `shareModal`) lists the files from `board.init`'s `dry_run` and says nothing is committed. Write
  `.collide/` writes them. See the Owner's choice on what it writes.
- The trust prompt (the mock's `trustModal`), when the board's line is untrusted and its `.collide/` brings
  something that waits on trust: what it brings (from `board.trust_review`), where each pack comes from and what it
  reads, "The built-in packs need no trust", Open without it, and Trust and open (`board.trust`). Open without it
  opens with the built-in packs, and a banner names what is not applied. See the Owner's choice on when it shows.

## Success criteria

- A fresh git repo with no `.collide/`, on a machine profile with no Collide config, opens to a board with no setup
  step (Stage 6a).
- Share with team writes `.collide/`, and a second clone opens with it after the trust prompt (Stage 6a).
- A folder that is not a git repo does not open, and offers Make it one. A missing or logged-out `claude` is named
  with its fix and a copy button, and the board still opens (Stage 6a).
- The empty board's Start with a task makes a scratch task, and the Field shows it.
- Nothing needs the `github` pack: the GitHub row is text.

## Tests

Write these first.

- `app/test/`: the check rows for each state; the empty board's parts; the banner shown only without `.collide/`;
  the trust window's words from a `board.trust_review` answer.
- `app/browser/first-run.spec.ts` (bridge, Stage 6a test "the no-setup open and first-run checks"): the world's
  fake `claude` in three forms (missing, logged out, logged in); a plain folder and Make it one; a fresh repo opens
  with no setup; Start with a task; Share with team, then the test commits and clones; the clone opens to the trust
  prompt; Trust and open shows the shared view.
- `host/tests/`: `board.init` with views.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 6a: No-setup open; Empty board. Success Criteria: the fresh repo; Share with team and the second clone;
    the folder that is not a git repo, and `claude` missing or logged out. Tests: "the no-setup open and first-run
    checks".
  - 0.11: First run works with no pack and no tasks (revision 25). 0.9: Trust. 0.7: settings layers.
- Design: notes 10 and 13. Mock functions: `welcomeHtml`, `gitInitModal`, `emptyHtml`, `addScratch`, `shareModal`,
  `trustModal`, and the banner in `fieldHtml`.
- Carried: `ts-0097-kw` and `ts-0102-vu` (the app's trust prompt prints `pack_review`'s diff; Stage 6a owns the Trust
  UI and Share with team).
- **Owner's choice, recommendation taken (2026-11-24): what Share with team writes.** The mock lists `board.yaml`,
  `views/` (your saved views) and `config.yaml` (this board's settings). `board.init` writes `board.yaml` only, and no
  setting is per board and per person to write. Recommend `board.yaml` plus the person's saved views of this board,
  copied to `.collide/views/`, and no `config.yaml` until per-board settings exist. Alternative: `board.yaml` only, as
  built; views are shared one at a time (`ts-0190-tz`).
- **Owner's choice, recommendation taken (2026-11-24): when the trust prompt shows.** Stage 6a says "only for repo
  packs", and 0.11 says "only for packs from the repo or ones the user adds". 0.9 and the code hold repo views and the
  repo's `config.yaml` until the line is trusted too, and the mock's Share window says a clone "is asked to trust them
  first". Recommend asking whenever the line is untrusted and `.collide/` brings packs, views or settings, worded by
  what it brings ("this repo brings two views and its settings"). Alternative: ask only for packs, and apply repo views
  and settings untrusted; that changes 0.9's rule and the trust hash.
- **Decision for the item, with a recommendation: trusting what the person shared.** Recommend that Share records
  the hash of what it wrote, and a line whose committed `.collide/` matches it is trusted with no prompt for that
  person. Alternative: the person trusts their own files once, like a teammate.
- Out of scope: the GitHub source and the `gh` check (Stage 5, `ts-0172-xc`); planning sessions (9d); launching from
  the board (9a).
- Area: `app/src/` (the first-run page, the empty board, the banner, the share and trust windows),
  `host/src/board_init.rs` (if views are shared), `app/src/owner/methods.ts`, `app/test/`, `app/browser/`,
  `host/tests/`.
- Dependencies: none.
- The test rule: no test touches the real home, real launchd, a real daemon, the keychain or the network. Each test
  that runs the `collide` binary clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to
  a temp root, as `host/tests/common/mod.rs` `TempRoot` does. A temp repository sets `GIT_CONFIG_GLOBAL` and
  `GIT_CONFIG_SYSTEM` to empty files. Keychain writes go through `MemorySecretStore` (`host/src/secret_store.rs`),
  never the login keychain. Browser tests run the installed Chrome with a temp profile
  (`app/browser/support/chrome.ts`), and the bridge's daemon on a temp root (`app/browser/support/world.ts`). Copy
  is checked by replacing `navigator.clipboard.writeText` in the page (an init script) and reading what it got; no
  test writes the system clipboard.
- Stage 6a item (revision 25 scope, split 2026-11-24). Items of Stage 6a run one at a time, in id order, because they
  share `app/`, `core`, `host` and the generated files. No builder runs `tauri build`.
- Nothing here runs a real `claude`: the checks find the world's fake `claude`.
- Owner decisions (2026-11-24): the owner took the recommendations at the split.

### Built

- Daemon. `board.init` takes `dry_run` and `board`'s saved views (`host/src/board_init.rs`: `init`, `Shared`,
  `personal_view_files`, `write_all`; wired in `host/src/daemon.rs` `board_init`). It lists or writes
  `.collide/board.yaml` and a byte copy of the person's saved views of this board in `.collide/views/`, and no
  `config.yaml`. A failure part-way removes what it made. It records the hash of what it wrote
  (`host/src/trust.rs` `trust_written`, `host/src/board_lines.rs` `Lines::trust_written`) only when the line has no
  Collide folder, so the person's own commit of exactly those bytes asks nothing. `BoardInitResult.trusted` is new
  in `core/src/owner/board.rs`; `pnpm gen` refreshed `schemas/BoardInit*.json` and
  `packages/pack-sdk/src/generated/BoardInit*`.
- App, `app/src/firstrun/`: `FirstRunPage` (the folder, its checks, Check again, Open the board), `model.ts`
  (`canOpen`, `needsGit`, `checkRows`), `CopyButton`, `GitInitWindow` (Make it one), `EmptyBoard` (Plan it first, off
  until 9d; Start with a task; the sources; Add a pack), `scratch.ts` (`makeScratch`), `Share.tsx` (`ShareBanner`,
  `ShareWindow`, `shareNotice`), `trust.ts` (`broughtBy`, `bringsWords`), `useTrust`, `TrustWindow` (Trust and open,
  Open without it) and the "Not trusted" banner. `loop.openChecked` reads the checks before `board.open`.
  `useField` gained `sharedRead` and `reload`, and `FieldPage` the `notices` and `empty` slots. `BoardMenu` leads to
  the first-run page. `App.tsx` wires them.
- Fix found by the bridge spec: a board with no task has no `task.changed` to announce when its first listing ends,
  so the Field stayed "settling" and never showed the empty board. `FieldFeed` (`app/src/field/live.ts`) now reads a
  settling board again every `SETTLE_POLL_MS` (1500 ms) until it has settled.
- Tests. `host/tests/board_init.rs` (12 new, among them
  `a_dry_run_lists_the_board_file_and_each_saved_view_and_writes_nothing`,
  `init_copies_the_saved_views_of_this_board_byte_for_byte_and_leaves_the_originals`,
  `what_init_wrote_is_trusted_once_the_person_commits_exactly_that`, `a_commit_that_adds_anything_else_asks`,
  `a_clone_of_the_shared_repo_is_not_trusted`, `a_line_that_already_commits_a_collide_folder_records_nothing`).
  `app/test/first-run.test.tsx` (45: check rows, the page, the empty board, the banner, the trust window words, opening
  a folder, the world's checks), `app/test/field-live.test.ts` (2 new, for the settling read),
  `app/browser/fake-first-run.spec.ts` (19, fake owner: each check state with Copy through a replaced
  `navigator.clipboard`, Make it one and its Cancel, the empty board, Start with a task, Share and its refusal,
  the banner gone once committed, the trust window, Open without it, Escape, Trust and open) and
  `app/browser/first-run.spec.ts` (5, bridge over a real daemon: no `claude`, `claude` present, a plain folder made a
  repo with only `.git` written, a fresh repo with no setup and a first task, Share then commit then a clone that asks
  and, once trusted, lists the shared view). Shared worlds: `app/test/support/first-run-world.ts`; `World.start`
  gained `Plan.claude: 'none'`. `fake-shell.spec.ts` and `fake-missing.spec.ts` follow the menu now leading to the
  first-run page.
- Red at runtime: the Rust tests (8 of 21 failed with the daemon's `board_init` stubbed to the old behaviour). For the
  app, after the code was in, each of these was broken in the source and seen to fail, then restored:
  `canOpen` always true (1 test), `shareNotice` ignoring `shared` (2), `broughtBy` ignoring views (6), `makeScratch`
  using the wrong source (1), and `watchSettling` made a no-op (the new settling test). The two fake-owner specs and the
  bridge specs were written after the code and have no runtime red of their own.
- Existing Rust tests that assumed `init` leaves the line untrusted were updated to the owner's choice (a commit of
  exactly what `init` wrote is trusted): `host/tests/board_cli.rs`
  `after_init_and_a_commit_of_exactly_that_the_line_is_trusted` (was `..._the_line_is_untrusted_at_a_real_hash`) and a
  new `..._a_commit_that_adds_more_the_line_is_untrusted_at_a_real_hash`; `host/tests/board_process.rs` adds
  `a_commit_of_exactly_what_init_wrote_is_already_trusted`, and its three tests that need an untrusted line now commit
  one more view of the team's own (the agent test also compares `trust.yaml` before and after, since the person's
  `init` now writes it). `collide init` is the same daemon call as Share, so it records the trust too.
- Checks after the last edit are in the report.

### Decisions for the owner

- The bridge cannot show a logged-in or logged-out `claude`: the daemon probes the login only outside a test root, and
  answers "could not check" under one. The bridge spec covers missing and present `claude`; the two login rows are the
  fake-owner spec's and `app/test/first-run.test.tsx`. Alternative: a test-only login answer in the daemon; not done,
  as it adds a path that only tests take.
- The first-run page's "Found in this folder" is the built-in sources only (scratch, plan), as text. The folder's
  contents before it is opened are not known without a read of the folder by the daemon. Alternative: a `board.checks`
  field that lists what it finds (`.collide/`, packs). Recommend later, if the owner wants it.
- Share writes the person's saved views of this board, and no `config.yaml`, as the owner chose. The "written, not yet
  committed" banner is kept for the session only; after a restart it shows nothing until the person shares again or
  commits. Alternative: read the working tree on load (a new daemon call). Recommend keeping it session-only.
- The settling poll (1500 ms) never stops for a source that cannot start (for example `node` missing for the plan
  pack): the board keeps asking, and the empty board does not show while a source is unstarted. Alternative: show the
  empty board when every unstarted source has a reason, with the reason in a notice. Recommend this for the item that
  handles a source that does not start.

- Review (2026-11-25): the daemon dropped a failure to record Share's trust (`unwrap_or(false)`); it now writes it to
  the daemon log, and Share still reports `trusted: false`, so the person is asked as a teammate would be. The
  settling poll stays: no daemon event marks the end of a board's first listing. Proposed for a later revision: an
  event when the first listing ends, which replaces the poll.

### Not done, by design

- Plan it first (planning sessions): Stage 9d. The button is off and says so.
- The GitHub row and a GitHub source: Stage 5 (`ts-0172-xc`). The row says so.
- Launching a run from the board, and the login being required for it: Stage 9a. The checks only say, and the board
  opens either way.
- `config.yaml` in Share: left out by the owner's choice; the person commits their own repo settings.
