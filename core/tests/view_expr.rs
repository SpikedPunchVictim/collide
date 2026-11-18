//! Tests for CEL for views (ts-0117): the compile checks, the static estimate, the task a view sees, its caps, and
//! the step budgets of a pass. Pure: no file, no process. The step counter is thread local, and each test runs on its
//! own thread, so no test sees another's budget.

use cel::{Env, ExecutionError, Program};
use collide_core::domain::{Provenance, TrustLevel};
use collide_core::item::{Category, Link};
use collide_core::testing::fake_task;
use collide_core::view::{MAX_CEL_SOURCE_BYTES, MAX_LIST_ENTRIES, MAX_STRING_BYTES};
use collide_core::view_expr::{
    self, Budgets, CompiledExpr, ExprError, MAX_STEPS_PER_PASS, MAX_STEPS_PER_TASK, PassOutcome,
    TASK_NAMES, TaskVars, check_cost, check_task_caps, compile, compile_with, estimate,
    is_budget_error,
};
use serde_json::json;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

// --- fixtures ---

fn vars(title: &str) -> TaskVars {
    TaskVars::bind(&task(title), "task", None).expect("a small task binds")
}

fn task(title: &str) -> collide_core::provider::Task {
    let mut task = fake_task("T-01", title, "ready", Category::Ready);
    task.key = "plan:T-01".into();
    task
}

fn names(prefix: &str, count: usize) -> Vec<String> {
    (0..count).map(|index| format!("{prefix}{index}")).collect()
}

fn link(link_type: &str, to: &str) -> Link {
    serde_json::from_value(json!({ "type": link_type, "to": to })).expect("a link reads")
}

fn must_compile(source: &str) -> CompiledExpr {
    compile(source).unwrap_or_else(|error| panic!("`{source}` should compile: {error}"))
}

fn evaluate_pass(compiled: &CompiledExpr, tasks: &[TaskVars], budgets: &Budgets) -> PassOutcome {
    view_expr::evaluate_pass(compiled, tasks, budgets).expect("a thread starts")
}

/// A budget no test reaches by accident.
fn unlimited() -> Budgets {
    Budgets {
        per_task: u64::MAX,
        per_pass: u64::MAX,
    }
}

/// A `true` of `bytes` bytes: `true`, then `&&true` terms, then white space.
fn padded_true(bytes: usize) -> String {
    let terms = (bytes - 4) / 6;
    let mut source = format!("true{}", "&&true".repeat(terms));
    source.push_str(&" ".repeat(bytes - source.len()));
    source
}

// --- the source cap ---

#[test]
fn a_source_of_1023_and_of_1024_bytes_compiles() {
    for bytes in [1023, MAX_CEL_SOURCE_BYTES] {
        let source = padded_true(bytes);
        assert_eq!(source.len(), bytes);
        must_compile(&source);
    }
}

#[test]
fn a_source_of_1025_bytes_is_refused_with_its_length_and_the_cap() {
    let source = padded_true(MAX_CEL_SOURCE_BYTES + 1);
    let error = compile(&source).unwrap_err();
    assert_eq!(error, ExprError::TooLong { bytes: 1025 });
    let text = error.to_string();
    assert!(text.contains("1025") && text.contains("1024"), "{text}");
}

#[test]
fn an_empty_source_is_refused() {
    assert_eq!(compile("").unwrap_err(), ExprError::Empty);
    assert_eq!(compile("  \n ").unwrap_err(), ExprError::Empty);
}

#[test]
fn a_source_that_does_not_parse_is_a_syntax_error_with_the_parsers_message() {
    let error = compile("status ==").unwrap_err();
    assert!(matches!(error, ExprError::Syntax { .. }), "{error:?}");
}

#[test]
fn a_hundred_thousand_nested_brackets_are_refused_before_the_parser_sees_them() {
    let source = "[".repeat(100_000);
    let parsed = AtomicBool::new(false);
    let error = compile_with(&source, &|text| {
        parsed.store(true, Ordering::SeqCst);
        Env::stdlib().compile(text)
    })
    .unwrap_err();
    assert_eq!(error, ExprError::TooLong { bytes: 100_000 });
    assert!(
        !parsed.load(Ordering::SeqCst),
        "the parser was never called"
    );
}

// --- a panic ---

#[test]
fn a_compile_that_panics_is_an_error_on_that_expression_and_the_next_one_compiles() {
    let error = compile_with("status == 'ready'", &|_| panic!("the parser broke")).unwrap_err();
    match error {
        ExprError::Panicked { message } => {
            assert!(message.contains("the parser broke"), "{message}")
        }
        other => panic!("{other:?}"),
    }
    must_compile("status == 'ready'");
}

#[test]
fn an_evaluation_that_panics_is_counted_for_that_task_and_the_others_are_still_tested() {
    let compiled = must_compile("status == 'ready'");
    let tasks = [vars("a"), vars("b"), vars("c")];
    let calls = AtomicUsize::new(0);
    let outcome =
        view_expr::evaluate_pass_with(&compiled, &tasks, &unlimited(), &|program, context| {
            if calls.fetch_add(1, Ordering::SeqCst) == 1 {
                panic!("the evaluation broke");
            }
            program.execute(context)
        })
        .expect("a thread starts");
    assert_eq!(outcome.matched, [0, 2]);
    assert_eq!(outcome.error_count(), 1);
    let reason = outcome.errors.keys().next().unwrap();
    assert!(
        reason.contains("panicked") && reason.contains("the evaluation broke"),
        "{reason}"
    );
}

// --- the stack ---

#[test]
fn deep_sources_within_the_cap_are_parsed_walked_and_run_without_overflowing_the_callers_stack() {
    // Twenty macros inside one another, 394 bytes, overflowed a 2 MB stack in the parser (a debug build).
    let mut nested = "true".to_string();
    for depth in 0..45 {
        nested = format!("labels.exists(v{depth}, {nested})");
    }
    let nots = format!("{}true", "!".repeat(1_000));
    let sums = vec!["1"; 500].join("+") + " == 500";
    for source in [nested, nots, sums] {
        assert!(source.len() <= MAX_CEL_SOURCE_BYTES, "{}", source.len());
        let compiled = must_compile(&source);
        let _ = estimate(&compiled);
        let outcome = evaluate_pass(&compiled, &[small_lists()], &unlimited());
        assert_eq!(
            outcome.matched.len() + outcome.errors.len(),
            1,
            "{source}: {outcome:?}"
        );
    }
}

// --- the names a view may use ---

#[test]
fn the_corpus_variable_is_refused_with_its_name_and_the_names_a_view_may_use() {
    let error = compile("tasks.exists(t, t.key == 'plan:T-01')").unwrap_err();
    assert_eq!(
        error,
        ExprError::UnknownName {
            name: "tasks".into()
        }
    );
    let text = error.to_string();
    for name in TASK_NAMES {
        assert!(text.contains(name), "{name} is listed: {text}");
    }
}

#[test]
fn any_unknown_name_is_refused_not_only_tasks() {
    for (source, name) in [
        ("corpus.size() > 1", "corpus"),
        ("depends_on.size() > 1", "depends_on"),
        ("id == 'x'", "id"),
        ("[1, 2].exists(n, n == missing)", "missing"),
        ("has(everything.x)", "everything"),
        ("{'a': other}.size() == 1", "other"),
    ] {
        assert_eq!(
            compile(source).unwrap_err(),
            ExprError::UnknownName { name: name.into() },
            "{source}"
        );
    }
}

#[test]
fn a_macros_own_variable_is_allowed_inside_it_and_refused_outside() {
    must_compile("labels.exists(l, l == 'x')");
    must_compile("labels.exists(l, collections.exists(c, c == l))");
    must_compile("labels.map(l, l + '!').filter(m, m.startsWith('p')).size() == 1");
    assert_eq!(
        compile("labels.exists(l, true) && l == 'x'").unwrap_err(),
        ExprError::UnknownName { name: "l".into() }
    );
}

#[test]
fn a_name_in_a_macros_range_is_checked_outside_the_macros_variables() {
    // `l` is not yet bound where the range is read.
    assert_eq!(
        compile("l.exists(l, true)").unwrap_err(),
        ExprError::UnknownName { name: "l".into() }
    );
}

#[test]
fn every_task_name_and_the_type_names_compile() {
    for name in TASK_NAMES {
        must_compile(&format!("type({name}) != null_type"));
    }
    must_compile("type(labels) == list && type(title) == string && type(fields) == map");
}

#[test]
fn every_task_name_is_bound_to_a_value() {
    let compiled = must_compile(
        "size(key) > 0 && size(source) > 0 && size(title) > 0 && size(status) > 0 && size(category) > 0 \
         && size(kind) > 0 && size(labels) > 0 && size(collections) > 0 && size(deps) > 0 \
         && size(author) > 0 && size(fields) > 0",
    );
    let mut task = task("a title");
    task.labels = vec!["mvp".into()];
    task.collections = vec!["plan:E-07".into()];
    task.links = vec![link("blocked-by", "T-00")];
    task.fields.insert("estimate".into(), json!(3));
    let provenance = Provenance {
        author: Some("ana".into()),
        trust: TrustLevel::Trusted,
    };
    let bound = TaskVars::bind(&task, "task", Some(&provenance)).unwrap();
    let outcome = evaluate_pass(&compiled, &[bound], &unlimited());
    assert_eq!(
        (outcome.matched.as_slice(), outcome.errors.len()),
        ([0].as_slice(), 0)
    );
}

// --- the task a view sees ---

#[test]
fn a_task_is_bound_as_its_source_listed_it() {
    let mut task = task("Build the thing");
    task.labels = vec!["mvp".into()];
    task.collections = vec!["plan:E-07".into()];
    task.state.category = Category::Active;
    task.state.status = "doing".into();
    let bound = TaskVars::bind(&task, "spike", None).unwrap();
    assert_eq!(
        (
            bound.key.as_str(),
            bound.source.as_str(),
            bound.title.as_str(),
            bound.status.as_str(),
            bound.category.as_str(),
            bound.kind.as_str(),
        ),
        (
            "plan:T-01",
            "plan",
            "Build the thing",
            "doing",
            "active",
            "spike"
        )
    );
    assert_eq!(
        (bound.labels, bound.collections),
        (vec!["mvp".to_string()], vec!["plan:E-07".to_string()])
    );
}

#[test]
fn author_comes_from_the_provenance_and_is_empty_when_unknown() {
    let known = Provenance {
        author: Some("ana".into()),
        trust: TrustLevel::Untrusted,
    };
    let unknown = Provenance {
        author: None,
        trust: TrustLevel::Trusted,
    };
    let task = task("t");
    assert_eq!(
        TaskVars::bind(&task, "", Some(&known)).unwrap().author,
        "ana"
    );
    assert_eq!(
        TaskVars::bind(&task, "", Some(&unknown)).unwrap().author,
        ""
    );
    assert_eq!(TaskVars::bind(&task, "", None).unwrap().author, "");
}

#[test]
fn deps_are_the_targets_of_the_blocking_dependency_links_in_order() {
    let mut task = task("t");
    task.links = vec![
        link("blocked-by", "T-02"),
        link("relates-to", "T-09"),
        link("depends_on", "T-03"),
    ];
    let mut loose = link("blocked-by", "T-04");
    loose.blocking = Some(false);
    task.links.push(loose);
    assert_eq!(
        TaskVars::bind(&task, "", None).unwrap().deps,
        ["T-02", "T-03"]
    );
}

#[test]
fn has_of_a_source_field_is_false_for_a_task_without_it_and_true_for_one_with_it() {
    let compiled = must_compile("has(fields.estimate) && fields.estimate > 2");
    let mut with = task("with");
    with.fields.insert("estimate".into(), json!(3));
    let tasks = [
        TaskVars::bind(&task("without"), "", None).unwrap(),
        TaskVars::bind(&with, "", None).unwrap(),
    ];
    let outcome = evaluate_pass(&compiled, &tasks, &unlimited());
    assert_eq!(outcome.matched, [1]);
    assert_eq!(outcome.error_count(), 0, "{:?}", outcome.errors);
}

#[test]
fn a_read_of_a_missing_field_without_has_excludes_the_task_and_is_counted_with_its_reason() {
    let compiled = must_compile("fields.estimate > 2");
    let mut with = task("with");
    with.fields.insert("estimate".into(), json!(3));
    let tasks = [
        TaskVars::bind(&task("a"), "", None).unwrap(),
        TaskVars::bind(&with, "", None).unwrap(),
        TaskVars::bind(&task("b"), "", None).unwrap(),
    ];
    let outcome = evaluate_pass(&compiled, &tasks, &unlimited());
    assert_eq!(outcome.matched, [1]);
    assert_eq!(outcome.errors.len(), 1);
    let (reason, count) = outcome.errors.iter().next().unwrap();
    assert!(reason.contains("estimate"), "{reason}");
    assert_eq!(*count, 2);
}

// --- the caps ---

#[test]
fn a_list_of_exactly_the_cap_is_accepted_and_one_more_is_not() {
    let mut task = task("t");
    task.labels = names("l", MAX_LIST_ENTRIES);
    assert_eq!(check_task_caps(&task, None), Ok(()));
    task.labels = names("l", MAX_LIST_ENTRIES + 1);
    let reason = check_task_caps(&task, None).unwrap_err().reason;
    assert!(
        reason.contains("`labels`") && reason.contains("1001") && reason.contains("1000"),
        "{reason}"
    );
}

#[test]
fn collections_and_dependencies_are_capped_like_labels() {
    let mut task = task("t");
    task.collections = names("c", MAX_LIST_ENTRIES + 1);
    assert!(
        check_task_caps(&task, None)
            .unwrap_err()
            .reason
            .contains("`collections`")
    );
    task.collections.clear();
    task.links = names("T-", MAX_LIST_ENTRIES)
        .iter()
        .map(|to| link("blocked-by", to))
        .collect();
    assert_eq!(check_task_caps(&task, None), Ok(()));
    task.links.push(link("depends_on", "T-extra"));
    assert!(
        check_task_caps(&task, None)
            .unwrap_err()
            .reason
            .contains("`deps`")
    );
}

#[test]
fn links_that_are_not_dependencies_do_not_count_toward_the_cap() {
    let mut task = task("t");
    task.links = names("T-", MAX_LIST_ENTRIES + 1)
        .iter()
        .map(|to| link("relates-to", to))
        .collect();
    assert_eq!(check_task_caps(&task, None), Ok(()));
}

#[test]
fn a_string_of_exactly_the_cap_is_accepted_and_one_more_byte_is_not() {
    let mut task = task(&"t".repeat(MAX_STRING_BYTES));
    assert_eq!(check_task_caps(&task, None), Ok(()));
    task.title.push('t');
    let reason = check_task_caps(&task, None).unwrap_err().reason;
    assert!(
        reason.contains("`title`") && reason.contains("4097") && reason.contains("4096"),
        "{reason}"
    );
}

#[test]
fn the_cap_counts_bytes_not_characters() {
    // 2,049 two-byte characters are 4,098 bytes.
    let task = task(&"é".repeat(MAX_STRING_BYTES / 2 + 1));
    assert!(
        check_task_caps(&task, None)
            .unwrap_err()
            .reason
            .contains("4098")
    );
}

#[test]
fn a_long_string_inside_a_list_or_in_fields_or_as_the_author_is_over_the_cap() {
    let long = "x".repeat(MAX_STRING_BYTES + 1);
    let mut in_list = task("t");
    in_list.labels = vec!["ok".into(), "ok".into(), long.clone()];
    assert!(
        check_task_caps(&in_list, None)
            .unwrap_err()
            .reason
            .contains("`labels[2]`")
    );

    let mut in_fields = task("t");
    in_fields
        .fields
        .insert("notes".into(), json!({ "deep": [long.clone()] }));
    let reason = check_task_caps(&in_fields, None).unwrap_err().reason;
    assert!(reason.contains("fields.notes.deep[0]"), "{reason}");

    let mut many = task("t");
    many.fields
        .insert("tags".into(), json!(vec![0; MAX_LIST_ENTRIES + 1]));
    assert!(
        check_task_caps(&many, None)
            .unwrap_err()
            .reason
            .contains("fields.tags")
    );

    assert!(
        check_task_caps(&task("t"), Some(&long))
            .unwrap_err()
            .reason
            .contains("`author`")
    );
}

#[test]
fn a_task_over_a_cap_is_not_bound() {
    let mut over = task("t");
    over.labels = names("l", MAX_LIST_ENTRIES + 1);
    assert!(
        TaskVars::bind(&over, "", None)
            .unwrap_err()
            .reason
            .contains("`labels`")
    );
    let provenance = Provenance {
        author: Some("a".repeat(MAX_STRING_BYTES + 1)),
        trust: TrustLevel::Trusted,
    };
    assert!(TaskVars::bind(&task("t"), "", Some(&provenance)).is_err());
}

// --- the static estimate ---

/// E1 to E6 of S5, with the plan's names (`deps` for `depends_on`).
const ORDINARY: [&str; 6] = [
    "status == 'ready'",
    "'plan:E-07' in collections || 'mvp' in labels",
    "size(deps) == 0 && labels.exists(l, l.startsWith('p'))",
    "title.contains('edge') && author == 'me'",
    "key.matches('^plan:T-0[0-9]-1.*') && category != 'dropped'",
    "deps.all(d, d.startsWith('T-')) && !(kind in ['bug', 'spike'])",
];

const NESTED: [&str; 4] = [
    "labels.all(a, labels.exists(b, a == b))",
    "labels.filter(a, collections.exists(b, a == b)).size() > 0",
    "labels.map(a, labels.map(b, a + b)).size() > 0",
    "labels.filter(a, labels.exists(b, labels.exists(c, a == c && b == c && false))).size() > 0",
];

/// A task with a few of each list, so that the measured steps are for lists well under the estimate's bound.
fn small_lists() -> TaskVars {
    let mut task = task("an edge case");
    task.labels = names("p", 5);
    task.collections = vec!["plan:E-07".into(), "plan:E-08".into()];
    task.links = vec![
        link("blocked-by", "T-01"),
        link("blocked-by", "T-02"),
        link("blocked-by", "T-03"),
    ];
    TaskVars::bind(&task, "task", None).unwrap()
}

fn measured_steps(source: &str, task: &TaskVars) -> u64 {
    let outcome = evaluate_pass(
        &must_compile(source),
        std::slice::from_ref(task),
        &unlimited(),
    );
    assert_eq!(outcome.error_count(), 0, "{source}: {:?}", outcome.errors);
    outcome.steps
}

#[test]
fn the_estimate_is_never_under_the_steps_the_counter_measures() {
    let task = small_lists();
    for source in ORDINARY.iter().chain(NESTED.iter()) {
        let estimated = estimate(&must_compile(source)).steps_per_task;
        let measured = measured_steps(source, &task);
        assert!(
            measured > 0 && u128::from(measured) <= estimated,
            "{source}: {measured} against {estimated}"
        );
    }
}

#[test]
fn the_estimate_of_a_macro_over_a_list_in_the_source_uses_its_length() {
    let small = estimate(&must_compile(
        "['a', 'b'].exists(x, ['c', 'd'].exists(y, x == y))",
    ))
    .steps_per_task;
    let unknown = estimate(&must_compile(
        "labels.exists(x, collections.exists(y, x == y))",
    ))
    .steps_per_task;
    assert!(small < 100, "{small}");
    assert!(unknown > 100 * small, "{unknown} against {small}");
}

#[test]
fn an_ordinary_expression_passes_the_cost_check_on_a_board_of_the_usual_size() {
    for source in ORDINARY {
        let found = check_cost(&must_compile(source), 1_600, &Budgets::default())
            .unwrap_or_else(|error| panic!("{source}: {error}"));
        assert!(
            found.steps_per_task < 600,
            "{source}: {}",
            found.steps_per_task
        );
    }
}

#[test]
fn a_nested_comprehension_that_cannot_finish_within_the_budget_is_refused_with_the_estimate_shown()
{
    for source in NESTED {
        let compiled = must_compile(source);
        let per_task = estimate(&compiled).steps_per_task;
        let error = check_cost(&compiled, 1_600, &Budgets::default()).unwrap_err();
        assert_eq!(
            error,
            ExprError::TooCostly {
                per_task,
                tasks: 1_600,
                total: per_task * 1_600,
                limit: MAX_STEPS_PER_PASS,
            },
            "{source}"
        );
        let text = error.to_string();
        assert!(
            text.contains(&per_task.to_string()) && text.contains("2000000"),
            "{text}"
        );
    }
}

#[test]
fn a_board_smaller_than_the_floor_is_estimated_at_the_floor() {
    let compiled = must_compile(NESTED[0]);
    let error = check_cost(&compiled, 3, &Budgets::default()).unwrap_err();
    assert!(
        matches!(error, ExprError::TooCostly { tasks: 1_600, .. }),
        "{error:?}"
    );
}

#[test]
fn a_bigger_board_is_estimated_at_its_size() {
    let source = ORDINARY[2];
    let compiled = must_compile(source);
    let per_task = estimate(&compiled).steps_per_task;
    let tasks = (u128::from(MAX_STEPS_PER_PASS) / per_task) as usize + 1;
    assert!(tasks > 1_600, "{tasks}");
    let error = check_cost(&compiled, tasks, &Budgets::default()).unwrap_err();
    assert!(
        matches!(error, ExprError::TooCostly { tasks: counted, .. } if counted == tasks as u128),
        "{error:?}"
    );
    assert!(check_cost(&compiled, tasks - 1, &Budgets::default()).is_ok());
}

#[test]
fn a_deep_nesting_does_not_overflow_the_estimate() {
    let mut source = "true".to_string();
    for depth in 0..25 {
        source = format!("labels.exists(v{depth}, {source})");
    }
    assert!(source.len() <= MAX_CEL_SOURCE_BYTES, "{}", source.len());
    let compiled = compile(&source).unwrap_or_else(|error| panic!("{error}"));
    assert!(matches!(
        check_cost(&compiled, 1_600, &Budgets::default()),
        Err(ExprError::TooCostly { .. })
    ));
}

#[test]
fn matches_adds_a_note_and_other_calls_do_not() {
    assert!(estimate(&must_compile("key.matches('^plan:')")).uses_matches);
    assert!(estimate(&must_compile("matches(key, '^plan:')")).uses_matches);
    assert!(estimate(&must_compile("labels.exists(l, l.matches('^p'))")).uses_matches);
    assert!(!estimate(&must_compile("key.startsWith('plan:')")).uses_matches);
}

// --- the budgets ---

#[test]
fn the_budgets_are_s5s_suggested_limits() {
    assert_eq!(
        (MAX_STEPS_PER_TASK, MAX_STEPS_PER_PASS),
        (10_000, 2_000_000)
    );
    assert_eq!(
        Budgets::default(),
        Budgets {
            per_task: 10_000,
            per_pass: 2_000_000
        }
    );
}

fn hostile() -> TaskVars {
    let mut task = task("hostile");
    task.labels = names("l", MAX_LIST_ENTRIES);
    TaskVars::bind(&task, "task", None).expect("a list at its cap binds")
}

fn tame(count: usize) -> TaskVars {
    let mut task = task("tame");
    task.labels = names("l", count);
    TaskVars::bind(&task, "task", None).unwrap()
}

#[test]
fn a_hostile_task_stops_at_the_per_task_budget_and_the_pass_goes_on_to_the_next_task() {
    let compiled = must_compile("labels.all(a, labels.exists(b, a == b))");
    let tasks = [tame(2), hostile(), tame(3)];
    let outcome = evaluate_pass(&compiled, &tasks, &Budgets::default());
    assert_eq!(outcome.over_task_budget, 1);
    assert_eq!(
        outcome.matched,
        [0, 2],
        "the tasks around it were still tested"
    );
    assert_eq!(outcome.error_count(), 0);
    assert!(!outcome.pass_budget_exhausted);
    // The hostile task used its budget and a few nodes past it; the others a few dozen.
    assert!(
        (10_001..10_400).contains(&outcome.steps),
        "{}",
        outcome.steps
    );
}

#[test]
fn a_pass_over_many_cheap_tasks_stops_at_the_pass_budget_and_counts_the_tasks_it_did_not_reach() {
    let compiled = must_compile("status == 'ready'");
    let tasks: Vec<TaskVars> = (0..500).map(|_| vars("t")).collect();
    let budgets = Budgets {
        per_task: 100,
        per_pass: 1_000,
    };
    let outcome = evaluate_pass(&compiled, &tasks, &budgets);
    assert!(outcome.pass_budget_exhausted);
    assert!(outcome.not_reached > 100, "{}", outcome.not_reached);
    assert_eq!(
        outcome.matched.len() + outcome.not_reached,
        500,
        "every task is matched or not reached"
    );
    assert_eq!(
        outcome.matched,
        (0..outcome.matched.len()).collect::<Vec<_>>(),
        "it stopped in order"
    );
    assert_eq!(
        outcome.over_task_budget, 0,
        "the stop is the pass's, not a task's"
    );
    assert!((1_000..1_100).contains(&outcome.steps), "{}", outcome.steps);
}

#[test]
fn a_pass_whose_budget_is_spent_exactly_between_two_tasks_counts_the_rest_as_not_reached() {
    // Three steps for each task: ten of them use exactly 30.
    let tasks: Vec<TaskVars> = (0..20).map(|_| vars("t")).collect();
    let budgets = Budgets {
        per_task: 100,
        per_pass: 30,
    };
    let outcome = evaluate_pass(&must_compile("status == 'ready'"), &tasks, &budgets);
    assert_eq!(
        (outcome.matched.len(), outcome.not_reached, outcome.steps),
        (10, 10, 30)
    );
    assert!(outcome.pass_budget_exhausted);
}

#[test]
fn a_pass_never_runs_on_even_when_the_task_budget_is_larger_than_the_pass_budget() {
    let compiled = must_compile("labels.all(a, labels.exists(b, a == b))");
    let budgets = Budgets {
        per_task: 10_000,
        per_pass: 1_000,
    };
    let outcome = evaluate_pass(&compiled, &[hostile(), tame(1)], &budgets);
    assert!(outcome.pass_budget_exhausted);
    assert_eq!(
        outcome.not_reached, 2,
        "the task it stopped in, and the one after"
    );
    assert_eq!(outcome.over_task_budget, 0);
    assert!((1_000..1_200).contains(&outcome.steps), "{}", outcome.steps);
}

#[test]
fn a_pass_that_finishes_within_its_budget_does_not_say_it_ran_out() {
    let outcome = evaluate_pass(
        &must_compile("status == 'ready'"),
        &[vars("a"), vars("b")],
        &Budgets::default(),
    );
    assert_eq!(
        (
            outcome.matched.len(),
            outcome.not_reached,
            outcome.pass_budget_exhausted
        ),
        (2, 0, false)
    );
    assert_eq!(outcome.steps, 6, "three nodes for each task");
}

#[test]
fn the_step_counter_starts_from_zero_for_each_task_and_for_each_pass() {
    let compiled = must_compile("status == 'ready'");
    let budgets = Budgets {
        per_task: 5,
        per_pass: 1_000,
    };
    // Three steps a task: a counter that carried over would pass 5 at the second task.
    let tasks: Vec<TaskVars> = (0..10).map(|_| vars("t")).collect();
    let first = evaluate_pass(&compiled, &tasks, &budgets);
    assert_eq!((first.matched.len(), first.over_task_budget), (10, 0));
    // A pass after one that was stopped starts from zero as well.
    let stopped = evaluate_pass(
        &must_compile(NESTED[0]),
        &[hostile()],
        &Budgets {
            per_task: 50,
            per_pass: 40,
        },
    );
    assert!(stopped.pass_budget_exhausted);
    let after = evaluate_pass(&compiled, &tasks, &budgets);
    assert_eq!((after.matched.len(), after.steps), (10, 30));
}

#[test]
fn a_pass_runs_on_its_own_thread_and_leaves_the_callers_counter_alone() {
    cel::steps::set_limit(1_000_000);
    let stopped = evaluate_pass(
        &must_compile(NESTED[0]),
        &[hostile()],
        &Budgets {
            per_task: 50,
            per_pass: 40,
        },
    );
    assert!(stopped.pass_budget_exhausted);
    assert_eq!(
        cel::steps::used(),
        0,
        "the steps were counted on the pass's thread"
    );
    let program = Program::compile("[1, 2, 3].all(a, [1, 2, 3].exists(b, a == b))").unwrap();
    assert_eq!(
        program.execute(&cel::Context::default()),
        Ok(cel::Value::Bool(true))
    );
    assert!(cel::steps::used() > 0 && cel::steps::used() < 1_000_000);
}

// --- errors counted by reason ---

#[test]
fn errors_are_counted_by_reason_and_not_hidden() {
    let compiled = must_compile("fields.n > 3");
    let make = |fields: serde_json::Value| {
        let mut task = task("t");
        if let serde_json::Value::Object(map) = fields {
            task.fields.extend(map);
        }
        TaskVars::bind(&task, "", None).unwrap()
    };
    let tasks = [
        make(json!({})),
        make(json!({ "n": "text" })),
        make(json!({})),
        make(json!({ "n": 5 })),
        make(json!({})),
        make(json!({ "n": 1 })),
    ];
    let outcome = evaluate_pass(&compiled, &tasks, &unlimited());
    assert_eq!(outcome.matched, [3]);
    assert_eq!(outcome.error_count(), 4);
    assert_eq!(outcome.errors.len(), 2, "{:?}", outcome.errors);
    assert_eq!(outcome.errors.get("No such key: n"), Some(&3));
}

#[test]
fn an_expression_that_gives_no_boolean_is_counted_for_each_task() {
    let outcome = evaluate_pass(
        &must_compile("title"),
        &[vars("a"), vars("b")],
        &unlimited(),
    );
    assert!(outcome.matched.is_empty());
    assert_eq!(
        outcome
            .errors
            .get("the expression gave a string, not true or false"),
        Some(&2),
        "{:?}",
        outcome.errors
    );
}

#[test]
fn a_reason_that_quotes_a_long_value_is_cut() {
    let mut task = task("t");
    task.fields
        .insert("n".into(), json!("x".repeat(MAX_STRING_BYTES)));
    let bound = TaskVars::bind(&task, "", None).unwrap();
    let outcome = evaluate_pass(&must_compile("fields.n > 3"), &[bound], &unlimited());
    let reason = outcome.errors.keys().next().expect("an error");
    assert!(reason.len() < 200, "{}", reason.len());
}

// --- the budget error ---

#[test]
fn the_budget_error_is_recognised_by_one_function_and_the_message_it_matches_is_pinned() {
    cel::steps::set_limit(1);
    let error = Program::compile("1 + 1 + 1")
        .unwrap()
        .execute(&cel::Context::default())
        .unwrap_err();
    cel::steps::set_limit(u64::MAX);
    // If a change to the vendored crate moves these words, this test fails, and no view silently stops counting.
    assert_eq!(
        error.to_string(),
        "Error executing function 'cost': step limit exceeded"
    );
    assert!(is_budget_error(&error));
}

#[test]
fn another_error_is_not_a_budget_error() {
    let type_error = Program::compile("1 + 'a'")
        .unwrap()
        .execute(&cel::Context::default())
        .unwrap_err();
    assert!(!is_budget_error(&type_error), "{type_error}");
    assert!(!is_budget_error(&ExecutionError::function_error(
        "cost",
        "something else"
    )));
    assert!(!is_budget_error(&ExecutionError::function_error(
        "size",
        "step limit exceeded"
    )));
}
