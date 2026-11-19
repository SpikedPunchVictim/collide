//! The ready rule (0.10, Collide's own build tasks): an item is ready when its status category is `ready` and every
//! `blocked-by` target is `done` or `dropped`. A `blocked-by` cycle is reported and none of its items is ready. A
//! `blocked-by` target that is no item is reported, and its item is not ready: a typo in a link must not make an
//! item look free to start. A pure function over the items a provider lists; it reads nothing else. Which item a
//! target names is not its business: the caller passes the resolver, the one that `follow` walks.

use crate::item::Category;
use crate::provider::Task;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// The link type that blocks (the plan provider declares it `blocking: true`).
const BLOCKED_BY: &str = "blocked-by";

/// A `blocked-by` link whose target is no item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UnknownTarget {
    pub from: String,
    pub to: String,
}

/// What the rule found over a set of items. Items are named by `key`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Readiness {
    /// The keys of the ready items, in the order the items were given.
    pub ready: Vec<String>,
    /// Each `blocked-by` cycle (or an item that blocks itself): its keys in sorted order, cycles sorted.
    pub cycles: Vec<Vec<String>>,
    /// Each blocker that is no item, in the order of the items.
    pub unknown_targets: Vec<UnknownTarget>,
}

fn blockers(task: &Task) -> impl Iterator<Item = &str> {
    task.links
        .iter()
        .filter(|link| link.link_type == BLOCKED_BY && link.blocking != Some(false))
        .map(|link| link.to.as_str())
}

/// Applies the rule to `tasks`. `resolve` gives the key of the item a link's target names, or `None` when it names
/// none. A key it gives that is not one of `tasks` counts as no item. With two items of one key the first one counts.
pub fn readiness(tasks: &[Task], resolve: impl Fn(&str) -> Option<String>) -> Readiness {
    let mut by_key: HashMap<&str, usize> = HashMap::new();
    for (position, task) in tasks.iter().enumerate() {
        by_key.entry(task.key.as_str()).or_insert(position);
    }
    let index = |target: &str| resolve(target).and_then(|key| by_key.get(key.as_str()).copied());

    let mut unknown_targets = Vec::new();
    let mut edges: Vec<Vec<usize>> = vec![Vec::new(); tasks.len()];
    for (position, task) in tasks.iter().enumerate() {
        for target in blockers(task) {
            match index(target) {
                Some(to) => edges[position].push(to),
                None => unknown_targets.push(UnknownTarget {
                    from: task.key.clone(),
                    to: target.to_string(),
                }),
            }
        }
    }

    let cycles = find_cycles(&edges);
    let in_cycle: Vec<bool> = {
        let mut flags = vec![false; tasks.len()];
        for member in cycles.iter().flatten() {
            flags[*member] = true;
        }
        flags
    };
    let finished = |position: usize| {
        matches!(
            tasks[position].state.category,
            Category::Done | Category::Dropped
        )
    };
    let ready = tasks
        .iter()
        .enumerate()
        .filter(|(position, task)| {
            task.state.category == Category::Ready
                && !in_cycle[*position]
                && blockers(task).all(|target| index(target).is_some_and(finished))
        })
        .map(|(_, task)| task.key.clone())
        .collect();

    let mut cycles: Vec<Vec<String>> = cycles
        .into_iter()
        .map(|members| {
            let mut keys: Vec<String> = members.iter().map(|&m| tasks[m].key.clone()).collect();
            keys.sort();
            keys
        })
        .collect();
    cycles.sort();
    Readiness {
        ready,
        cycles,
        unknown_targets,
    }
}

/// The strongly connected components of `edges` that are cycles: more than one node, or one node with an edge to
/// itself (Tarjan's algorithm).
fn find_cycles(edges: &[Vec<usize>]) -> Vec<Vec<usize>> {
    struct Walk<'a> {
        edges: &'a [Vec<usize>],
        next_index: usize,
        index: Vec<Option<usize>>,
        low_link: Vec<usize>,
        on_stack: Vec<bool>,
        stack: Vec<usize>,
        cycles: Vec<Vec<usize>>,
    }

    impl Walk<'_> {
        fn visit(&mut self, node: usize) {
            self.index[node] = Some(self.next_index);
            self.low_link[node] = self.next_index;
            self.next_index += 1;
            self.stack.push(node);
            self.on_stack[node] = true;
            for &target in &self.edges[node] {
                match self.index[target] {
                    None => {
                        self.visit(target);
                        self.low_link[node] = self.low_link[node].min(self.low_link[target]);
                    }
                    Some(order) if self.on_stack[target] => {
                        self.low_link[node] = self.low_link[node].min(order);
                    }
                    Some(_) => {}
                }
            }
            if Some(self.low_link[node]) == self.index[node] {
                let mut members = Vec::new();
                while let Some(member) = self.stack.pop() {
                    self.on_stack[member] = false;
                    members.push(member);
                    if member == node {
                        break;
                    }
                }
                if members.len() > 1 || self.edges[node].contains(&node) {
                    self.cycles.push(members);
                }
            }
        }
    }

    let count = edges.len();
    let mut walk = Walk {
        edges,
        next_index: 0,
        index: vec![None; count],
        low_link: vec![0; count],
        on_stack: vec![false; count],
        stack: Vec::new(),
        cycles: Vec::new(),
    };
    for node in 0..count {
        if walk.index[node].is_none() {
            walk.visit(node);
        }
    }
    walk.cycles
}
