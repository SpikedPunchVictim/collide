---
id: ts-0106-nr
kind: task
status: ready
title: "Repository skills: measure a snapshot plugin"
owners: []
links:
  - { type: blocked-by, to: ts-0103-sa }
---

## Goal

Revision 21 has a phase read the repository's skills from the folder it starts in. `ts-0103-sa` found that no tested
line loads `.claude/skills/` without also loading the worktree's settings and hooks. The owner chose (2026-11-16) to
measure one more route before `ts-0104-gx` builds the line: **a plugin in the snapshot whose skills are the worktree's
own, live.**

The route: Collide writes a plugin folder in the snapshot (a `.claude-plugin/plugin.json` and nothing else of its
own) whose `skills` entry is a symlink to `<the folder the phase starts in>/.claude/skills`, so the skills are read
from the worktree and are not frozen at the line's commit. The phase's line is the one the owner chose in
`ts-0103-sa` (candidate 2: `--add-dir` and `CLAUDE_CODE_ADDITIONAL_DIRECTORIES_CLAUDE_MD=1`) plus `--plugin-dir
<that plugin folder>`. The plugin has no hooks, MCP servers, agents or commands, so it adds nothing that runs code.

No product code changes here: the output is test harness code, measurements and proposed plan text in this item.

**Part 1, with the fake.** In the `ts-0103-sa` harness (`host/tests/adapters/claude/repo_instructions.rs`), build the
plugin folder and the line as data and run it against the fake `claude`. Assert the four snapshot flags are still
present, no `<worktree>/.claude/` settings path is passed, the plugin folder holds only the manifest and the symlink,
and the symlink resolves to the worktree's `.claude/skills`. If the installed `claude --help` names no
`--plugin-dir`, record that and stop: the route is not open.

**Part 2, one real session, approved** (see `### Owner approval`). In the `ts-0103-sa` fixture (stand-in names), run
the line once with `claude` (haiku) and record from the `system` `init` line and the reply:

- whether `repo-canary-skill` is in the `init` line's `skills`, and under what name (a plugin may prefix it);
- whether the model can give the skill's word from its description;
- that the root `CLAUDE.md` and its import still load, and the nested one does not (as in `ts-0103-sa`);
- that no settings hook, local hook, MCP server or agent of the worktree loads, and no marker file appears;
- that auto memory stays off.

## Success criteria

- Part 1: a test for the plugin line that records what the fake saw and asserts the rules above; a test that the
  plugin folder holds nothing but the manifest and the symlink.
- Part 2: a table in this item like `ts-0103-sa`'s for the plugin line, with the `claude` version, the stream line that
  shows each finding, the spend against the cap and the marker files' absence.
- A recommendation: either `ts-0104-gx` adds the plugin to the `worktree` line, or skills do not load and the Roles
  page says so. Proposed text for 0.3 (the line) and Context options. Proposed, not applied.
- Nothing from the owner's real home is read or changed by a test, except what `claude` itself writes under its own
  state (listed in the item, none deleted). The temp roots are removed.
- No product code or plan text changes. No new crate.

## Tests

Write these first, in the `ts-0103-sa` test files:

- the plugin line through the fake, as above;
- the plugin folder's contents and the symlink's target;
- the Part 2 step as one gated test (`COLLIDE_REAL_CLAUDE=1` and a ledger), skipped without them and saying why, as
  the `ts-0103-sa` live tests are, through `run_session` (which checks the opt-in itself).

## Notes

- IMPLEMENTATION_PLAN.md: 0.3, the repository's own instructions (the line, measured `ts-0103-sa`) and Context
  options (`repo_instructions`); Stage 4a, "The repository's own instructions"; revision 21.
- `ts-0103-sa`: the harness, the fixture, the findings table, and its decisions 3 (config none in every real session)
  and 8 (the session outside the ledger).
- **The guards.** No test, stub or check may remove the opt-in check or the ledger of a gated test, not even to see a
  test fail. A gated test is seen red with its gate in place, or reported as unseen.
- Area: `host/tests/adapters/claude/` and the item's own Notes. No change in `host/src`.
- The test rule: no test touches the real home, real launchd or a real daemon. Each test clears the environment
  (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root. Settings files are fixtures.
- Stage 4a item, added 2026-11-16 by the owner after `ts-0103-sa`. It is built before `ts-0104-gx`, which it blocks.

### Owner approval

Given by the owner on 2026-11-16: one real session with `haiku`, `--max-budget-usd 0.05` on the line and $0.10 in all
(a ledger with that limit; one re-run if a fixture fault needs it), on the terms of `ts-0103-sa`: config none
(`--setting-sources ""`), a fresh temp repository, and the `claude` child may use the real `HOME` for its login only.
Nothing from the owner's account or home is written down.
