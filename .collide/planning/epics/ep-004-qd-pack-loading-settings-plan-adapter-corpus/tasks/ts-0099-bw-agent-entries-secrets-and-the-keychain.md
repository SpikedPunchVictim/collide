---
id: ts-0099-bw
kind: task
status: active
title: "Agent entries, secrets and the keychain port"
owners: []
links:
  - { type: blocked-by, to: ts-0098-nx }
---

## Goal

The agent entries of 0.7 through the layers, with every load check this stage owns, and secrets through a keychain port
with `collide config set`. Launching an agent, and the test that a secret reaches the agent and appears in no file,
argv or log, are Stage 6b's (4a "checks agent entries at load only").

**Agent entries** (a pure function in `core`, `agent_layers`). For each agent id, each field, the value and the layer:

- the first layer holds the built-in entry (`settings::builtin_agents()`: Claude Code, `haiku`, `config: none`, a 180 s
  timeout, no budget, `read: ["<board root>"]`, tools `read` and `search`); the user's file changes it field by field
  and adds entries; the repo's file may set only `ask.budget_usd` and `ask.timeout_s` (defaults for every ask, not per
  id);
- `ask.agent` names an entry that exists, else the load fails naming `ask.agent`; with no config at all it names
  `claude-haiku`;
- a repo `ask.budget_usd` that reaches an agent that cannot enforce one (ACP, external) is ignored, with a notice that
  names the agent, and is not an error;
- `collide config get agents.<id>.<field>` shows the value and the layer, for every field: `default` for the built-in
  entry, `user` or `repo`.

**Load checks.** `check_config_file` already covers the timeout range (5 to 1800), `tools` of `read` and `search`, a
budget on an agent that cannot enforce one, a literal value on a secret-named `env` line (hint: use `secrets`), and the
refused ACP arguments. This item adds, in `host`: a `read` folder that does not exist is an error naming the key and
the folder (`<board root>` is the line's default root); a `secrets` entry whose keychain item is missing is an error
naming the item and the key (the port's `contains`; the value is never read at load).

**The keychain port** (`host/src/keychain.rs`): a trait with `set(item, secret)`, `contains(item)`, `get(item)` and
`delete(item)`, a `MemoryKeychain` double, and the real store behind the trait (see the dependency decision). Items live
under one service name, `collide`. A secret type that does not print: its `Debug` and `Display` write `<secret>`.

**`collide config set <key> <value> [--secret]`**, serving `config.set` (`ConfigSetParams`):

- `--secret` takes the value from stdin, never from the command line (argv is visible to other processes and to shell
  history), and the CLI refuses a value argument with it. The key is a pack setting marked `secret: true`
  (`<pack>.<key>`, kept under the item `pack.<pack>.<key>`) or an agent's secret reference (`agents.<id>.secrets.<ENV>`,
  kept under the item name the entry holds). A key that is neither is refused naming it. The value goes to the keychain
  only. The daemon does not log, journal or echo the request's `value`; the result carries no value.
- Without `--secret`, the key is checked against the registry or the pack's schema, the value is validated, and a secret
  key is refused ("use `--secret`"). The write is to the user's file (the board's own user file when `board` is given)
  through `yaml_out`. When the existing file holds a comment, the write is refused and says to edit the file by hand,
  because rewriting would drop the comment. A repo write (`layer: repo`) is refused with "edit and commit
  `.collide/config.yaml`".
- The result is the setting after the write, with its layer. This fills the `cli: config` stub that `ts-0098-nx`
  retagged.

## Success criteria

- With no config at all the built-in entry resolves to Claude Code, `haiku`, `config: none`, a 180 s timeout and no
  budget, and `ask.agent` names it.
- A repo file that sets `command`, `args`, `env`, `secrets`, `read`, `tools`, `model`, `config` or `ask.agent` fails the
  load and names the key. The same file's `budget_usd` and `timeout_s` load. The repo's `budget_usd` wins over the
  user's and the user's `timeout_s` wins over the repo's.
- `collide config get agents.claude-haiku.timeout_s` names the layer, and so does every other field of every entry.
- `tools: [write]` fails. A `budget_usd` on an ACP entry fails with "only a timeout applies". An `env` line
  `OPENAI_API_KEY: sk-test` fails with the hint to use `secrets`. A `read` folder that does not exist fails naming the
  key.
- Names that should match as secrets (`API_KEY`, `GITHUB_TOKEN`, `DB_PASSWORD`, `CLIENT_SECRET`, in lower case too) do,
  and names that should not (`KEYBOARD`, `TOKENIZER`, `PASSWORD_FILE`, `SECRETARY`) do not. The rule is a suffix
  match as built, so `MONKEY` matches; a test pins that and the Notes say so.
- A `secrets` entry names a keychain item and holds no value in any file. A missing item is an error that names the
  item and the key. A present one is not read.
- `ask.agent: nobody` fails naming `ask.agent`. An ACP entry that gets a repo budget loads, with a notice naming it.
- `collide config set --secret <pack>.<key>` with the value on stdin stores it in the fake keychain and in no file.
  Searching the temp root, the daemon's log and its journal for the value finds nothing, and the CLI's own argv never
  held it. A value given on the command line with `--secret` is refused.
- `config set` of a non-secret key writes the user's file, and `config get` shows the new value and layer `user`. A
  file that holds a comment is not rewritten, and the message says why. A repo write is refused.
- `Debug` of the secret type is `<secret>`.
- The real store compiles and satisfies the trait's contract in one test gated for the owner (`ts-0105-jv`); no
  automated test touches the real keychain.
- `cli: config` is filled: the stub tag moves to `FILLED_TAGS` in `checks/tests/real_tree.rs`.

## Tests

Write these first:

- `agent_layers` as a table: every field's layer, the repo's budget over the user's, the user's timeout over the repo's,
  an entry added by the user, a change to the built-in entry, `ask.agent` naming a missing entry, a notice for an ACP
  entry;
- each refusal above through `check_config_file` and the repo-file refusals, one case each, the message naming the key;
- the secret-name table with the matching and non-matching names;
- `host` load checks: a `read` folder that exists and one that does not, `<board root>` expanded, a missing item and a
  present item in the `MemoryKeychain`;
- the keychain port contract, run over `MemoryKeychain` (and, gated for the owner, the real store): set, contains, get,
  replace, delete, a missing item;
- `config set`: through the daemon and the CLI with a temp `HOME` and the `MemoryKeychain` injected: a secret from
  stdin, a secret on argv (refused), a secret not found in any file, the log or the journal (the value is a canary
  string), a non-secret write, a comment-bearing file, a repo write, an unknown key, an invalid value;
- `pnpm gen` and `pnpm check` with the stub moved to filled.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 4a: Settings (agent entries; secrets from the OS keychain only); Success Criteria (Agent entries); Tests
    (Agent entries).
  - 0.7: Settings; Agent entries; Layers and sources; Secrets are names. 0.3: Agents the ask box can use.
  - 6b owns the launch and the secrets launch test; this item stops at load.
- Carried from `ts-0025-st`: the keychain, and whether `ask.agent` names an entry.
- **Decision for the owner, with a recommendation: the budget's precedence.** 0.7 says an entry's own `budget_usd` in
  the user's file wins over the repo's default. The 4a success criterion (owner, 2026-11-14) says the repo's
  `budget_usd` wins over the user's while the user's `timeout_s` wins over the repo's. They disagree. The criterion
  follows the general rule (the repo's value wins, except an `overridable` key, and `ask.timeout_s` is the only one), so
  a team's cost cap holds. This could change what a developer can spend. Recommend the criterion: a team budget is a
  cap. Alternative: the user's entry wins, and the team's value is only a default for entries that set none. Then the
  plan text for 4a changes.
- **Decision for the owner, with a recommendation: the keychain crate.** One store is needed by this stage and by 6b
  (the endpoint token, `ui_tools::KeychainTokens`; secrets at launch).
  - `security-framework` 2.11.1 (MIT OR Apache-2.0), macOS only. Its `passwords` module sets, reads and deletes a
    generic password. Small, and it is the layer under the next option. Con: the other platforms (the plan defers
    Linux and Windows) need their own store behind the same trait.
  - `keyring` 3.6 (MIT OR Apache-2.0; from memory, not checked here): one API over macOS, Linux and Windows. Con:
    more code and features to choose, and for now only macOS is used.
  - The macOS `security` command: puts the secret in argv. No.
  - Neither, port and fake only, with the real store in 6b: then `config set --secret` would not work in 4a, against the
    plan's text.
  Recommend `security-framework`, behind `#[cfg(target_os = "macos")]`, in `host` only, as the plan is macOS first.
  `core` never names it.
- **Decision for the item, with a recommendation: comments in a rewritten file.** `yaml_out` drops comments. Recommend
  refusing a rewrite when the file has a comment, and saying so. Alternative: keep a `.bak` and rewrite. The
  Settings page (6a) can choose its own way.
- **Decision for the item:** the keychain item for a pack secret is `pack.<pack>.<key>` under service `collide`. An
  agent's reference is the item name the user wrote, under the same service.
- **Decision for the item:** `config set --secret` reads stdin to its end and trims one trailing newline. A terminal
  prompt that does not echo is a later addition.
- The secret-name rule is a suffix match (`SECRET_NAME_ENDINGS`), so `MONKEY` reads as a secret. Recommend keeping it:
  a false positive asks the person to use `secrets`, and a false negative leaks. A word-boundary rule is a later change.
- Do not fill `ui_tools: endpoint token store`: it stays a 6b stub. This item's store is what it will use.
- Area: `core/src/agent_layers.rs` (new, pure), `core/src/settings.rs`, `host/src/keychain.rs` (new),
  `host/src/settings_files.rs`, `host/src/daemon.rs` (`config.set`), `host/src/commands.rs`,
  `checks/tests/real_tree.rs`, `stub-tags.json`, `host/Cargo.toml` (only if the owner takes the crate), `host/tests`,
  `core/tests`.
- The test rule: no test touches the real home, real launchd or a real daemon. Each test clears the environment
  (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as `host/tests/common/mod.rs` `TempRoot`
  does. Settings files are fixtures, never the owner's own `settings.json`.
- Stage 4a item (revision 23 scope, split 2026-11-16). Items of Stage 4a run one at a time, in id order, because
  they share `core`, `host` and the generated files.
- Owner decisions (2026-11-16, at the split): the recommendations above are taken under the owner's delegation; they
  stay overridable until the item is built. The owner chose two (2026-11-16): the budget's precedence follows the 4a
  criterion, so the repo's `budget_usd` wins over an entry's own and a team's cap holds (0.7 now says so); and
  `security-framework` 2.11.1 is approved as a new dependency, macOS only, behind the keychain trait with a memory
  double for tests.

### Built

- `core/src/agent_layers.rs` (new, pure): `agent_layers(&AgentInputs)` gives each entry, each field's layer,
  `ask.agent`, the
  notices and the problems. `AgentView::config_entries` makes the `agents.<id>.<field>` entries. Also in `core`:
  `settings::is_secret_name`, `check_agents`, `ConfigFile::with_value`, `DEFAULT_TIMEOUT_S`; `yaml_out::has_comment`;
  `setting_value::value_from_text`; `settings_layers::find_setting` (and `KeyInfo` is public).
- `host/src/keychain.rs`: `Secret` (prints `<secret>`), the `Keychain` trait, `KeychainError`, `MemoryKeychain`,
  `pack_item`, `MacKeychain` (`security-framework` 2.11.1, macOS only, service `collide`, `contains` reads attributes
  only), `UnavailableKeychain` elsewhere, `system_keychain`, and `keychain_for_root` (memory under
  `COLLIDE_DAEMON_ROOT`).
- `host/src/atomic_write.rs`: one helper (temp file in the folder, mode on the open file, rename, sync). `shim.rs` and
  `trust.rs` use it and keep their own wording and error types; `config set` uses it too.
- `host/src/agent_checks.rs`: a `read` folder that is not a folder, and a `secrets` item the keychain lacks (`contains`
  only). `settings_files.rs` runs `check_agents` on the two user files, naming the file.
- `host/src/config_get.rs`: gather and resolve are shared; `get` answers `agents.<id>.<field>` and `agents.<id>`, adds
  `notices` to `ConfigGetResult` (`pnpm gen` ran), and runs the load checks. New `ConfigService::set` and `SetError`.
- `config.set` served in `daemon.rs` (`DaemonOptions.keychain`, memory by default; `collide-daemon` picks it by root).
  Its params error carries no serde text, no line is logged for the call, and the answer holds no value.
- `host/src/config_set_cli.rs` and `config_cli::run_config`: `collide config set <key> <value> [--layer user|repo]
  [--global]` and `collide config set <key> --secret` (stdin, 64 KiB cap, UTF-8, one trailing `\n` or `\r\n` trimmed,
  empty refused). `cli: config` is built and moved to `FILLED_TAGS` (41); `ui_tools: endpoint token store` stays a 6b
  stub.
- Cargo.lock gained `security-framework` 2.11.1, `security-framework-sys` 2.17.0 and `core-foundation` 0.9.4.
- Tests: `core/tests/agent_layers.rs` (37), `settings_edit.rs` (18), `settings_layers.rs` (+3), `setting_value.rs` (+2);
  `host/tests/atomic_write.rs` (9), `keychain.rs` (11, one gated by `COLLIDE_REAL_KEYCHAIN=1`, not run),
  `agent_checks.rs` (13), `config_agents.rs` (19), `config_set.rs` (35), `config_set_command.rs` (17),
  `config_process.rs` (9, real binary and daemon under a temp root). Changed existing tests: `config_get.rs`
  (`config.set`
  is served now), `config_process.rs` (the stub test), `command_tree.rs` (see below), `real_tree.rs` (the tag moved).
- Red first: seen red by assertion were the 8 of 10 keychain tests and the 9 of 13 `agent_checks` tests (over `todo!`
  stubs), and the new `agent_layers`, `settings_layers` (`find_setting`) tests. Not seen red by assertion:
  `settings_edit`
  (the first red run stopped at an earlier binary), and `config_set`, `config_agents`, `config_set_command`,
  `value_from_text`, which were written first but failed only to compile until the code existed.
- Gates: `cargo build`, `cargo test --workspace` (3628 passed, 0 failed), `clippy -D warnings`, `fmt --check`, `pnpm
  test`,
  `pnpm typecheck`, `pnpm --filter @collide/app build`, `pnpm build:packs`, `pnpm check` (`stubs: ok`) all pass. No
  `packs/plan` timeout and no `shim_child` flake were seen.
- **Incident.** `host/tests/command_tree.rs` ran `collide config set disk.floor_gib 3` with the real environment, as
  a stub check. Once `set` was built, that run reached the real home: a real `collide-daemon` started under launchd
  and wrote `~/.config/collide/config.yaml` and `~/Library/Application Support/Collide/`. At the review, with the
  owner's yes, the daemon's launchd job was booted out and both folders, which held only what that run made, were
  removed. The cause is fixed: `command_tree.rs`, `schema_command.rs` and `task_cli.rs` now start `collide` with
  `env_clear` and a temp `HOME` and `COLLIDE_DAEMON_ROOT`, and `checks/tests/real_tree.rs` fails any `host/tests`
  file that starts the binary without `env_clear` (`every_test_that_runs_the_collide_binary_clears_its_environment`,
  seen red on those three).

### Decisions for the owner

1. **`config get ask.agent`.** It still prints `<not set> (default)`, because the existing tests pin it and the full
   listing. `agent_layers` knows the resolved id (`claude-haiku`). Recommend showing the resolved value, as a change to
   those two tests. Alternative: leave it.
2. **Machine checks refuse every `config get`.** A missing `read` folder or keychain item fails the whole call, as every
   other problem does. `config set --secret` works regardless, so the way out is one command. Alternative: refuse only
   `agents.*` and the full listing, and make the rest a notice. Recommend keeping it simple (as built).
3. **Where the CLI writes.** In a checkout, `config set` writes the board's own user file, so a pack key can be checked
   against its schema; `--global` writes the user's own file (Collide keys only, because the wire's `board` is both the
   schema context and the target). Alternative: a second wire field. Recommend as built; revisit with the Settings page.
4. **Agent fields.** `config set agents.<id>.<field>` is refused, naming the file to edit; only
  `agents.<id>.secrets.<ENV>`
   with `--secret` is set here. Recommend leaving it to the Settings page (6a).
5. **A write that adds no new problem is allowed** even when the file already has one, so a bad `ask.agent` can be fixed
   with `config set`. A write that adds one is refused and nothing is written.
6. **A string value is read as the setting's type** (`"7"` for an integer, `"false"` for a boolean) before the schema
   check; text that does not read as the type stays text and the check names the type, never the value.
7. Plan text proposed for 0.7, Secrets: "`collide config set --secret` reads the value from standard input and refuses
   one on the command line. A file that holds a comment is not rewritten by `config set`; edit it by hand."

- Owner decisions (2026-11-16, at the build): 1 taken at the review: with no layer setting it, `config get ask.agent`
  shows the resolved `"claude-haiku"` (layer `default`). 2 to 6 as built, on delegation. 7 in the plan's 0.7 Secrets
  text. The Cargo.lock gained `security-framework` 2.11.1 (owner-approved), with `security-framework-sys` 2.17.0 and
  `core-foundation` 0.9.4 under it.

### Not done, by design

- Launching an agent and the test that a secret reaches it and no file, argv or log (6b). `ui_tools: endpoint token
  store`.
- The real-keychain test was not run (owner, `ts-0105-jv`). No test touched the real keychain.
- ts-0100 (git mirror): its lock or state writes can call `atomic_write::write_atomically`. ts-0101 (user packs): the
  `config.pack_*` methods are still "not served", and `config_get.rs` is where the user files are read.
  ts-0102 (daemon lines, trust prompt): `ConfigService::gather` loads `HEAD` at each call; a line cache replaces
  `load` there.
- `config set` of a profile, of a repo value (`--layer repo` is refused by design), and of an agent field.
