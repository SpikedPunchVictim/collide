---
id: ts-0083-uf
kind: task
status: active
title: "Owner check: one real prompt through the app"
owners: []
links:
  - { type: blocked-by, to: ts-0082-fp }
  - { type: blocked-by, to: ts-0071-fd }
---

## Goal

The Stage 3c check that only the owner runs: one real prompt on a worktree of Collide, through the app. The owner
reviews the diff and accepts it.

A build agent never runs it. It never starts the app against the owner's daemon, never runs `claude`, and never
touches the owner's worktree. It writes into this item, under `### Owner checklist`, the command lines and a short
checklist. The owner runs them and records the result here as owner judgement, with what was awkward.

This item also records when the dogfooding starts: from then on, the owner's Collide work that fits a prompt goes
through the thin loop (Stage 3c criterion).

## Success criteria

- The owner runs one real prompt on a worktree of Collide through the app, reviews the diff and accepts it (Stage 3c
  criterion). Owner-run, and recorded as owner judgement in this item, with what was awkward.
- The checklist holds:
  - the command lines to build and start the app, with `<collide worktree>` as a placeholder;
  - how to make the worktree of Collide and open it as a board;
  - the model, chosen by the owner through `models.yaml` for the builder's alias (haiku suggested for the first
    prompt), and a small prompt, since the page has no budget field in 3c;
  - what to look at;
  - a line for the owner's result, and one for what was awkward.
- A line records the date the owner starts sending prompt-sized Collide work through the thin loop (Stage 3c
  criterion: dogfooding).

## Tests

None by a build agent: this check is the owner's. The checklist names the automated tests it relies on:

- `ts-0077-ws`: the app's connection, and the daemon started on demand;
- `ts-0078-ye`: the run up to review, and the permission request;
- `ts-0079-pu`: accept into a checkout, including a linked worktree;
- `ts-0080-tq`: re-prompt and discard;
- `ts-0082-fp`: the loop in a browser.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 3c: the owner's real prompt and the dogfooding criteria.
  - 0.2: Starting the daemon (the app starts it on demand).
  - 0.3: Two version chips.
  - Part L.
- What the checklist covers. The build agent fills in the exact commands once the app connects.
  - **Set up.** Make a worktree of Collide on a branch of its own. Check the builder's alias in `models.yaml`. Build
    and start the app.
  - **Open.** Open the worktree as a board. Look at the two version chips.
  - **Prompt.** Type a small, real Collide change. Answer any permission request.
  - **Look for:**
    - a fresh worktree under Collide's data folder, and nothing changed in `<collide worktree>` while the run works;
    - "ready to review" with a diff that matches the change;
    - optional: close the app mid-run and open it again. The same run and item show.
  - **Accept.** The worktree's branch has the change (`git log`). The run's worktree and branch are gone. The
    scratch task is `done`.
  - **Record.** The result, the time it took, and what was awkward.
- **Decision for the owner, with a recommendation: the order.** This item waits on `ts-0071-fd`, the owner's 3b
  checks, so a real run is checked from the CLI before the app. Recommend that the Stage 3c build items start now:
  none needs anything from `ts-0071-fd`.
- Area: this item's own text. No code.
- The test rule, for anything automated this check leans on: no test touches the real home, real launchd or a real
  daemon. Each test clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root,
  as `host/tests/common/mod.rs` `TempRoot` does. Settings files are fixtures, never the owner's own
  `settings.json`. The owner's own run here uses the owner's real setup by choice.
- Stage 3c item (revision 22 scope, split 2026-11-15). Items of Stage 3c run one at a time, in id order, because
  they share `host`, the app and the generated files.
- Owner decisions (2026-11-15, at the split): the order (Stage 3c builds while `ts-0071-fd` is open; only this check
  waits on it). The recommendation, taken under the owner's delegation; the owner may change it before the item is
  built.

### Owner checklist

Checked against the code at f4dad8a on 2026-11-17: no command, path, table or message drifted; two notes added (`pnpm
install` is needed once, and any older daemon is to be stopped).

Corrected on 2026-11-18, at the owner's first run: `sqlite3 -readonly` fails on the store with `unable to open database
file (14)`. The store is in WAL mode, and a read-only open needs the `-shm` file, which is gone whenever the daemon has
the store closed. The queries now open it with `sqlite3 -cmd 'PRAGMA query_only=1'`, which reads and refuses any write
(checked on a copy of the store).

Written from the code and the Built and Decisions sections of `ts-0077-ws` to `ts-0082-fp`. Every command below is the
owner's to run; nothing here was run by the build agent, and no command was tried against the real app. Run it after
the checks of `ts-0071-fd`, so a real run has been seen from the CLI first. The run spends real money on your
`claude` login, and the page has no budget field, so keep the prompt small.

**Setup, once.**

1. Build the daemon, the CLI and the app. The app starts the `collide-daemon` that sits beside its own executable, so
   all three must be in one folder. Build from `<collide checkout>`.

   ```sh
   cd <collide checkout>
   pnpm install                                         # once since ts-0109: the pack SDK needs `yaml`
   pnpm build:packs
   pnpm --filter @collide/app build                     # the page, into app/dist; the app embeds it
   cargo build -p collide-host                          # collide, collide-daemon
   pnpm --filter @collide/app exec tauri build --debug --no-bundle
   export COLLIDE_BIN=<collide checkout>/target/debug
   ls "$COLLIDE_BIN/collide-app" "$COLLIDE_BIN/collide-daemon" "$COLLIDE_BIN/collide"
   ```

   Not confirmed from the code: that the last build puts `collide-app` in the workspace's `target/debug` and that its
   window shows the embedded page. The flags `--debug` and `--no-bundle` are from `tauri build --help`, and
   `bundle.active` is `false` in `app/src-tauri/tauri.conf.json`. Whether `cargo run -p collide-app` shows the built
   page is also not confirmed, so use the build above. If `ls` fails, find the binary under `target/` and say so in the
   result.

2. Stop a daemon left from an older build. One from before `ts-0077-ws` does not know the app and has no `run.start`.
   A daemon from any build older than the one just made may also lack newer fields of a run (`warnings` of
   `ts-0104-gx`, in `core/src/owner/run.rs`), so stop it as well.
   A daemon that is not there is fine.

   ```sh
   grep 'starting, pid' "$HOME/Library/Application Support/Collide/daemon.log" | tail -1
   ps -p <daemon pid>                        # prints a row only if it is still running
   kill -TERM <daemon pid>
   ```

   There is no `collide-daemon stop`. The app starts a fresh daemon on its first call.

3. Map the builder's alias. The page starts the built-in builder, whose alias is `fast` (`BUILDER_MODEL_ALIAS` in
   `host/src/builtin_roles.rs`), and has no way to pick another. The file is `~/.config/collide/models.yaml`; if it
   exists, add the alias under `tools.claude` and keep the rest. It is read at each run start, so no restart follows.

   ```yaml
   tools:
     claude:
       fast: { name: haiku }
   ```

   A start with no mapping is refused before any run exists.

4. Do not set `COLLIDE_DAEMON_ROOT`. With it set, the daemon starts without the launchd job and does not fetch the
   published `claude` version, so the second chip would be missing. Without it, the files are in
   `~/Library/Application Support/Collide/`:

   | what | path |
   |---|---|
   | daemon log | `daemon.log` |
   | store (SQLite) | `store/store.db` |
   | the run's worktree | `workspaces/<run id>/` |
   | run folders | `runs/<run id>/build/` |

5. Make the worktree of Collide on a branch of its own. This is `<collide worktree>`, the board the page opens. Put
   it outside `<collide checkout>`, and do not switch its branch while the run works: accept merges into the branch
   the worktree is on.

   ```sh
   git -C <collide checkout> worktree add -b owner-check-0083 <collide worktree> main
   git -C <collide worktree> status --short                       # nothing
   git -C <collide worktree> log --oneline -1
   git -C <collide checkout> worktree list
   git -C <collide checkout> branch --list 'collide/*'
   ls <collide worktree>/.collide/planning/epics/ep-003-da-headless-core/tasks/ts-0085-zs-*
   ```

6. Start the app from a Terminal window, in the foreground, so `HOME`, `SHELL` and `PATH` are the ones your login
   shell has. The first call starts the daemon through a one-shot launchd job and waits for the login shell's
   environment, so the first load may take several seconds. Quitting the app does not stop the daemon. The app's
   errors go to this terminal.

   ```sh
   "$COLLIDE_BIN/collide-app"
   ```

**Open the worktree as a board.**

In the window, under "Repository", type the full path of `<collide worktree>` in "Folder path" and press "Open
folder". Use an absolute path: a relative one, and so `~`, is refused (`ts-0075-hj` decision 13). The "Repository"
list then shows it. A folder inside the worktree opens the worktree's top level. Opening reads git and writes nothing
in the folder.

**The prompt.**

A small change that fits `haiku`: `ts-0085-zs`, in the same folder as this item. The first line becomes the scratch
task's title, so keep it short. Suggested text for "What should the agent do?":

> Keep the re-prompt notice across the run change (ts-0085-zs).
> Do what `.collide/planning/epics/ep-003-da-headless-core/tasks/ts-0085-zs-page-keep-the-re-prompt-notice.md` says.
> Change `app/src/loop/state.ts` and add its tests to `app/test/loop-page.test.ts`. Commit when done.

The fresh worktree has no `node_modules`. The agent may ask to run `pnpm install` or the tests. That request is the
permission check: allow it once (it needs the network and takes a while), or deny it and see the agent carry on. Both
are fine here. This check is about the loop, not about whether the agent's code is right. Press "Start". The button
stays off until a repository is chosen and there is text.

**What to look at.**

- [ ] The two chips, top right, before and after the start. Hover or focus each one to read its tooltip.
  - In use: the version of `claude` (`2.1.x`). It matches `claude --version` in your terminal. If it reads `unknown`,
    the tooltip says why.
  - A version outside the tested range reads "Outside the tested range" in red, and the tooltip names the range and
    the smoke check.
  - Published: `latest <x>`, and the tooltip says "Stable is <y>." and when it was checked. It is hidden when
    `claude.check_published: false` is in `~/.config/collide/config.yaml`.
- [ ] After "Start": the prompt box empties. "The run" shows your words, the badge "Working" and "Step: build. The
  agent is working." Open "Details": note the run id, the work folder (under `workspaces/<run id>`) and "Branch the
  work started from" (`owner-check-0083`).
- [ ] A fresh worktree and its folder:
  - `git -C <collide checkout> worktree list` gains one entry, the work folder.
  - `git -C <collide checkout> branch --list 'collide/*'` gains `collide/<run id>`.
  - `ls "$HOME/Library/Application Support/Collide/workspaces/"` shows `<run id>`.
  - `git -C <work folder> log --oneline -1` is the same commit as `<collide worktree>`'s `HEAD` at the start.
- [ ] Nothing changing in `<collide worktree>` while the run works. Repeat these a few times, and again at the end:
  - `git -C <collide worktree> status --short` prints nothing.
  - `git -C <collide worktree> log --oneline -1` is unchanged.
  - `git -C <collide worktree> branch --show-current` is `owner-check-0083`.
- [ ] Any permission request. The card's title says what is asked ("Wants to run a command"), with `Tool:`, a reason
  and "What it would use": the input, or "input not available until approved". It offers "Allow once" and "Deny",
  and nothing like "always". After you answer, the run goes on and the card goes.
- [ ] "Ready to review":
  - The badge reads "Finished: done". "Needs you" holds a card titled "The agent finished and its changes are ready
    to review".
  - "What changed" lists each file as `changed, N added, M removed`. Pick a file to read its patch. The files and the
    patch match the change you asked for, and nothing else.
  - `git -C <work folder> log --oneline -5` shows the agent's commit. If the agent left changes uncommitted, Collide's
    own commit "Commit what the agent left uncommitted" is on top, authored `Collide`.
  - Read the diff for names that should not be public. The repository's commit hooks look at files that are not in a
    fresh worktree, so they check nothing on the agent's commits.
  - The scratch task is `review`:

    ```sh
    sqlite3 -cmd 'PRAGMA query_only=1' "$HOME/Library/Application Support/Collide/store/store.db" \
      "SELECT b.root, t.num, t.title, t.status FROM scratch_tasks t JOIN boards b ON b.id = t.board;"
    ```

- [ ] Optional: close and reopen, mid-run or at review. Quit the app (Cmd-Q) and check that the run goes on:
  `tail -5 "$HOME/Library/Application Support/Collide/daemon.log"`. Start `"$COLLIDE_BIN/collide-app"` again. The
  page shows the same run (same id in "Details") and the same item, with the diff if it is at review.
- [ ] Accept. Press "Accept". The page says "Accepted. The work is merged into your checkout." and "Needs you" says
  "Nothing is waiting on you." Then:

  ```sh
  git -C <collide worktree> log --oneline -5                      # the agent's commit(s) on top of the old HEAD
  git -C <collide worktree> status --short                        # nothing
  git -C <collide checkout> worktree list                         # the work folder is gone
  git -C <collide checkout> branch --list 'collide/*'             # collide/<run id> is gone
  ls "$HOME/Library/Application Support/Collide/workspaces/"      # no <run id>
  sqlite3 -cmd 'PRAGMA query_only=1' "$HOME/Library/Application Support/Collide/store/store.db" \
    "SELECT b.root, t.num, t.title, t.status FROM scratch_tasks t JOIN boards b ON b.id = t.board;"   # status done
  ```

  The branch `owner-check-0083` holds the change and `main` does not. To keep it, merge or cherry-pick it into `main`
  yourself. To drop it, `git worktree remove <collide worktree>`, then `git branch -D owner-check-0083`.

If the page does not follow the run (it stays at "Working" after the agent has finished), note it. The webview's
event listening against `app/src-tauri/capabilities/main.json` was never run in a real window (see Gaps).

Not part of this check: re-prompt and discard on a real run. The tests below cover them. Try them on a later prompt.

If something goes wrong:

- A run you want to end now: `"$COLLIDE_BIN/collide" stop <run id>`. The page has no stop control. The run then has a
  review item with "Send back" and "Discard" only.
- "Could not reach Collide": read `daemon.log`. After three failed starts in five minutes the app stops trying; quit
  and start it again. `"$COLLIDE_BIN/collide-daemon" start` prints `started` or `already running`.

**Results.**

- Owner result (2026-11-18): run by the owner, no failure reported. Read by the agent from the store, git and
  `daemon.log`: `run-1794991783994-2`, started from the app on a board branch made from `main` a minute before; the
  agent asked for Edit and Bash, committed `2634ea0` (`ts-0085-zs`, three files under `app/`) and the phase ended with
  code 0; at review the scratch task was `review`. After Accept the commit sits on the old HEAD of the board branch, the
  board has nothing uncommitted, the work folder, its worktree entry and its `collide/<run id>` branch are gone, the
  scratch task is `done` and the run `ended` with outcome `done`. The diff holds no name that should not be public. Two
  notes. `collide stop` after the accept stopped nothing (`stopping 0 phase(s)`) and left the app window open, as it
  should: it ends a run, not the app (Cmd-Q quits it). And `daemon.log` has `a connection was refused: could not read
  the pid of the connecting peer (LOCAL_PEERPID)` just before each CLI connection: the CLI's probe (`launch.rs`
  `answers()`, connect and close) closes before the daemon reads its pid, and the daemon logs that as a refusal. Log
  noise, not a refused command.
- Time it took (from "Start" to "Accepted", and how long the agent worked):
- What was awkward:

**Dogfooding.** From this date, the owner's Collide work that fits a prompt goes through the thin loop (Stage 3c
criterion). `ts-0084-sy` and `ts-0085-zs` are the first candidates (part L).

- Date dogfooding starts:

**Gaps.**

- No budget field on the page, and no budget cap at all: the quick run starts with no budget (`ts-0081-wm`,
  `ts-0078-ye` `start_quick`). The page shows no cost; the daemon records turns in the store (`ts-0067-kw`), and no
  command prints them.
- No stop or pause control on the page, and one run at a time (`ts-0081-wm`, Not done). Use `collide stop <run id>`.
- Permission answers are "Allow once" and "Deny" only; there is no always-allow (`ts-0081-wm`).
- No folder dialog: the repository is a typed absolute path, and a linked worktree is its own board, not the main
  checkout (`ts-0081-wm` Notes, the repository picker; `ts-0075-hj` decision 5, moved to Stage 14).
- The app finds `collide-daemon` beside its own executable and nothing is packed into a bundle, hence the `target/debug`
  folder above (`ts-0077-ws`, decisions for the item).
- The real `LaunchStarter` and the webview's `listen` against the capability file were never run by a test or in a
  window (`ts-0077-ws`, Not done). This check is their first real run.
- A failure of `daemon.recapture_environment` is written to the app's standard error only (`ts-0077-ws`, Not done).
- A failed or stopped run gets a review item with "Send back" and "Discard" only, and a failed run's title carries the
  cause's own name, such as `no_result` (`ts-0078-ye` decision 1; `ts-0080-tq` decision 7).
- A busy checkout, a conflict, a checkout on another branch or a dirty work folder refuses the accept and the item
  stays; nothing waits for the checkout (`ts-0079-pu`, Not done).
- After accept, the run's diff is gone: its branch is deleted (`ts-0079-pu`, Not done).
- The "Sent back" notice after a re-prompt can pass unseen (`ts-0082-fp`, decisions; `ts-0085-zs`).
- A discarded task is `dropped` and cannot be started again (`ts-0080-tq`, Not done).
- The page follows one run; the items of other runs are listed with an "Open" button (`ts-0081-wm` decisions 3 and 5).
- Not recorded in an item: a fresh worktree has no `node_modules` or `target`, so the agent cannot run the tests
  without installing; and the repository's commit hooks check nothing in a fresh worktree.
- Not verifiable from the code or `--help`: `ps`, `sqlite3`, `git` and `kill` behave as on any Mac; the exact words of
  a permission card depend on what the agent asks; and what the agent does with the suggested prompt.

**Automated tests it relies on:**

- `ts-0077-ws`: the app's connection and the daemon started on demand
  - `host/tests/app_file.rs`, `host/tests/owner_client.rs`;
  - `app/src-tauri/tests/daemon_link.rs`, `app/src-tauri/tests/rpc.rs`.
- `ts-0078-ye`: the run up to review, and the permission request
  - `host/tests/adapters/claude/quick_run.rs`:
    `a_prompt_runs_in_a_fresh_worktree_on_its_own_branch_and_ends_in_a_review_item`,
    `a_permission_request_in_a_quick_run_is_a_queue_item_and_allow_lets_the_phase_go_on`,
    `a_board_whose_root_is_a_linked_worktree_runs_from_the_branch_it_is_on`;
  - `host/tests/quick_sequence.rs`.
- `ts-0079-pu`: accept into a checkout, including a linked worktree
  - `host/tests/adapters/claude/accept.rs`: `accept_merges_the_branch_into_the_checkout_and_removes_the_worktree`,
    `a_board_whose_root_is_a_linked_worktree_merges_into_the_branch_it_is_on`.
- `ts-0080-tq`: re-prompt and discard
  - `host/tests/adapters/claude/reprompt.rs`, `host/tests/adapters/claude/discard.rs`.
- `ts-0081-wm`: the page and the chips
  - `app/test/loop-page.test.ts`, `app/test/loop-view.test.tsx`.
- `ts-0082-fp`: the loop in a browser against fake Claude
  - `app/browser/loop.spec.ts`, `app/browser/bridge.spec.ts`.
