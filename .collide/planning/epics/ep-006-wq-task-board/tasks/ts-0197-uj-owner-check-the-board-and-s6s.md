---
id: ts-0197-uj
kind: task
status: ready
title: "Owner check: the board and S6's walkthrough"
owners: []
links:
  - { type: blocked-by, to: ts-0218-sc }
---

## Goal

The Stage 6a checks that only the owner runs: the walkthrough of S6's scenario 2, and scenario 1 up to the board, in
the bundled app, timed beside S6's advisory limits; and the checks that only the bundled app shows (the CSP and
capabilities in the real webview, the folder picker, external links through the OS, the copy buttons, a secret in
the real keychain).

A build agent never runs it. It never runs `tauri build`, never starts the app or a daemon against the owner's data,
and never runs `claude`. It writes the command lines and a short checklist under `### Owner checklist`, from the
code and from the Built sections of `ts-0179-ke` to `ts-0196-uw`. The owner runs them and records the result here
as owner judgement, with what was awkward.

## Success criteria

- The owner's walkthrough of S6 scenario 2, and scenario 1 up to the board, is recorded: the time per scenario
  beside S6's advisory limit (S1 150 s, S2 60 s), the errors, and what was awkward (R5). Owner-run, in the bundled
  app. S6's thresholds are advisory (Stage 1).
- The bundled app's webview runs under the CSP, and a page script cannot call any command but `rpc`. Owner-run.
- An external link asks first, then opens in the default browser. Owner-run.
- "Open a folder…" and "Locate the folder…" open the system folder picker. Owner-run.
- A Copy button on the first-run page puts its command on the clipboard. Owner-run.
- A secret set from Settings lands in the login keychain under Collide's service, and Settings shows it Set, never
  its value. Owner-run, with a throwaway value the owner removes afterwards.
- The differences from the mock that `ts-0185-ge` recorded are each closed or carried. Owner-run.
- The checklist holds the command lines, with `<collide checkout>` and `<scratch repo>` as placeholders, what to look
  at, how to clean up, and a line for the owner's result and one for what was awkward.
- A line records the date Stage 6a is judged done by the owner.

## Tests

None by a build agent: this check is the owner's. The checklist names the automated tests it relies on:

- `ts-0179-ke`: the frame and the fake-owner page;
- `ts-0180-ad` to `ts-0182-ad`: the wire, the Field and live updates;
- `ts-0183-ee`, `ts-0184-hv`: the webview rules and the drawer;
- `ts-0186-ch`, `ts-0187-hb`: `unblocks`, `bottlenecks`, run order and graph;
- `ts-0188-kb`, `ts-0189-fg`: Changes, the diff window and the id peek;
- `ts-0190-tz`, `ts-0191-nv`: the filter bar and views; boards, missing and locate;
- `ts-0192-es` to `ts-0194-qp`: the first-run checks, Settings, the empty board, Share with team and trust;
- `ts-0195-ju`: Planning;
- `ts-0196-uw`: the S6 smoke run.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 6a: Success Criteria (S6's scenarios and the owner's walkthrough; the external link; the CSP); the Tauri
    shell bullet.
  - R5; Stage 1 (S6's thresholds are advisory). Part L.
- What the checklist covers. The build agent fills in the exact commands once the earlier items are built.
  - **Set up.** The owner builds the bundled app (`tauri build --debug --no-bundle`, about a minute on a warm
    `target`) from `<collide checkout>`.
    Make `<scratch repo>` in a folder made for the check. Say which steps use the real `HOME` (the bundled app
    does), and how to remove what they leave.
  - **Scenario 1.** Open `<scratch repo>` as a fresh folder; the checks; the empty board; Start with a task. Time
    it.
  - **Scenario 2.** The S2 fixture board of `ts-0196-uw`; the chips; Save as a view "Slow spots". Time it.
  - **Bundled-app checks.** The CSP (the webview's console with the hostile fixture task); the capability list; an
    external link; the folder picker; Copy; a secret set and removed; Start with defaults from the app menu and from
    its key with the page broken by a fixture, then Reset… (`ts-0215-gt`, revision 29).
  - **Record.** The times beside the limits, the errors, what was awkward, and the date.
- **Owner's choice: the S2 board for the walkthrough.** Recommend the fixture board of `ts-0196-uw`, so the five
  targets are known. Alternative: a copy of a real board with labels added by hand.
  - Decided: the fixture board of `ts-0196-uw` (`fixtures/packs/s6-board/`), so the five targets are known.
- Area: this item's own text. No code.
- The test rule, for anything automated this check leans on: no test touches the real home, real launchd, a real
  daemon, the keychain or the network. Each test that runs the `collide` binary clears the environment
  (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as `host/tests/common/mod.rs` `TempRoot`
  does. Keychain writes in tests go through `MemorySecretStore`, never the login keychain. The owner's own run here
  uses the bundled app, the real `HOME` and the login keychain by choice, where the checklist says so.
- Stage 6a item (revision 25 scope, split 2026-11-24). Items of Stage 6a run one at a time, in id order, because they
  share `app/`, `core`, `host` and the generated files.
- Nothing here starts a run of a real `claude`. Scenario 1 stops at the board, before any run. The first-run checks
  (step 7) do start `claude --version`; with a root override there is no login probe, and Part B has one.
- Owner decisions (2026-11-24): the owner took the recommendations at the split.

### Owner checklist

You run this by hand. Nothing here is automated. Placeholders: `<collide checkout>` (this repository), `<check root>`
(a folder made for the check), `<s1 repo>` and `<s2 repo>` (made under `<check root>`). In the command blocks they are
`$CO`, `$CHECK`, `$S1` and `$S2`.

**What the build is.** `app/src-tauri/tauri.conf.json` has `bundle.active: false`, so `tauri build` makes no `.app`:
it makes the binary `target/<profile>/collide-app`, with the page (`app/dist`) compiled into it. You run that binary.
The daemon is another crate (`collide-host`): `collide-daemon` and `collide` must sit in the same `target/<profile>`,
because the app starts the daemon from beside itself (`host/src/launch.rs`, `ensure_running_from_cli_kind`). The
built-in packs are found by `host/src/pack_location.rs`, `locate_bundle`: `app/src-tauri/resources/builtin-packs`
above the binary, which `pnpm build:packs` fills.

**Which bundle.** Recommended: one debug bundle for everything.

- The web inspector exists only in a debug build (`tauri-runtime-wry` turns it on under `debug_assertions`, and the
  crate has no `devtools` feature). A release bundle has none, so none of the console checks could run in it.
- A debug bundle still carries the production policy. `tauri build` (debug or release) uses the `custom-protocol`
  feature, so the page loads from `tauri://localhost` and Tauri sends `app.security.csp`, not `devCsp`. Step 14 checks
  this by `location.href`.
- The two timings are your own hand time on the same React page and the same daemon in either build. Build time,
  debug, on a warm `target/debug` (the agent's run, 2026-11-26): `pnpm build:packs` 2 s, `pnpm --filter @collide/app
  build` 2 s, `cargo build -p collide-host` 16 s, `tauri build --debug --no-bundle` 45 s (it recompiles `tauri`,
  `wry` and `collide-app`); about 65 s in all. A cold `target` takes longer. Release: this item's earlier estimate
  was about 40 minutes, and no release build has ever been made here.
- Alternative: add a release build after the debug one, and repeat steps 6 to 8 and 10 to 12 only if a time is
  near its limit. You then lose the console in that run. Command: the build step below without `--debug`.

**What an overridden daemon root changes.** With `COLLIDE_DAEMON_ROOT` set (`host/src/daemon.rs`, `ROOT_ENV`) the
daemon starts with `setsid`, not a launchd job (`host/src/launch.rs`, `launchctl_from_env`), keeps secrets in memory
and not in the keychain (`host/src/secret_store.rs`, `secret_store_for_root`), and answers "could not check" to the
login probe, with no Copy button (`host/src/first_run.rs`). So every step runs under an overridden `HOME` and root,
except Part B, which sets only `HOME`, because the keychain, the launchd job and the login row need a daemon that
runs as for a person. The steps that need the real `HOME`, and what they leave:

- Step 1, the build: cargo, rustup and the pnpm store live in the real `HOME`. It leaves `target/`, `node_modules/`,
  `app/dist` and the staged packs in `<collide checkout>`; step 44 shows git sees nothing new.
- Steps 37 and 41, `security`: the login keychain is the real user's. Step 41 deletes the item.
- Part B (steps 31 to 41): the daemon loads a launchd job in your real user domain, and the keychain item goes to
  your real login keychain. Stopping the daemon boots the job out (`boot_out_recorded_job`); step 42 checks it.
- Step 21, "Open in browser": your real default browser makes a request. Close the tab after.
- Anything the webview writes in `~/Library`. WebKit does not follow an overridden `HOME` (the agent's run saw the
  files change): it keeps the page's data in `~/Library/WebKit/collide-app` and `~/Library/Caches/collide-app`, and
  a preferences file `~/Library/Preferences/collide-app.plist`, named for the binary. They exist already from the
  earlier looks, so the diff of step 42 shows nothing new. The page's stored state (the folds, the drawer width)
  is in there, shared by every run of a bare `collide-app`. Steps 2 and 42 list `~/Library` before and after.

Nothing here touches your real daemon root (`~/Library/Application Support/Collide`), `~/.config/collide` or its
trust file, unless step 32's fallback says so.

**Set up**

1. In terminal 1 (real `HOME`), build. Run it from `<collide checkout>`. Keep terminal 1 for `security`, `find` and
   the clean-up. Check the three binaries exist.

```sh
cd <collide checkout>
pnpm install
pnpm build:packs
pnpm --filter @collide/app build
cargo build -p collide-host
pnpm --filter @collide/app exec tauri build --debug --no-bundle
ls target/debug/collide-app target/debug/collide target/debug/collide-daemon
```

   `tauri build` runs no front-end build of its own (`tauri.conf.json` has no `beforeBuildCommand`), so the third
   line is needed. If this CLI refuses `--no-bundle`, drop it: `bundle.active` is already false. The agent's run took
   it as written: the last lines read `Built application at: <collide checkout>/target/debug/collide-app`, no
   `.app` and no `bundle` folder; `collide-app` is about 30 MB.

2. Still in terminal 1: make `<check root>`, the shell stub, the two environment files, and the "before" lists. The
   stub hands the daemon this terminal's `PATH`: a fresh `HOME` reads none of your shell's startup files, and the
   built-in plan pack and the S2 pack run `node` 22.18 or newer (`packs/plan/pack.yaml`). The daemon runs
   `$SHELL -l -i -c <script>` to read its environment (`host/src/base_env.rs`), so the stub's fourth argument is that
   script. The folder is under `/tmp` so that the socket path stays short.

```sh
CO="$PWD"
CHECK="$(cd "$(mktemp -d /tmp/collide-check.XXXXXX)" && pwd -P)"; echo "$CHECK"
mkdir "$CHECK/home" "$CHECK/homeb"
printf '#!/bin/sh\nexport PATH="%s"\neval "$4"\n' "$PATH" > "$CHECK/shell"
chmod +x "$CHECK/shell"
cat > "$CHECK/env.sh" <<EOF
export CO="$CO" CHECK="$CHECK" BIN="$CO/target/debug" S1="$CHECK/s1-repo" S2="$CHECK/s2-repo"
export HOME="$CHECK/home" COLLIDE_DAEMON_ROOT="$CHECK/d" SHELL="$CHECK/shell"
EOF
cat > "$CHECK/env-b.sh" <<EOF
export CO="$CO" CHECK="$CHECK" BIN="$CO/target/debug" S1="$CHECK/s1-repo" S2="$CHECK/s2-repo"
export HOME="$CHECK/homeb" SHELL="$CHECK/shell"
unset COLLIDE_DAEMON_ROOT
EOF
printf %s "$CHECK/homeb/Library/Application Support/Collide/run/d.sock" | wc -c
find ~/Library -maxdepth 3 -iname '*collide*' 2>/dev/null | sort > "$CHECK/lib-before.txt"
launchctl list | grep dev.collide.daemon | sort > "$CHECK/launchd-before.txt"; true
```

   The number must be 103 or less (`MAX_SOCKET_PATH_BYTES`, `host/src/daemon.rs`). Over it the daemon will not start:
   use a shorter folder. `pwd -P` matters: the board compares real paths. `env.sh` is Part A (root set), `env-b.sh`
   is Part B (no root). The agent's run printed 86. The `find` over `~/Library` is slow: 100 s the first time, 10 s
   the next.

3. Open terminal 3. Load `env.sh`. Make `<s1 repo>` and `<s2 repo>` as git repositories, with a repo-local identity
   (no global git config is touched). `<s2 repo>` gets the fixture board, as `app/browser/s6.spec.ts` does
   (`openSlowSpotsBoard`): the pack copied into `.collide/packs/`, named in `.collide/board.yaml`, committed.

```sh
. "$CHECK/env.sh"
for repo in "$S1" "$S2"; do
  git init -q -b main "$repo"
  git -C "$repo" config user.name check
  git -C "$repo" config user.email check@example.invalid
  echo "# check" > "$repo/README.md"
  git -C "$repo" add README.md
  git -C "$repo" commit -q -m first
done
mkdir -p "$S2/.collide/packs"
cp -R "$CO/fixtures/packs/s6-board" "$S2/.collide/packs/s6-board"
printf 'packs:\n  - source: repo:.collide/packs/s6-board\n' > "$S2/.collide/board.yaml"
git -C "$S2" add .
git -C "$S2" commit -q -m 'the S6 board'
echo "$S1"; echo "$S2"
```

   Keep terminal 3 for `collide`, `git` and `ls` in Part A. Its `HOME` and root are the check's, so `collide` reaches
   the check's daemon and never yours.

4. Open terminal 2. Load `env.sh` and start the app. Keep the terminal in view: the app's errors print there. When
   all is well it prints nothing. The window may open behind other windows: click the Dock icon. It opens at 800 x
   600; widen it past 900 px, or the drawer lies over the board. The daemon starts at once under `<check root>/d`
   (`d/data/daemon.log`, `d/run/d.sock`), as the app's child, and outlives it (step 30).

```sh
. "$CHECK/env.sh"
"$BIN/collide-app"
```

5. The window shows "Open a project" with "Choose a folder…": the daemon of the check has no board yet. If it shows
   an error instead, read terminal 2 and stop; record it.

**Part A. Scenario 1, up to the board** (limit 150 s)

S6's clock starts at the first action, and S6's limit also covered about 20 s of waiting for a run. This walkthrough
stops at the board, before any run, so the time is a lower bound.

6. Start the clock on clicking "Choose a folder…". In the system picker press Command-Shift-G, type the text that
   `echo "$S1"` printed in terminal 3, press Return, then "Open". Time spent in the picker counts.

7. The page lists "Checks": "A git repository" (ok); "Claude Code is installed" (ok, with its version, from
   `claude --version`); "Could not check that Claude Code is logged in" (a dash: this daemon has a root override, so
   it does not ask `claude`); "GitHub" (a dash: Stage 5). No row shows a cross. Click "Open the board".

8. The board is empty: "Nothing on this board yet". In "What needs doing" type `Add a --quiet flag to the CLI` and
   click "Add as a scratch task". Stop the clock when the card with that title shows on the board and "Empty board" is
   gone. Write the time down: scenario 1.

9. In terminal 3, read what the daemon holds. One line must read `scratch:1  ready  -  Add a --quiet flag to the
   CLI`.

```sh
cd "$S1" && "$BIN/collide" tasks
```

**Part A. Scenario 2** (limit 60 s)

10. Open the board menu (top left: with a board open it is labelled with the board's name, `s1-repo`). Under "Open
    boards" it has the heading "Open a folder…" and the button "Choose a folder…". Click the button. The system
    picker opens. Press Command-Shift-G, type the text that `echo "$S2"` printed, Return, "Open". This is the "Open
    a folder…" check: it must be the system picker, folders only. (Below the button the menu has a text box and
    "Open folder": a typed path works too, and is not this check.)

11. The page lists "Checks" for `<s2 repo>`. Click "Open the board". The window "Trust the packs this repo
    brings?" names `s6-board` and says it runs code on this Mac. Click "Trust and open". The Field fills with 17
    tasks, and the "Epics" strip shows three cards: Performance, Cache layer, Config and CLI (in that order). Wait
    until the cards are there.

12. Start the clock on the first click of "Show only the tasks of Performance" (the funnel on the Performance card).
    Then, in the "Filters" bar: the chip "Ready" (group "Where it stands"), the chip "perf" (group "Labels"). The
    count reads "5 tasks". Click "Save as a view…", replace "My view" with `Slow spots` (capital S, one space), click
    "Save view". Stop the clock when the heading "Slow spots" shows over a board of exactly five cards: Speed up index
    scan, Trim allocation churn, Speed up startup path, Rework batch size, Fix lock contention. Write the time
    down: scenario 2. With only the epic chosen the view holds 13 tasks and is not the end state.

13. In terminal 3, check the saved view by the CLI and by its file (`host/src/task_cli.rs`, `parse_tasks`;
    `host/src/views.rs`, `view_file_stem`). The CLI must list the five keys `s6:T-07-001`, `s6:T-07-004`,
    `s6:T-07-007`, `s6:T-07-010`, `s6:T-07-013`. The file is `slow-spots.yaml` and holds `name: "Slow spots"` (the name
    is quoted) and a `where:` line.

```sh
cd "$S2" && "$BIN/collide" tasks --view "Slow spots"
cat "$HOME"/.config/collide/boards/*/views/slow-spots.yaml
```

**Part A. Bundled-app checks**

The console (steps 14 to 17) needs the debug bundle's inspector.

14. Open the web inspector: right-click the page, then "Inspect Element", then the Console. No such item means this is
    not a debug build: record it and skip to step 18. In the console type `location.href`. It must start with
    `tauri://localhost`. A `http://127.0.0.1:…` address means no policy is sent (the `tauri dev` result of
    `ts-0185-ge`): record it.

15. The policy probe. Type this. A policy in force says the image was refused under `img-src`. With no policy the
    console shows a host-not-found error instead: `.invalid` never resolves, so nothing leaves the machine.

```js
new Image().src = 'https://collide-probe.invalid/x.png'
```

16. A page script may call no command but `rpc` (`app/src-tauri/capabilities/main.json` allows only `allow-rpc` and
    the listen and unlisten events; `build.rs` lists the one app command, `rpc`). Type both lines. Each must be
    refused: the promise rejects and the console prints the error. No folder picker may open. The error's words are
    Tauri's; they were not confirmed from the code, so a refusal with nothing opening is the pass.

```js
window.__TAURI_INTERNALS__.invoke('plugin:dialog|open', {}).then(console.log, console.error)
window.__TAURI_INTERNALS__.invoke('open_external', { url: 'https://example.com' }).then(console.log, console.error)
```

17. Read the console from the top. No other policy violation or error since the app started.

18. The hostile task. In terminal 3, add an epic and a task to `<s1 repo>`: the body is `HOSTILE_BODY` in
    `app/test/support/hostile.ts`. Nothing needs committing: the plan pack reads the working tree. `collide tasks` in
    `<s1 repo>` lists `plan:ts-9001-hz` a few seconds after the files are written, not at once.

```sh
E="$S1/.collide/planning/epics/ep-009-hz-look"
mkdir -p "$E/tasks"
cat > "$E/epic.md" <<'EOF'
---
id: ep-009-hz
kind: epic
status: active
title: "Hostile look"
owners: []
links: []
---
EOF
cat > "$E/tasks/ts-9001-hz-hostile-look.md" <<'EOF'
---
id: ts-9001-hz
kind: task
status: ready
title: "Hostile body look"
owners: []
links: []
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

19. In the window, open the board menu and pick the `<s1 repo>` board. The card "Hostile body look" is there (the
    strip shows the epic "Hostile look" too). Open it. The script and image lines show as text. The `javascript:`,
    `file:` and `data:` links are plain text followed by "(link not opened: <scheme>: links are never opened)". The
    image reads "[image not loaded: tracking pixel, from http://127.0.0.1:9/pixel.png]". The table is a table.
    Nothing runs: in the console `window.__pwned` is `undefined`, and no new error shows.

20. External link. Click "the docs". The window "Open this link in your browser?" names `example.com` and the whole
    address, with Cancel first and focused. Click Cancel, click "the docs" again, press Escape: nothing opens. Click
    "write to us": the window shows `mailto:team@example.com` and names no host. Cancel it.

21. Click "the docs" and then "Open in browser". Your default browser opens on that address (a real request made by
    your browser; `shell.open_external` runs `/usr/bin/open`, and accepts only http, https and mailto). Close the tab.

22. Locate. In terminal 3, move `<s1 repo>`. Quit the app (Command-Q in its window), and start it again in terminal
    2 as in step 4. A restart opens the board last opened or picked, which is `<s1 repo>` (picked in step 19), so it
    shows at once. The page is headed "s1-repo: its folder is gone" and says "Nothing has been deleted", with
    "Locate the folder…" and "Remove from Collide" (off). The daemon knows within seconds that the folder moved, but a
    board already on the screen keeps drawing until the page reloads: that is why the app is restarted.

```sh
mv "$S1" "$S1-moved"
```

23. Click "Locate the folder…". The window "Locate s1-repo" opens. Click "Choose a folder…": the system picker
    opens. Pick `<check root>/s1-repo-moved` (Command-Shift-G, the text `echo "$S1-moved"` printed), then "Use this
    folder". The window closes by itself and the board is back, now named `s1-repo-moved` (the name follows the
    folder). Only when the daemon has a note does the window stay, headed "<name> is back", with "Close". This is
    the "Locate the folder…" check.

24. The carried item of `ts-0186-ch` (the colour of the "Unblocks N" chip, left for this look): no board here has a
    task that unblocks three. To see one, clone this repository and open it as a board, then pick the `bottlenecks`
    view. Judge the colour, or carry it. Skip it if you do not want to.

```sh
git clone "$CO" "$CHECK/clone"
```

   Open `<check root>/clone` as in step 10. It has this repository's plan, so it takes a minute to list.

**Part A. Start with defaults, then Reset…** (`ts-0215-gt`)

25. In terminal 3 break the page's files on purpose, as `app/browser/defaults.spec.ts` does: a settings file that
    does not parse, and a view that does not compile, in the folder of the board that has "Slow spots". The command
    must exit 1 and name `config.yaml` (`...config.yaml:1:7: expected mapping start (key path: disk)`).

```sh
mkdir -p "$HOME/.config/collide"
printf 'disk: [unclosed\n' > "$HOME/.config/collide/config.yaml"
VIEWS="$(dirname "$(ls "$HOME"/.config/collide/boards/*/views/slow-spots.yaml)")"
printf 'name: broken\nwhere: "status =="\n' > "$VIEWS/broken.yaml"
cd "$S2" && "$BIN/collide" config get; echo "exit=$?"
```

26. Blank the page: in the inspector's console type `document.body.innerHTML = ''`. Then press
    Control-Option-Shift-Escape (`app/src-tauri/src/menu.rs`, `START_WITH_DEFAULTS_KEY`). The window reloads. If
    macOS takes the key, record it and use step 28 for the check. The banner reads "Started with defaults. Your
    settings, views and packs are set aside for this window." with "Start normally" and "Reset…". The board shows its
    tasks: the window opens on the board last opened or picked in a window, which is `s1-repo-moved` (the clone, if you
    did step 24), not the board of "Slow spots" (a command in a terminal never moves it). Pick the `<s2 repo>` board
    in the board menu; a pick in this window saves nothing. The view picker (the "All tasks" button) lists
    "All tasks", "ready", "bottlenecks" and the three `collide/plan/` views, and not "Slow spots" or "broken".
    Settings shows every key at Default and lists no problem.

27. Click "Start normally": the window starts normally, on the board last opened or picked in a window
    (`s1-repo-moved` or the clone again, since the pick in step 26 was not kept), and Settings now reports the broken
    `config.yaml` where the table was. Pick the `<s2 repo>` board: a normal window keeps the pick.

28. Blank the page again (step 26's console line). This time use the menu: the first menu, next to the Apple menu
    (it carries the app's name, which for a bare binary is its file name), then "Start with Defaults…". The same
    banner shows. The window opens on `<s2 repo>`, the board you picked in step 27. Wait a second for the page to load
    before the next step.

29. With the window on `<s2 repo>`, click "Reset…" (it stays off until the boards have been read). The window
    "Reset your settings, views and packs?" lists what it moves (settings file, packs and their lock, "The folder of
    this board, s2-repo: your views", the window's stored state) and what it leaves (other boards, the repo's
    `.collide` folder, the keychain). If it names another board, cancel: Reset moves only the folder of the board it
    names, and `<s2 repo>`'s views would stay. Click "Move them aside". The window starts normally with
    "Reset done. Your settings, views and packs were moved to <folder>. Nothing was deleted." and a "Dismiss"
    button. In terminal 3 check what moved (`host/src/set_aside.rs`, `reset`): into
    `set-aside/<YYYY-MM-DD-HHMMSS>/` under `.config/collide`, with paths kept, by rename; nothing deleted. It must
    list `config.yaml` and `boards/<id>/views/` with both views (`broken.yaml` and `slow-spots.yaml`). `trust.yaml`
    stays where it was: the second command prints `boards`, `set-aside` and `trust.yaml`. The third now exits 0 and
    prints every key with `(default)`.

```sh
ls -R "$HOME/.config/collide/set-aside"
ls "$HOME/.config/collide"
cd "$S2" && "$BIN/collide" config get; echo "exit=$?"
```

    To put the files back by hand (no Collide command does it): quit the app, then
    `cp -Rn "$HOME/.config/collide/set-aside/<stamp>/." "$HOME/.config/collide/"`. The check does not need it.

30. Quit the app (Command-Q). Stop the daemon of Part A by the pid in its own log (there is no stop command for it;
    `collide stop` stops a run). The kill runs only if that pid is still a `collide-daemon`. Then check that nothing
    of the check is left running: the last two lines print nothing. The daemon's own command line does not hold
    `<check root>` (the path is in its environment), so the `ps -p` line is the one that sees it; the other sees the
    pack's `node` provider, which can take a second to end, and `grep -v grep` keeps it from finding itself. Do not
    run `collide` again: it would start a daemon.

```sh
pid="$(grep 'starting, pid' "$COLLIDE_DAEMON_ROOT/data/daemon.log" | tail -1 | sed 's/.*pid //')"
ps -p "$pid" -o command= | grep -q collide-daemon && kill -TERM "$pid"
sleep 3
ps -p "$pid" -o pid=,command=
ps -ax -o pid=,command= | grep -F "$CHECK" | grep -v grep
```

**Part B. The keychain and the login row** (`HOME` overridden, no daemon root)

31. In terminal 3, load `env-b.sh`: it sets `HOME` to `<check root>/homeb` and unsets the root. The daemon now runs as
    for a person: a launchd job (label prefix `dev.collide.daemon`), the macOS keychain, the login probe. Its root is
    `<check root>/homeb/Library/Application Support/Collide`. Make the repository with the throwaway secret. A pack
    asks for a secret by `config: { <name>: { type: string, secret: true } }`
    (`app/browser/settings.spec.ts`, `PACK`); the item is named `pack.chk.token` (`host/src/secret_store.rs`,
    `pack_item`) under the service `collide` (`SERVICE`).

```sh
. "$CHECK/env-b.sh"
R="$CHECK/secret-repo"
git init -q -b main "$R"
git -C "$R" config user.name check
git -C "$R" config user.email check@example.invalid
mkdir -p "$R/.collide/packs/chk"
printf 'apiVersion: collide/v0\nname: chk\nversion: 1.0.0\nconfig:\n  token: { type: string, secret: true }\n' \
  > "$R/.collide/packs/chk/pack.yaml"
printf 'packs:\n  - source: repo:.collide/packs/chk\n' > "$R/.collide/board.yaml"
echo check > "$R/README.md"
git -C "$R" add .
git -C "$R" commit -q -m first
```

32. In terminal 2, start the app again, with `env-b.sh`. The first-run page shows: this `HOME` has no boards.

```sh
. "$CHECK/env-b.sh"
"$BIN/collide-app"
```

    Not confirmed from the code: whether the keychain follows an overridden `HOME`. The expected result is that it
    does not, and the item goes to your real login keychain. If macOS shows a dialog that a keychain cannot be found,
    press Cancel; never "Reset To Defaults". If the app cannot reach a keychain, stop and record it. The fallback is
    to run steps 32 to 41 with your real `HOME` and no environment file. That touches your real Collide state:
    `~/Library/Application Support/Collide`, `~/.config/collide` (trust, this board's folder), a launchd job and the
    board's row in the real store. There is no board removal yet ("Remove from Collide" is off), so that row stays.
    Do it only if you accept that, and remove only what `lib-before.txt` and a fresh `ls` show was added.

33. Click "Choose a folder…" and pick `<check root>/secret-repo` (Command-Shift-G; `echo "$R"` prints it). The
    "Checks" show git ok and Claude Code installed. The login row reads that Claude Code is not logged in: this
    `HOME` is empty, so `claude auth status` answers logged out, and the daemon writes `.claude` and `.claude.json`
    into `<check root>/homeb`. That row has a Copy button.

34. Click Copy: it reads "Copied". The command is `claude` (`host/src/first_run.rs`). In terminal 1 run `pbpaste`,
    or paste into a text editor. It must print `claude`. If the row says "could not check", there is no Copy button:
    record it. Afterwards copy any other text, so the clipboard holds nothing of the check.

```sh
pbpaste; echo
```

35. Click "Open the board". The window "Trust the … this repo brings?" names `chk`. Click "Trust and open". Click
    "Settings" (top), then the tab "Secrets". The row `token` reads "Not set", with the button "Set…" (its name for a
    screen reader is "Set token").

36. Click "Set token". The window "Set token" opens. In "Value" type `collide-check-not-a-secret` and click "Save to
    keychain". The window closes, the row reads "Set", and its button reads "Replace…". The value shows nowhere in
    the page.

37. In terminal 1 (real `HOME`), see the item. The command prints the item's attributes: the keychain file, `"svce"`
    `collide`, `"acct"` `pack.chk.token`. It has no `-w` and no `-g`, so it prints no value and asks for no password.
    "Could not be found" means the secret went elsewhere: record it.

```sh
security find-generic-password -s collide -a pack.chk.token
```

38. Check the value is in no file of the check.

```sh
grep -rl 'collide-check-not-a-secret' "$CHECK" || echo none
```

39. Quit the app (Command-Q) and start it again as in step 32. Pick the board, open Settings, "Secrets": the row still
    reads "Set" (the daemon reads the item's attributes only, so no keychain prompt shows).

40. Quit the app. In terminal 3 stop the daemon of Part B by the pid in its log (`daemon.log` sits straight in the
    root of a daemon with no override, not under `data/` as in Part A), and check nothing is left, as in step 30.

```sh
. "$CHECK/env-b.sh"
D="$HOME/Library/Application Support/Collide"
pid="$(grep 'starting, pid' "$D/daemon.log" | tail -1 | sed 's/.*pid //')"
ps -p "$pid" -o command= | grep -q collide-daemon && kill -TERM "$pid"
sleep 3
ps -p "$pid" -o pid=,command=
ps -ax -o pid=,command= | grep -F "$CHECK" | grep -v grep
```

41. In terminal 1, delete the keychain item, and check it is gone. The keychain may ask for your login password: it is
    the system's own dialog.

```sh
security delete-generic-password -s collide -a pack.chk.token
security find-generic-password -s collide -a pack.chk.token; echo "exit=$?"
```

    The second command must say the item could not be found, and exit non-zero.

**Clean up**

42. Terminal 1. The launchd job of Part B must be gone: the first command prints nothing. Then see what the app
    left in `~/Library`. A line starting `<` is new since step 2. The agent's run showed nothing new: the webview's
    folders already existed (see the list of real-`HOME` steps above), and the app wrote into them. Read each path
    before you act. Remove a new path only by its exact path, and never one that is also in `lib-before.txt`. The
    `find` can take 10 to 100 s.

```sh
launchctl list | grep dev.collide.daemon | diff - "$CHECK/launchd-before.txt"
find ~/Library -maxdepth 3 -iname '*collide*' 2>/dev/null | sort | diff - "$CHECK/lib-before.txt"
```

43. Terminal 1. Remove `<check root>`. The daemon keeps its pack cache read-only (`host/src/pack_cache.rs`), so
    `rm -rf` alone fails. The `case` stops it if `$CHECK` is empty or is not the folder step 2 made. Then close
    terminals 2 and 3: they hold the check's `HOME`. Close the browser tab of step 21 if it is open.

```sh
echo "$CHECK"
case "$CHECK" in
  /private/tmp/collide-check.*) chmod -R u+w "$CHECK" && rm -rf "$CHECK" ;;
  *) echo "not the check" ;;
esac
```

44. Check `<collide checkout>` is only built, not changed, and that the keychain item is gone (step 41).

```sh
git -C <collide checkout> status --short
```

**Record**

45. Copy this below the checklist, and fill it in. Each difference of `ts-0185-ge` is closed or carried.

```text
Check on <date>. Debug bundle built in <minutes>. It took <minutes>.
Scenario 1 (limit 150 s): <seconds>. Scenario 2 (limit 60 s): <seconds>. Errors: <what>. Awkward: <what>.
Console (steps 14 to 17): page address <...>; img-src probe <refused | not>; other commands <refused | not>.
Hostile task (19): <pass | what>.  External link (20, 21): <pass | what>.
Open a folder… (10): <pass | what>.  Locate the folder… (23): <pass | what>.  Unblocks chip colour (24): <...>.
Start with defaults (26, 28): <key, menu: pass | what>.  Reset… (29): <pass | what>.
Copy (34): <pass | what>.  Secret (36 to 41): Set shown <yes | no>; item seen <yes | no>; deleted <yes | no>.
ts-0185-ge differences:
1. Epics on the Field: the strip in the Epics region against the mock's notes 58 to 63 (`docs/mockups/board.html`,
   "Mock", "Show design notes"): <closed | carried: what>.
2. Header words: no underscore ("In review"). No board here has such a status, so this rests on the test
   (`app/test/field-model.test.ts`, `wordsOf`): <closed | carried>.
3. The policy under the bundled app (the `tauri dev` probe showed none): steps 14 and 15: <closed | carried>.
4. The page fills the window width (fixed after the look): widen the window on the Field: <closed | carried>.
5. The `claude` status command is `claude auth status` (step 33's login row used it): <closed | carried>.
Stage 6a judged done on <date>, or not done because <what>.
```

### The agent's run, 2026-11-26

The agent ran Part A and the Part A clean-up before yours. It cannot click in the app's window, so steps 6 to 29 ran
on the test bridge in headless Chrome (loopback only), with a small Playwright script and the built `collide` for the
CLI steps. The bridge served the page over the daemon of `<check root>` itself: `collide-bridge --root $CHECK/d` finds
the daemon the app started (the socket answers) and starts none, so the page, the CLI and step 30 used one daemon
and one `HOME`, as the checklist has it. The bridge page has no system picker, only a text box, so the picker
checks of steps 6, 10 and 23 are yours. `pnpm install` was skipped (network).

- Build, on a warm `target/debug`: `pnpm build:packs` 2 s, `pnpm --filter @collide/app build` 2 s,
  `cargo build -p collide-host` 16 s, `tauri build --debug --no-bundle` 45 s. It worked as written and made
  `target/debug/collide-app` (about 30 MB), no `.app`. `git status --short` showed nothing new after it. The bridge
  needed one more build (33 s, `--features collide-app/test-bridge`), which only the agent's run needed.
- Passed: 1 to 5. The app started, drew "Open a project" with "Choose a folder…" at 800 x 600, and printed nothing.
  The daemon started under `<check root>/d` (pid in `d/data/daemon.log`, socket `d/run/d.sock`) with the check's
  `HOME` and root in its environment; nothing under the real `~/Library/Application Support/Collide` or
  `~/.config/collide` changed. A picture of the app's own window, alone, showed that page (not kept).
- Passed on the bridge, with the texts as the checklist names them: 7 to 9 (scenario 1 took 3.4 s of script time),
  11 (17 tasks, three cards), 12 ("13 tasks" with the epic alone, "5 tasks", five cards, 2.5 s of script time), 13,
  15 (Chrome: `Loading the image ... violates ... "img-src 'self' data:"`), 18 to 20, 22, 23 (see below), 25, 27, 29
  (with the board picked) and 30. Step 21 only on the bridge, which records the link and opens none.
- Changed in the checklist: the build times; step 4 (silent start, where the daemon is); step 7's row titles and the
  `claude --version` it runs; step 9's line; step 10 (the menu button carries the board's name, "Open a folder…" is a
  heading); step 11 ("Trust the packs this repo brings?", the card order); step 13 (`name: "Slow spots"` is quoted);
  step 18 (the CLI lists the plan task a few seconds late); step 19 ("without a reload" cannot be seen there);
  step 22 (a restart opens the first board in the list); step 23 (the window closes by itself, and the board is then
  named `s1-repo-moved`); steps 25 to 29 (a restart opens the first board in the list, not the board you were on, so
  you must pick `<s2 repo>` before Reset, or the board's folder with the views is not set aside; the agent's first
  Reset moved only `config.yaml` for this reason); step 30 and step 40 (the last `ps | grep` printed its own line,
  and the daemon's command line has no check path, so it was never what that line saw; the pack's `node` provider
  lingers for a second); step 35 (the button reads "Set…"); step 40 (Part B's log is `$D/daemon.log`, by
  `DaemonPaths::for_home`, not `$D/data/daemon.log`); step 42 and the real-`HOME` list (the webview's files).
- Found, not a bug: the webview ignores an overridden `HOME` and writes into the real `~/Library/WebKit/collide-app`,
  `~/Library/Caches/collide-app` and `~/Library/Preferences/collide-app.plist` (all there since the earlier looks).
  Step 42's diff finds no new path.
- Found, small, not fixed: the defaults banner shows before the board list has loaded. A Reset confirmed in that first
  moment (a script clicked it within a few milliseconds) names "No board is open, so no board folder" and leaves the
  board's views where they were; a person reading the window would see the line. `app/src/App.tsx`, `reset`
  (reads `state.boardId` at the click).
- Found, for you to judge: after "Start normally", Reset or a restart the window opens on the first board in the
  list, not the board you were on (`app/src/loop/state.ts`, the `boardId` fallback to `boards[0]`). Steps 26 to 29
  now say so.
- Not run by the agent: 6, 10 and 23's system pickers; 14, 16 and 17 (the inspector); 21 in the real browser; 24
  (optional); step 26's key and step 28's menu item (no clicks in the window; read from
  `app/src-tauri/src/menu.rs`, `build_menu`); all of Part B. Part B's commands were read against the code: service
  `collide`, item `pack.chk.token` (`pack_item`), the Copy text `claude` (`PROGRAM`), the window "Set token" with
  "Value" and "Save to keychain", the 86-byte socket path under `MAX_SOCKET_PATH_BYTES`, and the job booted out on stop
  (`boot_out_recorded_job`) agree with the steps. The one wrong line was step 40's log path (fixed).
- Clean-up: step 42's two diffs were empty; step 43's `rm` ran as written; step 44 showed only the agent's own
  folder `app/.agent-run-0197/`, which was then deleted.
