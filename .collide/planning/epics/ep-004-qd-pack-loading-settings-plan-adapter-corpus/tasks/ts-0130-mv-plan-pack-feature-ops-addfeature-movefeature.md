---
id: ts-0130-mv
kind: task
status: done
title: "Plan pack: feature ops addFeature, moveFeature and renameSection"
owners: []
links:
  - { type: blocked-by, to: ts-0129-uv }
---

## Goal

The provider writes a proposal's Features part: add a feature, move one to a section, and rename a section across every
proposal that uses it.

What exists. `packs/plan/src/features.ts` parses the part (`parseFeatures`, `SECTION_MAX`) and reports problems with
their lines; `feature-target.ts` and `links.ts` handle `pr-0003-km#F2`; `frontmatter-edit.ts` and `atomic.ts` edit a
file and write it by temp and rename under the root's lock (`lock.ts`); `core/src/section.rs` holds the near-duplicate
rule and `collide sections` lists names. `proposedRefusal` in `write.ts` refuses `proposed` while the part has problems.
No op writes a feature: `OPS` in `write.ts` has `setStatus`, `claim`, `unclaim`, `link`, `create` and `reid`.

What this adds.
- `addFeature { section, title, summary }` on a proposal. `section` is required and may be empty. It takes the next
  `F<n>` (one past the highest in the part) and writes the entry under that section's heading, matched after trimming
  and in any case, or under `### Unsorted`, which is made at the end of the part when it is missing. An empty section is
  `Unsorted`.
- `moveFeature { feature, section }`: moves the entry, with its continuation lines and its number, under the section's
  heading, and makes the heading when it is missing.
- `renameSection { from, to }`, which has no `id`: it edits every proposal whose Features part has a section named
  `from`, renames the heading, and merges the entries into the heading that already has the name `to` when there is
  one. It returns each file in `items[]` and `files`.
- Each op reads the part again after its edit and refuses to write when the result has a problem (the text the problem
  names is in the message). A part that has problems before the op is refused with them, so a write never leaves a
  part worse.
- Titles and summaries are checked (one line for the title, no `**`, a summary of one line or continuation lines), and
  the op declarations carry argument schemas.

## Success criteria

- `addFeature` with an empty section writes under `### Unsorted` with the next number, and creates the heading when it
  is missing.
- `addFeature` into an existing section appends under it; `F` numbers are never reused within the file.
- `moveFeature` keeps the number and the continuation lines and leaves the part without problems.
- `renameSection` of `Pack` to `packs` over three proposals changes the three files, returns them in `items[]`, merges
  into an existing `packs` heading, and leaves each part without problems. A fourth proposal without the section is not
  touched.
- A proposal with a problem in its Features part is refused with the problem and its line, and not written.
- A stale version on any file makes the op return `conflict` with that file's current task, and write none.

## Tests

Write these first.

- Provider tests over proposal fixtures: every case above, a section with a `#` or over 40 characters, an `Unsorted`
  in another case, a part with no heading at all, CRLF line ends, a feature with continuation lines.
- `renameSection` over three, then a mid-way failure injected between files, with each file whole afterwards.
- The existing `features.test.ts` and `feature-links.test.ts` unedited.
- `collide sections --json` before and after a rename (the direct CLI, until `ts-0136-rr`): the near-duplicate group is
  gone.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 4c: Feature ops ("`addFeature {section, title, summary}` ... `moveFeature {feature, section}`; and
    `renameSection {from, to}` ... Each leaves the part without problems"); Success Criteria (`addFeature` with an empty
    section; `renameSection` of `Pack` to `packs` over three proposals).
  - Revision 14 (2026-11-14): features in proposals. 0.10: "A proposal's features" (the part, sections by name,
    near-duplicates). `docs/tasks-and-docs.md` lists the ops with the features.
- **Decision for the item, with a recommendation: a number is "one past the highest in the file".** 0.10 says a removed
  feature's number is not reused. A file cannot show a number that was removed from it. Recommend the highest present
  plus one, and write that limit in the op's description. Alternative: also read the proposal in every ref, as ids do,
  to find the highest ever. Heavier, and still blind to a number removed before any ref held it.
- **Decision for the item: what `renameSection` touches.** Every proposal under the root, whatever its status, so a
  rename never leaves a name half changed. Alternative: only proposals that are not dropped, as `collide sections`
  lists. Then a rejected proposal keeps the old name.
- **Decision for the item: all files or none.** The op reads every file and checks every version first, then writes each
  by temp and rename. A crash between renames leaves each file whole but the rename partly done; the result lists what
  changed. A rename is idempotent, so running it again finishes it.
- Out of scope: the planning view's Sort and merge buttons (Stage 9d), the read-only Features view (Stage 6a).
- Area: `packs/plan/src/features.ts`, `feature-ops.ts` (new), `write.ts`, `frontmatter-edit.ts`, `provider.ts`,
  `packs/plan/test/`.
- The test rule: no test touches the real home, real launchd or a real daemon. Each test that runs the `collide` binary
  clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as
  `host/tests/common/mod.rs` `TempRoot` does. Settings files are fixtures, never the owner's own `settings.json`. A temp
  repository sets `GIT_CONFIG_GLOBAL` and `GIT_CONFIG_SYSTEM` to empty files.
- Stage 4c item (revision 23 scope, split 2026-11-18). Items of Stage 4c run one at a time, in id order, because they
  share `core`, `host`, `packs/plan` and the generated files.
- Nothing here runs a real `claude`. No dependency is added.
- Owner decisions: none recorded yet. The recommendations stand until the owner answers at the split.

### Built

- Layout: `readFeaturePart` in `packs/plan/src/features.ts` is `parseFeatures` with the places of the part as well
  (`FeaturePart`: the `## Features` line, the line after the part, each heading, each entry's first and last line, as
  indexes into the body's lines). `parseFeatures` is a wrapper over it, so the parser is still one machine; the
  problems and the wire `Feature` are unchanged.
- Text edits: `packs/plan/src/feature-text.ts` (new), pure. `addEntry`, `moveEntry` and `renameHeadings` work on a
  body and an `eol` and return the new body or a refusal. Each reads the part first (a part with problems is refused
  with every problem, its line and the text of that line; a body with no `## Features` part is refused), edits whole
  lines, and then reads the new part again: a problem, or features that are not the ones the edit means, refuse. A line
  that is not touched keeps its own line ending; a line written ends with the file's.
- Ops: `packs/plan/src/feature-ops.ts` (new). `addFeature { section, title, summary }` and
  `moveFeature { feature, section }` are `OpSpec`s in `FEATURE_OPS`, registered in `OPS` of `write.ts`. `check` holds
  the argument rules (a section is 1 to 40 characters, no `#`, no control character, and may be empty; a title is one
  line of 1 to 200 characters with no `**`; a summary is one line or several, with no blank line; `feature` is
  `F<n>`). `prepare` runs the edit once to refuse or to find nothing to do (a feature already in the section is `ok`
  with no files), and hands the write the same edit to make on the body with the file's line ending. A proposal only;
  any status; any actor. The number is one past the highest in the file.
- `renameSection { from, to, versions? }`: `packs/plan/src/rename-section.ts` (new), on the path of `create`: no `id`,
  no `version` (an `id` or a `version` is refused), `applyOp` locks every root that exists, as `reid` does, and
  `renameSectionLocked` does the work. It finds the proposals with a heading for `from`, reads each file again and
  checks its version (the one the scan saw, and the one in `versions` when sent: a stale one answers `conflict` with
  that file's current task), makes and checks the new text of every file, and only then replaces each by temp and
  rename, in scan order, with the `between-files` failpoint after each file but the last. It returns the first
  changed file as `task` and `version`, every changed file in `items[]`, and the shared ones in `files`.
- Merge rules in `renameHeadings`: every heading that matches `from` (trimmed, any case) is one section. When another
  heading already matches `to`, it keeps its spelling and takes the entries; the `from` headings go, with the blank line
  before each. Otherwise the first `from` heading is renamed and the others merge into it, so a case change alone works.
- Write path: `Prepared.body` (`op-util.ts`) may return `{ refuse }` as well as the new body, and `writeLocked`
  refuses with it. `forProposal` is exported from `person-ops.ts`. `failpoint` in `atomic.ts` is exported and takes
  `between-files`.
- Declarations (`provider.ts`): `addFeature` and `moveFeature` on the proposal kind, after `reject`; `renameSection`
  beside `reid` in the provider's own ops. Each carries a schema, and a `description` in the schema that says the number
  is one past the highest in the file, so one removed from the file can be given again. No wire type changed, so there
  was no `pnpm gen`.
- Tests: `packs/plan/test/feature-ops.test.ts` (new, 71 tests): `addFeature` (empty section and heading made; second
  one under it; into an existing section, any case and trimmed; gaps in numbers; `UNSORTED` in another case; entries
  with no heading; the empty part of a new proposal; a part at the end of a file with no final line end; CRLF; a summary
  of several lines; an agent; each refused argument; a fenced-block line; a part with problems, two of them with their
  lines; a stale version; a task; no part), `moveFeature` (to another section, with its continuation lines; to a missing
  one; to Unsorted; already there; CRLF; each refusal; problems; stale), `renameSection` (the item's `Pack` to `packs`
  over three and a fourth untouched, with a merge in two shapes; any status; a case change; an empty section and
  `Unsorted` as `to`; CRLF; stale `versions` with conflict and none written; current versions; a problem in a part
  that has the section, and one that does not; nothing to rename; run again; an id and a version refused; each refused
  argument; a writer killed between two files, with every file whole and a second run that finishes; the private root
  in two cases) and the declarations. `host/tests/task_cli.rs`
  `sections_json_after_a_rename_has_no_near_duplicate_group_for_the_renamed_name`: `collide sections --json` before
  (two near-duplicate groups) and after a `renameSection` run on the provider itself (one group, `Packs` at 5 features).
  The three declaration tests that list ops (`provider.test.ts`, `transitions.test.ts`, `write.test.ts`) gained the new
  names. `features.test.ts` and `feature-links.test.ts` are unedited.
- Red at runtime: with `addFeature`, `moveFeature` and `renameSection` taken out of `OPS`, `OP_NAMES` and the
  declarations (the code as it was), all 71 tests of `feature-ops.test.ts` failed, each on `unknown op` or on the
  missing declaration, and the Rust test failed at `assert_eq!(reply["outcome"], "ok")` with the provider's
  `unknown op "renameSection"`. Two bugs the tests found in the first draft: a part at the end of the body read an
  empty line past its last line (`lastContent`), and the rename left a blank line at the end of a part when its last
  heading was merged away (the blank line before a removed heading goes with it).

### Decisions for the owner

- **`renameSection` also edits the proposals in the private root** (`ts-0125-yr`) when it runs in the main checkout,
  holding both locks as `reid` does. A private item is in `items[]` and not in `files[]`. In another checkout it
  edits the shared root only, as every private read and write does. Alternative: shared only. Then a private proposal
  keeps the old name, and `collide sections` (which reads the private root) lists both names again. Recommend editing
  both: the item's own reason ("a rename never leaves a name half changed").
- **`renameSection` takes an optional `versions`** (an item id to the version the caller read). It has no `id` and so no
  `version`; the item says a stale version on any file is a `conflict` with that file's current task, and this is the
  argument that can say it. Without it the op still refuses a file that changed between its scan and its read. A
  version for a proposal the rename does not touch is ignored. Alternative: no argument, and a rename that cannot
  tell a stale read from a fresh one. Recommend the argument: it costs one optional key, and the app's merge button
  has the versions in hand.
- **A rename that changes nothing is `refused`, not `ok`.** `ok` carries a `task`, and there is none to name. Running
  it again after it finished says `no proposal has a section named "Pack"` and writes nothing, which is the "again
  finishes it" the item asks for. Alternative: `ok` with the first proposal that had the name. Recommend the refusal.
- **Matching.** `from` and `to` match headings as `core/src/section.rs` does (trimmed, any case). Every heading that
  matches `from` is one section: a file with `### Pack` and `### pack` ends with one. A heading that already matches
  `to` keeps its own spelling (`packs` stays `packs` when the rename says `Packs`). `from` of `Unsorted` (any case) is
  refused: Unsorted is a part with no name, and entries above any heading would stay behind. `to` of `Unsorted` is
  allowed, and merges into the `### Unsorted` heading or is renamed to it. Alternative: refuse both. Recommend this.
- **A body with no `## Features` part is refused** by `addFeature` and `moveFeature` (`add the line "## Features"
  first`). A new proposal has the part, so this meets only a hand-made file. Alternative: make the part at the end of
  the body. Recommend refusing: where it goes among the other parts is a person's call.
- **Entries above any heading, and no `### Unsorted`.** `addFeature` with an empty section makes `### Unsorted` at the
  end of the part, as the item says, even when entries stand above the first heading. Both are section "" to the parser
  and to `collide sections`. Alternative: add to the entries above the first heading. Recommend the literal rule.
- **Any status.** `addFeature`, `moveFeature` and `renameSection` write a proposal of any status, a rejected one too.
  Alternative: only `draft` and `proposed`. Recommend any: `renameSection` has to touch every status, and a person fixes
  a typo in an accepted proposal.
- **Limits.** A title is 1 to 200 characters (as an item's `TITLE_MAX`), one line, no `**`. A summary has no length cap
  of its own (the body cap holds), no control character but a tab, and no blank line (one paragraph, so the part's
  parser, which skips blank lines, reads back what was written). Each line is trimmed. A summary line that starts
  with three backticks is refused by the read-again, with the line and its text. Alternative: a summary cap.
- **An empty section is left where it is.** Moving the last entry out of a section leaves its heading. Alternative:
  remove it. Recommend leaving it: the heading may be meant for the next feature.
- **The kill test.** The mid-way failure is a real kill (`COLLIDE_FAILPOINT=between-files`, as the `before-rename` test
  does) in a provider process, not an error thrown in the same process. A killed process also shows the lock left
  behind and the temp-file cleanup. Alternative: an in-process hook. Recommend the kill.
- **Plan text to add** (0.10, "A proposal's features"; Stage 4c Feature ops): a feature's number is one past the
  highest in the file; `renameSection` takes an optional `versions`, edits the private root's proposals in the main
  checkout, and refuses when no proposal has the name; every feature op reads the part again and refuses a result with
  a problem.
- Owner decisions: none recorded yet. The recommendations stand until the owner answers.

- Owner decisions (2026-11-18, at the build): the item's three decisions as it recommends, on the owner's delegation,
  and the builder's as recommended above. The plan text was applied to 0.10 (the `F<n>` rule) and to Stage 4c's Feature
  ops. `renameSection` through the daemon is carried to `ts-0136-rr` Notes.

### Not done, by design

- The command words on the command line and in the app: `collide task <id> addFeature <section> <title> <summary>` and
  `moveFeature` are read from the declared schemas (the host's positional rule), but `renameSection` has no `id`, and
  `write_op` in `host/src/task_daemon.rs` sends an id and a version for any op but `create`, so the provider refuses it
  with "leave id out". A rename through the daemon (`task.apply` with `source`, which already carries an op with no
  id) and its CLI word belong to `ts-0136-rr`; carried to its Notes. The test here runs the provider itself.
- The Features view's Sort and merge buttons (Stage 9d) and the read-only Features view (Stage 6a), as the item says.
- A feature number that was removed before any file held it: not remembered (the item's decision). Reading every ref, as
  ids do, is not built.
- Hooks: no op has `movesTo`, so no `task.before_status` hook fires for a feature write. A later item may declare one.
