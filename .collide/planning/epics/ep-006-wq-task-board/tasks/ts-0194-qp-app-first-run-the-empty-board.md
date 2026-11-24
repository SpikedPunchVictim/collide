---
id: ts-0194-qp
kind: task
status: ready
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
- Owner decisions: none recorded yet. The recommendations stand until the owner answers at the split.
