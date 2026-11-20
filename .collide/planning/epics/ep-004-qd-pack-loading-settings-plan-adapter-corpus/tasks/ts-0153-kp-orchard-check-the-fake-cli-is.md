---
id: ts-0153-kp
kind: task
status: done
title: "Orchard check: the fake CLI is held to the real one's envelopes"
owners: []
links:
  - { type: blocked-by, to: ts-0145-cq }
  - { type: blocked-by, to: ts-0148-cv }
  - { type: blocked-by, to: ts-0150-dj }
---

## Goal

The fake plan CLI (`fixtures/orchard-arch/plan`) drifted from the real one and every test still passed: the create
envelope's `data.id` was an object where the real CLI prints a string (`ts-0145-cq`), and `ts-0150-dj` lists more. Drift
is found only when an owner check runs the adapter on a real clone. Record the shape of the real CLI's envelopes once,
and test the fake against those shapes, so drift fails the gates instead.

## Success criteria

- A shape reader in the fixtures turns one run of the CLI into its shape: exit code, whether stdout and stderr are
  empty, stderr's line count, and the JSON envelope as its key tree with each leaf's type. A string leaf is named by the
  reference pack's id pattern it fits (`ids.ts`), or `string`. Values are kept only where they are the CLI's own words:
  `command`, `schema_version`, a status the pack declares.
- A recorder script runs, against a plan CLI and a root given as arguments, each command the adapter calls: `status`,
  `set` (a legal move and an illegal one), and `new task`. It writes one shape file per case under
  `fixtures/orchard-arch/contract/`. It names no path of its own.
- The shape files come from the real CLI, recorded once (Notes). They hold no title, id, slug or other text from the
  real repo, and pass the denylist.
- A test runs the fake through the same cases on the fixture tree and fails, naming the case and the path in the
  envelope, when its shape differs from the recorded one.
- Every difference the recording shows is fixed in the fake (and in the adapter if it read the wrong shape), or filed as
  an item, before this item is done.

## Tests

- Write first and see red at runtime: the shape test over a shape file edited by hand to differ from the fake (a leaf's
  type, a missing key), red; restored, green.
- The shape reader: unit tests for a leaf per type, an id-pattern string, a kept constant, an array of objects.

## Notes

- Owner 2026-11-19: file this item (yes), on the reviewer's offer to hold the fake to recorded real envelopes.
- Order of work. The builder writes the shape reader, the recorder and the test, and records the fake's own shapes as a
  first set. The builder does not run the real CLI. The reviewer then runs the recorder against the real CLI and puts
  its shape files in place; the diff against the fake's set is the drift. The builder is resumed to fix it.
- The recording run follows the rules of the agent run of `ts-0121-xp`: a temp clone with `--no-hardlinks` of the
  owner's copy, a temp `HOME`, the plan CLI's packages installed inside the temp clone, write commands only on a made-up
  task in the temp clone, the owner's copy never written, the temp clone removed by exact path after. The copy's path
  and branch names go in no tracked file, commit or item.
- Recording again is by hand when the real CLI changes; the shape files carry the date they were recorded.
- Owner decisions: none recorded yet. The recommendations stand until the owner answers.

### Built

The first step of "Order of work": the shape reader, the recorder, the shape test, and the fake's own shapes as the
first recorded set. The real CLI was not run and the owner's copy was not read.

- `fixtures/orchard-arch/contract/shape.ts`: the reader. `shapeOf`, `envelopeShape`, `mergeShapes`, `runShape` (the
  shape of one run), and `diffShapes` / `diffRuns` (a run against a recorded shape; each message names the path).
  How it reads, which is also its header comment:
  - **Leaves.** `null`, `boolean`, `number`. A string is `=<status>` when it is a status the pack declares
    (`STATUSES` of `declaration.ts`), else the name of the first id pattern of `ids.ts` it fits (`task-id`,
    `task-short-id`, `proposal-id`, `epic-id`, `id`, `short-id`), else `string`. At the top of the envelope only,
    `command` keeps its word (one lowercase word with hyphens, 24 characters at most) and `schema_version` keeps its
    number (`=1`). A path, a title, a slug, a branch, a date or a sha is `string`.
  - **Arrays.** Elements are merged into one shape (a list of zero or one). Leaves that meet are joined with `|`,
    sorted. Objects that meet keep every key; a key that some of them lack ends in `?`. A leaf that meets an object
    or a list is `mixed`. The reader does this for the fake and the real alike, as it is the one function.
  - **Data keys.** A key is a schema key when it is camelCase or snake_case words (`blockedTasks`, `schema_version`),
    40 characters at most. Anything else is data: an id, a slug with a hyphen, a date, a path. An object with any
    data key is a map. Every key of a map is named by what it is (`<task-id>` and the like, a declared status as
    itself, else `<string>`), and the shapes of its values are merged under that name. A map of one-word data
    keys (a first name, say) cannot be told from a schema key: it is kept. That is the known limit. The second
    layer is the reviewer reading the recorded files and the denylist grep before the commit.
  - **Standard streams.** Only the exit code, `stdout_empty`, `stdout_json`, `stderr_empty` and the line count of
    standard error (`stderr_lines`, a blank line counts, the last newline does not). No text of either stream.
  - **Comparing.** A leaf matches when each of its alternatives was recorded. An empty list or an empty object, on
    either side, matches any list or object: nothing was seen there. A recorded key without `?` must be in the run,
    in every element; a key in the run that is not recorded is a difference.
- `fixtures/orchard-arch/contract/cases.ts`: the cases, `argvOf` (the pack's own argument order: `--root <root>/arch
  --format json`, `--dry-run` last) and `runCase` (one child by `spawnSync`, a 120 s deadline, killed at it), and
  `taskStatus`.
- `fixtures/orchard-arch/contract/record.ts`: the recorder, run with node itself.
- `fixtures/orchard-arch/contract/fake-root.ts`: the root the fake is recorded on, made in a folder: the `basic` tree
  with the open questions of the `kinds` tree added, so that `status` has a queue.
- `fixtures/orchard-arch/contract/shapes/*.json`: the first set, recorded from the fake (`"source": "fake"`).
- `fixtures/arch-plan-tests/tsconfig.json`: the contract folder added to `include`.
- Tests, in `fixtures/arch-plan-tests/test/`: `shape.test.ts` (the reader: a leaf per type, an id pattern, a kept
  constant, an array of objects, a map, a diff per kind of difference, and the leak test: an envelope full of a path,
  a title, a slug, a branch, a date, a sha and an e-mail, as values and as map keys, whose shape contains none of
  them) and `contract.test.ts` (the shape files are one per case; the fake keeps the recorded shape of each case, in
  order, on a root of its own; a shape edited by hand is told apart; the recorder prints only the paths, writes only the
  case files under its folder, leaves no home, records the committed shapes over the fake, refuses a task that is not
  `ready` and a CLI that cannot be run, each in its own words).
- **Red at runtime** (each stubbed, run, restored; the last run green with 75 tests of the two files): `diffShapes`
  returning no difference (10 red, including the two hand-edited-shape tests); `leafOf` returning `string` for every
  text (21 red); `keyOf` keeping every key (4 red, the leak test among them); a shape file edited on disk (a leaf
  of `new-task` as `string`, and a key the fake lacks): `keeps the recorded shape of new-task` red, the case and the
  path named; the recorder without its `ready` check, without removing its home, and printing the exit code after a path
  (one red each).

#### The cases

The Orchard pack calls the plan CLI in two places: `write.ts` (`setStatus` runs `set <id> status=<status>`; `create`
runs `new task <proposal> <title> --epic <epic> [--depends-on <ids>]`; each with `--root <root>/arch --format json`,
and `--dry-run` for a preview) and `queue.ts` (`status --root <root>/arch --format json`, the human queue, which the
watcher runs too since `ts-0151-fk`). Nothing else: `renumber` is the pack's own business (it reads `ID-MAP.md`), not
a call. One case each, run in this order on one root (the writes change it):

| Case | Arguments (before `--root ... --format json`) |
|---|---|
| `status` | `status` |
| `set-dry-run` | `set <task> status=doing`, with `--dry-run` |
| `set-legal` | `set <task> status=doing` |
| `set-illegal` | `set <task> status=specified` (the task is `doing` now; no lifecycle goes back to `specified`) |
| `set-no-task` | `set T-99-999 status=doing` |
| `new-task-dry-run` | `new task <proposal> <title> --epic <epic>`, with `--dry-run` |
| `new-task` | `new task <proposal> <title> --epic <epic>` |
| `new-task-depends-on` | `new task <proposal> <title> --epic <epic> --depends-on <task>` |
| `new-task-no-proposal` | `new task P99 <title> --epic <epic>` |

`<title>` is made up (`Probe task`). Each shape file names its case and these arguments, its `source` (`fake` or
`real`) and the day it was recorded. The files are in `contract/shapes/`, one level under the folder the criterion
names, so that the folder keeps the code beside them.

#### Built, step 2: the drift the real set showed

The shape files in `contract/shapes/` are now the real CLI's (`"source": "real"`, recorded 2026-11-20), put in place
by the reviewer. `contract.test.ts` was 6 red, and showed three differences (the first two fixed, the third noted):

1. **`status`: `data.evidence`** is an object `{ inferred, measured, unmeasured }` of three numbers; the fake gave a
   list. Fixed in `fixtures/orchard-arch/plan` (`commandStatus`): three zeros. The pack does not read `evidence`
   (`queue.ts` reads `data.queue[].question` only; the word `evidence` in `kinds.ts` is an ADR's link type), so the
   pack read no wrong shape.
2. **`set` on a task that is not there, `new task` on a proposal that is not**: exit 2, empty standard output and two
   lines on standard error, a message that ends in the id, then one indented hint that names a `plan` command. The
   fake printed one line. Fixed: `UsageError` takes an optional hint, which `main` prints as `  <hint>` on a second
   line; the made-up words are `no such task: <id>` / `no such proposal: <id>` and `Check the ids with plan status.` /
   `Check the proposals with plan status.` (`renumber`'s own refusal for a missing task is unchanged: it is not a call
   the pack makes, and nothing recorded says how the real one words it). The pack read nothing wrong here: for exit 2
   `execute` in `write.ts` makes the whole of standard error, trimmed, the refusal's message, so it keeps both lines
   (it already did for an illegal move, which is two lines). I added a pack test for the new case (see below).
3. **Seen while recording, not fixed:** the real `new task` also takes `--blocked-by` (comma-separated ids). The fake
   already takes it (its header says so); the pack does not use it, so no case records it.

Tests, in `fixtures/arch-plan-tests/test/`:
- `fake-plan.test.ts`: `status` counts the evidence as an object of three numbers; `refuses a task that is not there
  with its id and a hint` and `... a proposal that is not there ...` (two lines, the first ending in the id, the second
  indented and naming `plan <command>`). All three were red at runtime against the old fake.
- `write.test.ts`: `a proposal the CLI does not know is a refusal with the CLI's message` now compares the reply to the
  CLI's own two-line standard error. Red against the old fake (one line); green now.
- `contract.test.ts`, three tests reworked so that they hold with either set committed: the two hand-edit tests build
  their edit from the committed shape (the first leaf of the `new-task` envelope set to a word no run gives; a key
  added to the `status` envelope; the committed exit code of `set-no-task` plus one), and `records shapes that hold
  against the committed ones, when it runs over the fake` checks the recorder's files against the committed set with
  `diffRuns` (no problems), the arguments, and `source` `fake`; it no longer compares bytes.
- Red at runtime, each mutation restored and the last run green (168 tests of `fake-plan`, `shape`, `contract`):
  `diffShapes` returning nothing (10 red, both leaf and key hand-edit tests among them); `diffRuns` not comparing the
  exit code (2 red, the exit-code hand-edit test among them); the fake's `evidence` a list again (4 red: the status
  case, the key hand-edit test, the recorder test and the evidence test); the fake without its hint line (6 red: both
  refusal cases, the exit hand-edit test, the recorder test and both new refusal tests); the recorder writing exit 0
  in every shape (1 red, the recorder test).

### Decisions for the owner

1. **Recording the real CLI (for the reviewer).** The exact command, with placeholders, from the repository root:

       node fixtures/orchard-arch/contract/record.ts --cli <CLI> --root <ROOT> --out <OUT> --source real \
         --task <TASK> --proposal <PROPOSAL> --epic <EPIC>

   - `<CLI>` is the plan CLI executable of the throw-away clone (`<ROOT>/plan`), `<ROOT>` the clone's checkout root
     (the folder that holds `arch/`), `<OUT>` a folder outside the clone and the repository.
   - `<TASK>` is the short id (`T-nn-nnn`) of a made-up task in the clone whose status is `ready`; the recorder reads
     the status line of its file and refuses to run if it is not `ready`. `<PROPOSAL>` is a proposal id the clone
     has (`Pnn`) and `<EPIC>` an epic id it has (`E-nn` or with its slug). The task is also the dependency of
     `new-task-depends-on`. The made-up task needs to be made by hand (a `ready` copy of a task file in the clone).
   - What the run needs: node 22.18 or later (it runs `record.ts` itself), and whatever the real CLI needs inside the
     clone: its packages installed there, as in the agent run of `ts-0121-xp`. The recorder gives the CLI a `HOME` and
     an empty git config inside `<OUT>` (`.home-*`, removed at the end) and the `PATH` it has. No network of its own.
   - What it does: runs the nine cases in order against `<ROOT>`, so `<ROOT>` is changed (a task moved to `doing`,
     three tasks made, the epic's table). It writes only `<OUT>/<case>.json` and prints one line per file, the path.
     It prints nothing of the CLI's output, and its errors name an argument, never a value from the run.
   - Swapping the real set in: `cp <OUT>/*.json fixtures/orchard-arch/contract/shapes/` (the names are the cases'; the
     `source` becomes `real`). `git diff` on that folder is the drift against the fake's set. Read the key names in
     the files, and run the denylist grep (`grep -i -E -f`) over the folder, before the commit. Then
     `pnpm --filter @collide/arch-plan-tests test`: `contract.test.ts` is red where the fake differs, with the case and
     the path. Fix the fake (and the adapter if it read the wrong shape), or file an item.
   - The fake's own set is made again the same way, on a root made by
     `node fixtures/orchard-arch/contract/fake-root.ts <new folder>`, with `--task T-03-003 --proposal P03 --epic E-07`.
2. **The shapes are in `contract/shapes/`, the code in `contract/`.** Alternative: the shape files straight in
   `contract/`, as the criterion words it. Recommendation: what I built; a folder swap is one `cp`.
3. **An empty list or object matches any.** Alternative: a recorded non-empty list fails against an empty one, so the
   fake must fill every list the real CLI fills. Recommendation: what I built. The fake's `status` fills only `queue`
   and the real one will fill more: with the strict rule the fake's root would have to grow a task in every state.
   The reviewer sees these in the shape diff and fills them in the fake where the adapter reads them. The cost is
   that an unfilled list in the fake hides drift in its elements until the root has one.
4. **A string is kept only as a status, an id pattern name or `string`.** Alternative: keep `corpus` (`arch`) and
   `kind` (`task`), which are the CLI's own words. Recommendation: what I built; both would need a rule for what a
   CLI's own word is, and the type is what the adapter reads.
5. **Cases beyond the item's list.** The item names `status`, `set` legal and illegal, and `new task`. I added the
   two dry runs (the pack's preview calls them), `new task` with `--depends-on`, a `set` on a task that is not
   there and a `new task` on a proposal that is not (the pack reads exit 2 as a refusal for both). Alternative: the
   named cases only. Recommendation: what I built; each is a call the pack makes or a refusal it parses.
6. **The legal and the illegal move.** `ready` to `doing` is the legal one, and `doing` to `specified` the illegal one.
   If the real lifecycle differs the real recording shows it (an illegal move that exits 0, say) and is a finding.
7. **The gate's refusal is not a case.** Exit 1 with an `errors` list in the envelope (the pack's `gateRefusal`)
   needs a task file the real CLI's validator rejects, and making one writes a bad file in the clone by hand.
   Alternative: a case that edits a task file of the clone to break a rule, then runs `set`. Recommendation: leave it
   out until the recording shows what the gate prints for another reason (`status: started` is exit 2 in the fake).
8. **The recording date is the shape file's only date.** It is the recorder's own clock, not the run's.

9. **The fake's refusal words, again.** Made up, as `ts-0150-dj` decided for the illegal move: `no such task: <id>` and
   `no such proposal: <id>`, each with one hint line naming `plan status`. Alternative: none that keeps the rule that no
   real word enters the repo. The shape test holds the form (two lines, the first ending in the id, the second
   indented and naming a command), not the words.
10. **Only what the pack reads is filled in the fake.** The real set has more keys that the fake fills as empty lists
    or objects, and the "empty matches any" rule lets them pass (see Not done). Alternative: fill them all, with a
    root that has proposals, epics, blocked tasks and a velocity. Recommendation: what I did; none of them is read.
    The cost is that a change in the pack that starts to read one of them is not caught until the fake fills it.

Proposed plan text, for Stage 4c, after the Orchard provider bullets: "The fake plan CLI of the Orchard fixtures is
held to the real CLI's envelopes: `fixtures/orchard-arch/contract` records the shape of each call the pack makes (the
exit code, which streams are empty, the line count of standard error, and the JSON envelope as its key tree with leaf
types, ids named by pattern, and no other text), and a test runs the fake through the same calls. Recording the real
CLI is by hand, in a throw-away clone, when it changes."

### Not done, by design

- Recording the real CLI: done by the reviewer (step 2 above). Recording again is by hand when the real CLI changes.
- Keys the real set has that the fake fills as an empty list or object, which "empty matches any" lets pass. None is
  read by the pack, so none is filled; the reviewer may judge otherwise:
  - `status`: `data.proposals[]` (`file`, `key`, `openObligations`, `status`, `totalObligations`), `data.epics[]`
    (`file`, `key`, `status`, `tasksByStatus`, `totalTasks`), `data.blockedTasks[]` (`blockedBy`, `file`, `task`),
    `data.velocity` (a map of `adrs` and `spikes` counts). The fake gives `[]` or `{}` for each. `queue.blocks` and
    `data.suppressed` are empty in the real set too, so nothing was seen there.
  - `new task` and `set`: `data.errors` is `[]` on exit 0 in every real shape, so the shape of an error in the
    envelope was not seen. The pack reads `data.errors[0].file` and `.message` for the gate's refusal (exit 1,
    `gateRefusal`), which is not a case (Decision 7): it is the one thing the pack reads that no recording backs.
  - `new task`: `frontmatter.depends_on` and `frontmatter.blocked_by` are `[]` in the real shapes of `new-task` and its
    dry run (the pack reads only `frontmatter.status`).
- The real `new task --blocked-by`: neither the pack nor a case uses it.
- The gate's refusal as a case (Decision 7).
- `ts-0155-wb`, `ts-0156-nt` and `ts-0158-ow`: not changed and not made trivial. `ts-0138-pn` stays blocked until the
  reviewer has landed the real set and this fix.

### At review

- The real set was recorded by the reviewer on 2026-11-20, under the agent-run rules of `ts-0121-xp`: a `--no-hardlinks`
  clone of the owner's copy in a temp folder, a temp `HOME`, packages installed inside the clone, writes only on a
  made-up task (made with the real `new task`, then set to `ready` with the real `set`). The clone was removed by exact
  path after. No denylist hit in the nine files; their keys are the CLI's schema words.
- Setup changed since `ts-0121-xp`: the real launcher now runs the CLI from source, so no build. In a fresh clone it
  needs `npm install` in the CLI's own package folder (the root `npm install` is not enough) and `tsx` added there with
  `npm install --no-save tsx`: its dev mode loads the sources through `tsx`, which its package does not list (the
  owner's own install supplies it from elsewhere). The `ts-0138-pn` checklist must say so.
- Drift found and fixed in step 2: `status`'s `data.evidence` is an object of three counts, not a list; a `set` on a
  missing task and a `new task` on a missing proposal print two stderr lines, not one. The pack read neither wrongly.
- Accepted gap (decision 7): the gate's exit-1 refusal, whose `errors[0].file` and `.message` the pack reads, is not
  recorded. No legal call provokes it: `doing` to `done` is a plain exit-2 refusal in the real CLI. Record it when a
  real run shows one.
- Decisions 1 to 10 taken as recommended (owner delegation). The proposed plan text applied, shortened, as a 4c bullet
  after the Orchard pack's.
