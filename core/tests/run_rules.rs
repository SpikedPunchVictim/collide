//! Tests for the run-state rules (ts-0038-ng): recording a hook result and the `pending` poll, the answer of an
//! `integrate` and the `integrating` hold, and recovery from the event log after a restart (0.2, 0.4, 0.5, 0.7).

use collide_core::duration::Duration;
use collide_core::events::{Event, EventBody, StepKind, StepResult};
use collide_core::hook::{HookKind, HookResult, HookStatus};
use collide_core::non_empty::NonEmpty;
use collide_core::run_rules::{
    AfterIntegrate, CheckWorld, EntryKey, Finding, HookEntry, IntegrateAnswer, IntegrateError,
    NextPoll, OutcomeUnknown, Recorded, Settled, WaitTiming, apply_integrate_answer, poll_delay,
    record_hook_result, recover, settle,
};
use collide_core::state::{PauseCause, RunState};
use serde_json::{Value, json};
use std::collections::BTreeMap;

fn secs(n: u64) -> Duration {
    Duration::from_secs(n)
}

fn non_empty(text: &str) -> NonEmpty {
    NonEmpty::new(text).unwrap()
}

// --- Recording a hook result and the pending poll ---

fn key(hook: &str, phase: &str, round: u32) -> EntryKey {
    EntryKey {
        hook: hook.to_string(),
        phase: phase.to_string(),
        round,
        instance: None,
    }
}

fn answer(status: HookStatus, message: &str, retry_after: Option<u32>) -> HookResult {
    HookResult {
        status,
        message: message.to_string(),
        outputs: BTreeMap::new(),
        retry_after,
    }
}

fn pending(message: &str) -> HookResult {
    answer(HookStatus::Pending, message, None)
}

/// A `wait` phase that started at uptime 100 s, polled every 30 s, giving up after 1 h.
fn ci_wait() -> WaitTiming {
    WaitTiming {
        interval: secs(30),
        timeout: secs(3600),
        started: secs(100),
    }
}

/// Records `result` for the `demo/ci` wait hook of phase `ci`, round 1, at uptime `now` seconds.
fn record_wait(entries: &[HookEntry], now: u64, result: HookResult) -> Recorded {
    record_hook_result(
        entries,
        key("demo/ci", "ci", 1),
        HookKind::Wait,
        Some(&ci_wait()),
        secs(now),
        result,
    )
}

#[test]
fn a_wait_hook_answering_pending_three_times_leaves_one_entry() {
    let first = record_wait(&[], 100, pending("0 of 3 checks"));
    let second = record_wait(&first.entries, 130, pending("1 of 3 checks"));
    let third = record_wait(&second.entries, 160, pending("2 of 3 checks"));
    assert_eq!(third.entries.len(), 1);
    assert_eq!(third.entries[0].result.message, "2 of 3 checks");
}

#[test]
fn the_non_pending_result_that_ends_the_polling_is_the_entry_kept() {
    let first = record_wait(&[], 100, pending("0 of 3 checks"));
    let second = record_wait(&first.entries, 130, pending("1 of 3 checks"));
    let done = record_wait(
        &second.entries,
        160,
        answer(HookStatus::Ok, "all green", None),
    );
    assert_eq!(done.entries.len(), 1);
    assert_eq!(done.entries[0].result.status, HookStatus::Ok);
    assert_eq!(done.entries[0].result.message, "all green");
}

#[test]
fn a_pending_answer_never_replaces_a_settled_entry_of_the_same_key() {
    let settled = record_wait(&[], 100, answer(HookStatus::Ok, "green", None));
    let again = record_wait(&settled.entries, 200, pending("rerun started"));
    assert_eq!(again.entries.len(), 2);
    assert_eq!(again.entries[0].result.status, HookStatus::Ok);
    assert_eq!(again.entries[1].result.status, HookStatus::Pending);
}

#[test]
fn pending_entries_of_another_phase_round_hook_or_instance_stay_separate() {
    let base = record_wait(&[], 100, pending("ci"));
    let mut other_instance = key("demo/ci", "ci", 1);
    other_instance.instance = Some("b".to_string());
    let others = [
        key("demo/ci", "ci", 2),
        key("demo/ci", "deploy", 1),
        key("demo/other", "ci", 1),
        other_instance,
    ];
    let mut entries = base.entries;
    for other in others {
        entries = record_hook_result(
            &entries,
            other,
            HookKind::Wait,
            Some(&ci_wait()),
            secs(130),
            pending("elsewhere"),
        )
        .entries;
    }
    assert_eq!(entries.len(), 5);
}

#[test]
fn a_pending_replaces_in_place_and_keeps_the_order_of_the_entries() {
    let gate = record_hook_result(
        &[],
        key("demo/gate", "build", 1),
        HookKind::Gate,
        None,
        secs(100),
        answer(HookStatus::Ok, "", None),
    );
    let waiting = record_wait(&gate.entries, 100, pending("a"));
    let after = record_hook_result(
        &waiting.entries,
        key("demo/gate", "fix", 1),
        HookKind::Gate,
        None,
        secs(110),
        answer(HookStatus::Fail, "", None),
    );
    let polled = record_wait(&after.entries, 130, pending("b"));
    let hooks: Vec<_> = polled.entries.iter().map(|e| e.key.hook.as_str()).collect();
    assert_eq!(hooks, ["demo/gate", "demo/ci", "demo/gate"]);
    assert_eq!(polled.entries[1].result.message, "b");
}

#[test]
fn a_pending_from_a_hook_that_is_not_wait_or_a_pull_request_integrate_is_an_error() {
    for kind in [
        HookKind::Workspace,
        HookKind::Record,
        HookKind::Integrate,
        HookKind::Sync,
        HookKind::Gate,
        HookKind::Script,
        HookKind::Decision,
        HookKind::Point,
    ] {
        let recorded = record_hook_result(
            &[],
            key("demo/hook", "build", 2),
            kind,
            None,
            secs(100),
            pending("not yet"),
        );
        let result = &recorded.result;
        assert_eq!(result.status, HookStatus::Error, "{kind:?}");
        assert!(
            result.message.contains("demo/hook")
                && result.message.contains("build")
                && result.message.contains("round 2")
                && result.message.contains(&format!("{kind:?}")),
            "{kind:?}: {}",
            result.message
        );
        assert_eq!(recorded.next, NextPoll::None, "{kind:?}");
        assert_eq!(recorded.entries.len(), 1);
        assert_eq!(recorded.entries[0].result.status, HookStatus::Error);
    }
}

#[test]
fn a_pull_request_integrate_may_answer_pending_and_is_not_polled_by_this_rule() {
    let recorded = record_hook_result(
        &[],
        key("github/pull-request", "accept", 1),
        HookKind::IntegratePullRequest,
        None,
        secs(100),
        pending("PR open"),
    );
    assert_eq!(recorded.result.status, HookStatus::Pending);
    assert_eq!(recorded.next, NextPoll::None);
}

#[test]
fn the_poll_delay_is_the_larger_of_retry_after_and_the_interval_and_never_under_one_second() {
    // (interval in seconds, retryAfter hint, delay in seconds)
    let table: [(u64, Option<u32>, u64); 9] = [
        (30, None, 30),
        (30, Some(5), 30),
        (30, Some(30), 30),
        (30, Some(90), 90),
        (30, Some(0), 30),
        (0, None, 1),
        (0, Some(0), 1),
        (0, Some(5), 5),
        (1, Some(0), 1),
    ];
    for (interval, hint, delay) in table {
        assert_eq!(
            poll_delay(secs(interval), hint),
            secs(delay),
            "interval {interval}s, retryAfter {hint:?}"
        );
    }
}

#[test]
fn the_next_poll_is_the_delay_after_now() {
    let recorded = record_wait(&[], 160, answer(HookStatus::Pending, "waiting", Some(120)));
    assert_eq!(recorded.next, NextPoll::At(secs(280)));
    let no_hint = record_wait(&[], 160, pending("waiting"));
    assert_eq!(no_hint.next, NextPoll::At(secs(190)));
}

#[test]
fn a_poll_that_would_fall_past_the_timeout_is_scheduled_at_the_timeout() {
    // The deadline is 100 + 3600 = 3700 s.
    let recorded = record_wait(&[], 3690, pending("waiting"));
    assert_eq!(recorded.next, NextPoll::At(secs(3700)));
}

#[test]
fn a_wait_at_its_timeout_while_pending_is_an_error_with_the_last_message() {
    let before = record_wait(&[], 3699, pending("2 of 3 checks"));
    assert_eq!(before.result.status, HookStatus::Pending);

    let at = record_wait(&before.entries, 3700, pending("2 of 3 checks"));
    assert_eq!(at.result.status, HookStatus::Error);
    assert!(
        at.result.message.contains("2 of 3 checks"),
        "{}",
        at.result.message
    );
    assert!(
        at.result.message.contains("demo/ci"),
        "{}",
        at.result.message
    );
    assert!(at.result.message.contains("1h"), "{}", at.result.message);
    assert_eq!(at.next, NextPoll::None);
    assert_eq!(at.entries.len(), 1);
    assert_eq!(at.entries[0].result.status, HookStatus::Error);
}

#[test]
fn a_wait_past_its_timeout_while_pending_is_an_error_too() {
    let late = record_wait(&[], 9000, pending("slow"));
    assert_eq!(late.result.status, HookStatus::Error);
}

#[test]
fn a_wait_that_answers_ok_at_its_timeout_is_still_ok() {
    let ok = record_wait(&[], 3700, answer(HookStatus::Ok, "just in time", None));
    assert_eq!(ok.result.status, HookStatus::Ok);
}

#[test]
fn an_uptime_reading_before_the_start_does_not_count_as_elapsed_time() {
    let early = record_wait(&[], 10, pending("clock reading before the start"));
    assert_eq!(early.result.status, HookStatus::Pending);
}

#[test]
fn a_pending_wait_with_no_timing_is_an_error_naming_the_hook() {
    let recorded = record_hook_result(
        &[],
        key("demo/ci", "ci", 1),
        HookKind::Wait,
        None,
        secs(100),
        pending("waiting"),
    );
    assert_eq!(recorded.result.status, HookStatus::Error);
    assert!(
        recorded.result.message.contains("demo/ci"),
        "{}",
        recorded.result.message
    );
}

#[test]
fn a_result_that_is_not_pending_has_no_next_poll_and_ignores_retry_after() {
    for status in [
        HookStatus::Ok,
        HookStatus::Fail,
        HookStatus::Unverified,
        HookStatus::Refused,
        HookStatus::Error,
    ] {
        let recorded = record_wait(&[], 130, answer(status, "x", Some(600)));
        assert_eq!(recorded.next, NextPoll::None, "{status:?}");
        assert_eq!(recorded.result.status, status);
    }
}

// --- An integrate answer and the integrating hold ---

fn merged(commit: &str) -> IntegrateAnswer {
    IntegrateAnswer::Merged {
        commit: non_empty(commit),
    }
}

fn pr_pending() -> IntegrateAnswer {
    IntegrateAnswer::Pending {
        url: non_empty("https://example.test/pr/7"),
        head: non_empty("b2c3d4"),
    }
}

fn ended() -> RunState {
    RunState::Ended {
        outcome: non_empty("done"),
    }
}

const BOTH: [AfterIntegrate; 2] = [AfterIntegrate::RunAfterIntegrate, AfterIntegrate::SourceOps];

#[test]
fn an_integrate_of_pending_leaves_the_run_integrating_and_holds_the_ops_back() {
    let step = apply_integrate_answer(&ended(), &pr_pending()).unwrap();
    assert_eq!(step.run, Some(RunState::Integrating {}));
    assert!(step.release.is_empty());
    assert_eq!(step.held, BOTH);
}

#[test]
fn a_later_merged_releases_what_pending_held_back() {
    let held = apply_integrate_answer(&ended(), &pr_pending()).unwrap();
    let run = held.run.unwrap();
    let released = apply_integrate_answer(&run, &merged("abc123")).unwrap();
    assert_eq!(released.release, BOTH);
    assert!(released.held.is_empty());
    assert_eq!(released.run, None, "the run leaves `integrating`");
}

#[test]
fn a_pending_answered_again_keeps_the_run_integrating_and_the_hold() {
    let step = apply_integrate_answer(&RunState::Integrating {}, &pr_pending()).unwrap();
    assert_eq!(step.run, Some(RunState::Integrating {}));
    assert_eq!(step.held, BOTH);
}

#[test]
fn a_merged_answer_straight_away_runs_after_integrate_before_the_source_ops() {
    let step = apply_integrate_answer(&ended(), &merged("abc123")).unwrap();
    assert_eq!(step.run, None);
    assert_eq!(
        step.release,
        [AfterIntegrate::RunAfterIntegrate, AfterIntegrate::SourceOps]
    );
    assert!(step.held.is_empty());
}

#[test]
fn an_integrate_answer_for_a_run_that_is_not_finished_is_refused_naming_the_state() {
    let unfinished = [
        RunState::Running {},
        RunState::Paused {
            cause: PauseCause::Disk,
        },
        RunState::Failed {},
        RunState::Stopped {},
    ];
    for state in unfinished {
        let error = apply_integrate_answer(&state, &merged("abc123")).unwrap_err();
        assert!(matches!(error, IntegrateError::RunNotFinished { .. }));
        let text = error.to_string();
        assert!(text.contains("integrate"), "{text}");
        assert!(
            text.contains(match state {
                RunState::Running {} => "running",
                RunState::Paused { .. } => "paused",
                RunState::Failed {} => "failed",
                _ => "stopped",
            }),
            "{text}"
        );
    }
}

fn result_with(status: HookStatus, outputs: Value) -> HookResult {
    HookResult {
        status,
        message: String::new(),
        outputs: serde_json::from_value(outputs).unwrap(),
        retry_after: None,
    }
}

#[test]
fn an_ok_result_with_a_commit_is_merged() {
    let result = result_with(HookStatus::Ok, json!({ "commit": "abc123" }));
    assert_eq!(
        IntegrateAnswer::from_result(&result).unwrap(),
        merged("abc123")
    );
}

#[test]
fn a_pending_result_with_a_url_and_a_head_is_pending() {
    let result = result_with(
        HookStatus::Pending,
        json!({ "url": "https://example.test/pr/7", "head": "b2c3d4" }),
    );
    assert_eq!(IntegrateAnswer::from_result(&result).unwrap(), pr_pending());
}

#[test]
fn an_answer_missing_or_blanking_an_output_names_it() {
    let cases = [
        (HookStatus::Ok, json!({}), "commit"),
        (HookStatus::Ok, json!({ "commit": "" }), "commit"),
        (HookStatus::Ok, json!({ "commit": 7 }), "commit"),
        (HookStatus::Pending, json!({ "head": "b2" }), "url"),
        (HookStatus::Pending, json!({ "url": "u" }), "head"),
    ];
    for (status, outputs, name) in cases {
        let error =
            IntegrateAnswer::from_result(&result_with(status, outputs.clone())).unwrap_err();
        assert!(
            matches!(&error, IntegrateError::BadOutput { name: n, .. } if n == name),
            "{outputs}: {error:?}"
        );
        assert!(error.to_string().contains(name), "{error}");
    }
}

#[test]
fn a_result_that_is_neither_ok_nor_pending_is_not_an_answer() {
    for status in [
        HookStatus::Fail,
        HookStatus::Unverified,
        HookStatus::Refused,
        HookStatus::Error,
    ] {
        let error = IntegrateAnswer::from_result(&result_with(status, json!({}))).unwrap_err();
        assert!(
            matches!(error, IntegrateError::NotAnAnswer { status: s } if s == status),
            "{status:?}"
        );
    }
}

// --- Recovery from the event log ---

fn event(seq: u64, body: Value) -> Event {
    serde_json::from_value(json!({
        "seq": seq,
        "wall_ms": 1_700_000_000_000u64 + seq,
        "actor": { "kind": "workflow", "run": "r-9", "phase": "accept" },
        "experiment": null,
        "body": body
    }))
    .unwrap_or_else(|error| panic!("{error}"))
}

fn intent(seq: u64, step: &str, kind: &str, action: &str) -> Event {
    event(
        seq,
        json!({ "event": "step.intent", "step": step, "kind": kind, "action": action }),
    )
}

fn outcome(seq: u64, step: &str, kind: &str, result: &str) -> Event {
    event(
        seq,
        json!({ "event": "step.outcome", "step": step, "kind": kind, "result": result, "detail": "" }),
    )
}

fn transition(seq: u64) -> Event {
    event(
        seq,
        json!({ "event": "run.transition", "run": "r-9", "from": null, "to": { "state": "running" } }),
    )
}

#[test]
fn an_intent_with_no_outcome_yields_a_check_for_that_step_and_nothing_else() {
    let log = [
        transition(1),
        intent(2, "s-1", "integrate", "merge branch task/ts-1 into main"),
    ];
    assert_eq!(
        recover(&log),
        [CheckWorld {
            step: "s-1".to_string(),
            kind: StepKind::Integrate,
            action: "merge branch task/ts-1 into main".to_string(),
            intent_seq: 2,
        }]
    );
}

#[test]
fn an_intent_with_its_outcome_needs_no_check() {
    for result in ["succeeded", "failed"] {
        let log = [
            intent(1, "s-1", "integrate", "merge"),
            outcome(2, "s-1", "integrate", result),
        ];
        assert!(recover(&log).is_empty(), "{result}");
    }
}

#[test]
fn only_the_steps_left_open_are_checked_in_the_order_of_their_intents() {
    let log = [
        intent(1, "s-1", "source_op", "set status done"),
        intent(2, "s-2", "remote_write", "set label done"),
        outcome(3, "s-1", "source_op", "succeeded"),
        intent(4, "s-3", "after_integrate_hook", "run notify"),
    ];
    let steps: Vec<_> = recover(&log).into_iter().map(|c| c.step).collect();
    assert_eq!(steps, ["s-2", "s-3"]);
}

#[test]
fn an_outcome_closes_only_the_intent_of_its_own_step_and_kind() {
    let log = [
        intent(1, "s-1", "integrate", "merge"),
        outcome(2, "s-1", "source_op", "succeeded"),
        outcome(3, "s-2", "integrate", "succeeded"),
    ];
    let open = recover(&log);
    assert_eq!(open.len(), 1);
    assert_eq!(open[0].kind, StepKind::Integrate);
}

#[test]
fn a_step_written_again_after_its_check_is_open_with_its_newest_intent() {
    let log = [
        intent(1, "s-1", "integrate", "merge"),
        intent(
            5,
            "s-1",
            "integrate",
            "merge again after the check found nothing",
        ),
    ];
    let open = recover(&log);
    assert_eq!(open.len(), 1);
    assert_eq!(open[0].intent_seq, 5);
    assert_eq!(open[0].action, "merge again after the check found nothing");
}

#[test]
fn an_outcome_with_no_intent_is_ignored() {
    assert!(recover(&[outcome(1, "s-1", "integrate", "succeeded")]).is_empty());
}

#[test]
fn every_kind_of_side_effect_step_left_open_is_a_check() {
    for kind in [
        "integrate",
        "source_op",
        "after_integrate_hook",
        "remote_write",
    ] {
        assert_eq!(recover(&[intent(1, "s-1", kind, "x")]).len(), 1, "{kind}");
    }
}

fn open_merge() -> CheckWorld {
    recover(&[intent(7, "s-1", "integrate", "merge task/ts-1 into main")])
        .pop()
        .unwrap()
}

#[test]
fn a_merge_whose_change_is_already_at_the_target_settles_as_a_success() {
    let settled = settle(
        &open_merge(),
        Finding::Done {
            detail: "abc123 is already an ancestor of main".to_string(),
        },
    );
    assert_eq!(
        settled,
        Settled::Succeeded {
            outcome: Box::new(EventBody::StepOutcome {
                step: "s-1".to_string(),
                kind: StepKind::Integrate,
                result: StepResult::Succeeded,
                detail: "abc123 is already an ancestor of main".to_string(),
            })
        }
    );
}

#[test]
fn a_check_that_finds_nothing_done_runs_the_step_again() {
    assert_eq!(settle(&open_merge(), Finding::NotDone), Settled::RunStep);
}

#[test]
fn an_inconclusive_check_yields_an_outcome_unknown_queue_item() {
    let settled = settle(
        &open_merge(),
        Finding::Inconclusive {
            reason: "the remote did not answer".to_string(),
        },
    );
    let Settled::OutcomeUnknown(item) = settled else {
        panic!("expected outcome unknown, got {settled:?}");
    };
    assert_eq!(
        item,
        OutcomeUnknown {
            step: "s-1".to_string(),
            kind: StepKind::Integrate,
            action: "merge task/ts-1 into main".to_string(),
            reason: "the remote did not answer".to_string(),
        }
    );
    let text = item.to_string();
    for part in [
        "outcome unknown",
        "s-1",
        "merge task/ts-1 into main",
        "the remote did not answer",
    ] {
        assert!(text.contains(part), "{text}");
    }
}

#[test]
fn the_inconclusive_log_ends_in_the_queue_never_in_a_repeat() {
    let log = [
        intent(1, "s-1", "remote_write", "set label done"),
        intent(2, "s-2", "integrate", "open pull request"),
        outcome(3, "s-2", "integrate", "succeeded"),
    ];
    let checks = recover(&log);
    assert_eq!(checks.len(), 1);
    let settled = settle(
        &checks[0],
        Finding::Inconclusive {
            reason: "the label API returned 502".to_string(),
        },
    );
    assert!(matches!(settled, Settled::OutcomeUnknown(_)), "{settled:?}");
}
