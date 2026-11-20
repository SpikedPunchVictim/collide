//! Tests for short-form id lookup (ts-0020) and the epic helpers beside it.

use collide_core::provider::Task;
use collide_core::short_id::{Lookup, epic_short_form, matching_ids, resolve};
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

#[test]
fn matching_ids_reads_a_short_form_over_bare_ids() {
    let ids = [
        "ts-0012-ua",
        "ts-0012-qb",
        "ts-0030-zz",
        "plan:ts-0012-ua",
        "T-01-042",
    ];
    assert_eq!(matching_ids(ids, "12"), ["ts-0012-ua", "ts-0012-qb"]);
    assert_eq!(matching_ids(ids, "12-qb"), ["ts-0012-qb"]);
    assert!(matching_ids(ids, "99").is_empty());
    assert!(matching_ids(ids, "T-01-042").is_empty(), "not a short form");
}

// ---- a source whose ids are not Collide's (ts-0149-kq) ----

/// An item of another source: `key` is `source:short`, `id` is the long form the source gives it.
fn foreign(key: &str, id: &str) -> Task {
    let mut item = task(id, None);
    item.key = key.to_string();
    item
}

fn mixed() -> Vec<Task> {
    let mut items = corpus();
    items.push(foreign("plan:WORK-12", "WORK-12-mend-the-door"));
    items.push(foreign("plan:WORK-13", "WORK-13-paint-it"));
    items
}

#[test]
fn a_bare_id_that_is_the_short_id_of_one_item_names_it_whatever_its_shape() {
    assert_eq!(one(&mixed(), "WORK-12"), "WORK-12-mend-the-door");
}

#[test]
fn a_bare_id_that_is_the_whole_id_of_one_item_names_it() {
    assert_eq!(
        one(&mixed(), "WORK-13-paint-it"),
        "WORK-13-paint-it",
        "the long form"
    );
}

#[test]
fn two_sources_with_one_short_id_are_ambiguous_and_listed_in_id_order() {
    let mut items = mixed();
    items.push(foreign("other:WORK-12", "WORK-12-other"));

    let found = resolve(&items, "WORK-12");

    assert_eq!(ids(&found), ["WORK-12-mend-the-door", "WORK-12-other"]);
}

#[test]
fn an_exact_match_is_taken_before_collides_short_forms() {
    let mut items = mixed();
    items.push(foreign("other:12", "12"));

    assert_eq!(one(&items, "12"), "12");
}

#[test]
fn a_short_form_still_reads_when_no_item_has_that_id_exactly() {
    assert_eq!(one(&mixed(), "7"), "ts-0007-ga");
}

#[test]
fn a_query_that_names_no_item_and_is_no_form_is_malformed_even_with_foreign_items() {
    assert!(matches!(resolve(&mixed(), "WORK-99"), Lookup::Malformed));
}
