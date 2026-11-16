---
id: ts-0103-sa
kind: task
status: ready
title: "Repository instructions: measure the launch line"
owners: []
links:
  - { type: blocked-by, to: ts-0102-vu }
---

## Goal

Answer the question 0.3 leaves open before `ts-0104-gx` builds the change (revision 21; revision 22): **which launch
line loads the worktree's own instructions (its `CLAUDE.md` files and skills) while settings, hooks and MCP config stay
from the snapshot?** The plan says 4a measures it first: "with the fake and then once with the real `claude`".

Under today's line (`--setting-sources user`, `--settings`, `--mcp-config`, `--strict-mcp-config`) nothing from the
worktree loads (measured S1). The change must bring in only the instructions. No product code changes here: the output
is measurements, test harness code, and proposed plan text in this item.

**Part 1, with the fake** (a build agent; no `claude`, no network, no spend). Build the candidate launch lines as data
in a test and run each against the fake `claude` (`host/tests/adapters/claude/fake_claude.rs`), which records argv,
environment and working directory. Candidates, from `claude --help` and the plan's S1 and S9 notes; the real step
decides between them:

1. the current line with the working directory in the worktree (the baseline);
2. the baseline plus `--add-dir <worktree>`, and the environment variable that makes Claude Code read `CLAUDE.md` from
   added directories, if the installed `claude --help` or its documentation names one (the measurement finds out; do not
   assume it exists in 2.1.325);
3. the baseline with the instruction files' text read by Collide and passed as an appended system prompt, which
   loads `CLAUDE.md` text but neither skills nor `@` imports;
4. `--setting-sources user,project` with the snapshot's `--settings`: loads project settings and hooks, so it is here
   only to record that it fails the rule.

For each, a test records what Collide would pass and proves, for every candidate except 4, that the four snapshot flags
are still present and that no project settings path is named. The tests are the harness the real step reuses.

**Part 2, one real run, after the owner's approval** (see `### Owner approval`). In a temp repository (stand-in names
only), with canary text in each file below, run the candidate lines that Part 1 leaves with `claude` (haiku, one short
prompt each, the same gated harness as `ts-0070-mk`: `COLLIDE_REAL_CLAUDE=1`), and record from the stream's `system`
`init` message and the model's reply what loaded:

- a root `CLAUDE.md`, a nested `CLAUDE.md` in a subfolder, an `@import` inside one, and a project skill in
  `.claude/skills/<name>/SKILL.md`: should load;
- `.claude/settings.json` with a hook that writes a marker, `.claude/settings.local.json`, `.mcp.json` with a server
  that would write a marker, and `.claude/agents/<name>.md`: must not load, and the marker files must not appear;
- **a session started at the person's checkout** (revision 22): the root, with an untracked
  `.claude/settings.local.json` and `.mcp.json`, started with the snapshot's flags: loads none of them;
- **a `claude` that the agent starts itself**, inside a worktree it made with `git worktree add` (revision 22): which of
  the settings, hooks, MCP servers and instructions of that worktree it loads. The snapshot's flags reach only the
  session Collide starts, so the expected answer is "everything"; the measurement says so or not.

## Success criteria

- Part 1: each candidate has a test that names it, records the argv, environment and working directory the fake saw, and
  asserts the snapshot flags (`--setting-sources user`, `--settings <snapshot>/settings.json`, `--mcp-config
  <snapshot>/mcp.json`, `--strict-mcp-config`) are present in 1 to 3 and that no `<worktree>/.claude/` path is passed.
  Candidate 4 is asserted to fail that rule.
- The fake run proves the harness: the canary files exist in the worktree and the fake sees the same working directory.
- Part 2, only with the owner's approval recorded below: a table in this item with, for each candidate and each file
  above, `loaded`, `not loaded` or `not run`, with the `claude` version and the stream line that shows it; the spend
  against the cap; the marker files' absence; and the three questions of revision 22 answered (the person's checkout; an
  agent's own `claude` in its worktree; whether auto memory stays off under the chosen line).
- A recommendation for the line `ts-0104-gx` builds, or the statement that no tested line does it and what that means
  for the plan (for example "skills do not load, CLAUDE.md does"), with proposed text for 0.3 ("Unmeasured: which launch
  line loads ...") and for the Context options. Proposed, not applied.
- Nothing from the owner's real home is read or changed by a test, except what `claude` itself writes under its own
  state (reported, as `ts-0070-mk` did); the temp repository and the harness are removed at the end.
- No product code or plan text changes. No new crate.

## Tests

Write these first (Part 1), in a new test file under `host/tests/adapters/claude/` using the existing fake and canary
helpers:

- one test per candidate, as above; the baseline is first and equals `launch_spec`'s own output for a role with no
  options;
- a test that the harness' worktree holds the canary files and that the fake's recorded working directory is that
  worktree;
- a test that candidate 4 is refused by the rule (it names `user,project`);
- the Part 2 steps as gated tests (`COLLIDE_REAL_CLAUDE=1`), written so that without the variable they say they are
  skipped and why, as `real.rs` does. They are not run until the owner approves.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 4a: "The repository's own instructions" (measure with the fake, then once with the real `claude`; the revision
    22 measurements).
  - 0.3: The tool's config paths in a repo (the repository's own instructions; "Unmeasured"); Context options
    (`repo_instructions`); Acknowledgement. 0.4: where an agent starts (the root session, revision 22). Revision 21 and
    revision 22: the worktrees an agent makes for itself.
  - S1 record: what loads under the snapshot line. `ts-0070-mk`: the gated real harness and its approval terms.
- The tested version range and the `claude` binary choice (`COLLIDE_REAL_CLAUDE_BIN`) are as `ts-0070-mk` left them.
- **Decision for the owner, with a recommendation: the real run.** It spends money and runs `claude`. Recommend the
  terms of `ts-0070-mk`: haiku, a per-step cap and a total of $0.50 (two to four short sessions), run once, in temp
  repositories made for it, with the `claude` child allowed the real `HOME` for its login only and Collide's own `HOME`
  and daemon root in a temp folder. The agent that builds this item does not run Part 2 until `### Owner approval` holds
  the owner's yes, the cap and the date.
- **Decision for the item:** if the candidates fail (nothing loads instructions without loading project settings),
  record that and stop. The recommended fallback, which `ts-0104-gx` then builds, is candidate 3 (CLAUDE.md text as
  appended system prompt) with skills not loaded, and a note on the Roles page later.
- **Decision for the item:** the temp repository's files use stand-in names (Orchard, a folder `docs/guide.md`), never
  the real repository's.
- The measurement can show that the chosen line also loads the user's own `CLAUDE.md` or auto memory; record it, since
  0.3 says the user's own stay on and auto memory is off by the snapshot's `autoMemoryEnabled: false`.

### Owner approval

Given by the owner on 2026-11-16, on the terms of `ts-0070-mk`: `haiku`, a cap on each step, $0.50 in all, a fresh
temp repository, and the `claude` child may use the real `HOME` for its login only. The build agent records here
which candidates were run and what was spent.

- Area: `host/tests/adapters/claude/` (the harness and the gated steps), the item's own Notes. No change in `host/src`.
- The test rule: no test touches the real home, real launchd or a real daemon. Each test clears the environment
  (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as `host/tests/common/mod.rs` `TempRoot`
  does. Settings files are fixtures, never the owner's own `settings.json`.
- Stage 4a item (revision 23 scope, split 2026-11-16). Items of Stage 4a run one at a time, in id order, because
  they share `core`, `host` and the generated files.
- Owner decisions (2026-11-16, at the split): the recommendations above are taken under the owner's delegation; they
  stay overridable until the item is built. Part 2's real run is approved (see `### Owner approval`).
