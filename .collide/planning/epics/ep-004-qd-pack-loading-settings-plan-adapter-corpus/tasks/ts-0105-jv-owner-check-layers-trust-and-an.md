---
id: ts-0105-jv
kind: task
status: active
title: "Owner check: layers, trust and an agent entry read back"
owners: []
links:
  - { type: blocked-by, to: ts-0104-gx }
  - { type: blocked-by, to: ts-0107-rj }
---

## Goal

The Stage 4a checks that only the owner runs, on the real setup: an agent entry written by the owner and read back with
its layers, a trust prompt that shows a diff, a secret in the real keychain, a `git:` pack, and the repository's own
instructions under the real `claude`.

A build agent never runs it. It never touches the owner's real `~/.config/collide`, keychain or `claude` login, never
starts a daemon against the owner's data, and never runs `claude`. It writes the command lines and a short checklist
under `### Owner checklist`, from the code and from the Built sections of `ts-0093-am` to `ts-0104-gx`. The owner runs
them and records the result here as owner judgement, with what was awkward.

## Success criteria

- The owner runs `collide config get` on an agent entry they wrote and reads the layers back (4a criterion, "recorded as
  owner judgement in the stage's task item"): each field says the built-in layer, `user` or `repo`, and the secret is
  not shown. Owner-run.
- The owner sees one trust prompt end to end, on a repository made for the check: `collide init`, a commit, `collide
  trust`, then a committed change to a repo pack and a second prompt that shows the diff. Owner-run.
- The owner sets one secret into the real keychain with `collide config set --secret`, sees `collide config get` report
  it without the value, and removes the item afterwards. Owner-run, and it is the only place the real keychain is
  touched.
- The owner adds one `git:` pack from a local repository and one from a host the owner picks, sees the new-host question
  once, and runs `collide pack update`. Owner-run.
- The owner runs one real prompt in a worktree whose `CLAUDE.md` holds a canary, with the default role and with
  `repo_instructions: none`, and records which loaded (the line `ts-0103-sa` chose). Real `claude`, owner's login, small
  spend; owner-run.
- The checklist holds the command lines, with `<collide checkout>`, `<scratch repo>` and `<pack repo>` as placeholders,
  what to look at, how to clean up, and a line for the owner's result and one for what was awkward.
- A line records the date Stage 4a is judged done by the owner.

## Tests

None by a build agent: this check is the owner's. The checklist names the automated tests it relies on:

- `ts-0096-ph`: the board loader and the snapshot-not-workspace test;
- `ts-0097-kw`: the trust hash, the store and the diff;
- `ts-0098-nx`, `ts-0099-bw`: the layers, the agent entries and the fake keychain;
- `ts-0100-pu`, `ts-0101-bd`: the fetch rules against fixture remotes, user packs and working copies;
- `ts-0102-vu`: the lines, `collide init`, `collide trust` and the disk floor;
- `ts-0103-sa`, `ts-0104-gx`: the measured launch line and the role options, against the fake.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 4a: Success Criteria (the owner's `collide config get` on an agent entry); Tests.
  - 0.7: Settings; Agent entries; Secrets are names. 0.9: Trust. 0.3: The repository's own instructions.
  - Part L.
- What the checklist covers. The build agent fills in the exact commands once the earlier items are built.
  - **Set up.** Build the CLI and the daemon from `<collide checkout>`. Make `<scratch repo>`: a new git repository with
    one commit, and `<pack repo>`: a bare repository that holds a small pack (the fixture `fixtures/packs/pr-ci`'s files
    will do). Use a fresh `HOME` for everything except the keychain step and the real-`claude` step, so the owner's own
    settings stay as they are. Say in the checklist which steps use the real `HOME`.
  - **Agent entry.** Write an entry in `~/.config/collide/config.yaml` (a copy of the example in 0.7 with one change),
    run `collide config get agents.<id>.<field>` for each field, and with a repo file that sets `budget_usd` and
    `timeout_s`. Read the layers back.
  - **Trust.** `collide init`, commit, `collide trust`; change the pack in a commit; `collide trust` again and read the
    diff. Answer `n` once, to see that nothing is trusted.
  - **Secret.** `printf %s '<a made-up value>' | collide config set --secret <pack>.<key>`; `collide config get`; remove
    the item with the `security` command the checklist gives; check the value is in no file under the fresh `HOME`.
  - **`git:` pack.** `collide pack add git:<pack repo>//pack@main`, the review, `y`; move the remote's branch; see
    nothing changes; `collide pack update`; `collide pack list`. Then once with a real host.
  - **Instructions.** A worktree of `<scratch repo>` with a `CLAUDE.md` that says to start the reply with a made-up
    word; run `collide run` with the default role and again with a role that sets `repo_instructions: none`; look at the
    reply and the run's warning line after editing `CLAUDE.md` in the worktree.
  - **Record.** The result, the time it took, the spend, and what was awkward.
- **Decision for the owner, with a recommendation: how much real spend.** The real-`claude` step is one or two short
  prompts on haiku. Recommend the owner sets the cap in the checklist (a figure of `$0.30`), as `ts-0070-mk` did with
  its cap.
- Area: this item's own text. No code.
- The test rule, for anything automated this check leans on: no test touches the real home, real launchd or a real
  daemon. Each test clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as
  `host/tests/common/mod.rs` `TempRoot` does. Settings files are fixtures, never the owner's own `settings.json`. The
  owner's own run here uses the owner's real setup by choice, where the checklist says so.
- Stage 4a item (revision 23 scope, split 2026-11-16). Items of Stage 4a run one at a time, in id order, because
  they share `core`, `host` and the generated files.
- Owner decisions (2026-11-16, at the split): the recommendation above is taken under the owner's delegation; it
  stays overridable until the item is built.
- Owner decisions (2026-11-17, after `ts-0107-rj`): the default line now carries the skills plugin, and a skill's own
  `hooks` and `allowed-tools` take effect (accepted by the owner). The Instructions step adds a skill in the
  worktree's `.claude/skills/` and checks that it is listed as `repo-skills:<name>`, that its use asks for permission
  first, and that the warning names it after an edit.

### Owner checklist

Written from the code and the Built, Decisions and Not done sections of `ts-0093-am` to `ts-0104-gx`, `ts-0106-nr` and
`ts-0107-rj`. Every command below is the owner's to run: **not run by the writer**, and none was tried against a real
daemon, keychain or `claude`. A command or claim marked "(unconfirmed)" could not be confirmed from the code; the note
says what was looked for. Run it after the checks of `ts-0071-fd` and `ts-0083-uf`, so a real run has been seen
before.

**Three modes, and what each touches.** Setup defines one shell function for each. Use one terminal throughout.

- `isolated`: `HOME` is `<temp>/home`, `COLLIDE_DAEMON_ROOT` is `<temp>/root`. Used by steps 2, 3 and 5. Your own
  settings, packs, trust and daemon files stay as they are.
- `real_keychain`: your own `HOME`, `COLLIDE_DAEMON_ROOT` unset. Used by step 4 only. A daemon under a root override
  keeps secrets in memory (`secret_store_for_root` in `host/src/secret_store.rs`), so only a daemon with no override
  reaches the keychain. It runs under launchd and keeps its files in your `Library/Application Support/Collide`.
- `real_login`: your own `HOME`, `COLLIDE_DAEMON_ROOT` is `<temp>/root-claude`. Used by step 6 only. `claude`'s login is
  in your own home. The temp root keeps Collide's runs, worktrees, store and log in the temp folder. Collide still
  reads your own `~/.config/collide` (`models.yaml`), and `claude` writes under `~/.claude/projects`.

The keychain is in memory in the other two modes, so steps 2, 3, 5 and 6 never touch it.

**Spend.** Only step 6 starts `claude`. **The cap is yours to set.** The recommendation is **$0.30** in all: two runs of
`--budget 0.15`, on haiku. Change the figure below if you want another. The spend lands on whichever credential your
`claude` uses. How far past `--budget` a last turn may go is (unconfirmed): `ts-0083-uf` records that a prompt run has
no cap, and `--budget` is the file run's `--max-budget-usd`; watch the `total` line of the record.

### 1. Set up

1. Build from `<collide checkout>`. `collide` starts the `collide-daemon` that sits beside it, so use both from one
   folder. The loader finds the built-in packs and the SDK in the folder `pnpm build:packs` stages
   (`host/src/pack_location.rs`), found from the executable.

   ```sh
   cd <collide checkout>
   pnpm install                      # only if dependencies changed
   pnpm build:packs
   cargo build -p collide-host       # target/debug/collide and target/debug/collide-daemon
   export COLLIDE_BIN=<collide checkout>/target/debug
   ls "$COLLIDE_BIN/collide" "$COLLIDE_BIN/collide-daemon"
   ```

2. Make the temp folder, the three modes and the two repositories. `$SCRATCH` is `<scratch repo>` and `$PACKREPO` is
   `<pack repo>`. The short `/tmp` path keeps the daemon's socket path under its limit of 103 bytes
   (`MAX_SOCKET_PATH_BYTES` in `host/src/daemon.rs`). Git gets its own identity because the fresh `HOME` has none.

   ```sh
   collide() { "$COLLIDE_BIN/collide" "$@"; }
   export REAL_HOME="$HOME"
   export CHECK="$(mktemp -d /tmp/collide-check.XXXXXX)"
   export SCRATCH="$CHECK/scratch" PACKREPO="$CHECK/pack-repo"
   mkdir -p "$CHECK/home" "$SCRATCH" "$PACKREPO"
   export GIT_AUTHOR_NAME=owner-check GIT_AUTHOR_EMAIL=owner-check@example.invalid
   export GIT_COMMITTER_NAME=owner-check GIT_COMMITTER_EMAIL=owner-check@example.invalid
   isolated()      { export HOME="$CHECK/home" COLLIDE_DAEMON_ROOT="$CHECK/root"; }
   real_keychain() { export HOME="$REAL_HOME"; unset COLLIDE_DAEMON_ROOT; }
   real_login()    { export HOME="$REAL_HOME" COLLIDE_DAEMON_ROOT="$CHECK/root-claude"; }

   isolated
   cd "$SCRATCH" && git init -q -b main && printf '# scratch\n' > README.md
   git add -A && git commit -q -m "first commit"
   ```

   The environment variable names are from `host/src/daemon.rs` (`ROOT_ENV`) and `host/src/launch.rs`
   (`FORWARDED_ENV`: `COLLIDE_DAEMON_ROOT`, `HOME`, `SHELL`). Under `isolated` the files are `$CHECK/root/data/` (log,
   store, runs, workspaces, cache) and `$CHECK/home/.config/collide/` (settings, trust, hosts, packs).

3. Every daemon in this check is started by the first `collide` command that needs one, and stays up after it. Cleanup
   at the end stops each by its pid, which its own log holds.

### 2. Agent entry and its layers (`isolated`)

Goal: `collide config get` on an entry you wrote reads each field back with its layer, and a secret is not shown.
The layers are `default` (the built-in entry), `user` and `repo` (`ts-0098-nx`, `ts-0099-bw`). The first `collide`
command starts the daemon and may take several seconds.

1. Write the user file. It has no comment on purpose: `collide config set` does not rewrite a file that holds one
   (`ts-0099-bw`, decision 7).

   ```sh
   isolated
   mkdir -p "$HOME/.config/collide"
   cat > "$HOME/.config/collide/config.yaml" <<'EOF'
   agents:
     claude-haiku:
       model: sonnet
       timeout_s: 240
     my-acp:
       kind: acp
       command: opencode
       args: [acp]
       model: "ollama/qwen2.5:7b"
   ask: { agent: claude-haiku }
   EOF
   ```

2. Read every field of the built-in entry, then the other entry, then everything. A line is `key = value  (layer)`;
   a value is JSON, so a string is quoted. Field order is `AGENT_KEYS` in `core/src/settings.rs`.

   ```sh
   cd "$SCRATCH"
   for f in kind command args model config read tools timeout_s budget_usd env secrets; do
     collide config get "agents.claude-haiku.$f"
   done
   collide config get agents.my-acp
   collide config get
   ```

   Pass, for `claude-haiku`: `kind` `"claude-code"`, `config` `"none"`, `read` `["<board root>"]`, `tools`
   `["read","search"]` all `(default)`; `model` `"sonnet"` and `timeout_s` `240` `(user)`; `command`, `args`,
   `budget_usd`, `env` and `secrets` `<not set>` `(default)`. For `my-acp`: `kind`, `command`, `args` and `model`
   `(user)`; `timeout_s` `180` `(default)`. `ask.agent` reads `"claude-haiku"` `(user)`. Outside a checkout the
   command says it shows the user's layers only, on standard error.

3. Add the repo layer: a team's defaults in a committed `.collide/config.yaml`. `collide init` also writes
   `.collide/board.yaml`, which step 3 uses. The repo layer is the committed `HEAD`, never the working tree
   (`ts-0098-nx`, decision 2).

   ```sh
   collide init
   cat > .collide/config.yaml <<'EOF'
   ask: { budget_usd: 0.5, timeout_s: 120 }
   EOF
   git add .collide && git commit -q -m "board file and team defaults"
   collide config get agents.claude-haiku.budget_usd
   collide config get agents.claude-haiku.timeout_s
   collide config get agents.my-acp.timeout_s
   collide config get agents.my-acp.budget_usd
   ```

   Pass: `claude-haiku` `budget_usd` `0.5` `(repo)`. `claude-haiku` `timeout_s` stays `240` `(user)`: an entry's own
   value beats the repo's default. `my-acp` `timeout_s` is `120` `(repo)`. `my-acp` `budget_usd` is `<not set>`
   `(default)`: an ACP agent cannot enforce a budget, so the repo's cap is not applied
   (`agents.<id>.budget_usd` in `core/src/agent_layers.rs`). The notice that says so is on the wire and is **not
   printed by `collide config get`** (it prints entries only, `host/src/config_cli.rs`): note it under "awkward".

4. The repo layer is the commit, not the file. A refusal names its key.

   ```sh
   printf 'ask: { budget_usd: 0.9 }\n' > .collide/config.yaml
   collide config get agents.claude-haiku.budget_usd          # still 0.5 (repo)
   git checkout -- .collide/config.yaml
   printf 'agents:\n  claude-haiku:\n    model: opus\n' >> .collide/config.yaml
   git add .collide && git commit -q -m "bad repo file"
   collide config get agents.claude-haiku.model; echo "exit $?"
   git revert --no-edit HEAD
   collide config get agents.claude-haiku.model
   ```

   Pass: the first read still says `0.5`. The second prints "the settings have problems:" and a line that names the
   `agents` key, exits 1, and prints nothing on standard output (the repo file may not set an agent field,
   `repo_file_refusals`). After the revert the read works again.

5. A secret by name, with the in-memory keychain of this mode. An entry holds the name of a keychain item, never a
   value. A missing item fails every `config get`, by design (`ts-0099-bw`, decision 2).

   ```sh
   cat > "$HOME/.config/collide/config.yaml" <<'EOF'
   agents:
     claude-haiku:
       model: sonnet
       timeout_s: 240
     my-acp:
       kind: acp
       command: opencode
       args: [acp]
       model: "ollama/qwen2.5:7b"
       secrets: { OWL_API_KEY: collide-owner-check-owl }
   ask: { agent: claude-haiku }
   EOF
   collide config get agents.my-acp; echo "exit $?"
   printf %s 'owl-made-up-value-0001' | collide config set --secret agents.my-acp.secrets.OWL_API_KEY
   collide config get agents.my-acp.secrets
   grep -rlF 'owl-made-up-value-0001' "$CHECK"; echo "grep exit $?"
   ```

   Pass: the first read fails, exit 1, with "the keychain has no item `collide-owner-check-owl`; set it with `collide
   config set --secret agents.my-acp.secrets.OWL_API_KEY`". The `set` prints `agents.my-acp.secrets.OWL_API_KEY =
   <secret, in the keychain>  (user)`. The read then works and shows `{"OWL_API_KEY":"collide-owner-check-owl"}`
   `(user)`: the item's name, not the value. `grep` prints no file and exits 1. The value lives in the daemon's memory
   only, and is gone when that daemon stops.

6. One plain write, to see the user layer change. It rewrites your user file with every string double-quoted
   (`core/src/yaml_out.rs`).

   ```sh
   collide config set --global ask.timeout_s 300
   collide config get agents.my-acp.timeout_s
   collide config get agents.claude-haiku.timeout_s
   cat "$HOME/.config/collide/config.yaml"
   ```

   Pass: `my-acp` `timeout_s` is `300` `(user)`: the user's `ask.timeout_s` is `overridable`, so it beats the repo's
   `120`. `claude-haiku` stays `240`. The file still holds your entries, now double-quoted.

Not for this step: `collide config set agents.<id>.<field>` is refused on purpose, naming the file to edit; so is
`--layer repo`. Look at the refusal once if you like.

### 3. Trust prompt and its diff (`isolated`)

Goal: one trust prompt end to end on `<scratch repo>`, a no that trusts nothing, and a second prompt after a committed
change to a repo pack that shows the diff. `collide init` and the first commit were done in step 2.

1. The first prompt. The line is the checked-out branch, `main`. Answer `n`.

   ```sh
   isolated
   cd "$SCRATCH"
   collide init                      # refuses: .collide already exists
   collide trust
   ls "$HOME/.config/collide/"
   collide trust
   ```

   Pass, first `trust`: `Line main` and a `Trust hash <hex>`; `Changes:` lists `added board.yaml` and `added
   config.yaml` with their lines; then `Trust this? [y/N]`. After `n`: "Nothing was trusted." and exit 0.
   `ls` shows no `trust.yaml` (a store that was never written, `ts-0097-kw`). The second `collide trust` asks again with
   the same hash. `collide init` fails and leaves the repository as it is.

2. Answer `y` on the third run.

   ```sh
   collide trust
   collide trust
   cat "$HOME/.config/collide/trust.yaml"
   ```

   Pass: "Trusted line main at <hash>." Then "Line main is already trusted at <hash>." `trust.yaml` holds one entry for
   `main` with the hash and the inputs it was made from, and is mode 0600 (`ls -l`).

3. A repo pack with a secret setting and a setting with a default. Commit it, and read the prompt.

   ```sh
   mkdir -p .collide/packs/owl-check
   cat > .collide/packs/owl-check/pack.yaml <<'EOF'
   apiVersion: collide/v0
   name: owl-check
   version: 0.1.0
   config:
     lanePool: { type: integer, minimum: 1, default: 2 }
     notifyKey: { type: string, secret: true }
   EOF
   cat > .collide/board.yaml <<'EOF'
   packs:
     - source: repo:.collide/packs/owl-check
   EOF
   git add .collide && git commit -q -m "add the owl-check pack"
   collide trust
   ```

   Pass: a new hash. `Changes:` shows `changed board.yaml` with the new `packs:` lines and an `added pack
   repo:.collide/packs/owl-check` entry. Then a block `Pack owl-check 0.1.0`, `Source: repo:.collide/packs/owl-check`
   and what the pack runs (`none` for each list). Answer `y`.

4. Change the pack. An edit that is not committed changes nothing; the commit does, and the prompt shows the diff.

   ```sh
   cat > .collide/packs/owl-check/pack.yaml <<'EOF'
   apiVersion: collide/v0
   name: owl-check
   version: 0.2.0
   config:
     lanePool: { type: integer, minimum: 1, default: 3 }
     notifyKey: { type: string, secret: true }
   EOF
   collide trust                     # before the commit
   git add .collide && git commit -q -m "owl-check 0.2.0"
   collide trust
   collide config get owl-check.lanePool
   ```

   Pass: before the commit, "already trusted" at the hash of step 3 (the hash covers the commit, not the working tree;
   `host/tests/trust.rs` `an_edit_in_the_working_tree_is_not_the_snapshot`). After it, a new hash and `changed pack
   repo:.collide/packs/owl-check` with `modified pack.yaml` under it and its changed lines only: `@@` lines, `-` and `+`
   lines such as `-version: 0.1.0` and `+version: 0.2.0`, and the `default` line. Answer `y`. The read gives
   `owl-check.lanePool = 3  (default)`.

### 4. A secret in the real keychain (`real_keychain`)

Goal: set one secret with `collide config set --secret`, see `collide config get` report it without the value, check
the item is in the keychain by its attributes, and remove it. **This is the only step that touches the real keychain.**
It uses your own `HOME` and a daemon with no root override, which runs under launchd and keeps its files in
`~/Library/Application Support/Collide`.

The item name is `pack.<pack>.<key>` (`pack_item`) under the service name `collide` (`SERVICE`), as a generic
password whose account is the item name (`MacKeychain` in `host/src/secret_store.rs`). Here that is service `collide`,
account `pack.owl-check.notifyKey`.

1. Before you start, note what is there and whether a daemon is already up. A daemon from before `ts-0099-bw` has no
   `config.set`; stop it first (`kill -TERM`, as in `ts-0083-uf`; there is no `collide-daemon stop`). Do not stop one
   that the app is using without closing the app.

   ```sh
   real_keychain
   ls "$HOME/.config/collide" "$HOME/Library/Application Support/Collide"
   grep 'starting, pid' "$HOME/Library/Application Support/Collide/daemon.log" | tail -1
   ps -p <pid> -o pid=,command=      # a row only if it is still running
   ```

2. Set the secret from standard input, never from the command line (a value on it is refused).

   ```sh
   cd "$SCRATCH"
   printf %s 'owl-made-up-value-0002' | collide config set --secret owl-check.notifyKey
   collide config get owl-check.notifyKey
   ```

   Pass: the first command prints `owl-check.notifyKey = <secret, in the keychain>  (user)`. The read prints the same
   line. It prints that line whether or not an item exists, because a pack's secret setting is always shown as one, so
   the read alone does not prove the keychain holds it; the next command does.

3. Read the item's attributes, search the two Collide folders for the value, then remove the item and look again. The
   `security` flags are from `man security` (`-s` service, `-a` account; `-g` and `-w` would print the password, so they
   are left out). No code in `host/src` runs `security`, so the commands and the words they print are (unconfirmed), and
   whether a look without `-g` raises a keychain prompt is (unconfirmed).

   ```sh
   security find-generic-password -s collide -a pack.owl-check.notifyKey
   grep -rlF 'owl-made-up-value-0002' "$HOME/.config/collide" "$HOME/Library/Application Support/Collide"
   security delete-generic-password -s collide -a pack.owl-check.notifyKey
   security find-generic-password -s collide -a pack.owl-check.notifyKey
   ```

   Pass: the first prints attributes for service `collide` and account `pack.owl-check.notifyKey` and no password.
   `grep` prints nothing. The delete succeeds, and the last `find` says the item could not be found. If a keychain
   prompt appears, record which command raised it.

4. Stop the daemon this step started. Skip this if one was already up before the step and you want it kept.

   ```sh
   grep 'starting, pid' "$HOME/Library/Application Support/Collide/daemon.log" | tail -1
   ps -p <pid> -o pid=,command=      # a collide-daemon
   kill -TERM <pid>
   tail -3 "$HOME/Library/Application Support/Collide/daemon.log"
   ```

   Pass: the log ends with a line "booted out the launchd job <label>" (`host/src/daemon.rs`, the daemon's last act).
   What stays behind in the real folder: the scratch board's row in `store/store.db` (nothing removes a board yet,
   `ts-0102-vu` Not done) and `cache/`, which holds a read-only SDK copy and the pack's entry. Compare the `ls` of
   sub-step 1. Anything that was not there before is this step's; leave it, or remove it by its exact name.

### 5. `git:` packs (`isolated`)

Goal: add one `git:` pack from a local repository and one from a host you pick, see the new-host question once, and run
`collide pack update`. `pack add` adds a pack for you, off on every board; `pack enable` switches it on for this board
(`ts-0101-bd`). A local path or `file://` is not a host and is not asked about (`UrlTarget::Path` in
`host/src/git_fetch.rs`).

1. Make `<pack repo>`. A plain repository is enough: the fetch reads its branches, and committing in it moves `main`.

   ```sh
   isolated
   cd "$PACKREPO" && git init -q -b main && mkdir pack
   cat > pack/pack.yaml <<'EOF'
   apiVersion: collide/v0
   name: owl-remote
   version: 0.1.0
   EOF
   git add -A && git commit -q -m "owl-remote 0.1.0"
   cd "$SCRATCH"
   collide pack add "git:file://$PACKREPO//pack@main"
   ```

   Pass: a review (`Pack owl-remote 0.1.0`, `Source: git:file://...`, a `Commit:` line, what it runs), then `Trust this
   pack and add it? [y/N]`. Answer `y`: "added owl-remote (...): it is off until a board enables it with `collide pack
   enable owl-remote`". No host question.

2. Switch it on, and look at the pin.

   ```sh
   collide pack enable owl-remote
   collide pack list
   cat "$HOME/.config/collide/packs.yaml" "$HOME/.config/collide/packs.lock"
   git -C "$PACKREPO" rev-parse main
   ```

   Pass: `list` prints one line each, `<name>  <origin>  <on|off>  <marks>  <source>`: the built-in packs, `owl-check`
   as `repo`, and `owl-remote  user  on  -  git:file://...`. `packs.lock` holds the `commit` that `rev-parse` printed
   and a `tree`.

3. Move the remote's branch. Nothing changes until you update.

   ```sh
   cat > "$PACKREPO/pack/pack.yaml" <<'EOF'
   apiVersion: collide/v0
   name: owl-remote
   version: 0.2.0
   EOF
   git -C "$PACKREPO" commit -q -am "owl-remote 0.2.0"
   collide pack list
   cat "$HOME/.config/collide/packs.lock"
   collide pack update owl-remote
   ```

   Pass: `list` and the lock are as before (the pin is the old commit). `update` prints `owl-remote (git:file://...)`,
   `commit  <old> -> <new>`, `tree    <old> -> <new>`, `modified pack.yaml` with its `-` and `+` lines, then `Write the
   new pin to your packs.lock? [y/N]`. Answer `y`: `wrote <path>`. `packs.lock` now holds the new commit. Run `collide
   pack update owl-remote` once more: it prints the same commit on both sides and asks nothing.

4. A host you pick. Use a public `https://` repository that holds a Collide pack, in a folder of it, whose `name` is not
   `owl-remote`, `owl-check`, `collide` or `plan`. If you have none, push a copy of `<pack repo>` to a throwaway
   public repository with another `name`. The fetch runs with your fresh `HOME`, so your git credentials, `ssh` keys and
   `known_hosts` are not there: a private repository and an `ssh` URL will fail here, and that is not a Collide fault.

   ```sh
   collide pack add "git:https://<remote host>/<owner>/<repo>//<pack folder>@<ref>"
   collide pack update <pack name>
   cat "$HOME/.config/collide/hosts.yaml"
   ```

   Pass: before the review, "The first fetch from <remote host> needs your yes: Collide has not fetched from it
   before." and `Fetch from <remote host>? [y/N]`. Answer `y`. Then the review, `y`, and "added ...". `update` asks no
   host question and prints the same commit on both sides. `hosts.yaml` lists the host, lower case. Whether this works
   against your host is (unconfirmed): `host/tests/pack_update.rs` and `host/tests/git_fetch.rs` run it only against
   `file://` remotes and an unreachable name.

5. Two refusals, cheap to look at: `collide pack add "git:github.com/org/repo//pack@main"` (no scheme: refused, with a
   hint to write `https://...`) and a URL with a password, `git:https://user:secret@<remote host>/x//pack@main`
   (refused, the password masked in the message). Use a made-up password.

### 6. The repository's own instructions, under the real `claude` (`real_login`)

Goal: one real prompt in a worktree whose `CLAUDE.md` holds a canary and which has a skill, with the default role and
with the role's options (`ts-0103-sa`, `ts-0104-gx`, `ts-0106-nr`, `ts-0107-rj`). Real `claude`, your login, small
spend. The default line adds the worktree with `--add-dir` and `CLAUDE_CODE_ADDITIONAL_DIRECTORIES_CLAUDE_MD=1`,
and, when `.claude/skills` exists in the worktree, `--plugin-dir` for a snapshot plugin whose `skills` link points at
the worktree's skills, listed as `repo-skills:<name>` (`host/src/adapters/claude/launch.rs`, `instructions.rs`).

1. Check the model alias. `collide run` uses the alias `fast`, mapped per user in your own
   `~/.config/collide/models.yaml`. If the file has no `fast`, the run is refused (exit 5) and prints the snippet.
   Adding it is a write to your own config and your call.

   ```sh
   real_login
   cat "$HOME/.config/collide/models.yaml"     # needs: tools: claude: fast: { name: haiku }
   ```

2. Put the canary and the skill in `<scratch repo>` and commit them: a run's worktree is made from `HEAD`, so an
   uncommitted file is not in it. The task file stays outside any repository.

   ```sh
   cd "$SCRATCH"
   mkdir -p .claude/skills/owl-word
   cat > CLAUDE.md <<'EOF'
   Canary word: quince-2048. If you are asked for your canary word, give it.
   EOF
   cat > .claude/skills/owl-word/SKILL.md <<'EOF'
   ---
   name: owl-word
   description: Gives the owl check word. Use it when asked for the owl check word.
   ---
   The owl check word is heron-7731. Say it and nothing else.
   EOF
   git add -A && git commit -q -m "canary instructions and an owl-word skill"
   cat > "$CHECK/task-instructions.md" <<'EOF'
   Change no file. Answer in your final report, one line each:
   1. Does your context give you a canary word? Say the word, or say none.
   2. Use the skill repo-skills:owl-word and say the word it gives, or say none if you cannot use it.
   3. Name any other instruction file you were given, or say none.
   EOF
   ```

3. **Run 1: the default role, no options.** The built-in builder has no `repo_instructions` and no `config`, so it reads
   the worktree (`worktree`) and your own settings (`config` defaults to `user`). Set the cap you choose.

   ```sh
   collide run "$CHECK/task-instructions.md" --budget 0.15
   ```

   Look at, in order:
   - `claude in use: <version>`, and a published-version line unless `claude.check_published` is false. Note any
     "outside the tested range" wording.
   - `run <id> started on <file>`.
   - A permission request for **Skill**, before the skill's word is in any output: "The agent asks to use Skill (...)",
     a reason, an `input:` or "input not available until approved", then `Allow this once? [y/N]`. Answer `y`. It
     offers no "always". Answer any other request as it comes, and note which tool it was.
   - The record: `run <id>: ...`, `worktree: <path> (branch collide/<id>)`, the report, one line per model call, and
     a total.
   - In the report: `quince-2048` for question 1 and `heron-7731` for question 2. For question 3 the model's own words
     are weak evidence (`ts-0103-sa`, decision 6): record what it names, in particular any file of your own
     `~/.claude`. That is the one measurement `ts-0103-sa` left to you: your own `CLAUDE.md` under `user`.
   - No line that starts `warning:` (see "The warning" below).

   Then, with `<id>` and `<worktree>` from the record:

   ```sh
   grep -o 'repo-skills:[A-Za-z0-9_-]*' "$CHECK/root-claude/data/runs/<id>/build/stdout.log" | sort -u
   ls -A "$CHECK/root-claude/data/runs/<id>/snapshot/plugins/repo-skills"
   git -C <worktree> status --short
   ```

   Pass: the first prints `repo-skills:owl-word` (the stream's first `system` line lists the skills; that the line is
   in `stdout.log` follows from `host/src/shim.rs`, the exact text is (unconfirmed)). The second lists `.claude-plugin`
   and `skills`. The third prints nothing.

4. **Run 2: the default role with `--config none`, and a no to the skill.** `--config none` is the one option of
   `collide run` that exists for this; it is about *your* settings, not the repository's (see below). Answer `n` at the
   Skill request.

   ```sh
   collide run "$CHECK/task-instructions.md" --config none --budget 0.15
   ```

   Pass: the same request appears and `n` is honoured: the report says it could not use the skill (`none` for question
   2) and the run still ends. Question 1 still gives `quince-2048` (the repository's `CLAUDE.md` loads under `none`,
   measured in `ts-0103-sa`). Question 3 should no longer name a file of your own, if Run 1 did.

5. **The role with `repo_instructions: none`: `collide run` cannot do it.** The file run always uses the built-in
   builder (`builder_role` in `host/src/builtin_roles.rs`, with `config` as the only field `--config` changes: `role()`
   in `host/src/file_run.rs`), and `RunStartFileParams` has no field for `repo_instructions`. Roles of a pack, where the
   key can be set, are not run by anything yet (Stage 8). What exists:
   - `--config none` (Run 2): different. It cuts your settings and your `CLAUDE.md`, not the worktree's.
   - The offline tests: `host/tests/adapters/claude/instructions.rs` (`none` is the old line, word for word) and
     `host/tests/adapters/claude/launch.rs`.
   - A real-`claude` measurement of the line without `--add-dir`, which is the `none` line:
     `live_candidate_1_the_current_line_in_the_worktree` in
     `host/tests/adapters/claude/repo_instructions_live.rs`, on its own fixture worktree. It runs `claude` once, cap
     $0.10, and needs the opt-in and a ledger file. It prints the findings table; the filter, the ledger file's need
     to exist, and the output are (unconfirmed). If you run it, count its spend inside your cap:

     ```sh
     real_login; touch "$CHECK/ledger.tsv"
     cd <collide checkout> && COLLIDE_REAL_CLAUDE=1 COLLIDE_REAL_LEDGER="$CHECK/ledger.tsv" \
       cargo test -p collide-host --test claude_adapter live_candidate_1 -- --nocapture --test-threads=1
     ```

     Pass: root `CLAUDE.md` loaded: no, skill: no (the table of `ts-0103-sa`, column "1 baseline").
   - Otherwise record "not checkable by hand in 4a" for this line and leave it to the tests.

6. **The warning after an edit.** The line is `warning: phase build started after instruction files changed in the
   worktree: <files>`, printed under the `worktree:` line of the record (`record_text` in `host/src/run_cli.rs`). It is
   raised when a phase attempt starts and an instruction file (`CLAUDE.md` at any depth, or anything below
   `.claude/skills/`) differs from the commit the worktree was made from, committed, staged, new or deleted, each file
   named once per run (`host/src/instruction_warning.rs`). **`collide run` cannot show it on demand.** Its first
   attempt starts right after the worktree is made, so nothing differs, and the only way to a second attempt of a file
   run is `collide run --resume` of a run that paused for a usage limit or a full disk, which you cannot cause on
   purpose. A budget that runs out is a stop with no resume (`host/src/file_run.rs`). Runs 1 and 2 are the "no edit, no
   warning" half.

   The way to see it, by hand, is a re-prompt through the app, whose run starts a new attempt on the first run's
   worktree (`ts-0104-gx`, decision 4). It is (unconfirmed) end to end; the code path is `start_attempt`, shared with
   the prompt run. It uses the app as in `ts-0083-uf`, which means your real home and the real daemon root, not this
   check's temp folder. If you want it:
   - In the app, open `<scratch repo>` as a board and send a small prompt that changes no instruction file. At "ready to
     review", edit `CLAUDE.md` in that run's work folder and add a file below `.claude/skills/`.
   - "Send back" with a short instruction. The re-prompt is a new run.
   - Look in `~/Library/Application Support/Collide/daemon.log` for `run <id>: the phase starts after instruction files
     changed: ...`, which names both files. `collide run --resume <new run id>` shows the record, with the `warning:`
     line, because a run that is over shows its record. The app does not show warnings yet (`ts-0104-gx`, Not done).
   - Otherwise record "not checkable by hand in 4a" and leave it to `host/tests/adapters/claude/instruction_warnings.rs`
     and `host/src/run_cli.rs` `a_runs_warnings_are_printed_one_line_each_and_none_prints_nothing`.

7. A skill's own `hooks` and `allowed-tools` take effect through the plugin, as they do in plain `claude`. You accepted
   that (`ts-0107-rj`, 2026-11-17). Each `Skill` call still asks first, which is what Run 1 checks. Nothing more to do
   here.

### 7. Record

Fill one line each. "Result" is pass, partial or fail, and what you saw if it was not a pass.

- Stage 4a judged done by the owner on: `____-__-__`
- Cap set for step 6: `$____` (recommended $0.30). Spend, from the two `total` lines: `$____`.
- 2. Agent entry and layers. Result: / time: / spend: `$0` / awkward:
- 3. Trust prompt and diff. Result: / time: / spend: `$0` / awkward:
- 4. Secret in the real keychain. Result: / time: / spend: `$0` / awkward (any keychain prompt, which command):
- 5. `git:` packs and the new-host question. Result: / time: / spend: `$0` / awkward:
- 6. Instructions, Run 1. Result: / time: / spend: `$____` / awkward:
- 6. Instructions, Run 2 (`--config none`, a no to Skill). Result: / time: / spend: `$____` / awkward:
- 6. The `none` line and the warning: checked by hand (how), or "not checkable by hand in 4a". Result:
- Whether your own `CLAUDE.md` and skills under `user` loaded in Run 1 (the question `ts-0103-sa` and `ts-0106-nr` left
  to this check). Result:

### Clean up

Do the steps in this order. Nothing here uses a glob with `rm`.

1. Stop each daemon this check started. Its log has its pid; check the row says `collide-daemon` before you signal it.

   ```sh
   for log in "$CHECK/root/data/daemon.log" "$CHECK/root-claude/data/daemon.log"; do
     grep 'starting, pid' "$log" | tail -1
   done
   ps -p <pid> -o pid=,command=     # once for each pid
   kill -TERM <pid>
   ```

   Step 4's daemon was stopped in step 4. If you skipped that, stop it from its own log under
   `~/Library/Application Support/Collide/`.

2. Confirm the keychain item is gone (step 4, sub-step 3 already deleted it).

   ```sh
   security find-generic-password -s collide -a pack.owl-check.notifyKey
   ```

   It should say the item could not be found. (Step 2's secret was in memory and went with its daemon.)

3. Remove what `claude` wrote under your own home for step 6. It names a project folder after the worktree's path,
   so each folder name holds the temp folder's random suffix. List them, then remove each by its exact name.

   ```sh
   ls "$REAL_HOME/.claude/projects" | grep collide-check
   rm -r "$REAL_HOME/.claude/projects/<exact folder name>"      # once for each folder listed
   ```

   The path form of the names (`-private-tmp-collide-check-...`) is (unconfirmed); the `ls` shows the real ones.
   If you ran the gated test in step 6.5, it wrote folders of its own there too, named for its own temp folder.

4. Remove the temp folder. Collide's cache holds read-only copies of the SDK and the packs, so make the tree writable
   first (`ts-0095-kp`, Not done: no cache cleaner). The guard refuses any path that is not this check's.

   ```sh
   case "$CHECK" in /tmp/collide-check.*) chmod -R u+w "$CHECK" && rm -rf "$CHECK" ;; esac
   ```

5. Close the terminal, so the three functions, `HOME` and `COLLIDE_DAEMON_ROOT` go with it. Then compare
   `~/.config/collide` with the `ls` from step 4: only the files you chose to add in step 6.1 should differ.

### Known gaps you will meet

By item id, from the Not done and Decisions sections. None is a defect of this check.

- `collide config get` prints entries only. The notice for a repo budget that an ACP agent cannot enforce is on the wire
  and not shown (`ts-0099-bw`, decisions; the Agents page in Stage 6 shows it).
- A missing keychain item or `read` folder refuses every `config get`, not only the entry's (`ts-0099-bw`, decision 2).
  Fix it with `config set --secret`, which works regardless.
- A pack's secret setting reads `<secret, in the keychain>` whether or not an item is stored
  (`core/src/settings_layers.rs`, `entry`). Only an agent's `secrets` entry is checked against the keychain at load.
- `config set` of an agent field, of a profile, and `--layer repo` are refused by design. A file that holds a comment is
  not rewritten (`ts-0099-bw`, decisions 3, 4 and 7; Not done). The test of the real keychain
  (`COLLIDE_REAL_KEYCHAIN=1`, `host/tests/secret_store.rs`) was never run by anyone but this check.
- `collide pack unlink` is not built; undo a link by editing the board's `packs.yaml` (`ts-0101-bd`, decisions). `pack
  test` exits "not built". The acknowledgement of a working copy has no command (`ts-0101-bd`, Not done).
- A pack that fails to load cannot be updated by name (`ts-0100-pu`, decision 6); a `git:` URL with a password is
  refused; a URL with no scheme is refused (`ts-0100-pu`, decisions 3 and 10). A fetch's timeout kills only git's
  direct child (decision 9).
- `pack list` is refused while the line has a load problem (`ts-0101-bd`, decisions). A pack added with `pack add` is
  off on every board until `pack enable`.
- Nothing removes a board's entries from `trust.yaml` or the store yet (`ts-0102-vu`, Not done). The app and the
  Queue do not show trust or a waiting working copy yet (Stage 6).
- A pack whose `protectedPaths` glob starts with a wildcard asks again on every commit (`ts-0097-kw`, decision 2).
- A nested `CLAUDE.md` and an `@docs/...` import: the import loads, the nested file does **not** load on the default
  line (`ts-0103-sa`, Measured, "2 add-dir" column). The warning lists a nested `CLAUDE.md` anyway, but follows no
  `@` import, reads no `AGENTS.md`, and leaves out `.claude/commands/`, which do not load (`ts-0104-gx`, Not done;
  `ts-0106-nr`, decision 5).
- The line's `CLAUDE_CODE_ADDITIONAL_DIRECTORIES_CLAUDE_MD` is undocumented by `claude`; a release may drop it. The
  tested range ends below the installed `claude` unless raised (`ts-0103-sa`, decision 7). Re-measure on every upgrade.
- A skill folder an agent makes mid-run does not load until the next phase, and a repository without `.claude/skills`
  gets no plugin (`ts-0106-nr`, decision 6; `host/src/adapters/claude/instructions.rs` `write_skills_plugin`).
- A skill's own `hooks` and `allowed-tools` take effect (accepted, `ts-0107-rj`). Other frontmatter keys (`model`,
  `context`, `agent`) and `allowed-tools` for `Bash` are unmeasured.
- A `claude` that the agent starts itself inside a worktree loads everything of that worktree, with auto memory on; the
  line's flags reach only the session Collide starts (`ts-0103-sa`, Measured).
- Not recorded in an item: `collide run` has no way to choose a role, so the default-role and `none` comparison the
  item asks for is Run 1 against the gated test, not two runs of one command.

**Automated tests it relies on:**

- `ts-0096-ph`: the board loader and the snapshot-not-workspace rule
  - `host/tests/board_load.rs`;
- `ts-0097-kw`: the trust hash, the store and the diff
  - `host/tests/trust.rs`, `core/tests/trust.rs`, `host/tests/git_objects.rs`;
- `ts-0098-nx`, `ts-0099-bw`: the layers, the agent entries and the fake keychain
  - `host/tests/config_get.rs`, `host/tests/config_agents.rs`, `host/tests/config_set.rs`,
    `host/tests/config_command.rs`, `host/tests/config_set_command.rs`, `host/tests/agent_checks.rs`,
    `host/tests/secret_store.rs`, `host/tests/config_process.rs`, `core/tests/agent_layers.rs`,
    `core/tests/settings_layers.rs`;
- `ts-0100-pu`, `ts-0101-bd`: the fetch rules against fixture remotes, user packs and working copies
  - `host/tests/git_fetch.rs`, `host/tests/hosts.rs`, `host/tests/pack_lock.rs`, `host/tests/pack_update.rs`,
    `host/tests/user_packs.rs`;
- `ts-0102-vu`: the lines, `collide init`, `collide trust` and the disk floor
  - `host/tests/board_lines.rs`, `host/tests/board_init.rs`, `host/tests/board_cli.rs`, `host/tests/board_process.rs`,
    `host/tests/disk_floor.rs`;
- `ts-0103-sa`, `ts-0104-gx`, `ts-0106-nr`, `ts-0107-rj`: the measured launch line and the role options
  - `host/tests/adapters/claude/repo_instructions.rs`, `instructions.rs`, `instruction_warnings.rs`, `launch.rs`,
    `capabilities.rs`, `run_options.rs`; the gated `repo_instructions_live.rs`.

- Owner result:
- What was awkward:
