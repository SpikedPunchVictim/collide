//! The Claude `AgentSession` (ts-0062-rj; 0.3, Two core traits and The stream protocol): `start`, the stream parser,
//! the turn input, and the phase's outcome. The main path of each recording is played through fake Claude under the
//! real shim ([`super::fake::play`]); the single lines the recordings do not hold are fed by hand.

use super::fake::{FakeClaude, Played, assigned, play, recording};
use super::{ASSIGNED, common::TempRoot};
use collide_core::adapter::{
    AgentSession, ExitInfo, PhaseFailure, PhaseOutcome, StampedEvent, Step,
};
use collide_core::agent::{
    AgentEvent, FinalReportSource, PermissionReply, PermissionRequest, SessionStart, ToolKind,
    TurnEndReason,
};
use collide_core::shim::{ExitEnd, ExitRecord, SHIM_PROTOCOL};
use collide_host::adapters::claude::session::ClaudeSession;
use serde_json::{Value, json};

fn session() -> ClaudeSession {
    ClaudeSession::new(assigned())
}

fn line(value: Value) -> Vec<u8> {
    let mut bytes = serde_json::to_vec(&value).unwrap();
    bytes.push(b'\n');
    bytes
}

fn written(step: &Step) -> Vec<Value> {
    step.writes
        .iter()
        .map(|bytes| {
            assert_eq!(bytes.last(), Some(&b'\n'), "one message is one line");
            serde_json::from_slice(bytes).expect("a write is JSON")
        })
        .collect()
}

fn events(step: &Step) -> Vec<AgentEvent> {
    step.events
        .iter()
        .map(|stamped| stamped.event.clone())
        .collect()
}

/// The events of `played` that are not `Other`, in order.
fn named(played: &Played) -> Vec<AgentEvent> {
    played
        .agent_events()
        .into_iter()
        .filter(|event| !matches!(event, AgentEvent::Other { .. }))
        .collect()
}

fn exit_with(code: i32) -> ExitInfo {
    ExitInfo {
        record: Some(ExitRecord {
            protocol: SHIM_PROTOCOL,
            end: ExitEnd::Exited { code },
            cpu_user_us: 1,
            cpu_system_us: 1,
            max_rss_bytes: 1,
        }),
        stderr_tail: String::new(),
    }
}

fn allow_all(session: &mut ClaudeSession, event: &AgentEvent) -> Step {
    match event {
        AgentEvent::PermissionRequest { request } => {
            session.answer(&request.id, PermissionReply::Allow {})
        }
        _ => Step::default(),
    }
}

fn answer_nothing(_: &mut ClaudeSession, _: &AgentEvent) -> Step {
    Step::default()
}

fn turn_end(reason: TurnEndReason, cancel_requested: bool, empty: bool) -> AgentEvent {
    AgentEvent::TurnEnd {
        reason,
        cancel_requested,
        empty,
    }
}

fn report(text: &str) -> AgentEvent {
    AgentEvent::FinalReport {
        text: text.into(),
        source: FinalReportSource::ResultField,
    }
}

fn message(text: &str) -> AgentEvent {
    AgentEvent::Message { text: text.into() }
}

/// A `can_use_tool` request as Claude writes it, without the input.
fn request_line(request_id: &str, input: Option<Value>) -> Vec<u8> {
    let mut request = json!({
        "subtype": "can_use_tool", "tool_name": "Bash", "display_name": "Bash",
        "tool_use_id": "toolu_77", "description": "Run it",
    });
    if let Some(input) = input {
        request["input"] = input;
    }
    line(json!({ "type": "control_request", "request_id": request_id, "request": request }))
}

fn result_line(fields: Value) -> Vec<u8> {
    let mut result = json!({
        "type": "result", "subtype": "success", "is_error": false, "result": "done",
        "stop_reason": "end_turn", "num_turns": 1,
        "usage": { "input_tokens": 4, "output_tokens": 9 },
    });
    for (key, value) in fields.as_object().unwrap() {
        result[key] = value.clone();
    }
    line(result)
}

fn tool_kind_of(tool: &str) -> ToolKind {
    let mut session = session();
    let step = session.feed(
        &line(json!({"type": "assistant", "message": {"content": [
            {"type": "tool_use", "id": "toolu_1", "name": tool, "input": {}}]}})),
        1,
    );
    match &events(&step)[..] {
        [AgentEvent::ToolCall { kind, .. }] => *kind,
        other => panic!("expected one tool call, got {other:?}"),
    }
}

// ---- start ---------------------------------------------------------------------------------------------------------

#[test]
fn an_assigned_session_is_bound_at_once_with_no_write_and_no_stored_time() {
    let step = session().start();
    assert_eq!(
        step.events,
        vec![StampedEvent {
            wall_ns: None,
            event: AgentEvent::SessionBound {
                id: ASSIGNED.into()
            },
        }]
    );
    assert!(step.writes.is_empty());
}

#[test]
fn a_resumed_session_is_bound_at_once_to_the_id_it_resumes() {
    let mut session = ClaudeSession::new(SessionStart::Resume { id: "abc-1".into() });
    let step = session.start();
    assert_eq!(
        events(&step),
        [AgentEvent::SessionBound { id: "abc-1".into() }]
    );
    assert!(step.writes.is_empty());
}

#[test]
fn a_session_the_agent_would_name_binds_nothing_because_claude_cannot_do_that() {
    let step = ClaudeSession::new(SessionStart::AgentAssigned {}).start();
    assert_eq!(step, Step::default());
}

#[test]
fn system_init_adds_no_second_session_bound() {
    let mut session = session();
    let init = line(json!({"type": "system", "subtype": "init", "session_id": ASSIGNED}));
    assert_eq!(session.feed(&init, 5), Step::default());
}

// ---- send_turn, answer, cancel ------------------------------------------------------------------------------------

#[test]
fn a_turn_is_a_user_message_on_stdin_in_the_shape_s1_measured() {
    let step = session().send_turn("fix the bug");
    assert_eq!(
        written(&step),
        [json!({
            "type": "user",
            "message": {"role": "user", "content": [{"type": "text", "text": "fix the bug"}]},
            "parent_tool_use_id": null,
            "session_id": "",
        })]
    );
    assert!(step.events.is_empty());
}

#[test]
fn a_turn_with_quotes_and_newlines_is_still_one_line() {
    let step = session().send_turn("say \"hi\"\nthen stop");
    assert_eq!(step.writes.len(), 1);
    assert_eq!(step.writes[0].iter().filter(|b| **b == b'\n').count(), 1);
    assert_eq!(
        written(&step)[0]["message"]["content"][0]["text"],
        "say \"hi\"\nthen stop"
    );
}

#[test]
fn allow_answers_with_behavior_allow_and_the_input_the_request_carried() {
    let mut session = session();
    session.feed(&request_line("req-1", Some(json!({"command": "ls"}))), 5);
    let step = session.answer("req-1", PermissionReply::Allow {});
    assert_eq!(
        written(&step),
        [json!({
            "type": "control_response",
            "response": {"subtype": "success", "request_id": "req-1",
                         "response": {"behavior": "allow", "updatedInput": {"command": "ls"}}},
        })]
    );
}

#[test]
fn deny_answers_with_behavior_deny_and_the_message() {
    let mut session = session();
    session.feed(&request_line("req-1", Some(json!({"command": "ls"}))), 5);
    let step = session.answer(
        "req-1",
        PermissionReply::Deny {
            message: Some("Not in this phase.".into()),
        },
    );
    assert_eq!(
        written(&step),
        [json!({
            "type": "control_response",
            "response": {"subtype": "success", "request_id": "req-1",
                         "response": {"behavior": "deny", "message": "Not in this phase."}},
        })]
    );
}

#[test]
fn a_deny_with_no_message_still_carries_one_because_claude_requires_it() {
    let mut session = session();
    session.feed(&request_line("req-1", Some(json!({}))), 5);
    let step = session.answer("req-1", PermissionReply::Deny { message: None });
    let message = &written(&step)[0]["response"]["response"]["message"];
    assert!(message.as_str().is_some_and(|m| !m.is_empty()), "{message}");
}

#[test]
fn a_cancel_reply_is_a_deny_that_says_the_host_cancelled_and_it_does_not_interrupt_the_turn() {
    let mut session = session();
    session.feed(&request_line("req-1", Some(json!({}))), 5);
    let step = session.answer("req-1", PermissionReply::Cancel {});
    let response = &written(&step)[0]["response"]["response"];
    assert_eq!(response["behavior"], "deny");
    assert!(
        response["message"].as_str().unwrap().contains("cancel"),
        "{response}"
    );
    assert!(response.get("interrupt").is_none(), "{response}");
}

#[test]
fn an_answer_to_an_id_the_session_does_not_hold_writes_nothing() {
    let mut session = session();
    assert_eq!(
        session.answer("nope", PermissionReply::Allow {}),
        Step::default()
    );
}

#[test]
fn a_request_is_answered_once() {
    let mut session = session();
    session.feed(&request_line("req-1", Some(json!({}))), 5);
    assert_eq!(
        session
            .answer("req-1", PermissionReply::Allow {})
            .writes
            .len(),
        1
    );
    assert_eq!(
        session.answer("req-1", PermissionReply::Allow {}),
        Step::default()
    );
}

#[test]
fn cancel_writes_an_interrupt_control_request_and_each_has_an_id_of_its_own() {
    let mut session = session();
    let first = written(&session.cancel());
    let second = written(&session.cancel());
    assert_eq!(
        first,
        [
            json!({"type": "control_request", "request_id": "collide-interrupt-1",
                "request": {"subtype": "interrupt"}})
        ]
    );
    assert_eq!(second[0]["request_id"], "collide-interrupt-2");
}

// ---- feed: what a line becomes -------------------------------------------------------------------------------------

#[test]
fn an_assistant_text_block_is_a_message_stamped_with_the_time_of_its_line() {
    let step = session().feed(
        &line(json!({"type": "assistant", "message": {"content": [{"type": "text", "text": "hello"}]}})),
        42,
    );
    assert_eq!(
        step.events,
        vec![StampedEvent {
            wall_ns: Some(42),
            event: message("hello"),
        }]
    );
    assert!(step.writes.is_empty(), "a line never writes");
}

#[test]
fn a_tool_use_block_is_a_tool_call_with_its_id_name_and_input() {
    let step = session().feed(
        &line(json!({"type": "assistant", "message": {"content": [
            {"type": "tool_use", "id": "toolu_9", "name": "Read", "input": {"file_path": "/work/repo/a.txt"}}]}})),
        1,
    );
    assert_eq!(
        events(&step),
        [AgentEvent::ToolCall {
            id: "toolu_9".into(),
            tool: Some("Read".into()),
            kind: ToolKind::Read,
            input: Some(json!({"file_path": "/work/repo/a.txt"})),
        }]
    );
}

#[test]
fn several_blocks_of_one_message_are_several_events_in_order() {
    let step = session().feed(
        &line(json!({"type": "assistant", "message": {"content": [
            {"type": "text", "text": "one"},
            {"type": "tool_use", "id": "t", "name": "Bash", "input": {"command": "ls"}},
            {"type": "text", "text": "two"}]}})),
        1,
    );
    let kinds: Vec<&str> = events(&step)
        .iter()
        .map(|event| match event {
            AgentEvent::Message { .. } => "message",
            AgentEvent::ToolCall { .. } => "tool_call",
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(kinds, ["message", "tool_call", "message"]);
}

#[test]
fn thinking_and_an_empty_text_block_are_not_messages() {
    let step = session().feed(
        &line(json!({"type": "assistant", "message": {"content": [
            {"type": "thinking", "thinking": "hmm", "signature": "x"},
            {"type": "text", "text": ""}]}})),
        1,
    );
    assert!(step.events.is_empty(), "{:?}", step.events);
}

#[test]
fn a_block_type_the_parser_does_not_know_is_other_and_names_it() {
    let step = session().feed(
        &line(json!({"type": "assistant", "message": {"content": [{"type": "server_tool_use", "id": "x"}]}})),
        1,
    );
    assert_eq!(
        events(&step),
        [AgentEvent::Other {
            raw_type: "assistant/server_tool_use".into(),
            raw: None
        }]
    );
}

#[test]
fn tools_are_classified_by_name() {
    for (tool, kind) in [
        ("Read", ToolKind::Read),
        ("Grep", ToolKind::Search),
        ("Glob", ToolKind::Search),
        ("Edit", ToolKind::Edit),
        ("Write", ToolKind::Edit),
        ("NotebookEdit", ToolKind::Edit),
        ("Bash", ToolKind::Execute),
        ("WebFetch", ToolKind::Fetch),
        ("WebSearch", ToolKind::Fetch),
        ("mcp__tracker__create_issue", ToolKind::Other),
        ("SomethingNew", ToolKind::Other),
    ] {
        assert_eq!(tool_kind_of(tool), kind, "{tool}");
    }
}

#[test]
fn an_event_type_the_parser_does_not_know_is_other_with_its_type_name() {
    let mut session = session();
    let step = session.feed(&line(json!({"type": "something_new", "x": 1})), 7);
    assert_eq!(
        step.events,
        vec![StampedEvent {
            wall_ns: Some(7),
            event: AgentEvent::Other {
                raw_type: "something_new".into(),
                raw: None
            },
        }]
    );
}

#[test]
fn a_system_event_other_than_init_is_other_named_with_its_subtype() {
    let step = session().feed(
        &line(json!({"type": "system", "subtype": "status", "status": "compacting"})),
        7,
    );
    assert_eq!(
        events(&step),
        [AgentEvent::Other {
            raw_type: "system/status".into(),
            raw: None
        }]
    );
}

#[test]
fn a_line_that_is_not_json_is_other_and_not_dropped() {
    let step = session().feed(b"warming up...\n", 7);
    assert_eq!(
        events(&step),
        [AgentEvent::Other {
            raw_type: "unparsed_line".into(),
            raw: None
        }]
    );
}

#[test]
fn a_json_line_with_no_type_is_other_too() {
    let step = session().feed(&line(json!({"hello": "world"})), 7);
    assert_eq!(
        events(&step),
        [AgentEvent::Other {
            raw_type: "unparsed_line".into(),
            raw: None
        }]
    );
}

// ---- permission requests -------------------------------------------------------------------------------------------

#[test]
fn a_can_use_tool_request_is_a_permission_request_with_the_agents_own_options_empty() {
    let step = session().feed(&request_line("req-1", Some(json!({"command": "ls"}))), 9);
    assert_eq!(
        step.events,
        vec![StampedEvent {
            wall_ns: Some(9),
            event: AgentEvent::PermissionRequest {
                request: PermissionRequest {
                    id: "req-1".into(),
                    tool_call_id: "toolu_77".into(),
                    tool: Some("Bash".into()),
                    kind: ToolKind::Execute,
                    input: Some(json!({"command": "ls"})),
                    reason: None,
                    options: vec![],
                },
            },
        }]
    );
    assert!(step.writes.is_empty(), "a line never answers a request");
}

#[test]
fn a_request_whose_input_is_unknown_carries_none() {
    let step = session().feed(&request_line("req-1", None), 9);
    let [AgentEvent::PermissionRequest { request }] = &events(&step)[..] else {
        panic!("{:?}", step.events)
    };
    assert_eq!(request.input, None);
}

#[test]
fn a_null_input_is_unknown_too() {
    let step = session().feed(&request_line("req-1", Some(Value::Null)), 9);
    let [AgentEvent::PermissionRequest { request }] = &events(&step)[..] else {
        panic!("{:?}", step.events)
    };
    assert_eq!(request.input, None);
}

#[test]
fn allowing_a_request_with_unknown_input_does_not_send_an_empty_one_in_its_place() {
    let mut session = session();
    session.feed(&request_line("req-1", None), 9);
    let step = session.answer("req-1", PermissionReply::Allow {});
    let response = &written(&step)[0]["response"]["response"];
    assert_eq!(response["behavior"], "allow");
    assert!(response.get("updatedInput").is_none(), "{response}");
}

#[test]
fn a_hook_ask_carries_the_hooks_text_as_the_reason() {
    let mut request = json!({
        "subtype": "can_use_tool", "tool_name": "Bash", "input": {"command": "ls"}, "tool_use_id": "toolu_77",
        "decision_reason_type": "hook", "decision_reason": "Policy: ask first.",
    });
    request["decision_reason_hook_event"] = json!("PreToolUse");
    let step = session().feed(
        &line(json!({"type": "control_request", "request_id": "req-2", "request": request})),
        1,
    );
    let [AgentEvent::PermissionRequest { request }] = &events(&step)[..] else {
        panic!("{:?}", step.events)
    };
    assert_eq!(request.reason.as_deref(), Some("Policy: ask first."));
    assert!(
        request.options.is_empty(),
        "no answer offers an always option"
    );
}

#[test]
fn a_control_request_of_another_kind_is_other_with_its_subtype() {
    let step = session().feed(
        &line(
            json!({"type": "control_request", "request_id": "r", "request": {"subtype": "elicit"}}),
        ),
        1,
    );
    assert_eq!(
        events(&step),
        [AgentEvent::Other {
            raw_type: "control_request/elicit".into(),
            raw: None
        }]
    );
}

#[test]
fn a_can_use_tool_request_with_no_request_id_cannot_be_answered_so_it_is_other() {
    let step = session().feed(
        &line(json!({"type": "control_request", "request": {"subtype": "can_use_tool", "tool_name": "Bash"}})),
        1,
    );
    assert_eq!(
        events(&step),
        [AgentEvent::Other {
            raw_type: "control_request/can_use_tool".into(),
            raw: None
        }]
    );
}

// ---- the result: TurnEnd and FinalReport ---------------------------------------------------------------------------

#[test]
fn a_result_is_the_final_report_from_result_result_and_then_the_end_of_the_turn() {
    let mut session = session();
    session.feed(
        &line(json!({"type": "assistant", "message": {"content": [{"type": "text", "text": "done"}]}})),
        1,
    );
    let step = session.feed(&result_line(json!({"result": "All finished."})), 2);
    assert_eq!(
        events(&step),
        [
            report("All finished."),
            turn_end(TurnEndReason::EndTurn {}, false, false)
        ]
    );
    assert!(step.events.iter().all(|stamped| stamped.wall_ns == Some(2)));
}

#[test]
fn each_stop_reason_of_a_result_is_its_own_turn_end_reason() {
    for (fields, reason) in [
        (
            json!({"stop_reason": "end_turn"}),
            TurnEndReason::EndTurn {},
        ),
        (json!({"stop_reason": null}), TurnEndReason::EndTurn {}),
        (
            json!({"stop_reason": "max_tokens"}),
            TurnEndReason::MaxTokens {},
        ),
        (json!({"stop_reason": "refusal"}), TurnEndReason::Refusal {}),
        (
            json!({"subtype": "error_max_turns", "is_error": true, "result": null, "stop_reason": "tool_use"}),
            TurnEndReason::MaxTurns {},
        ),
    ] {
        let mut session = session();
        let step = session.feed(&result_line(fields.clone()), 2);
        let Some(AgentEvent::TurnEnd { reason: got, .. }) = events(&step).pop() else {
            panic!("no turn end for {fields}")
        };
        assert_eq!(got, reason, "{fields}");
    }
}

#[test]
fn a_cancel_makes_the_turn_end_cancelled_whatever_the_agents_own_reason() {
    for stop_reason in ["end_turn", "max_tokens", "refusal", "tool_use"] {
        let mut session = session();
        session.cancel();
        let step = session.feed(&result_line(json!({"stop_reason": stop_reason})), 2);
        let Some(AgentEvent::TurnEnd {
            reason,
            cancel_requested,
            ..
        }) = events(&step).pop()
        else {
            panic!("no turn end")
        };
        assert_eq!(reason, TurnEndReason::Cancelled {}, "{stop_reason}");
        assert!(cancel_requested, "{stop_reason}");
    }
}

#[test]
fn a_cancel_makes_an_error_result_cancelled_too() {
    let mut session = session();
    session.cancel();
    let step = session.feed(
        &result_line(
            json!({"subtype": "error_during_execution", "is_error": true, "result": null}),
        ),
        2,
    );
    let Some(AgentEvent::TurnEnd { reason, .. }) = events(&step).pop() else {
        panic!()
    };
    assert_eq!(reason, TurnEndReason::Cancelled {});
}

#[test]
fn a_new_turn_is_a_new_thing_to_cancel() {
    let mut session = session();
    session.cancel();
    session.send_turn("again");
    let step = session.feed(&result_line(json!({})), 2);
    let Some(AgentEvent::TurnEnd {
        reason,
        cancel_requested,
        ..
    }) = events(&step).pop()
    else {
        panic!()
    };
    assert_eq!(reason, TurnEndReason::EndTurn {});
    assert!(!cancel_requested);
}

#[test]
fn a_turn_with_no_message_no_tool_call_and_no_usage_is_empty() {
    let mut session = session();
    let step = session.feed(
        &result_line(
            json!({"result": "", "usage": {"input_tokens": 0, "output_tokens": 0}, "num_turns": 0}),
        ),
        2,
    );
    assert_eq!(
        events(&step),
        [turn_end(TurnEndReason::EndTurn {}, false, true)]
    );
}

#[test]
fn a_result_with_no_usage_field_counts_as_unreported_usage() {
    let mut session = session();
    let mut result = serde_json::from_slice::<Value>(&result_line(json!({"result": ""}))).unwrap();
    result.as_object_mut().unwrap().remove("usage");
    let step = session.feed(&line(result), 2);
    assert_eq!(
        events(&step),
        [turn_end(TurnEndReason::EndTurn {}, false, true)]
    );
}

#[test]
fn a_message_or_a_tool_call_or_usage_makes_the_turn_not_empty() {
    let quiet = json!({"result": "", "usage": {"input_tokens": 0, "output_tokens": 0}});
    let say =
        line(json!({"type": "assistant", "message": {"content": [{"type": "text", "text": "x"}]}}));
    let call = line(json!({"type": "assistant", "message": {"content": [
        {"type": "tool_use", "id": "t", "name": "Bash", "input": {}}]}}));
    for earlier in [Some(say), Some(call), None] {
        let mut session = session();
        let fields = if earlier.is_some() {
            quiet.clone()
        } else {
            json!({"result": "", "usage": {"cache_read_input_tokens": 5}})
        };
        if let Some(earlier) = &earlier {
            session.feed(earlier, 1);
        }
        let step = session.feed(&result_line(fields), 2);
        let Some(AgentEvent::TurnEnd { empty, .. }) = events(&step).pop() else {
            panic!()
        };
        assert!(!empty);
    }
}

#[test]
fn the_next_turn_starts_with_nothing_said() {
    let mut session = session();
    session.feed(
        &line(
            json!({"type": "assistant", "message": {"content": [{"type": "text", "text": "x"}]}}),
        ),
        1,
    );
    session.feed(&result_line(json!({})), 2);
    let quiet = json!({"result": "", "usage": {"input_tokens": 0, "output_tokens": 0}});
    let step = session.feed(&result_line(quiet), 3);
    let Some(AgentEvent::TurnEnd { empty, .. }) = events(&step).pop() else {
        panic!()
    };
    assert!(
        empty,
        "the first turn's message does not count for the second"
    );
}

#[test]
fn an_error_result_is_never_the_report_and_ends_the_turn_as_an_error_with_the_status() {
    let mut session = session();
    let step = session.feed(
        &result_line(
            json!({"is_error": true, "result": "You've hit your limit", "api_error_status": 429}),
        ),
        2,
    );
    assert_eq!(
        events(&step),
        [turn_end(
            TurnEndReason::Error { status: Some(429) },
            false,
            false
        )]
    );
}

#[test]
fn an_empty_result_text_is_no_report() {
    let step = session().feed(&result_line(json!({"result": ""})), 2);
    assert!(
        !events(&step)
            .iter()
            .any(|e| matches!(e, AgentEvent::FinalReport { .. }))
    );
}

// ---- finish --------------------------------------------------------------------------------------------------------

#[test]
fn finish_is_ended_after_a_turn_that_ended_normally_even_when_it_was_empty() {
    let mut session = session();
    session.feed(&result_line(json!({"result": "", "usage": {}})), 2);
    let finished = session.finish(&exit_with(0));
    assert_eq!(finished.outcome, PhaseOutcome::Ended);
    assert_eq!(finished.step, Step::default());
}

#[test]
fn finish_names_the_failure_for_each_turn_end() {
    for (fields, outcome) in [
        (
            json!({"stop_reason": "max_tokens"}),
            PhaseOutcome::Failed {
                cause: PhaseFailure::MaxTokens,
            },
        ),
        (
            json!({"stop_reason": "refusal"}),
            PhaseOutcome::Failed {
                cause: PhaseFailure::Refusal,
            },
        ),
        (
            json!({"subtype": "error_max_turns", "is_error": true, "result": null}),
            PhaseOutcome::Failed {
                cause: PhaseFailure::MaxTurns,
            },
        ),
        (
            json!({"is_error": true, "api_error_status": 500}),
            PhaseOutcome::Failed {
                cause: PhaseFailure::Api { status: Some(500) },
            },
        ),
    ] {
        let mut session = session();
        session.feed(&result_line(fields.clone()), 2);
        assert_eq!(session.finish(&exit_with(0)).outcome, outcome, "{fields}");
    }
}

#[test]
fn finish_with_no_result_is_no_result_and_with_no_exit_record_is_lost() {
    assert_eq!(
        session().finish(&exit_with(3)).outcome,
        PhaseOutcome::Failed {
            cause: PhaseFailure::NoResult
        }
    );
    let mut lost = session();
    lost.feed(&result_line(json!({})), 2);
    let no_record = ExitInfo {
        record: None,
        stderr_tail: "shim died".into(),
    };
    assert_eq!(lost.finish(&no_record).outcome, PhaseOutcome::Lost);
}

#[test]
fn a_turn_that_was_sent_and_never_ended_is_no_result_even_after_an_earlier_one_ended() {
    let mut session = session();
    session.feed(&result_line(json!({})), 2);
    session.send_turn("and another");
    assert_eq!(
        session.finish(&exit_with(143)).outcome,
        PhaseOutcome::Failed {
            cause: PhaseFailure::NoResult
        }
    );
}

#[test]
fn a_process_that_dies_mid_turn_with_exit_code_3_is_no_result() {
    let root = TempRoot::new();
    let mut fake = FakeClaude::start(&root, &recording("exit-code"));
    let mut session = session();
    session.start();
    let turn = session.send_turn("go");
    fake.send(&turn.writes[0]);
    let mut fed = Vec::new();
    while let Some(bytes) = fake.next_line() {
        fed.extend(events(&session.feed(&bytes, 1)));
    }
    let ended = fake.finish();
    assert_eq!(ended.code, Some(3));
    assert!(!fed.iter().any(|e| matches!(e, AgentEvent::TurnEnd { .. })));
    let outcome = session.finish(&exit_with(3)).outcome;
    assert_eq!(
        outcome,
        PhaseOutcome::Failed {
            cause: PhaseFailure::NoResult
        }
    );
}

// ---- over the recordings -------------------------------------------------------------------------------------------

#[test]
fn the_report_recording_gives_messages_tool_calls_the_final_report_and_the_end_of_the_turn() {
    let played = play("report", assigned(), &["read the notes"], answer_nothing);
    assert_eq!(
        named(&played),
        [
            AgentEvent::SessionBound {
                id: ASSIGNED.into()
            },
            message("Reading the notes."),
            AgentEvent::ToolCall {
                id: "toolu_01".into(),
                tool: Some("Read".into()),
                kind: ToolKind::Read,
                input: Some(json!({"file_path": "/work/repo/notes.txt"})),
            },
            AgentEvent::ToolCall {
                id: "toolu_02".into(),
                tool: Some("Grep".into()),
                kind: ToolKind::Search,
                input: Some(json!({"pattern": "First", "path": "/work/repo"})),
            },
            message("The notes start with the first line."),
            report("The notes start with the first line."),
            turn_end(TurnEndReason::EndTurn {}, false, false),
        ]
    );
    assert_eq!(
        played.exit.record.as_ref().unwrap().end,
        ExitEnd::Exited { code: 0 }
    );
    let mut session = played.session;
    assert_eq!(session.finish(&played.exit).outcome, PhaseOutcome::Ended);
}

#[test]
fn the_permission_recording_asks_waits_for_the_allow_and_fake_claude_got_the_answer_it_expects() {
    let played = play(
        "permission-request",
        assigned(),
        &["touch the file"],
        allow_all,
    );
    let request = played
        .agent_events()
        .into_iter()
        .find_map(|event| match event {
            AgentEvent::PermissionRequest { request } => Some(request),
            _ => None,
        })
        .expect("a permission request");
    assert_eq!(request.id, "req-01");
    assert_eq!(request.tool_call_id, "toolu_01");
    assert_eq!(request.kind, ToolKind::Execute);
    assert_eq!(
        request.input,
        Some(json!({"command": "touch out.txt", "description": "Create the file"}))
    );
    // Fake Claude exits 97 when the allow it expects (the request id and the echoed input) is not what it read.
    assert_eq!(
        played.exit.record.as_ref().unwrap().end,
        ExitEnd::Exited { code: 0 },
        "{}",
        played.exit.stderr_tail
    );
    let tail = named(&played);
    assert_eq!(
        tail.last(),
        Some(&turn_end(TurnEndReason::EndTurn {}, false, false))
    );
    assert!(tail.contains(&report("I created out.txt.")));
}

#[test]
fn the_host_deny_recording_gets_the_deny_and_the_message_it_expects() {
    let played = play(
        "host-deny",
        assigned(),
        &["touch the file"],
        |session, event| match event {
            AgentEvent::PermissionRequest { request } => session.answer(
                &request.id,
                PermissionReply::Deny {
                    message: Some("Not in this phase.".into()),
                },
            ),
            _ => Step::default(),
        },
    );
    assert_eq!(
        played.exit.record.as_ref().unwrap().end,
        ExitEnd::Exited { code: 0 },
        "{}",
        played.exit.stderr_tail
    );
    assert!(named(&played).contains(&report("The host refused the command.")));
}

#[test]
fn the_multi_turn_recording_gives_a_report_and_an_end_for_each_turn_and_binds_the_session_once() {
    let played = play(
        "multi-turn",
        assigned(),
        &["first", "second"],
        answer_nothing,
    );
    let all = named(&played);
    assert_eq!(
        all.iter()
            .filter(|e| matches!(e, AgentEvent::SessionBound { .. }))
            .count(),
        1
    );
    assert_eq!(
        all.iter()
            .filter(|e| matches!(e, AgentEvent::TurnEnd { .. }))
            .count(),
        2
    );
    let reports: Vec<&AgentEvent> = all
        .iter()
        .filter(|e| matches!(e, AgentEvent::FinalReport { .. }))
        .collect();
    assert_eq!(reports, [&report("ONE"), &report("TWO")]);
    assert_eq!(
        played.exit.record.as_ref().unwrap().end,
        ExitEnd::Exited { code: 0 },
        "{}",
        played.exit.stderr_tail
    );
}

#[test]
fn the_cancelled_recording_gets_the_interrupt_it_expects_and_ends_cancelled_and_stopped() {
    let played = play(
        "cancelled",
        assigned(),
        &["wait a while"],
        |session, event| match event {
            AgentEvent::ToolCall { .. } => session.cancel(),
            _ => Step::default(),
        },
    );
    assert_eq!(
        played.exit.record.as_ref().unwrap().end,
        ExitEnd::Exited { code: 0 },
        "{}",
        played.exit.stderr_tail
    );
    let Some(AgentEvent::TurnEnd {
        reason,
        cancel_requested,
        ..
    }) = named(&played).pop()
    else {
        panic!("no turn end")
    };
    assert_eq!(reason, TurnEndReason::Cancelled {});
    assert!(cancel_requested);
    let mut session = played.session;
    assert_eq!(session.finish(&played.exit).outcome, PhaseOutcome::Stopped);
}

#[test]
fn the_turn_end_recordings_end_as_end_of_turn_max_tokens_max_turns_and_refusal() {
    for (name, reason, outcome) in [
        ("report", TurnEndReason::EndTurn {}, PhaseOutcome::Ended),
        (
            "max-tokens",
            TurnEndReason::MaxTokens {},
            PhaseOutcome::Failed {
                cause: PhaseFailure::MaxTokens,
            },
        ),
        (
            "max-turns",
            TurnEndReason::MaxTurns {},
            PhaseOutcome::Failed {
                cause: PhaseFailure::MaxTurns,
            },
        ),
        (
            "refusal",
            TurnEndReason::Refusal {},
            PhaseOutcome::Failed {
                cause: PhaseFailure::Refusal,
            },
        ),
    ] {
        let mut played = play(name, assigned(), &["go"], answer_nothing);
        assert_eq!(
            named(&played).pop(),
            Some(turn_end(reason, false, false)),
            "{name}"
        );
        assert_eq!(
            played.session.finish(&played.exit).outcome,
            outcome,
            "{name}"
        );
    }
}

#[test]
fn the_compaction_recording_is_an_empty_turn_with_no_report() {
    let played = play("compaction", assigned(), &["/compact"], answer_nothing);
    let all = named(&played);
    assert_eq!(
        all.last(),
        Some(&turn_end(TurnEndReason::EndTurn {}, false, true))
    );
    assert!(
        !all.iter()
            .any(|e| matches!(e, AgentEvent::FinalReport { .. }))
    );
}

#[test]
fn the_error_recording_is_an_error_turn_with_no_report_and_exit_code_1() {
    let mut played = play("error-result", assigned(), &["go"], answer_nothing);
    assert!(named(&played).contains(&turn_end(
        TurnEndReason::Error { status: None },
        false,
        false
    )));
    assert!(
        !named(&played)
            .iter()
            .any(|e| matches!(e, AgentEvent::FinalReport { .. }))
    );
    assert_eq!(
        played.exit.record.as_ref().unwrap().end,
        ExitEnd::Exited { code: 1 }
    );
    assert_eq!(
        played.session.finish(&played.exit).outcome,
        PhaseOutcome::Failed {
            cause: PhaseFailure::Api { status: None }
        }
    );
}

#[test]
fn every_other_recording_is_read_to_its_end_with_one_session_bound_and_one_turn_end_and_nothing_dropped()
 {
    // The recordings whose cases ts-0063-rw and ts-0064-hw read in full: here each is read to its end without a panic,
    // and fake Claude exits as the recording says.
    for (name, code) in [
        ("limit", 0),
        ("blocked-prompt", 0),
        ("hook-deny", 0),
        ("hook-failure", 0),
        ("hook-block", 0),
        ("disallowed-tool", 0),
    ] {
        let played = play(name, assigned(), &["go"], allow_all);
        let all = played.agent_events();
        assert_eq!(
            all.iter()
                .filter(|e| matches!(e, AgentEvent::SessionBound { .. }))
                .count(),
            1,
            "{name}"
        );
        assert_eq!(
            all.iter()
                .filter(|e| matches!(e, AgentEvent::TurnEnd { .. }))
                .count(),
            1,
            "{name}"
        );
        assert_eq!(
            played.exit.record.as_ref().unwrap().end,
            ExitEnd::Exited { code },
            "{name}: {}",
            played.exit.stderr_tail
        );
    }
}
