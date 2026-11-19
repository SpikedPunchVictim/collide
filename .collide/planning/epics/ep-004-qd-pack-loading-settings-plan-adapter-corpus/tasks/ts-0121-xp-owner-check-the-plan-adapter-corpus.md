---
id: ts-0121-xp
kind: task
status: active
title: "Owner check: the plan adapter, corpus and views on the Orchard repo"
owners: []
links:
  - { type: blocked-by, to: ts-0120-wm }
---

## Goal

The Stage 4b checks that only the owner runs, on the Orchard repo: the plan adapter, the corpus and the views against
the real `arch/` tree and the real plan CLI, which the agent-built items only meet as fixtures.

A build agent never runs it. It never touches the Orchard repo or any other repo, never starts a daemon against the
owner's data, and never runs `claude`. It writes the command lines and a short checklist under `### Owner checklist`,
from the code and from the Built sections of `ts-0108-dq` to `ts-0120-wm`. The owner runs them and records the result
here as owner judgement, with what was awkward.

The owner's run is also the one place the fixtures are compared with the real thing: the `arch/` shapes of
`fixtures/orchard-arch/` against the real files, and the fake plan CLI against the real one (its `--format json`,
`--dry-run` and exit codes). A difference is recorded here and becomes a fix item.

## Success criteria

- A task shows `doing` in one worktree and `ready` at the board root, with the divergence reported by `collide task <id>
  --json` and `task.get`. Owner-run.
- The MVP view's `closure` for T-01-042 has the same task set and the same steps as the prototype board's generator's
  for T-01-042 at the same commit. Owner-run. A difference is the closure rules of `ts-0118-wp` to correct, and the
  record says which tasks differ.
- `"plan:E-07" in collections` returns the same tasks as the prototype board's E-07 epic. Owner-run.
- `setStatus` with an illegal transition comes back `refused`, with plan's message. Owner-run.
- A legal `setStatus` reports exactly the task file and its regenerated epic table as touched, and `setStatus` with
  category `active` writes `doing`. Owner-run.
- A write into a checkout with a running phase is queued, not applied. Owner-run, with a phase that does not call
  `claude` (see Notes).
- A write to the board root during a rebase waits, and the CLI says why. It applies once the rebase ends. Owner-run, in
  a copy of the repo made for the check.
- An edited provider script or view on a branch is not used, and a changed hash asks again. Owner-run.
- A scratch task created with `collide task new` is listed for that board only. Owner-run.
- The fixtures match the real `arch/` shapes and the real plan CLI, or each difference is written down. Owner-run.
- The checklist holds the command lines, with `<collide checkout>`, `<orchard copy>` and `<scratch repo>` as
  placeholders, what to look at, how to clean up, and a line for the owner's result and one for what was awkward.
- A line records the date Stage 4b is judged done by the owner.

## Tests

None by a build agent: this check is the owner's. The checklist names the automated tests it relies on:

- `ts-0108-dq`, `ts-0109-qj`: the supervisor against a fake provider, and the SDK's hooks;
- `ts-0110-ra`, `ts-0111-vn`, `ts-0112-px`: the adapter against fixture `arch/` trees and the fake plan CLI, in one
  checkout and two (rename, collision, watch);
- `ts-0113-ck`, `ts-0114-hr`: the daemon's sources and trust gate, and the write guards (root busy, the phase queue);
- `ts-0115-ug`, `ts-0116-wh`: the corpus, its cache and its events;
- `ts-0117-gu`, `ts-0118-wp`, `ts-0119-tj`: CEL, the views, and the views in the daemon;
- `ts-0120-wm`: the CLI.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 4b: Success Criteria ("On the Orchard repo", the nine criteria above); Tests.
  - 0.4: Root busy. 0.9: Trust. 0.10: Providers, Ref-scoped sources, Views. Part L.
- What the checklist covers. The build agent fills in the exact commands once the earlier items are built.
  - **Set up.** Build the CLI and the daemon from `<collide checkout>`. Make `<orchard copy>` with `git clone` of the
    repo or `git worktree add` into a folder made for the check (never the repo the owner works in), and a second
    worktree of it. Use a fresh `HOME` and `COLLIDE_DAEMON_ROOT` for everything, so the owner's settings and daemon stay
    as they are. Say in the checklist which steps, if any, use the real `HOME`.
  - **Read side.** `collide init`, commit, `collide trust`; `collide tasks`; `collide task <id> --json` for a task whose
    state differs between the two worktrees (change one with the plan CLI, in the worktree).
  - **Views.** `collide tasks --view <name>` for the MVP view and a view on `"plan:E-07" in collections`; compare the
    set and the steps with the generator's output at the same commit, and the E-07 set with the epic's.
  - **Writes.** An illegal `setStatus` through `collide task <id> setStatus <status>`, a legal one, and one by category
    `active`; the files touched (`git status` in the copy) against the report.
  - **Queue and wait.** A write while a phase is live in the checkout (a workflow of the fixture packs that runs a
    `sleep`), then one in a copy stopped mid-rebase, read the CLI's message, finish the rebase, see the write apply.
  - **Trust.** Edit the adapter script and a view in a commit on a branch that is not the line, see neither used; change
    one on the line and see the second prompt.
  - **Scratch.** `collide task new` in `<orchard copy>` and in `<scratch repo>`; each lists only its own.
  - **Fixtures.** The real `arch/` task file, epic file with its generated table, id map head, and the plan CLI's
    `--format json`, `--dry-run` and an illegal-status run, next to `fixtures/orchard-arch/`; write the differences.
  - **Record.** The result, the time it took, and what was awkward.
- **Decision for the owner, with a recommendation: the phase for the queue check.** The queue criterion needs a live
  phase in a checkout. Recommend a fixture workflow whose phase is a shell `sleep`, which needs no `claude` and no
  spend. Alternative: a real `claude` phase on haiku, which costs, and the owner sets the cap in the checklist (a figure
  of `$0.10`) if they choose it.
- **Decision for the owner: what to share first.** If the owner supplies a sanitized sample of the `arch/` files and the
  plan CLI's output before `ts-0110-ra` is built (see that item's Notes), the fixtures start from the real shape and
  this step's fixture comparison is short.
- Area: this item's own text. No code.
- The test rule, for anything automated this check leans on: no test touches the real home, real launchd or a real
  daemon. Each test clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as
  `host/tests/common/mod.rs` `TempRoot` does. Settings files are fixtures, never the owner's own `settings.json`. The
  owner's own run here uses the owner's real repo copy by choice, where the checklist says so.
- Stage 4b item (revision 23 scope, split 2026-11-17). Items of Stage 4b run one at a time, in id order, because they
  share `core`, `host` and the generated files.
- Nothing here runs a real `claude`, unless the owner picks the alternative above.
- Owner decisions (2026-11-17, at the split): the recommendations above, on delegation (a fixture phase that runs
  `sleep`). The owner made a local copy of the Orchard repo; which steps of this check could run on it is decided when
  the checklist is written.

### Owner checklist

**Run by an agent on 2026-11-19**, at the owner's ask, on a temp clone of the owner's copy with a temp `HOME` and daemon
root and the stand-in `claude`. Its results are in step 9. A line it found wrong is fixed or followed by a line that
begins **(agent run)**.

Written 2026-11-18 from the code at `83d9915` and the Built sections of `ts-0108-dq` to `ts-0120-wm`. **Every command
below was not run by the writer.** A line marked **(unconfirmed)** is a step or an expected output that the writer
could not confirm from code; it says what was looked for. If a pass line does not come out, that is a result: write it
in step 9.

The names. `<collide checkout>` is your Collide repository. `<orchard copy>` is your local copy of the Orchard repo;
**no step runs in it**: step 1 reads it once, with `git clone`, and everything else runs in `<check copy>`, a clone
made for this check. `<check worktree>` is a worktree of `<check copy>`. `<scratch repo>` is a new empty repository.
`<temp>` is the one folder this check lives in. Each is a shell variable below (`$COPY`, `$WT`, `$SCRATCH`, `$CHECK`).

**Your own state stays apart.** Every step after step 1 runs with a fresh `HOME` and `COLLIDE_DAEMON_ROOT` inside
`<temp>`, so your settings, trust file, daemon and runs are not touched (`ROOT_ENV` in `host/src/daemon.rs`;
`FORWARDED_ENV` in `host/src/launch.rs` hands `COLLIDE_DAEMON_ROOT`, `HOME` and `SHELL` to the daemon). With
`COLLIDE_DAEMON_ROOT` set, no launchd job is loaded. **Only step 1's build uses the real `HOME`** (cargo and pnpm
caches); it runs before the temp `HOME` is set. No step needs the real keychain, and none starts the real `claude`.

**Spend: none.** The queue check (step 5) uses a stand-in `claude`, a shell script that plays a recorded session and
then sleeps. The item asked for a fixture workflow phase that runs `sleep`. **None exists**: no fixture pack has a
phase that runs a command, and no code runs a pack workflow's phases before Stage 8. The only live phase today is
`collide run <task-file>`, which starts `claude`. The stand-in is the smallest way to hold one open at no cost.

The real plan CLI is the one thing the writer cannot see. Whether `./plan` runs in a fresh clone, and whether it needs
the real `HOME` or a toolchain installed first, is **(unconfirmed)**. Step 1 tests it before anything else.

### 1. Set up

1. Build from `<collide checkout>`, with your real `HOME`. `collide` starts the `collide-daemon` beside it, so use
   both from one folder. The loader finds the built-in packs and the SDK in the folder `pnpm build:packs` stages.

```sh
cd <collide checkout>
pnpm install                 # once since ts-0109-qj: the pack SDK needs `yaml`
pnpm build:packs
cargo build -p collide-host  # target/debug/collide and target/debug/collide-daemon
ls target/debug/collide target/debug/collide-daemon
```

2. Make `<temp>` and the environment file. The short `/tmp` path keeps the daemon's socket path under its limit of 103
   bytes (`MAX_SOCKET_PATH_BYTES` in `host/src/daemon.rs`). Git gets an identity because the fresh `HOME` has none.
   Replace `<collide checkout>` on the first line. Paste the whole block.

```sh
export COLLIDE_SRC=<collide checkout>
export CHECK="$(mktemp -d /tmp/collide-check.XXXXXX)"
mkdir -p "$CHECK/home" "$CHECK/bin" "$CHECK/root/data/workspaces" "$CHECK/scratch"
cat > "$CHECK/env.sh" <<EOF
export CHECK="$CHECK"
export COLLIDE_BIN="$COLLIDE_SRC/target/debug"
export COLLIDE_SRC="$COLLIDE_SRC"
export COPY="$CHECK/copy" SCRATCH="$CHECK/scratch" WT="$CHECK/root/data/workspaces/check-wt"
export HOME="$CHECK/home" COLLIDE_DAEMON_ROOT="$CHECK/root"
export GIT_AUTHOR_NAME=owner-check GIT_AUTHOR_EMAIL=owner-check@example.invalid
export GIT_COMMITTER_NAME=owner-check GIT_COMMITTER_EMAIL=owner-check@example.invalid
export PATH="$CHECK/bin:$PATH"
collide() { "\$COLLIDE_BIN/collide" "\$@"; }
# a JSON field, by dotted path: ... --json | jf view.steps
jf() { node -e 'let d=JSON.parse(require("fs").readFileSync(0,"utf8"))
for(const k of process.argv[1].split("."))d=d[k];console.log(JSON.stringify(d,null,1))' "\$1"; }
# the task ids of a text listing, without the source, sorted
members() { awk '{print \$1}' | sed 's/^plan://' | sort; }
# the steps of a view, one line each: ... --json | steps
steps() { node -e 'const v=JSON.parse(require("fs").readFileSync(0,"utf8")).view
v.steps.forEach(s=>console.log(s.map(k=>k.replace(/^plan:/,"")).join(" ")))'; }
# the status line of a task file, and the file: status_of T-NN-NNN [checkout]
status_of() { find "\${2:-\$COPY}/arch" -name "\$1-*.md" -exec grep -m1 '^status:' {} +; }
file_of() { find "\${2:-\$COPY}/arch" -name "\$1-*.md"; }
EOF
printf 'export PATH="%s"\n' "$CHECK/bin:$PATH" > "$CHECK/home/.zshenv"
echo "$CHECK"
```

   In every other terminal, run `. <temp>/env.sh` (the folder `echo` printed) before anything else. `env.sh` sets the
   temp `HOME` in that shell, so run nothing else from your own account in it.

   The `.zshenv` line is for the daemon, which captures its base environment from a login shell started with `-l -i
   -c` (`host/src/base_env.rs`). Under the temp `HOME` that shell reads no profile of yours, so it may find neither
   `node` nor the stand-in `claude`. Putting your `PATH` in `.zshenv` is the fix; that it reaches the daemon is
   **(unconfirmed)**: the writer read how the shell is started, not what `path_helper` does to the order. Check it:

```sh
. "$CHECK/env.sh"
"$SHELL" -l -i -c 'command -v node; command -v claude' 2>/dev/null
```

   Pass: `node` is the one you use. The `claude` line may name your real one here: the stand-in is made in sub-step 5,
   and step 5a checks this again before it starts a run. If your `SHELL` is not zsh, write the same line in its own
   startup file for non-interactive shells, or skip step 5a.

3. Make `<check copy>`: a clone of `<orchard copy>`, which only reads it. `--no-hardlinks` shares no files with it.

```sh
. "$CHECK/env.sh"
git clone -q --no-hardlinks <orchard copy> "$COPY"
git -C "$COPY" branch --show-current
ls "$COPY/arch" "$COPY/plan"
```

   The clone checks out the branch the copy had checked out. That branch must hold `arch/` and the plan CLI. If it
   does not, switch to the one that does: `git -C "$COPY" switch -c check-line --track origin/BRANCH` (BRANCH is
   yours). Call whichever branch is checked out in `<check copy>` the line; every step below keeps it checked out.

   Try the real plan CLI in the clone before going on. It must start with the temp `HOME`:

```sh
cd "$COPY" && ./plan --help | head -5
```

   If it fails for a missing package or toolchain, install what the Orchard repo's own README says, in `<check copy>`
   only, and write what you needed in step 9 (awkward). Whether that needs your real `HOME` is **(unconfirmed)**; if
   it does, say so in step 9 and do the install in a terminal that has not sourced `env.sh`.

   **(agent run)** It failed: after the README's `npm install` alone no command is found, since the CLI's `dist/` is not
   committed. `npm install` and then `npm run build`, in `<check copy>` with the temp `HOME`, made it work. A worktree
   made later needs the same (step 5a).

4. Pick the tasks. Each is a short id (`T-NN-NNN`) from your own `arch/`. Statuses and the lifecycle are the real
   repo's; the commands below assume `specified`, `ready`, `doing`, `done`, as `fixtures/orchard-arch/plan` does.

```sh
cd "$COPY"
grep -rl '^status: ready$' arch/proposals --include='T-*.md' | head -12
grep -rl '^status: specified$' arch/proposals --include='T-*.md' | head -4
```

   Six tasks in `ready`, one in `specified`, and one goal. `GOAL` is the task the item names, `T-01-042`, if it exists
   and has open dependencies; otherwise one that does. Edit the ids, then append them to `env.sh`.

```sh
cat >> "$CHECK/env.sh" <<'EOF'
export GOAL=T-01-042
export T_DIV=T-NN-NNN   # ready: step 2, doing in <check worktree>
export T_LEG=T-NN-NNN   # ready: step 4, a legal status write
export T_CAT=T-NN-NNN   # ready: step 4, a write by category
export T_Q1=T-NN-NNN    # ready: step 5, queued behind a live phase
export T_Q2=T-NN-NNN    # ready: step 5, queued behind a rebase
export T_DRY=T-NN-NNN   # ready: step 8, a dry run of the real plan CLI
export T_ILL=T-NN-NNN   # specified: steps 4 and 8, an illegal move to done
EOF
```

   Open `env.sh` and replace each `T-NN-NNN`. Then `. "$CHECK/env.sh"` again, and look at each:

```sh
for t in $T_DIV $T_LEG $T_CAT $T_Q1 $T_Q2 $T_DRY $T_ILL $GOAL; do echo "$t  $(status_of $t)"; done
```

   Pass: `ready` for the six, `specified` for `T_ILL`, and one path each.

5. The stand-in `claude`, for step 5 only. It answers `--version` as `2.1.325 (Claude Code)` (inside the range the
   tests use, `host/tests/adapters/claude/run_command.rs`) and otherwise plays the first four steps of a recorded
   session with `host/tests/adapters/claude/fixtures/fake_claude.py`, then sleeps 120 seconds. It is
   `ClaudeBinary::write`'s script in `host/tests/adapters/claude/fake.rs` and the `cut_recording` of `run_command.rs`,
   which run against a daemon in the tests. That the daemon accepts it through its version check and its smoke check
   when it is live, and not only in the tests, is **(unconfirmed)**; if `collide run` says the version is not usable,
   write that in step 9 and do step 5's rebase part only.

```sh
. "$CHECK/env.sh"
grep -v -e '^#' -e '^$' "$COLLIDE_SRC/host/tests/adapters/claude/fixtures/recordings/report.jsonl" | head -4 \
  > "$CHECK/cut.jsonl"
echo '{"sleep": 120}' >> "$CHECK/cut.jsonl"
cat > "$CHECK/bin/claude" <<EOF
#!/bin/sh
if [ "\$*" = "--version" ]; then echo "2.1.325 (Claude Code)"; exit 0; fi
exec /usr/bin/python3 $COLLIDE_SRC/host/tests/adapters/claude/fixtures/fake_claude.py $CHECK/cut.jsonl "\$@"
EOF
chmod +x "$CHECK/bin/claude"
"$CHECK/bin/claude" --version
mkdir -p "$HOME/.config/collide"
printf 'tools:\n  claude:\n    fast: { name: haiku }\n' > "$HOME/.config/collide/models.yaml"
```

   Pass: `2.1.325 (Claude Code)`. The `models.yaml` maps the `fast` alias the run asks for (see `run_command.rs`); the
   file is in the temp `HOME`, not yours. The run also prints the published version, which is a read of the npm
   dist-tags and nothing more; `collide config set claude.check_published false` turns it off.

Record the time this step took in step 9.

### 2. Read side

Goal: the plan adapter lists the real `arch/` through the daemon, and one task shows two states in two checkouts.
Criterion: "A task shows `doing` in one worktree and `ready` at the board root".

1. Make the board in `<check copy>`. The reference pack is the adapter in `fixtures/packs/orchard/providers/`, with a
   minimal manifest like the one in `host/tests/daemon_sources.rs` (`MANIFEST`). The fixture pack's own `pack.yaml`
   names hooks that are not in the fixture folder, so it is **not used here** (that it would not load is
   **(unconfirmed)**; the writer did not try). `collide init` refuses when `.collide/` already exists; if it does,
   skip it and put the `packs:` entry in the board file that is there.

```sh
. "$CHECK/env.sh"
cd "$COPY"
collide init
mkdir -p .collide/packs/reference .collide/views
cp -R "$COLLIDE_SRC/fixtures/packs/orchard/providers" .collide/packs/reference/providers
cat > .collide/packs/reference/pack.yaml <<'EOF'
apiVersion: collide/v0
name: reference
version: 0.1.0
requires:
  sdk: ">=0.2 <0.3"
  tools:
    - { name: node }
providers:
  plan: { script: providers/plan.ts }
EOF
cat > .collide/board.yaml <<'EOF'
packs:
  - source: repo:.collide/packs/reference
EOF
git add .collide && git commit -q -m "check: reference pack"
collide trust
```

   `collide init` starts the daemon, and says so once. Pass for `trust`: `Line <the line>`, a `Trust hash`, `Changes:`
   listing the added files, a block `Pack reference 0.1.0` with `Source: repo:.collide/packs/reference` and the
   provider `plan` and its script under what the pack runs, then `Trust this? [y/N]`. Read the review; answer `y`.
   Then "Trusted line ... at <hash>." The exact review lines are `render_review` in `host/src/pack_cli.rs`, which the
   writer did not read; take the shape of step 3 of `ts-0105-jv`.

2. List the board. Since `ts-0136-rr` every form of `collide tasks` goes through the daemon, and `--checkout` lists one
   checkout. A task key is `plan:<short id>`, such as `plan:T-01-042`. A bare short id of a plan task is refused (agent
   run: "not an item id", `ts-0149-kq`), so always give the key.

```sh
cd "$COPY"
time collide tasks --checkout "$COPY" > "$CHECK/all.txt"
wc -l < "$CHECK/all.txt"
head -5 "$CHECK/all.txt"
ls arch/proposals/*/tasks/T-*.md | wc -l
collide tasks --checkout "$COPY" --json | jf refreshing
```

   Pass: one line per task, as `key  status  kind  title` (a task has no kind: `-`); the first command may wait while
   the first listing finishes, then prints; the two counts agree, less any file the adapter reports as skipped on
   standard error (`collide: skipped <path>: <reason>`; write each in step 9, which is a shape the fixtures lack);
   `refreshing` is `false`. **Write the time:** this is the figure for a real tree of about 1,660 files (`ts-0116-wh`
   records its own for 1,600 tasks of fixture data). Run the `time` line once more after stopping the daemon (Clean
   up, sub-step 1; the next `collide` starts a new one); the second figure is the start from the store's cache
   (`ts-0116-wh`). Write both.

3. Make `<check worktree>` and change one task in it only. The worktree **must be under**
   `<temp>/root/data/workspaces/`: the corpus classes a checkout as a workspace by that folder, and the divergence is
   only computed for workspaces (`corpus_service.rs` `discover`, `core/src/corpus.rs` `divergences_of`).

```sh
git -C "$COPY" worktree add -q -b check-wt "$WT"
status_of "$T_DIV" "$WT"
sed -i '' 's/^status: ready$/status: doing/' "$(file_of "$T_DIV" "$WT")"
status_of "$T_DIV" "$WT"
status_of "$T_DIV" "$COPY"
sleep 5
collide task "plan:$T_DIV" --json | jf copies
collide tasks --checkout "$COPY" --json | jf divergent
collide tasks --checkout "$WT" | grep "$T_DIV"
collide tasks --checkout "$COPY" | grep "$T_DIV"
```

   Pass: `copies` has two entries, one for `$COPY` with `state.status` `ready` and one for `$WT` with `doing`, and
   `divergent` is `["plan:<T_DIV>"]` and no other key. The two listings show `doing` and `ready`. The change is seen
   by the watch within a few seconds; if the first read still shows `ready`, read again, and write the delay in step
   9. If the file path in `status_of` shows nothing, your `arch/` layout differs from the fixtures: write it in step
   8. **(agent run)** At 5 s there was one copy; the second showed at about 20 s.

### 3. Views

Goal: the closure and a collection view give the same tasks as the prototype board's generator, and the two closure
decisions of `ts-0118-wp` are settled. Criteria: "The MVP view's `closure` for T-01-042 has the same task set and the
same steps", and "`"plan:E-07" in collections` returns the same tasks as the prototype board's E-07 epic".

**The generator** is the prototype board's script that makes the run order for a goal. Run it on the tree of `<check
copy>`, at the commit the line is on (`git -C "$COPY" rev-parse HEAD`), and save what it prints as the files named
below. Its output shape is yours; write each as ids without the `plan:` source (`T-NN-NNN`), sorted, one per line, and
steps one line each with the ids sorted and space separated. Collide's own order inside a step is by key.

1. Write the views and trust them. A view is read from the line's commit and is part of the trust hash, so it must be
   committed and trusted. The closure view has no `where`, so its members are the closure and nothing else.

```sh
cd "$COPY"
cat > .collide/views/closure-check.yaml <<EOF
name: closure-check
closure: { goal: plan:$GOAL, follow: depends_on }
EOF
cat > .collide/views/epic-07.yaml <<'EOF'
name: epic-07
where: '"plan:E-07" in collections'
EOF
git add .collide && git commit -q -m "check: views"
collide trust
```

   Pass for `trust`: a new hash; `Changes:` shows the two added view files. Answer `y`.

2. The closure. `collide tasks --view` prints members; `--json` carries `view.steps`.

```sh
collide tasks --view closure-check | members > "$CHECK/closure.members"
wc -l < "$CHECK/closure.members"
collide tasks --view closure-check --json | steps > "$CHECK/closure.steps"
cat "$CHECK/closure.steps"
collide tasks --view closure-check --json | jf view.cycle
```

   Compare with the generator's two files:

```sh
diff "$CHECK/closure.members" "$CHECK/generator.members" && echo "same set"
diff "$CHECK/closure.steps" "$CHECK/generator.steps" && echo "same steps"
```

   Pass: both "same". The rule built (`ts-0118-wp`, plan 0.10 Views): from the goal, along the links of `follow:
   depends_on`, through open tasks only (not `done`, not `dropped`), with the goal always in; steps are layers, the
   first holding tasks with no open dependency in the set. A difference is the closure rules of `ts-0118-wp` to
   correct: write which tasks differ and on which side. Differences you may meet, and what each means:
   - A task only a `done` task reaches is in the generator's set and not in Collide's: the generator walks through
     done tasks. Collide's rule leaves out R2's three stranded tasks, as the plan says.
   - A task reached by a `blocked_by` link is in Collide's set and not the generator's: `follow: depends_on` walks
     both `blocked-by` and `depends_on` (`ts-0118-wp`, decision "follow walks DEPENDENCY_LINKS", changed at its
     brief). Write it as a difference of that decision, not of the walk.
   - The two disagree on the order inside a step only: not a difference; write it and move on.

3. **Decision: a done goal is walked from.** As built, the goal is always in the set and its open dependencies are in
   too, even when the goal is `done`. The alternative is the goal alone. Make the goal done in the check copy, so the
   commit is the same for both, and run both again:

```sh
sed -i '' 's/^status: .*/status: done/' "$(file_of "$GOAL")"
git add -A && git commit -q -m "check: done goal"
sleep 5
collide tasks --view closure-check | members > "$CHECK/done-goal.members"
wc -l < "$CHECK/done-goal.members"
```

   Run the generator for the same goal at that commit and save `done-goal.generator.members`; then `diff
   "$CHECK/done-goal.members" "$CHECK/done-goal.generator.members"`. Settled by the generator:
   - The generator lists the goal and its open dependencies: **keep as built**. Write "done goal: as built".
   - The generator lists the goal alone, or nothing: the alternative is what the prototype does. Write "done goal:
     generator walks nothing" (or "the goal alone"), the task counts, and that `ts-0118-wp` decision 2 needs a fix
     item.
   - The generator refuses a done goal with an error: write its message. It treats a done goal as a mistake, which
     neither reading is; the owner picks.

   Undo the commit before the next sub-step: `git reset -q --hard HEAD~1`.

4. **Decision: a goal that matches nothing is a count on the result.** As built, a goal that names no task is not an
   error: the closure is empty, the other parts of the view still give members, and `view.goal_unmatched` names the
   key on the result. The alternative is an error and no members. Two views, committed and trusted: one with the
   closure alone, to compare with the generator, and one with a `where` beside it.

```sh
cat > .collide/views/goal-missing.yaml <<'EOF'
name: goal-missing
closure: { goal: plan:T-99-999, follow: depends_on }
EOF
cat > .collide/views/goal-missing-mixed.yaml <<'EOF'
name: goal-missing-mixed
where: '"plan:E-07" in collections'
closure: { goal: plan:T-99-999, follow: depends_on }
EOF
ls arch/proposals | grep '^99-'         # nothing: the goal names no task
git add .collide && git commit -q -m "check: missing goal"
collide trust
collide tasks --view goal-missing; echo "exit $?"
collide tasks --view goal-missing --json | jf view.goal_unmatched
collide tasks --view goal-missing --json | jf view.steps
collide tasks --view epic-07 | members > "$CHECK/epic-07.members"
collide tasks --view goal-missing-mixed | members | diff - "$CHECK/epic-07.members" && echo "mixed = epic"
collide tasks --view goal-missing-mixed --json | jf view.goal_unmatched
```

   Pass, Collide's side: `goal-missing` prints no task and exits 0, `goal_unmatched` is `"plan:T-99-999"`, `steps` is
   `[]`; the mixed view lists the same tasks as `epic-07` and carries the same `goal_unmatched`. The text form says
   nothing of the goal (`errors_line` in `host/src/task_daemon.rs` counts invalid items, evaluation errors and the
   step budget only), so the count is seen in `--json` only. Settled by the generator, run for `T-99-999`:
   - It prints an empty run order and exits 0: **keep as built**; write "missing goal: as built".
   - It stops with an error (a missing task): it treats a goal that names nothing as a failure. For a view that has a
     closure and nothing else, the alternative (an error, no members) is closer. Write its message and that
     `ts-0118-wp` decision 3 needs a fix item. The mixed view is Collide's rule alone: the generator cannot speak for
     it.

5. The collection view, against the epic and the generator's E-07:

```sh
collide tasks --view epic-07 | members > "$CHECK/epic-07.members"
wc -l < "$CHECK/epic-07.members"
diff "$CHECK/epic-07.members" "$CHECK/generator.epic-07.members" && echo "same as the generator's E-07"
sed -n '/BEGIN GENERATED tasks/,/END GENERATED/p' "$COPY"/arch/work/epics/E-07-*.md \
  | grep -o 'T-[0-9][0-9]-[0-9][0-9][0-9]' | sort -u \
  | diff - "$CHECK/epic-07.members" && echo "same as the epic's table"
```

   Pass: both "same". A task in the view and not the epic's table (or the reverse) is a difference: write it. The
   collection `plan:E-07` comes from each task's `epic:` field; a table row for a task with another `epic:` is the
   likeliest cause. If your epic is not `E-07`, change the id in `epic-07.yaml` and the globs.

6. Last, the item's own example view, with its `layout`, `workflow` and `profile` keys, on real data:

```sh
cp "$COLLIDE_SRC/fixtures/views/edge-mvp.yaml" .collide/views/mvp.yaml
git add .collide && git commit -q -m "check: the example view" && collide trust
collide tasks --view MVP --json | jf view
```

   Pass: no `error`; `unmatched_include` holds `github:acme/edge#212` (there is no GitHub source here, which is not an
   error); `unmatched_exclude` is empty if `plan:T-01-043` exists. The goal key inside is `plan:T-01-042`, so this
   needs that task; if you chose another goal, edit `closure.goal` in the copy first. `"mvp" in labels` matches
   nothing: a plan task has no labels (`ts-0110-ra`, Not done).

7. The two view exits (`ts-0120-wm`). A view with an error of its own lists no task and exits 10; an unknown view
   exits 3 and names the board's views.

```sh
cat > .collide/views/broken.yaml <<'EOF'
name: broken
where: 'status =='
EOF
git add .collide && git commit -q -m "check: a broken view" && collide trust
collide tasks --view broken; echo "exit $?"
collide tasks --view nosuch; echo "exit $?"
collide tasks --view closure-check > /dev/null; echo "exit $?"
```

   Pass: the first prints `collide: view `broken` has an error: ...` on standard error, no task on standard output,
   and `exit 10`. The second exits `3` and lists `closure-check`, `epic-07`, `goal-missing`, `goal-missing-mixed`,
   `broken` and `MVP`. The third exits `0`: one broken view does not spoil the others.

### 4. Writes

Goal: the three write outcomes through the daemon (`task.apply`), and the files each reports. Criteria: "`setStatus`
with an illegal transition comes back `refused`, with plan's message" and "A legal `setStatus` reports exactly the
task file and its regenerated epic table as touched, and `setStatus` with category `active` writes `doing`".

A write to a task key goes through the daemon and is applied by the provider, which runs the checkout's own `./plan`
(`./plan <command> ... --root <root>/arch --format json`; see `fixtures/packs/orchard/providers/plan/write.ts`). No
flag names the checkout, so the write goes to the board root, `<check copy>`. Commit after each sub-step so the next
`git status` starts clean: `git -C "$COPY" add -A && git -C "$COPY" commit -q -m "check: <what>"`. The daemon never
commits a write; `--commit` is not used here.

1. An illegal move, from `specified` to `done`. Then the same through the real plan CLI.

```sh
cd "$COPY"
collide task "plan:$T_ILL" setStatus status=done; echo "exit $?"
git status --short
./plan set "$T_ILL" status=done --root arch --format json --dry-run; echo "exit $?"
```

   Pass: the first prints `collide: refused: <message>` on standard error and `exit 5`; `git status --short` shows
   nothing; the file is as it was. The message is the plan CLI's own: put it beside the output of the third command
   (the shape of that output is a fixture comparison, step 8; here only the words count). If `specified` to `done` is
   legal in your lifecycle, pick another pair and say which in step 9.

2. A legal move, with the files it reports, and the same move's effect on disk.

```sh
collide task "plan:$T_LEG" setStatus status=doing --json | jf files
git status --short
git diff --stat
git diff -- arch/work | head -30
status_of "$T_LEG"
git add -A && git commit -q -m "check: legal write"
```

   Pass: `files` lists exactly two paths, the task file and the epic file whose generated table holds it, relative to
   the checkout; `git status --short` shows those two as modified and nothing else; the diff of the epic file changes
   the one table row, inside the generated region. `status_of` prints `status: doing`. If the task's epic has no
   table, one path is right: write it. A third file is a difference to write.

3. A move by category. A category names no status; `active` is the one status the plan source gives that category,
   `doing` (`fixtures/packs/orchard/providers/plan/declaration.ts`, `CATEGORY_STATUS`).

```sh
collide task "plan:$T_CAT" setStatus category=active --json | jf files
status_of "$T_CAT"
git add -A && git commit -q -m "check: category write"
```

   Pass: the same two paths in `files`, and `status: doing` in the file. Both statuses `doing` for `T_LEG` and `T_CAT`
   is the point: a category write and a status write end at the same value.

### 5. Queue and wait

Goal: a write into a checkout that is busy is stored and not applied, says why, and applies when the checkout is free.
Criteria: "A write into a checkout with a running phase is queued, not applied" and "A write to the board root during
a rebase waits, and the CLI says why. It applies once the rebase ends."

Use three terminals, each after `. <temp>/env.sh`: A runs the phase, B writes, C ends things. The write guard keeps a
`Conflict` item in the daemon's Queue with the title `A `setStatus` of plan:<id> waits: <reason>` (`write_guard.rs`,
`waiting_title`); the app has no view of it yet, so you see it through the CLI's message.

**5a. A live phase.** The only phase that can be live today is a `collide run`; its checkout is the run's new worktree
under `<temp>/root/data/workspaces/`, and a write to that checkout waits while the phase runs
(`LivePhases::blocking`). A phase that starts at the root does not block (`at_root`); nothing starts one before Stage
8.

1. Terminal A. First, the `claude` the daemon will start. It is looked up on the `PATH` of the login shell, and
   macOS's `path_helper` can put a system folder ahead of `<temp>/bin`.

```sh
"$SHELL" -l -i -c 'command -v claude' 2>/dev/null
```

   Pass: `<temp>/bin/claude`. If any other path is printed, **stop and skip 5a**: the run would start your real
   `claude`. Then a task file for the run, and the run. `--budget 0.01` is a backstop only; the stand-in costs
   nothing. **(agent run)** After its 120 second sleep the stand-in waits on its standard input and never ends by
   itself: sub-step 3's stop ends it.

```sh
cd "$COPY"
printf '# check phase\n' > check-phase.md
git add check-phase.md && git commit -q -m "check: phase file"
collide run check-phase.md --budget 0.01
```

   Pass: `claude in use: 2.1.325`, a `claude published:` line, then `run <id> started on <file>`. Note the run id.
   Leave A running. If the run says the version is not usable, or ends at once, the stand-in did not pass (see Set up,
   sub-step 5): write it in step 9 and go to 5b. **(agent run)** No `claude published:` line was printed
   (`ts-0150-dj`).

2. Terminal B. Find the run's worktree and write to it, without waiting.

```sh
cd "$COPY"
export RUNWT="$(git worktree list --porcelain | awk '/^worktree /{print $2}' \
  | grep '/workspaces/' | grep -v '/check-wt$')"
echo "$RUNWT"
collide task "plan:$T_Q1" setStatus status=doing --checkout "$RUNWT" --no-wait; echo "exit $?"
git -C "$RUNWT" status --short
```

   Pass: one worktree path, with a branch `collide/<run id>`. The write prints on standard error `collide: the write
   to plan:<T_Q1> is queued: a phase of run <id> is running in <path>; queue item write-<ms>-<n>` and `collide: the
   write stays queued (--no-wait); it is applied when the checkout is free`, and exits `11`. `git status --short` in
   the worktree shows nothing: **the write was not applied**.

3. Terminal C ends the phase (`collide stop <run>` stops a run, not the daemon). Then look at B's worktree.

```sh
collide stop <run id>
sleep 5
git -C "$RUNWT" status --short
status_of "$T_Q1" "$RUNWT"
```

   Pass: A exits `8` (stopped) with its record; `git status --short` in the worktree now shows two modified files, the
   task file and its epic table, and `status_of` prints `status: doing`: the queued write applied once the phase
   ended. Whether the worktree is still there after a stop, so the write has a place to land, is **(unconfirmed)**:
   the writer read `write_guard.rs` and `file_run.rs` and did not find which. If it is gone, a queue item titled `A
   ... was not applied: ...` is the other outcome (`write_guard.rs`); write what you see.

   **(agent run)** The worktree is kept. `collide stop` signalled no process (`ts-0143-ah`); the run ended when the
   stand-in's standard input closed. The queued write then failed: the run's worktree has no build of the plan CLI (step
   1). With `npm install` and `npm run build` made in that worktree while a second run's phase was live, the write
   queued and then applied.

**5b. A rebase at the root.** `root_busy` reads git's own state files: `rebase-merge`, `rebase-apply`, `MERGE_HEAD`,
`CHERRY_PICK_HEAD`, `REVERT_HEAD`, `BISECT_LOG` (`host/src/checkout.rs`). This makes a rebase stop on a conflict in
`<check copy>`, which is the board root.

1. Terminal C. Two commits that change one line, then a rebase that stops.

```sh
cd "$COPY"
LINE="$(git branch --show-current)"; echo "$LINE"
printf 'one\n' > check-conflict.txt && git add check-conflict.txt && git commit -q -m "check: base"
git switch -q -c check-side
printf 'two\n' > check-conflict.txt && git commit -q -am "check: side"
git switch -q "$LINE"
printf 'three\n' > check-conflict.txt && git commit -q -am "check: line"
git rebase check-side
git status | head -3
```

   Pass: git says there is a conflict in `check-conflict.txt` and `git status` says a rebase is in progress. The
   branch is detached while it lasts. Whether the daemon still finds the line then is **(unconfirmed)**: the writer
   read `root_busy`, not how the line is found. If the write is refused as "not trusted" or "no line" instead of
   waiting, write it in step 9, abort the rebase, and repeat with `git merge check-side` in place of the rebase
   (`MERGE_HEAD`). **(agent run)** Refused at once, exit 1, "the line `HEAD` is not trusted" (`ts-0146-se`);
   the merge passes.

2. Terminal B. A write to the root, which waits.

```sh
cd "$COPY"
collide task "plan:$T_Q2" setStatus status=doing
```

   Pass: standard error says `collide: the write to plan:<T_Q2> is queued: a rebase is in progress in <check copy>;
   queue item write-<ms>-<n>`, and the command does not return. The queue is polled every half second, for 10 minutes.

3. Terminal C. End the rebase.

```sh
git rebase --abort
git branch --show-current
git status --short
```

   Pass: B returns by itself within a few seconds with `ok  plan:<T_Q2>: the queued write was applied` and exit `0`.
   `git status --short` shows the two files of the write (the task and its epic table), and `status_of "$T_Q2"` prints
   `status: doing`. The branch is the line again. Commit: `git add -A && git commit -q -m "check: queued write"`.

### 6. Trust

Goal: a script or a view edited on a branch that is not the line is not used, and the same edit on the line asks
again. Criterion: "An edited provider script or view on a branch is not used, and a changed hash asks again."

The adapter's script is part of the repo pack, so its file is in the trust hash; a view is hashed with its blob id.
The hash covers the committed snapshot, not the working tree (`ts-0105-jv`, step 3). `<check worktree>` is on its own
branch, `check-wt`, so the edits go there.

1. Edit the script and a view on `check-wt`, and commit. The script edit writes a marker file when the provider
   starts, so you can see whether it ran. That the provider may write there is **(unconfirmed)**; if the marker never
   appears after step 3 either, read `<temp>/root/data/daemon.log` for the provider's start instead.
   **(agent run)** The marker works.

```sh
. "$CHECK/env.sh"
LINE="$(git -C "$COPY" branch --show-current)"
git -C "$WT" merge -q --no-edit "$LINE"      # the branch gets the line's views, so the edit below is a change
cd "$WT"
S=.collide/packs/reference/providers/plan.ts
{ printf "import { writeFileSync as mark } from 'node:fs'\nmark('%s/marker', 'ran')\n" "$CHECK"
  cat "$S"; } > "$CHECK/plan.ts.new"
cp "$CHECK/plan.ts.new" "$S"
cat > .collide/views/closure-check.yaml <<EOF
name: closure-check
exclude: [plan:$GOAL]
closure: { goal: plan:$GOAL, follow: depends_on }
EOF
git add .collide && git commit -q -m "check: edit the script and a view on a side branch"
```

   The merge is why the edit is a change on top of the line and cherry-picks cleanly in sub-step 3. The first line of
   the script must not be a `#!` line; the fixture's `plan.ts` begins with comments and an import.

2. From `<check copy>`, whose line is not that branch: neither edit is used.

```sh
cd "$COPY"
collide tasks --view closure-check | members | diff - "$CHECK/closure.members" && echo "view unchanged"
ls "$CHECK/marker"
collide trust --line check-wt
collide trust
```

   Pass: "view unchanged" (the goal is still in the set; the branch's `exclude` is not applied) and `ls` says there is
   no `marker`. `collide trust --line check-wt` shows a hash that is not the line's, and `Changes:` with the
   edited `providers/plan.ts` and the view in it. Since that branch was never trusted, they show as added files
   and not as changed lines (agent run). Answer `n`: "Nothing was trusted." The plain `collide trust` after it says
   the line is already trusted at its own hash.

3. The same two edits on the line. Cherry-pick the commit; it is a changed hash, and nothing is trusted yet.

```sh
git cherry-pick check-wt
collide tasks --view closure-check; echo "exit $?"
ls "$CHECK/marker"
collide trust
collide tasks --view closure-check | members | diff - "$CHECK/closure.members"
sleep 3
ls "$CHECK/marker"
```

   Pass, before `trust`: the command does not list the view's members as before; it says the line is not trusted, or
   that the view is unknown (write which words, and the exit code: the text is `ts-0113-ck`'s and `ts-0119-tj`'s,
   which the writer did not run). No `marker`. Then `trust` shows the hash that changed and the files with their changed
   lines (`providers/plan.ts`, the view);
   answer `y`. After it, the `diff` shows the goal gone from the set, and a `marker` file exists: the edited script
   ran. Undo for the steps after: `git reset -q --hard HEAD~1` and `collide trust` (`y`), so the line is as at step 5.

### 7. Scratch

Goal: `collide task new` makes a task that belongs to one board and is listed for that board only. Criterion: "A
scratch task created with `collide task new` is listed for that board only." A scratch task has no checkout, so
`--checkout` never lists it (`ts-0120-wm`, Not done: "The scratch tasks are not in `--checkout`"); a view does. Both
boards use the one daemon.

```sh
cd "$SCRATCH"
git init -q -b main && printf '# scratch\n' > README.md && git add -A && git commit -q -m "first commit"
collide init
mkdir -p .collide/views
cat > .collide/views/scratch-only.yaml <<'EOF'
name: scratch-only
where: 'source == "scratch"'
EOF
git add .collide && git commit -q -m "check: view"
collide trust
collide task new "scratch-beta"
collide tasks --view scratch-only
cd "$COPY"
cp "$SCRATCH/.collide/views/scratch-only.yaml" .collide/views/scratch-only.yaml
git add .collide && git commit -q -m "check: scratch view"
collide trust
collide task new "scratch-alpha"
collide tasks --view scratch-only
cd "$SCRATCH" && collide tasks --view scratch-only
```

Pass: `task new` prints a key such as `scratch:1`. In `<scratch repo>` the first listing shows `scratch-beta` only; in
`<check copy>`, `scratch-alpha` only; the last listing shows `scratch-beta` only, with no `plan:` task. Zero packs in
`<scratch repo>` is enough for the scratch source (`ts-0120-wm`'s tests); that it is enough on a live daemon is
**(unconfirmed)**.

### 8. Fixtures

Goal: the fixtures match the real thing, or each difference is written down. Criterion: "The fixtures match the real
`arch/` shapes and the real plan CLI, or each difference is written down." Put the real file or output beside the
fixture file; **write each difference in step 9**, with the real text beside the fixture's. Each becomes a fix item.

**Files.** The fixture tree is `fixtures/orchard-arch/basic/arch/` (and `fixtures/orchard-arch/hazards/arch/`, awkward
files the adapter must survive: conflict markers, bad YAML, a duplicate key, twin files of one id). Every value in it
is made up.

| Real, in `<check copy>/arch/` | Fixture, in `<collide checkout>/fixtures/orchard-arch/basic/arch/` |
|---|---|
| a task file, `proposals/NN-slug/tasks/T-NN-NNN-slug.md` | `proposals/03-harbor-index/tasks/T-03-001-*.md` |
| an epic file, `work/epics/E-NN-slug.md`, and its generated region | `work/epics/E-07-harbor-plan.md` |
| `ID-MAP.md` at the root of `arch/` | `ID-MAP.md` |
| a `PROPOSAL.md`; a question under `questions/` | `proposals/03-harbor-index/PROPOSAL.md`, `questions/` |

```sh
cd "$COPY"
f="$(file_of "$T_LEG")"; head -14 "$f"
head -14 "$COLLIDE_SRC/fixtures/orchard-arch/basic/arch/proposals/03-harbor-index/tasks/T-03-001-survey-the-quay.md"
sed -n '1,30p' arch/work/epics/E-07-*.md
sed -n '1,30p' "$COLLIDE_SRC/fixtures/orchard-arch/basic/arch/work/epics/E-07-harbor-plan.md"
head -12 arch/ID-MAP.md
head -12 "$COLLIDE_SRC/fixtures/orchard-arch/basic/arch/ID-MAP.md"
ls arch arch/proposals | head -20
```

Look at: the frontmatter keys of a task (the fixture has `title`, `status`, `epic`, `reviewed_by`, `depends_on`,
`blocked_by`, and flow lists `[]`; whether the real ones use block lists, other keys, or `epic:` as a bare id or an id
with a slug); the file and folder name patterns; the epic's `<!-- BEGIN GENERATED tasks -->` region (header `| ID |
Status |`, links as `[T-NN-NNN](../../proposals/...)`); the `ID-MAP.md` columns (`old id | new id | renamed |
detail`); and any folder or file kind under `arch/` the fixture lacks.

**The plan CLI.** The fake is `fixtures/orchard-arch/plan`, a node script. Its header says its shapes were read in
your copy: the JSON envelope (`schema_version` 1, `command`, `corpus`, `data`), the exit codes (0 done, 1 the write
gate refused it, 2 it would not do what it was asked), and what `--dry-run` reports. Run the real and the fake side by
side; `--dry-run` writes nothing in either. The fake takes the fixture's own ids: `T-03-003` is `ready`, `T-05-002` is
`specified`.

```sh
cd "$COPY"
FAKE="$COLLIDE_SRC/fixtures/orchard-arch/plan"; FARCH="$COLLIDE_SRC/fixtures/orchard-arch/basic/arch"
./plan set "$T_DRY" status=doing --root arch --format json --dry-run > "$CHECK/real-set.json"; echo "real set: exit $?"
"$FAKE" set T-03-003 status=doing --root "$FARCH" --format json --dry-run \
  > "$CHECK/fake-set.json"; echo "fake set: exit $?"
./plan set "$T_ILL" status=done --root arch --format json --dry-run > "$CHECK/real-bad.json" 2> "$CHECK/real-bad.err"
echo "real illegal: exit $?"
"$FAKE" set T-05-002 status=done --root "$FARCH" --format json --dry-run \
  > "$CHECK/fake-bad.json" 2> "$CHECK/fake-bad.err"; echo "fake illegal: exit $?"
./plan set "$T_DRY" status=doing --root arch --dry-run; echo "real text: exit $?"
git status --short
cat "$CHECK/real-set.json"; cat "$CHECK/fake-set.json"
cat "$CHECK/real-bad.json" "$CHECK/real-bad.err"; cat "$CHECK/fake-bad.json" "$CHECK/fake-bad.err"
```

Look at, and write down:
- The envelope keys: `schema_version`, `command`, `corpus`, `data`, and in `data` for a set: `results[]` with
  `relPath`, `changes[]` (`field`, `oldValue`, `newValue`) and `written`, then `dryRun`, `errors`, `regenerated`. The
  adapter reads `schema_version` 1, a `command` that matches, a `data` object, and files under `arch/` (`write.ts`); a
  real envelope that differs there fails every write.
- The paths: relative to `arch/` or to the repo, and `regenerated` holding the epic file.
- An illegal move: the fake exits 2 with a plain message on standard error and nothing on standard output. The real
  one's exit code, its message, and whether it prints an envelope with `errors` and exits 1 (`write.ts` says what the
  adapter makes of each; the words in step 4, sub-step 1 are what you saw through Collide).
- `git status --short` prints nothing: a `--dry-run` writes no file (the adapter refuses a dry run that did).
- A flag the adapter sends and the real CLI rejects: `--root`, `--format json`, `--dry-run`, with the command first
  (`write.ts`, where `./plan` is run with the command first; the fake refuses a flag before the command, as oclif
  does).

One more, the create (it is what `create` runs; the fake follows the real flags as read). `NN` is the number of any
proposal folder; `E-07` is the epic.

```sh
./plan new task NN "check title" --epic E-07 --root arch --format json --dry-run; echo "exit $?"
"$FAKE" new task 03 "check title" --epic E-07-harbor-plan --root "$FARCH" --format json --dry-run; echo "exit $?"
git status --short
```

Compare the two envelopes: `data.relPath`, `id` (`kind`, `num`, `suffix`, `scope`, `slug`), `frontmatter`, `written`,
`dryRun`, `errors`, `regenerated`. The fake leaves `ID-MAP.md` ids reserved (a number named there is not reused); the
real one's rule is the one to write if it differs.

### 9. Record

Lines marked **agent run** are the results of the run the owner asked an agent to make (2026-11-19), on a temp clone of
the owner's copy. What was awkward and the date Stage 4b is done are the owner's.

- Date and time (agent run): 2026-11-19, 09:58 to 10:25 UTC, 27 minutes at an agent's pace.
- Time per step (agent run): 1 Set up 5 min, 2 Read side 2, 3 Views 4, 4 Writes 1, 5 Queue 9 (with the stop's diagnosis
  and a second run), 6 Trust 1, 7 Scratch under 1, 8 Fixtures 4, Clean up 1.
- First `collide tasks --checkout` on the real tree (agent run): 5.7 s for 1,659 tasks. After a daemon restart: 5.5 s,
  then 0.5 s each (`ts-0150-dj`).
- The picks (agent run): `T-01-042` and `E-07` are not in the copy. The goal (`<goal>` below) was a task with 13 open
  dependencies, and the epic `<epic>` one with a generated table; the writes used `E-73`'s tasks.
- Result per step (agent run):
  - 1 Set up: pass. The plan CLI needed `npm install` and `npm run build` in the clone; the temp `HOME` was enough.
  - 2 Read side: pass. 1,659 lines for 1,659 task files, none skipped, `refreshing` false. At about 20 s, two copies
    (`ready` at the root, `doing` in the worktree) and `divergent` `["plan:T-36-044"]`, the listings `doing` and
    `ready`. A bare `collide task <goal>` is refused (`ts-0149-kq`); the text of `task <key>` has a blank `kind` line
    (`ts-0150-dj`).
  - 3 Views: closure for `<goal>`: 14 tasks, the generator's 14 (its 13 open and the goal), the same 5 steps, no cycle:
    pass. `<epic>`: fail, Collide 452, the epic's table 428, the board page 645 (`ts-0148-cv`). Exits 10 and 3, and the
    example view: pass. Not in the checklist: `collide tasks --ready` lists nothing and names 1,022 blockers as not
    items (`ts-0144-eh`).
  - 4 Writes: pass. Illegal: exit 5, `refused:` with the plan CLI's message word for word (the plan CLI itself exits 2),
    nothing written. Legal: the task file and the `E-73` epic file, one row of its generated table, `doing`.
    `category=active`: the same two files, `doing`.
  - 5 Queue: behind a live phase: queued (exit 11, both lines, the worktree clean), and applied once the phase ended,
    after the plan CLI was built in the worktree: pass. `collide stop` signalled no process (`ts-0143-ah`). During a
    rebase: fail, refused at once (`ts-0146-se`). With `git merge` instead: pass, applied 8 s after `--abort`, exit 0.
  - 6 Trust: pass. The branch's edits were not used and no marker was written; `trust --line check-wt` showed another
    hash, every file as added. On the line, before trust: exit 10, "view `closure-check` has an error: the line is not
    trusted, so this view is not read", no marker. After trust the goal left the set and the marker was written. The
    next listing still warned the line was not trusted (`ts-0150-dj`).
  - 7 Scratch: pass. `scratch:1` on each board; each lists its own only, with no `plan:` task.
  - 8 Fixtures: the differences below.
- Decision, a done goal is walked from (`ts-0118-wp`) (agent run): the alternative. The generator walks nothing (no open
  run; the 13 are listed as stranded); Collide gives 14 (`ts-0147-ve`).
- Decision, a goal that matches nothing is a count (agent run): the alternative, for a closure-only view. Collide's side
  passes (no task, exit 0, `goal_unmatched`, the mixed view equals the epic's). The generator stops, exit 2: no task
  file carries that id (`ts-0147-ve`).
- Differences found (agent run):
  - Create: the real `data.id` is a string, the full id with its slug; the fake's is an object (`kind`, `num`, `suffix`,
    `scope`, `slug`). Every create through the adapter on the real CLI fails (`ts-0145-cq`). The CLI's `task create`
    takes only the Collide plan's arguments (`ts-0149-kq`).
  - An illegal move: both exit 2 with nothing on standard output. The real message names the transition and the legal
    moves, on two lines; the fake's is one line in other words (`ts-0150-dj`).
  - Task frontmatter: the fixture's keys. `title`, `status`, `epic`, `depends_on` and `blocked_by` are in every file;
    `reviewed_by`, `self_review`, `backlog_id`, `migrated`, `awaiting` and `clears` in some. `epic:` is a bare id in
    1,129 files and an id with its slug in 530; `depends_on` is a block list in 471. The fixture has each shape: no
    item.
  - The epic file and its generated region: the same shape. `ID-MAP.md`: the fourth column has another name; the adapter
    reads the first two cells. `arch/` and its proposal folders hold more kinds of folder and file than the fixture; the
    adapter ignores them.
  - A set's envelope: the same keys; paths relative to `arch/`; `regenerated` holds the epic; a dry run writes nothing.
- What was awkward: ____ (owner)
- The unconfirmed lines (agent run): the setup `PATH`, right (in the login shell only `<temp>/bin` holds `claude`). The
  stand-in passes the version check but never ends by itself. The worktree after a stop is kept. The detached head
  during a rebase is wrong: the write is refused. The marker file works. `./plan` in a fresh clone needs `npm install`
  and `npm run build`. A never-trusted branch's review shows every file as added.
- Installs (agent run): the plan CLI's packages and build in the clone and in the second run's worktree, and the
  generator's packages (`--ignore-scripts`) in the clone. All were inside `<temp>` and went with it.
- Date the owner judges Stage 4b done: ____ (owner). Two criteria failed in the agent run: the epic's set (`ts-0148-cv`)
  and the write during a rebase (`ts-0146-se`).

### Clean up

Do the steps in this order. Nothing here uses a glob with `rm`. In a new terminal `CHECK` is unset: take the folder
name from the `echo` in step 1 and `export CHECK=<temp>` before anything below.

1. Stop any run still going, first: `. "$CHECK/env.sh"`, then `collide stop <run id>` (step 5a). The stand-in does not
   end by itself; check `ps -ax | grep fake_claude` shows nothing from this check.
2. Stop the daemon this check started. Its log has its pid; check the row says `collide-daemon` before you signal it.
   There is no `collide-daemon stop`; `collide stop` stops a run. Do not signal a daemon that is not this one.

```sh
. "$CHECK/env.sh"
grep 'starting, pid' "$CHECK/root/data/daemon.log" | tail -1
ps -p <pid> -o pid=,command=     # a collide-daemon, with the temp root in its environment
kill -TERM <pid>
```

3. Remove the temp folder. It holds `<check copy>`, `<check worktree>`, the run's worktree, the cache and the
   stand-in. Collide's cache holds read-only copies of the SDK and the packs, so make the tree writable first. The
   guard refuses any path that is not this check's.

```sh
case "$CHECK" in /tmp/collide-check.*) chmod -R u+w "$CHECK" && rm -rf "$CHECK" ;; esac
```

   `<orchard copy>` was only read, by `git clone`, and is as you left it. Nothing was written under your real
   `~/.claude`, `~/.config` or keychain; the build in step 1 wrote only to `<collide checkout>`'s `target/` and
   `node_modules/` and the cargo and pnpm caches.

### Known gaps

What you will meet, and the item it is carried to or from. None of it is a failure of this check.

- **`ts-0108-dq`.** A provider that exits while idle is noticed at the next call, not at once.
- **`ts-0109-qj`.** The staged SDK has no `.d.ts` files, so `tsc` in a pack sees no types for
  `@collide/pack-sdk/yaml`.
- **`ts-0110-ra`.** A plan task has no `labels`, `owners` or `checklist`, so `"mvp" in labels` matches nothing.
- **`ts-0111-vn`, `ts-0112-px`.** The fake lacks the real CLI's `--incoming` and other id kinds. `watch` without
  recursive `fs.watch` relies on a recheck alone.
- **`ts-0113-ck`.** The daemon does not commit (`commit: true` is refused); the CLI's `--commit` commits what a write
  reported (`ts-0120-wm`). Board-id reuse and the main-checkout root are Stage 6a's.
- **`ts-0114-hr`.** No phase starts at the root of a line before Stage 8, so a phase that does not block (`at_root`)
  is tested with a fake registration only. A queued write has no view in the app; its title says why. A shim adopted
  with no run context is not a live phase.
- **`ts-0115-ug`, `ts-0116-wh`.** Feature links are Stage 4c's. The time for 1,600 tasks is recorded, not asserted;
  yours is the real figure.
- **`ts-0117-gu`, `ts-0118-wp`.** `layout` defaults, drawn boards and `queue: true` lanes are Stage 6's and 4c's. A
  second `follow` variant is 4c's; only `depends_on` exists.
- **`ts-0119-tj`.** The plan has no CLI to list or save views; the app's Save as View is the only writer. A repo view
  is read again on every call; only the evaluation is kept.
- **`ts-0120-wm`.** The text form of a view does not show `goal_unmatched`, `unmatched_include` or `unmatched_exclude`;
  `--json` does. Since `ts-0136-rr` every task command goes through the daemon.
- **Here.** No fixture workflow phase runs `sleep`; step 5a uses a stand-in `claude` (see the start). The fixture
  pack's own `pack.yaml` is not loaded; the minimal manifest is. `fixtures/orchard-arch/` and the fake plan CLI are
  made up; their header says they follow the shapes read in your copy.

### Automated tests this check leans on

The item's Tests section, by item id. None of them starts a real `claude` or touches a real home, launchd or daemon.

- `ts-0108-dq`, `ts-0109-qj`: the supervisor against a fake provider, and the SDK's hooks.
- `ts-0110-ra`, `ts-0111-vn`, `ts-0112-px`: the adapter against the fixture `arch/` trees and the fake plan CLI, in
  one checkout and two (rename, collision, watch).
- `ts-0113-ck`, `ts-0114-hr`: the daemon's sources and trust gate, and the write guards (root busy, the phase queue).
- `ts-0115-ug`, `ts-0116-wh`: the corpus, its cache and its events.
- `ts-0117-gu`, `ts-0118-wp`, `ts-0119-tj`: CEL, the views, and the views in the daemon.
- `ts-0120-wm`: the CLI.

What this check adds that no test has: the real `arch/` tree, the real plan CLI, the real task count, and a live
daemon with a real git worktree and rebase.
