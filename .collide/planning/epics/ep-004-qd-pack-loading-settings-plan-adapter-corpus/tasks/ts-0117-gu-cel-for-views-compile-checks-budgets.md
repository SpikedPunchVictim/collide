---
id: ts-0117-gu
kind: task
status: done
title: "CEL for views: compile checks, budgets and caps"
owners: []
links:
  - { type: blocked-by, to: ts-0116-wh }
---

## Goal

A `core` module that compiles and runs one view expression the way 0.9 says CEL must run on untrusted task data: a
source cap, a compile that cannot take the process down, no corpus variable, a static cost estimate, caps on the data a
task brings, and a step budget for each task and for each pass.

What exists. `vendor/cel/` is `cel` 0.15.0 with S5's step counter (`cel::steps::set_limit`, `cel::steps::used`; thread
local; the budget error is a `FunctionError` for the function `cost`, "step limit exceeded"), and `core/Cargo.toml` uses
it with its default features. `core/tests/cel_step_budget.rs` proves the counter stops a runaway. `core/src/view.rs` has
`View` and `check_view`, which check only the length of `where:` against `MAX_CEL_SOURCE_BYTES` (1,024). Nothing in
`core` compiles or evaluates an expression, estimates a cost, caps a list, or binds a task.

What this adds, in `core/src/view_expr.rs` (or a module of that name):
- `compile(source)`: the 1 KB cap, a parse under `catch_unwind` so a panic is an error on that expression, and a walk of
  the syntax tree that refuses any name that is not a task variable or a macro's own variable (a corpus variable such as
  `tasks`).
- `estimate(&compiled)`: the static cost, S5's walk. Each macro multiplies its body by the size of the list it ranges
  over, with a stated bound for a list whose size it cannot know. It says when `matches` is used (it compiles its regex
  on every call).
- The task bindings: `key`, `source`, `title`, `status`, `category`, `kind`, `labels`, `collections`, `deps`, `author`
  and `fields`, from a listed task and its provenance.
- The caps on a task's data: 1,000 entries in `labels`, `collections` and `deps`, and about 4 KiB for each string. A
  task over a cap is not bound: the corpus lists it as an invalid item with the reason.
- `evaluate_pass`: runs an expression over many tasks with a budget for each task and one for the pass, and returns who
  matched, the errors counted by reason, the steps used, and whether the pass budget ran out.

## Success criteria

- A source of exactly 1,024 bytes compiles, 1,025 is refused, and the refusal gives the length and the cap. An empty
  source is refused.
- A compile that panics is an error on that expression. The next expression compiles. The panic is also caught on
  evaluation.
- A view that names a corpus variable (`tasks`, `corpus`, or any unknown name) is refused with the name and the list of
  names a view may use. A macro's own variable (`labels.exists(l, l == "x")`) is allowed.
- The estimate of an ordinary expression is small, never under the steps the counter measures for it, and a nested
  comprehension that cannot finish within the budget is refused with the estimate shown. `matches` adds a note.
- A hostile task, one with a list of 1,000 entries and `labels.all(a, labels.exists(b, a == b))`, stops at the per-task
  budget (10,000 steps) with a budget error for that task, and the pass goes on to the next task.
- A pass over many cheap tasks stops at the per-pass budget (2,000,000 steps), reports that it ran out, and counts the
  tasks it did not reach. It never runs on.
- An evaluation error (a type error, a missing `fields.x` read without `has`) excludes the task and is counted, with the
  reason, not hidden. `has(fields.x)` is false for a task without the field.
- A task with a list over 1,000 entries or a string over 4 KiB is an invalid item with the reason, listed with the
  source's other invalid items, and the corpus's ingest applies the check.
- The budget error is recognised by one function, and a test pins the message it matches, so a change to the vendored
  crate breaks a test and not a view.
- `core/tests/cel_step_budget.rs` still passes, and `core` gains no new dependency.

## Tests

Write these first (the stage's CEL view tests; the view-level ones are in `ts-0118-wp`).

- The source cap at 1,023, 1,024, 1,025 bytes, and empty.
- A compile that panics: through a compile function the test passes in, so a panic can be made on demand, and the
  expression after it still compiles. A list of 100,000 nested brackets is refused by the source cap before the parser
  sees it.
- A corpus variable refused; a macro variable allowed; `has(fields.x)` on a task with and without the field.
- The estimate against the counter's measured steps for the shapes S5 measured (E1 to E6 and the nested ones, at a small
  size), the estimate on a view that cannot finish within the budget, and the `matches` note.
- The per-task budget on the hostile task; the per-pass budget with a small budget and many cheap tasks; the counts.
- Evaluation errors counted by reason.
- Caps: a task with 1,001 labels and with a 4,097-byte title; the corpus's ingest lists each as invalid.
- The pinned budget message.
- The step counter is reset for each task, and a pass that follows a stopped one starts from zero.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 4b: Views ("CEL runs as 0.9 says: the vendored `cel` 0.15 with S5's step-counter patch, a per-task and a
    per-pass step budget, a 1 KB source cap, list caps at load, `catch_unwind` around compile, no corpus variable, and a
    static cost estimate when a view is saved and when it loads"); Tests: the CEL view tests (budgets, source over 1 KB,
    a list over its cap, a panic in compile, the static estimate, a corpus variable).
  - 0.9: Untrusted input (the CEL limits). 0.10: Views (fields, "bind per-task fields only", the static estimate).
  - S5 (`sp-0005-fv-schema-expressions`) Q4: the counter, the estimate, the suggested limits.
- Carried from `ts-0044-xq`: the budgets, the caps, `catch_unwind` and the estimate; the budget error is a plain
  `FunctionError` (match on its message or add a typed error); the default features of `cel` were kept until 4b says
  which functions views want; the counter is thread local; steps do not bound memory.
- **Owner's choice, with a recommendation: what the estimate refuses.** The plan says a view "whose estimate passes the
  per-pass budget is refused". An expression has no corpus to range over, so its cost is for one task. Recommend: refuse
  when the estimate for one task (lists at 64, S5's bound) times the board's current task count, at least 1,600, is over
  2,000,000, which is about 1,250 steps a task. The heaviest ordinary expression S5 measured is 16 steps, and every
  nested form is far over. Alternative: compare the estimate with the per-task budget of 10,000, which lets a nested
  form of two lists through to fail at run time on every task. The first says "refused" at save time, which is the point
  of the check. This decides which views a person can save.
- **Owner's choice, with a recommendation: the names a view can use.** 0.10 lists `key`, `source`, `title`, `status`,
  `category`, `labels`, `collections`, `deps` and `author`. S5's corpus also bound `id`, `kind` and `depends_on`, and
  the example view uses `closure.follow: depends_on`. Recommend the plan's nine, plus `kind` (a view on `kind in
  ['bug']` is natural, and the Field's column is the source's kind) and `fields`. `deps` only, no `depends_on`: one
  name. Alternative: bind exactly the nine. Without `kind` a view cannot tell a spike from a task. It sets what view
  authors can write.
- **Decision for the item, with a recommendation: `author`.** `Task` has no author. Bind it from the provenance the
  corpus holds (`CachedTask`), and an empty string when unknown. Stage 5's GitHub source supplies the real one.
- **Decision for the item, with a recommendation: the default features of `cel`.** Keep them (S5 measured with them;
  `matches` and timestamps stay), since taking a function away later would break saved views. Alternative:
  `default-features = false` with `regex`, which drops `chrono-tz` and `base64` from the closure and removes timestamps.
- **Decision for the item: where the caps apply.** In one function that the corpus's ingest and the binding both call,
  and `check_view` also caps `include:` and `exclude:` at 1,000 entries each. A pack's view is data from the repo, and
  untrusted too.
- **Decision for the item: threads.** The pass runs on one thread, because the counter is thread local. A multi-thread
  pass would need a shared budget; not here.
- Out of scope: the set algebra, closure and error counts of a view (`ts-0118-wp`); loading files and the daemon's
  methods (`ts-0119-tj`); the editor's warning text (Stage 6); a memory bound (steps do not bound memory).
- Area: `core/src/view_expr.rs`, `core/src/view.rs` (`check_view` caps), `core/src/corpus.rs` (ingest check),
  `core/src/lib.rs`, `core/tests/`. No `Cargo.toml` change expected; a change to one needs the owner's word.
- The test rule: no test touches the real home, real launchd or a real daemon. A test that runs the `collide` binary
  clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as
  `host/tests/common/mod.rs` `TempRoot` does. This item's tests are pure `core` tests and need neither. Settings files
  are fixtures, never the owner's own `settings.json`.
- Stage 4b item (revision 23 scope, split 2026-11-17). Items of Stage 4b run one at a time, in id order, because they
  share `core`, `host` and the generated files.
- Nothing here runs a real `claude`.
- Owner decisions (2026-11-17, at the split): the recommendations above, on delegation: the nine names plus `kind` and
  `fields`, with `deps` and no `depends_on`; the estimate rule as recommended.

### Built

A pure `core` module. No I/O, no daemon, no store, no new crate or package, no `Cargo.toml` change, no generated file.

- `core/src/view_expr.rs` (new, in `core/src/lib.rs`).
  - `compile(source)` and `compile_with(source, parse)` (the parse is given, so a test can make it panic). Refuses an
    empty source (`ExprError::Empty`), one over 1,024 bytes before the parser sees it (`TooLong { bytes }`, which names
    the length and the cap), a syntax error, a panic (`Panicked`), and a name that is no task variable, CEL type name
    or macro variable (`UnknownName { name }`, which lists `TASK_NAMES`). The walk is scope aware: a macro's variable
    is known inside its body and not in its range, and not after it.
  - `estimate(&compiled)` gives `Estimate { steps_per_task, uses_matches }`: S5's walk, saturating, a macro's body
    times the size of its range (a list in the source has its length, any other list is 64), a membership test adds
    the list. `check_cost(&compiled, board_tasks, &budgets)` refuses with `TooCostly { per_task, tasks, total, limit }`
    when `per_task` times `max(board_tasks, 1,600)` is over the pass budget. `MATCHES_NOTE` is the text for the note.
  - `check_task_caps(task, author)`: 1,000 entries in `labels`, `collections` and the dependencies, 4,096 bytes in
    each bound string, and the same two caps on every list, object and string inside `fields`. It is the one function
    the corpus's ingest and the binding call. `CapError { reason }` names the field.
  - `TaskVars::bind(task, kind, provenance)`: `key`, `source` (from the key), `title`, `status`, `category`, `kind`,
    `labels`, `collections`, `deps`, `author` (the provenance's, else empty) and `fields`. `TASK_NAMES` lists them.
    `deps` is the target of each `blocked-by` or `depends_on` link that is not marked `blocking: false`
    (`DEPENDENCY_LINKS`). A task over a cap is not bound.
  - `evaluate_pass(&compiled, &tasks, &budgets)` returns `PassOutcome { matched (positions), errors (by reason),
    over_task_budget, steps, pass_budget_exhausted, not_reached }`. Each task gets `min(per_task, what the pass has
    left)` steps, so a pass never runs on. A task that errors, panics, answers with a non-boolean or runs out of steps
    is excluded and counted. `Budgets::default()` is 10,000 and 2,000,000. `evaluate_pass_with` takes the run.
  - `is_budget_error(&ExecutionError)`: the one function that knows the crate's words (`cost`, `step limit exceeded`).
  - The parse and each pass run on a thread with a 64 MB stack (`STACK_BYTES`): see Decisions.
- `core/src/view.rs`: `MAX_LIST_ENTRIES` (1,000) and `MAX_STRING_BYTES` (4,096), and `check_view` refuses `include:` or
  `exclude:` over 1,000 entries (`ViewError::TooManyTaskKeys { key, entries }`).
- `core/src/corpus.rs`: `insert` calls `check_task_caps` first. An over-cap task is an invalid item with the reason, at
  its path and checkout, and any copy held for it is taken out; a later fixed task at that path clears the item. List,
  `task.changed` with a task, and the startup list all pass through it.
- Tests: `core/tests/view_expr.rs` (47), `core/tests/corpus.rs` (4 new), `core/tests/views_profiles_locks.rs` (2 new).
  `core/tests/cel_step_budget.rs` is unchanged and passes.
- **Seen red at runtime**, each against a stub of the old behavior, put back after:
  - caps check always `Ok`: 4 corpus tests (`a_task_with_1001_labels_is_an_invalid_item_...`,
    `a_task_with_a_4097_byte_title_...`, `an_over_cap_task_is_listed_with_the_sources_other_invalid_items`,
    `a_change_that_brings_an_over_cap_task_...`) and 6 of `view_expr.rs` (the list and string caps, `collections` and
    `deps`, bytes not characters, a string in a list, in `fields` or as the author, and not bound).
  - no source cap and no name check: 6 (`a_source_of_1025_bytes_...`, `a_hundred_thousand_nested_brackets_...`, the
    corpus variable, any unknown name, a macro's own variable, a name in a macro's range).
  - no step budget in a pass: 4 (the hostile task, a pass never runs on, the counter from zero, the caller's counter).
  - `is_budget_error` always false: `the_budget_error_is_recognised_by_one_function_...` (the pinned message).
  - parse and pass on the caller's thread: `a_deep_nesting_does_not_overflow_the_estimate` aborts the test binary with a
    stack overflow (SIGABRT), and `a_pass_runs_on_its_own_thread_...` fails.
  - estimate always 1: 5 (`the_estimate_is_never_under_...`, the refusal with the estimate shown, the floor, deep
    nesting, a list in the source). `matches` never noted, a list's length ignored, the floor removed: one test each.
  - `check_view` without the key cap: `include_and_exclude_over_the_cap_fail_each_with_its_count`.
  - a parse panic not turned into an error: `a_compile_that_panics_is_an_error_...`; an evaluation panic not caught for
    each task: `an_evaluation_that_panics_is_counted_...`.
  - the pass limit ignoring what the pass has left: 3; the not-reached count dropped when the budget is spent exactly
    between two tasks: 1 (`a_pass_whose_budget_is_spent_exactly_...`, added after a mutation survived); `blocking:
    false` ignored: 1; `author` not bound: 2.
- Not seen red: the 1,023 and 1,024 byte sources, the syntax error, `has(fields.x)`, evaluation errors by reason, the
  non-boolean answer, the cut reason, and the 500-term sum and 1,000-term `!` chain of the deep-source test.
- Checks: `cargo build --workspace`, `cargo test --workspace --no-fail-fast` (4,277 tests, none failed),
  `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`, `pnpm test`, `pnpm typecheck`,
  `pnpm --filter @collide/app build`, `pnpm build:packs` and `pnpm check` all pass.

### Decisions for the owner

- **The parse and each pass run on a thread with a 64 MB stack.** A finding: a 394-byte source of twenty macros inside
  one another overflowed the 2 MB stack of a test thread in the parser, in a debug build. That is a process abort that
  `catch_unwind` cannot catch, and under the 1 KB cap. S5 measured `+` chains, brackets and `!` chains in release builds
  on an 8 MB stack and did not try nested macros. Within 1 KB the parser's own recursion limit (96) allows about 45
  macros inside one another; 8 MB held those in a debug build. The thread costs address space, not memory, and about
  the time to start a thread. It also keeps a pass's step counter off the caller's thread. Alternative: run on the
  caller's thread and refuse a nesting over a small depth by counting brackets before the parse (about 12). That needs
  no thread, but it turns away views the parser accepts, and it covers one shape of deep source. Recommend as built.
  `evaluate_pass` and `compile` therefore return a `Result` for a thread that cannot start (`ExprError::Thread`).
  The step budget is for one pass on that one thread; "one thread per pass" is read as that.
- **`deps` is the target of each `blocked-by` or `depends_on` link that is not `blocking: false`.** The item names
  `deps` and `depends_on` for the closure and leaves the link types open. The plan provider declares `blocked-by` and
  Orchard's frontmatter says `depends_on`. Alternative: `blocked-by` only, which matches the ready rule and leaves
  `ts-0118-wp`'s closure with no `depends_on` link to follow. Recommend as built; `ts-0118-wp` should follow the same
  `DEPENDENCY_LINKS`.
- **The caps reach inside `fields`.** A string in `fields` is capped at 4,096 bytes and a list or an object in it at
  1,000 entries, at any depth, since a view reads `fields.x` and the plan says "the bound string fields". Alternative:
  cap the top level only; a 5,000-entry list in `fields` would then be the 4.2 s case of S5. Recommend as built.
- **`kind` is a parameter of `TaskVars::bind`, not read from the task.** A task may leave its kind to the source's only
  kind, which only the corpus knows (`Corpus::kind_of` is private). `ts-0118-wp` passes it, and either makes that
  method public or adds an accessor. Recommend.
- **The author is the provenance's, and the corpus holds none.** `bind` takes `Option<&Provenance>`; the corpus keeps
  tasks as their source listed them, so its ingest checks the caps with no author, and `bind` checks the author's
  length. Until Stage 5 `author` is empty for the plan source. As recommended in the item.
- **CEL's type names stay usable** (`bool`, `int`, `uint`, `double`, `string`, `bytes`, `list`, `map`, `null_type`,
  `type`), so `type(labels) == list` compiles. They name no data. Alternative: refuse them with the corpus names.
  Recommend as built.
- **An evaluation error's reason is cut at 160 bytes.** CEL errors can quote a task's own data (a comparison of two
  values prints both), and a reason is the key of a count. A long value then gives one reason, cut. Two tasks with
  different short values still give two reasons. Alternative: count by the error's kind only, which loses "No such key:
  estimate". Recommend as built.
- **`Category` has a name function in `board.rs` that is private, so `view_expr.rs` has its own.** Recommend a public
  `Category::name` in `item.rs` when one of them is next touched; it was left alone here to stay in the item's area.
- **`check_view` also caps `include:` and `exclude:` at 1,000 entries** (`ViewError::TooManyTaskKeys`), as the item
  says. It does not cap the length of each key.
- Plan text proposed for 0.9, after "Compile also runs under `catch_unwind`": "The parse and each pass run on a thread
  with its own large stack, since a source under 1 KB can nest deep enough to overflow a small one, and that abort
  cannot be caught."

- Owner decisions (2026-11-18, at the build): every recommendation above, on delegation. The 0.9 text is applied after
  "Compile also runs under `catch_unwind`", and 0.9's list caps now name `deps`, not `depends_on`. At review: a pass
  that gives each task its full budget whatever the pass has left failed three tests
  (`a_pass_never_runs_on_even_when_the_task_budget_is_larger_than_the_pass_budget` among them), and a macro variable
  left in scope after its macro failed `a_macros_own_variable_is_allowed_inside_it_and_refused_outside`. A probe of the
  deepest 1 KB sources (brackets, lists, maps, ternaries and calls refused by the parser's depth limit; 500-deep `+`,
  `&&`, select and index chains compiled) found the name check, the estimate and the drop, which run on the caller's
  thread, safe on a 2 MB stack.

### Not done, by design

- The set algebra, the closure, `compile_view` and `evaluate_view` (one result for one view, with the counts of
  errors, budget, unmatched keys and invalid items), and the view's use of `Corpus::kind_of`: `ts-0118-wp`.
- Reading view files, the daemon's `task.views` and `task.list` with a view, and the estimate run when a view loads
  and when it is saved: `ts-0119-tj`. `check_cost` takes the board's task count so that call can pass it.
- The editor's warning text for `matches` (`MATCHES_NOTE` is the text): Stage 6. A memory bound (steps do not bound
  memory): not here.
- A GitHub author for `author`: Stage 5.
