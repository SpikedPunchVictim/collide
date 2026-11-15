//! The fake hook runner (ts-0022-mc): a `HookRunner` that returns scripted results by hook name, so an engine test
//! drives a gate, a wait or a record with no script and no process. Like the real runner it returns the effective
//! result ([`collide_core::hook::effective_result`]), never a raw one.

use collide_core::hook::{HookKind, HookResult, HookStatus};
use collide_core::ports::{HookRunError, HookRunner};
use collide_core::testing::{FakeHookRunner, hook_request, hook_result};
use serde_json::json;

fn status_of(runner: &FakeHookRunner, hook: &str, kind: HookKind) -> HookStatus {
    runner.run(&hook_request(hook, kind)).unwrap().status
}

#[test]
fn a_scripted_hook_returns_its_result() {
    let runner =
        FakeHookRunner::new().on("demo/lint", [Ok(hook_result(HookStatus::Fail, "3 errors"))]);
    let result = runner
        .run(&hook_request("demo/lint", HookKind::Gate))
        .unwrap();
    assert_eq!(result.status, HookStatus::Fail);
    assert_eq!(result.message, "3 errors");
}

#[test]
fn results_come_in_the_order_scripted_and_the_last_one_repeats() {
    let runner = FakeHookRunner::new().on(
        "demo/ci",
        [
            Ok(hook_result(HookStatus::Pending, "queued")),
            Ok(hook_result(HookStatus::Pending, "running")),
            Ok(hook_result(HookStatus::Ok, "green")),
        ],
    );
    let messages: Vec<String> = (0..5)
        .map(|_| {
            runner
                .run(&hook_request("demo/ci", HookKind::Wait))
                .unwrap()
                .message
        })
        .collect();
    assert_eq!(
        messages,
        vec!["queued", "running", "green", "green", "green"]
    );
}

#[test]
fn each_hook_has_its_own_script() {
    let runner = FakeHookRunner::new()
        .on("demo/a", [Ok(hook_result(HookStatus::Ok, "a"))])
        .on("demo/b", [Ok(hook_result(HookStatus::Fail, "b"))]);
    assert_eq!(
        status_of(&runner, "demo/b", HookKind::Gate),
        HookStatus::Fail
    );
    assert_eq!(status_of(&runner, "demo/a", HookKind::Gate), HookStatus::Ok);
}

#[test]
fn a_scripted_run_error_is_returned_as_the_error() {
    let runner = FakeHookRunner::new().on(
        "demo/slow",
        [Err(HookRunError::NotStarted {
            message: "no such script".into(),
        })],
    );
    let error = runner
        .run(&hook_request("demo/slow", HookKind::Script))
        .unwrap_err();
    assert_eq!(
        error,
        HookRunError::NotStarted {
            message: "no such script".into()
        }
    );
    let stopped = FakeHookRunner::new().on("demo/slow", [Err(HookRunError::Stopped)]);
    assert_eq!(
        stopped
            .run(&hook_request("demo/slow", HookKind::Script))
            .unwrap_err(),
        HookRunError::Stopped
    );
}

#[test]
fn a_hook_with_no_script_did_not_start_and_the_message_names_it() {
    let error = FakeHookRunner::new()
        .run(&hook_request("demo/unscripted", HookKind::Gate))
        .unwrap_err();
    let HookRunError::NotStarted { message } = error else {
        panic!("expected NotStarted")
    };
    assert!(message.contains("demo/unscripted"), "{message}");
}

#[test]
fn the_outputs_of_a_result_reach_the_caller() {
    let mut scripted = hook_result(HookStatus::Ok, "made");
    scripted.outputs.insert("branch".into(), json!("feature/x"));
    let runner = FakeHookRunner::new().on("demo/create", [Ok(scripted)]);
    let result = runner
        .run(&hook_request("demo/create", HookKind::Workspace))
        .unwrap();
    assert_eq!(result.outputs["branch"], json!("feature/x"));
}

#[test]
fn a_pending_result_from_a_hook_that_may_not_answer_pending_comes_back_as_an_error() {
    let runner =
        FakeHookRunner::new().on("demo/lint", [Ok(hook_result(HookStatus::Pending, "later"))]);
    assert_eq!(
        status_of(&runner, "demo/lint", HookKind::Gate),
        HookStatus::Error
    );
}

#[test]
fn a_pending_result_from_a_wait_hook_stays_pending() {
    let runner =
        FakeHookRunner::new().on("demo/ci", [Ok(hook_result(HookStatus::Pending, "later"))]);
    assert_eq!(
        status_of(&runner, "demo/ci", HookKind::Wait),
        HookStatus::Pending
    );
}

#[test]
fn every_call_is_recorded_in_order_with_its_request() {
    let runner = FakeHookRunner::new()
        .on("demo/a", [Ok(hook_result(HookStatus::Ok, ""))])
        .on("demo/b", [Ok(hook_result(HookStatus::Ok, ""))]);
    runner.run(&hook_request("demo/b", HookKind::Gate)).unwrap();
    runner
        .run(&hook_request("demo/a", HookKind::Script))
        .unwrap();
    let calls = runner.calls();
    let seen: Vec<(&str, HookKind)> = calls
        .iter()
        .map(|c| (c.input.hook.as_str(), c.kind))
        .collect();
    assert_eq!(
        seen,
        vec![("demo/b", HookKind::Gate), ("demo/a", HookKind::Script)]
    );
}

#[test]
fn the_same_script_gives_the_same_results() {
    let run = || {
        let runner = FakeHookRunner::new().on(
            "demo/ci",
            [
                Ok(hook_result(HookStatus::Pending, "1")),
                Ok(hook_result(HookStatus::Ok, "2")),
            ],
        );
        (0..3)
            .map(|_| runner.run(&hook_request("demo/ci", HookKind::Wait)))
            .collect::<Vec<Result<HookResult, HookRunError>>>()
    };
    assert_eq!(run(), run());
}

#[test]
fn the_runner_is_usable_through_a_trait_object() {
    let runner: Box<dyn HookRunner> =
        Box::new(FakeHookRunner::new().on("demo/a", [Ok(hook_result(HookStatus::Ok, "fine"))]));
    assert_eq!(
        runner
            .run(&hook_request("demo/a", HookKind::Gate))
            .unwrap()
            .message,
        "fine"
    );
}
