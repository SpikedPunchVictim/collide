//! Evaluating a view over the corpus (0.10, Views): its members, the steps of its closure, and the counts of what
//! went wrong on it. Pure: no I/O, no daemon, no store. The daemon (`ts-0119-tj`) loads view files and calls this.
//!
//! - **Membership** is `(where ∪ include ∪ closure) − exclude`, as task keys, in the corpus's order (key order), each
//!   key once. A view with none of the three parts has no members.
//! - **`where`** is tested on each row of the corpus. A key at two paths has a row for each, and is a member when
//!   either row matches.
//! - **`include`, `exclude` and the closure goal** are task keys, as the corpus holds them (a key that was renamed
//!   still matches). A bare id is not a key: it is unmatched and counted, never guessed.
//! - **The closure** walks from the goal along the dependencies of [`crate::view_expr::dependencies`], the ones a
//!   view's `deps` holds, so the two cannot disagree. It walks open tasks only (a category other than `done` or
//!   `dropped`). The goal is always in, and is walked from even when it is done. The walk goes through a task that
//!   `exclude` names: exclude removes members, it does not cut the walk.
//! - **The steps** are the closure's layers. The first holds the tasks with no dependency in the set, and each later
//!   one the tasks whose dependencies are all in earlier steps, in key order. When the layering stalls, a cycle is
//!   below it: the last step holds every task it could not place, and [`ViewResult::cycle`] names the ones on a cycle.
//!   Tasks `exclude` names are left out of the steps.
//! - **Counts**: [`ViewCounts`]. A failure of one view is that view's. [`compile_view`] gives a [`ViewFailure`] for a
//!   view that is refused, and nothing else is affected.

use crate::corpus::{Corpus, Row};
use crate::item::Category;
use crate::provider::Task;
use crate::view::{Closure, Follow, View, ViewError, check_view};
use crate::view_expr::{
    Budgets, CompiledExpr, Estimate, ExprError, TaskVars, check_cost, compile, dependencies,
    evaluate_pass,
};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt;

/// Why a view is refused. Either way it has no members.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViewFailure {
    /// [`check_view`] refused it, with every reason.
    Check(Vec<ViewError>),
    /// Its `where` did not compile, names something a view may not, is too costly, or could not run.
    Expr(ExprError),
}

impl fmt::Display for ViewFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ViewFailure::Check(errors) => {
                let reasons: Vec<String> = errors.iter().map(ToString::to_string).collect();
                write!(f, "{}", reasons.join("; "))
            }
            ViewFailure::Expr(error) => write!(f, "`where:` is refused: {error}"),
        }
    }
}

impl std::error::Error for ViewFailure {}

impl From<ExprError> for ViewFailure {
    fn from(error: ExprError) -> ViewFailure {
        ViewFailure::Expr(error)
    }
}

/// A view that passed [`compile_view`]: its checks, and its `where` compiled and estimated.
#[derive(Debug)]
pub struct CompiledView {
    view: View,
    where_: Option<(CompiledExpr, Estimate)>,
}

impl CompiledView {
    pub fn view(&self) -> &View {
        &self.view
    }

    /// The estimate of the `where`, for the editor's note on `matches`. `None` for a view with no `where`.
    pub fn estimate(&self) -> Option<Estimate> {
        self.where_.as_ref().map(|(_, estimate)| *estimate)
    }
}

/// [`check_view`], then the `where`'s compile and its estimate against a board of `board_tasks` tasks, as one result
/// for one view.
pub fn compile_view(
    view: &View,
    board_tasks: usize,
    budgets: &Budgets,
) -> Result<CompiledView, ViewFailure> {
    check_view(view).map_err(ViewFailure::Check)?;
    let where_ = match &view.where_ {
        Some(source) => {
            let compiled = compile(source)?;
            let estimate = check_cost(&compiled, board_tasks, budgets)?;
            Some((compiled, estimate))
        }
        None => None,
    };
    Ok(CompiledView {
        view: view.clone(),
        where_,
    })
}

/// What went wrong on one view. A clean view has the default: nothing, nothing and `false`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ViewCounts {
    /// The tasks whose `where` raised an error, counted by reason. Each is excluded from `where`'s matches.
    pub errors: BTreeMap<String, u64>,
    /// The tasks that ran out of their own step budget.
    pub over_task_budget: u64,
    /// The pass ran out of its budget: the tasks after the last one tested were not tested.
    pub pass_budget_exhausted: bool,
    /// The tasks the `where` pass did not reach.
    pub not_reached: usize,
    /// The keys of `include` that match no task, so that a typo shows.
    pub unmatched_include: Vec<String>,
    /// The keys of `exclude` that match no task.
    pub unmatched_exclude: Vec<String>,
    /// The closure's goal, when it matches no task. This is an error on the view, and the closure is empty.
    pub goal_unmatched: Option<String>,
    /// The corpus's invalid items. Every view counts them: an unreadable file has no fields to match.
    pub invalid_items: usize,
}

impl ViewCounts {
    /// The tasks that raised an error, over every reason.
    pub fn error_count(&self) -> u64 {
        self.errors.values().sum()
    }
}

/// A view's members and the steps of its closure.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ViewResult {
    /// The keys of the members, in the corpus's order.
    pub members: Vec<String>,
    /// The closure's layers, in key order inside each. See the module comment.
    pub steps: Vec<Vec<String>>,
    /// The tasks of the closure that lie on a dependency cycle, in key order. When there are any, the last step is
    /// the tasks the layering could not place: these, and the tasks that wait on them.
    pub cycle: Vec<String>,
    pub counts: ViewCounts,
}

/// The members, steps and counts of `compiled` over `corpus`.
pub fn evaluate_view(
    compiled: &CompiledView,
    corpus: &Corpus,
    budgets: &Budgets,
) -> Result<ViewResult, ViewFailure> {
    let view = &compiled.view;
    let rows = corpus.rows();
    let mut counts = ViewCounts {
        invalid_items: corpus.invalid().len(),
        ..ViewCounts::default()
    };
    let mut members: BTreeSet<String> = BTreeSet::new();

    if let Some((expression, _)) = &compiled.where_ {
        members.extend(matching(expression, corpus, &rows, budgets, &mut counts)?);
    }
    let (included, unmatched) = resolve_keys(corpus, &view.include);
    counts.unmatched_include = unmatched;
    members.extend(included);
    let (excluded, unmatched) = resolve_keys(corpus, &view.exclude);
    counts.unmatched_exclude = unmatched;

    let mut closure = Reach::default();
    if let Some(spec) = &view.closure {
        match corpus.resolve_key(&spec.goal) {
            Some(goal) => closure = walk(corpus, &rows, spec, &goal),
            None => counts.goal_unmatched = Some(spec.goal.clone()),
        }
    }
    members.extend(closure.set.iter().cloned());

    let (mut steps, mut cycle) = layers(&closure);
    steps
        .iter_mut()
        .for_each(|step| step.retain(|key| !excluded.contains(key)));
    steps.retain(|step| !step.is_empty());
    cycle.retain(|key| !excluded.contains(key));

    let mut seen = BTreeSet::new();
    let members = rows
        .iter()
        .map(|row| &row.task.key)
        .filter(|key| members.contains(*key) && !excluded.contains(*key) && seen.insert(*key))
        .cloned()
        .collect();
    Ok(ViewResult {
        members,
        steps,
        cycle,
        counts,
    })
}

/// The keys of the rows `expression` matches, with what went wrong written into `counts`.
fn matching(
    expression: &CompiledExpr,
    corpus: &Corpus,
    rows: &[Row<'_>],
    budgets: &Budgets,
    counts: &mut ViewCounts,
) -> Result<Vec<String>, ViewFailure> {
    let mut bound = Vec::with_capacity(rows.len());
    let mut keys = Vec::with_capacity(rows.len());
    for row in rows {
        let source = row
            .task
            .key
            .split_once(':')
            .map_or("", |(source, _)| source);
        let kind = corpus.kind_of(source, row.task);
        // The corpus's ingest caps every task, so a bind that fails here is a bug, counted and not hidden.
        match TaskVars::bind(row.task, &kind, None) {
            Ok(vars) => {
                bound.push(vars);
                keys.push(row.task.key.clone());
            }
            Err(error) => *counts.errors.entry(error.reason).or_default() += 1,
        }
    }
    let outcome = evaluate_pass(expression, &bound, budgets)?;
    for (reason, tasks) in &outcome.errors {
        *counts.errors.entry(reason.clone()).or_default() += tasks;
    }
    counts.over_task_budget = outcome.over_task_budget;
    counts.pass_budget_exhausted = outcome.pass_budget_exhausted;
    counts.not_reached = outcome.not_reached;
    Ok(outcome
        .matched
        .into_iter()
        .map(|position| keys[position].clone())
        .collect())
}

/// The keys the corpus holds, for `wanted`, and the entries that match none.
fn resolve_keys(corpus: &Corpus, wanted: &[String]) -> (BTreeSet<String>, Vec<String>) {
    let mut found = BTreeSet::new();
    let mut unmatched = Vec::new();
    for key in wanted {
        match corpus.resolve_key(key) {
            Some(held) => {
                found.insert(held);
            }
            None => unmatched.push(key.clone()),
        }
    }
    (found, unmatched)
}

/// A closure: its tasks, and the dependencies each has inside the set.
#[derive(Debug, Default)]
struct Reach {
    set: BTreeSet<String>,
    needs: BTreeMap<String, BTreeSet<String>>,
}

fn is_open(task: &Task) -> bool {
    !matches!(task.state.category, Category::Done | Category::Dropped)
}

/// The closure of `goal`: the goal, and every open task it reaches through open tasks.
fn walk(corpus: &Corpus, rows: &[Row<'_>], spec: &Closure, goal: &str) -> Reach {
    let mut by_key: HashMap<&str, &Task> = HashMap::new();
    for row in rows {
        by_key.entry(row.task.key.as_str()).or_insert(row.task);
    }
    let mut reached: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut pending = vec![goal.to_string()];
    let mut set: BTreeSet<String> = BTreeSet::from([goal.to_string()]);
    while let Some(key) = pending.pop() {
        let Some(task) = by_key.get(key.as_str()) else {
            continue;
        };
        let source = key.split_once(':').map_or("", |(source, _)| source);
        let targets = match spec.follow {
            Follow::DependsOn => dependencies(task),
        };
        let mut found = BTreeSet::new();
        for target in targets {
            let Some(held) = corpus.resolve_target(source, &target) else {
                continue;
            };
            if by_key.get(held.as_str()).is_some_and(|task| is_open(task))
                && set.insert(held.clone())
            {
                pending.push(held.clone());
            }
            found.insert(held);
        }
        reached.insert(key, found);
    }
    let needs = reached
        .into_iter()
        .map(|(key, found)| {
            (
                key,
                found
                    .into_iter()
                    .filter(|held| set.contains(held))
                    .collect(),
            )
        })
        .collect();
    Reach { set, needs }
}

/// The closure's layers, and the tasks on a cycle.
fn layers(closure: &Reach) -> (Vec<Vec<String>>, Vec<String>) {
    let mut steps = Vec::new();
    let mut placed: BTreeSet<&String> = BTreeSet::new();
    let mut remaining: BTreeSet<&String> = closure.set.iter().collect();
    while !remaining.is_empty() {
        let layer: Vec<&String> = remaining
            .iter()
            .copied()
            .filter(|key| closure.needs[*key].iter().all(|need| placed.contains(need)))
            .collect();
        if layer.is_empty() {
            let cycle = remaining
                .iter()
                .copied()
                .filter(|key| reaches_itself(closure, &remaining, key))
                .cloned()
                .collect();
            steps.push(remaining.into_iter().cloned().collect());
            return (steps, cycle);
        }
        for key in &layer {
            remaining.remove(*key);
            placed.insert(*key);
        }
        steps.push(layer.into_iter().cloned().collect());
    }
    (steps, Vec::new())
}

/// Whether `start` depends, through `among`, on itself.
fn reaches_itself(closure: &Reach, among: &BTreeSet<&String>, start: &String) -> bool {
    let mut visited: BTreeSet<&String> = BTreeSet::new();
    let mut pending: Vec<&String> = closure.needs[start].iter().collect();
    while let Some(key) = pending.pop() {
        if key == start {
            return true;
        }
        if among.contains(key) && visited.insert(key) {
            pending.extend(closure.needs[key].iter());
        }
    }
    false
}
