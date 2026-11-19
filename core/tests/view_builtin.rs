//! Tests for the built-in `ready` view (ts-0135-cx; 0.10, Views): its members are the tasks the ready rule lists, with
//! a `blocked-by` cycle and an unknown blocker in its report. Pure: the corpus is fed fixture data.

use collide_core::corpus::Corpus;
use collide_core::item::{Category, StatusDecl};
use collide_core::owner::task::ViewReport;
use collide_core::provider::{Collection, ListResult, Scope, Task, TaskInvalid};
use collide_core::ready::{UnknownTarget, readiness};
use collide_core::testing::{default_initialize, fake_task};
use collide_core::view_builtin::{READY, ready_view};
use collide_core::view_eval::{CompiledView, ViewResult, evaluate_view, view_report};
use collide_core::view_expr::Budgets;
use serde_json::json;

const STATUSES: [(&str, Category); 5] = [
    ("backlog", Category::Backlog),
    ("ready", Category::Ready),
    ("active", Category::Active),
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

/// A task of `source` with `status`, blocked by each of `blocked_by` (names in its own source).
fn task(source: &str, id: &str, status: &str, blocked_by: &[&str]) -> Task {
    let mut task = fake_task(id, id, status, category_of(status));
    task.key = format!("{source}:{id}");
    task.links = blocked_by
        .iter()
        .map(|to| serde_json::from_value(json!({ "type": "blocked-by", "to": to })).unwrap())
        .collect();
    task
}

fn corpus(sources: &[(&str, Vec<Task>)], invalid: usize) -> Corpus {
    let mut corpus = Corpus::default();
    for (source, tasks) in sources {
        let mut init = default_initialize();
        init.scope = Scope::Global;
        init.statuses = STATUSES
            .iter()
            .map(|(name, category)| StatusDecl {
                name: (*name).into(),
                category: *category,
            })
            .collect();
        corpus.declare_source(source, &init);
        let bad = (0..invalid)
            .map(|n| TaskInvalid {
                path: format!("{source}/bad-{n}.md"),
                checkout: None,
                reason: "broken".into(),
            })
            .collect();
        corpus
            .load(
                source,
                None,
                ListResult {
                    tasks: tasks.clone(),
                    collections: Vec::<Collection>::new(),
                    invalid: bad,
                    next_cursor: None,
                },
            )
            .unwrap();
    }
    corpus
}

fn ready(corpus: &Corpus) -> ViewResult {
    evaluate_view(&CompiledView::ready(), corpus, &Budgets::default()).expect("ready evaluates")
}

fn keys(source: &str, ids: &[&str]) -> Vec<String> {
    ids.iter().map(|id| format!("{source}:{id}")).collect()
}

#[test]
fn the_view_is_called_ready_and_holds_no_part() {
    let view = ready_view();
    assert_eq!(view.name, READY);
    assert_eq!(READY, "ready");
    assert_eq!(view.where_, None);
    assert!(view.include.is_empty() && view.exclude.is_empty());
    assert!(view.closure.is_none() && !view.queue);
    assert!(CompiledView::ready().is_builtin());
}

#[test]
fn a_task_with_no_blocker_is_a_member_and_one_that_is_blocked_is_not() {
    let board = corpus(
        &[(
            "plan",
            vec![
                task("plan", "a-free", "ready", &[]),
                task("plan", "b-blocker", "active", &[]),
                task("plan", "c-blocked", "ready", &["b-blocker"]),
                task("plan", "d-not-ready", "backlog", &[]),
            ],
        )],
        0,
    );
    assert_eq!(ready(&board).members, keys("plan", &["a-free"]));
}

#[test]
fn a_done_blocker_and_a_dropped_blocker_both_unblock() {
    let board = corpus(
        &[(
            "plan",
            vec![
                task("plan", "a-done", "done", &[]),
                task("plan", "b-dropped", "dropped", &[]),
                task("plan", "c-after-done", "ready", &["a-done"]),
                task("plan", "d-after-dropped", "ready", &["b-dropped"]),
                task("plan", "e-after-both", "ready", &["a-done", "b-dropped"]),
            ],
        )],
        0,
    );
    assert_eq!(
        ready(&board).members,
        keys("plan", &["c-after-done", "d-after-dropped", "e-after-both"])
    );
}

#[test]
fn a_blocked_by_cycle_has_no_member_and_is_in_the_report() {
    let board = corpus(
        &[(
            "plan",
            vec![
                task("plan", "a-one", "ready", &["b-two"]),
                task("plan", "b-two", "ready", &["a-one"]),
                task("plan", "c-self", "ready", &["c-self"]),
                task("plan", "d-free", "ready", &[]),
                task("plan", "e-behind", "ready", &["a-one"]),
            ],
        )],
        0,
    );
    let result = ready(&board);
    assert_eq!(result.members, keys("plan", &["d-free"]));
    assert_eq!(result.cycle, keys("plan", &["a-one", "b-two", "c-self"]));
    assert_eq!(view_report("ready", &result).cycle, result.cycle);
}

#[test]
fn an_unknown_blocker_keeps_its_task_out_and_is_in_the_report() {
    let board = corpus(
        &[(
            "plan",
            vec![
                task("plan", "a-typo", "ready", &["nowhere"]),
                task("plan", "b-free", "ready", &[]),
            ],
        )],
        0,
    );
    let result = ready(&board);
    assert_eq!(result.members, keys("plan", &["b-free"]));
    let unknown = UnknownTarget {
        from: "plan:a-typo".into(),
        to: "nowhere".into(),
    };
    assert_eq!(result.unknown_blockers, std::slice::from_ref(&unknown));
    assert_eq!(view_report("ready", &result).unknown_blockers, [unknown]);
}

#[test]
fn the_members_are_what_the_rule_lists_over_one_source() {
    let tasks = vec![
        task("plan", "a", "done", &[]),
        task("plan", "b", "ready", &["a"]),
        task("plan", "c", "ready", &["b"]),
        task("plan", "d", "ready", &["e"]),
        task("plan", "e", "ready", &["d"]),
        task("plan", "f", "ready", &["gone"]),
        task("plan", "g", "dropped", &[]),
        task("plan", "h", "ready", &["g", "a"]),
    ];
    let expected = readiness(&tasks).ready;
    let board = corpus(&[("plan", tasks)], 0);
    let found: Vec<String> = ready(&board)
        .members
        .iter()
        .map(|key| key.strip_prefix("plan:").unwrap().to_string())
        .collect();
    assert_eq!(found, expected);
    assert_eq!(found, ["b", "h"]);
}

#[test]
fn a_blocker_is_a_name_in_its_own_source() {
    let board = corpus(
        &[
            (
                "github",
                vec![
                    task("github", "one", "active", &[]),
                    task("github", "two", "ready", &["one"]),
                ],
            ),
            (
                "plan",
                vec![
                    task("plan", "one", "done", &[]),
                    task("plan", "two", "ready", &["one"]),
                ],
            ),
        ],
        0,
    );
    assert_eq!(ready(&board).members, keys("plan", &["two"]));
}

#[test]
fn the_view_is_empty_over_an_empty_corpus_and_counts_the_invalid_items() {
    let result = ready(&corpus(&[("plan", Vec::new())], 2));
    assert!(result.members.is_empty());
    assert_eq!(result.counts.invalid_items, 2);
}

#[test]
fn a_clean_report_leaves_the_new_field_out_of_the_wire() {
    let board = corpus(&[("plan", vec![task("plan", "a", "ready", &[])])], 0);
    let report = view_report("ready", &ready(&board));
    assert_eq!(report.members, 1);
    let written = serde_json::to_value(&report).unwrap();
    assert!(written.get("unknown_blockers").is_none(), "{written}");
    let back: ViewReport = serde_json::from_value(written).unwrap();
    assert_eq!(back, report);
}

#[test]
fn the_unknown_blockers_of_a_report_read_back() {
    let report: ViewReport = serde_json::from_value(json!({
        "name": "ready", "members": 0,
        "unknown_blockers": [{ "from": "plan:a", "to": "gone" }],
    }))
    .unwrap();
    assert_eq!(report.unknown_blockers.len(), 1);
    assert_eq!(report.unknown_blockers[0].to, "gone");
}
