//! What `collide tasks` and `collide task` print (ts-0020). The human form is compact and stable, because people and
//! agents read it: each item's epic stands beside its id (`ep-002-va / ts-0007-ga`) and each link's target shows
//! its epic in short form (`blocked-by ts-0009-cu (ep-002)`). The JSON form is the provider's task plus `epic` and
//! `ready` fields.

use collide_core::feature::{derived_state, parse_feature_target};
use collide_core::item::{Category, Feature};
use collide_core::provider::Task;
use collide_core::ready::Readiness;
use collide_core::short_id::epic_short_form;
use serde_json::{Value, json};
use std::collections::HashSet;

/// The link type that a task builds a feature with.
const IMPLEMENTS: &str = "implements";

/// What the print calls a feature with no section.
const UNSORTED: &str = "Unsorted";

/// The features by section: sections in order of first appearance, then the unsorted ones (an empty section) last.
fn by_section(features: &[Feature]) -> Vec<(&str, Vec<&Feature>)> {
    let mut named: Vec<(&str, Vec<&Feature>)> = Vec::new();
    let mut unsorted: Vec<&Feature> = Vec::new();
    for feature in features {
        if feature.section.is_empty() {
            unsorted.push(feature);
        } else if let Some((_, group)) = named.iter_mut().find(|(name, _)| *name == feature.section)
        {
            group.push(feature);
        } else {
            named.push((feature.section.as_str(), vec![feature]));
        }
    }
    if !unsorted.is_empty() {
        named.push((UNSORTED, unsorted));
    }
    named
}

/// The items of one corpus and the ready rule's answer over all of them.
pub struct View<'a> {
    tasks: &'a [Task],
    readiness: &'a Readiness,
    ready: HashSet<&'a str>,
}

impl<'a> View<'a> {
    pub fn new(tasks: &'a [Task], readiness: &'a Readiness) -> Self {
        Self {
            tasks,
            readiness,
            ready: readiness.ready.iter().map(String::as_str).collect(),
        }
    }

    pub fn is_ready(&self, task: &Task) -> bool {
        self.ready.contains(task.id.as_str())
    }

    /// `ep-002-va / ts-0007-ga`, or just the id for an item with no epic.
    pub fn heading(task: &Task) -> String {
        match task.epic() {
            Some(epic) => format!("{epic} / {}", task.id),
            None => task.id.clone(),
        }
    }

    /// `ts-0009-cu (ep-002)`: a link target with its epic, `(not found)` when no item has the id. A target with no
    /// epic (an epic itself) has no bracket. A feature target, `pr-0003-km#F2`, is shown with the epic of its proposal,
    /// and `(feature not found)` when the proposal has no such feature.
    fn target(&self, to: &str) -> String {
        let (id, feature) = match parse_feature_target(to) {
            Ok(target) => (target.item.clone(), Some(target)),
            Err(_) => (to.to_string(), None),
        };
        let Some(task) = self.tasks.iter().find(|task| task.id == id) else {
            return format!("{to} (not found)");
        };
        if let Some(target) = &feature {
            let wanted = target.feature_id();
            if !task.features.iter().any(|feature| feature.id == wanted) {
                return format!("{to} (feature not found)");
            }
        }
        match task.epic() {
            Some(epic) => format!("{to} ({})", epic_short_form(epic)),
            None => to.to_string(),
        }
    }

    /// The tasks whose `implements` link names feature `feature` of `proposal`, in the order of the items.
    fn implementers(&self, proposal: &Task, feature: &Feature) -> Vec<&Task> {
        self.tasks
            .iter()
            .filter(|task| {
                task.links.iter().any(|link| {
                    link.link_type == IMPLEMENTS
                        && parse_feature_target(&link.to).is_ok_and(|target| {
                            target.item == proposal.id && target.feature_id() == feature.id
                        })
                })
            })
            .collect()
    }

    /// The lines under a feature: its derived state, then each task that implements it with its epic and status.
    fn feature_lines(&self, proposal: &Task, feature: &Feature) -> String {
        let implementers = self.implementers(proposal, feature);
        let categories: Vec<Category> = implementers
            .iter()
            .map(|task| task.state.category)
            .collect();
        let state = match derived_state(&proposal.state.status, &categories) {
            Ok(state) => state.to_string(),
            Err(error) => format!("unknown ({error})"),
        };
        let mut text = format!("        state  {state}\n");
        for task in implementers {
            let epic = task.epic().map_or(String::new(), |epic| {
                format!(" ({})", epic_short_form(epic))
            });
            text.push_str(&format!(
                "        {}{epic}  {}  {}\n",
                task.id, task.state.status, task.title
            ));
        }
        text
    }

    /// One line per item: heading, status, kind, title, in columns padded to the widest entry. An item whose status
    /// is `ready` but that the rule holds back shows `ready (blocked)`.
    pub fn list_text(&self, shown: &[&Task]) -> String {
        let rows: Vec<[String; 4]> = shown
            .iter()
            .map(|task| {
                let status = if task.state.category == collide_core::item::Category::Ready
                    && !self.is_ready(task)
                {
                    format!("{} (blocked)", task.state.status)
                } else {
                    task.state.status.clone()
                };
                [
                    Self::heading(task),
                    status,
                    task.kind.clone().unwrap_or_default(),
                    task.title.clone(),
                ]
            })
            .collect();
        let width = |column: usize| {
            rows.iter()
                .map(|row| row[column].chars().count())
                .max()
                .unwrap_or(0)
        };
        let (w0, w1, w2) = (width(0), width(1), width(2));
        let mut text = String::new();
        for row in &rows {
            text.push_str(&format!(
                "{:w0$}  {:w1$}  {:w2$}  {}\n",
                row[0], row[1], row[2], row[3]
            ));
        }
        text
    }

    /// Why an item is or is not ready, in a few words.
    fn ready_note(&self, task: &Task) -> String {
        if self.is_ready(task) {
            return "yes".to_string();
        }
        if task.state.category != collide_core::item::Category::Ready {
            return format!("no (status is {})", task.state.status);
        }
        if let Some(cycle) = self
            .readiness
            .cycles
            .iter()
            .find(|cycle| cycle.contains(&task.id))
        {
            return format!("no (in a blocked-by cycle: {})", cycle.join(", "));
        }
        let waiting: Vec<String> = task
            .links
            .iter()
            .filter(|link| link.link_type == "blocked-by" && link.blocking != Some(false))
            .filter_map(|link| match self.tasks.iter().find(|t| t.id == link.to) {
                Some(blocker)
                    if matches!(
                        blocker.state.category,
                        collide_core::item::Category::Done | collide_core::item::Category::Dropped
                    ) =>
                {
                    None
                }
                Some(blocker) => Some(format!("{} ({})", link.to, blocker.state.status)),
                None => Some(format!("{} (not found)", link.to)),
            })
            .collect();
        format!("no (waiting on {})", waiting.join(", "))
    }

    /// One item in full: heading, fields, links (the `part-of` link is the heading's epic, so it is not repeated),
    /// then the body.
    pub fn show_text(&self, task: &Task) -> String {
        let mut text = format!("{}\n", Self::heading(task));
        let mut field =
            |name: &str, value: String| text.push_str(&format!("  {name:<7} {value}\n"));
        field("title", task.title.clone());
        field("kind", task.kind.clone().unwrap_or_default());
        field("status", task.state.status.clone());
        field("ready", self.ready_note(task));
        if !task.owners.is_empty() {
            field("owners", task.owners.join(", "));
        }
        if let Some(path) = &task.path {
            field("path", path.clone());
        }
        let links: Vec<&collide_core::item::Link> = task
            .links
            .iter()
            .filter(|link| link.link_type != "part-of")
            .collect();
        if !links.is_empty() {
            text.push_str("  links\n");
            for link in links {
                text.push_str(&format!(
                    "    {} {}\n",
                    link.link_type,
                    self.target(&link.to)
                ));
            }
        }
        if !task.features.is_empty() {
            text.push_str("  features\n");
            for (section, features) in by_section(&task.features) {
                text.push_str(&format!("    {section}\n"));
                for feature in features {
                    text.push_str(&format!(
                        "      {} {}: {}\n",
                        feature.id, feature.title, feature.summary
                    ));
                    text.push_str(&self.feature_lines(task, feature));
                }
            }
        }
        if !task.feature_problems.is_empty() {
            text.push_str("  feature problems\n");
            for problem in &task.feature_problems {
                text.push_str(&format!("    line {}: {}\n", problem.line, problem.reason));
            }
        }
        if let Some(body) = task.body.as_deref().filter(|body| !body.trim().is_empty()) {
            text.push('\n');
            text.push_str(body.trim_matches('\n').trim_end());
            text.push('\n');
        }
        text
    }

    /// The provider's task with `epic` (the epic's id, or null) and `ready` added.
    pub fn task_json(&self, task: &Task) -> Value {
        let mut value = serde_json::to_value(task).expect("a task is JSON");
        value["epic"] = json!(task.epic());
        value["ready"] = json!(self.is_ready(task));
        value
    }
}
