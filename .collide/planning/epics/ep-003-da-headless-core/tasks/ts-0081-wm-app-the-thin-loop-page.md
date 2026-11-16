---
id: ts-0081-wm
kind: task
status: ready
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
- Stage 3c item (revision 22 scope, split 2026-11-15). Items of Stage 3c run one at a time, in id order, because
  they share `host`, the app and the generated files.
- Owner decisions (2026-11-15, at the split): the repository picker (`board.list` plus a typed path) and one page
  (the tabs wait for Stage 6a). The recommendation, taken under the owner's delegation; the owner may change it
  before the item is built.
