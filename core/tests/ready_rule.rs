//! Tests for the ready rule (ts-0020): a pure function over the tasks a provider lists.

use collide_core::provider::Task;
use collide_core::ready::{UnknownTarget, readiness};
use serde_json::json;

/// A task in `category` that is blocked by each of `blocked_by`.
fn task(id: &str, category: &str, blocked_by: &[&str]) -> Task {
    let links: Vec<_> = blocked_by
        .iter()
        .map(|to| json!({ "type": "blocked-by", "to": to }))
        .collect();
    serde_json::from_value(json!({
        "key": format!("plan:{id}"),
        "id": id,
        "title": id,
        "state": { "status": category, "category": category },
        "version": "v",
        "links": links,
    }))
    .unwrap()
}

fn ready_ids(tasks: &[Task]) -> Vec<String> {
    readiness(tasks).ready
}

#[test]
fn a_ready_task_with_no_links_is_ready() {
    assert_eq!(
        ready_ids(&[task("ts-0001-aa", "ready", &[])]),
        ["ts-0001-aa"]
    );
}

#[test]
fn only_the_ready_category_is_ready() {
    let tasks: Vec<_> = ["backlog", "active", "review", "done", "dropped"]
        .iter()
        .enumerate()
        .map(|(n, category)| task(&format!("ts-000{n}-aa"), category, &[]))
        .collect();
    assert!(ready_ids(&tasks).is_empty());
}

#[test]
fn a_task_blocked_by_an_unfinished_task_is_not_ready() {
    for blocker in ["backlog", "ready", "active", "review"] {
        let tasks = [
            task("ts-0001-aa", blocker, &[]),
            task("ts-0002-bb", "ready", &["ts-0001-aa"]),
        ];
        let ready = ready_ids(&tasks);
        assert!(
            !ready.contains(&"ts-0002-bb".to_string()),
            "blocked by a {blocker} task: {ready:?}"
        );
    }
}

#[test]
fn a_done_blocker_unblocks() {
    let tasks = [
        task("ts-0001-aa", "done", &[]),
        task("ts-0002-bb", "ready", &["ts-0001-aa"]),
    ];
    assert_eq!(ready_ids(&tasks), ["ts-0002-bb"]);
}

#[test]
fn a_dropped_blocker_unblocks() {
    let tasks = [
        task("ts-0001-aa", "dropped", &[]),
        task("ts-0002-bb", "ready", &["ts-0001-aa"]),
    ];
    assert_eq!(ready_ids(&tasks), ["ts-0002-bb"]);
}

#[test]
fn every_blocker_must_be_finished() {
    let tasks = [
        task("ts-0001-aa", "done", &[]),
        task("ts-0002-bb", "active", &[]),
        task("ts-0003-cc", "ready", &["ts-0001-aa", "ts-0002-bb"]),
    ];
    assert!(ready_ids(&tasks).is_empty());
}

#[test]
fn ready_ids_keep_the_order_of_the_input() {
    let tasks = [
        task("ts-0003-cc", "ready", &[]),
        task("ts-0001-aa", "ready", &[]),
        task("ts-0002-bb", "ready", &[]),
    ];
    assert_eq!(
        ready_ids(&tasks),
        ["ts-0003-cc", "ts-0001-aa", "ts-0002-bb"]
    );
}

#[test]
fn links_of_other_types_do_not_block() {
    let mut blocked = task("ts-0002-bb", "ready", &[]);
    blocked.links = serde_json::from_value(json!([
        { "type": "part-of", "to": "ep-001-aa" },
        { "type": "relates-to", "to": "ts-0001-aa" }
    ]))
    .unwrap();
    let tasks = [task("ts-0001-aa", "active", &[]), blocked];
    assert_eq!(ready_ids(&tasks), ["ts-0002-bb"]);
}

#[test]
fn a_blocked_by_link_marked_not_blocking_does_not_block() {
    let mut task_b = task("ts-0002-bb", "ready", &[]);
    task_b.links = serde_json::from_value(json!([
        { "type": "blocked-by", "to": "ts-0001-aa", "blocking": false }
    ]))
    .unwrap();
    let tasks = [task("ts-0001-aa", "active", &[]), task_b];
    assert_eq!(ready_ids(&tasks), ["ts-0002-bb"]);
}

#[test]
fn a_two_item_cycle_is_reported_and_neither_item_is_ready() {
    let tasks = [
        task("ts-0001-aa", "ready", &["ts-0002-bb"]),
        task("ts-0002-bb", "ready", &["ts-0001-aa"]),
        task("ts-0003-cc", "ready", &[]),
    ];
    let result = readiness(&tasks);
    assert_eq!(result.ready, ["ts-0003-cc"]);
    assert_eq!(result.cycles, [["ts-0001-aa", "ts-0002-bb"]]);
}

#[test]
fn a_cycle_member_that_is_done_still_leaves_the_cycle_unready() {
    let tasks = [
        task("ts-0001-aa", "ready", &["ts-0002-bb"]),
        task("ts-0002-bb", "done", &["ts-0001-aa"]),
    ];
    let result = readiness(&tasks);
    assert!(result.ready.is_empty());
    assert_eq!(result.cycles.len(), 1);
}

#[test]
fn a_longer_cycle_and_a_self_link_are_each_reported_once() {
    let tasks = [
        task("ts-0001-aa", "ready", &["ts-0002-bb"]),
        task("ts-0002-bb", "ready", &["ts-0003-cc"]),
        task("ts-0003-cc", "ready", &["ts-0001-aa"]),
        task("ts-0004-dd", "ready", &["ts-0004-dd"]),
    ];
    let result = readiness(&tasks);
    assert!(result.ready.is_empty());
    assert_eq!(
        result.cycles,
        [
            vec!["ts-0001-aa", "ts-0002-bb", "ts-0003-cc"],
            vec!["ts-0004-dd"]
        ]
    );
}

#[test]
fn a_task_blocked_by_a_cycle_member_is_blocked_and_is_not_in_the_cycle() {
    let tasks = [
        task("ts-0001-aa", "ready", &["ts-0002-bb"]),
        task("ts-0002-bb", "ready", &["ts-0001-aa"]),
        task("ts-0003-cc", "ready", &["ts-0001-aa"]),
    ];
    let result = readiness(&tasks);
    assert!(result.ready.is_empty());
    assert_eq!(result.cycles, [["ts-0001-aa", "ts-0002-bb"]]);
}

#[test]
fn a_chain_that_is_not_a_cycle_is_not_reported() {
    let tasks = [
        task("ts-0001-aa", "done", &[]),
        task("ts-0002-bb", "done", &["ts-0001-aa"]),
        task("ts-0003-cc", "ready", &["ts-0002-bb"]),
    ];
    let result = readiness(&tasks);
    assert_eq!(result.ready, ["ts-0003-cc"]);
    assert!(result.cycles.is_empty());
}

#[test]
fn a_blocker_that_is_no_item_blocks_and_is_reported() {
    let tasks = [
        task("ts-0001-aa", "ready", &["ts-9999-zz"]),
        task("ts-0002-bb", "ready", &[]),
    ];
    let result = readiness(&tasks);
    assert_eq!(result.ready, ["ts-0002-bb"]);
    assert_eq!(
        result.unknown_targets,
        [UnknownTarget {
            from: "ts-0001-aa".into(),
            to: "ts-9999-zz".into()
        }]
    );
}

#[test]
fn an_unknown_blocker_is_reported_even_when_the_item_is_not_ready_anyway() {
    let tasks = [task("ts-0001-aa", "active", &["ts-9999-zz"])];
    assert_eq!(readiness(&tasks).unknown_targets.len(), 1);
}
