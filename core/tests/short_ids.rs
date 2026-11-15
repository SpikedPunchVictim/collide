//! Tests for short-form id lookup (ts-0020) and the epic helpers beside it.

use collide_core::provider::Task;
use collide_core::short_id::{Lookup, epic_short_form, resolve};
use serde_json::json;

fn task(id: &str, epic: Option<&str>) -> Task {
    let links: Vec<_> = epic
        .map(|epic| json!({ "type": "part-of", "to": epic }))
        .into_iter()
        .collect();
    serde_json::from_value(json!({
        "key": format!("plan:{id}"),
        "id": id,
        "title": id,
        "state": { "status": "ready", "category": "ready" },
        "version": "v",
        "links": links,
    }))
    .unwrap()
}

fn corpus() -> Vec<Task> {
    vec![
        task("ep-002-va", None),
        task("ts-0007-ga", Some("ep-002-va")),
        task("ts-0012-ua", Some("ep-002-va")),
        task("ts-0012-qb", Some("ep-003-da")),
        task("ts-0030-zz", Some("ep-003-da")),
    ]
}

fn one(tasks: &[Task], query: &str) -> String {
    match resolve(tasks, query) {
        Lookup::One(task) => task.id.clone(),
        other => panic!("`{query}` should match one item, got {other:?}"),
    }
}

fn ids(lookup: &Lookup<'_>) -> Vec<String> {
    match lookup {
        Lookup::Many(tasks) => tasks.iter().map(|task| task.id.clone()).collect(),
        other => panic!("expected several matches, got {other:?}"),
    }
}

#[test]
fn a_number_that_matches_one_item_resolves() {
    assert_eq!(one(&corpus(), "7"), "ts-0007-ga");
    assert_eq!(one(&corpus(), "30"), "ts-0030-zz");
}

#[test]
fn a_zero_padded_number_resolves_too() {
    assert_eq!(one(&corpus(), "0007"), "ts-0007-ga");
    assert_eq!(one(&corpus(), "007"), "ts-0007-ga");
}

#[test]
fn a_number_and_suffix_pick_one_of_two_items_with_the_number() {
    assert_eq!(one(&corpus(), "12-ua"), "ts-0012-ua");
    assert_eq!(one(&corpus(), "0012-qb"), "ts-0012-qb");
}

#[test]
fn a_number_that_two_items_share_lists_both_in_id_order() {
    let corpus = corpus();
    assert_eq!(ids(&resolve(&corpus, "12")), ["ts-0012-qb", "ts-0012-ua"]);
}

#[test]
fn a_task_and_an_epic_with_one_number_are_two_matches_until_the_prefix_says_which() {
    let mut corpus = corpus();
    corpus.push(task("ep-007-xx", None));
    assert_eq!(ids(&resolve(&corpus, "7")), ["ep-007-xx", "ts-0007-ga"]);
    assert_eq!(one(&corpus, "ts-7"), "ts-0007-ga");
    assert_eq!(one(&corpus, "ep-7"), "ep-007-xx");
}

#[test]
fn the_full_id_resolves_even_when_a_short_form_would_be_ambiguous() {
    assert_eq!(one(&corpus(), "ts-0012-ua"), "ts-0012-ua");
    assert_eq!(one(&corpus(), "ep-002-va"), "ep-002-va");
}

#[test]
fn a_number_no_item_has_matches_nothing() {
    assert!(matches!(resolve(&corpus(), "99"), Lookup::None));
    assert!(matches!(resolve(&corpus(), "12-zz"), Lookup::None));
}

#[test]
fn text_that_is_no_id_form_is_malformed() {
    for query in [
        "",
        "abc",
        "12-",
        "-ua",
        "12-u",
        "12-uaa",
        "1.5",
        "ts-",
        "99999999999999999999999",
    ] {
        assert!(
            matches!(resolve(&corpus(), query), Lookup::Malformed),
            "`{query}`"
        );
    }
}

#[test]
fn the_epic_comes_from_the_part_of_link() {
    let corpus = corpus();
    assert_eq!(corpus[1].epic(), Some("ep-002-va"));
    assert_eq!(corpus[0].epic(), None);
}

#[test]
fn an_epic_is_shown_short_without_its_suffix() {
    assert_eq!(epic_short_form("ep-002-va"), "ep-002");
    assert_eq!(epic_short_form("ep-002"), "ep-002");
    assert_eq!(epic_short_form("ep-1000-zz"), "ep-1000");
}
