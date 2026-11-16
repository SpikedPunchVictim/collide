---
id: ts-0081-wm
kind: task
status: active
title: "App: the thin-loop page"
owners: []
links:
  - { type: blocked-by, to: ts-0080-tq }
---

## Goal

The window's one page for the thin loop (Stage 3c's pull from Stage 6), written against `OwnerClient`
(`app/src/owner/client.ts`):

- **The repository.** Pick a board, or open a folder (per the decision below), through `board.list` and `board.open`.
- **The prompt box.** A prompt creates a scratch task (`task.apply`) and starts `collide/quick` on it (`run.start`).
- **The two `claude` version chips** from `daemon.agent_versions` (0.3, Two version chips).
- **The run.** Its state and its phase, in words, with the prompt.
- **The one-item Queue:**
  - a permission request: what the agent asks to do, its input or "input not available until approved", and
    allow once or deny. Never an always-allow answer (0.3, 0.11);
  - "ready to review": the diff's files with their counts, one file's patch when picked, and accept, re-prompt (a
    box for the words) and discard.
- **Reopen.** On load the page reads `run.list` and `queue.list`, then follows the event channel, so a reopened
  window shows the same run and the same item.
- **Refusals** from the daemon (a conflict, a checkout on another branch) are shown in plain words, and the item
  stays.

## Success criteria

- Against `FakeOwner` (`app/src/owner/fake.ts`), a prompt makes the two calls with the right board, task key and
  workflow, and the page then follows the run's events to the review item.
- A permission request shows and is answered through `queue.answer`. An item with no input says "input not
  available until approved". No always-allow choice exists.
- The review item shows the diff and sends each of the three answers. A refused accept shows the reason, and the
  item stays.
- A page loaded while a run is live, or while its review item waits, shows the same run and item.
- The in-use chip is marked when its version is outside the tested range. Its tooltip names the range and when the
  smoke check passed. The published chip shows `latest`, with `stable` in its tooltip, and is hidden when the daemon
  sends no published versions (`claude.check_published: false`).
- The look follows 0.11's audience rule (revision 17): plain words, state in words as well as colour, sans-serif
  type, and engineer detail (run id, branch, paths, session id) behind a disclosure. Components are shadcn/ui
  (0.15).

## Tests

Write these first, in vitest with no browser, as `app/test/ui-reducer.test.ts` does:

- the page's reducer over a scripted `FakeOwner`: prompt, run events, review item;
- a permission request, with and without input, and its answers;
- the diff, a picked file's patch, and the three answers;
- a refused accept;
- a load with a live run, and a load with a waiting review item;
- the version chips: in range, out of range, with no in-use version, and with no published versions;
- rendered text names each state in words (`renderToString`, as `app/test/app-shell.test.tsx` does).

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 3c: what it pulls from Stages 6 and 9a; the two version chips.
  - 0.3: Two version chips.
  - 0.11: Interface direction (the approachable look, revision 17); Queue (the list and detail, the permission
    row).
  - 0.15: shadcn/ui on Radix and Tailwind.
- **Decision for the owner, with a recommendation: how the window picks its repository.** The window has no folder
  dialog: Stage 6a keeps its capabilities to `rpc` and one event channel. Recommend a list of the boards from
  `board.list` and a box to type a folder path for `board.open`. The alternatives are a native folder dialog, which
  adds a capability, or a path given when the app starts.
- **Decision for the owner, with a recommendation: one page or the tabs.** Stage 3c says "a window with one page".
  `app/src/AppShell.tsx` already has the tabs of Stage 2 (Field, Tracks, Queue, Experiments, with no subtitles and no
  Planning). Recommend the one page alone in 3c. `App.tsx` shows it in place of `AppShell`, and Stage 6a brings the
  tabs, with their subtitles (revision 19).
- **Decision for the item: the page's state.** Recommend a small reducer of its own, beside the page, since Stage
  9a replaces the page. The board UI's reducer (`app/src/ui/`) stays as it is.
- **Decision for the item: the stubbed actions.** `app/src/components/stub-actions.ts` holds `ui: launch`,
  `ui: queue answer`, `ui: review decision` and `ui: open item`, tagged Stage 9a. Recommend leaving them: the page
  calls the client through its own actions, so the 9a tags stay as they are.
- **Decision for the item: discard.** It deletes the work. Recommend a second press to confirm it, which says what
  is deleted.
- The page shows the prompt from `task.get`, because a Queue item's title leaves the task title out.
- Area: `app/src` (a new page folder, `App.tsx`), `app/test`.
- The test rule: no test touches the real home, real launchd or a real daemon. Each test clears the environment
  (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as `host/tests/common/mod.rs` `TempRoot`
  does. Settings files are fixtures, never the owner's own `settings.json`. This item's tests use `FakeOwner` and no
  daemon.
- Carried from `ts-0077-ws` (review): the shell tells the page when the event stream ends and when it comes back, as
  a notice on the one event channel. On its return the page reloads its runs and Queue with `run.list` and `run.get`,
  so a gap the daemon's log no longer holds never leaves the page behind. Test it with a daemon restart.
- Stage 3c item (revision 22 scope, split 2026-11-15). Items of Stage 3c run one at a time, in id order, because
  they share `host`, the app and the generated files.
- Owner decisions (2026-11-15, at the split): the repository picker (`board.list` plus a typed path) and one page
  (the tabs wait for Stage 6a). The recommendation, taken under the owner's delegation; the owner may change it
  before the item is built.

### Built

The window shows one page for the thin loop: pick a repository, send a prompt, watch the run, answer its permission
requests, and accept, send back or discard its review item. No wire type changed, so `pnpm gen` had nothing to
regenerate. No package was added.

- `app/src/loop/` (new, the page and its own state; the board UI's reducer in `app/src/ui/` is untouched):
  - `types.ts`, `words.ts`: the page's types, its words (`phaseWords`, `REQUEST_WORDS`, `INPUT_UNKNOWN_WORDS`,
    `fileStatusWords`, `countWords`, the notices) and `versionChips`, a pure function from `AgentVersions` and "now" to
    the two chips with their tooltips.
  - `state.ts`: `LoopState`, `LoopAction` and `loopReducer`.
  - `loop.ts`: `createLoop(owner)`, a plain-code controller over `OwnerClient`. Events only trigger a reload (the
    daemon is read again), and reloads that come together are met with one. `open` subscribes first, then loads
    `board.list`, `daemon.agent_versions`, `run.list`, `queue.list`, `run.get`, `task.get` and `run.diff`. `startRun`
    is `task.apply` (source `scratch`, op `create`, `{prompt}`) then `run.start` with `collide/quick`. Permission
    answers are `allow` and `deny` only. Discard asks on the first press and sends on the second.
  - `LoopPage.tsx`, `Chips.tsx`, `Repository.tsx`, `PromptBox.tsx`, `RunCard.tsx`, `Queue.tsx`: the views, a function of
    `LoopState` and `LoopActions`. `useLoop.ts`: the hook (`useSyncExternalStore`, `open` on mount, `close` on unmount).
  - Controls have roles and names (labelled fields, named buttons, `role="alert"` and `role="status"`). No test ids.
- `app/src/App.tsx` shows `LoopPage` over `createTauriOwner()` in place of `AppShell`. `AppShell`, its surfaces and
  `stub-actions.ts` stay as they were.
- `app/src/components/ui/`: `input.tsx`, `textarea.tsx`, `tooltip.tsx` (from `radix-ui`) and `native-select.tsx`.
- Shell half of the carried ts-0077 note, `app/src-tauri/src/shell.rs`: `CONNECTION_EVENT` (`shell.connection`), the
  `StreamState` kept in `PumpProgress`, and `tell_connection`. `pump_daemon_events` says `lost` when a stream that was
  up ends, and `follow_daemon` says `back` after the gap check and before the replay. `app/src/owner/client.ts`
  has `CONNECTION_EVENT` and `ConnectionEvent`, in `ChannelEvent`.
- Tests, written before the code they test:
  - `app/src-tauri/tests/daemon_link.rs`, with `Channel::connection_states` in `tests/common/mod.rs`:
    `a_stream_that_ends_is_told_to_the_window_and_so_is_its_return_before_the_replay` (real in-process daemon,
    restarted), `a_notice_of_the_connection_has_the_state_and_nothing_else`,
    `trouble_that_goes_on_is_told_once_and_a_pump_that_is_stopped_says_nothing`,
    `a_pump_that_found_no_daemon_tells_the_window_when_the_daemon_comes`,
    `a_pump_that_is_stopped_while_the_stream_is_up_says_nothing` and
    `a_first_connection_that_works_says_nothing_and_the_return_after_it_says_back` (a scripted `FakeDaemon`). The first
    four were red (they timed out on their assertions) before `shell.rs` changed. The last two were written with the
    code and not seen red before it; the last one fails when the `back` emission is removed (checked).
  - `app/test/tauri-owner.test.ts`: the event name in `client.ts` is the one in `shell.rs`.
  - `app/test/loop-page.test.ts` (47 tests, `FakeOwner` through `app/test/support/loop-world.ts`, no daemon): the
    repository, the chips' source, a prompt, the run and its events, permission requests, the review item, reopening,
    and the connection. Written before `loop.ts`.
  - `app/test/loop-view.test.tsx` (`renderToString`, no browser): words for each state, the chips and their tooltips,
    permission input present and absent, no "always" text, the review item and the two-step discard text, a refusal
    with the item and its buttons still there, the lost-contact banner, and the engineer detail only after
    `<details`. This file was written before the views, but I did not run it red before writing them. Instead I broke
    the code seven ways and saw the matching tests fail each time: an "Always allow" button; discard sent on the
    first press; the `back` reload removed; the published chip never hidden; the range flag ignoring the range; input
    shown with no input; engineer detail outside the disclosure. Each was put back and the suite is green.
- Checks, in the foreground: `cargo build --workspace`, `cargo test --workspace` (87 suites, 2587 passed, 0 failed),
  `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`, `pnpm test` (app 224, pack-sdk 66,
  plan 283, claude-contract 2), `pnpm typecheck`, `pnpm --filter @collide/app build`, `pnpm build:packs` and
  `pnpm check` (names, deps and stubs ok) all pass.

### Decisions for the owner

1. **The notice is `shell.connection` with `{state: 'lost' | 'back'}`.** The first connection says nothing. A first
   connection that failed first says `lost` and then `back`. A stopped pump says nothing. A trouble that goes on is told
   once. Recommend as built. Plan text for 0.2, beside the event channel: "The shell tells the window when the daemon's
   event stream ends (`lost`) and when it is back (`back`, sent before the replay). On `back` the window reloads from
   the daemon and never trusts a replay."
2. **The page reloads on events instead of applying them.** The reducer never reads an event body, so a gap, a replay
   and an ordinary event take one path. The cost is a few reads per event, coalesced. Recommend as built. The
   alternative applies events in the reducer, which is quicker and needs a rule for each event kind.
3. **Which run the page follows.** The run of a Queue item of kind permission or review first, else the newest run,
   and afterwards the task's newest run, so a re-prompt moves the page to the new run. `RunSummary` has no board id, so
   the board is found by calling `task.get` on each board. Recommend as built until the Tracks page (Stage 6a) says
   which run a person means. A `board_id` on `RunSummary` would remove the search.
4. **A task made for a start that was refused is reused for the same prompt**, so a second press makes no second task.
   A different prompt makes a new one. Recommend as built.
5. **Items of other kinds are listed and not answerable** ("This page cannot answer this one yet."), and an item of
   another run has an Open button. Recommend as built.
6. **Failures.** A refusal or a failed call is shown through one alert (the item stays). A load failure says "Could not
   reach Collide: ..." and the page can load again. Recommend as built.
7. **The repository choice is a native `select`**, with `NativeSelect` as a shadcn-style file, since the page has a
   handful of boards and a native control has a role and name for free. Recommend as built. The alternative is the
   Radix select, which needs a browser to test.
8. **Smoke dates are shown as ISO dates** ("passed on 2026-11-14"), which needs no locale. The published chip says
   `latest 2.1.330`, with `stable` and the age of the check in its tooltip. Recommend as built.
9. **Accept that merged says "Accepted. The work is merged into your checkout."**, `done` with no commit says
   "Accepted.", and `queued` and `integrating` say what happens next. Recommend as built.

- Owner decisions (2026-11-15, at the split): the picker, one page, the page's own reducer, the stub actions left as
  they are, and discard's second press. All taken as written above.

- Owner decisions (2026-11-15, at the build): decisions 1 to 9 taken under the owner's delegation, as recommended.
  Decision 1's plan text applied, shortened, to 0.2's "The UI speaks the owner's protocol". Decision 3's `board_id`
  on `RunSummary` is carried to Stage 6a, where the Tracks page says which run a person means. `loop-view.test.tsx`
  was not seen red before the views; seven mutations of the code each failed a matching test instead. Two of the six
  shell tests were not seen red. The builder kept its scratch in the session's scratchpad rather than its own `mktemp
  -d` folder, and deleted its own files by exact path.

### Not done, by design

- A click-through in a browser (`ts-0082-fp` drives the page, using the roles and names built here). The view tests
  render to a string, so a click, a focus and a tooltip opening are not tested here.
- The tabs, their subtitles and the folder dialog (Stage 6a).
- Answers other than permission and review: conflict, ask-human and the rest (Stage 9a). They are listed read-only.
- An always-allow answer: the page has no code for it, and a test scans for one.
- A stop or pause control for the run, and more than one run at a time.
- The Tauri window itself was not launched, by the rules of this item.
