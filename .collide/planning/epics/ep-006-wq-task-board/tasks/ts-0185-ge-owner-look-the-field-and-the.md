---
id: ts-0185-ge
kind: task
status: ready
title: "Owner look: the Field and the drawer beside the mock"
owners: []
links:
  - { type: blocked-by, to: ts-0184-hv }
---

## Goal

The owner looks at the Field and the drawer over real tasks, in the dev app, beside the board mock, before the rest
of Stage 6a builds on them. A difference found here costs one fix item now, not a change to six items later. The
owner also measures how to tell, without spending, whether `claude` is logged in, which `ts-0192-es` needs.

A build agent never runs it. It never starts the app or a daemon against the owner's data, and never runs `claude`.
Once `ts-0184-hv` is done, it writes the command lines and a short checklist under `### Owner checklist`, from the
code and from the Built sections of `ts-0179-ke` to `ts-0184-hv`. The owner runs them and records the result here as
owner judgement, with what was awkward.

## Success criteria

- The Field over a clone of the Collide repository (its plan tasks in `ep-006-wq` and others) matches the mock's
  layout, words, chips, folds and empty states for notes 1 to 7 and 14. Owner-run, in the dev app.
- The drawer over the same tasks matches notes 8, 18 and 39: its parts and order, the folds, the drag, the
  double-click and the narrow overlay. Owner-run.
- A status change made by hand in a worktree of the clone shows as a divergence chip, and the card flashes. Owner-run.
- The CSP is in force in the real webview: the hostile fixture task shows as text, and the webview's console shows
  the blocked image. Owner-run, in the dev app.
- The owner records a command that says whether `claude` is logged in without spending: its exit code and output
  when logged in, and with a fresh temp `HOME`. If no such command exists, that is the result. Owner-run, spend cap
  $0 (no prompt is sent).
- Each difference is written down as a numbered note, and becomes a fix item or a change to the mock. A line records
  the date of the look.

## Tests

None by a build agent: this check is the owner's. The checklist names the automated tests it relies on:

- `ts-0179-ke`: the frame and the fake-owner page;
- `ts-0180-ad`, `ts-0181-ft`: the wire and the Field;
- `ts-0182-ad`: `watch` and live updates;
- `ts-0183-ee`: the CSP, capabilities, markdown and the link window;
- `ts-0184-hv`: the drawer.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 6a: Field; Task drawer; the Tauri shell bullet (CSP). Part L.
  - The board mock, notes 1 to 8, 14, 18 and 39.
- What the checklist covers. The build agent fills in the exact commands once the earlier items are built.
  - **Set up.** Build the CLI, the daemon and the dev app from `<collide checkout>` (`pnpm tauri dev`, not a bundled
    build). Make `<collide clone>` of this repository in a folder made for the look. Use a fresh `HOME` and
    `COLLIDE_DAEMON_ROOT` where it can; say which steps, if any, use the real `HOME`.
  - **Field.** Open `<collide clone>`; the `ready` view and one other; columns, headers, folds, cards and chips,
    beside the mock's Field.
  - **Drawer.** A plan task with links and a scratch task; parts, folds, width.
  - **Live.** A worktree of the clone; a status edit there; the chip and the flash.
  - **Webview.** The hostile fixture task; the console; an external link's window.
  - **`claude` probe.** `claude --help`, then the status command it lists, if any, logged in and with a temp
    `HOME`. No prompt is sent.
  - **Record.** The differences as numbered notes, the probe's result, the time it took, and what was awkward.
- **Owner's choice, recommendation taken (2026-11-24): the midway look.** Recommend this item: the Field and drawer are
  the base of every later item, and the owner designs by looking. Alternative: no midway look; the differences wait for
  `ts-0197-uj`.
- **Owner's choice, recommendation taken (2026-11-24): the `claude` login probe.** Recommend the owner's measurement
  here, spend cap $0. Until a command is recorded, `ts-0192-es` shows "could not check" for logged in, and never blocks.
  Alternative: infer it from files `claude` writes under `HOME`; it reads private state and breaks when `claude`
  changes.
- Area: this item's own text. No code.
- The test rule, for anything automated this check leans on: no test touches the real home, real launchd, a real
  daemon, the keychain or the network. Each test that runs the `collide` binary clears the environment
  (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as `host/tests/common/mod.rs` `TempRoot`
  does. Keychain writes go through `MemorySecretStore`, never the login keychain. The owner's own look uses the
  dev app and a clone by choice, where the checklist says so.
- Stage 6a item (revision 25 scope, split 2026-11-24). Items of Stage 6a run one at a time, in id order, because they
  share `app/`, `core`, `host` and the generated files. The next item waits for this look.
- Nothing here runs a real `claude` for a prompt. The probe runs `claude` only for its help and status, by the owner.
- Owner decisions (2026-11-24): the owner took the recommendations at the split.
- Carry from `ts-0181-ft` (2026-11-24), for the owner to judge on the look: the plan lists each epic as a task, so on
  the whole board an epic is a card beside its tasks. My recommendation: leave epics out of the Field by kind, since a
  card's stage already names its epic. Alternative: keep them as cards, or show them as a header over their tasks. A
  status column is headed by its declared name with only its first letter raised (`In_review`), as the mock's
  `columnsOf` does.

### Owner checklist

Written from the code at `1db3162` (`ts-0184-hv`), 2026-11-24. Every command is the owner's to run. The build agent ran
none, and started no app, daemon or `claude`.

Relies on: the automated tests of `ts-0179-ke` (frame, fake-owner page), `ts-0180-ad` and `ts-0181-ft` (wire, Field),
`ts-0182-ad` (`watch`, live updates), `ts-0183-ee` (CSP, capabilities, markdown, link window) and `ts-0184-hv` (drawer).
This look is what they cannot see: the real window, real files, and the mock beside it.

Placeholders: `<collide checkout>` is this repository. `<look root>` is a folder made for the look with `mktemp -d`.
`<collide clone>` is a fresh clone of the checkout, made inside `<look root>`. In the commands, `$LOOK` is `<look root>`
and `$LOOK/clone` is `<collide clone>`.

Where the real `HOME` is used, and why. Everything else runs with a fresh `HOME` and `COLLIDE_DAEMON_ROOT` under
`<look root>`. With the root set, the daemon never touches `~/Library/Application Support/Collide/`
(`DaemonPaths::from_env` in `host/src/daemon.rs`).

- Step 1, the build: cargo and rustup find their toolchains and crate cache through `HOME`, and pnpm its store.
- Step 4, `tauri dev`: runs with the fresh `HOME`, so `CARGO_HOME` and `RUSTUP_HOME` are set to the real folders and the
  compile still finds the toolchain. If that step fails only because of the fresh `HOME`, run that one command with the
  real `HOME` and keep the rest of the environment. The daemon then reads the real `HOME`, and its root stays in
  `<look root>`. Say so in Record.
- The `claude` probe, steps 27 and 28: the first run uses the real `HOME`, since that is where a login lives.
- The webview keeps its storage (the folds and the drawer width) under `HOME`, so under the fresh one it starts empty.
  Not confirmed from the code: that WebKit follows `HOME`. If folds or a width are already set on the first look,
  the real one was used. Record it.

**Set up**

1. In a first terminal, build from `<collide checkout>`, with the real `HOME`.

```sh
cd <collide checkout>
pnpm install
pnpm build:packs
pnpm --filter @collide/app build
cargo build -p collide-host
```

   `app/src-tauri/tauri.conf.json` has `frontendDist: ../dist` and no `devUrl` or `beforeDevCommand`, so `tauri dev`
   shows what `app/dist` holds: the third command is needed. The agent's run saw the window draw that folder's
   page.

2. In the same terminal, make `<look root>`, a fresh `HOME`, and the environment file. The `shell` script hands the
   daemon this terminal's `PATH`: a fresh `HOME` reads none of your shell's startup files, and `collide/plan` needs
   `node` 22.18 or newer (`packs/plan/pack.yaml`). The daemon runs `$SHELL -l -i -c <script>` to read its environment
   (`host/src/base_env.rs`), so the script's fourth argument is that text, as in `app/browser/support/world.ts`.

```sh
LOOK="$(cd "$(mktemp -d)" && pwd -P)"; echo "$LOOK"
mkdir "$LOOK/home" "$LOOK/claude-home"
printf '#!/bin/sh\nexport PATH="%s"\neval "$4"\n' "$PATH" > "$LOOK/shell"
chmod +x "$LOOK/shell"
cat > "$LOOK/env.sh" <<EOF
export LOOK="$LOOK"
export COLLIDE_BIN="$PWD/target/debug"
export CARGO_HOME="${CARGO_HOME:-$HOME/.cargo}"
export RUSTUP_HOME="${RUSTUP_HOME:-$HOME/.rustup}"
export HOME="$LOOK/home"
export COLLIDE_DAEMON_ROOT="$LOOK/d"
export SHELL="$LOOK/shell"
EOF
printf %s "$LOOK/d/run/d.sock" | wc -c
```

   The last number must be 103 or less (`MAX_SOCKET_PATH_BYTES`, `host/src/daemon.rs`). Over it, the daemon will not
   start: use a shorter `TMPDIR`. The `pwd -P` matters: the board compares real paths.

3. Make the clone, in the same terminal. It has this repository's plan: the tasks of `ep-006-wq` and the other epics.

```sh
git clone <collide checkout> "$LOOK/clone"
git -C "$LOOK/clone" log --oneline -1
```

4. Open a second terminal. Load the environment and start the app. It builds `collide-app` into `target/debug`, which
   starts the `collide-daemon` beside itself (`LaunchStarter` in `app/src-tauri/src/shell.rs`). The first build takes
   minutes. Keep the terminal in view: the app's errors print there. The window opens at 800 × 600, under the 900 px
   of step 19, so the drawer lies over the board until you widen the window.

```sh
. <look root>/env.sh
cd <collide checkout>
pnpm --filter @collide/app exec tauri dev
```

5. In the window the Field says "No board is open". Open the board menu (top left, "Boards"), type the clone's absolute
   path in "Folder path" (`echo "$LOOK/clone"` in the first terminal) and press "Open folder". The menu button then
   names the clone and its line.

6. Open `<collide checkout>/docs/mockups/board.html` in a browser, beside the window. Press "Mock" (bottom left), then
   "Show design notes". The mock's default scenario is the one to compare with.

7. Open a third terminal and load the environment there. The second one is busy with `tauri dev`. Every `collide`,
   `git` and `sed` command from here to the clean-up runs in this third terminal: the first one has the real `HOME`
   and no `COLLIDE_DAEMON_ROOT`, so `collide` there would reach your real daemon. List what is ready, to compare with
   the app.

```sh
. <look root>/env.sh
echo "$COLLIDE_DAEMON_ROOT"
cd "$LOOK/clone"
"$COLLIDE_BIN/collide" tasks --ready
```

**Field** (key 1; the view picker reads "All tasks")

8. The frame, notes 1 and 2. The trail at the top left is the board menu, then the view picker. Settings is at the top
   right. Five tabs with subtitles and keys 1 to 5. Queue and Comms say when they arrive. Tracks shows a still preview
   under a banner that says its runs are made up. Switching a view stops nothing.

9. Columns and headers, note 3. Each header is the status name with its category in words beneath ("Ready", "can start";
   "Review", "waiting on a review"). The plan declares `ready`, `active`, `review`, `done` and `dropped`, so there is no
   Backlog column (`ts-0181-ft` Decisions). A status a task has that the plan did not declare is added after the
   declared ones (spikes may add some). Write down any column that looks wrong.

10. Folds, note 4. Done and Dropped start folded and show their counts. A click opens one. Fold and open a column, then
    pick another view and come back: a fold is kept for each view.

11. Cards, note 5. Title first, then the short id and the stage. A chip only when it says something: "Waits on N", or a
    differing copy. A card of an `active` task has a lit border (step 20 makes one).

12. Carry from `ts-0181-ft`, for you to judge. The plan lists each epic as a task, so an epic is a card on the whole
    board, beside its tasks. Find one (its id starts `ep-`). The recommendation is to leave epics out of the Field by
    kind, since a card's stage already names its epic. The alternatives are cards, or a header over their tasks. Record
    your choice.

13. Carry, the header words. A column is headed by its declared name with only its first letter raised, so a tracker
    status `in_review` reads `In_review`. The plan has no such status, so you see "Review" here, not `In_review`.
    Judge the rule from that and from the mock's `columnsOf`. Record whether the underscore should show.

14. Pick `ready` in the view picker. Its cards are the ones step 7 listed. The line above the board reads like the
    CLI's own lines. An empty column says "Nothing here". Then pick `collide/plan/spikes` (the picker names a pack's
    view by its source): only spikes show. All ten are concluded, so they are in Concluded, folded.

15. Make a scratch task in the look's own daemon root (it is kept in the store, in no file), and read its key.

```sh
cd "$LOOK/clone"
"$COLLIDE_BIN/collide" task new "Look: a scratch task"
```

    The Field then has two sources, so it groups by the six categories and each card carries a status chip
    (`groupMode` in `app/src/field/model.ts`; note 5). Pick "All tasks" again.

**Drawer** (notes 8, 18 and 39)

16. Click the card of `ts-0185-ge` (it waits on `ts-0184-hv`, and `ts-0186-ch` waits on it). The drawer opens beside the
    board. Compare with the mock's drawer. In order: the header (status chip, category in words, id, Close), the title,
    "Read only. Edit the file, or ask an agent; the board follows it.", Description, Waits on, Unblocks, Part of
    (the epic, and a chip for each view that holds it), "In each copy of the project", and "Details for engineers",
    folded at first. Absent on purpose: Changes (`ts-0189-fg`), the questions part (9a), Discuss with the Desk (9e).
    "Fields from plan" is absent too: the plan declares no fields, so note 18 cannot be seen on this clone.

17. Folds, note 39. Fold Description and open Details. Click another card: the folds hold. Click a link in Waits on: the
    drawer shows that task. Press Escape: the drawer closes and the focus goes back to the card.

18. Width. Drag the drawer's left edge (a 12 px strip, half over the drawer). It stops at 360 px and at 70% of the
    window. Click the edge or Tab to it: ArrowLeft widens and ArrowRight narrows by 40 px (Shift: 120). A double-click
    goes back to 460 px. Open another task: the width holds.

19. Make the window narrower than 900 px. The drawer lays over the board and the edge is gone. Then widen it again.
    Click the scratch card: no Waits on or Unblocks, no "In each copy of the project", and "Read only. Change it in
    scratch."

**Live** (note 14)

20. Edit a status in the main checkout of the clone, by hand. The task is any `status: ready` file. The card should
    leave Ready for Active within about a second, flash once, and the stamp reads "Live · updated just now". No toast.

```sh
cd "$LOOK/clone"
T=.collide/planning/epics/ep-006-wq-task-board/tasks
sed -i '' 's/^status: ready$/status: active/' $T/ts-0185-ge-*.md
grep -n '^status:' $T/ts-0185-ge-*.md
```

21. Make a worktree. The board shows a divergence only for a worktree under the daemon's workspaces folder,
    `<COLLIDE_DAEMON_ROOT>/data/workspaces/`, whether a run or you made it. A worktree made anywhere else is a copy and
    no chip shows (`discover` in `host/src/corpus_service.rs`; `ts-0182-ad` Decisions). Wait about 15 s for the daemon
    to see it (it looks again every 10 s).

```sh
W="$COLLIDE_DAEMON_ROOT/data/workspaces/wt-a"
mkdir -p "$COLLIDE_DAEMON_ROOT/data/workspaces"
git -C "$LOOK/clone" worktree add -b look/a "$W"
```

22. Edit a status in the worktree, by hand.

```sh
sed -i '' 's/^status: ready$/status: review/' "$W/$T"/ts-0188-kb-*.md
grep -n '^status:' "$W/$T"/ts-0188-kb-*.md
```

    Within about 11 s (the debounce, plus the 10 s recheck) the card of `ts-0188-kb` gets a chip, "Review in wt-a", and
    flashes once. Compare with the mock's "Review in Run R-121" (note 6). A hand-made worktree has no run, so the chip
    names its folder. Open the card: its copy row reads "differs from main", with the path under the label.

**Webview**

23. Open the web inspector: right-click the window, then "Inspect Element" (Tauri's debug builds have it; not confirmed
    from the code). Read the Console from the top; if it opened late, reload and read it again. Look for: no policy
    violation, the window loaded and styled, the board loaded and updated (steps 5 to 22). From `ts-0183-ee`
    Decisions, on first launch.

24. The hostile task. Add its file to the clone. The body is `HOSTILE_BODY` in `app/test/support/hostile.ts`. The board
    shows the new card without a reload.

```sh
cd "$LOOK/clone"
T=.collide/planning/epics/ep-006-wq-task-board/tasks
cat > $T/ts-9001-hz-hostile-look.md <<'EOF'
---
id: ts-9001-hz
kind: task
status: ready
title: "Hostile body look"
owners: []
---

## Steps

<script>window.__pwned = 'script'</script>

<img src="http://127.0.0.1:9/beacon.png" onerror="window.__pwned = 'onerror'">

<a href="javascript:window.__pwned = 'raw anchor'">raw anchor</a>

[script link](javascript:window.__pwned='link')

[file link](file:///etc/passwd)

[data link](data:text/html,hello)

![tracking pixel](http://127.0.0.1:9/pixel.png)

[the docs](https://example.com/docs?page=1&lang=en)

[write to us](mailto:team@example.com)

| left | right |
| :--- | ----: |
| a | b |
EOF
```

    Open it. The script and image lines show as visible text. The `javascript:`, `file:` and `data:` links are plain
    text followed by "(link not opened: <scheme>: links are never opened)". The image reads "[image not loaded:
    tracking pixel, from http://127.0.0.1:9/pixel.png]". The table is a table. Nothing runs, no dialog or window opens,
    and the console shows no new error.

25. Click "the docs". The window "Open this link in your browser?" names `example.com` and the whole address, and Cancel
    is first and has the focus. Cancel, and Escape, open nothing. Click "write to us": the window shows
    `mailto:team@example.com` and names no host. "Open in browser" on the https link opens your default browser on that
    address, a real request made by your browser: press it only if you want that.

26. The policy probe. In the inspector's console type this. A policy in force says it refused the image under
    `img-src`. With no policy the console shows a host-not-found error: `.invalid` never resolves, so nothing leaves the
    machine.

```js
new Image().src = 'https://collide-probe.invalid/x.png'
```

    Then type `location.href`. An address on Tauri's own protocol (`tauri://localhost`) is the page with the policy.
    A `http://127.0.0.1:…` or `localhost` address is a dev server, which sends none. With no `devUrl`, `tauri dev`
    serves `app/dist` itself (step 1), so expect `tauri://localhost` under `devCsp`; the agent's run could not open
    the inspector to see it. A dev-server address instead is the result to record, and the bundled check is
    `ts-0197-uj`.

**`claude` probe.** No prompt is ever sent. Do not run `claude` bare, with `-p`, or in a session. Use the first
terminal, which has the real `HOME`.

27. Read the help, and find the command it lists that reports login or auth status. Do not guess one. If it lists none,
    that is the result. On the agent's run the help listed `auth`, and `claude auth --help` listed `status`.

```sh
claude --help; echo "exit=$?"
```

28. Run the status command, logged in, with the real `HOME`. Record the exit code and the output.

```sh
claude <status command>; echo "exit=$?"
```

29. Run it again with `HOME` set to an empty folder. Record the exit code and the output. On the agent's run it
    answered `"loggedIn": false` with exit 1, and wrote `.claude/` and `.claude.json` into the folder.

```sh
HOME="$LOOK/claude-home" claude <status command>; echo "exit=$?"
```

**Record.** Fill this in below the checklist, before the clean-up.

30. Copy and fill. Each numbered difference becomes a fix item or a change to the mock.

```text
Look on <date>. It took <minutes>. Awkward: <what>.
Differences (mock note, what the mock shows, what the app shows, fix item or mock change):
1. <...>
Policy probe: page address <...>; console said <...>.
claude: status command <name or none>; logged in: exit <n>, <output>; empty HOME: exit <n>, <output>.
Judged: epics on the Field <leave out | cards | header>; `In_review` header <keep | change>.
```

**Clean up**

31. Quit the app and press Ctrl-C in the second terminal. Quitting does not stop the daemon, and there is no stop
    command for it. In the third terminal, stop the daemon of the look by the pid in its own log, then check that
    nothing of the look is left.

```sh
. <look root>/env.sh
pid="$(grep 'starting, pid' "$COLLIDE_DAEMON_ROOT/data/daemon.log" | tail -1 | sed 's/.*pid //')"
ps -p "$pid" -o pid=,command=
kill -TERM "$pid"
ps -ax -o pid=,command= | grep -F "$LOOK"
```

32. Remove the look, then check that `<collide checkout>` is only built, not changed. Close the second and third
    terminals: they hold the fresh `HOME`. The daemon keeps its pack cache read-only (`host/src/pack_cache.rs`), so
    `rm -rf` alone fails on it.

```sh
echo "$LOOK"
chmod -R u+w "$LOOK" && rm -rf "$LOOK"
git -C <collide checkout> status --short
```

### The agent's run, 2026-11-24

The agent ran the checklist on a throwaway look before yours. It cannot click in the app's window, so the steps after
4 ran on the test bridge in Chrome: the same page and daemon, under the app's policy. The app's window was only seen to
start, draw and reach its own daemon.

- Passed: 1 to 5, 7 to 11, 14 to 18, 20 to 22, 24 and 25. On the bridge page the probe of step 26 was refused under
  `img-src`. `claude`: the status command is `claude auth status`; logged in, exit 0 and `"loggedIn": true`; an empty
  `HOME`, exit 1 and `"loggedIn": false`.
- Fixed before your look: `tauri dev` started nothing (the crate has two binaries and named no `default-run`); below
  900 px the drawer was as tall as its text, so the end of a long task was out of reach; Cancel in the link window
  left the focus nowhere (Escape gave it back); step 32's `rm -rf`.
- Changed at your word after the run: the page was at most 1152 px wide, where the mock's board fills the window, so
  on a wide window the drawer at its limit covered the board. The page now fills the window.
- Left for your look: the window's first size is 800 × 600 (step 4).
- Not run by the agent: 6, 12 and 13 (yours to judge), 19 in the app's window, and 23 and 26 in its inspector.

### The owner's look, 2026-11-25

```text
Look on 2026-11-25. Awkward: none named.
Differences: none named.
Policy probe: page address http://127.0.0.1:1430/; console said "Failed to load resource: A server with the
specified hostname could not be found", and no img-src refusal.
claude: not run on the look; the agent's run above has it.
Judged: epics on the Field: cards, as an Epic card that shows all of the epic's tasks, so that an agent can be set to
run through them. `In_review` header: change, no underscore shows.
```

- Step 19, in the app's window: below 900 px the drawer scrolls to the end of a long task.
- Step 23: no errors in the webview's console.
- Step 26: no policy is in force under `tauri dev`. With no `devUrl`, it serves `app/dist` from the Tauri CLI's own
  dev server on `127.0.0.1:1430`, not on `tauri://localhost` as the step expected. That server sends no policy, so the
  image was asked for, and failed only because `.invalid` never resolves. `ts-0183-ee` Decisions say the same of a dev
  server: Tauri sends `csp` and `devCsp` only on its own protocol. The hostile text still showed as text (step 25),
  kept inert by the renderer. The bundled app's policy is checked in `ts-0197-uj`; the plan's wording on dev changes
  with the next revision.
- Step 25: two screenshots of the drawer over the hostile task. Its body shows as text, the refused links and the image
  say why, and the kept links are buttons. They stay with the owner: they show a local temp path.
- The header: fixed after the look. An underscore in a status name shows as a space (`in_review` reads "In review"),
  in the app and in the mock's `columnsOf`. A hyphen stays, as in "On-hold".
- The Epic card: a change to the mock and the plan, shaped with the owner before it is split into items.
