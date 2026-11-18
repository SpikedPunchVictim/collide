//! Tests for evaluating a view (ts-0118): the set algebra, the closure and its steps, and the counts of what went
//! wrong on one view. Pure: the corpus is fed fixture data, and no test reads a file but the fixture view or starts
//! a process.

use collide_core::corpus::{CheckoutInfo, CheckoutKind, Corpus};
use collide_core::item::{Category, StatusDecl};
use collide_core::provider::{Collection, InitializeResult, ListResult, Scope, Task, TaskInvalid};
use collide_core::testing::{default_initialize, fake_task};
use collide_core::view::{MAX_LIST_ENTRIES, View, ViewError};
use collide_core::view_eval::{ViewFailure, ViewResult, compile_view, evaluate_view};
use collide_core::view_expr::{Budgets, DEPENDENCY_LINKS, ExprError};
use collide_core::yaml::{load_yaml, load_yaml_file};
use serde_json::json;
use std::path::Path;

// --- fixtures ---

const STATUSES: [(&str, Category); 6] = [
    ("todo", Category::Backlog),
    ("ready", Category::Ready),
    ("doing", Category::Active),
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

fn init(scope: Scope) -> InitializeResult {
    let mut init = default_initialize();
    init.scope = scope;
    init.statuses = STATUSES
        .iter()
        .map(|(name, category)| StatusDecl {
            name: (*name).into(),
            category: *category,
        })
        .collect();
    init
}

/// A `plan` task, as the source lists it. `short` is `T-01-042`, and the key is `plan:T-01-042`.
fn task(short: &str, status: &str) -> Task {
    let mut task = fake_task(
        short,
        &format!("title of {short}"),
        status,
        category_of(status),
    );
    task.key = format!("plan:{short}");
    task
}

/// A task of the source `github`.
fn issue(number: u32, status: &str) -> Task {
    let id = format!("acme/edge#{number}");
    let mut task = fake_task(&id, "an issue", status, category_of(status));
    task.key = format!("github:{id}");
    task
}

fn with_link(mut task: Task, link_type: &str, to: &str) -> Task {
    task.links.push(
        serde_json::from_value(json!({ "type": link_type, "to": to })).expect("a link reads"),
    );
    task
}

fn depends(task: Task, to: &str) -> Task {
    with_link(task, "depends_on", to)
}

fn blocked_by(task: Task, to: &str) -> Task {
    with_link(task, "blocked-by", to)
}

fn labelled(mut task: Task, labels: &[&str]) -> Task {
    task.labels = labels.iter().map(|label| (*label).to_string()).collect();
    task
}

fn collected(mut task: Task, collections: &[&str]) -> Task {
    task.collections = collections.iter().map(|id| (*id).to_string()).collect();
    task
}

fn with_field(mut task: Task, name: &str, value: serde_json::Value) -> Task {
    task.fields.insert(name.into(), value);
    task
}

fn listed(tasks: Vec<Task>, invalid: Vec<TaskInvalid>) -> ListResult {
    ListResult {
        tasks,
        collections: Vec::<Collection>::new(),
        invalid,
        next_cursor: None,
    }
}

/// A board of the sources `plan` and `github`, neither ref-scoped.
fn board(tasks: Vec<Task>) -> Corpus {
    board_with_invalid(tasks, Vec::new())
}

fn board_with_invalid(tasks: Vec<Task>, invalid: Vec<TaskInvalid>) -> Corpus {
    let (plan, github): (Vec<Task>, Vec<Task>) = tasks
        .into_iter()
        .partition(|task| task.key.starts_with("plan:"));
    let mut corpus = Corpus::default();
    corpus.declare_source("plan", &init(Scope::Global));
    corpus.declare_source("github", &init(Scope::Global));
    corpus
        .load("plan", None, listed(plan, invalid))
        .expect("plan is declared");
    corpus
        .load("github", None, listed(github, Vec::new()))
        .expect("github is declared");
    corpus
}

fn view(yaml: &str) -> View {
    load_yaml(yaml.as_bytes(), "view.yaml").unwrap_or_else(|error| panic!("{error}\n{yaml}"))
}

fn try_run(view: &View, corpus: &Corpus, budgets: &Budgets) -> Result<ViewResult, ViewFailure> {
    let compiled = compile_view(view, corpus.rows().len(), budgets)?;
    evaluate_view(&compiled, corpus, budgets)
}

fn run(yaml: &str, corpus: &Corpus) -> ViewResult {
    let view = view(yaml);
    try_run(&view, corpus, &Budgets::default())
        .unwrap_or_else(|error| panic!("the view should evaluate: {error}\n{yaml}"))
}

fn keys(short: &[&str]) -> Vec<String> {
    short.iter().map(|short| format!("plan:{short}")).collect()
}

fn step(short: &[&str]) -> Vec<String> {
    keys(short)
}

// --- the set algebra ---

/// Six tasks: R1 and R2 are ready, D1 is done, and the `mvp` label and collection `plan:E-07` are on some.
fn algebra_board() -> Corpus {
    board(vec![
        labelled(task("R1", "ready"), &["mvp"]),
        task("R2", "ready"),
        collected(task("D1", "done"), &["plan:E-07"]),
        task("X1", "todo"),
        task("X2", "todo"),
        task("X3", "doing"),
    ])
}

#[test]
fn where_alone_is_the_tasks_it_matches() {
    let result = run("name: v\nwhere: \"status == 'ready'\"", &algebra_board());
    assert_eq!(result.members, keys(&["R1", "R2"]));
}

#[test]
fn include_alone_is_those_keys() {
    let result = run("name: v\ninclude: [plan:X2, plan:X1]", &algebra_board());
    assert_eq!(result.members, keys(&["X1", "X2"]), "in the corpus's order");
}

#[test]
fn closure_alone_is_the_goal_and_what_it_depends_on() {
    let corpus = board(vec![
        depends(task("A", "ready"), "B"),
        task("B", "ready"),
        task("C", "ready"),
    ]);
    let result = run(
        "name: v\nclosure: { goal: plan:A, follow: depends_on }",
        &corpus,
    );
    assert_eq!(result.members, keys(&["A", "B"]));
}

#[test]
fn a_view_with_no_part_has_no_members() {
    let result = run("name: v", &algebra_board());
    assert!(result.members.is_empty());
    assert!(result.steps.is_empty());
}

#[test]
fn where_include_and_closure_are_joined() {
    let corpus = board(vec![
        depends(task("A", "ready"), "B"),
        task("B", "doing"),
        labelled(task("M", "todo"), &["mvp"]),
        task("I", "todo"),
        task("N", "todo"),
    ]);
    let result = run(
        "name: v\nwhere: \"'mvp' in labels\"\ninclude: [plan:I]\n\
         closure: { goal: plan:A, follow: depends_on }",
        &corpus,
    );
    assert_eq!(result.members, keys(&["A", "B", "I", "M"]));
}

#[test]
fn a_task_in_exclude_is_out_even_when_where_include_and_the_closure_have_it() {
    let corpus = board(vec![
        depends(task("A", "ready"), "B"),
        labelled(task("B", "ready"), &["mvp"]),
        labelled(task("C", "ready"), &["mvp"]),
    ]);
    let result = run(
        "name: v\nwhere: \"'mvp' in labels\"\ninclude: [plan:B, plan:C]\n\
         exclude: [plan:B, plan:A]\nclosure: { goal: plan:A, follow: depends_on }",
        &corpus,
    );
    assert_eq!(result.members, keys(&["C"]));
}

#[test]
fn a_task_in_include_is_in_even_when_where_does_not_match() {
    let result = run(
        "name: v\nwhere: \"status == 'ready'\"\ninclude: [plan:X1]",
        &algebra_board(),
    );
    assert_eq!(result.members, keys(&["R1", "R2", "X1"]));
}

#[test]
fn a_task_that_several_parts_have_is_one_member() {
    let corpus = board(vec![depends(task("A", "ready"), "B"), task("B", "ready")]);
    let result = run(
        "name: v\nwhere: \"status == 'ready'\"\ninclude: [plan:A, plan:B]\n\
         closure: { goal: plan:A, follow: depends_on }",
        &corpus,
    );
    assert_eq!(result.members, keys(&["A", "B"]));
}

#[test]
fn a_key_at_two_paths_is_one_member() {
    let mut corpus = Corpus::default();
    corpus.set_checkouts(vec![
        CheckoutInfo {
            path: "/repo".into(),
            kind: CheckoutKind::Line { default: true },
        },
        CheckoutInfo {
            path: "/repo-line".into(),
            kind: CheckoutKind::Line { default: false },
        },
    ]);
    corpus.declare_source("plan", &init(Scope::Ref));
    for (checkout, path) in [("/repo", "arch/a.md"), ("/repo-line", "arch/b.md")] {
        let mut copy = task("A", "ready");
        copy.path = Some(path.into());
        corpus
            .load("plan", Some(checkout), listed(vec![copy], Vec::new()))
            .unwrap();
    }
    assert_eq!(
        corpus.rows().len(),
        2,
        "the fixture has one row for each path"
    );
    let result = run("name: v\nwhere: \"status == 'ready'\"", &corpus);
    assert_eq!(result.members, keys(&["A"]));
}

// --- fields ---

#[test]
fn has_is_false_for_a_task_without_the_field_and_the_view_works() {
    let corpus = board(vec![
        with_field(task("A", "ready"), "estimate", json!(3)),
        task("B", "ready"),
    ]);
    let result = run(
        "name: v\nwhere: \"has(fields.estimate) && fields.estimate > 2\"",
        &corpus,
    );
    assert_eq!(result.members, keys(&["A"]));
    assert!(result.counts.errors.is_empty());
}

#[test]
fn a_read_of_a_missing_field_without_has_excludes_the_task_and_counts_one_error_with_its_reason() {
    let corpus = board(vec![
        with_field(task("A", "ready"), "estimate", json!(3)),
        task("B", "ready"),
    ]);
    let result = run("name: v\nwhere: \"fields.estimate > 2\"", &corpus);
    assert_eq!(result.members, keys(&["A"]));
    assert_eq!(result.counts.error_count(), 1);
    let reasons: Vec<&String> = result.counts.errors.keys().collect();
    assert!(reasons[0].contains("estimate"), "{reasons:?}");
}

#[test]
fn an_errored_task_that_include_names_is_still_a_member() {
    let corpus = board(vec![task("B", "ready")]);
    let result = run(
        "name: v\nwhere: \"fields.estimate > 2\"\ninclude: [plan:B]",
        &corpus,
    );
    assert_eq!(result.members, keys(&["B"]));
    assert_eq!(result.counts.error_count(), 1);
}

#[test]
fn the_kind_a_task_leaves_to_its_source_is_the_sources_one_kind() {
    let mut declared = init(Scope::Global);
    declared.kinds = vec![collide_core::item::KindDecl {
        name: "spike".into(),
        statuses: Vec::new(),
        transitions: Vec::new(),
        ops: Vec::new(),
        roles: Vec::new(),
    }];
    let mut corpus = Corpus::default();
    corpus.declare_source("plan", &declared);
    corpus
        .load("plan", None, listed(vec![task("A", "ready")], Vec::new()))
        .unwrap();
    let result = run("name: v\nwhere: \"kind == 'spike'\"", &corpus);
    assert_eq!(result.members, keys(&["A"]));
}

// --- the closure ---

fn closure(goal: &str) -> String {
    format!("name: v\nclosure: {{ goal: {goal}, follow: depends_on }}")
}

#[test]
fn a_chain_is_its_tasks_with_the_deepest_dependency_first() {
    let corpus = board(vec![
        depends(task("A", "ready"), "B"),
        depends(task("B", "ready"), "C"),
        task("C", "ready"),
        task("Z", "ready"),
    ]);
    let result = run(&closure("plan:A"), &corpus);
    assert_eq!(result.members, keys(&["A", "B", "C"]));
    assert_eq!(result.steps, [step(&["C"]), step(&["B"]), step(&["A"])]);
    assert!(result.cycle.is_empty());
}

#[test]
fn a_diamond_has_its_two_middle_tasks_in_one_step_in_key_order() {
    let corpus = board(vec![
        depends(depends(task("A", "ready"), "C"), "B"),
        depends(task("B", "ready"), "D"),
        depends(task("C", "ready"), "D"),
        task("D", "ready"),
    ]);
    let result = run(&closure("plan:A"), &corpus);
    assert_eq!(result.members, keys(&["A", "B", "C", "D"]));
    assert_eq!(
        result.steps,
        [step(&["D"]), step(&["B", "C"]), step(&["A"])]
    );
}

#[test]
fn a_task_waits_for_its_deepest_dependency_not_its_nearest() {
    // A depends on B and on D; B depends on C; C depends on D. A is after B, so in the last step.
    let corpus = board(vec![
        depends(depends(task("A", "ready"), "B"), "D"),
        depends(task("B", "ready"), "C"),
        depends(task("C", "ready"), "D"),
        task("D", "ready"),
    ]);
    let result = run(&closure("plan:A"), &corpus);
    assert_eq!(
        result.steps,
        [step(&["D"]), step(&["C"]), step(&["B"]), step(&["A"])]
    );
}

/// R2's shape: the goal needs one open task and one done task, and the done task needs three more (the stranded
/// ones, T-01-044 to T-01-046) that nothing else reaches.
fn stranded_board() -> Corpus {
    board(vec![
        depends(depends(task("T-01-042", "ready"), "T-01-040"), "T-01-041"),
        task("T-01-040", "doing"),
        depends(task("T-01-041", "done"), "T-01-044"),
        task("T-01-043", "ready"),
        depends(depends(task("T-01-044", "ready"), "T-01-045"), "T-01-046"),
        task("T-01-045", "ready"),
        task("T-01-046", "ready"),
    ])
}

#[test]
fn a_done_dependency_is_not_in_the_set_and_is_not_walked_through() {
    let result = run(&closure("plan:T-01-042"), &stranded_board());
    assert_eq!(result.members, keys(&["T-01-040", "T-01-042"]));
    assert_eq!(result.steps, [step(&["T-01-040"]), step(&["T-01-042"])]);
}

#[test]
fn a_dropped_dependency_is_not_walked_through_either() {
    let corpus = board(vec![
        depends(task("A", "ready"), "B"),
        depends(task("B", "dropped"), "C"),
        task("C", "ready"),
    ]);
    let result = run(&closure("plan:A"), &corpus);
    assert_eq!(result.members, keys(&["A"]));
}

#[test]
fn a_done_goal_is_in_the_set_and_its_open_dependencies_are_walked() {
    let corpus = board(vec![
        depends(task("A", "done"), "B"),
        depends(task("B", "ready"), "C"),
        task("C", "done"),
    ]);
    let result = run(&closure("plan:A"), &corpus);
    assert_eq!(result.members, keys(&["A", "B"]));
    assert_eq!(result.steps, [step(&["B"]), step(&["A"])]);
}

#[test]
fn a_done_goal_with_nothing_open_below_it_is_alone() {
    let corpus = board(vec![depends(task("A", "done"), "B"), task("B", "done")]);
    let result = run(&closure("plan:A"), &corpus);
    assert_eq!(result.members, keys(&["A"]));
    assert_eq!(result.steps, [step(&["A"])]);
}

#[test]
fn a_goal_that_matches_nothing_is_reported_and_the_other_parts_still_count() {
    let corpus = board(vec![task("A", "ready"), task("B", "ready")]);
    let result = run(
        "name: v\ninclude: [plan:A]\nclosure: { goal: plan:GONE, follow: depends_on }",
        &corpus,
    );
    assert_eq!(result.counts.goal_unmatched.as_deref(), Some("plan:GONE"));
    assert_eq!(result.members, keys(&["A"]));
    assert!(result.steps.is_empty());
}

#[test]
fn a_bare_id_as_the_goal_is_not_guessed() {
    let corpus = board(vec![task("T-01-042", "ready")]);
    let result = run(&closure("T-01-042"), &corpus);
    assert_eq!(result.counts.goal_unmatched.as_deref(), Some("T-01-042"));
    assert!(result.members.is_empty());
}

#[test]
fn a_goal_that_is_found_is_not_reported() {
    let result = run(&closure("plan:A"), &board(vec![task("A", "ready")]));
    assert_eq!(result.counts.goal_unmatched, None);
}

#[test]
fn a_dependency_on_a_key_the_corpus_lacks_is_skipped() {
    let corpus = board(vec![
        depends(depends(task("A", "ready"), "GONE"), "plan:ALSO-GONE"),
        task("B", "ready"),
    ]);
    let result = run(&closure("plan:A"), &corpus);
    assert_eq!(result.members, keys(&["A"]));
    assert_eq!(result.steps, [step(&["A"])]);
}

#[test]
fn a_cycle_of_two_ends_has_each_task_once_and_is_the_last_step_marked() {
    let corpus = board(vec![
        depends(task("A", "ready"), "B"),
        depends(task("B", "ready"), "A"),
    ]);
    let result = run(&closure("plan:A"), &corpus);
    assert_eq!(result.members, keys(&["A", "B"]));
    assert_eq!(result.steps, [step(&["A", "B"])]);
    assert_eq!(result.cycle, keys(&["A", "B"]));
}

#[test]
fn a_cycle_of_three_below_a_task_is_the_last_step_after_the_layers_that_do_not_need_it() {
    let corpus = board(vec![
        depends(depends(task("G", "ready"), "A"), "F"),
        depends(task("A", "ready"), "B"),
        depends(task("B", "ready"), "C"),
        depends(task("C", "ready"), "A"),
        task("F", "ready"),
    ]);
    let result = run(&closure("plan:G"), &corpus);
    assert_eq!(result.members, keys(&["A", "B", "C", "F", "G"]));
    assert_eq!(result.steps, [step(&["F"]), step(&["A", "B", "C", "G"])]);
    assert_eq!(
        result.cycle,
        keys(&["A", "B", "C"]),
        "G waits on the cycle, and is not in it"
    );
}

#[test]
fn a_task_that_depends_on_itself_is_a_cycle_of_one() {
    let corpus = board(vec![depends(task("A", "ready"), "A")]);
    let result = run(&closure("plan:A"), &corpus);
    assert_eq!(result.members, keys(&["A"]));
    assert_eq!(result.cycle, keys(&["A"]));
}

#[test]
fn a_closure_without_a_cycle_reports_none() {
    let corpus = board(vec![depends(task("A", "ready"), "B"), task("B", "ready")]);
    assert!(run(&closure("plan:A"), &corpus).cycle.is_empty());
}

#[test]
fn every_dependency_link_is_followed_and_a_non_blocking_one_is_not() {
    assert_eq!(DEPENDENCY_LINKS, ["blocked-by", "depends_on"]);
    let mut not_blocking = task("N", "ready");
    not_blocking.links.push(
        serde_json::from_value(json!({ "type": "blocked-by", "to": "Q", "blocking": false }))
            .unwrap(),
    );
    let corpus = board(vec![
        blocked_by(depends(task("A", "ready"), "B"), "C"),
        task("B", "ready"),
        task("C", "ready"),
        with_link(task("D", "ready"), "relates-to", "A"),
        not_blocking,
        task("Q", "ready"),
    ]);
    assert_eq!(
        run(&closure("plan:A"), &corpus).members,
        keys(&["A", "B", "C"])
    );
    assert_eq!(run(&closure("plan:D"), &corpus).members, keys(&["D"]));
    assert_eq!(run(&closure("plan:N"), &corpus).members, keys(&["N"]));
}

#[test]
fn the_closure_and_the_deps_variable_read_the_same_dependencies() {
    let mut a = with_link(
        blocked_by(depends(task("A", "ready"), "B"), "C"),
        "relates-to",
        "D",
    );
    a.links.push(
        serde_json::from_value(json!({ "type": "blocked-by", "to": "E", "blocking": false }))
            .unwrap(),
    );
    let corpus = board(vec![
        a,
        task("B", "ready"),
        task("C", "ready"),
        task("D", "ready"),
        task("E", "ready"),
    ]);
    let by_variable = run("name: v\nwhere: \"deps == ['B', 'C']\"", &corpus);
    assert_eq!(
        by_variable.members,
        keys(&["A"]),
        "the variable holds B and C, and nothing else"
    );
    let by_closure = run(&closure("plan:A"), &corpus);
    assert_eq!(
        by_closure.members,
        keys(&["A", "B", "C"]),
        "the closure walks the same two"
    );
}

#[test]
fn a_dependency_on_another_source_is_walked() {
    let corpus = board(vec![
        depends(task("A", "ready"), "github:acme/edge#7"),
        depends(issue(7, "ready"), "plan:B"),
        task("B", "ready"),
    ]);
    let result = run(&closure("plan:A"), &corpus);
    assert_eq!(
        result.members,
        ["github:acme/edge#7", "plan:A", "plan:B"]
            .map(String::from)
            .to_vec()
    );
}

#[test]
fn the_closure_is_computed_before_exclude_and_the_steps_leave_the_excluded_out() {
    let corpus = board(vec![
        depends(task("A", "ready"), "B"),
        depends(task("B", "ready"), "C"),
        task("C", "ready"),
    ]);
    let result = run(
        "name: v\nexclude: [plan:B]\nclosure: { goal: plan:A, follow: depends_on }",
        &corpus,
    );
    assert_eq!(result.members, keys(&["A", "C"]), "C is reached through B");
    assert_eq!(result.steps, [step(&["C"]), step(&["A"])]);
}

// --- errors on a view, and the counts ---

#[test]
fn several_views_on_one_corpus_one_broken_each_way_leave_the_others_unaffected() {
    let corpus = algebra_board();
    let budgets = Budgets::default();
    let long = format!("true{}", "&&true".repeat(200));
    let broken: Vec<(&str, String)> = vec![
        ("a where that does not compile", "status ==".to_string()),
        ("a where over 1 KB", long),
        ("a corpus variable", "tasks.size() > 0".to_string()),
        (
            "a where that is too costly",
            "labels.all(a, labels.all(b, labels.all(c, a == b && b == c)))".to_string(),
        ),
        ("an empty where", "  ".to_string()),
    ];
    let good = view("name: good\nwhere: \"status == 'ready'\"");
    for (reason, source) in &broken {
        let bad = View {
            where_: Some(source.clone()),
            ..view("name: bad")
        };
        let failure = try_run(&bad, &corpus, &budgets).expect_err(reason);
        assert!(!failure.to_string().is_empty(), "{reason}");
        let result = try_run(&good, &corpus, &budgets).unwrap();
        assert_eq!(result.members, keys(&["R1", "R2"]), "after {reason}");
    }
}

#[test]
fn each_way_a_view_is_refused_is_named() {
    let corpus = algebra_board();
    let budgets = Budgets::default();
    let refuse = |where_: &str| {
        let bad = View {
            where_: Some(where_.into()),
            ..view("name: bad")
        };
        compile_view(&bad, corpus.rows().len(), &budgets).unwrap_err()
    };
    assert!(matches!(
        refuse("status =="),
        ViewFailure::Expr(ExprError::Syntax { .. })
    ));
    assert!(matches!(
        refuse("tasks.size() > 0"),
        ViewFailure::Expr(ExprError::UnknownName { .. })
    ));
    assert!(matches!(
        refuse("labels.all(a, labels.all(b, labels.all(c, a == b && b == c)))"),
        ViewFailure::Expr(ExprError::TooCostly { .. })
    ));
    let long = format!("true{}", "&&true".repeat(200));
    assert_eq!(
        refuse(&long),
        ViewFailure::Check(vec![ViewError::WhereTooLong { bytes: long.len() }])
    );
    let unnamed = View {
        name: String::new(),
        ..view("name: x")
    };
    assert_eq!(
        compile_view(&unnamed, 0, &budgets).unwrap_err(),
        ViewFailure::Check(vec![ViewError::EmptyName])
    );
}

#[test]
fn a_view_that_check_view_refuses_says_every_reason_and_not_only_the_first() {
    let bad = View {
        name: String::new(),
        where_: Some(" ".into()),
        ..view("name: x")
    };
    let ViewFailure::Check(errors) = compile_view(&bad, 0, &Budgets::default()).unwrap_err() else {
        panic!("a check failure");
    };
    assert_eq!(errors, [ViewError::EmptyName, ViewError::EmptyWhere]);
}

#[test]
fn a_view_with_no_where_has_no_estimate_and_one_with_a_where_has_one() {
    let budgets = Budgets::default();
    let none = compile_view(&view("name: v"), 10, &budgets).unwrap();
    assert!(none.estimate().is_none());
    let some = compile_view(&view("name: v\nwhere: \"status == 'ready'\""), 10, &budgets).unwrap();
    assert!(
        some.estimate()
            .is_some_and(|found| found.steps_per_task > 0)
    );
}

fn hostile_board() -> Corpus {
    let many: Vec<String> = (0..MAX_LIST_ENTRIES)
        .map(|index| format!("l{index}"))
        .collect();
    let mut hostile = task("HOSTILE", "ready");
    hostile.labels = many;
    board(vec![
        labelled(task("A", "ready"), &["x", "y"]),
        hostile,
        labelled(task("Z", "ready"), &["x", "y", "z"]),
    ])
}

/// Passes the static estimate, and costs a task with 1,000 labels more than the task budget below.
const EACH_LABEL: &str = "labels.all(a, a.size() > 0)";

#[test]
fn a_runaway_where_on_one_hostile_task_is_counted_for_that_task_and_the_others_are_tested() {
    let source = view(&format!("name: v\nwhere: \"{EACH_LABEL}\""));
    let budgets = Budgets {
        per_task: 500,
        ..Budgets::default()
    };
    let result = try_run(&source, &hostile_board(), &budgets).unwrap();
    assert_eq!(result.counts.over_task_budget, 1);
    assert_eq!(result.members, keys(&["A", "Z"]));
    assert!(!result.counts.pass_budget_exhausted);
    assert_eq!(result.counts.error_count(), 0);
}

#[test]
fn a_pass_that_runs_out_of_budget_says_so_and_counts_the_tasks_it_did_not_reach() {
    let tasks: Vec<Task> = (0..200)
        .map(|index| task(&format!("T-{index:03}"), "ready"))
        .collect();
    let corpus = board(tasks);
    let budgets = Budgets {
        per_task: 100,
        per_pass: 300,
    };
    let view = view("name: v\nwhere: \"status == 'ready'\"");
    // The estimate is for a board of the usual size, so it is checked against the default budgets.
    let compiled = compile_view(&view, corpus.rows().len(), &Budgets::default()).unwrap();
    let result = evaluate_view(&compiled, &corpus, &budgets).unwrap();
    assert!(result.counts.pass_budget_exhausted);
    assert!(result.counts.not_reached > 0 && result.counts.not_reached < 200);
    assert_eq!(
        result.members.len() + result.counts.not_reached,
        200,
        "a task is a member or was not reached"
    );
}

#[test]
fn a_pass_that_finishes_does_not_say_it_ran_out() {
    let result = run("name: v\nwhere: \"status == 'ready'\"", &algebra_board());
    assert!(!result.counts.pass_budget_exhausted);
    assert_eq!(result.counts.not_reached, 0);
}

#[test]
fn an_evaluation_error_is_counted_by_reason_and_each_task_that_raised_it_is_counted() {
    let corpus = board(vec![
        task("A", "ready"),
        task("B", "ready"),
        with_field(task("C", "ready"), "estimate", json!("three")),
        with_field(task("D", "ready"), "estimate", json!(5)),
    ]);
    let result = run("name: v\nwhere: \"fields.estimate > 4\"", &corpus);
    assert_eq!(result.members, keys(&["D"]));
    assert_eq!(result.counts.error_count(), 3);
    assert!(
        result.counts.errors.len() >= 2,
        "{:?}",
        result.counts.errors
    );
}

#[test]
fn include_and_exclude_keys_that_match_nothing_are_listed_so_a_typo_shows() {
    let result = run(
        "name: v\ninclude: [plan:R1, plan:TYPO, T-01-042]\nexclude: [plan:X1, plan:NOPE]",
        &algebra_board(),
    );
    assert_eq!(result.counts.unmatched_include, ["plan:TYPO", "T-01-042"]);
    assert_eq!(result.counts.unmatched_exclude, ["plan:NOPE"]);
    assert_eq!(result.members, keys(&["R1"]));
}

#[test]
fn a_key_that_was_renamed_still_matches() {
    let mut corpus = algebra_board();
    corpus
        .apply_renamed(
            "plan",
            &collide_core::change::Rename {
                from: "plan:R1".into(),
                to: "plan:R9".into(),
                checkout: None,
            },
        )
        .unwrap();
    let result = run("name: v\ninclude: [plan:R1]", &corpus);
    assert_eq!(result.members, keys(&["R9"]));
    assert!(result.counts.unmatched_include.is_empty());
}

fn invalid(path: &str) -> TaskInvalid {
    TaskInvalid {
        path: path.into(),
        checkout: None,
        reason: "not a task".into(),
    }
}

#[test]
fn the_corpus_invalid_items_count_on_every_view_even_a_narrow_one() {
    let mut undeclared = task("U", "todo");
    undeclared.state.status = "not_a_status".into();
    let corpus = board_with_invalid(
        vec![task("A", "ready"), undeclared],
        vec![invalid("arch/broken.md"), invalid("arch/also.md")],
    );
    assert_eq!(corpus.invalid().len(), 3);
    for yaml in [
        "name: v",
        "name: v\nwhere: \"status == 'nothing'\"",
        "name: v\ninclude: [plan:A]",
        "name: v\nclosure: { goal: plan:A, follow: depends_on }",
    ] {
        assert_eq!(run(yaml, &corpus).counts.invalid_items, 3, "{yaml}");
    }
}

#[test]
fn a_clean_corpus_has_no_counts() {
    let result = run("name: v\nwhere: \"status == 'ready'\"", &algebra_board());
    assert_eq!(result.counts, Default::default());
}

// --- collections and labels ---

#[test]
fn a_collection_test_returns_exactly_the_tasks_whose_collections_hold_it() {
    let corpus = board(vec![
        collected(task("A", "ready"), &["plan:E-07"]),
        collected(task("B", "ready"), &["plan:E-07", "plan:E-08"]),
        collected(task("C", "ready"), &["plan:E-08"]),
        collected(task("D", "ready"), &["plan:E-070"]),
        task("E", "ready"),
    ]);
    let result = run("name: v\nwhere: \"'plan:E-07' in collections\"", &corpus);
    assert_eq!(result.members, keys(&["A", "B"]));
}

#[test]
fn a_label_test_returns_exactly_the_tasks_with_the_label() {
    let result = run("name: v\nwhere: \"'mvp' in labels\"", &algebra_board());
    assert_eq!(result.members, keys(&["R1"]));
}

// --- the example view, end to end ---

#[test]
fn the_example_view_of_the_plan_gives_its_members_its_steps_and_its_counts() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../fixtures/views/edge-mvp.yaml");
    let example: View = load_yaml_file(&path).unwrap();
    let mut gh_epic = issue(212, "ready");
    gh_epic.title = "hand-picked".into();
    let corpus = board(vec![
        // The collection and the label bring these in.
        collected(task("T-01-041", "doing"), &["plan:E-07"]),
        labelled(task("T-01-050", "todo"), &["mvp"]),
        // The goal needs T-01-040, and a done task that needs three stranded ones.
        depends(depends(task("T-01-042", "ready"), "T-01-040"), "T-01-039"),
        task("T-01-040", "doing"),
        depends(task("T-01-039", "done"), "T-01-044"),
        // Excluded though the collection has it.
        collected(task("T-01-043", "ready"), &["plan:E-07"]),
        task("T-01-044", "ready"),
        // Not in the view at all.
        task("T-01-060", "ready"),
        gh_epic,
    ]);
    let budgets = Budgets::default();
    let result = try_run(&example, &corpus, &budgets).unwrap();
    assert_eq!(
        result.members,
        [
            "github:acme/edge#212",
            "plan:T-01-040",
            "plan:T-01-041",
            "plan:T-01-042",
            "plan:T-01-050",
        ]
        .map(String::from)
        .to_vec()
    );
    assert_eq!(result.steps, [step(&["T-01-040"]), step(&["T-01-042"])]);
    assert_eq!(result.counts, Default::default());
}

// --- order ---

#[test]
fn the_same_inputs_give_the_same_members_in_the_same_order_every_time() {
    let corpus = algebra_board();
    let yaml = "name: v\nwhere: \"status == 'ready' || 'mvp' in labels\"\ninclude: [plan:X3]";
    let first = run(yaml, &corpus);
    for _ in 0..5 {
        assert_eq!(run(yaml, &corpus), first);
    }
}

#[test]
fn a_shuffled_listing_gives_the_same_members_steps_and_counts() {
    let tasks = vec![
        depends(depends(task("A", "ready"), "C"), "B"),
        depends(task("B", "ready"), "D"),
        depends(task("C", "ready"), "D"),
        task("D", "ready"),
        task("E", "ready"),
    ];
    let yaml =
        "name: v\nwhere: \"status == 'ready'\"\nclosure: { goal: plan:A, follow: depends_on }";
    let expected = run(yaml, &board(tasks.clone()));
    assert_eq!(expected.members, keys(&["A", "B", "C", "D", "E"]));
    let mut reversed = tasks.clone();
    reversed.reverse();
    let mut rotated = tasks.clone();
    rotated.rotate_left(2);
    for shuffled in [reversed, rotated] {
        assert_eq!(run(yaml, &board(shuffled)), expected);
    }
}
