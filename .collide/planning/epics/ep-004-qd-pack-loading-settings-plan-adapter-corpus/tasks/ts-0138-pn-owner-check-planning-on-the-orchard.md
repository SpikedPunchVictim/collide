---
id: ts-0138-pn
kind: task
status: active
title: "Owner check: planning on the Orchard repo and Collide's own records"
owners: []
links:
  - { type: blocked-by, to: ts-0137-pg }
  - { type: blocked-by, to: ts-0139-wq }
  - { type: blocked-by, to: ts-0140-gk }
  - { type: blocked-by, to: ts-0141-sn }
  - { type: blocked-by, to: ts-0143-ah }
  - { type: blocked-by, to: ts-0144-eh }
  - { type: blocked-by, to: ts-0145-cq }
  - { type: blocked-by, to: ts-0146-se }
  - { type: blocked-by, to: ts-0147-ve }
  - { type: blocked-by, to: ts-0148-cv }
  - { type: blocked-by, to: ts-0149-kq }
  - { type: blocked-by, to: ts-0151-fk }
  - { type: blocked-by, to: ts-0152-jq }
  - { type: blocked-by, to: ts-0153-kp }
  - { type: blocked-by, to: ts-0155-wb }
  - { type: blocked-by, to: ts-0156-nt }
  - { type: blocked-by, to: ts-0157-hd }
  - { type: blocked-by, to: ts-0158-ow }
  - { type: blocked-by, to: ts-0159-ra }
---

## Goal

The Stage 4c checks that only the owner runs, on the Orchard repo and on Collide's own planning folders: the Decisions
needed view against the plan CLI's human queue, the private root and the layout against a real repository, and the
planning provider against the real `arch/` shapes of ADRs, spikes and questions.

A build agent never runs it. It never touches the Orchard repo or any other repo, never starts a daemon against the
owner's data, and never runs `claude`. It writes the command lines and a short checklist under `### Owner checklist`,
from the code and from the Built sections of `ts-0124-xm` to `ts-0137-pg`, `ts-0139-wq` and `ts-0140-gk`. The owner runs
them and records the result here as owner judgement, with what was awkward.

The owner's run is also the one place the fixtures of `ts-0137-pg` are compared with the real `arch/` files and the real
`plan status --format json`. A difference is recorded here and becomes a fix item.

## Success criteria

- Over the Orchard repo, the Decisions needed view lists the same question ids as the human queue in
  `plan status --format json`. Owner-run, in a clone made for the check.
- An ADR's `evidence` link shows from the spike's side as "evidence for", for an ADR and spike of the real repo.
  Owner-run.
- Collide's spike records in `.collide/planning/spikes/` list in `collide tasks --view spikes --json` with their
  statuses and no item from inside a spike folder other than its record. Owner-run, on a clone of the Collide repo.
- A private-root item never appears in `git status` of the Collide clone, and is listed by `collide tasks`. Owner-run.
- A layout path outside the repository or under `.git/` is refused with the reason. Owner-run.
- A proposal is accepted from the app's connection, and the same from the CLI waits as a confirmation. Owner-run, with
  the app if it can show it (the Queue page of Stage 3c), else by the answer from the CLI's `queue` command.
- `collide task reid` on an item whose id was made twice (two branches merged, in a clone) rewrites the links that name
  it. Owner-run.
- The fixtures of `ts-0137-pg` match the real `arch/` shapes and the real plan CLI, or each difference is written down.
  Owner-run.
- A run started from the owner's own Claude Code session, after `collide guide`, shows on the loop page (revision 24).
  Owner-run, with a small `--budget`.
- A run told to file a follow-up runs `collide task create` with no permission prompt (revision 24; checks
  `ts-0140-gk`'s allow rule), and the item lands in the private root with `fields.run`. Owner-run, with a small
  `--budget`. It has no `raised-by` and is not in the `follow-ups` view: no run records a planning item yet (a quick
  run's task is scratch, a file run records none), as plan 0.10 Follow-ups says. The view is checked with an item
  made by a person.
- The checklist holds the command lines, with `<collide checkout>`, `<orchard copy>` and `<collide clone>` as
  placeholders, what to look at, how to clean up, and a line for the owner's result and one for what was awkward.
- A line records the date Stage 4c is judged done by the owner.

## Tests

None by a build agent: this check is the owner's. The checklist names the automated tests it relies on:

- `ts-0124-xm`, `ts-0125-uh`: the layout setting and the private root;
- `ts-0126-nf`, `ts-0127-js`: the spike and question kinds, the planning links and ids;
- `ts-0128-bb`, `ts-0129-uv`: the actor and the person-only rule, the proposal's transitions;
- `ts-0130-mv` to `ts-0134-ch`: feature ops, the quality parts, coverage, claim blocks;
- `ts-0135-cx`, `ts-0136-rr`: the pack's views and roles, the CLI behind the daemon;
- `ts-0137-pg`: the Orchard plan provider for ADRs, spikes and questions;
- `ts-0139-wq`, `ts-0140-gk`: follow-ups and `collide guide` (revision 24).

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 4c: Success Criteria (the criteria above that name the Orchard repo, Collide's records, the private root and
    the layout); Tests.
  - 0.10: Planning sources. 0.9: Trust. Part L.
- What the checklist covers. The build agent fills in the exact commands once the earlier items are built.
  - **Set up.** Build the CLI and the daemon from `<collide checkout>`. Make `<orchard copy>` as a clone of the owner's
    local copy in a folder made for the check (never the repo the owner works in) and `<collide clone>` of this
    repository. Use a fresh `HOME` and `COLLIDE_DAEMON_ROOT` for everything. Say which steps, if any, use the real
    `HOME`.
  - **Spikes.** `collide tasks --view spikes --json` in `<collide clone>`.
  - **Private root and layout.** Make an item in the private root, see `git status`; set a layout path of each bad
    kind and read the refusal.
  - **Orchard.** `collide init`, commit, `collide trust`; `collide tasks --view decisions-needed --json` against
    `plan status --format json`; the evidence link from a spike.
  - **Accept.** A proposal from the app, then from the CLI.
  - **Own terminal.** `collide guide` in the owner's own Claude Code session, then a run started from it, seen on
    the loop page; then a quick run that files a follow-up with no permission prompt.
  - **Reid.** The two-branch collision in a clone.
  - **Fixtures.** The real ADR, spike and question files and the plan CLI's JSON, next to `fixtures/orchard-arch/`.
  - **Record.** The result, the time it took, and what was awkward.
- **Decision for the owner, with a recommendation: the app for the accept step.** The confirmation is shown by the Queue
  lane of Stage 3c's loop page; Stage 6 builds the full Queue page. Recommend the loop page, since it exists. If it
  does not show the kind well, that is a result to record.
- **Decision for the owner: what to share first.** The sanitized sample of `ts-0137-pg`'s Owner's choice makes this
  check short.
- Area: this item's own text. No code.
- The test rule, for anything automated this check leans on: no test touches the real home, real launchd or a real
  daemon. Each test clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as
  `host/tests/common/mod.rs` `TempRoot` does. Settings files are fixtures, never the owner's own `settings.json`. The
  owner's own run here uses the owner's real repo copy by choice, where the checklist says so.
- Stage 4c item (revision 23 scope, split 2026-11-18). Items of Stage 4c run one at a time, in id order, because they
  share `core`, `host`, `packs/plan` and the generated files.
- Nothing here runs a real `claude`.
- Owner decisions: none recorded yet. The recommendations stand until the owner answers at the split.

### Owner checklist

Written 2026-11-20 from the code at `9002001` and the Built, Decisions, Not done and At review sections of `ts-0124-xm`
to `ts-0137-pg` and `ts-0139-wq` to `ts-0158-ow`. **Every command below was not run by the writer.** A line marked
**(unconfirmed)** is a step or an expected output that the writer could not confirm from code; it says what was looked
for. If a pass line does not come out, that is a result: write it in step 9.

Run on 2026-11-20 and 2026-11-21, by an agent and the owner together. The text below has that run's fixes, and no
**(unconfirmed)** line is left where the run settled it. Step 9 holds the record.

Owner decisions, taken as the item recommends until you answer otherwise:

- The accept step (5) uses the loop page of Stage 3c. If it shows the confirmation badly, that is a result to record.
- No sanitized sample is needed. Step 8 compares by shape: key names and section names, never the text of a file.

The names. `<collide checkout>` is your Collide repository. `<orchard copy>` is your local copy of the Orchard repo.
**No step runs in `<orchard copy>`**: step 1 reads it once, with `git clone --no-hardlinks`, and every step runs in
`<check copy>`, the clone made for this check. `<collide clone>` is a clone of `<collide checkout>` made for this check;
the steps that write Collide's own records write there, never in `<collide checkout>`. `<temp>` is the one folder this
check lives in. Each is a shell variable below (`$CHECK` is `<temp>`, `$CLONE`, `$COPY`).

**Your own state stays apart.** Every step but step 6 runs with a fresh `HOME` and `COLLIDE_DAEMON_ROOT` inside
`<temp>`, so your settings, trust file, daemon, runs and private root are not touched (the CLI and daemon read
`COLLIDE_DAEMON_ROOT`: `host/src/stop_cli.rs`). With `COLLIDE_DAEMON_ROOT` set, no launchd job is loaded. Step 1's build
uses your real `HOME` (cargo and pnpm caches) and runs before the temp `HOME` is set. **Step 6 is the one step that uses
your real `HOME`, your real daemon and your own Claude Code session, by design**; it says what it touches and how to
undo it.

**Spend.** Only step 6 starts `claude`. Each file run there has `--budget 0.10`, the figure the code accepts as any
finite number above 0 (`host/src/run_cli.rs`). The quick run of step 6c has no budget field: the one-line prompt is its
limit. No other step runs `claude`.

**`collide stop` does not stop the daemon.** It stops a run (`host/src/stop_cli.rs`). There is no command to stop the
daemon. This check ends it by its pid from `daemon.log`, as `ts-0121-xp` did; `stop_daemon` below does that.

### 1. Set up

1. Build, with your real `HOME`. `collide` starts the `collide-daemon` that sits beside it, and the app starts the same
   one, so all three binaries must be in one folder. The loader finds the built-in packs and the SDK in the folder
   `pnpm build:packs` stages.

```sh
cd <collide checkout>
pnpm install                 # the pack SDK needs `yaml`
pnpm build:packs
pnpm --filter @collide/app build
cargo build -p collide-host  # target/debug/collide and target/debug/collide-daemon
pnpm --filter @collide/app exec tauri build --debug --no-bundle
ls target/debug/collide target/debug/collide-daemon target/debug/collide-app
```

   The three app lines are those of `ts-0083-uf`. The last build puts `collide-app` in `target/debug` (confirmed in the
   run). It is slow: 41 minutes on the run's machine, where `cargo build` took 7. It also rewrites
   `app/src-tauri/Cargo.toml` (the `tauri` and `tauri-build` lines gain `features = []`); put it back after the build:

```sh
git checkout -- app/src-tauri/Cargo.toml
```

2. Make `<temp>` and the environment file. The short `/tmp` path keeps the daemon's socket path under its limit of 103
   bytes (`MAX_SOCKET_PATH_BYTES` in `host/src/daemon.rs`). Git gets an identity because the fresh `HOME` has none.
   Replace `<collide checkout>` on the first line, then paste the whole block.

```sh
export COLLIDE_SRC=<collide checkout>
export CHECK="$(mktemp -d /tmp/collide-check.XXXXXX)"
mkdir -p "$CHECK/home" "$CHECK/root"
cat > "$CHECK/env.sh" <<EOF
export CHECK="$CHECK"
export COLLIDE_SRC="$COLLIDE_SRC"
export COLLIDE_BIN="$COLLIDE_SRC/target/debug"
export CLONE="$CHECK/collide-clone" COPY="$CHECK/copy"
export HOME="$CHECK/home" COLLIDE_DAEMON_ROOT="$CHECK/root"
export GIT_AUTHOR_NAME=owner-check GIT_AUTHOR_EMAIL=owner-check@example.invalid
export GIT_COMMITTER_NAME=owner-check GIT_COMMITTER_EMAIL=owner-check@example.invalid
EOF
cat >> "$CHECK/env.sh" <<'EOF'
collide() { "$COLLIDE_BIN/collide" "$@"; }
# the pid of this check's daemon, from its log, and an end to it by SIGTERM (there is no `collide` command for this)
daemon_pid() {
  grep 'starting, pid' "$COLLIDE_DAEMON_ROOT/data/daemon.log" 2>/dev/null | tail -1 | sed -E 's/.*pid ([0-9]+).*/\1/'
}
stop_daemon() {
  local pid; pid="$(daemon_pid)"
  [ -n "$pid" ] && ps -p "$pid" -o command= | grep -q collide-daemon || { echo "no daemon of this check"; return 0; }
  kill -TERM "$pid"
  while kill -0 "$pid" 2>/dev/null; do sleep 1; done
  echo "daemon $pid ended"
}
# a task listing as key, status, path: ... --json | rows
rows() { node -e 'const d=JSON.parse(require("fs").readFileSync(0,"utf8"))
for(const t of d.tasks)console.log(t.key,t.state.status,t.path??"-")'; }
EOF
printf 'export PATH="%s"\n' "$PATH" > "$CHECK/home/.zshenv"
cp "$CHECK/home/.zshenv" "$CHECK/home/.zprofile"
echo "$CHECK"
```

   In every other terminal of this check, run `. <temp>/env.sh` (the folder `echo` printed) first. It sets the temp
   `HOME` in that shell, so run nothing else from your own account in it.

   The `.zshenv` line is for the daemon, which captures its base environment from a login shell started with `-l -i -c`
   (`host/src/base_env.rs`). Under the temp `HOME` that shell reads none of your profile, so it may find no `node`.
   `.zshenv` alone is not enough: a login shell then runs the system's `/etc/zprofile`, whose `path_helper` puts the
   system folders first, and the run found an older `node` there. The same line in `.zprofile`, read after it, wins.
   Check it:

```sh
. "$CHECK/env.sh"
"$SHELL" -l -i -c 'command -v node' 2>/dev/null
```

   Pass: `node` is the one you use. If your `SHELL` is not zsh, write the same line in its own startup file.

3. Make `<collide clone>`. It is the repository the Collide-side steps write in (2, 3, 5, 7), and the board of step 6.

```sh
. "$CHECK/env.sh"
git clone -q --no-hardlinks "$COLLIDE_SRC" "$CLONE"
git -C "$CLONE" branch --show-current
ls "$CLONE/.collide/planning"
```

   Pass: a branch name, and `epics` and `spikes`. The clone holds what is committed in `<collide checkout>`: commit or
   stash anything you want in the check first.

4. Make `<check copy>`: a clone of `<orchard copy>`, which this only reads.

```sh
. "$CHECK/env.sh"
git clone -q --no-hardlinks <orchard copy> "$COPY"
git -C "$COPY" branch --show-current
ls "$COPY/arch" "$COPY/plan"
```

   The clone checks out the branch the copy had checked out. That branch must hold `arch/` and the plan CLI. If it does
   not, switch to the one that does, with `git -C "$COPY" switch -c check-line --track origin/<branch>`. Call whichever
   branch is checked out in `<check copy>` the line; every step keeps it checked out.

5. Install the real plan CLI in `<check copy>`. It runs from source, with no build, but a fresh clone needs two installs
   **inside the plan CLI's package folder** (the one `./plan` runs; the root `npm install` is not enough): its own
   `npm install`, and `npm install --no-save tsx`, since its dev mode loads the sources through `tsx` and its package
   does not list it (`ts-0153-kp`, At review). `./plan` is a launcher: open it to see which folder it runs. The installs
   need the network and use the temp `HOME`.

```sh
. "$CHECK/env.sh"
cd "$COPY"
head -20 plan
cd <the plan CLI's package folder (the one `./plan` runs), inside <check copy>>
npm install
npm install --no-save tsx
cd "$COPY" && ./plan --help | head -5
```

   Pass: `./plan --help` prints help. If `npm install` needs your registry login, copy only your `.npmrc` into the temp
   `HOME` for these two commands and delete it after; write that in step 9 (awkward). The real CLI is run only inside
   `<check copy>`. Its writes, here and in step 8, change `<check copy>` and nothing else.

6. Two ids from your own `arch/`, for steps 4 and 8: a proposal id such as `P03` (the number of a folder in
   `arch/proposals/`) and an epic id such as `E-07` (the `epic:` line of any task file). Append them to `env.sh`.

```sh
cat >> "$CHECK/env.sh" <<'EOF'
export PROPOSAL_ID=P00   # edit: a proposal of <check copy>
export EPIC_ID=E-00      # edit: an epic of <check copy>
EOF
```

Record the time this step took in step 9.

### 2. Spikes

Goal: the spike records of Collide's own tree list as spikes, with their statuses, and nothing from inside a spike
folder. Criterion: "Collide's spike records in `.collide/planning/spikes/` list in `collide tasks --view spikes --json`
with their statuses and no item from inside a spike folder other than its record."

The plan pack reads one level of `spikes/`, and only `record.md` of each folder after one `lstat` (`ts-0126-nf`). The
folders hold `work/`, `out/` and `fixture/` too, which must not list. A built-in pack needs no `collide trust`.

```sh
. "$CHECK/env.sh"
cd "$CLONE"
collide tasks --view spikes --json | rows | tee "$CHECK/spikes.txt"
wc -l < "$CHECK/spikes.txt"
ls -d .collide/planning/spikes/*/record.md | wc -l
grep -c '/record.md$' "$CHECK/spikes.txt"
grep -cE '/(work|out|fixture)/' "$CHECK/spikes.txt"
collide tasks --view spikes 2>&1 >/dev/null | grep -c skipped
```

Pass: the first three numbers are equal (ten at the writer's reading; yours is the count of records), every status is
one of `planned`, `running`, `concluded`, `abandoned`, `superseded`, and the last two numbers are `0`. The first command
also starts this check's daemon, and says so on standard error, with a line about `setsid` and, on a cold start, the
line that it waits for the full list. Each task has a `path` in the JSON (confirmed in the run).

### 3. Private root and layout

Goal: a private item is listed and never shows in `git status`; a layout path outside the repository or under `.git/` is
refused with the reason. Criteria: "A private-root item never appears in `git status` of the Collide clone, and is
listed by `collide tasks`" and "A layout path outside the repository or under `.git/` is refused with the reason."

The private root is `<pack data>/planning/`, here `<temp>/root/data/pack-data/<board id>/collide~plan/planning/`
(`pack_data_dir` in `host/src/board_load.rs`; `ts-0125-uh`). It is read for the main checkout only.

**3a. A private item.**

```sh
. "$CHECK/env.sh"
cd "$CLONE"
collide task create "Private probe" epic=ep-004-qd root=private; echo "exit $?"
git status --short
collide tasks | grep 'Private probe'
find "$CHECK/root/data/pack-data" -name '*private-probe*'
collide task create "Private probe two" epic=ep-004-qd root=private --commit; echo "exit $?"
git status --short
git log --oneline -1
```

Pass: the first create prints `ok  created ep-004-qd / ts-<id>` and a `title` line, with no `file` line, and exits 0.
`git status --short` prints nothing. `collide tasks` lists the item and `find` names one file under the temp root. The
second create exits 1 with `--commit refused:` and the private root in the words (`commit_reported` in
`host/src/task_cli.rs`), the item is still written, and `git status --short` still prints nothing and `git log` is the
clone's own last commit.

**3b. The `follow-ups` view.** It lists open private items with a `raised-by` link (`packs/plan/pack.yaml`). A person
may pass `raisedBy` and `run` from the CLI (`ts-0149-kq`, decision 8); a run's agent may not. Use any task id of the
clone for `<task id>`, such as `ts-0138-pn`.

```sh
collide task create "Raised probe" epic=ep-004-qd root=private raisedBy=<task id> run=run-probe
collide tasks --view follow-ups
```

Pass: exactly one line, the "Raised probe" item. The two items of 3a are not there: they have no `raised-by`. Step 6
leans on this: a run's follow-up with no `raised-by` is not in this view.

**3c. A layout that is refused.** `collide/plan.layout` is a repo-only setting in the committed `.collide/config.yaml`,
and the daemon reads the committed line, not the working copy. A provider reads its settings at its start, so each case
commits, ends the daemon (`stop_daemon`), and lists again. Work on a branch.

```sh
cd "$CLONE"
git switch -q -c layout-check
mkdir -p .collide
printf 'config:\n  collide/plan:\n    layout:\n      root: ../outside\n' > .collide/config.yaml
git add .collide/config.yaml && git commit -q -m "check: a root outside the repository"
stop_daemon
collide tasks; echo "exit $?"
```

Pass: standard error has a line ``collide: source plan may be out of date: ... refused `initialize` ...: <reason>``
naming `layout.root` and the rule (a `..` part), and the command exits 0. Standard output still lists the tasks the
daemon read before, under the old layout: a source that fails to start keeps its cached tasks, marked stale
(`corpus_service.rs`). The exact words of the reason are the provider's (`src/layout.ts`). Then two more cases, each the
same four lines with a new `root`:

```sh
printf 'config:\n  collide/plan:\n    layout:\n      root: .git/planning\n' > .collide/config.yaml
git commit -q -a -m "check: a root under .git" && stop_daemon && collide tasks; echo "exit $?"
ln -s "$CHECK" outside-link
printf 'config:\n  collide/plan:\n    layout:\n      root: outside-link\n' > .collide/config.yaml
git add outside-link .collide/config.yaml && git commit -q -m "check: a link out" && stop_daemon
collide tasks; echo "exit $?"
```

Pass: each is refused with the reason (a root under `.git/`, git's own folder; a path that goes through a symbolic link
which leaves the repository), the same way. Then put the clone back:

```sh
git switch -q main && git branch -q -D layout-check && stop_daemon
collide tasks | head -3
```

Pass: Collide's tasks list again. That a layout change in a commit reaches the running provider only after a restart is
read from `sources.rs` ("a settings change reaches a provider at its next start"); if a case lists with no error without
the `stop_daemon`, write that in step 9.

### 4. Orchard

Goal: the Decisions needed view lists the same questions as the plan CLI's human queue, an ADR's evidence shows from the
spike's side, and the CLI forms work on a real clone. Criteria: "Over the Orchard repo, the Decisions needed view lists
the same question ids as the human queue in `plan status --format json`" and "An ADR's `evidence` link shows from the
spike's side as 'evidence for', for an ADR and spike of the real repo."

**4a. Make the board** in `<check copy>`. The reference pack is the provider in `fixtures/packs/orchard/providers/`. Its
own `pack.yaml` names workspace hooks that are not in the fixture folder, so the manifest is the short header below and
the file's text from its `providers:` line to its end, as `host/tests/daemon_reference_kinds.rs` makes it (`manifest`):
the provider, the two views and the `roles` setting. The pack is called `reference` here. `collide init` refuses when
`.collide/` exists; if it does, skip it and put the `packs:` entry in the board file that is there.

```sh
. "$CHECK/env.sh"
cd "$COPY"
collide init
mkdir -p .collide/packs/reference
cp -R "$COLLIDE_SRC/fixtures/packs/orchard/providers" .collide/packs/reference/providers
{ printf 'apiVersion: collide/v0\nname: reference\nversion: 0.1.0\nrequires:\n  sdk: ">=0.2 <0.3"\n  tools:\n'
  printf '    - { name: node }\n'
  sed -n '/^providers:/,$p' "$COLLIDE_SRC/fixtures/packs/orchard/pack.yaml"; } > .collide/packs/reference/pack.yaml
printf 'packs:\n  - source: repo:.collide/packs/reference\n' > .collide/board.yaml
git add .collide && git commit -q -m "check: reference pack"
collide trust
```

Pass for `trust`: `Line <the line>`, a `Trust hash`, `Changes:` listing the added files, a block `Pack reference 0.1.0`
with `Source: repo:.collide/packs/reference` and the provider `plan`, then `Trust this? [y/N]`. Read the review; answer
`y`. Then "Trusted line ... at <hash>." `collide trust` asks only at a terminal: with its input piped it refuses ("no
terminal to ask at"), exit 5; then read the review and run `collide trust --yes`. `collide init` writes the board file
and starts no daemon; the first `collide tasks` starts it. The manifest above loads (confirmed in the run).

**4b. The Decisions needed view against the human queue.** The provider runs
`./plan status --root <root>/arch --format json` itself and puts each question the queue names in the collection
`plan:queue`, which the view lists. Compare the two, from the real CLI's own output.

```sh
cd "$COPY"
time collide tasks > "$CHECK/all.txt"   # the time shows on the terminal; standard output goes to the file
wc -l < "$CHECK/all.txt"
./plan status --root "$COPY/arch" --format json > "$CHECK/status.json"
node -e 'const s=JSON.parse(require("fs").readFileSync(process.argv[1],"utf8"))
for(const q of s.data.queue)console.log(q.question)' "$CHECK/status.json" > "$CHECK/queue.ids"
collide tasks --view decisions-needed | awk '{print $1}' | sed 's/^plan://' > "$CHECK/view.ids"
sed -E -i '' 's/^(Q-[0-9]+-[0-9]+).*/\1/' "$CHECK/queue.ids"   # the queue names a question with its slug
wc -l "$CHECK/queue.ids" "$CHECK/view.ids"
sort "$CHECK/queue.ids" | diff - <(sort "$CHECK/view.ids") && echo "same ids"
diff "$CHECK/queue.ids" "$CHECK/view.ids" && echo "same order"
```

Pass: the listing prints one line per item as `key  status  kind  title`, the time is the figure for a real tree (write
it), `same ids`, and the two counts agree. `same order` should print too: the plan CLI sorts by id and the view lists by
key. If the counts differ, write both lists' ids, never their titles. A line `collide: skipped <path>: <reason>` on
standard error is a file the provider could not read: write each reason in step 9. `data.queue[].question` is the field
the provider reads (`queue.ts`). The real CLI gives the full id there, slug and all, and the view's keys are the short
id: that is why `queue.ids` is cut to the short id. If your ids have another shape, change the `sed` to fit.

**4c. The evidence link from the spike's side.** An ADR's `evidence` points at a spike; the spike shows the incoming
link by its inverse, "evidence for" (`evidence` in `fixtures/packs/orchard/providers/plan/declaration.ts`).

```sh
for k in $(collide tasks --kind spike | awk '{print $1}'); do
  collide task "$k" --json | node -e 'const d=JSON.parse(require("fs").readFileSync(0,"utf8"))
  const e=d.links.filter(l=>l.dir==="in"&&l.type==="evidence"&&!l.missing).map(l=>l.key)
  if(e.length)console.log(d.task.key,"<-",e.join(" "))'
done | head -5
```

Pass: at least one spike line, naming one or more ADR keys. Then the text form of one of them, with a spike key from the
output: `collide task plan:<spike id> | grep -i evidence`. Pass: the line reads "evidence for" and the ADR, with the
ADR's status. A link's JSON keys are `category`, `dir`, `key`, `kind`, `missing`, `stale`, `status` and `type`. The loop
runs the CLI once per spike: about 100 s for a hundred spikes. If the copy has no ADR with evidence, write that in step
9.

**4d. The CLI forms on a real clone** (`ts-0149-kq`, Not done: they were never run on a real tree). Writes go through
the real `./plan`, in `<check copy>`. Give the first `collide task create` a made-up title.

```sh
cd "$COPY"
collide tasks --ready | head -5
collide tasks --kind adr | head -3
collide task create "Probe task" epic="$EPIC_ID" proposal="$PROPOSAL_ID"; echo "exit $?"
git status --short
```

Pass: `--ready` prints tasks, or says why none is ready; the create prints `ok  created <id with its slug>`, a `title`
line and a `file` line for each file the real plan CLI wrote (the task, and the epic's file whose table it regenerates);
`git status` shows them. Take the new id for `<probe key>`, then:

```sh
collide task "<probe key>" | head -8
collide task "<probe key>" setStatus ready; echo "exit $?"
collide task "<probe key>" setStatus done; echo "exit $?"
```

Pass: the text shows the title and its status; `setStatus ready` exits 0; the move to `done` is refused with the plan
CLI's own words ("not a legal transition", and the legal moves from `ready`) and exit 5. A bare id (`collide task <id>`
with no `plan:`) resolves to the item.

**4e. Two checks of the build, apart from the Success criteria.**

First, `--kind task` (`ts-0156-nt`). On this board, every item listed with `--kind task` has `task` in its kind column.

```sh
collide tasks --kind task | awk '{print $3}' | sort | uniq -c
collide tasks | awk '$3=="task"' | wc -l
(cd "$CLONE" && collide tasks --kind task | awk '{print $3}' | sort | uniq -c)
```

Pass: one row of the first command, `task`, with the same count as the second; and Collide's tasks in the clone are
`task` too. The third line goes to the clone's board in the same daemon. One board cannot hold both plans: the Orchard
pack's provider is named `plan`, a board has one source of that name, and a board's own pack replaces the built-in one
(`sources::plan` in `host/src/sources.rs`). So the "mixed board" of `ts-0156-nt`'s Success criteria does not exist; the
two boards above stand for it, and `kind_of` reads each item's own kind on either.

Second, the wait after a restart (`ts-0155-wb`). After the daemon ends, the first `collide tasks` serves nothing at once
and waits for the relist; when that takes over 0.5 s it prints one line on standard error.

```sh
secs() { perl -MTime::HiRes=time -e '$t=time; system @ARGV; printf STDERR "took %.2f s\n", time-$t; exit($?>>8)' "$@"; }
secs "$COLLIDE_BIN/collide" tasks > "$CHECK/settled.txt"
stop_daemon
secs "$COLLIDE_BIN/collide" tasks > "$CHECK/first.txt" 2> "$CHECK/first.err"; echo "exit $?"
cat "$CHECK/first.err"
cmp "$CHECK/settled.txt" "$CHECK/first.txt" && echo "stdout equal"
```

`secs` stands in for zsh's `time`, whose report does not follow a redirection of the command it times. The second
`took` line is in `first.err`.

Pass: `stdout equal`, exit 0. Standard error has `collide: the daemon was not running; started it`. When the second
`time` is over 0.5 s it also has `collide: the daemon is still listing the board's sources; waiting for the full list`,
once. When it is under 0.5 s the second line is not there, and that is a pass; write both times. The time over the real
tree is what `ts-0116-wh` left unmeasured.

### 5. Accept

Goal: a proposal is accepted from the app, and the same from the CLI is a confirmation that waits. Criterion: "A
proposal is accepted from the app's connection, and the same from the CLI waits as a confirmation." This is the loop
page of Stage 3c, by the decision above; the full Queue page is Stage 6.

The ops `accept`, `defer`, `reject`, `resolve` and `reopen` are a person's (`ts-0128-bb`, `ts-0129-uv`): the CLI writes
nothing, prints the Queue item's id and exits 12; the app's Accept applies it.

1. Make a proposal that can be accepted: made up, on a branch of `<collide clone>`. A move to `proposed` needs a clean
   Features part, so the second line adds one feature (the run did it this way and did not try an empty part).

```sh
. "$CHECK/env.sh"
cd "$CLONE"
git switch -q -c accept-check
collide task create "Accept probe" kind=proposal epic=ep-004-qd --commit
```

   It prints `ok  created ep-004-qd / pr-<id>`. Put that id after `plan:` in the next lines.

```sh
export PROPOSAL=plan:pr-<id>
collide task "$PROPOSAL" addFeature section=Probe title="Probe feature" summary="A made-up feature." --commit
collide task "$PROPOSAL" setStatus proposed --commit
```

   Pass: each exits 0. If `setStatus proposed` is refused for the Features part, the words list each problem: write
   them, fix the part by hand, commit, and go on.

2. The CLI, first. Nothing is written.

```sh
collide task "$PROPOSAL" accept; echo "exit $?"
git status --short
```

   Pass: standard error `collide: a confirmation waits in the app: <reason>; queue item confirm-<ms>-<n>; nothing was
   written`, exit 12, and `git status --short` prints nothing.

3. The app. Start it from a terminal that sourced `env.sh`, in the foreground, so it uses the temp `HOME` and root and
   talks to the daemon of this check. The app code reads both (`daemon_access` in `app/src-tauri/src/lib.rs`); with
   `COLLIDE_DAEMON_ROOT` set the "published" version chip is missing, as `ts-0083-uf` says.

```sh
. "$CHECK/env.sh"
"$COLLIDE_BIN/collide-app"
```

   Under "Repository", type the full path of `<collide clone>` (absolute, no `~`) into "Folder path" and press "Open
   folder". Look at "Needs you":

   - [ ] A row for the confirmation, with an Accept and a Dismiss button, and a title that names the proposal. The words
         of the title are `waiting_title`'s in the daemon: **(unconfirmed)**. The CLI's reason in 5.2 reads `Confirm:
         accept proposal ...`; the run did not write down the row's words. Write what it says.
   - [ ] Press Accept. The row goes.

```sh
cd "$CLONE"
git diff
collide task "$PROPOSAL" | head -4
```

   Pass: the diff changes the proposal's `status:` to `accepted` and adds one `by: human:<today>` line, nothing else.
   The date is the daemon's clock. If the row is missing, or Accept does nothing, that is the result to record, with
   what the page showed.

4. Put the clone back: `git switch -q main && git branch -q -D accept-check`, with the files left uncommitted removed by
   `git checkout -- . && git clean -fd .collide` if the diff above is still there. Quit the app, and **close the
   terminal that started it**: that terminal holds the temp `HOME`, and step 6 must not start anything from it.

### 6. Own terminal

Goal: after `collide guide`, a run started from your own Claude Code session shows on the loop page, and a run told to
file a follow-up does so with no permission prompt. Criteria: the two of Success criteria that name revision 24 (the
loop page; the follow-up with `collide task create`).

The follow-up has `run:` and no `raised-by`, so it is **not in the `follow-ups` view**: a quick run's task is a scratch
task and a file run records no task (plan 0.10 Follow-ups; `ts-0139-wq`, decision 6; `shape_create` in
`host/src/follow_ups.rs`). It is a private item, listed by `collide tasks`. Step 3b is the check that the view works.
What this step decides is whether Claude runs the command with no permission prompt (`ts-0140-gk`, Not done: whether
real Claude honours `Bash(collide:*)` beside `--permission-prompt-tool stdio`). A pass is: no permission row in "Needs
you" for the `collide` command, and the item exists.

**What this step touches, by design.** Your real `HOME`: the real daemon (started by a launchd job), the real store, the
runs and worktrees in `~/Library/Application Support/Collide/`, the private root of `<collide clone>` under the real
`pack-data` folder there, and the real `claude` login. The repository is still `<collide clone>`. Clean-up is below.

6.0. Before anything starts.

1. A new terminal window, never one that sourced `env.sh`; call it your own terminal. Check it first: `echo $HOME`
   prints your own home folder, and `echo $COLLIDE_DAEMON_ROOT` prints nothing. An app or `collide` started with the
   temp `HOME` talks to a daemon of its own and never sees your runs (it happened in the run: `ts-0164-pa`). Stop an
   older daemon of yours: one from an
   older build lacks the code of `ts-0139-wq` and `ts-0140-gk`. There is no `collide` command for it;
   `collide stop <run>` stops a run. A daemon that is not there is fine.

```sh
grep 'starting, pid' "$HOME/Library/Application Support/Collide/daemon.log" | tail -1
ps -p <daemon pid> -o pid=,command=
kill -TERM <daemon pid>
```

2. `~/.config/collide/models.yaml` must map the alias `fast`, which the builder and the quick run ask for
   (`BUILDER_MODEL_ALIAS` in `host/src/builtin_roles.rs`). If it does, leave it. If not, `ts-0083-uf` step 3 has the
   lines. A run with no mapping is refused before it starts.

3. A task file in `<collide clone>`, committed, since the run's worktree is made from the committed tree. Do this in a
   terminal that sourced `env.sh`; it only uses git. The epic id is Collide's own.

```sh
. "$CHECK/env.sh"
cd "$CLONE"
printf 'Run this one command and nothing else, then stop. Do not use sleep.\n\n' > follow-up-probe.md
printf 'collide task create "Follow-up probe" epic=ep-004-qd\n' >> follow-up-probe.md
git add follow-up-probe.md && git commit -q -m "check: a follow-up task file"
```

**6a. The guide starts no daemon.** In a terminal that sourced `env.sh`, with a root of its own:

```sh
. "$CHECK/env.sh"
COLLIDE_DAEMON_ROOT="$CHECK/guide-root" collide guide | head -30
ls "$CHECK/guide-root"
```

Pass: the guide prints the commands, their forms and the exit codes; `ls` says there is no such folder (`collide guide`
starts no daemon and writes nothing: `host/tests/guide_command.rs`). Read it as an agent would: it should say that
`collide run` is refused inside a run, and that a follow-up is made with `collide task create`.

**6b. A file run, started from your own Claude Code session.** In your own terminal:

```sh
export PATH="<collide checkout>/target/debug:$PATH"
cd <temp>/collide-clone
claude
```

   Ask the session, in these words:

   > Run `collide guide` and read it. Then start one run with `collide run follow-up-probe.md --budget 0.10` in the
   > background, and tell me the run id it prints. Do not use sleep.

   `collide run` stays until the phase ends, and a long wait may time out in the session's shell: that is why it is
   started in the background. `--budget 0.10` is a backstop; the run is one command on the small model. Start the loop
   page from another own terminal, to watch it:

```sh
"<collide checkout>/target/debug/collide-app"
```

   What to look at on the page:

   - [ ] The run card shows the run id the session printed, with its state moving from running to ended. In the run it
     did not: the page followed an older run's waiting Review item (`ts-0159-ra`). Once that is fixed, this look is the
     one re-check left of this step.
   - [ ] "Needs you" shows no `permission` row naming the `collide` command at any time. A file run ends with no row at
         all: it makes no Review item (confirmed in the run).
   - [ ] The session did run `collide guide` first, in its own transcript.

   Then, in your own terminal, from `<temp>/collide-clone`:

```sh
collide tasks | grep 'Follow-up probe'
collide tasks --view follow-ups
```

   Pass: the first command lists the item (kind `-` or `task`); it is a private item, so `git status --short` in the
   clone prints nothing for it. The second command does **not** list it, as said above; write what you see. Then look at
   its fields:

```sh
collide task "plan:<the key from the first command>" --json | node -e '
const d=JSON.parse(require("fs").readFileSync(0,"utf8"))
console.log(JSON.stringify(d.task.fields,null,1)); console.log(d.links)'
```

   Pass: `fields` has `root` `private` and a `run` that is the run id of the page; `links` has no `raised-by`. Whether a
   run with a stale daemon, or a refused `collide` call, shows as `waiting` (exit 9 of `collide run`) is what a missing
   permission looks like: write the run's end state and exit code in step 9.

**6c. A quick run, from the loop page.** The criterion names a quick run, so do it once. The page has no budget field
and a quick run has no cap (`ts-0083-uf`): the one-line prompt is the limit, on the small model.

1. In the app window, under "Repository", open `<temp>/collide-clone` (the full path). Type this in "What should the
   agent do?" and press "Start":

   > Run this one command and then stop: collide task create "Follow-up probe two" epic=ep-004-qd

2. Look at the page while it runs, as in 6b. The run card shows the run; "Needs you" shows no permission row for the
   `collide` command. When the run ends, a Review item appears with Accept, Re-prompt and Discard. Press **Discard**
   (and confirm): it removes the run's worktree and branch (`discard` in `host/src/file_run/discard.rs`). In the run
   nothing was removed (`ts-0160-th`): check with `git worktree list` in `<collide clone>`, write what the page said,
   and remove what is left as in "Clean up".

3. Look for the item as in 6b: `collide tasks | grep 'Follow-up probe two'` from `<temp>/collide-clone`, in your own
   terminal. Pass: listed, a private item, not in `collide tasks --view follow-ups`.

If either run shows a permission row for `collide`, that is the result: press Deny, and write the row's words in step 9
(the allow rule of `ts-0140-gk` did not take). The undo is in "Clean up".

### 7. Reid

Goal: `collide task reid` on an item whose id was made twice rewrites the links that name it. Criterion: "`collide task
reid` on an item whose id was made twice (two branches merged, in a clone) rewrites the links that name it."

`collide task create` draws the next free id across every local branch, every fetched remote branch and the working tree
(`HELP` in `host/src/task_cli.rs`), so the CLI will not make the same id on two branches. The state a merge would leave
is made by hand: a copy of the file under another name, with the same `id:`. Everything runs on a branch of
`<collide clone>`. The probe is a task of the epic `ep-004-qd`.

```sh
. "$CHECK/env.sh"
cd "$CLONE"
git switch -q -c reid-check
collide task create "Reid probe" epic=ep-004-qd seed=7 --commit
collide task create "Reid linker" epic=ep-004-qd seed=8 --commit
```

Each prints `ok  created ep-004-qd / ts-<id>`. `ID` is the probe's id; `LINKER` is the linker's, after `plan:`.

```sh
export ID=ts-<probe id> LINKER=plan:ts-<linker id>
collide task "$LINKER" link "$ID" type=blocked-by --commit
FILE="$(find .collide/planning -name "$ID-*.md")"
echo "$FILE"
COPY_FILE="$(dirname "$FILE")/$ID-zcopy.md"   # sorts after "$FILE" (`reid-probe`), so the copy is the one set aside
cp "$FILE" "$COPY_FILE"
sed -i '' 's/^title: .*/title: "Reid probe copy"/' "$COPY_FILE"
git add -A .collide && git commit -q -m "check: the same id twice"
stop_daemon
collide tasks 2>&1 >/dev/null | grep skipped
```

Pass: `echo "$FILE"` prints one path; the last command prints one `collide: skipped <path>: <reason>` line for the copy,
and the reason says the id is taken twice. The first file in scan order is the item; the second is the one set aside.
`stop_daemon` is there because a running daemon's watcher, seeing the copy, lists two rows with the one key, and the
reid below is then refused as "matches 2 items", exit 3 (`ts-0161-ta`); a fresh listing has one. Then:

```sh
collide task reid "$ID" --commit; echo "exit $?"
git show --stat --format=%s HEAD
collide tasks 2>&1 >/dev/null | grep -c skipped
grep -rl "$ID" .collide/planning
```

Pass: exit 0; the output names the old and the new id and the files touched; `links   rewritten in <key>` for the
linker; the commit touches the renamed copy (its `id:` line and name) and the linker's file; no `skipped` line is left
(`0`); the last command lists only the first probe's own file. Open the linker's file: its `blocked-by` target is the
**new** id. That is the documented limit: the id cannot tell the two files apart, so a link meant for the first file is
rewritten too (`HELP`, `reidLocked` in `packs/plan/src/reid.ts`). Then put the clone back:

```sh
git switch -q main && git branch -q -D reid-check
```

### 8. Fixtures

Goal: the fixtures match the real thing, or each difference is written down. Criterion: "The fixtures of `ts-0137-pg`
match the real `arch/` shapes and the real plan CLI, or each difference is written down." Each difference goes into step
9 and becomes a fix item. The record is part of a public repository: write key names and generic heading words only,
never the text of a real file, a title, a path or an id.

**8a. The plan CLI's JSON, recorded from the real CLI** (`ts-0153-kp`). The recorder writes the shape of each envelope
the Orchard pack reads, nine cases, one file each, and runs the real CLI's writes on one made-up task. Make the task
with the real `new task` and set it to `ready` with the real `set`, in `<check copy>`. Start from a clean `arch`, since
4d made a probe task:

```sh
. "$CHECK/env.sh"
cd "$COPY"
git checkout -q -- arch && git clean -fdq arch
./plan new task "$PROPOSAL_ID" "Probe task" --epic "$EPIC_ID" --root "$COPY/arch" --format json > "$CHECK/new.json"
node -e 'console.log(JSON.parse(require("fs").readFileSync(process.argv[1],"utf8")).data.id)' "$CHECK/new.json"
```

Pass: one task id, with its slug after it (`T-<nn>-<nnn>-<slug>`). Copy it **without the slug**: the recorder takes the
short id and refuses one that does not fit.

```sh
export TASK_ID=T-00-000    # edit: the id printed, without its slug
./plan set "$TASK_ID" status=ready --root "$COPY/arch" --format json > "$CHECK/set.json"; echo "exit $?"
node "$COLLIDE_SRC/fixtures/orchard-arch/contract/record.ts" --cli "$COPY/plan" --root "$COPY" --out "$CHECK/shapes" \
  --source real --task "$TASK_ID" --proposal "$PROPOSAL_ID" --epic "$EPIC_ID"
```

Pass: exit 0, then one path printed for each of nine files in `<temp>/shapes`. The recorder refuses, in its own words, a
task that is not `ready`, an id that does not fit, or a root with no `arch`. It writes only under `--out`, leaves no
home folder, and prints nothing of the CLI's own output. It runs the cases' writes on `<check copy>`.

Compare with the committed shapes, leaving out `recorded`, the day:

```sh
strip() { node -e '
const d=JSON.parse(require("fs").readFileSync(0,"utf8")); delete d.recorded
console.log(JSON.stringify(d,null,2))'; }
for f in "$COLLIDE_SRC"/fixtures/orchard-arch/contract/shapes/*.json; do
  n="$(basename "$f")"
  strip < "$f" > "$CHECK/want-$n"
  strip < "$CHECK/shapes/$n" > "$CHECK/got-$n"
  diff "$CHECK/want-$n" "$CHECK/got-$n" > "$CHECK/diff-$n" && echo "same  $n" || echo "DIFF  $n"
done
```

Pass: `same` for every file. For each `DIFF`, read `<temp>/diff-<name>`: it names the key or the value kind. Write the
file name and the key in step 9; a key path is a schema word. Whether the cases are all legal on your tree (for example
`P99` and `T-99-999` are meant to be absent) is the recorder's own rule. The gate's exit-1 refusal is not one of the
cases: no legal call provokes it (`ts-0153-kp`, Decision 7). If one of your calls does refuse with exit 1, write its
`errors[0]` key names in step 9.

**8b. The real ADR, spike and question files, by shape.** The fixture tree is `fixtures/orchard-arch/kinds/arch/`: ADRs
directly under `arch/`, spikes as `spikes/<id>/PLAN.md`, questions as `proposals/<proposal>/questions/*.md`. The check
lists the frontmatter key names and the `##` heading lines of each kind, real against fixture, and nothing else.

```sh
F="$COLLIDE_SRC/fixtures/orchard-arch/kinds/arch"
files_of() {   # kind, arch folder
  case "$1" in
    adr) find "$2" -maxdepth 1 -name '[0-9]*.md' -print0 ;;
    spike) find "$2/spikes" -name PLAN.md -print0 ;;
    question) find "$2/proposals" -path '*/questions/*.md' -print0 ;;
  esac; }
keys_of() { xargs -0 awk '
  FNR==1{n=0}
  /^---[ ]*$/{n++; next}
  n==1 && /^[A-Za-z_][A-Za-z0-9_-]*:/{sub(/:.*/,""); print}' | sort -u; }
heads_of() { xargs -0 awk '/^##+ /{sub(/^#+ */,"## "); print}' | sort -u; }
for kind in adr spike question; do
  files_of $kind "$COPY/arch" | keys_of  > "$CHECK/$kind.keys.real"
  files_of $kind "$F"         | keys_of  > "$CHECK/$kind.keys.fix"
  files_of $kind "$COPY/arch" | heads_of > "$CHECK/$kind.heads.real"
  files_of $kind "$F"         | heads_of > "$CHECK/$kind.heads.fix"
  for what in keys heads; do
    diff "$CHECK/$kind.$what.real" "$CHECK/$kind.$what.fix" > "$CHECK/$kind.$what.diff" \
      && echo "same  $kind $what" || echo "DIFF  $kind $what"
  done
done
wc -l "$CHECK"/*.real
```

Pass: `same` for the six pairs, or each difference named in step 9. A key the real files have and the fixture lacks is a
fix item for the fixture (and for the provider if it should read it); a key the fixture has and no real file has is made
up, and goes in as well. A line count of `0` in a `.real` file means the `find` pattern found no file of that kind: the
layout of your tree differs from the fixture's, and that is itself a result; find the files by hand and say where they
are, by folder shape only.

### 9. Record

Write the result here, as owner judgement, and say what was awkward. Commit nothing from this folder: the owner's words
go in this item.

Run on 2026-11-20 and 2026-11-21. An agent ran every step a build agent may run, in a temp `HOME` and root. The owner
ran 5.3, 6b, 6c and the clean-up of their own state; the agent read the real store, transcripts and log, read-only, to
fill in what the page did not show. Lines marked agent-run are for the owner to judge; owner-run lines are the owner's.

- Time (agent-run): set up about 50 minutes, nearly all of it the builds (`cargo build` 445 s, `tauri build` 2,478 s);
  the clones 1 s and 4 s; the plan CLI's installs 12 s. Whole-step times of the other steps were not kept; the figures
  they measured are below.
- 1 Set up (agent-run): pass, after one fix. The temp `.zshenv` lost to `/etc/zprofile`'s `path_helper`, and the
  daemon's login shell found an older `node`; the same line in `.zprofile` fixed it. The `npm install` pair worked with
  the temp `HOME`, with no registry login. `collide-app` is in `target/debug`. `tauri build` rewrote
  `app/src-tauri/Cargo.toml`, put back by hand.
- 2 Spikes (agent-run): pass. 10 records, 10 listed, 10 `record.md` paths, none from `work/`, `out/` or `fixture/`, no
  `skipped` line, all `concluded`. 6 s for the first listing, with the daemon's start.
- 3 Private root and layout (agent-run):
  - 3a: pass. `git status` empty, the item listed, one file under the temp root's pack data. `--commit` exits 1:
    "--commit refused: ... private root, which is in no repository; the write was made and nothing was committed".
  - 3b: pass. The view lists only "Raised probe".
  - 3c: the criterion passes; the output differs from the text. Each layout is refused with its reason: "a `..` part",
    "under .git/, git's own folder", "goes through the symbolic link ... which leaves the repository". The command exits
    0 and lists the 176 tasks of the old layout, with "source plan may be out of date: ... refused `initialize`". A
    source that fails to start keeps its cached tasks, marked stale (`ts-0141-sn`, `ts-0152-jq`).
- 4 Orchard (agent-run):
  - 4a: pass. The manifest loads; the trust review reads as the text says. A piped `y` is refused, exit 5; `--yes`
    used. `collide init` started no daemon.
  - 4b: pass. First listing 10 s: 1,973 items (31 ADRs, 175 questions, 108 spikes, 1,659 tasks), the file counts of
    8b, no `skipped` line. Queue 54, view 54, the same ids in the same order once the queue's ids are cut to the short
    id (the CLI gives them with their slug).
  - 4c: pass. Two spikes show "evidence for" an ADR, with its status; only two of the 31 ADRs name evidence. The JSON
    key is `type`, not `link_type`. The loop took about 98 s.
  - 4d: pass. `--ready` lists 99, open questions first. The create wrote the task and the epic's file (two `file`
    lines). `setStatus ready` exit 0; `done` refused with the plan CLI's words, exit 5. A bare id resolves.
  - 4e: pass. `--kind task` lists 1,660, the count of `task` lines (with the probe); the clone's are `task`. Restart:
    0.44 s settled, 6.58 s after the stop; the started, `setsid` and wait lines once each; output equal; exit 0.
- 5 Accept:
  - 5.1 and 5.2 (agent-run): pass. The proposal, one feature, `proposed`. The CLI's accept exits 12: "a confirmation
    waits in the app: Confirm: accept proposal ...; queue item confirm-...; nothing was written"; `git status` empty.
    The confirmation is kept in the store and survives a restart.
  - 5.3 (owner-run): "accept worked". The row's words and the diff were not written down.
- 6 Own terminal:
  - 6a (agent-run): pass. 34 lines, no daemon folder. It names `collide task create` for a follow-up, says a scratch
    task is not one, and says `collide run` is refused inside a run.
  - 6b (owner-run, three file runs: `run-1795250629470-0`, `run-1795251885848-0`, `run-1795252860346-1`): the follow-up
    criterion passes; the loop page criterion fails. Each run used the small model in Claude's default permission mode,
    ran `collide task create` with no permission denial and no permission line in the daemon's log, cost $0.003 to
    $0.006, and ended with exit 0. Its item is private, with `fields` `root: private` and `run` its run id, links
    `part-of` only, not in `follow-ups`. With the app on the real daemon before the third run started, the owner saw
    "nothing" on the run card (`ts-0159-ra`). Only the third run is a fair look: the second's app had the temp `HOME`.
    Re-check after `ts-0159-ra` (owner-run, 2026-11-21): pass. With the app on the real daemon, the owner ran
    `collide run follow-up-probe.md --budget 0.10` in their own terminal: run `run-1795321946830-2`, $0.0055, exit 0.
    The card switched to it by itself and showed it ended. Its item is private, with `run:` and no `raised-by`; the
    daemon's log has no permission line. An earlier try that day from Claude Code (`run-1795315707074-0`) ran and ended
    while the owner was not watching the card, so it does not count. The card's Details said Task "unknown" and left
    Workflow and the branch blank (`ts-0178-fw`).
  - 6c (owner-run): the criterion passes; Discard fails. A mistaken first try (prompt "Gg", `run-1795250812644-1`) does
    not count. The quick run `run-1795253516592-2` ran the command with no denial, $0.0038, exit 0; its item is private,
    with `run:` and no `raised-by`. Discard's dialog read "Delete this work? This deletes the agent's copy of the
    project and its branch, which hold 0 changed files. ...". After Delete the worktree and branch stayed, and the store
    kept the Review item with no discard step (`ts-0160-th`). The owner removed them by hand.
    Re-check after `ts-0160-th` (owner-run, 2026-11-21): pass. Quick run `run-1795322167564-3`, exit 0. Discard's
    dialog read "Delete this work? Run run-1795322167564-3. This deletes the agent's copy of the project and its
    branch. They hold 0 changed files. Your own checkout is not touched. This cannot be undone." After Delete the card
    went and "Needs you" read "Nothing is waiting on you". The worktree and the branch are gone (`git worktree list`,
    `git branch --list 'collide/*'`). The re-check's first quick run, `run-1795315777377-1`, was discarded the same
    way. The two stale Review items of the first run were gone too.
  - `claude` 2.1.330 is outside the tested range (2.1.323 to 2.1.325); the smoke check passed and the runs went on, as
    designed.
- 7 Reid (agent-run): the criterion passes. The set-aside file got a new id, the linker's `blocked-by` was rewritten to
  it (the documented limit), and the commit touched the renamed file and the linker. Two findings: a copy named
  `-copy.md` sorts before its original, so the skip line named the original (the text now uses the test's
  `<id>-zcopy.md`); and with the daemon running the listing held two rows of the one key, and the bare-id reid was
  refused "matches 2 items", exit 3, until a restart. Right after the reid, one stale `skipped` line showed once
  (`ts-0161-ta`).
- 8 Fixtures (agent-run):
  - 8a: pass. The nine shapes recorded from the real CLI equal the committed ones; the recorder took 21 s. `new task`'s
    `data.id` carries the slug, and the recorder takes the short id (the text now says so).
  - 8b: the frontmatter keys are the same for the three kinds (11, 6 and 6 keys). The headings differ: the fixtures
    have none, and the real files share a few common ones (`ts-0162-yh`). Files: ADRs 31 real and 6 fixture, spikes 108
    and 4, questions 175 and 5.
- Differences found, each a fix item:
  - The loop page does not show a file run (`ts-0159-ra`). Fixed, and re-checked: pass.
  - Discard from the page did not apply (`ts-0160-th`). Fixed, and re-checked: pass.
  - A file run's card says Task "unknown" and leaves two rows blank (`ts-0178-fw`), seen at the re-check.
  - A watch update keeps a second row of one key (`ts-0161-ta`).
  - The fixtures have no section headings (`ts-0162-yh`).
  - The text's own expectations, fixed in place: 1.1, 1.2, 2, 3a, 3c, 4a to 4e, 5.1, 5.4, 6.0, 6b, 6c, 7, 8a.
- What was awkward (agent-run, for the owner to add to):
  - The app's build: 41 minutes, and it dirties a tracked file.
  - The app started from a terminal that held `env.sh` ran its own daemon in the deleted temp home, made the folder
    again, and showed none of the owner's runs (`ts-0164-pa`). The text now says to check `echo $HOME`.
  - The real daemon tries the boards of deleted check folders every 10 s and logs each time, and logs a listing failure
    as it stops (`ts-0163-su`).
  - The owner's queue keeps two stale Review items, and a mistaken prompt left a 20 MB worktree that the app cannot
    remove (`ts-0160-th`). Both were gone at the re-check.
  - zsh's `time` report does not follow a redirection; the text uses `secs`.
- The owner's own awkward notes: ____ (owner)
- Step 6's follow-up is not in the `follow-ups` view (plan 0.10): whether your own-terminal runs' follow-ups should
  show there before review shows them (Stage 9a): ____ (owner)
- Date the owner judges Stage 4c done: after `ts-0159-ra` and the owner's re-check of 6b's run card. 2026-11-21
  (owner: the re-check and its clean-up were done that day).

### Clean up

1. Your own state, from step 6, first. In your own terminal, from `<temp>/collide-clone`:

```sh
git worktree list
git branch --list 'collide/*'
find "$HOME/Library/Application Support/Collide/pack-data" -name '*follow-up-probe*'
```

   Remove each run's worktree and branch the page did not (6b's run, and 6c's if Discard did not apply), by the exact
   path and name those commands print: `git worktree remove --force <path>` and `git branch -D collide/<run id>`. A run
   whose clone is already gone is not in `git worktree list`: its folder stays in
   `~/Library/Application Support/Collide/workspaces/<run id>/`; remove it by that exact path. Remove each private file
   the `find` printed with `rm <exact path>`. The run rows in the real store and the folders `runs/<run id>/` stay: they
   are history. Remove a folder by its exact path if you do not want it. Then end the real daemon, as in 6.0, and quit
   the app.

2. This check's daemon, from a terminal that sourced `env.sh`:

```sh
. "$CHECK/env.sh"
stop_daemon
```

3. The temp folder, with the clones inside it. The `case` stops the command if `$CHECK` is anything else.

```sh
case "$CHECK" in
  /tmp/collide-check.*) chmod -R u+w "$CHECK" && rm -rf "$CHECK" ;;
  *) echo "CHECK is not the check folder: nothing removed" ;;
esac
```

4. Look that nothing is left, and that nothing of yours moved:

```sh
ls -d /tmp/collide-check.* 2>/dev/null
ps -ax -o pid=,command= | grep '[c]ollide-daemon'
git -C <collide checkout> status --short
git -C <orchard copy> status --short
```

   Pass: no folder; no daemon of the check (a daemon of yours that you did not stop may show: it is not this check's);
   `<collide checkout>` and `<orchard copy>` show what they showed before. Close the terminals that sourced `env.sh`.

### Known gaps

What you will meet, and the item it is carried to or from. None of it is a failure of this check.

- **`ts-0139-wq`, `ts-0140-gk`.** A run's follow-up has `raised-by` only when its task is a planning-source item; a
  quick run's task is scratch and a file run has none, so step 6's item is not in `follow-ups`. Follow-ups from a Plan
  or Spike run are Stage 9d. Keep, Share and Move are Stage 9a and 12. `task.preview` is not shaped for a run's agent.
  Whether Claude honours `Bash(collide:*)` beside the stdio permission tool is step 6's result.
- **`ts-0124-xm`.** A layout root other than `.collide/planning` is not git-ignored for its `.write-lock` and
  `.collide-tmp` file: they show in `git status` while a write runs or after one is killed. `code` is accepted and does
  nothing until Stage 9.
- **`ts-0125-uh`.** Share and Move between roots are Stage 12. A private item is seen from the main checkout only.
- **`ts-0126-nf`, `ts-0127-js`.** `watch` does not yet skip the rest of a spike folder or read `questions/` (Stage 6a).
- **`ts-0129-uv`.** The phase-end check of a hand-written `status:` or `by:` line is Stage 8. The app has no buttons for
  the person-only ops; the loop page answers the confirmation only.
- **`ts-0130-mv` to `ts-0134-ch`.** The CLI text shows features only: the decision log, the ledger, claims and
  pre-registration show in `--json` and in the app later.
- **`ts-0135-cx`.** `queue: true` lanes are Stage 6: `decisions-needed` is a view, not yet a lane.
- **`ts-0136-rr`.** No `--no-daemon`: every task command starts a daemon and says so once. Nothing in the app calls
  `task.create`, `task.reid` or `task.sections`.
- **`ts-0141-sn`, `ts-0152-jq`.** The app does not show `not_started` or `sources_unread`. A failure of `discover` has
  no test.
- **`ts-0149-kq`.** `dependsOn=a,b` is words between commas; a title with a comma cannot be given.
- **`ts-0151-fk`, `ts-0157-hd`.** A change between the daemon's first listing and the watcher's first read is not
  reported. The startup reconcile does not name link changes.
- **`ts-0153-kp`.** The gate's exit-1 refusal of the plan CLI (`errors[0].file`, `.message`) is not recorded: no legal
  call provokes it. Keys the real CLI has and the fake leaves empty (`status` proposals, epics, blocked tasks, velocity)
  pass as "empty matches any". `new task --blocked-by` is not a case.
- **`ts-0155-wb`.** The app does not show the wait after a restart.
- **`ts-0156-nt`.** One board cannot hold Collide's plan and Orchard's: both are the `plan` source (step 4e).
- **`ts-0158-ow`.** A failed queue is not retried by the watcher, only by a `get`. A question's place in the queue can
  differ from a fresh `get` until a question changes.
- **Here.** The fixtures and the fake plan CLI are made up; their headers say they follow the shapes read in your copy.
  The reference pack's own `pack.yaml` is not loaded: its header is replaced (step 4a).

### Automated tests this check leans on

The item's Tests section, by item id, with the files they are in. None of them starts a real `claude` or touches a real
home, launchd or daemon.

- `ts-0124-xm`, `ts-0125-uh`: `packs/plan/test/layout.test.ts`, `packs/plan/test/private-root.test.ts`,
  `host/tests/daemon_plan_layout.rs`.
- `ts-0126-nf`, `ts-0127-js`: `packs/plan/test/spikes.test.ts`, `packs/plan/test/questions.test.ts`.
- `ts-0128-bb`, `ts-0129-uv`: `host/tests/person_only.rs`, `host/tests/daemon_plan_person_ops.rs`,
  `packs/plan/test/transitions.test.ts`.
- `ts-0130-mv` to `ts-0134-ch`: `packs/plan/test/feature-ops.test.ts`, `decisions.test.ts`, `assumptions.test.ts`,
  `findings.test.ts`, `claims.test.ts`, `preregistration.test.ts`, `coverage.test.ts`, `feature-links.test.ts`.
- `ts-0135-cx`, `ts-0136-rr`: `host/tests/views_find.rs`, `host/tests/daemon_plan_views.rs`,
  `host/tests/task_daemon_cli.rs`, `core/tests/view_builtin.rs`.
- `ts-0137-pg`: `fixtures/arch-plan-tests/test/kinds.test.ts`, `queue.test.ts`, `host/tests/daemon_reference_kinds.rs`.
- `ts-0139-wq`, `ts-0140-gk`: `packs/plan/test/follow-ups.test.ts`, `host/tests/daemon_follow_ups.rs`,
  `host/tests/guide_command.rs`, `host/tests/collide_path.rs`.
- `ts-0141-sn`, `ts-0152-jq`, `ts-0155-wb`: `host/tests/daemon_corpus.rs`, `host/tests/task_daemon_cli.rs`.
- `ts-0142-ua`, `ts-0146-se`: `host/tests/write_guard.rs`, `host/tests/board_held_line.rs`.
- `ts-0143-ah`: `host/tests/base_env.rs`, `host/tests/phases.rs`. `ts-0144-eh`, `ts-0147-ve`:
  `core/tests/ready_rule.rs`, `core/tests/view_eval.rs`. `ts-0149-kq`: `core/tests/short_ids.rs`,
  `host/tests/task_args.rs`.
- `ts-0145-cq`, `ts-0148-cv`, `ts-0150-dj`, `ts-0151-fk`, `ts-0157-hd`, `ts-0158-ow`: `fixtures/arch-plan-tests/test/`
  (`write`, `list`, `fake-plan`, `watch`, `watch-collections`, `get-from-read`).
- `ts-0153-kp`: `fixtures/arch-plan-tests/test/shape.test.ts` and `contract.test.ts`; the real shapes are the files in
  `fixtures/orchard-arch/contract/shapes/`.
- `ts-0154-gr`, `ts-0156-nt`: the bounded run's tests in `host/tests/common/mod.rs`'s users, and the Orchard provider
  and daemon tests for `--kind task`.

What this check adds that no test has: the real Orchard tree and plan CLI, a real daemon with the app's loop page, and
your own Claude Code session with the real `claude`.
