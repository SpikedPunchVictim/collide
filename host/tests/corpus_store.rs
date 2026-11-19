//! The corpus cache in the store (ts-0116-wh): what a source listed or was told, kept by board, source and checkout,
//! so the next start can compare a fresh `list` with it. Each test has a store of its own under the test's temp root.

mod common;

use collide_core::provider::{InitializeResult, Scope, Task, TaskInvalid};
use collide_core::testing::provider::default_initialize;
use collide_host::store::Store;
use common::TempRoot;
use rusqlite::Connection;
use serde_json::json;

fn task(key: &str, status: &str, checkout: Option<&str>, path: Option<&str>) -> Task {
    serde_json::from_value(json!({
        "key": key,
        "id": key.rsplit(':').next().unwrap(),
        "title": format!("title of {key}"),
        "state": { "status": status, "category": "ready" },
        "version": "v1",
        "checkout": checkout,
        "path": path,
    }))
    .expect("a task reads")
}

fn invalid(path: &str, checkout: Option<&str>, reason: &str) -> TaskInvalid {
    TaskInvalid {
        path: path.to_string(),
        checkout: checkout.map(str::to_string),
        reason: reason.to_string(),
    }
}

fn store(root: &TempRoot) -> Store {
    Store::open(&root.join("store")).expect("the store opens")
}

fn keys(store: &Store, board: &str) -> Vec<String> {
    let mut keys: Vec<String> = store
        .corpus_cache(board)
        .unwrap()
        .tasks
        .iter()
        .map(|cached| {
            format!(
                "{}@{}",
                cached.task.key,
                cached.checkout.as_deref().unwrap_or("-")
            )
        })
        .collect();
    keys.sort();
    keys
}

#[test]
fn a_replaced_checkout_reads_back_with_its_tasks_and_invalid_items() {
    let root = TempRoot::new();
    let store = store(&root);
    let one = task("plan:1", "todo", Some("/a"), Some("arch/1.md"));
    let bad = invalid("arch/bad.md", Some("/a"), "no frontmatter");
    store
        .corpus_replace(
            "board-1",
            "plan",
            Some("/a"),
            std::slice::from_ref(&one),
            std::slice::from_ref(&bad),
        )
        .unwrap();

    let cache = store.corpus_cache("board-1").unwrap();
    assert_eq!(cache.tasks.len(), 1);
    assert_eq!(cache.tasks[0].source, "plan");
    assert_eq!(cache.tasks[0].checkout.as_deref(), Some("/a"));
    assert_eq!(cache.tasks[0].task, one);
    assert_eq!(cache.invalid.len(), 1);
    assert_eq!(cache.invalid[0].item, bad);
    assert!(cache.dropped.is_empty());
}

#[test]
fn replacing_one_checkout_drops_what_it_no_longer_lists_and_leaves_the_others() {
    let root = TempRoot::new();
    let store = store(&root);
    let at = |key: &str, checkout: &str| task(key, "todo", Some(checkout), Some("x.md"));
    store
        .corpus_replace(
            "board-1",
            "plan",
            Some("/a"),
            &[at("plan:1", "/a"), at("plan:2", "/a")],
            &[],
        )
        .unwrap();
    store
        .corpus_replace("board-1", "plan", Some("/b"), &[at("plan:1", "/b")], &[])
        .unwrap();
    store
        .corpus_replace("board-1", "plan", Some("/a"), &[at("plan:2", "/a")], &[])
        .unwrap();
    assert_eq!(keys(&store, "board-1"), ["plan:1@/b", "plan:2@/a"]);
}

#[test]
fn a_source_with_no_checkout_is_kept_apart_from_the_same_key_in_a_checkout() {
    let root = TempRoot::new();
    let store = store(&root);
    store
        .corpus_replace(
            "board-1",
            "gh",
            None,
            &[task("gh:1", "todo", None, None)],
            &[],
        )
        .unwrap();
    let cache = store.corpus_cache("board-1").unwrap();
    assert_eq!(cache.tasks[0].checkout, None);
    assert_eq!(keys(&store, "board-1"), ["gh:1@-"]);
}

#[test]
fn boards_do_not_see_each_others_rows() {
    let root = TempRoot::new();
    let store = store(&root);
    store
        .corpus_put_task(
            "board-1",
            "plan",
            Some("/a"),
            &task("plan:1", "todo", Some("/a"), None),
        )
        .unwrap();
    store
        .corpus_put_task(
            "board-2",
            "plan",
            Some("/b"),
            &task("plan:2", "todo", Some("/b"), None),
        )
        .unwrap();
    assert_eq!(keys(&store, "board-1"), ["plan:1@/a"]);
    assert_eq!(keys(&store, "board-2"), ["plan:2@/b"]);
}

#[test]
fn a_put_replaces_the_copy_of_that_key_and_a_drop_removes_it() {
    let root = TempRoot::new();
    let store = store(&root);
    let first = task("plan:1", "todo", Some("/a"), Some("arch/1.md"));
    store
        .corpus_put_task("board-1", "plan", Some("/a"), &first)
        .unwrap();
    let moved = task("plan:1", "doing", Some("/a"), Some("arch/1.md"));
    store
        .corpus_put_task("board-1", "plan", Some("/a"), &moved)
        .unwrap();
    let cache = store.corpus_cache("board-1").unwrap();
    assert_eq!(cache.tasks.len(), 1);
    assert_eq!(cache.tasks[0].task.state.status, "doing");

    store
        .corpus_drop_task("board-1", "plan", Some("/a"), "plan:1")
        .unwrap();
    assert!(store.corpus_cache("board-1").unwrap().tasks.is_empty());
    store
        .corpus_drop_task("board-1", "plan", Some("/a"), "plan:1")
        .unwrap();
}

#[test]
fn an_invalid_item_replaces_the_task_at_its_path_and_a_task_at_the_path_clears_the_item() {
    let root = TempRoot::new();
    let store = store(&root);
    store
        .corpus_put_task(
            "board-1",
            "plan",
            Some("/a"),
            &task("plan:1", "todo", Some("/a"), Some("arch/1.md")),
        )
        .unwrap();
    store
        .corpus_put_invalid(
            "board-1",
            "plan",
            &invalid("arch/1.md", Some("/a"), "broken"),
        )
        .unwrap();
    let cache = store.corpus_cache("board-1").unwrap();
    assert!(cache.tasks.is_empty(), "the task at the path left");
    assert_eq!(cache.invalid.len(), 1);

    store
        .corpus_put_task(
            "board-1",
            "plan",
            Some("/a"),
            &task("plan:1", "todo", Some("/a"), Some("arch/1.md")),
        )
        .unwrap();
    let cache = store.corpus_cache("board-1").unwrap();
    assert_eq!(cache.tasks.len(), 1);
    assert!(cache.invalid.is_empty(), "the repaired path is clear");
}

#[test]
fn the_body_of_a_task_is_not_kept() {
    let root = TempRoot::new();
    let store = store(&root);
    let mut long = task("plan:1", "todo", Some("/a"), None);
    long.body = Some("a long prompt".to_string());
    store
        .corpus_put_task("board-1", "plan", Some("/a"), &long)
        .unwrap();
    assert_eq!(
        store.corpus_cache("board-1").unwrap().tasks[0].task.body,
        None
    );
}

#[test]
fn a_row_that_does_not_read_is_dropped_and_named_and_the_others_are_kept() {
    let root = TempRoot::new();
    let store = store(&root);
    store
        .corpus_put_task(
            "board-1",
            "plan",
            Some("/a"),
            &task("plan:1", "todo", Some("/a"), None),
        )
        .unwrap();
    store
        .corpus_put_task(
            "board-1",
            "plan",
            Some("/a"),
            &task("plan:2", "todo", Some("/a"), None),
        )
        .unwrap();
    let conn = Connection::open(store.db_path()).unwrap();
    conn.execute(
        "UPDATE corpus_tasks SET json = '{not json' WHERE key = 'plan:2'",
        [],
    )
    .unwrap();
    drop(conn);

    let cache = store.corpus_cache("board-1").unwrap();
    assert_eq!(cache.tasks.len(), 1);
    assert_eq!(cache.tasks[0].task.key, "plan:1");
    assert_eq!(cache.dropped.len(), 1);
    assert!(cache.dropped[0].contains("plan:2"), "{:?}", cache.dropped);
    assert!(
        store.corpus_cache("board-1").unwrap().dropped.is_empty(),
        "the row is gone"
    );
}

#[test]
fn a_task_cached_before_the_quality_parts_reads_back_with_them_empty_and_is_not_dropped() {
    let root = TempRoot::new();
    let store = store(&root);
    store
        .corpus_put_task(
            "board-1",
            "plan",
            Some("/a"),
            &task("plan:1", "draft", Some("/a"), None),
        )
        .unwrap();
    // The row as migration 10 first wrote it: features and their problems, none of the keys of revision 18.
    let older = json!({
        "key": "plan:1", "id": "1", "title": "title of plan:1", "kind": "proposal",
        "state": { "status": "draft", "category": "active" }, "version": "v1", "checkout": "/a",
        "features": [{ "id": "F1", "title": "A", "section": "", "summary": "b" }],
        "featureProblems": [{ "line": 4, "reason": "stray" }]
    });
    let conn = Connection::open(store.db_path()).unwrap();
    conn.execute(
        "UPDATE corpus_tasks SET json = ?1 WHERE key = 'plan:1'",
        [older.to_string()],
    )
    .unwrap();
    drop(conn);

    let cache = store.corpus_cache("board-1").unwrap();
    assert!(cache.dropped.is_empty(), "{:?}", cache.dropped);
    let read = &cache.tasks[0].task;
    assert_eq!(read.features.len(), 1);
    assert_eq!(read.feature_problems.len(), 1);
    assert!(read.decisions.is_empty() && read.assumptions.is_empty());
    assert!(read.findings.is_empty() && read.part_problems.is_empty());
}

#[test]
fn a_task_with_decisions_and_part_problems_reads_back_as_it_was_put() {
    let root = TempRoot::new();
    let store = store(&root);
    let mut proposal = task("plan:1", "draft", Some("/a"), None);
    let parts = json!({
        "decisions": [{
            "id": "D1", "title": "T", "text": "x", "passedOn": ["y"], "line": 3,
            "evidence": [{ "target": "sp-0001-aa", "kind": "item", "status": "concluded" }],
            "confidence": "low"
        }],
        "partProblems": [{ "part": "Decision", "line": 3, "reason": "D1 is odd" }]
    });
    let mut doc = serde_json::to_value(&proposal).unwrap();
    doc.as_object_mut()
        .unwrap()
        .extend(parts.as_object().unwrap().clone());
    proposal = serde_json::from_value(doc).unwrap();
    store
        .corpus_put_task("board-1", "plan", Some("/a"), &proposal)
        .unwrap();

    let cache = store.corpus_cache("board-1").unwrap();
    assert_eq!(cache.tasks[0].task, proposal);
    assert_eq!(cache.tasks[0].task.decisions[0].passed_on, ["y"]);
}

#[test]
fn a_task_cached_before_coverage_reads_back_with_none_and_is_not_dropped() {
    let root = TempRoot::new();
    let store = store(&root);
    store
        .corpus_put_task(
            "board-1",
            "plan",
            Some("/a"),
            &task("plan:1", "draft", Some("/a"), None),
        )
        .unwrap();
    // What the last release wrote: a proposal with features and quality parts, and no `coverage`.
    let older = json!({
        "key": "plan:1", "id": "1", "title": "title of plan:1", "kind": "proposal",
        "state": { "status": "draft", "category": "active" }, "version": "v1", "checkout": "/a",
        "features": [{ "id": "F1", "title": "A", "section": "", "summary": "b" }],
        "decisions": [], "partProblems": [{ "part": "Decision", "line": 4, "reason": "stray" }]
    });
    let conn = Connection::open(store.db_path()).unwrap();
    conn.execute(
        "UPDATE corpus_tasks SET json = ?1 WHERE key = 'plan:1'",
        [older.to_string()],
    )
    .unwrap();
    drop(conn);

    let cache = store.corpus_cache("board-1").unwrap();
    assert!(cache.dropped.is_empty(), "{:?}", cache.dropped);
    let read = &cache.tasks[0].task;
    assert_eq!(read.features.len(), 1);
    assert!(read.coverage.is_none());
}

#[test]
fn a_task_with_coverage_reads_back_as_it_was_put() {
    let root = TempRoot::new();
    let store = store(&root);
    let mut doc = serde_json::to_value(task("plan:1", "draft", Some("/a"), None)).unwrap();
    doc["coverage"] =
        json!({ "featuresWithoutTask": ["F2"], "tasksWithoutFeature": ["ts-0002-bb"] });
    let proposal: Task = serde_json::from_value(doc).unwrap();
    store
        .corpus_put_task("board-1", "plan", Some("/a"), &proposal)
        .unwrap();

    let cache = store.corpus_cache("board-1").unwrap();
    assert_eq!(cache.tasks[0].task, proposal);
    let coverage = cache.tasks[0].task.coverage.as_ref().unwrap();
    assert_eq!(coverage.features_without_task, ["F2"]);
}

#[test]
fn a_second_handle_on_the_same_folder_reads_what_the_first_wrote_before_it_closed() {
    let root = TempRoot::new();
    let first = store(&root);
    first
        .corpus_put_task(
            "board-1",
            "plan",
            Some("/a"),
            &task("plan:1", "todo", Some("/a"), None),
        )
        .unwrap();
    let second = store(&root);
    assert_eq!(keys(&second, "board-1"), ["plan:1@/a"]);
}

fn declared(scope: Scope) -> InitializeResult {
    InitializeResult {
        scope,
        ..default_initialize()
    }
}

#[test]
fn a_sources_declaration_reads_back_and_a_newer_one_replaces_it() {
    let root = TempRoot::new();
    let store = store(&root);
    store
        .corpus_put_source("board-1", "plan", &declared(Scope::Global))
        .unwrap();
    store
        .corpus_put_source("board-1", "plan", &declared(Scope::Ref))
        .unwrap();
    store
        .corpus_put_source("board-2", "plan", &declared(Scope::Global))
        .unwrap();

    let cache = store.corpus_cache("board-1").unwrap();

    assert_eq!(cache.sources, [("plan".to_string(), declared(Scope::Ref))]);
    assert!(cache.dropped.is_empty());
}

#[test]
fn a_declaration_cached_before_the_inverse_reads_back_with_none_and_is_not_dropped() {
    let root = TempRoot::new();
    let store = store(&root);
    store
        .corpus_put_source("board-1", "plan", &declared(Scope::Ref))
        .unwrap();
    // What the last release wrote: link types with a direction, `blocking` and a target, and no `inverse`.
    let mut older = serde_json::to_value(declared(Scope::Ref)).unwrap();
    older["links"] = json!([
        { "name": "blocked-by", "dir": "out", "blocking": true },
        { "name": "implements", "dir": "out", "blocking": false, "target": "feature" }
    ]);
    let conn = Connection::open(store.db_path()).unwrap();
    conn.execute(
        "UPDATE corpus_sources SET json = ?1 WHERE source = 'plan'",
        [older.to_string()],
    )
    .unwrap();
    drop(conn);

    let cache = store.corpus_cache("board-1").unwrap();
    assert!(cache.dropped.is_empty(), "{:?}", cache.dropped);
    let links = &cache.sources[0].1.links;
    assert_eq!(links.len(), 2);
    assert!(links.iter().all(|decl| decl.inverse.is_none()));
}

#[test]
fn a_declaration_with_an_inverse_reads_back_as_it_was_put() {
    let root = TempRoot::new();
    let store = store(&root);
    let mut init = declared(Scope::Ref);
    init.links = serde_json::from_value(json!([
        { "name": "blocked-by", "dir": "out", "blocking": true, "inverse": "blocks" }
    ]))
    .unwrap();
    store.corpus_put_source("board-1", "plan", &init).unwrap();

    let cache = store.corpus_cache("board-1").unwrap();
    assert_eq!(cache.sources, [("plan".to_string(), init)]);
    assert_eq!(
        cache.sources[0].1.links[0].inverse.as_deref(),
        Some("blocks")
    );
}

#[test]
fn a_declaration_that_does_not_read_is_dropped_and_named_and_the_others_are_kept() {
    let root = TempRoot::new();
    let store = store(&root);
    store
        .corpus_put_source("board-1", "plan", &declared(Scope::Ref))
        .unwrap();
    store
        .corpus_put_source("board-1", "gh", &declared(Scope::Global))
        .unwrap();
    let conn = Connection::open(store.db_path()).unwrap();
    conn.execute(
        "UPDATE corpus_sources SET json = '{not json' WHERE source = 'gh'",
        [],
    )
    .unwrap();
    drop(conn);

    let cache = store.corpus_cache("board-1").unwrap();

    assert_eq!(cache.sources, [("plan".to_string(), declared(Scope::Ref))]);
    assert_eq!(cache.dropped.len(), 1);
    assert!(cache.dropped[0].contains("gh"), "{:?}", cache.dropped);
    assert!(store.corpus_cache("board-1").unwrap().dropped.is_empty());
}
