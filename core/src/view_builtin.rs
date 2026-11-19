//! The views Collide itself holds, which no `where` can say (ts-0135-cx; 0.10, Views). A view's expression sees one
//! task, and whether a task is ready depends on the states of the tasks that block it. So `ready` is a rule in core,
//! the one `collide tasks --ready` applies ([`crate::corpus::Corpus::readiness`]), and the two agree by construction.
//!
//! - **Members** are the tasks the ready rule lists, in the corpus's order, each key once.
//! - **The report** names each `blocked-by` cycle (`cycle`, the keys on one, in key order) and each blocker that is no
//!   item (`unknown_blockers`). None of those tasks is ready.
//! - It holds no `where`, `include`, `exclude` or closure: [`ready_view`] is a name and nothing else.

use crate::corpus::Corpus;
use crate::view::View;
use crate::view_eval::{ViewCounts, ViewResult};
use std::collections::BTreeSet;

/// The name of the built-in view of the tasks that are ready.
pub const READY: &str = "ready";

/// The `ready` view as `task.views` lists it: its name, and no part.
pub fn ready_view() -> View {
    View {
        name: READY.to_string(),
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
