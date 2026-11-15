//! A `result` that is not the report (ts-0063-rw; 0.3, The stream protocol and Usage and rate limits): an error result,
//! a prompt a hook blocked, and `permission_denials`. The recordings are played through fake Claude under the real
//! shim; the shapes the recordings do not hold (a 500, a result with no cost) are fed by hand.

use super::fake::{assigned, play};
use super::session::{
    answer_nothing, events, exit_with, line, named, report, result_line, session, turn_end,
};
use collide_core::adapter::{AgentSession, PhaseFailure, PhaseOutcome};
use collide_core::agent::{AgentEvent, Money, TurnEndReason, Usage, UsageQuality, UsageScope};
use serde_json::{Value, json};

fn total_cost(amount: f64) -> AgentEvent {
    AgentEvent::Usage {
        usage: Usage {
            scope: UsageScope::RunningTotal,
            input: None,
            output: None,
            cache_read: None,
            cache_write: None,
            cache_write_1h: None,
            cache_write_5m: None,
            thought: None,
            context_used: None,
            context_size: None,
            cost: Some(Money {
                amount,
                currency: "USD".into(),
            }),
            quality: UsageQuality::Exact,
        },
    }
}

fn blocked_line(kind: &str, reason: Option<&str>) -> Vec<u8> {
    let mut outcome = json!({ "kind": kind, "prompt_kept": false });
    if let Some(reason) = reason {
        outcome["reason"] = json!(reason);
    }
    line(json!({
        "type": "system", "subtype": "informational", "level": "warning", "prevent_continuation": true,
        "content": "UserPromptSubmit operation blocked by hook:\nPolicy.", "prompt_submit_outcome": outcome,
    }))
}

fn blocked_result() -> Vec<u8> {
    result_line(json!({
        "result": "UserPromptSubmit operation blocked by hook:\nPolicy.\n\nOriginal prompt: do the task",
        "stop_reason": null, "num_turns": 0, "total_cost_usd": 0.0041,
        "usage": {"input_tokens": 0, "output_tokens": 0},
    }))
}

fn reports_in(events: &[AgentEvent]) -> usize {
    events
        .iter()
        .filter(|event| matches!(event, AgentEvent::FinalReport { .. }))
        .count()
}

// ---- a prompt blocked by a hook ------------------------------------------------------------------------------------

#[test]
fn the_blocked_prompt_recording_ends_the_turn_blocked_with_the_hooks_reason_and_no_report() {
    let mut played = play(
        "blocked-prompt",
        assigned(),
        &["do the task"],
        answer_nothing,
    );
    let all = named(&played);
    let blocked = turn_end(
        TurnEndReason::Blocked {
            reason: Some("Policy: no prompts about the release.".into()),
        },
        false,
        true,
    );
    assert_eq!(all.last(), Some(&blocked));
    assert_eq!(reports_in(&all), 0, "the result text is not the report");
    assert_eq!(
        played.session.finish(&played.exit).outcome,
        PhaseOutcome::Failed {
            cause: PhaseFailure::Blocked {
                reason: Some("Policy: no prompts about the release.".into())
            }
        }
    );
}

#[test]
fn a_prompt_a_hook_stopped_is_blocked_too_with_the_reason_it_gave() {
    let played = play(
        "stopped-prompt",
        assigned(),
        &["do the task"],
        answer_nothing,
    );
    let all = named(&played);
    assert_eq!(
        all.last(),
        Some(&turn_end(
            TurnEndReason::Blocked {
                reason: Some("Stopped by policy.".into())
            },
            false,
            true
        ))
    );
    assert_eq!(reports_in(&all), 0);
}

#[test]
fn both_kinds_of_block_end_the_turn_blocked_and_a_block_with_no_reason_has_none() {
    for (kind, reason) in [
        ("blocked_by_hook", Some("why")),
        ("stopped_by_hook", Some("why")),
        ("blocked_by_hook", None),
        ("stopped_by_hook", None),
    ] {
        let mut session = session();
        assert_eq!(
            events(&session.feed(&blocked_line(kind, reason), 1)),
            [],
            "the line itself is no event"
        );
        let step = session.feed(&blocked_result(), 2);
        assert_eq!(
            events(&step),
            [turn_end(
                TurnEndReason::Blocked {
                    reason: reason.map(String::from)
                },
                false,
                true
            )],
            "{kind} {reason:?}"
        );
    }
}

#[test]
fn the_text_of_a_blocked_result_and_the_original_prompt_in_it_reach_no_event() {
    let mut session = session();
    let mut all = events(&session.feed(&blocked_line("blocked_by_hook", Some("why")), 1));
    all.extend(events(&session.feed(&blocked_result(), 2)));
    let written = serde_json::to_string(&all).unwrap();
    assert!(!written.contains("Original prompt"), "{written}");
    assert!(!written.contains("do the task"), "{written}");
}

#[test]
fn a_block_is_for_its_own_turn_only() {
    let mut session = session();
    session.feed(&blocked_line("blocked_by_hook", Some("why")), 1);
    session.feed(&blocked_result(), 2);
    session.send_turn("a different prompt");
    let step = session.feed(&result_line(json!({"result": "Done."})), 3);
    assert_eq!(
        events(&step),
        [
            report("Done."),
            turn_end(TurnEndReason::EndTurn {}, false, false)
        ]
    );
}

#[test]
fn a_block_the_session_never_saw_is_not_inferred_from_a_result_with_no_turns() {
    // A `/compact` also reports 0 turns (S9): only the informational line says a hook blocked the prompt.
    let mut session = session();
    let step = session.feed(&blocked_result(), 2);
    let Some(AgentEvent::TurnEnd { reason, .. }) = events(&step).pop() else {
        panic!()
    };
    assert_eq!(reason, TurnEndReason::EndTurn {});
}

#[test]
fn an_informational_line_of_a_kind_the_parser_does_not_know_is_other_and_blocks_nothing() {
    let mut session = session();
    let step = session.feed(&blocked_line("a_kind_from_a_later_claude", None), 1);
    assert_eq!(
        events(&step),
        [AgentEvent::Other {
            raw_type: "system/informational".into(),
            raw: None
        }]
    );
    let step = session.feed(&result_line(json!({"result": "Done."})), 2);
    assert_eq!(reports_in(&events(&step)), 1);
}

#[test]
fn a_cancel_makes_a_blocked_turn_cancelled() {
    let mut session = session();
    session.feed(&blocked_line("blocked_by_hook", Some("why")), 1);
    session.cancel();
    let step = session.feed(&blocked_result(), 2);
    let Some(AgentEvent::TurnEnd { reason, .. }) = events(&step).pop() else {
        panic!()
    };
    assert_eq!(reason, TurnEndReason::Cancelled {});
}

// ---- error results -------------------------------------------------------------------------------------------------

#[test]
fn a_429_gives_a_limited_event_with_no_reset_time_the_cost_and_no_report() {
    let mut played = play("limit", assigned(), &["go"], answer_nothing);
    let all = named(&played);
    assert_eq!(reports_in(&all), 0);
    let tail: Vec<AgentEvent> = all.iter().rev().take(3).rev().cloned().collect();
    assert_eq!(
        tail,
        [
            total_cost(0.0041),
            AgentEvent::Limited { reset_at_ms: None },
            turn_end(TurnEndReason::Error { status: Some(429) }, false, false),
        ]
    );
    assert_eq!(
        played.session.finish(&played.exit).outcome,
        PhaseOutcome::Failed {
            cause: PhaseFailure::Api { status: Some(429) }
        }
    );
}

#[test]
fn a_budget_error_carries_its_cost_and_is_no_report() {
    let mut played = play("error-result", assigned(), &["go"], answer_nothing);
    let all = named(&played);
    assert_eq!(reports_in(&all), 0);
    let tail: Vec<AgentEvent> = all.iter().rev().take(2).rev().cloned().collect();
    assert_eq!(
        tail,
        [
            total_cost(0.00353),
            turn_end(TurnEndReason::Error { status: None }, false, false)
        ]
    );
    assert!(!all.iter().any(|e| matches!(e, AgentEvent::Limited { .. })));
    assert_eq!(
        played.session.finish(&played.exit).outcome,
        PhaseOutcome::Failed {
            cause: PhaseFailure::Api { status: None }
        }
    );
}

#[test]
fn a_500_fails_the_phase_with_its_status_carries_its_cost_and_is_no_limit() {
    let mut session = session();
    let step = session.feed(
        &result_line(json!({
            "is_error": true, "api_error_status": 500, "result": "API Error: 500 internal error",
            "stop_reason": "stop_sequence", "total_cost_usd": 0.02,
        })),
        2,
    );
    assert_eq!(
        events(&step),
        [
            total_cost(0.02),
            turn_end(TurnEndReason::Error { status: Some(500) }, false, false)
        ]
    );
    assert_eq!(
        session.finish(&exit_with(1)).outcome,
        PhaseOutcome::Failed {
            cause: PhaseFailure::Api { status: Some(500) }
        }
    );
}

#[test]
fn an_error_result_that_reports_no_cost_carries_no_usage() {
    let mut session = session();
    let mut result: Value = serde_json::from_slice(&result_line(
        json!({"is_error": true, "api_error_status": 500, "result": "API Error"}),
    ))
    .unwrap();
    result.as_object_mut().unwrap().remove("total_cost_usd");
    let step = session.feed(&line(result.clone()), 2);
    assert_eq!(
        events(&step),
        [turn_end(
            TurnEndReason::Error { status: Some(500) },
            false,
            false
        )]
    );
    result["total_cost_usd"] = Value::Null;
    let step = session.feed(&line(result), 3);
    assert!(
        !events(&step)
            .iter()
            .any(|e| matches!(e, AgentEvent::Usage { .. }))
    );
}

#[test]
fn an_error_result_is_never_the_report_whatever_its_status_or_subtype() {
    for fields in [
        json!({"is_error": true, "api_error_status": 429, "result": "You've hit your limit"}),
        json!({"is_error": true, "api_error_status": 500, "result": "API Error: 500"}),
        json!({"is_error": true, "api_error_status": 529, "result": "Overloaded"}),
        json!({"is_error": true, "api_error_status": null, "result": "Something failed"}),
        json!({"subtype": "error_max_budget_usd", "is_error": true, "result": "x"}),
        json!({"subtype": "error_during_execution", "is_error": false, "result": "x"}),
    ] {
        let step = session().feed(&result_line(fields.clone()), 2);
        assert_eq!(reports_in(&events(&step)), 0, "{fields}");
    }
}

#[test]
fn only_a_429_is_a_limit() {
    for status in [400, 401, 403, 500, 529] {
        let step = session().feed(
            &result_line(json!({"is_error": true, "api_error_status": status})),
            2,
        );
        assert!(
            !events(&step)
                .iter()
                .any(|e| matches!(e, AgentEvent::Limited { .. })),
            "{status}"
        );
    }
}

// ---- permission_denials --------------------------------------------------------------------------------------------

#[test]
fn permission_denials_are_not_read_as_prompts_the_user_refused() {
    // The list mixes hook, host, rule and auto denies (S9): the cause comes from the call's own metadata, so the result
    // adds nothing of its own.
    let denials =
        json!([{"tool_name": "Bash", "tool_use_id": "toolu_01", "tool_input": {"command": "ls"}}]);
    let with = session().feed(
        &result_line(json!({"result": "Done.", "permission_denials": denials})),
        2,
    );
    let without = session().feed(&result_line(json!({"result": "Done."})), 2);
    assert_eq!(events(&with), events(&without));
}
