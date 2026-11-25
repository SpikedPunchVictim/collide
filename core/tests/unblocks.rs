//! Tests for `unblocks` and the built-in `bottlenecks` view (ts-0186-ch; 0.10, Views): how many open tasks wait on a
//! task, directly or through other open tasks, each counted once; a `where` that tests it; the view that lists the
//! tasks with 3 or more, the most first; and the number on each task of `task.list`. Hand counts over fixture data.
//! Pure: no file is read and no process starts.

use collide_core::corpus::{Corpus, ListFilter, Page};
use collide_core::item::{Category, StatusDecl};
use collide_core::provider::{Collection, ListResult, Scope, Task};
use collide_core::testing::{default_initialize, fake_task};
use collide_core::view::View;
use collide_core::view_builtin::{BOTTLENECKS, bottlenecks_view};
use collide_core::view_eval::{CompiledView, ViewResult, compile_view, evaluate_view, view_report};
use collide_core::view_expr::Budgets;
use serde_json::json;
use std::sync::Arc;

const STATUSES: [(&str, Category); 6] = [
    ("backlog", Category::Backlog),
    ("ready", Category::Ready),
    ("active", Category::Active),
    ("review", Category::Review),
    ("done", Category::Done),
    ("dropped", Category::Dropped),
];

fn category_of(status: &str) -> Category {
    STATUSES
        .iter()
        .find(|(name, _)| *name == status)
        .map(|(_, category)| *category)
        .unwrap_or_else(|| panic!("`{status}` is not a fixture status"))
}

/// A `plan` task with `status`, which waits on each of `waits_on` (ids in `plan`).
fn task(id: &str, status: &str, waits_on: &[&str]) -> Task {
    let mut task = fake_task(id, id, status, category_of(status));
    task.key = format!("plan:{id}");
    task.links = waits_on
        .iter()
        .map(|to| serde_json::from_value(json!({ "type": "blocked-by", "to": to })).unwrap())
        .collect();
    task
}

fn init() -> collide_core::provider::InitializeResult {
    let mut init = default_initialize();
    init.scope = Scope::Global;
    init.statuses = STATUSES
        .iter()
        .map(|(name, category)| StatusDecl {
            name: (*name).into(),
            category: *category,
        })
        .collect();
    init
}

fn corpus(tasks: Vec<Task>) -> Corpus {
    let mut corpus = Corpus::default();
    corpus.declare_source("plan", &init());
    corpus
        .load(
            "plan",
            None,
            ListResult {
                tasks,
                collections: Vec::<Collection>::new(),
                invalid: Vec::new(),
                next_cursor: None,
            },
        )
        .unwrap();
    corpus
}

fn count(corpus: &Corpus, id: &str) -> u32 {
    let key = format!("plan:{id}");
    *corpus
        .unblocks()
        .get(&key)
        .unwrap_or_else(|| panic!("{key} has no count"))
}

/// Three open tasks wait on `t-target`, and two more wait on one of those three (`d-four` on `a-one` only, `e-five`
/// on `a-one` and on `b-two`: two paths to `t-target`).
fn hub_board() -> Corpus {
    corpus(vec![
        task("t-target", "ready", &[]),
        task("a-one", "ready", &["t-target"]),
        task("b-two", "active", &["t-target"]),
        task("c-three", "review", &["t-target"]),
        task("d-four", "backlog", &["a-one"]),
        task("e-five", "ready", &["a-one", "b-two"]),
    ])
}

fn members(view: &View, corpus: &Corpus) -> Vec<String> {
    let compiled = compile_view(view, corpus.rows().len(), &Budgets::default()).expect("compiles");
    evaluate_view(&compiled, corpus, &Budgets::default())
        .expect("evaluates")
        .members
}

fn bottlenecks(corpus: &Corpus) -> ViewResult {
    evaluate_view(&CompiledView::bottlenecks(), corpus, &Budgets::default())
        .expect("bottlenecks evaluates")
}

fn keys(ids: &[&str]) -> Vec<String> {
    ids.iter().map(|id| format!("plan:{id}")).collect()
}

// --- the count ---

#[test]
fn three_wait_on_one_and_two_more_wait_on_one_of_them_one_by_two_paths_gives_five() {
    let board = hub_board();
    assert_eq!(count(&board, "t-target"), 5);
    assert_eq!(count(&board, "a-one"), 2);
    assert_eq!(count(&board, "b-two"), 1);
    assert_eq!(count(&board, "c-three"), 0);
    assert_eq!(count(&board, "d-four"), 0);
}

#[test]
fn a_done_task_in_a_chain_stops_the_walk() {
    let board = corpus(vec![
        task("t-target", "ready", &[]),
        task("x-done", "done", &["t-target"]),
        task("y-behind-done", "ready", &["x-done"]),
        task("z-direct", "ready", &["t-target"]),
        task("w-dropped", "dropped", &["t-target"]),
        task("v-behind-dropped", "ready", &["w-dropped"]),
    ]);
    assert_eq!(count(&board, "t-target"), 1, "only z-direct is counted");
}

#[test]
fn a_task_waiting_on_two_tasks_that_wait_on_one_is_counted_once() {
    let board = corpus(vec![
        task("t-target", "ready", &[]),
        task("a-one", "ready", &["t-target"]),
        task("b-two", "ready", &["t-target"]),
        task("c-both", "ready", &["a-one", "b-two", "t-target"]),
    ]);
    assert_eq!(count(&board, "t-target"), 3);
}

#[test]
fn a_cycle_of_three_gives_each_a_finite_count_that_leaves_out_itself() {
    let board = corpus(vec![
        task("a-one", "ready", &["b-two"]),
        task("b-two", "ready", &["c-three"]),
        task("c-three", "ready", &["a-one"]),
        task("d-self", "ready", &["d-self"]),
    ]);
    for id in ["a-one", "b-two", "c-three"] {
        assert_eq!(count(&board, id), 2, "{id}");
    }
    assert_eq!(count(&board, "d-self"), 0);
}

#[test]
fn a_task_that_waits_on_a_cycle_is_counted_by_each_task_on_it() {
    let board = corpus(vec![
        task("a-one", "ready", &["b-two"]),
        task("b-two", "ready", &["a-one"]),
        task("e-behind", "ready", &["a-one"]),
    ]);
    assert_eq!(count(&board, "a-one"), 2, "b-two and e-behind");
    assert_eq!(count(&board, "b-two"), 2, "a-one and e-behind");
    assert_eq!(count(&board, "e-behind"), 0);
}

#[test]
fn a_link_that_does_not_block_and_a_target_that_is_no_task_add_nothing() {
    let mut loose = task("a-loose", "ready", &[]);
    loose.links = vec![
        serde_json::from_value(json!({ "type": "informs", "to": "t-target" })).unwrap(),
        serde_json::from_value(
            json!({ "type": "blocked-by", "to": "t-target", "blocking": false }),
        )
        .unwrap(),
        serde_json::from_value(json!({ "type": "blocked-by", "to": "no-such-task" })).unwrap(),
    ];
    let board = corpus(vec![task("t-target", "ready", &[]), loose]);
    assert_eq!(count(&board, "t-target"), 0);
}

#[test]
fn a_depends_on_link_counts_as_a_blocked_by_does_for_deps() {
    let mut waiter = task("a-waiter", "ready", &[]);
    waiter.links =
        vec![serde_json::from_value(json!({ "type": "depends_on", "to": "t-target" })).unwrap()];
    let board = corpus(vec![task("t-target", "ready", &[]), waiter]);
    assert_eq!(count(&board, "t-target"), 1);
}

#[test]
fn the_counts_are_worked_out_once_for_a_version_and_again_when_the_corpus_changes() {
    let mut board = hub_board();
    let first = board.unblocks();
    assert!(
        Arc::ptr_eq(&first, &board.unblocks()),
        "one pass for a version"
    );
    assert!(
        Arc::ptr_eq(&first, &board.clone().unblocks()),
        "a copy of that version has it too"
    );
    board
        .load(
            "plan",
            None,
            ListResult {
                tasks: vec![
                    task("t-target", "ready", &[]),
                    task("a-one", "ready", &["t-target"]),
                ],
                collections: Vec::new(),
                invalid: Vec::new(),
                next_cursor: None,
            },
        )
        .unwrap();
    assert!(!Arc::ptr_eq(&first, &board.unblocks()));
    assert_eq!(count(&board, "t-target"), 1);
}

// --- where: unblocks ---

fn file_view(source: &str) -> View {
    serde_json::from_value(json!({ "name": "hubs", "where": source })).expect("a view reads")
}

#[test]
fn a_file_view_tests_unblocks_as_a_number() {
    let board = hub_board();
    assert_eq!(
        members(&file_view("unblocks >= 3"), &board),
        keys(&["t-target"])
    );
    assert_eq!(
        members(&file_view("unblocks == 2"), &board),
        keys(&["a-one"])
    );
    assert_eq!(
        members(&file_view("unblocks > 0 && category == 'ready'"), &board),
        keys(&["a-one", "t-target"])
    );
}

// --- bottlenecks ---

/// `waiters` open tasks that each wait on `hub`, named after it.
fn waiters(hub: &str, how_many: usize) -> Vec<Task> {
    (0..how_many)
        .map(|n| task(&format!("w-{hub}-{n}"), "ready", &[hub]))
        .collect()
}

#[test]
fn bottlenecks_lists_the_open_tasks_with_three_or_more_the_most_first_and_ties_by_key() {
    let mut tasks = vec![
        task("a-small", "ready", &[]),
        task("b-small", "ready", &[]),
        task("c-two", "ready", &[]),
        task("d-done", "done", &[]),
        task("z-big", "active", &[]),
    ];
    tasks.extend(waiters("a-small", 3));
    tasks.extend(waiters("b-small", 3));
    tasks.extend(waiters("c-two", 2));
    tasks.extend(waiters("d-done", 3));
    tasks.extend(waiters("z-big", 4));
    let board = corpus(tasks);

    let result = bottlenecks(&board);

    assert_eq!(result.members, keys(&["z-big", "a-small", "b-small"]));
    assert!(result.steps.is_empty() && result.cycle.is_empty());
}

#[test]
fn bottlenecks_counts_a_task_through_the_tasks_that_wait_on_it() {
    assert_eq!(bottlenecks(&hub_board()).members, keys(&["t-target"]));
}

#[test]
fn bottlenecks_is_a_built_in_view_called_bottlenecks_with_no_part() {
    let view = bottlenecks_view();
    assert_eq!(view.name, BOTTLENECKS);
    assert_eq!(BOTTLENECKS, "bottlenecks");
    assert_eq!(view.where_, None);
    assert!(view.include.is_empty() && view.exclude.is_empty());
    assert!(view.closure.is_none() && !view.queue);
    assert!(CompiledView::bottlenecks().is_builtin());
    assert_eq!(CompiledView::bottlenecks().view(), &view);
}

#[test]
fn bottlenecks_reports_its_members_and_counts_the_invalid_items_like_ready() {
    let result = bottlenecks(&hub_board());
    let report = view_report(BOTTLENECKS, &result);
    assert_eq!(report.name, BOTTLENECKS);
    assert_eq!(report.members, 1);
    assert!(report.error.is_none() && report.cycle.is_empty());
}

// --- task.list ---

fn listed_counts(
    board: &Corpus,
    filter: &ListFilter<'_>,
    page: &Page<'_>,
) -> Vec<(String, Option<u32>)> {
    board
        .list(filter, page)
        .tasks
        .iter()
        .map(|task| (task.key.clone(), task.unblocks))
        .collect()
}

#[test]
fn each_task_of_task_list_carries_its_unblocks() {
    let counts = listed_counts(&hub_board(), &ListFilter::default(), &Page::default());
    let by_id = |id: &str| {
        counts
            .iter()
            .find(|(key, _)| key == &format!("plan:{id}"))
            .map(|(_, count)| *count)
    };
    assert_eq!(by_id("t-target"), Some(Some(5)));
    assert_eq!(by_id("a-one"), Some(Some(2)));
    assert_eq!(by_id("c-three"), Some(Some(0)));
    assert_eq!(counts.len(), 6);
}

#[test]
fn the_whole_corpus_listing_carries_it_too() {
    let listed = hub_board().list_result();
    let target = listed.tasks.iter().find(|task| task.key == "plan:t-target");
    assert_eq!(target.and_then(|task| task.unblocks), Some(5));
}

#[test]
fn a_list_given_an_order_lists_in_that_order_and_pages_along_it() {
    let board = hub_board();
    let order = keys(&["e-five", "c-three", "a-one", "b-two"]);
    let only = order.iter().cloned().collect();
    let filter = ListFilter {
        only: Some(&only),
        order: Some(&order),
        ..Default::default()
    };
    let all = board.list(&filter, &Page::default());
    let listed: Vec<&str> = all.tasks.iter().map(|task| task.key.as_str()).collect();
    assert_eq!(
        listed,
        ["plan:e-five", "plan:c-three", "plan:a-one", "plan:b-two"]
    );

    let first = board.list(
        &filter,
        &Page {
            limit: Some(3),
            cursor: None,
        },
    );
    let cursor = first.next_cursor.clone().expect("a page is left");
    let second = board.list(
        &filter,
        &Page {
            limit: Some(3),
            cursor: Some(&cursor),
        },
    );
    let paged: Vec<&str> = first
        .tasks
        .iter()
        .chain(&second.tasks)
        .map(|task| task.key.as_str())
        .collect();
    assert_eq!(paged, listed);
    assert!(second.next_cursor.is_none());
}
