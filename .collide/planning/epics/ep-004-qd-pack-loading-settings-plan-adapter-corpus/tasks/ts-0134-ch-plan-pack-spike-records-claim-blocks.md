---
id: ts-0134-ch
kind: task
status: ready
title: "Plan pack: spike records, claim blocks and the Pre-registration part"
owners: []
links:
  - { type: blocked-by, to: ts-0133-kg }
---

## Goal

The provider parses a spike record's fenced `claim` blocks and its Pre-registration part, reports their problems, and a
view counts them.

What exists. From `ts-0126-nf` a spike lists with its status and body; nothing reads the body. Collide's ten records are
the older shape, with no claim blocks and no Pre-registration. From `ts-0131-jh`, `Task.part_problems` is on the wire
and the shared entry parser exists. `ViewReport` and `ViewEntry` (`core/src/owner/task.rs`) count `errors`,
`invalid_items` and `error_count`, filled by `core/src/view_eval.rs`. `docs/research/planning.md` 4.3 shows a claim
block and the part's labels.

What this adds.
- `packs/plan/src/claims.ts` (new): in a spike record, a fenced block whose info string is `claim` is YAML, read with
  the `yaml` package in core mode (S5: never 1.1, no `Date`, duplicate keys and the alias limit on), with a cap on a
  block's size. Fences are matched by length, so a block shown inside a longer fence (as the plan's own example is) is
  text and not a claim. Keys: `id` (`Q<n>.c<n>`), `label` (`measured`, `inferred` or `unmeasured`), `says`, and the
  optional `value`, `unit`, `tolerance`, `repro`, `output`, `cost_usd`, `verifiable` (`yes` or `no`) and `load_bearing`
  (a flag). An unknown key, a missing `id` or `says`, a bad `id`, a bad `label`, a repeated `id`, a YAML error and an
  oversize block are each a problem, with the line of the block, in `part_problems` under the part `claims`.
- `Task.claims` (additive, spikes only): `id`, `label`, `load_bearing`, `verifiable`, for the planning view's tables and
  Stage 9's verify gate. The prose is never compared.
- The Pre-registration part: a `## Pre-registration` heading with the labels `Hypothesis:`, `Cheapest test:`,
  `Kill criterion:` and `Budget:`, and `### Amendment <date>` sections below the original. A missing label, or an empty
  kill criterion, is a problem. `Task.preregistration` (additive): the four texts, the number of amendments, and
  `hash_matches` when frontmatter has `preregistered`. The hash is over the original text, up to the first amendment, so
  an amendment keeps it true and an edit of the original does not.
- A view counts the problems of its members: `ViewReport.part_problems` and `ViewEntry.problem_count` (additive), and
  `collide tasks --view` prints the count on stderr as it does the other counts (exit 0).

## Success criteria

- A record with a claim block missing its `id`, one with `label: guessed`, and a repeated claim id lists, each with a
  problem and its line, and the record is still an item with its status.
- A malformed claim block is a counted parse error on the view: a view over spikes reports the count in
  `part_problems`, and a record with none adds nothing (the stage's criterion).
- A claim block inside a four-backtick fence is not parsed; one with a duplicate key, an alias bomb or an unquoted `no`
  behaves as S5 says; a block over the cap is a problem.
- Collide's ten records read with no problem and no `claims`.
- A record with a Pre-registration part gives the four texts; an amendment below it keeps `hash_matches` true; a
  change to the original makes it false; a frontmatter hash that reads as a number (`1e10`) is a `task.invalid`, as
  any number field is.
- A record with no Pre-registration part has no `preregistration` key and no problem.

## Tests

Write these first.

- Parser fixtures for claim blocks: every key and value, each problem, nested fences of two lengths, an indented fence,
  a block with no closing fence, tabs, CRLF, and the S5 hazards (an unquoted `no`, a hash like `1e10`, a duplicate key,
  an alias bomb, a block over the cap).
- Fixtures for the Pre-registration part: complete, missing labels, one amendment, two, an original edited after the
  hash, `preregistered` unquoted.
- Provider tests for `toTask` on a spike; a view test in `core/tests` over a corpus of spikes with problems, and one
  over a corpus with none, for the count; a daemon `task.list` test with an inline view.
- A test that the CLI prints the count (the daemon path of `ts-0120-wm`).

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 4c: Planning ("`collide/plan`'s record parser (claim blocks)"); Quality parts ("a spike's Pre-registration
    part"); Success Criteria ("a malformed claim block is a counted parse error on the view"); Tests (S5's hazards;
    "parser fixtures for each part and the Pre-registration part").
  - 0.10: Planning sources ("A `spike`"; its pre-registration; frontmatter parsing with `yaml`). 0.8: Task. Revision 18.
  - `docs/research/planning.md` 4.1 to 4.4 (the format and the claim block). Spike S5 for the hazards.
- **Owner's choice, with a recommendation: how a bad claim block shows.** The plan says "a counted parse error on the
  view". Recommend the record stays an item, with `part_problems`, and the view counts them: the owner sees the record
  and a count. Alternative: the record is a `task.invalid`, which the view counts in `invalid_items` as it counts a bad
  file. Smaller, but one bad block hides the whole record, status and all, from every view. This sets what the owner
  sees for a record with a typo.
- **Decision for the item, with a recommendation: the hash.** The part's hash is the first 16 lowercase hex characters
  of the SHA-256 of the original text (from the heading line to the line before the first `### Amendment` or the next
  `##`, LF line ends, trailing blank lines dropped), written quoted. Approving, which writes it, is Stage 9; this item
  only reads it. Alternative: a git blob hash. It needs git for a pure read. Node's `crypto` is built in, so no
  dependency is added.
- **Decision for the item: only spikes are searched for claim blocks.** A proposal or question that shows a block in its
  prose is not parsed.
- **Decision for the item: the count is of problem entries**, not of records, so two bad blocks in one record count two.
- Area: `packs/plan/src/claims.ts` (new), `preregistration.ts` (new), `parts.ts`, `scan.ts`, `provider.ts`,
  `core/src/provider.rs`, `core/src/owner/task.rs`, `core/src/view_eval.rs`, `host/src/daemon.rs` (the view's entry),
  `host/src/task_daemon.rs` (the count), the generated schemas and TypeScript, `packs/plan/test/`, `core/tests/`.
- The test rule: no test touches the real home, real launchd or a real daemon. Each test that runs the `collide` binary
  clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as
  `host/tests/common/mod.rs` `TempRoot` does. Settings files are fixtures, never the owner's own `settings.json`. A temp
  repository sets `GIT_CONFIG_GLOBAL` and `GIT_CONFIG_SYSTEM` to empty files.
- Stage 4c item (revision 23 scope, split 2026-11-18). Items of Stage 4c run one at a time, in id order, because they
  share `core`, `host`, `packs/plan` and the generated files.
- Nothing here runs a real `claude`. No dependency is added.
- Owner decisions: none recorded yet. The recommendations stand until the owner answers at the split.
