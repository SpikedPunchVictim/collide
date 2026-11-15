//! The vendored `cel` with its step counter (0.9, CEL runs on untrusted task data): a runaway expression stops with
//! a step-budget error, and an ordinary expression answers as the unpatched crate does, within the budget.
//!
//! The counter is thread-local, so each test sets its budget on the thread that evaluates.

use cel::{Context, Env, Value};
use std::sync::Arc;
use std::sync::mpsc;
use std::time::Duration;

/// Longer than the patched crate needs to stop (well under a second), far shorter than the unpatched crate needs to
/// finish (hours), so an unpatched build fails the test instead of hanging it.
const RUNAWAY_DEADLINE: Duration = Duration::from_secs(30);

/// A triple-nested macro: the shape S5 measured as not finishing in 30 s on the stock crate.
const RUNAWAY: &str =
    "items.filter(a, items.exists(b, items.exists(c, a == c && b == c && false)))";

fn eval(source: &str, items: &[i64]) -> Result<Value, cel::ExecutionError> {
    let program = Env::stdlib().compile(source).expect("compiles");
    let mut context = Context::with_env(Arc::new(Env::stdlib()));
    context.add_variable_from_value("items", items.to_vec());
    program.execute(&context)
}

#[test]
fn a_runaway_expression_stops_with_a_step_budget_error() {
    let (sender, receiver) = mpsc::channel();
    // The evaluation runs on its own thread so that an unpatched build fails on the deadline. That thread is left
    // running when it does not stop; the test binary exits without it.
    std::thread::spawn(move || {
        cel::steps::set_limit(1_000_000);
        let items: Vec<i64> = (0..3_000).collect();
        let _ = sender.send(eval(RUNAWAY, &items));
    });
    let result = receiver
        .recv_timeout(RUNAWAY_DEADLINE)
        .expect("the runaway expression stopped within the deadline");
    let error = result.expect_err("the runaway expression is an error, not an answer");
    assert!(error.to_string().contains("step limit exceeded"), "{error}");
}

#[test]
fn the_steps_a_stopped_expression_used_are_just_over_the_budget() {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        cel::steps::set_limit(5_000);
        let items: Vec<i64> = (0..3_000).collect();
        let _ = eval(RUNAWAY, &items);
        let _ = sender.send(cel::steps::used());
    });
    let used = receiver.recv_timeout(RUNAWAY_DEADLINE).expect("it stopped");
    // The error unwinds through a few more nodes, each of which still counts.
    assert!((5_001..5_100).contains(&used), "{used}");
}

/// The ordinary expressions and the answers the unpatched crate gives for them (checked against stock cel 0.15.0).
fn ordinary_cases() -> Vec<(&'static str, Value)> {
    vec![
        ("items.size() == 5", Value::Bool(true)),
        ("items.exists(i, i > 3)", Value::Bool(true)),
        ("items.all(i, i >= 1)", Value::Bool(true)),
        ("items.map(i, i * 2)", Value::from(vec![2_i64, 4, 6, 8, 10])),
        (
            "items.filter(i, i % 2 == 1)",
            Value::from(vec![1_i64, 3, 5]),
        ),
        (
            "items.filter(a, items.exists(b, b == a + 1))",
            Value::from(vec![1_i64, 2, 3, 4]),
        ),
        ("'ready' == 'ready' && !(3 in [1, 2])", Value::Bool(true)),
        ("'plan:ep-07'.startsWith('plan:')", Value::Bool(true)),
        ("'abc123'.matches('^[a-z]+[0-9]+$')", Value::Bool(true)),
        ("size(items) > 2 ? 'big' : 'small'", Value::from("big")),
    ]
}

#[test]
fn ordinary_expressions_answer_as_the_unpatched_crate_within_the_budget() {
    cel::steps::set_limit(10_000);
    for (source, expected) in ordinary_cases() {
        let answer = eval(source, &[1, 2, 3, 4, 5]).unwrap_or_else(|e| panic!("{source}: {e}"));
        assert_eq!(answer, expected, "{source}");
    }
    assert!(
        cel::steps::used() > 0 && cel::steps::used() < 10_000,
        "{}",
        cel::steps::used()
    );
}

#[test]
fn with_no_budget_set_nothing_is_limited() {
    // A fresh thread has the default budget: unlimited.
    let answer = std::thread::spawn(|| {
        let items: Vec<i64> = (0..100).collect();
        eval(
            "items.filter(a, items.exists(b, items.exists(c, a == c && b == c)))",
            &items,
        )
    })
    .join()
    .expect("no panic")
    .expect("no step budget by default");
    assert_eq!(answer, Value::from((0..100_i64).collect::<Vec<_>>()));
}
