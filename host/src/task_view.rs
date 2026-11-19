//! What `collide task <id>` prints (ts-0020, ts-0136-rr). The text is built from the daemon's `task.get` answer: the
//! item, its links one hop out in both directions with the other end's state, and the link types its source declares,
//! so an incoming link is named by its inverse ("evidence for"). The JSON form is the answer itself.

use collide_core::feature::derived_state;
use collide_core::hook::HookLink;
use collide_core::item::{Category, Coverage, Feature, LinkDir, LinkTypeDecl};
use collide_core::owner::TaskGetResult;
use collide_core::provider::Task;
use serde_json::Value;

/// The link type that a task builds a feature with.
const IMPLEMENTS: &str = "implements";

/// The link type that makes an item part of an epic: the epic is a line of its own, and the epic's own children are
/// not listed.
const PART_OF: &str = "part-of";

/// What the print calls a feature with no section.
const UNSORTED: &str = "Unsorted";

/// The widest a field's name is padded to, at least: the names of the lines every item has.
const NAME_WIDTH: usize = 7;

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

/// `ep-002-va / ts-0007-ga`, or just the id for an item with no epic.
pub fn heading(task: &Task) -> String {
    match task.epic() {
        Some(epic) => format!("{epic} / {}", task.id),
        None => task.id.clone(),
    }
}

/// How an incoming link reads from the item it points at: its type's inverse when the source declares one.
fn incoming_label(link: &HookLink, types: &[LinkTypeDecl]) -> String {
    types
        .iter()
        .find(|decl| decl.name == link.link_type)
        .and_then(|decl| decl.inverse.clone())
        .unwrap_or_else(|| format!("{} (incoming)", link.link_type))
}

/// The other end of a link: its key, the feature it names, and its status (`(not found)` when the corpus holds no
/// such item, `(proposal or feature not found)` for a link to a feature that the corpus cannot place).
fn other_end(link: &HookLink) -> String {
    let key = match &link.feature {
        Some(feature) => format!("{}#{feature}", link.key),
        None => link.key.clone(),
    };
    if link.missing {
        let what = if link.feature.is_some() {
            "proposal or feature not found"
        } else {
            "not found"
        };
        return format!("{key} ({what})");
    }
    format!("{key} ({})", link.status)
}

/// Why an item is or is not ready, in a few words, from its own links: a blocker that is not done or dropped, or is no
/// item, keeps it waiting. The daemon's `ready` view is the whole rule, and the one that finds a cycle.
fn ready_note(task: &Task, links: &[HookLink]) -> String {
    if task.state.category != Category::Ready {
        return format!("no (status is {})", task.state.status);
    }
    let waiting: Vec<String> = task
        .links
        .iter()
        .filter(|link| link.link_type == "blocked-by" && link.blocking != Some(false))
        .filter_map(|link| {
            let blocker = links.iter().find(|held| {
                held.dir == LinkDir::Out
                    && held.link_type == link.link_type
                    && held
                        .key
                        .split_once(':')
                        .map_or(held.key.as_str(), |(_, id)| id)
                        == link.to
            });
            match blocker {
                Some(blocker) if blocker.missing => Some(format!("{} (not found)", link.to)),
                Some(blocker) if matches!(blocker.category, Category::Done | Category::Dropped) => {
                    None
                }
                Some(blocker) => Some(format!("{} ({})", link.to, blocker.status)),
                None => Some(format!("{} (not found)", link.to)),
            }
        })
        .collect();
    if waiting.is_empty() {
        "yes".to_string()
    } else {
        format!("no (waiting on {})", waiting.join(", "))
    }
}

/// The lines under a feature: its derived state, then each task whose `implements` link names it, with its status.
fn feature_lines(proposal: &Task, feature: &Feature, links: &[HookLink]) -> String {
    let implementers: Vec<&HookLink> = links
        .iter()
        .filter(|link| {
            link.dir == LinkDir::In
                && link.link_type == IMPLEMENTS
                && link.feature.as_deref() == Some(feature.id.as_str())
        })
        .collect();
    let categories: Vec<Category> = implementers.iter().map(|link| link.category).collect();
    let state = match derived_state(&proposal.state.status, &categories) {
        Ok(state) => state.to_string(),
        Err(error) => format!("unknown ({error})"),
    };
    let mut text = format!("        state  {state}\n");
    for link in implementers {
        text.push_str(&format!("        {}  {}\n", link.key, link.status));
    }
    text
}

/// What an item's coverage says, as one line: the features no task implements and the included tasks that implement no
/// feature, each counted and named.
fn coverage_text(coverage: &Coverage) -> String {
    if coverage.features_without_task.is_empty() && coverage.tasks_without_feature.is_empty() {
        return "every feature has a task, and every included task has a feature".to_string();
    }
    let mut parts = Vec::new();
    if !coverage.features_without_task.is_empty() {
        parts.push(format!(
            "{} feature(s) with no task ({})",
            coverage.features_without_task.len(),
            coverage.features_without_task.join(", ")
        ));
    }
    if !coverage.tasks_without_feature.is_empty() {
        parts.push(format!(
            "{} included task(s) with no feature ({})",
            coverage.tasks_without_feature.len(),
            coverage.tasks_without_feature.join(", ")
        ));
    }
    parts.join("; ")
}

/// A field's value as one line: text as it is, anything else as compact JSON.
fn field_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

/// One item in full: heading, fields, links, then the features, and the body. A line is left out when the item has
/// nothing for it. Claims and the problems in the item's parts are counted.
pub fn show_text(got: &TaskGetResult, task: &Task) -> String {
    let mut lines: Vec<(String, String)> = vec![
        ("title".to_string(), task.title.clone()),
        ("kind".to_string(), task.kind.clone().unwrap_or_default()),
        ("status".to_string(), task.state.status.clone()),
        ("ready".to_string(), ready_note(task, &got.links)),
    ];
    if !task.owners.is_empty() {
        lines.push(("owners".to_string(), task.owners.join(", ")));
    }
    if let Some(path) = &task.path {
        lines.push(("path".to_string(), path.clone()));
    }
    for (name, value) in &task.fields {
        lines.push((name.clone(), field_text(value)));
    }
    if !task.claims.is_empty() {
        lines.push(("claims".to_string(), task.claims.len().to_string()));
    }
    if let Some(coverage) = &task.coverage {
        lines.push(("coverage".to_string(), coverage_text(coverage)));
    }
    if !task.part_problems.is_empty() {
        lines.push((
            "problems".to_string(),
            format!("{} in the parts of this item", task.part_problems.len()),
        ));
    }
    let width = lines
        .iter()
        .map(|(name, _)| name.chars().count())
        .max()
        .unwrap_or(0)
        .max(NAME_WIDTH);
    let mut text = format!("{}\n", heading(task));
    for (name, value) in &lines {
        text.push_str(&format!("  {name:<width$} {value}\n"));
    }
    let links: Vec<&HookLink> = got
        .links
        .iter()
        .filter(|link| link.link_type != PART_OF)
        .collect();
    if !links.is_empty() {
        text.push_str("  links\n");
        for link in links {
            let label = match link.dir {
                LinkDir::Out => link.link_type.clone(),
                LinkDir::In => incoming_label(link, &got.link_types),
            };
            text.push_str(&format!("    {label} {}\n", other_end(link)));
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
                text.push_str(&feature_lines(task, feature, &got.links));
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
