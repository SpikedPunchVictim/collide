---
id: ts-0117-gu
kind: task
status: ready
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
