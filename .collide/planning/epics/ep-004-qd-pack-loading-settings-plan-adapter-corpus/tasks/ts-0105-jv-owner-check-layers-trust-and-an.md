---
id: ts-0105-jv
kind: task
status: ready
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

### Owner checklist

Not written yet. The build agent writes it after `ts-0104-gx` is built, from the code and the Built sections of the
items above, and marks each command as not run by it.
