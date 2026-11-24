---
id: ts-0179-ke
kind: task
status: ready
title: "App: the five tabs, the top bar and a fake-owner browser page"
owners: []
links:
  - { type: blocked-by, to: ts-0178-fw }
---

## Goal

The window shows the board's frame in place of the loop page: the top bar with the trail and the board menu, the five
tabs with their subtitles and keys, and the Settings button. Queue and Comms say what arrives and when; Tracks shows a
still preview of its design. A browser page over a fake owner lets the UI's tests run in Chrome with no Tauri and no
daemon.

What exists.
- `app/src/App.tsx` shows `LoopPage` (`app/src/loop/`) alone. `app/src/AppShell.tsx` has Stage 2's four tabs (Field,
  Tracks, Queue, Experiments) with no subtitles, over the reducer of `app/src/ui/` (`state.ts` `SURFACES`,
  `reducer.ts`). It is tested (`app/test/app-shell.test.tsx`, `surfaces.test.tsx`) and not shown.
- `app/src/components/` holds Stage 2's surfaces over invented data (`placeholders.ts`, `placeholders-detail.ts`,
  `stub-actions.ts`) and the shadcn parts in `components/ui/` (`tabs.tsx`, `button.tsx`, `badge.tsx`, `card.tsx`,
  `tooltip.tsx`).
- The owner client (`app/src/owner/`: `client.ts`, `tauri.ts`, `bridge.ts`, `fake.ts` `FakeOwner`, `methods.ts`) does
  the handshake, the reconnect and the lost and back states. `app/src/loop/Repository.tsx` lists the boards of
  `board.list` and takes a typed folder for `board.open`.
- The vitest tests of `app/test/` render with `react-dom/server` in Node over `FakeOwner`: no DOM and no clicks.
  Playwright (`app/playwright.config.ts`, `app/browser/`) drives the installed Chrome against a real daemon through
  `collide-bridge`; its page is built by `vite.bridge.config.ts` from `app/src/bridge-main.tsx`. Nothing runs in a
  browser against a fake owner, which Stage 6a's criterion asks for.

What this adds.
- `App.tsx` shows the board's frame. The loop page stays reachable: "Open the Loop page" on Queue and Tracks opens
  it, and it has a way back.
- The top bar (note 1). At the top left, the trail: the board menu, then a slot for the view picker (`ts-0181-ft`
  fills it). The board menu lists every board of `board.list` with its default line, and "Open a folder…" with a
  typed path, as `Repository.tsx` does now (`ts-0191-nv` adds the picker). Switching boards swaps what the surfaces
  read and stops nothing. At the top right, the Settings button; its page is `ts-0193-gk`'s, and until then it says
  so. No Desk and no autopilot control (later stages).
- The five tabs (note 2; 0.11), in this order, with their subtitles: Field "the whole board", Tracks "what's running
  now", Queue "what needs you", Planning "what to build next", Comms "sessions with an agent". Keys 1 to 5 switch
  them, except while a text box has the focus. Field and Planning show a placeholder until `ts-0181-ft` and
  `ts-0195-ju`.
- Queue says it arrives in Stage 9a and Comms in Stage 9b, in the mock's words. Tracks shows a still preview of its
  design under the mock's banner ("A preview of Stage 9b… these runs are made up"), with no data and no calls.
- The tabs are a list of the window's own. `SURFACES` in `app/src/ui/state.ts`, which the in-app agent's `open`
  tool names, stays as it is: aligning the two is Stage 6b's.
- Stage 2's surfaces and invented data that the frame replaces are removed once nothing uses them. The ui reducer
  stays.
- The fake-owner page: a vite entry (`app/browser/fake/`, with a config like `vite.bridge.config.ts`) that mounts
  `App` over a `FakeOwner` playing a scripted world named in the URL hash. A Playwright fixture serves the built page
  from a small `node:http` server on loopback, with the same headers the bridge sends. Specs named
  `app/browser/fake-*.spec.ts` drive it. The worlds live in `app/test/support/`, beside `loop-world.ts`, so vitest and
  the page share them.

## Success criteria

- The window shows the five tabs with their subtitles, in order. Keys 1 to 5 switch them; the same key typed into a
  text box does not.
- The board menu lists every board of `board.list` with its default line. Switching boards changes the board the
  surfaces read and calls no stop or cancel method.
- Queue and Comms name what arrives and its stage. Tracks shows the still preview and its banner and calls no run
  method.
- "Open the Loop page" shows the loop page, and its way back returns to the tab.
- A Playwright spec over the fake-owner page passes with no daemon and no Tauri (Stage 6a: "The UI's tests run in a
  browser against a fake owner, with no Tauri").
- The loop page's tests (`app/test/loop-*.test.ts*`, `app/browser/loop.spec.ts`) pass through the new way in.

## Tests

Write these first.

- `app/test/`: the frame over a `FakeOwner` with two boards: tab names and subtitles in order, the board menu's rows
  with their lines, the Tracks preview makes no call.
- `app/browser/fake-shell.spec.ts`: keys 1 to 5, a key typed in a text box, switching boards (the fake owner's call
  log has no stop), "Open the Loop page" and back.
- `app/browser/loop.spec.ts`: reaches the loop page through the frame.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 6a: the Tauri shell bullet (the client runs in a browser against a fake owner), Tabs, Line selector;
    Success Criteria: "The UI's tests run in a browser against a fake owner, with no Tauri".
  - 0.11: the surfaces and their subtitles (revision 25); Line selector. 0.2: the UI speaks the owner's protocol.
- Design: notes 1 and 2. Mock functions: `crumbHtml`, `popHtml`, `tabsHtml`, `topRightHtml` (the Settings button
  only), `laterHtml`, and `tracksHtml` (its banner and still rows).
- **Decision for the item, with a recommendation: the fake-owner page.** Recommend the vite entry served to
  Playwright, as above. It needs no new dependency, and clicks, keys, hover and drag are tested where they behave.
  Alternatives: vitest's browser mode (`@vitest/browser`, a new dependency, so an Owner's choice), or reading the
  criterion as met by the Node renders and the bridge specs, and changing its words.
- **Decision for the item, with a recommendation: the Tracks preview.** Recommend the mock's Tracks rows as static
  React with its made-up runs and no handlers, so it follows the theme. Alternative: two images taken from the mock,
  light and dark.
- Carried from `ts-0081-wm`: "the tabs, their subtitles and the folder dialog (Stage 6a)". The tabs are here; the
  folder dialog is `ts-0191-nv`'s. Carried from `ts-0082-fp`: "Stage 6a's Playwright smoke run over `FakeOwner` is
  not built; this item's runner is what it will reuse." The fake-owner page is here; the smoke run is `ts-0196-uw`'s.
- Area: `app/src/App.tsx`, `app/src/AppShell.tsx` (or its successor), `app/src/components/`, `app/browser/fake/`,
  `app/browser/support/`, `app/test/support/`, `app/package.json` scripts.
- Dependencies: none.
- The test rule: no test touches the real home, real launchd, a real daemon, the keychain or the network. Each test
  that runs the `collide` binary clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to
  a temp root, as `host/tests/common/mod.rs` `TempRoot` does. A temp repository sets `GIT_CONFIG_GLOBAL` and
  `GIT_CONFIG_SYSTEM` to empty files. Keychain writes go through `MemorySecretStore` (`host/src/secret_store.rs`),
  never the login keychain. Browser tests run the installed Chrome with a temp profile
  (`app/browser/support/chrome.ts`), and the bridge's daemon on a temp root (`app/browser/support/world.ts`).
- Stage 6a item (revision 25 scope, split 2026-11-24). Items of Stage 6a run one at a time, in id order, because they
  share `app/`, `core`, `host` and the generated files. No builder runs `tauri build`.
- Nothing here runs a real `claude`.
- Owner decisions (2026-11-24): the owner took the recommendations at the split.

### Built

No Rust changed and nothing is generated. The window is now the board's frame; the loop page is one button away.

- `app/src/App.tsx`: `App` holds one loop (`useLoop`), the shown tab and whether the loop page is shown. It shows
  `Frame`, or the loop page under a "Back to <tab>" button. One loop serves both, so the board chosen in the frame's
  menu is the board the loop page prompts, and a run followed there is still followed after the way back.
- `app/src/frame/tabs.ts`: `TABS` (the five, in key order, with subtitles), `tabForKey`, `isTyping`, `isTab`. The list
  is the window's own; `SURFACES` and the ui reducer are untouched.
- `app/src/frame/Frame.tsx`: `Frame`, a function of its props. The top bar (brand, trail, tabs, Settings), keys 1 to 5
  on `window` (not with a modifier, not while a text box has the focus), and the page of the tab. Settings shows a
  page that says it arrives later in Stage 6a; any tab or key leaves it. `viewPicker` is the empty slot in the trail.
- `app/src/frame/BoardMenu.tsx`: `BoardMenu` (a popover) and `BoardMenuBody` (rows with name, default line and
  folder; "Open a folder…" with a typed path and the daemon's reasons for a refusal). Choosing a board only dispatches
  the loop's `board_chosen`: no call.
- `app/src/frame/boards.ts`: `boardName`, `defaultLine`. `app/src/frame/LaterPage.tsx`: the page of Queue, Comms,
  Field, Planning and Settings. `app/src/frame/TracksPreview.tsx`: the mock's banner and its two made-up runs as
  static React, with no handlers.
- `app/src/components/ui/popover.tsx`: the shadcn popover over the `radix-ui` package already installed.
- Removed, because nothing uses them now: `AppShell.tsx`, `components/{field,tracks,queue,run,compare,experiments}/`,
  `LensBanner.tsx`, `placeholders.ts`, `placeholders-detail.ts`, `stub-actions.ts`, and their tests
  (`app-shell.test.tsx`, `surfaces.test.tsx`, `context-view.test.tsx`). Kept: `components/types.ts` and `words.ts`
  (the loop page uses them), `unimplemented.ts`, the ui reducer and `SURFACES`.
- The fake-owner page: `app/browser/fake/` (`index.html`, `main.tsx`), `app/vite.fake.config.ts`,
  `app/test/support/worlds.ts` (`twoBoards`, `noBoards`, shared by vitest and the page), `board`'s new `line`
  argument in `loop-world.ts`. The page reads `#world=<name>` and exposes `window.collideFake.calls()`, the methods
  the client called. `app/browser/support/fake-site.ts` is the `node:http` server (loopback, `Host` and `Origin`
  checks, `GET` only, plain names only, `Cache-Control: no-store`, `Connection: close`, as the bridge's page route
  does). `fixtures.ts` has `fakeSite` and `openFake`; `global-setup.ts` builds the page once. `pnpm test:fake` runs
  the fake-owner specs alone.

Tests. Red at runtime means: run against an empty `Frame`, `BoardMenuBody`, `TracksPreview` and `tabs.ts`, or against
the old `App`, and failing on the missing words or elements.

- `app/test/frame.test.tsx` (38): the five tabs in order with subtitles, one selected; `tabForKey` for 1 to 5 and for
  the keys that show none; `isTyping`; the top bar (board and line, "Boards", Settings, no Desk, the view slot); the
  board menu body (every board with its line and folder, the current one, "Open a folder…", a refusal's words); each
  tab's page; the Tracks preview (banner, runs, phases in words, one button only). 33 red against the empty modules.
  The 5 that stayed green are guards that an empty module also meets: "1 shows field" and the four "anything else lets
  them through" cases of `isTyping`.
- `app/browser/fake-shell.spec.ts` (12), all red against the old `App`: the five tabs and subtitles; keys 1 to 5; a key
  typed into a text box; Queue and Comms; Field and Planning; Tracks with no call made; the board menu and "switching
  stops nothing" (the call log is unchanged and has no stop or cancel); a folder typed into the menu (opened, and
  refused with its reason); no board open; Settings; "Open the Loop page" and back from Queue and from Tracks; the
  menu's board is the loop page's board.
- `app/browser/fake-site.spec.ts` (2): red when the `Cache-Control` header and the `Host` check are removed from
  `fake-site.ts`.
- `app/browser/loop.spec.ts`: `openLoopPage` (Queue, then "Open the Loop page") before each test's first step, and
  again after each reload.

The vitest test of the Tracks preview cannot show "makes no call": `react-dom/server` runs no effects and there is no
DOM. What it checks is that the preview has no control but one. The call-free claim is the browser spec's (the fake's
call log is the same before and after showing Tracks).

### Decisions for the owner

- **One loop for the frame and the loop page.** The frame reads `boards`, `boardId` and `openFolder` from `useLoop`.
  The cost: the loop's reads (`board.list`, `queue.list`, `run.list`, `daemon.agent_versions`) also run while a tab
  shows. The alternative is a small board hook of the frame's own plus the loop mounted only on its page; then the
  board chosen in the menu would not be the loop page's board. Recommendation: as built. Stage 9a drops the loop and
  the frame keeps a board hook then.
- **The fake-owner page is built in global setup**, beside the bridge page (smallest). So `playwright test fake-` also
  runs the cargo build of the bridge. Alternative: a worker fixture that builds the page itself, so the fake specs
  need no cargo. Recommendation: as built until it is slow; `ts-0196-uw` can split it.
- **The Tracks preview always shows the two made-up runs.** The mock shows "No runs on <board> yet" for a board other
  than its own. A still preview has no board. I also left out the mock's hint "click a phase to see it alone", since
  nothing clicks. Alternative: copy both. Recommendation: as built.
- **Colours come from the existing tokens.** `index.css` has no done, working or waiting colours, so the preview's
  phases use primary shades and a ring for the one running. Recommendation: `ts-0181-ft` adds the category colours,
  and the preview moves to them.
- **Queue is dimmed until it is active**, as the mock dims it while no ask waits. Comms has no Loop-page button (the
  item names Queue and Tracks).
- **The loop page replaces the frame**, under a "Back to <tab>" bar of `App`'s own. `LoopPage` is unchanged, so its
  tests stand. Alternative: the loop page inside the frame, under the tabs. It needs a tab of its own, which the five
  do not have.
- **Words I wrote** where the mock has none: the placeholders of Field and Planning ("It arrives later in Stage 6a."),
  Comms ("Sessions with an agent that are not a task or a plan." from note 2, "It arrives in Stage 9b"), and the
  Settings page ("Settings arrive later in Stage 6a.").
- **`unimplemented.ts` stays**, with no caller. `stub-tags.json` still lists the six `ui:` tags of the removed stubs
  (9a, 9c, 15); the stub check counts them at zero. Proposed plan text: none. Recommendation: drop the six tags when a
  later item registers its own, or now if the owner prefers.
- **Chrome's temp profile is under `/tmp`** (`support/chrome.ts`, from `ts-0082-fp`). Not changed here.

### Not done, by design

- The view picker (`ts-0181-ft` fills the slot), the Field and Planning pages (`ts-0181-ft`, `ts-0195-ju`), the
  Settings page (`ts-0193-gk`).
- The system folder picker and the "folder not found" row (`ts-0191-nv`); the menu has the typed path only.
- The Playwright smoke run of scenarios 1 and 2 over `FakeOwner` (`ts-0196-uw`).
- Aligning the window's tabs with `SURFACES` and the ui reducer (Stage 6b).
