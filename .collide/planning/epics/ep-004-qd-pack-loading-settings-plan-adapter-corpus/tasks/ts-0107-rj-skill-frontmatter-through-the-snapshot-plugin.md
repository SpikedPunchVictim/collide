---
id: ts-0107-rj
kind: task
status: active
title: "Skill frontmatter through the snapshot plugin: measure"
owners: []
links:
  - { type: blocked-by, to: ts-0104-gx }
---

## Goal

`ts-0104-gx` built the `worktree` line and the skills plugin of `ts-0106-nr`, with the plugin off the line
(`PLUGIN_ON_THE_LINE` in `host/src/adapters/claude/instructions.rs`). Its gated session could not answer whether a
skill's own frontmatter takes effect through the plugin: the model's `Skill` calls were refused with a permission error,
because the session's harness closes stdin and answers no permission request (its Measured section). Plan 0.3 says that
what runs code or grants access without the model asking comes from the snapshot, so a repository skill's `hooks` (they
run code) and `allowed-tools` (they skip a permission request) matter.

**One gated session, approved** (see `### Owner approval`), with a harness that answers permission requests: the host
allows `Skill` and refuses every other tool. The fixture is the one of `ts-0104-gx` (a skill in the temp worktree's
`.claude/skills/` whose frontmatter has a `hooks` entry that writes a marker in the temp root and an `allowed-tools`
entry for `Write`), on the line `frontmatter_line` builds (Collide's line, with the plugin named by hand while the seam
is off). Record from the stream and the temp root:

- the skill's listing, and whether `Skill` ran (a `can_use_tool` request for it, answered allow);
- whether the hook's marker was written (the frontmatter hook ran);
- whether `Write` ran without a `can_use_tool` request (`allowed-tools` took effect), or a request for it came and
  was refused (it did not);
- the spend.

**The outcome.** If neither the hook nor `allowed-tools` took effect, set `PLUGIN_ON_THE_LINE` to `true` (the tests of
`ts-0104-gx` follow the seam) and propose the plan text that says so. If either took effect, the plugin stays off, the
finding is recorded, and the owner chooses: strip `hooks` and `allowed-tools` from a per-phase copy of the skills, or
accept them as plain `claude` would. If the session again cannot tell (the skill never runs), the plugin stays off and
the item says why.

## Success criteria

- The gated test, through a path that checks the opt-in (`real::unavailable()`) and then the ledger
  (`real::check_ledger_within`) before anything starts, as `run_session_within` does. It is skipped without them and
  says why.
- A table in this item of what the session showed, with the `claude` version, the stream line for each finding and
  the spend, under `### Measured`.
- `PLUGIN_ON_THE_LINE` set by the rule above; the item's tests and the full gates pass either way.
- Nothing from the owner's real home is read or changed by a test, except what `claude` itself writes under its own
  state (listed in the report, none deleted). The temp roots are removed. No new crate.

## Tests

- An offline test (no `claude`) that the session's answer function allows `Skill` and refuses every other tool, seen
  red at runtime.
- An offline test that the gated session's line is `frontmatter_line`'s, with haiku, the cap, config none, the plugin
  and the variable (`ts-0104-gx` has one; extend it if the harness changes).
- The gated test. It is seen red only with its gates in place, or reported as unseen.

## Notes

- IMPLEMENTATION_PLAN.md: 0.3, The repository's own instructions ("The line"); Stage 4a Status.
- Builds on `ts-0104-gx` (`repo_instructions_live.rs`: `frontmatter_line`, `add_frontmatter_skill`,
  `describe_frontmatter`, `FRONTMATTER_*`) and on `real.rs` `run`, whose `Case::answer` answers permission requests.
  `real::run` checks the ledger but not the opt-in: the session's path must check both.
- Added by the owner on 2026-11-17, after `ts-0104-gx`'s session could not tell. Built before `ts-0105-jv`, so the
  owner's contract run checks the line as it ships.

### Owner approval

Given by the owner on 2026-11-17: one gated session with `haiku`, `--max-budget-usd 0.05` on the line, and a fresh
ledger with a limit of $0.05, so there is no re-run ($0.005333 was spent in `ts-0104-gx`; both items together stay
inside its $0.10). The terms of `ts-0103-sa`: config none (`--setting-sources ""`), a fresh temp repository, the
`claude` child may use the real `HOME` for its login only, and nothing from the owner's account or home is written down.
No guard is stubbed.
