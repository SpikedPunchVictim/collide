---
id: ts-0100-pu
kind: task
status: active
title: "Pack sources: git: URLs, the mirror and the fetch rules"
owners: []
links:
  - { type: blocked-by, to: ts-0099-bw }
---

## Goal

`git:` packs, from any git URL (a repository on disk included), pinned in a lock, fetched into a bare mirror under the
fetch rules of 0.7 (R5), and moved only by `collide pack update`. Sources `repo:` and `dev:` already reach a tree id
(`ts-0096-ph`, `ts-0094-av`); this item makes `git:` reach one through the same materializer (`ts-0095-kp`), which reads
the mirror as it reads any git directory.

- **The locator** (`core`, on `PackSource::Git`): `<url>//<path>@<ref>`, where `//<path>` and `@<ref>` are optional. The
  `//` that follows a scheme (`https://`, `file://`, `ssh://`) is part of the URL, and the path separator is the first
  `//` after it; an scp-like URL (`git@host:org/repo`) and a bare path on disk (`/srv/repo.git`) are URLs too; the ref
  is the text after the last `@` that follows the path. Without a ref it is `HEAD`. A URL that starts with `-` is
  refused (the parser already does; the loader and the lock check keep the message), and so is a locator with a NUL or
  a newline, an empty URL, and a path with a `..` part.
- **The mirror**: `~/Library/Application Support/Collide/cache/git/<name>/`, a bare repository, `<name>` the URL made
  safe for a folder (letters, digits, `.`, `-` kept; the rest `_`; at most 100 characters) followed by 8 hex characters
  of the URL's own hash, so two URLs never share a folder. One fetch updates it.
- **The fetch rules**, in one function, `fetch(mirror, url, hosts)`:
  - the first fetch from a host Collide has not fetched from before asks, naming the host: `KnownHosts`
    (`~/.config/collide/hosts.yaml`, through `yaml_out`) says whether it is known, and an unknown host returns
    `NeedsApproval { host }` and runs nothing; a caller that has the person's yes records the host and calls again;
    a `file` URL (a path on disk) has no host and never asks;
  - git runs with `GIT_TERMINAL_PROMPT=0`, `GIT_ALLOW_PROTOCOL=https:ssh:file` (which takes precedence over the user's
    config, so `protocol.ext.allow=always` does not open `ext::`), `--` before the URL, and a timeout (120 s, the
    constant a test can read), through `bounded_run`;
  - the environment is cleared except `PATH`, `HOME`, `SSH_AUTH_SOCK` and `XDG_CONFIG_HOME`, because the plan has the
    fetch use "the user's git credentials"; `GIT_SSH_COMMAND`, `GIT_ASKPASS` and `GIT_EXEC_PATH` are not passed;
  - it fetches every branch and tag into the mirror (`+refs/heads/*:refs/heads/*`, `+refs/tags/*:refs/tags/*`), so a
    locked commit reachable from a branch is there;
  - a timeout fails with the pack's name and the time.
- **Resolve and pin**: `resolve(source, mirror)` gives the commit for the ref and the tree id at `<commit>:<path>` (the
  reader), so a lock entry is `{ source, commit, tree }`. **The lock**: `.collide/packs.lock` for a repo pack,
  `~/.config/collide/packs.lock` for a user pack, written by `yaml_out`, in source order, atomically. A load reads the
  lock and never the remote: the pin is the lock's `tree`, and the mirror is read only on a cache miss.
- **A lock whose commit the remote no longer has** fails at the fetch-then-read, naming the pack, the commit and the
  source. A fetched tree id that differs from the lock's for the same commit fails naming both.
- **`collide pack update <name> [--to <ref>]`** (0.7, Switching packs), serving a new owner method `config.pack_update`
  (`board`, `name`, optional `to`; result: the pack, the old and new pins, and the diff): fetches, resolves the ref (the
  source's own, or `--to`), builds the diff of the pack's manifest, workflow graph and hooks (the trust diff of
  `ts-0097-kw` between the two trees), and writes the lock only when the person confirms (`confirm`: false returns the
  preview). For a repo pack it changes the repo's lock in the user's checkout (the default line's root), uncommitted,
  for the user to commit; a `repo:` pack is refused ("a repo pack changes by a commit alone"). A `dev:` pack is refused.
  Trust for the new tree id is asked again by `ts-0097-kw` as for any change.
- **The loader** (`ts-0096-ph`) now fetches a locked `git:` pack that is not in the cache, but only when its host is
  known; otherwise the load reports `NeedsApproval` with the host, and nothing is fetched. The caller asks
  (`ts-0102-vu`).

## Success criteria

- A `git:` locator parses into URL, path and ref for each form: `https://h/o/r//pack@v1.2`,
  `file:///tmp/r.git//pack@main`, `/tmp/r.git//pack`, `git@h:o/r//pack@abc123`, `https://h/o/r` (no path, no ref), and a
  path with `@` inside the URL's user part. A URL that starts with `-`, a NUL, a newline, an empty URL and a path with
  `..` are refused, naming the form.
- A pack from a `git:` source on a local bare repository (the fixture remote) loads by its pinned tree id. Moving the
  remote's branch changes nothing until `collide pack update`.
- A lock whose commit the remote no longer has (the branch was rewritten and the object pruned) fails, naming the pack
  and the commit.
- The first fetch from a new host asks, naming the host, and fetches nothing: a fixture host list is empty, and the call
  returns `NeedsApproval` with git never started (a fake `git` that fails if run proves it). With the host recorded, the
  fetch runs. A path on disk never asks.
- A fetch that takes longer than the timeout (a fake `git` that sleeps, under a short timeout the test sets) fails with
  the pack's name and is not left running.
- A URL that starts with `-` is refused at load, before git runs.
- `ext::sh -c "touch marker"` as a URL is refused by the allowlist even when `~/.gitconfig` sets
  `protocol.ext.allow=always`; the marker file is never made. A `--upload-pack=...` URL is a `-` refusal.
- The fetch is given `--` before the URL, `GIT_TERMINAL_PROMPT=0` and the allowlist (a fake `git` records its argv and
  environment), and no `GIT_SSH_COMMAND` or `GIT_ASKPASS` is passed even when the caller has them.
- A fixture remote that prompts for credentials (a `file` remote cannot, so a fake `git` that exits as `git` does when
  the terminal prompt is off) fails with the pack's name, quickly, and never waits on a terminal.
- `collide pack update <name>` for a user pack: preview shows the old and new commit and the diff; with `confirm` it
  writes the user's lock and the next load uses the new tree id. For a repo `git:` pack it edits `.collide/packs.lock`
  in the checkout, uncommitted. For a `repo:` or `dev:` pack it is refused with the reason. `--to` takes a branch, a tag
  or a commit.
- Two lines or two boards that pin the same tree id share one cache entry and one mirror.
- The locked pack's name, source, commit and tree id appear in the loaded line, for the fingerprint later.
- `pnpm gen` is run for the new wire method, and `pnpm check` passes.
- No new crate.

## Tests

Write these first, with a fixture remote that is a bare repository in a temp root, reached as a `file` URL, and a
`git` shim script for the cases that need to watch or hang:

- the locator parser as a table (each form above, each refusal);
- the mirror name (safe, bounded, two URLs differ, one URL is stable);
- `fetch`: first fetch of an unknown host (nothing runs), a known host, a path on disk, a branch moved, a tag added, a
  commit pruned, a hang, a prompt, an `ext::` URL with a permissive `~/.gitconfig`, the argv and environment record;
- resolve and pin: a branch, a tag, a commit, `HEAD`, a nested path, a path that is a file, a missing ref;
- the lock: written in order, atomic, read back, a tree id that differs from the fetched one, a commit gone;
- the loader: a `git:` pack with a lock and a primed cache loads without any git; with a cold cache and a known host it
  fetches and loads; with a cold cache and an unknown host it reports `NeedsApproval`;
- `pack update`: preview, confirm, `--to`, a repo pack's lock edit, the refusals; the wire type round trip.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 4a: Pack sources and scope (the sources; the `git:` fetch rules; user packs' lock); Success Criteria (the
    `git:` pack on a local bare repo; the lock whose commit is gone; the `-` URL; the new host; the timeout); Tests (the
    fetch rules against a fixture remote that prompts, a hanging remote and a URL that starts with `-`).
  - 0.7: Pack sources (the `git:` locator and fetch rules; the lock); Switching packs (`collide pack update`).
  - 0.9: Trust, per source. S4 record: the tree id as the lock value.
- Carried from `ts-0026-es`: the `git:` locator grammar `<url>//<path>@<ref>`, fetching, and comparing a pin with a
  fetched tree.
- **Decision for the owner, with a recommendation: where the new-host question is asked.** The plan says a new host asks
  first, and 0.9 says trust is asked on open. Recommend one question for each moment the host is first needed: the CLI
  asks inline on `collide pack add` and `pack update` (a user pack), and for a repo pack that a clone has not fetched
  yet, the trust prompt of `ts-0102-vu` lists the hosts it would contact and records them when the person says yes, so
  the person answers once. A load never fetches from a host that is not yet recorded. Alternative: a separate `collide
  pack fetch` command. Recommend against it: it adds a step a person must know.
- **Decision for the item, with a recommendation: what is fetched.** Every branch and tag. A locked commit that is on a
  branch is then present, and `collide pack update --to <commit>` works for any reachable commit. Alternative: fetch
  only the wanted ref, which fails for a commit that is not a branch tip on servers that refuse commit fetches.
  Recommend all branches and tags: it is a mirror.
- **Decision for the item:** the locator grammar is the plan's `<url>//<path>@<ref>`. For a URL that itself holds `//`
  (every URL with a scheme), the first `//` after the scheme's own is the separator; the rules are in the Goal and the
  table test fixes them. A repo whose path holds `//` is refused.
- **Decision for the item:** `config.pack_update` is new wire (params, result, `pnpm gen`), because the plan names
  `collide pack update` (0.7) and a success criterion uses it, and `owner::config` has no method for it.
- **Decision for the item:** the known-hosts file is `~/.config/collide/hosts.yaml`, a list of host names, written by
  `yaml_out`. A host is recorded when the person says yes and never removed by Collide.
- The hardening of `ts-0094-av` (a cleared environment and no user git config) is for reading; this item's fetch needs
  the user's credentials and builds its own environment, listed above.
- Area: `core/src/lock.rs` (locator), `core/src/owner/config.rs` (`config.pack_update`), `host/src/git_fetch.rs` and
  `pack_lock.rs` (new), `host/src/board_load.rs`, `host/src/hosts.rs` (new), `host/src/commands.rs` (the `pack update`
  subcommand only; the rest of `pack` is `ts-0101-bd`), `stub-tags.json` (the `cli: pack` tag moves from stage 4b to
  4a here, where `commands.rs` is first touched; `ts-0101-bd` fills it), `host/tests`, `fixtures/locks`.
- The test rule: no test touches the real home, real launchd or a real daemon. Each test clears the environment
  (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as `host/tests/common/mod.rs` `TempRoot`
  does. Settings files are fixtures, never the owner's own `settings.json`.
- Stage 4a item (revision 23 scope, split 2026-11-16). Items of Stage 4a run one at a time, in id order, because
  they share `core`, `host` and the generated files.
- Owner decisions (2026-11-16, at the split): the recommendations above are taken under the owner's delegation; they
  stay overridable until the item is built.

### Built

Tests were written before the code. Red seen (by a compile error or a failing run before the code existed):
`owner_protocol` (no `Method::ConfigPackUpdate`), `hosts`, `git_fetch` and `pack_lock` (modules missing), and the
`board_load` fetch tests (`with_fetch` and `needs_host` missing). **Not seen red:** `git_locator` and `pack_update`
(the `command` module included) were written first but first run after their code existed, so they have no red record.

- **Locator** (`core/src/lock.rs`): `GitLocator` (`parse`, `reference`, `target`), `LocatorError`, `UrlTarget`
  and `url_target`, `SourceError::BadLocator`. `PackSource::parse` uses it for `git:`.
  Tests: `core/tests/git_locator.rs`.
- **Wire** (`core/src/owner/config.rs`, `owner.rs`, `schema.rs`): `config.pack_update` with `ConfigPackUpdateParams`,
  `ConfigPackUpdateResult`, `PackPin`, `PackFileChange`, `PackFileChangeKind`. `pnpm gen` was run (`schemas/`,
  `packages/pack-sdk/src/generated/`); `app/src/owner/methods.ts` has the method. Round trip: `owner_protocol`.
- **Hosts** (`host/src/hosts.rs`): `KnownHosts` (`is_known`, `record`), `~/.config/collide/hosts.yaml`, mode 0600.
- **Fetch** (`host/src/git_fetch.rs`): `mirror_name`, `mirror_path`, `FetchEnv`, `FetchError`, `GitFetch::fetch`,
  `FETCH_TIMEOUT`. Tests: `host/tests/git_fetch.rs` (new host runs nothing, known host, path on disk, branch moved, tag
  added, commit pruned, hang, prompt, `ext::` with a permissive `.gitconfig` in the temp HOME, argv and environment).
- **Pins and lock** (`host/src/pack_lock.rs`): `resolve_pin`, `read_lock`, `set_pin`. Tests: `host/tests/pack_lock.rs`.
- **Loader** (`host/src/board_load.rs`): `FetchSetup`, `LoadContext::with_fetch`, `LoadProblem::needs_host`,
  `pack_from_mirror`. A load reads the lock, never the remote; the mirror is read only on a cache miss. Tests are
  the fetch tests added to `host/tests/board_load.rs`.
- **Update** (`host/src/pack_update.rs`, `config_get.rs`, `daemon.rs`): `pack_update`, `ConfigService::pack_update`,
  the `Method::ConfigPackUpdate` handler, and `system_load_context` now takes the home so the daemon can fetch.
  Tests: `host/tests/pack_update.rs` (preview, confirm leaves the lock uncommitted, next load uses the new tree,
  other pins keep their place, `--to` branch/tag/commit, older tag, missing ref, same pin, refusals, user pack,
  new host, approved host, other host).
- **Command** (`host/src/pack_cli.rs`, `bin/collide.rs`): `collide pack update <name> [--to <ref>] [--yes]
  [--allow-host <host>]`; the `command` module in `pack_update.rs` and two tests in `command_tree.rs` cover it.
- **Retag** `cli: pack` from 4b to 4a: `commands.rs`, `stub-tags.json`, `checks/tests/real_tree.rs`,
  `host/tests/command_tree.rs`. The other `collide pack` words stay the stub (exit 6).
- Test support: `host/tests/common/remote.rs` (a bare remote on disk), `Fixture::fetching` in `config_fixture.rs`.

### Decisions for the owner

1. **New host over the wire.** The daemon returns `needs_host` in the result (no RPC error), and a caller that has
   the person's yes sends `approve_hosts`; only the host the fetch needs is recorded. Alternative: an RPC error
   code. Recommend the result field: it is a normal outcome, not a failure.
2. **Extra refspec `+HEAD:refs/collide/head`** beside branches and tags, so a locator with no ref (`HEAD`) works on
   a remote whose default branch is not known. Alternative: ask the remote with `ls-remote`. Recommend keeping it.
3. **Scheme-less URLs** (`github.com/org/p`) parse but are refused at fetch with a hint to write `https://...`.
   Alternative: guess `https://`. Recommend refusing; a guess is a host the person did not name.
4. **Mirror name hash** is FNV-1a 32-bit (no hash crate, no new crate). Alternative: SHA-1 through git. Recommend
   FNV: it only keeps two URLs apart.
5. **`hosts.yaml`** is a `hosts:` list, lower case, mode 0600, never pruned by Collide.
6. **A pack that fails to load cannot be updated by name**, because the line is loaded first. Alternative: update
   by source. Recommend leaving it until a person hits it; a broken pack is rarely fixed by moving its pin.
7. **A user pack is found** by `packs.yaml`, its lock entry and the name in the manifest at the locked commit in
   the mirror (the loader does not load user packs before `ts-0101-bd`). `ts-0101-bd` may replace this lookup.
8. **CLI flags** `--yes` and `--allow-host` are mine; the confirming call names the commit that was shown, so a
   remote that moves in between changes nothing.
9. **Timeout** kills only git's direct child, and is told from other failures by the message prefix `run_bounded`
   gives. Alternative: a process-group kill and a typed error from `run_bounded`. Recommend a later item.
10. **Credentials in a URL** are not redacted in messages. Recommend a later item; `https://user:token@host` is
    rare and the plan names the user's git credentials, not URL secrets.
11. **Plan text to propose** (0.7, Switching packs): "`collide pack update <name> [--to <ref>] [--yes]` previews the
    old and new commit and the changed files, and writes the lock only on a yes; a host never fetched from is asked
    about first." Stage 4a: `cli: pack` is filled in two items, `update` in `ts-0100-pu`, the rest in `ts-0101-bd`.

- Owner decisions (2026-11-16, at the build): 1 to 5, 7 and 8 as built, on delegation. 6 and the cold-cache host case
  carried to `ts-0101-bd` and `ts-0102-vu` as notes there. 9 as built, carried in `ts-0102-vu`'s note. 10 changed at
  the review: a `git:` URL with a password is refused (`LocatorError::Password`, its message masks the password as
  `***`), because a fetch uses the person's git credentials and the locator is committed in the lock; tests in
  `core/tests/git_locator.rs`. 11 in the plan's 0.7 Switching packs text.

### Not done, by design

- User packs, `pack add/remove/switch/list/link/enable/disable`, name clash, `dev:` rules, working copies: `ts-0101-bd`.
- The daemon's lines, the trust prompt and asking for a new host in the app: `ts-0102-vu`.
- A user pack's "next load uses the new tree" is asserted on the user's lock file only, since the loader does not
  load user packs until `ts-0101-bd`.
- Trust for the new tree is asked by the existing trust flow on the next open; this item does not call it.
