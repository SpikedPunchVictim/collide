---
id: ts-0138-pn
kind: task
status: ready
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

   The three app lines are those of `ts-0083-uf`. That the last build puts `collide-app` in `target/debug` is
   **(unconfirmed)**: `ts-0083-uf` says the same. If `ls` fails for `collide-app`, find it under `target/` and say so in
   step 9; steps 5 and 6c need it.

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
echo "$CHECK"
```

   In every other terminal of this check, run `. <temp>/env.sh` (the folder `echo` printed) first. It sets the temp
   `HOME` in that shell, so run nothing else from your own account in it.

   The `.zshenv` line is for the daemon, which captures its base environment from a login shell started with `-l -i -c`
   (`host/src/base_env.rs`). Under the temp `HOME` that shell reads none of your profile, so it may find no `node`.
   Putting your `PATH` in `.zshenv` is the fix; that it reaches the daemon is **(unconfirmed)**, as in `ts-0121-xp`.
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
also starts this check's daemon, and says so once on standard error. Whether a task has a `path` in the JSON is
**(unconfirmed)**: the writer read `pathOf` in `packs/plan/src/scan.ts`, which gives one for every file item. If `rows`
prints `-` for the path, count with `collide tasks --view spikes` alone and say so in step 9.

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

Pass: the first create prints `ok  created plan:ts-<id>` and a `title` line, with no `file` line, and exits 0.
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

Pass: nothing on standard output from the plan source; a line `collide: source plan could not start: <reason>` on
standard error naming `layout.root` and the rule (a `..` part); exit 1. The exact words of the reason are the provider's
(`src/layout.ts`); write them in step 9. Then two more cases, each the same four lines with a new `root`:

```sh
printf 'config:\n  collide/plan:\n    layout:\n      root: .git/planning\n' > .collide/config.yaml
git commit -q -a -m "check: a root under .git" && stop_daemon && collide tasks; echo "exit $?"
ln -s "$CHECK" outside-link
printf 'config:\n  collide/plan:\n    layout:\n      root: outside-link\n' > .collide/config.yaml
git add outside-link .collide/config.yaml && git commit -q -m "check: a link out" && stop_daemon
collide tasks; echo "exit $?"
```

Pass: each is refused with the reason (a root under `.git`; a link that leaves the repository) and exit 1. Then put the
clone back:

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
`y`. Then "Trusted line ... at <hash>." `collide init` starts the daemon and says so once. Whether the manifest above
loads is **(unconfirmed)**: it is the test's manifest with the header written by hand; if `collide trust` or the first
listing refuses it, write the words in step 9.

**4b. The Decisions needed view against the human queue.** The provider runs
`./plan status --root <root>/arch --format json` itself and puts each question the queue names in the collection
`plan:queue`, which the view lists. Compare the two, from the real CLI's own output.

```sh
cd "$COPY"
time collide tasks > "$CHECK/all.txt"
wc -l < "$CHECK/all.txt"
./plan status --root "$COPY/arch" --format json > "$CHECK/status.json"
node -e 'const s=JSON.parse(require("fs").readFileSync(process.argv[1],"utf8"))
for(const q of s.data.queue)console.log(q.question)' "$CHECK/status.json" > "$CHECK/queue.ids"
collide tasks --view decisions-needed | awk '{print $1}' | sed 's/^plan://' > "$CHECK/view.ids"
wc -l "$CHECK/queue.ids" "$CHECK/view.ids"
sort "$CHECK/queue.ids" | diff - <(sort "$CHECK/view.ids") && echo "same ids"
diff "$CHECK/queue.ids" "$CHECK/view.ids" && echo "same order"
```

Pass: the listing prints one line per item as `key  status  kind  title`, the time is the figure for a real tree (write
it), `same ids`, and the two counts agree. `same order` should print too: the plan CLI sorts by id and the view lists by
key. If the counts differ, write both lists' ids, never their titles. A line `collide: skipped <path>: <reason>` on
standard error is a file the provider could not read: write each reason in step 9. `data.queue[].question` is the field
the provider reads (`queue.ts`); that the real CLI names it the same is what the shape files of step 8 hold.

**4c. The evidence link from the spike's side.** An ADR's `evidence` points at a spike; the spike shows the incoming
link by its inverse, "evidence for" (`evidence` in `fixtures/packs/orchard/providers/plan/declaration.ts`).

```sh
for k in $(collide tasks --kind spike | awk '{print $1}'); do
  collide task "$k" --json | node -e 'const d=JSON.parse(require("fs").readFileSync(0,"utf8"))
  const e=d.links.filter(l=>l.dir==="in"&&l.link_type==="evidence"&&!l.missing).map(l=>l.key)
  if(e.length)console.log(d.task.key,"<-",e.join(" "))'
done | head -5
```

Pass: at least one spike line, naming one or more ADR keys. Then the text form of one of them, with a spike key from the
output: `collide task plan:<spike id> | grep -i evidence`. Pass: the line reads "evidence for" and the ADR. The JSON
field names (`dir`, `link_type`, `key`, `missing`) are those of the Rust fields the daemon test reads; that they
serialize unchanged is **(unconfirmed)**: if the loop prints nothing, run `collide task plan:<spike id> --json` and look
at `links`. If the copy has no ADR with evidence, write that in step 9.

**4d. The CLI forms on a real clone** (`ts-0149-kq`, Not done: they were never run on a real tree). Writes go through
the real `./plan`, in `<check copy>`. Give the first `collide task create` a made-up title.

```sh
cd "$COPY"
collide tasks --ready | head -5
collide tasks --kind adr | head -3
collide task create "Probe task" epic="$EPIC_ID" proposal="$PROPOSAL_ID"; echo "exit $?"
git status --short
```

Pass: `--ready` prints tasks, or says why none is ready; the create prints `ok  created plan:<id>`; `git status` shows
the files the real plan CLI wrote. Take the new key for `<probe key>`, then:

```sh
collide task "<probe key>" | head -8
collide task "<probe key>" setStatus ready; echo "exit $?"
collide task "<probe key>" setStatus done; echo "exit $?"
```

Pass: the text shows the title and its status; `setStatus ready` exits 0; the move to `done` is refused with the plan
CLI's own words and exit 5 (that the plan CLI's lifecycle forbids `ready` to `done` is **(unconfirmed)**: if it allows
it, write that in step 9). A bare id (`collide task <id>` with no `plan:`) may be refused as "not an item id"
(`ts-0149-kq`): write the words either way.

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
time collide tasks > "$CHECK/settled.txt"
stop_daemon
time collide tasks > "$CHECK/first.txt" 2> "$CHECK/first.err"; echo "exit $?"
cat "$CHECK/first.err"
cmp "$CHECK/settled.txt" "$CHECK/first.txt" && echo "stdout equal"
```

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
   Features part; whether an empty one is clean is **(unconfirmed)**, so the second line adds one feature.

```sh
. "$CHECK/env.sh"
cd "$CLONE"
git switch -q -c accept-check
collide task create "Accept probe" kind=proposal epic=ep-004-qd --commit
```

   Copy the key it prints (`plan:pr-<id>`) into the next lines.

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
         of the title are `waiting_title`'s in the daemon: **(unconfirmed)**; write what it says.
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
   `git checkout -- . && git clean -fd .collide` if the diff above is still there. Quit the app.

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

1. A terminal that has **not** sourced `env.sh`; call it your own terminal. Stop an older daemon of yours: one from an
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

   > Run `collide guide` and read it. Then start one run with `collide run follow-up-probe.md --budget 0.10` in the >
   background, and tell me the run id it prints. Do not use sleep.

   `collide run` stays until the phase ends, and a long wait may time out in the session's shell: that is why it is
   started in the background. `--budget 0.10` is a backstop; the run is one command on the small model. Start the loop
   page from another own terminal, to watch it:

```sh
"<collide checkout>/target/debug/collide-app"
```

   What to look at on the page:

   - [ ] The run card shows the run id the session printed, with its state moving from running to ended. Whether the
     page lists a file run in the card (it reads `run.list` with no board, `app/src/loop/loop.ts`, `sync`) is
     **(unconfirmed)**: the writer read the code and never saw a file run on the page. If the card stays empty, that is
     the result for the first criterion.
   - [ ] "Needs you" shows no `permission` row naming the `collide` command at any time. A file run may end with no row
         at all; whether it leaves one is **(unconfirmed)** (`quick_run.rs` makes a Review item for a quick run, and the
         writer found none for a file run).
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
   (and confirm): it removes the run's worktree and branch (`discard` in `host/src/file_run/discard.rs`).

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

Copy the two keys. `ID` is the probe's id without `plan:`.

```sh
export ID=ts-<probe id> LINKER=plan:ts-<linker id>
collide task "$LINKER" link "$ID" type=blocked-by --commit
FILE="$(find .collide/planning -name "$ID-*.md")"
echo "$FILE"
cp "$FILE" "${FILE%.md}-copy.md"
sed -i '' 's/^title: .*/title: "Reid probe copy"/' "${FILE%.md}-copy.md"
git add -A .collide && git commit -q -m "check: the same id twice"
collide tasks 2>&1 >/dev/null | grep skipped
```

Pass: `echo "$FILE"` prints one path; the last command prints one `collide: skipped <path>: <reason>` line for the copy,
and the reason says the id is taken twice. The first file is the item; the second in scan order is the one set aside.
Then:

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

Pass: one task id. Copy it:

```sh
export TASK_ID=T-00-000    # edit: the id printed
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

- Time taken, in total and per step: 1 ____ 2 ____ 3 ____ 4 ____ 5 ____ 6 ____ 7 ____ 8 ____
- 1 Set up: ____ (the `npm install` pair; `collide-app` found in `target/debug`?)
- 2 Spikes: ____ (records, listed, `skipped` lines)
- 3 Private root and layout: ____ (3a `git status`; 3b `follow-ups`; 3c the three refusals, with their words)
- 4 Orchard: ____ (4a trust and load; 4b the id lists, both counts, the time of the first listing; 4c; 4d; 4e
  `--kind task`, the two times of the restart and whether the waiting line printed)
- 5 Accept: ____ (CLI exit 12; the row on the page and its words; the diff after Accept)
- 6 Own terminal: ____ (6a; 6b the run id, the card, any permission row, the end state and exit code; 6c the same)
- 7 Reid: ____ (the skipped line, the commit, the linker's target)
- 8 Fixtures: ____ (every `DIFF` file and key; every key of 8b)
- Differences found, each one line, for a fix item: ____
- What was awkward: ____
- Step 6's follow-up is not in the `follow-ups` view (plan 0.10): whether your own-terminal runs' follow-ups should
  show there before review shows them (Stage 9a): ____
- Date the owner judges Stage 4c done: ____ (owner)

### Clean up

1. Your own state, from step 6, first. In your own terminal, from `<temp>/collide-clone`:

```sh
git worktree list
git branch --list 'collide/*'
find "$HOME/Library/Application Support/Collide/pack-data" -name '*follow-up-probe*'
```

   Remove each run's worktree and branch the page did not (6b's run, and 6c's if Discard was not pressed), by the exact
   path and name those commands print: `git worktree remove --force <path>` and `git branch -D collide/<run id>`. Remove
   each private file the `find` printed with `rm <exact path>`. The run rows in the real store and the folders
   `runs/<run id>/` stay: they are history. Remove a folder by its exact path if you do not want it. Then end the real
   daemon, as in 6.0, and quit the app.

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
- **`ts-0156-nt`.** No test, and here no way found, to put Collide's plan and Orchard's on one board (step 4e).
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
