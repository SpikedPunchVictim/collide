---
id: ts-0134-ch
kind: task
status: done
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
- Owner decisions (2026-11-18, at the build): the item's choices as it recommends (a bad block leaves the record an item
  and the view counts its problems; the hash as written; spikes only; the count is of entries), and the builder's
  decisions 1 to 11 as built, on the owner's delegation, including 9 (the Pre-registration part is read by its own label
  reader, not `readPart`, because `readPart` reads entries and flags unknown labels, which would flag prose). The
  builder's plan text (decision 12) is applied to 0.8, 0.10 and the Stage 4c criterion. At review four mutations were
  each caught: the hash taken over the amendments, the view counting records not entries, a snake-case `loadBearing` on
  the wire, and the view counting an excluded item's problems.

### Built

- `core/src/item.rs`: `Claim` (`id`, `label`, `loadBearing`, `verifiable` left out when none) and `Preregistration`
  (`hypothesis`, `cheapestTest`, `killCriterion`, `budget`, each left out when absent; `amendments`; `hashMatches`
  left out when the frontmatter has no `preregistered`). `core/src/provider.rs`: `Task.claims` (left out when empty)
  and `Task.preregistration` (left out when none). `core/src/schema.rs` registers both. `pnpm gen` rewrote
  `schemas/` and `packages/pack-sdk/src/generated/`. The task literals in `core/src/testing/provider.rs` and
  `host/src/scratch.rs` take the two new fields.
- `core/src/owner/task.rs`: `ViewReport.part_problems` and `ViewEntry.problem_count`, both left out when 0, so a
  clean view serializes as before. `core/src/view_eval.rs`: `ViewCounts.part_problems` counts the problem entries of
  the view's members, a member once however many paths hold it. `host/src/daemon.rs` `view_entry` fills
  `problem_count`. `host/src/task_daemon.rs` `tasks` prints one stderr line when the count is above 0 and exits 0.
- `packs/plan/src/claims.ts` (new): `readClaims`. `packs/plan/src/preregistration.ts` (new): `readPreregistration`.
  `read-file.ts` exports `YAML_OPTIONS` and `MAX_ALIAS_COUNT`, which the frontmatter reader and the claim reader
  share. `scan.ts`: `spikePartsOf`, `qualityPartsOf` for spikes, `toTask` fills `claims` and `preregistration`.
- Tests, `packs/plan/test/claims.test.ts` (44) and `packs/plan/test/preregistration.test.ts` (31): every key, each
  problem, nested fences of both characters and of lengths 4 and 5, an indented fence, an unclosed fence, tabs, CRLF,
  the S5 hazards (unquoted `no`, `1e10`, duplicate key, alias bomb, merge key, cap), the hash, amendments, an edited
  original, a number-valued `preregistered`, list equals get, only spikes searched, and Collide's own spikes (the
  copied fixture and the real folder) with no problem and no `claims`.
- Rust tests: `core/tests/provider_protocol.rs` (`a_spike_carries_claims_and_a_preregistration_and_they_round_trip`,
  `a_task_with_no_claims_or_preregistration_serializes_neither_key`,
  `a_preregistration_with_only_its_amendments_keeps_no_text_and_no_hash_key`,
  `a_claim_or_a_preregistration_with_a_missing_extra_or_snake_case_key_is_refused`,
  `claims_and_preregistration_are_in_the_task_schema`), `core/tests/view_eval.rs` (4: counts each entry of its
  members, ignores a task the view does not hold, adds nothing when clean, a member at two paths counts once),
  `core/tests/owner_protocol.rs` (2: the two counts round trip and are left out at 0; both are in the schemas and not
  required), `host/tests/daemon_corpus.rs`
  (`a_view_over_spikes_with_part_problems_reports_their_count_in_task_list_and_task_views`),
  `host/tests/task_daemon_cli.rs` (3: the stderr line and exit 0, a clean view says nothing and `--json` carries the
  count, `task --json` of a spike carries `claims` and `partProblems`). The fixture provider
  `host/tests/fixtures/corpus_pack_provider.mjs` passes `partProblems` through.
- Seen red at runtime: with `readClaims` and `readPreregistration` stubbed to return nothing, 36 of 44 claim tests
  and 26 of 31 pre-registration tests failed (the rest assert that nothing is there). With the view count stubbed, the
  4 `view_eval` tests failed. With `view_entry` leaving `problem_count` at 0, the daemon test failed on the
  `task.views` wait; with no stderr line, both CLI view tests failed. Mutations reverted: hashing the whole part
  (5 failed), not dropping trailing blank lines (9), any three-character fence closing a claim (2), no repeated-id
  check (2), counting a member at two paths twice (1). The wire tests in `provider_protocol.rs` and
  `owner_protocol.rs` were written against the finished types and not seen red by a mutation.
- App test doubles under `app/test/support/` need no change: every new key is optional on the wire.

### Decisions for the owner

1. **A bad block is not listed in `claims`.** Only a block with no problem is a claim. Alternative: list a block that
   has only soft problems. Recommend as built: a half-read claim is worse than none.
2. **One problem entry per violation**: each unknown key, and each fault of one block. A problem carries the line of
   the block's opening fence. The count in a view is of these entries.
3. **Cap**: 8 KiB per block, checked before parsing. Ids are `Q<n>.c<n>` counting from 1, like parts.
4. **Wrong types on `value`, `unit`, `tolerance`, `repro`, `output`, `cost_usd`** are problems, not just the carried
   keys. Alternative: check only what is carried. Recommend as built: the YAML hazard of an unquoted `1e10` or `no`
   is the same for all.
5. **`verifiable`** is `yes` or `no` in the file and an optional boolean on the wire; `loadBearing` defaults to
   false. An unclosed `claim` fence is one problem and is not read. A repeated id is a problem on the later block,
   and an id is used by any block with a well-formed id, even a refused one.
6. **Claims and the Pre-registration part are read for spikes only.**
7. **Label syntax**: a label is at column 0, in any case, and its text runs over following lines up to a blank line,
   a heading, a fence or the next label. Only an empty Kill criterion is a problem (as well as a missing label);
   an empty Hypothesis, Cheapest test or Budget is not. The first duplicate label wins, silently. A second
   `## Pre-registration` is a problem and is not read. The amendment date is not checked.
8. **Hash**: the part's lines from the heading to the last non-blank line, joined by LF, no final line end (so CRLF
   and trailing blank lines do not change it). `hashMatches` is exact string equality, so an upper-case or non-string
   hash gives false. Stage 9 will need this function; it is not exported on the wire.
9. **`readPart` from `parts.ts` is not reused** for the Pre-registration part, against the brief. It reads entries
   (`- **D1 title**:`) and reports an unknown label as a problem, which would flag prose such as `Stop rule:`. The
   fence helpers and `PartProblem` are shared. Alternative: add an entry-less, loose mode to `readPart`. Recommend as
   built until a second label-style part exists.
10. **Problem part names** are `claims` and `Pre-registration`.
11. **`part_problems` and `problem_count` are left out at 0**, so a clean view is byte-for-byte as before. The CLI
    prints its own stderr line, not folded into the "could not check" line, because an error is not a problem.
12. **Plan text to propose**, 0.8, Task: `claims` and `preregistration` as above, and `ViewReport.part_problems` and
    `ViewEntry.problem_count`. 0.10, A spike: the claim keys, the id form, the cap, the hash rule and the label rules.

### Not done, by design

- Showing `claims`, `preregistration` or the problem counts in the CLI list output or the app: `ts-0136-rr` and
  Stage 6a.
- Writing a claim or an amendment, and any check that a claim has a repro or that the hash matches: Stage 9 and
  Stage 8.
- Pre-registration for kinds other than spikes.
