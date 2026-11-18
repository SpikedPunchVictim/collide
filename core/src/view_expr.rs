//! CEL for views (0.9, Untrusted input; 0.10, Views): compiling and running one `where:` expression over task data
//! that a stranger may have written. It is pure: no I/O, no process, no store.
//!
//! What stands between an expression and the process:
//!
//! - **A source cap.** [`compile`] refuses a source over [`MAX_CEL_SOURCE_BYTES`] before the parser sees it. A deep
//!   source overflows the stack, and a very deep one panics inside the parser's error formatter.
//! - **A panic is an error on that expression.** The parse runs on its own thread, so a panic in it is
//!   [`ExprError::Panicked`]; each evaluation runs under `catch_unwind`, in [`evaluate_pass`].
//! - **A stack of its own.** The parser and the evaluator recurse once for each level of nesting, and a source of
//!   400 bytes (twenty macros inside one another) overflows the 2 MB stack of a test thread in a debug build, which
//!   aborts the process and cannot be caught. The parse and each pass therefore run on a thread with [`STACK_BYTES`].
//! - **No corpus variable.** An expression sees one task. [`compile`] walks the syntax tree and refuses a name that is
//!   not one of [`TASK_NAMES`], a type name or a macro's own variable.
//! - **A static estimate.** [`estimate`] walks the tree, and multiplies each macro's body by the size of the list it
//!   ranges over. [`check_cost`] refuses an expression whose estimate, over the board's tasks, passes the pass budget.
//! - **Caps on task data.** [`check_task_caps`] limits the lists and strings a task brings. The corpus's ingest calls
//!   it, so an over-cap task is an invalid item, and [`TaskVars::bind`] calls it, so one is never bound.
//! - **Step budgets.** [`evaluate_pass`] sets the vendored `cel` step counter (`cel::steps`) for each task, and stops
//!   the pass when its own budget is spent. The counter is thread local: a pass runs on one thread, and nothing here
//!   shares a budget across threads. Steps bound time, not memory.

use crate::domain::Provenance;
use crate::item::Category;
use crate::provider::Task;
use crate::view::{MAX_CEL_SOURCE_BYTES, MAX_LIST_ENTRIES, MAX_STRING_BYTES};
use cel::common::ast::{EntryExpr, Expr, IdedExpr};
use cel::{Context, Env, ExecutionError, ParseErrors, Program, Value};
use serde_json::Value as Json;
use std::collections::BTreeMap;
use std::fmt;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

/// The steps one task may use (0.9, S5's suggested limit).
pub const MAX_STEPS_PER_TASK: u64 = 10_000;

/// The steps one pass over the board's tasks may use (0.9, S5's suggested limit).
pub const MAX_STEPS_PER_PASS: u64 = 2_000_000;

/// The size the estimate assumes for a list it cannot know (S5: "other lists at most 64").
pub const ESTIMATE_LIST_SIZE: u128 = 64;

/// The estimate counts at least this many tasks, so that a board that is small today does not save a view that is
/// refused on a board of the usual size (a board has about 1,600 tasks).
pub const MIN_ESTIMATE_TASKS: u128 = 1_600;

/// The names a view may use for a task (0.10, Fields), and `kind` and `fields`.
pub const TASK_NAMES: [&str; 11] = [
    "key",
    "source",
    "title",
    "status",
    "category",
    "kind",
    "labels",
    "collections",
    "deps",
    "author",
    "fields",
];

/// CEL's own type names (`type(labels) == list`). They name no data.
const TYPE_NAMES: [&str; 10] = [
    "bool",
    "int",
    "uint",
    "double",
    "string",
    "bytes",
    "list",
    "map",
    "null_type",
    "type",
];

/// The link types that make a task depend on another. `blocked-by` is the plan provider's; `depends_on` is the name
/// a view's `closure.follow` uses. A link whose `blocking` is false is not a dependency.
pub const DEPENDENCY_LINKS: [&str; 2] = ["blocked-by", "depends_on"];

/// What the vendored crate's budget error says. The one function that recognises it is [`is_budget_error`], and a
/// test pins both, so a change to the crate fails a test and not a view.
const BUDGET_FUNCTION: &str = "cost";
const BUDGET_MESSAGE: &str = "step limit exceeded";

/// The stack of the thread a parse or a pass runs on. It is address space, not memory: only what the recursion
/// touches is used.
pub const STACK_BYTES: usize = 64 << 20;

/// The longest reason kept for an evaluation error. An error can quote a task's own data.
const MAX_REASON_BYTES: usize = 160;

/// The note a view editor shows when an expression uses `matches` (0.10: it recompiles its regex on every call).
pub const MATCHES_NOTE: &str =
    "`matches` compiles its regex on every call, so it costs about 20 times a plain test";

// --- compile ---

/// Why an expression was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExprError {
    /// The source is empty or only white space.
    Empty,
    /// The source is longer than [`MAX_CEL_SOURCE_BYTES`].
    TooLong { bytes: usize },
    /// The parser's own message.
    Syntax { message: String },
    /// The parser panicked on this source.
    Panicked { message: String },
    /// The thread for the parse or the pass could not be started.
    Thread { message: String },
    /// A name that is no task variable, type name or macro variable, such as the corpus variable `tasks`.
    UnknownName { name: String },
    /// The estimate, over `tasks` tasks, passes `limit` steps.
    TooCostly {
        per_task: u128,
        tasks: u128,
        total: u128,
        limit: u64,
    },
}

impl fmt::Display for ExprError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExprError::Empty => write!(f, "the expression is empty"),
            ExprError::TooLong { bytes } => write!(
                f,
                "the expression is {bytes} bytes; a CEL source is capped at {MAX_CEL_SOURCE_BYTES}"
            ),
            ExprError::Syntax { message } => write!(f, "the expression does not parse: {message}"),
            ExprError::Panicked { message } => {
                write!(f, "the expression made the parser panic: {message}")
            }
            ExprError::Thread { message } => {
                write!(
                    f,
                    "no thread could be started for the expression: {message}"
                )
            }
            ExprError::UnknownName { name } => write!(
                f,
                "`{name}` is not a name a view can use; a view sees one task, with: {}",
                TASK_NAMES.join(", ")
            ),
            ExprError::TooCostly {
                per_task,
                tasks,
                total,
                limit,
            } => write!(
                f,
                "the expression is estimated at {per_task} steps for a task, {total} for {tasks} tasks; \
                 a pass is limited to {limit}"
            ),
        }
    }
}

impl std::error::Error for ExprError {}

/// An expression that passed [`compile`]: it parses, and names only what a view may.
#[derive(Debug)]
pub struct CompiledExpr {
    program: Program,
}

impl CompiledExpr {
    pub fn program(&self) -> &Program {
        &self.program
    }
}

/// Compiles one view expression with the vendored `cel`'s standard environment.
pub fn compile(source: &str) -> Result<CompiledExpr, ExprError> {
    compile_with(source, &|text| Env::stdlib().compile(text))
}

/// [`compile`] with the parse given, so that a test can make it panic.
pub fn compile_with(
    source: &str,
    parse: &(dyn Fn(&str) -> Result<Program, ParseErrors> + Sync),
) -> Result<CompiledExpr, ExprError> {
    if source.trim().is_empty() {
        return Err(ExprError::Empty);
    }
    if source.len() > MAX_CEL_SOURCE_BYTES {
        return Err(ExprError::TooLong {
            bytes: source.len(),
        });
    }
    let parsed =
        on_big_stack("view-expr-parse", || parse(source))?.map_err(|errors| ExprError::Syntax {
            message: errors.to_string(),
        })?;
    if let Some(name) = first_unknown_name(parsed.expression(), &mut Vec::new()) {
        return Err(ExprError::UnknownName { name });
    }
    Ok(CompiledExpr { program: parsed })
}

/// Runs `work` on a new thread with [`STACK_BYTES`] of stack, and waits for it. A panic in it is
/// [`ExprError::Panicked`].
fn on_big_stack<T: Send>(name: &str, work: impl FnOnce() -> T + Send) -> Result<T, ExprError> {
    std::thread::scope(|scope| {
        let handle = std::thread::Builder::new()
            .name(name.to_string())
            .stack_size(STACK_BYTES)
            .spawn_scoped(scope, work)
            .map_err(|error| ExprError::Thread {
                message: error.to_string(),
            })?;
        handle.join().map_err(|panic| ExprError::Panicked {
            message: panic_message(panic.as_ref()),
        })
    })
}

fn panic_message(panic: &(dyn std::any::Any + Send)) -> String {
    let text = panic
        .downcast_ref::<&str>()
        .map(|text| (*text).to_string())
        .or_else(|| panic.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "no message".to_string());
    shorten(&text)
}

/// The first name in `expr` that a view may not use. `scope` holds the variables of the macros around it.
fn first_unknown_name<'a>(expr: &'a IdedExpr, scope: &mut Vec<&'a str>) -> Option<String> {
    match &expr.expr {
        Expr::Unspecified | Expr::Literal(_) => None,
        Expr::Ident(name) => {
            let known = scope.contains(&name.as_str())
                || TASK_NAMES.contains(&name.as_str())
                || TYPE_NAMES.contains(&name.as_str());
            (!known).then(|| name.clone())
        }
        Expr::Select(select) => first_unknown_name(&select.operand, scope),
        Expr::Call(call) => call
            .target
            .iter()
            .map(|target| target.as_ref())
            .chain(call.args.iter())
            .find_map(|part| first_unknown_name(part, scope)),
        Expr::List(list) => list
            .elements
            .iter()
            .find_map(|part| first_unknown_name(part, scope)),
        Expr::Map(map) => first_unknown_entry_name(&map.entries, scope),
        Expr::Struct(strukt) => first_unknown_entry_name(&strukt.entries, scope),
        Expr::Comprehension(comprehension) => {
            // The range and the start value are outside the macro's variables; the rest is inside.
            let outer = [&comprehension.iter_range, &comprehension.accu_init]
                .into_iter()
                .find_map(|part| first_unknown_name(part, scope));
            if outer.is_some() {
                return outer;
            }
            let depth = scope.len();
            scope.push(&comprehension.accu_var);
            scope.push(&comprehension.iter_var);
            if let Some(second) = &comprehension.iter_var2 {
                scope.push(second);
            }
            let inner = [
                &comprehension.loop_cond,
                &comprehension.loop_step,
                &comprehension.result,
            ]
            .into_iter()
            .find_map(|part| first_unknown_name(part, scope));
            scope.truncate(depth);
            inner
        }
    }
}

fn first_unknown_entry_name<'a>(
    entries: &'a [cel::common::ast::IdedEntryExpr],
    scope: &mut Vec<&'a str>,
) -> Option<String> {
    entries.iter().find_map(|entry| match &entry.expr {
        EntryExpr::MapEntry(pair) => [&pair.key, &pair.value]
            .into_iter()
            .find_map(|part| first_unknown_name(part, scope)),
        EntryExpr::StructField(field) => first_unknown_name(&field.value, scope),
    })
}

// --- estimate ---

/// The static cost of an expression for one task, in the unit the step counter counts (S5: an upper bound on node
/// evaluations; it overestimated by 1.1 to 11 times).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Estimate {
    pub steps_per_task: u128,
    /// The expression calls `matches`: show [`MATCHES_NOTE`].
    pub uses_matches: bool,
}

/// The estimate of `compiled` for one task. A macro multiplies its body by the size of the list it ranges over: a
/// list written in the source has its length, and any other list is taken as [`ESTIMATE_LIST_SIZE`], since the
/// caps and the data are not known here.
pub fn estimate(compiled: &CompiledExpr) -> Estimate {
    let mut uses_matches = false;
    let steps_per_task = cost(compiled.program.expression(), &mut uses_matches);
    Estimate {
        steps_per_task,
        uses_matches,
    }
}

/// The estimate refuses an expression when its cost for one task, times the board's task count (at least
/// [`MIN_ESTIMATE_TASKS`]), is over the pass budget. A one-task estimate stands in for the whole pass because an
/// expression has no corpus to range over.
pub fn check_cost(
    compiled: &CompiledExpr,
    board_tasks: usize,
    budgets: &Budgets,
) -> Result<Estimate, ExprError> {
    let found = estimate(compiled);
    let tasks = (board_tasks as u128).max(MIN_ESTIMATE_TASKS);
    let total = found.steps_per_task.saturating_mul(tasks);
    if total > u128::from(budgets.per_pass) {
        return Err(ExprError::TooCostly {
            per_task: found.steps_per_task,
            tasks,
            total,
            limit: budgets.per_pass,
        });
    }
    Ok(found)
}

/// The size of the list `expr` produces, as far as the source says.
fn list_size(expr: &IdedExpr) -> u128 {
    match &expr.expr {
        Expr::List(list) => list.elements.len() as u128,
        Expr::Comprehension(comprehension) => list_size(&comprehension.iter_range),
        Expr::Call(call) if call.func_name == "_+_" && call.args.len() == 2 => {
            list_size(&call.args[0]).saturating_add(list_size(&call.args[1]))
        }
        _ => ESTIMATE_LIST_SIZE,
    }
}

fn sum(costs: impl Iterator<Item = u128>) -> u128 {
    costs.fold(0, u128::saturating_add)
}

fn entries_cost(entries: &[cel::common::ast::IdedEntryExpr], uses_matches: &mut bool) -> u128 {
    entries.iter().fold(0, |total, entry| {
        let one = match &entry.expr {
            EntryExpr::MapEntry(pair) => {
                cost(&pair.key, uses_matches).saturating_add(cost(&pair.value, uses_matches))
            }
            EntryExpr::StructField(field) => cost(&field.value, uses_matches),
        };
        total.saturating_add(one)
    })
}

/// An upper bound on the node evaluations of `expr`. Saturating.
fn cost(expr: &IdedExpr, uses_matches: &mut bool) -> u128 {
    match &expr.expr {
        Expr::Unspecified | Expr::Literal(_) | Expr::Ident(_) => 1,
        Expr::Select(select) => cost(&select.operand, uses_matches).saturating_add(1),
        Expr::Call(call) => {
            if call.func_name == "matches" {
                *uses_matches = true;
            }
            let target = call
                .target
                .as_ref()
                .map_or(0, |target| cost(target, uses_matches));
            let mut total = sum(call
                .args
                .iter()
                .map(|argument| cost(argument, uses_matches)))
            .saturating_add(target)
            .saturating_add(1);
            if call.func_name == "@in" && call.args.len() == 2 {
                // A membership test scans the list.
                total = total.saturating_add(list_size(&call.args[1]));
            }
            total
        }
        Expr::List(list) => {
            sum(list.elements.iter().map(|part| cost(part, uses_matches))).saturating_add(1)
        }
        Expr::Map(map) => entries_cost(&map.entries, uses_matches).saturating_add(1),
        Expr::Struct(strukt) => entries_cost(&strukt.entries, uses_matches).saturating_add(1),
        Expr::Comprehension(comprehension) => {
            let per_round = cost(&comprehension.loop_cond, uses_matches)
                .saturating_add(cost(&comprehension.loop_step, uses_matches));
            1u128
                .saturating_add(cost(&comprehension.iter_range, uses_matches))
                .saturating_add(cost(&comprehension.accu_init, uses_matches))
                .saturating_add(list_size(&comprehension.iter_range).saturating_mul(per_round))
                .saturating_add(cost(&comprehension.result, uses_matches))
        }
    }
}

// --- the task a view sees ---

/// Why a task is not bound: a list or a string over its cap (0.9).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapError {
    pub reason: String,
}

impl fmt::Display for CapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.reason)
    }
}

impl std::error::Error for CapError {}

fn too_long(name: &str, bytes: usize) -> CapError {
    CapError {
        reason: format!(
            "`{name}` is {bytes} bytes; a string a view reads is capped at {MAX_STRING_BYTES}"
        ),
    }
}

fn too_many(name: &str, entries: usize) -> CapError {
    CapError {
        reason: format!(
            "`{name}` has {entries} entries; a list a view reads is capped at {MAX_LIST_ENTRIES}"
        ),
    }
}

fn check_string(name: &str, text: &str) -> Result<(), CapError> {
    if text.len() > MAX_STRING_BYTES {
        return Err(too_long(name, text.len()));
    }
    Ok(())
}

fn check_strings(name: &str, items: &[String]) -> Result<(), CapError> {
    if items.len() > MAX_LIST_ENTRIES {
        return Err(too_many(name, items.len()));
    }
    for (index, item) in items.iter().enumerate() {
        if item.len() > MAX_STRING_BYTES {
            return Err(too_long(&format!("{name}[{index}]"), item.len()));
        }
    }
    Ok(())
}

fn check_json(name: &str, value: &Json) -> Result<(), CapError> {
    match value {
        Json::String(text) => check_string(name, text),
        Json::Array(items) => {
            if items.len() > MAX_LIST_ENTRIES {
                return Err(too_many(name, items.len()));
            }
            for (index, item) in items.iter().enumerate() {
                check_json(&format!("{name}[{index}]"), item)?;
            }
            Ok(())
        }
        Json::Object(map) => {
            if map.len() > MAX_LIST_ENTRIES {
                return Err(too_many(name, map.len()));
            }
            for (key, item) in map {
                check_string(&format!("{name} (a key)"), key)?;
                check_json(&format!("{name}.{key}"), item)?;
            }
            Ok(())
        }
        Json::Null | Json::Bool(_) | Json::Number(_) => Ok(()),
    }
}

/// The dependencies of a task: the targets of its [`DEPENDENCY_LINKS`] links, in order.
fn dependencies(task: &Task) -> Vec<String> {
    task.links
        .iter()
        .filter(|link| {
            DEPENDENCY_LINKS.contains(&link.link_type.as_str()) && link.blocking != Some(false)
        })
        .map(|link| link.to.clone())
        .collect()
}

/// The caps of 0.9 on what a view reads of a task: [`MAX_LIST_ENTRIES`] in `labels`, `collections` and the
/// dependencies, and [`MAX_STRING_BYTES`] in each string, `fields` included (a list or an object inside `fields` has
/// the list cap). The body is not bound, so it is not checked. `author` is the provenance's author, when there is
/// one. The corpus's ingest and [`TaskVars::bind`] both call this, so the two cannot disagree.
pub fn check_task_caps(task: &Task, author: Option<&str>) -> Result<(), CapError> {
    check_string("key", &task.key)?;
    check_string("title", &task.title)?;
    check_string("status", &task.state.status)?;
    if let Some(kind) = &task.kind {
        check_string("kind", kind)?;
    }
    if let Some(author) = author {
        check_string("author", author)?;
    }
    check_strings("labels", &task.labels)?;
    check_strings("collections", &task.collections)?;
    check_strings("deps", &dependencies(task))?;
    if task.fields.len() > MAX_LIST_ENTRIES {
        return Err(too_many("fields", task.fields.len()));
    }
    for (name, value) in &task.fields {
        check_string("fields (a key)", name)?;
        check_json(&format!("fields.{name}"), value)?;
    }
    Ok(())
}

fn category_name(category: Category) -> &'static str {
    match category {
        Category::Backlog => "backlog",
        Category::Ready => "ready",
        Category::Active => "active",
        Category::Review => "review",
        Category::Done => "done",
        Category::Dropped => "dropped",
    }
}

/// One task as a view sees it (0.10, Fields): the normalized fields, and the source's own under `fields`.
#[derive(Debug, Clone, PartialEq)]
pub struct TaskVars {
    pub key: String,
    pub source: String,
    pub title: String,
    pub status: String,
    pub category: String,
    pub kind: String,
    pub labels: Vec<String>,
    pub collections: Vec<String>,
    pub deps: Vec<String>,
    pub author: String,
    pub fields: BTreeMap<String, Json>,
}

impl TaskVars {
    /// Binds a listed task. `kind` is the task's kind as the corpus knows it (a task may leave it to its source's
    /// only kind), empty when there is none. `author` is the provenance's, and empty when the source gave none: the
    /// plan provider has no author, and Stage 5's GitHub source supplies one. A task over a cap is not bound.
    pub fn bind(
        task: &Task,
        kind: &str,
        provenance: Option<&Provenance>,
    ) -> Result<TaskVars, CapError> {
        let author = provenance.and_then(|provenance| provenance.author.as_deref());
        check_task_caps(task, author)?;
        Ok(TaskVars {
            key: task.key.clone(),
            source: task
                .key
                .split_once(':')
                .map_or_else(String::new, |(source, _)| source.to_string()),
            title: task.title.clone(),
            status: task.state.status.clone(),
            category: category_name(task.state.category).to_string(),
            kind: kind.to_string(),
            labels: task.labels.clone(),
            collections: task.collections.clone(),
            deps: dependencies(task),
            author: author.unwrap_or_default().to_string(),
            fields: task.fields.clone(),
        })
    }

    fn context(&self, env: &Arc<Env>) -> Context<'static, 'static> {
        let mut context = Context::with_env(Arc::clone(env));
        let mut bind = |name: &str, value: Result<Value, cel::SerializationError>| {
            context
                .add_variable_from_value(name, value.expect("a string, a list or JSON converts"));
        };
        bind("key", cel::to_value(&self.key));
        bind("source", cel::to_value(&self.source));
        bind("title", cel::to_value(&self.title));
        bind("status", cel::to_value(&self.status));
        bind("category", cel::to_value(&self.category));
        bind("kind", cel::to_value(&self.kind));
        bind("labels", cel::to_value(&self.labels));
        bind("collections", cel::to_value(&self.collections));
        bind("deps", cel::to_value(&self.deps));
        bind("author", cel::to_value(&self.author));
        bind("fields", cel::to_value(&self.fields));
        context
    }
}

// --- evaluate ---

/// The step budgets of a pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Budgets {
    pub per_task: u64,
    pub per_pass: u64,
}

impl Default for Budgets {
    fn default() -> Budgets {
        Budgets {
            per_task: MAX_STEPS_PER_TASK,
            per_pass: MAX_STEPS_PER_PASS,
        }
    }
}

/// Whether `error` is the vendored crate's step-budget error. This is the one place that knows its words.
pub fn is_budget_error(error: &ExecutionError) -> bool {
    matches!(
        error,
        ExecutionError::FunctionError { function, message }
            if function == BUDGET_FUNCTION && message == BUDGET_MESSAGE
    )
}

/// What a pass found.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PassOutcome {
    /// The positions, in the order the tasks were given, of the tasks the expression matched.
    pub matched: Vec<usize>,
    /// The tasks that raised an evaluation error (or a panic), counted by reason. Each is excluded.
    pub errors: BTreeMap<String, u64>,
    /// The tasks that ran out of their own step budget. Each is excluded.
    pub over_task_budget: u64,
    /// The steps the pass used. It can pass the pass budget by the few nodes the stop unwinds through.
    pub steps: u64,
    /// The pass budget ran out: the tasks after the last one tested were not reached.
    pub pass_budget_exhausted: bool,
    /// The tasks the pass did not reach, the one it stopped in included.
    pub not_reached: usize,
}

impl PassOutcome {
    /// The tasks that raised an error, over every reason.
    pub fn error_count(&self) -> u64 {
        self.errors.values().sum()
    }
}

fn shorten(text: &str) -> String {
    if text.len() <= MAX_REASON_BYTES {
        return text.to_string();
    }
    let mut end = MAX_REASON_BYTES;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...", &text[..end])
}

/// Tests `compiled` against each task, in order, on one thread with a stack of its own. Each task gets
/// `budgets.per_task` steps, and the pass gets `budgets.per_pass`: a task is given no more than what the pass has left,
/// so the pass never runs on. A task that errors, panics or runs out of steps is excluded and counted, and the pass
/// goes on to the next.
pub fn evaluate_pass(
    compiled: &CompiledExpr,
    tasks: &[TaskVars],
    budgets: &Budgets,
) -> Result<PassOutcome, ExprError> {
    evaluate_pass_with(compiled, tasks, budgets, &|program, context| {
        program.execute(context)
    })
}

/// [`evaluate_pass`] with the run given, so that a test can make an evaluation panic.
pub fn evaluate_pass_with(
    compiled: &CompiledExpr,
    tasks: &[TaskVars],
    budgets: &Budgets,
    run: &(dyn Fn(&Program, &Context) -> Result<Value, ExecutionError> + Sync),
) -> Result<PassOutcome, ExprError> {
    on_big_stack("view-expr-pass", || pass(compiled, tasks, budgets, run))
}

fn pass(
    compiled: &CompiledExpr,
    tasks: &[TaskVars],
    budgets: &Budgets,
    run: &dyn Fn(&Program, &Context) -> Result<Value, ExecutionError>,
) -> PassOutcome {
    let env = Arc::new(Env::stdlib());
    let mut outcome = PassOutcome::default();
    for (index, task) in tasks.iter().enumerate() {
        let remaining = budgets.per_pass.saturating_sub(outcome.steps);
        if remaining == 0 {
            outcome.pass_budget_exhausted = true;
            outcome.not_reached = tasks.len() - index;
            break;
        }
        let limit = budgets.per_task.min(remaining);
        // Resets the used count: each task starts from zero, and so does a pass that follows a stopped one.
        cel::steps::set_limit(limit);
        let result = catch_unwind(AssertUnwindSafe(|| {
            run(compiled.program(), &task.context(&env))
        }));
        let used = cel::steps::used();
        outcome.steps = outcome.steps.saturating_add(used);
        let out_of_steps = used > limit;
        match result {
            Ok(Ok(Value::Bool(true))) if !out_of_steps => outcome.matched.push(index),
            Ok(Ok(Value::Bool(false))) if !out_of_steps => {}
            Ok(Ok(other)) if !out_of_steps => {
                let reason = format!(
                    "the expression gave a {}, not true or false",
                    other.type_of()
                );
                *outcome.errors.entry(reason).or_default() += 1;
            }
            Ok(Err(error)) if !out_of_steps && !is_budget_error(&error) => {
                let reason = shorten(&error.to_string());
                *outcome.errors.entry(reason).or_default() += 1;
            }
            Err(panic) => {
                let reason = format!("the evaluation panicked: {}", panic_message(panic.as_ref()));
                *outcome.errors.entry(reason).or_default() += 1;
            }
            // The budget error, or an answer given after the budget ran out.
            Ok(_) => {
                if limit < budgets.per_task {
                    outcome.pass_budget_exhausted = true;
                    outcome.not_reached = tasks.len() - index;
                    break;
                }
                outcome.over_task_budget += 1;
            }
        }
    }
    outcome
}
