//! Short ids (0.10, ids): `12` or `12-ua` name the item `ts-0012-ua` when exactly one item matches. The number is
//! compared as a number, so zero padding does not matter, and a prefix (`ts-12`) narrows the match to one kind of
//! item. The epic of an item is part of what a person needs to tell two matches apart, so the epic helpers live here.

use crate::provider::Task;

/// What an id, or a short form of one, says: an optional two-letter prefix, a number and an optional two-letter
/// suffix.
#[derive(Debug, Clone, PartialEq, Eq)]
struct IdParts<'a> {
    prefix: Option<&'a str>,
    number: u64,
    suffix: Option<&'a str>,
}

fn is_letters(text: &str) -> bool {
    text.len() == 2 && text.bytes().all(|byte| byte.is_ascii_lowercase())
}

fn parse_number(text: &str) -> Option<u64> {
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

/// Reads `12`, `12-ua`, `ts-12` and `ts-0012-ua`; anything else is `None`.
fn parse(text: &str) -> Option<IdParts<'_>> {
    let parts: Vec<&str> = text.split('-').collect();
    let (prefix, number, suffix) = match parts.as_slice() {
        [number] => (None, *number, None),
        [first, second] if parse_number(first).is_some() => (None, *first, Some(*second)),
        [first, second] => (Some(*first), *second, None),
        [prefix, number, suffix] => (Some(*prefix), *number, Some(*suffix)),
        _ => return None,
    };
    if prefix.is_some_and(|prefix| !is_letters(prefix)) || suffix.is_some_and(|s| !is_letters(s)) {
        return None;
    }
    Some(IdParts {
        prefix,
        number: parse_number(number)?,
        suffix,
    })
}

/// Whether `item` is something that `query` could mean.
fn matches(query: &IdParts<'_>, item: &IdParts<'_>) -> bool {
    query.number == item.number
        && query
            .prefix
            .is_none_or(|prefix| item.prefix == Some(prefix))
        && query
            .suffix
            .is_none_or(|suffix| item.suffix == Some(suffix))
}

/// What `query` named among the items.
#[derive(Debug)]
pub enum Lookup<'a> {
    /// Exactly one item.
    One(&'a Task),
    /// Several items, in id order: the caller asks for more of the id.
    Many(Vec<&'a Task>),
    /// The query is an id form and no item has it.
    None,
    /// The query is not an id or a short form of one.
    Malformed,
}

/// Finds the item `query` names: a full id first, then a short form. A short form that fits several items is
/// `Many`, never a guess.
pub fn resolve<'a>(tasks: &'a [Task], query: &str) -> Lookup<'a> {
    if let Some(task) = tasks.iter().find(|task| task.id == query) {
        return Lookup::One(task);
    }
    let Some(query) = parse(query) else {
        return Lookup::Malformed;
    };
    let mut found: Vec<&Task> = tasks
        .iter()
        .filter(|task| parse(&task.id).is_some_and(|item| matches(&query, &item)))
        .collect();
    found.sort_by(|a, b| a.id.cmp(&b.id));
    match found.as_slice() {
        [] => Lookup::None,
        [one] => Lookup::One(one),
        _ => Lookup::Many(found),
    }
}

/// The ids among `ids` that `query` could mean as a short form (`12`, `12-ua`, `ts-12`), as `resolve` reads one. It is
/// empty when `query` is not a short form or no id fits. It is for a caller that holds ids and not [`Task`]s.
pub fn matching_ids<'a>(ids: impl IntoIterator<Item = &'a str>, query: &str) -> Vec<&'a str> {
    let Some(query) = parse(query) else {
        return Vec::new();
    };
    ids.into_iter()
        .filter(|id| parse(id).is_some_and(|item| matches(&query, &item)))
        .collect()
}

/// An epic id without its random suffix: `ep-002-va` is shown as `ep-002` beside a link target.
pub fn epic_short_form(epic: &str) -> &str {
    match epic.rsplit_once('-') {
        Some((head, suffix)) if is_letters(suffix) && head.contains('-') => head,
        _ => epic,
    }
}
