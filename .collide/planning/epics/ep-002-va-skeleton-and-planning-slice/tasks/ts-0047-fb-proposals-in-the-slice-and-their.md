---
id: ts-0047-fb
kind: task
status: active
title: "Proposals in the slice and their Features part"
owners: []
links:
  - { type: blocked-by, to: ts-0030-qw }
  - { type: blocked-by, to: ts-0037-vg }
---

## Goal

The slice reads proposals and their feature sets (revision 14). The markdown provider reads the `.md` files directly in each epic's `proposals/` folder as kind `proposal`, with 0.10's six statuses and their categories: `draft` (active), `proposed` (review), `accepted` (done), `deferred` (backlog), `rejected` and `superseded` (dropped). `create` with `kind=proposal` writes a `draft` into the epic's `proposals/` with a `pr-` id (four digits), and `setStatus` moves among the six. `core` gains the `Feature` type (`id` as `F<n>`, `title`, `section`, empty when unsorted, and `summary`) and `FeatureProblem` (`line`, `reason`), and the task carries `features` and `featureProblems`. The provider parses each proposal's `## Features` part as 0.10 writes it. A problem in a `draft` is reported in `featureProblems`, never as a `task.invalid`. The move to `proposed` is refused while the part has a problem, with every problem and its line in one message. `collide task <proposal>` prints the features by section, and the problems.

## Success criteria

- A proposal file in `proposals/` lists as kind `proposal` with its status and category. A status outside the six is a `task.invalid`.
- `collide task create "<title>" kind=proposal epic=<epic>` writes `pr-NNNN-xx-<slug>.md` in that epic's `proposals/` as `draft`, numbered across refs as tasks are.
- Over 0.10's example, `collide task <id> --json` lists F1 and F2 in section `Packs`, and F3 with section `""`. An entry above the first heading also has `""`, and `### unsorted` counts as Unsorted.
- Each problem is reported with its line, all in one pass: an entry not in the form `- **F<n> <title>**: <summary>`, a repeated `F<n>`, `F0`, an empty section heading or one over 40 characters, a `####` heading, a line that is neither an entry nor a continuation, a continuation indented by one space, and a second `## Features` part.
- A `draft` with problems lists with its `featureProblems` and no `task.invalid`. `setStatus proposed` on it is refused, the message lists every problem with its line, and the file is unchanged. With the part fixed, the same call succeeds and changes only the `status` line (`git diff`).
- A proposal with no Features part has no features and no problems, and moves to `proposed`.
- `collide task <proposal>` prints the features grouped by section, sections in order of first appearance and Unsorted last, then the problems.
- `features` and `featureProblems` are in the generated `Task` schema, and a provider that reports neither validates unchanged.

## Tests

Write first: provider tests over proposal fixtures (0.10's example, each problem above, no Features part, `### unsorted`, an entry above the first heading); the refusal at `proposed` and the success once fixed; `create` of a proposal; a `core` serde and schema test for the new fields; a CLI test for the print.

## Notes

- IMPLEMENTATION_PLAN.md, 0.8 (Task (cache): `features`, `featureProblems`), 0.10 (Planning sources: kinds, the proposal's statuses, "A proposal's features"), Stage 2: Skeleton (Planning contracts, "From revision 14"; the slice's "features" bullet; the criteria's "features (revision 14)"; Tests, "Features").
- Area: `core` (types, schema, `planning_kinds` gains `proposal`), `packs/plan` (scan, item, create, write, and a new parser module), `host` (the CLI's print).
- The part runs from the line `## Features` to the next line that starts with `## `. A `## Features` inside a fenced code block elsewhere in the body is not the part. A fenced block inside the part is a problem.
- Not in this item: `implements`, the feature target and the derived state (`ts-0048-pg`); sections and near-duplicates (`ts-0049-kn`); the proposal's transitions, the person-only rule and `by:` (Stage 4c, which has the actor); the feature ops `addFeature`, `moveFeature` and `renameSection` (Stage 4c).
- Generated files change: run `pnpm gen` and commit its output, byte-stable on a second run.
- Done when the criteria hold.
- Built: `core` has `Feature` (`id`, `title`, `section`, `summary`) and `FeatureProblem` (`line`, `reason`) in `core/src/item.rs`, both registered as schema types, and `Task` carries `features` and `featureProblems` (omitted when empty, so a task that has neither serializes and validates as before). `planning_kinds()` gains `proposal` with its six statuses. In `packs/plan`: `src/features.ts` is the parser (`parseFeatures(body, firstLine)`); `scan.ts` also reads `proposals/`; `item.ts` holds the six statuses and `stateOf` (a status word with its category, the one `ItemState` model); `create.ts` and `ids.ts` take `kind=proposal` (`pr-`, four digits, counted from `proposals/` in refs and the working tree); `write.ts` refuses `setStatus proposed` while the part has a problem; `provider.ts` declares the kind. `host/src/task_view.rs` prints the features by section (first appearance, Unsorted last), then the problems; `task_cli.rs` has `kind=task|epic|proposal` in the usage and help. Tests: `packs/plan/test/features.test.ts` and `proposals.test.ts`, `core/tests` (kinds, serde, schema keys), `host/tests/task_cli.rs` (print, JSON, create, refusal and success).
- Decisions for the owner: (1) a problem's `line` is the line of the file, not of the body, so an agent can open the file at that line. (2) A proposal declares its own `setStatus` (`status` only, the six words), because a category does not name one status (`rejected` and `superseded` are both `dropped`); `setStatus` with `category` on a proposal is refused. The provider-level `setStatus` is unchanged. (3) `create` writes a proposal with an empty `## Features` part and a `## Decision` part, the two parts 0.10 names; the plan gives no template. (4) The basic provider fixture's `pr-0001-aa` was an unreadable stand-in that proved `proposals/` was never opened; it is now a valid proposal over 0.10's example, and the tests that counted six items count seven. (5) An item is read by the kind in its frontmatter, wherever its folder is; no check ties `proposals/` to `kind: proposal`. (6) Parser details the plan does not fix: an entry needs a non-empty title and summary; `F01` is not an entry (no leading zeros); a continuation joins the summary with one space; a tab is not an indent; `###foo` is not a heading; a second `## Features` part is one problem on its heading and its content is not read; a fenced block inside the part is one problem on its opening line and its lines are skipped; a repeated `F<n>` keeps the first and reports the second with the line of the first.
- Not done by design: `implements`, feature targets, the derived state, sections and near-duplicates, transitions, the person-only rule and `by:`, the feature ops.
