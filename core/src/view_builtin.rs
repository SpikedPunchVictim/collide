//! The views Collide itself holds, which no `where` can say (ts-0135-cx, ts-0186-ch; 0.10, Views). A view's expression
//! sees one task, and whether a task is ready depends on the states of the tasks that block it. So `ready` is a rule in
//! core, the one `collide tasks --ready` applies ([`crate::corpus::Corpus::readiness`]), and the two agree by
//! construction.
//!
//! - **`ready`: members** are the tasks the ready rule lists, in the corpus's order, each key once.
//! - **`ready`: the report** names each `blocked-by` cycle (`cycle`, the keys on one, in key order) and each blocker
//!   that is no item (`unknown_blockers`). None of those tasks is ready.
//! - **`bottlenecks`: members** are the open tasks (a category other than `done` and `dropped`) that
//!   [`crate::corpus::Corpus::unblocks`] gives at least [`BOTTLENECK_MIN`], the most first, ties in key order. A view's
//!   `layout.sort` is the surface's, so the order cannot be a `where` and a sort: it is a rule here, and `task.list`
//!   lists through it.
//! - Neither holds a `where`, `include`, `exclude` or closure: [`ready_view`] and [`bottlenecks_view`] are a name and
//!   nothing else.

use crate::corpus::Corpus;
use crate::view::View;
use crate::view_eval::{ViewCounts, ViewResult, is_open};
use std::collections::BTreeSet;

/// The name of the built-in view of the tasks that are ready.
pub const READY: &str = "ready";

/// The name of the built-in view of the tasks that many open tasks wait on.
pub const BOTTLENECKS: &str = "bottlenecks";

/// How many open tasks must wait on a task for it to be in `bottlenecks` (note 41).
pub const BOTTLENECK_MIN: u32 = 3;

/// A view core holds. They are listed in this order, first in the order of precedence ([`BuiltinView::ALL`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinView {
    Ready,
    Bottlenecks,
}

impl BuiltinView {
    pub const ALL: [BuiltinView; 2] = [BuiltinView::Ready, BuiltinView::Bottlenecks];

    /// The built-in view called `name`, when there is one.
    pub fn named(name: &str) -> Option<BuiltinView> {
        BuiltinView::ALL
            .into_iter()
            .find(|each| each.name() == name)
    }

    pub fn name(self) -> &'static str {
        match self {
            BuiltinView::Ready => READY,
            BuiltinView::Bottlenecks => BOTTLENECKS,
        }
    }

    /// The view as `task.views` lists it: its name, and no part.
    pub fn view(self) -> View {
        named_view(self.name())
    }

    /// The members of the view over `corpus`, and what its rule found wrong on the way.
    pub fn evaluate(self, corpus: &Corpus) -> ViewResult {
        match self {
            BuiltinView::Ready => evaluate_ready(corpus),
            BuiltinView::Bottlenecks => evaluate_bottlenecks(corpus),
        }
    }
}

/// The `ready` view as `task.views` lists it: its name, and no part.
pub fn ready_view() -> View {
    named_view(READY)
}

/// The `bottlenecks` view as `task.views` lists it: its name, and no part.
pub fn bottlenecks_view() -> View {
    named_view(BOTTLENECKS)
}

fn named_view(name: &str) -> View {
    View {
        name: name.to_string(),
        where_: None,
        include: Vec::new(),
        exclude: Vec::new(),
        closure: None,
        workflow: None,
        profile: None,
        layout: None,
        queue: false,
    }
}

/// The members of `ready` over `corpus`, and what the rule found wrong on the way.
pub fn evaluate_ready(corpus: &Corpus) -> ViewResult {
    let found = corpus.readiness();
    let mut seen = BTreeSet::new();
    let mut part_problems = 0;
    let members = corpus
        .rows()
        .iter()
        .filter(|row| found.ready.contains(&row.task.key) && seen.insert(row.task.key.clone()))
        .map(|row| {
            part_problems += row.task.part_problems.len();
            row.task.key.clone()
        })
        .collect();
    let mut cycle: Vec<String> = found.cycles.into_iter().flatten().collect();
    cycle.sort();
    ViewResult {
        members,
        steps: Vec::new(),
        cycle,
        unknown_blockers: found.unknown_blockers,
        counts: ViewCounts {
            invalid_items: corpus.invalid().len(),
            part_problems,
            ..ViewCounts::default()
        },
    }
}

/// The members of `bottlenecks` over `corpus`: the open tasks with `unblocks >= BOTTLENECK_MIN`, the most first, ties
/// in key order, each key once.
pub fn evaluate_bottlenecks(corpus: &Corpus) -> ViewResult {
    let unblocks = corpus.unblocks();
    let mut seen = BTreeSet::new();
    let mut part_problems = 0;
    let mut found: Vec<(u32, String)> = Vec::new();
    for row in corpus.rows() {
        let key = &row.task.key;
        let count = unblocks.get(key).copied().unwrap_or(0);
        if is_open(row.task) && count >= BOTTLENECK_MIN && seen.insert(key.clone()) {
            part_problems += row.task.part_problems.len();
            found.push((count, key.clone()));
        }
    }
    found.sort_by(|(more, first), (less, second)| less.cmp(more).then_with(|| first.cmp(second)));
    ViewResult {
        members: found.into_iter().map(|(_, key)| key).collect(),
        counts: ViewCounts {
            invalid_items: corpus.invalid().len(),
            part_problems,
            ..ViewCounts::default()
        },
        ..ViewResult::default()
    }
}
