//! The scripted ACP double (ts-0022-mc): an `AgentSession` that is fed the messages of an ACP agent (S7, the
//! `opencode acp` run) and writes the client's side of the conversation. The shared contract is in
//! `agent_session_contract.rs`; these tests are what only ACP has: the handshake, request ids, a permission request
//! that waits across feeds, and the turn end that the session reads from the `session/prompt` response.
//!
//! The ids a session writes are deterministic: `initialize` is 0, `session/new` is 1, the first `session/prompt` is
//! 2 and each later turn is the next number. A script that answers a request names that id.

use collide_core::adapter::{AgentSession, ExitInfo, PhaseFailure, PhaseOutcome, StampedEvent};
use collide_core::agent::{
    AgentEvent, FinalReportSource, PermissionReply, ToolKind, TurnEndReason, UsageQuality,
    UsageScope,
};
use collide_core::shim::{ExitEnd, ExitRecord, SHIM_PROTOCOL};
use collide_core::testing::{AcpSession, acp};
use serde_json::{Value, json};

const SESSION: &str = "ses_1";

fn parse(bytes: &[u8]) -> Value {
    serde_json::from_slice(bytes).expect("the session writes JSON")
}

fn written(writes: &[Vec<u8>]) -> Vec<Value> {
    writes.iter().map(|bytes| parse(bytes)).collect()
}

fn events(step: &collide_core::adapter::Step) -> Vec<AgentEvent> {
    step.events.iter().map(|e| e.event.clone()).collect()
}

fn exited() -> ExitInfo {
    ExitInfo {
        record: Some(ExitRecord {
            protocol: SHIM_PROTOCOL,
            end: ExitEnd::Exited { code: 0 },
            cpu_user_us: 10,
            cpu_system_us: 5,
            max_rss_bytes: 4096,
        }),
        stderr_tail: String::new(),
    }
}

/// A session that has finished the handshake and written its first prompt (id 2).
fn ready() -> AcpSession {
    let mut session = AcpSession::new("/work", "do the task");
    session.start();
    session.feed(&acp::initialize_response(0), 100);
    session.feed(&acp::new_session_response(1, SESSION), 110);
    session
}

fn bash_call() -> Value {
    json!({ "toolCallId": "call-1", "title": "bash", "kind": "execute", "status": "pending", "rawInput": {} })
}

// --- The handshake and the request ids ---

#[test]
fn start_writes_initialize_as_request_0_and_advertises_no_fs_or_terminal() {
    let mut session = AcpSession::new("/work", "do the task");
    let step = session.start();
    assert!(step.events.is_empty());
    assert_eq!(
        written(&step.writes),
        vec![json!({
            "jsonrpc": "2.0", "id": 0, "method": "initialize",
            "params": { "protocolVersion": 1, "clientCapabilities": {} }
        })]
    );
}

#[test]
fn the_initialize_response_makes_the_session_write_session_new_as_request_1_with_the_cwd() {
    let mut session = AcpSession::new("/work", "do the task");
    session.start();
    let step = session.feed(&acp::initialize_response(0), 100);
    assert!(step.events.is_empty());
    assert_eq!(
        written(&step.writes),
        vec![json!({
            "jsonrpc": "2.0", "id": 1, "method": "session/new",
            "params": { "cwd": "/work", "mcpServers": [] }
        })]
    );
}

#[test]
fn the_session_new_response_binds_the_id_and_writes_the_first_prompt_as_request_2() {
    let mut session = AcpSession::new("/work", "do the task");
    session.start();
    session.feed(&acp::initialize_response(0), 100);
    let step = session.feed(&acp::new_session_response(1, SESSION), 110);
    assert_eq!(
        step.events,
        vec![StampedEvent {
            wall_ns: Some(110),
            event: AgentEvent::SessionBound { id: SESSION.into() }
        }]
    );
    assert_eq!(
        written(&step.writes),
        vec![json!({
            "jsonrpc": "2.0", "id": 2, "method": "session/prompt",
            "params": { "sessionId": SESSION, "prompt": [{ "type": "text", "text": "do the task" }] }
        })]
    );
}

#[test]
fn each_later_turn_takes_the_next_request_id() {
    let mut session = ready();
    let first = written(&session.send_turn("again").writes);
    let second = written(&session.send_turn("and again").writes);
    assert_eq!(first[0]["id"], json!(3));
    assert_eq!(second[0]["id"], json!(4));
    assert_eq!(first[0]["params"]["prompt"][0]["text"], json!("again"));
}

// --- A permission request, held pending across feeds ---

#[test]
fn a_permission_request_is_held_pending_across_several_feeds_and_answered_after() {
    let mut session = ready();
    let asked = session.feed(
        &acp::permission_request(7, SESSION, bash_call(), acp::standard_options()),
        200,
    );
    assert_eq!(asked.events.len(), 1);
    assert!(asked.writes.is_empty());

    // The agent goes on while the request waits: nothing is answered for the host.
    for (index, text) in ["still", "working", "on it"].iter().enumerate() {
        let step = session.feed(&acp::message_chunk(SESSION, text), 210 + index as u64);
        assert!(
            step.writes.is_empty(),
            "feeding must not answer the request"
        );
    }
    let step = session.feed(&acp::tool_call(SESSION, "call-2", "read", "read"), 230);
    assert!(step.writes.is_empty());

    // After all of that, the request is still there to answer, by the id the agent gave it.
    let answer = session.answer("7", PermissionReply::Allow {});
    assert_eq!(
        written(&answer.writes),
        vec![json!({
            "jsonrpc": "2.0", "id": 7,
            "result": { "outcome": { "outcome": "selected", "optionId": "once" } }
        })]
    );
    // It is answered once only.
    assert!(
        session
            .answer("7", PermissionReply::Allow {})
            .writes
            .is_empty()
    );
}

#[test]
fn a_permission_request_becomes_a_normalized_event_with_the_agents_ids_and_options() {
    let mut session = ready();
    let step = session.feed(
        &acp::permission_request(7, SESSION, bash_call(), acp::standard_options()),
        200,
    );
    let AgentEvent::PermissionRequest { request } = &step.events[0].event else {
        panic!("expected a permission request, got {:?}", step.events);
    };
    assert_eq!(step.events[0].wall_ns, Some(200));
    assert_eq!(request.id, "7");
    assert_eq!(request.tool_call_id, "call-1");
    assert_eq!(request.tool.as_deref(), Some("bash"));
    assert_eq!(request.kind, ToolKind::Execute);
    assert_eq!(request.options.len(), 3);
    assert_eq!(request.options[0].id, "once");
}

#[test]
fn a_request_with_an_empty_raw_input_has_no_input_because_the_command_is_not_known_yet() {
    let mut session = ready();
    let step = session.feed(
        &acp::permission_request(7, SESSION, bash_call(), acp::standard_options()),
        200,
    );
    let AgentEvent::PermissionRequest { request } = &step.events[0].event else {
        panic!("expected a permission request");
    };
    assert_eq!(request.input, None);
}

#[test]
fn a_request_with_a_raw_input_keeps_it() {
    let mut session = ready();
    let call = json!({
        "toolCallId": "call-1", "title": "write", "kind": "edit",
        "rawInput": { "filepath": "a.txt" }
    });
    let step = session.feed(
        &acp::permission_request(7, SESSION, call, acp::standard_options()),
        200,
    );
    let AgentEvent::PermissionRequest { request } = &step.events[0].event else {
        panic!("expected a permission request");
    };
    assert_eq!(request.input, Some(json!({ "filepath": "a.txt" })));
}

#[test]
fn deny_selects_the_reject_once_option_and_drops_the_message() {
    let mut session = ready();
    session.feed(
        &acp::permission_request(7, SESSION, bash_call(), acp::standard_options()),
        200,
    );
    let step = session.answer(
        "7",
        PermissionReply::Deny {
            message: Some("not in this folder".into()),
        },
    );
    assert_eq!(
        written(&step.writes),
        vec![json!({
            "jsonrpc": "2.0", "id": 7,
            "result": { "outcome": { "outcome": "selected", "optionId": "reject" } }
        })]
    );
}

#[test]
fn a_cancel_reply_answers_the_request_cancelled() {
    let mut session = ready();
    session.feed(
        &acp::permission_request(7, SESSION, bash_call(), acp::standard_options()),
        200,
    );
    let step = session.answer("7", PermissionReply::Cancel {});
    assert_eq!(
        written(&step.writes),
        vec![
            json!({ "jsonrpc": "2.0", "id": 7, "result": { "outcome": { "outcome": "cancelled" } } })
        ]
    );
}

#[test]
fn allow_with_no_allow_once_option_answers_cancelled_and_reports_the_request_unanswerable() {
    let mut session = ready();
    let only_always = json!([{ "optionId": "always", "name": "Always", "kind": "allow_always" }]);
    session.feed(
        &acp::permission_request(7, SESSION, bash_call(), only_always),
        200,
    );
    let step = session.answer("7", PermissionReply::Allow {});
    assert_eq!(
        written(&step.writes),
        vec![
            json!({ "jsonrpc": "2.0", "id": 7, "result": { "outcome": { "outcome": "cancelled" } } })
        ]
    );
    assert_eq!(
        events(&step),
        vec![AgentEvent::PermissionUnanswerable {
            request_id: "7".into()
        }]
    );
    assert!(step.events.iter().all(|event| event.wall_ns.is_none()));
}

// --- Cancel, and how a turn ends ---

#[test]
fn cancel_writes_session_cancel_and_answers_every_pending_request_cancelled() {
    let mut session = ready();
    session.feed(
        &acp::permission_request(7, SESSION, bash_call(), acp::standard_options()),
        200,
    );
    session.feed(
        &acp::permission_request(8, SESSION, bash_call(), acp::standard_options()),
        210,
    );
    let step = session.cancel();
    assert_eq!(
        written(&step.writes),
        vec![
            json!({ "jsonrpc": "2.0", "method": "session/cancel", "params": { "sessionId": SESSION } }),
            json!({ "jsonrpc": "2.0", "id": 7, "result": { "outcome": { "outcome": "cancelled" } } }),
            json!({ "jsonrpc": "2.0", "id": 8, "result": { "outcome": { "outcome": "cancelled" } } }),
        ]
    );
    // Nothing is left to answer.
    assert!(
        session
            .answer("7", PermissionReply::Allow {})
            .writes
            .is_empty()
    );
}

#[test]
fn a_turn_the_agent_ends_end_turn_after_a_cancel_is_reported_cancelled() {
    // opencode answers a cancelled prompt with `end_turn` (S7, O1); the session reports Cancelled itself.
    let mut session = ready();
    session.cancel();
    let step = session.feed(&acp::prompt_response(2, "end_turn", None), 300);
    assert_eq!(
        events(&step).last(),
        Some(&AgentEvent::TurnEnd {
            reason: TurnEndReason::Cancelled {},
            cancel_requested: true,
            empty: true,
        })
    );
    assert_eq!(session.finish(&exited()).outcome, PhaseOutcome::Stopped);
}

#[test]
fn each_stop_reason_is_a_turn_end_reason() {
    let cases = [
        ("end_turn", TurnEndReason::EndTurn {}),
        ("max_tokens", TurnEndReason::MaxTokens {}),
        ("max_turn_requests", TurnEndReason::MaxTurns {}),
        ("refusal", TurnEndReason::Refusal {}),
        ("cancelled", TurnEndReason::Cancelled {}),
    ];
    for (stop, reason) in cases {
        let mut session = ready();
        session.feed(&acp::message_chunk(SESSION, "words"), 200);
        let step = session.feed(&acp::prompt_response(2, stop, None), 300);
        assert_eq!(
            events(&step).last(),
            Some(&AgentEvent::TurnEnd {
                reason,
                cancel_requested: false,
                empty: false
            }),
            "stop reason {stop}"
        );
    }
}

#[test]
fn a_turn_with_words_ends_and_the_final_report_is_the_chunks_after_the_last_tool_call() {
    let mut session = ready();
    session.feed(&acp::message_chunk(SESSION, "let me look"), 200);
    session.feed(&acp::tool_call(SESSION, "call-1", "read", "read"), 210);
    session.feed(&acp::message_chunk(SESSION, "all "), 220);
    session.feed(&acp::message_chunk(SESSION, "done"), 230);
    let step = session.feed(&acp::prompt_response(2, "end_turn", None), 300);
    assert_eq!(
        step.events[0],
        StampedEvent {
            wall_ns: Some(300),
            event: AgentEvent::FinalReport {
                text: "all done".into(),
                source: FinalReportSource::Reconstructed
            }
        }
    );
    assert_eq!(session.finish(&exited()).outcome, PhaseOutcome::Ended);
}

#[test]
fn a_turn_that_ends_with_no_output_and_no_usage_is_empty_and_the_phase_failed() {
    // S7, O4: a failed model call is a clean `end_turn` with nothing in it.
    let mut session = ready();
    let step = session.feed(&acp::prompt_response(2, "end_turn", None), 300);
    assert_eq!(
        events(&step),
        vec![AgentEvent::TurnEnd {
            reason: TurnEndReason::EndTurn {},
            cancel_requested: false,
            empty: true
        }]
    );
    assert_eq!(
        session.finish(&exited()).outcome,
        PhaseOutcome::Failed {
            cause: PhaseFailure::EmptyTurn
        }
    );
}

#[test]
fn prompt_usage_is_a_partial_turn_usage_event_before_the_turn_end() {
    let mut session = ready();
    let usage = json!({ "inputTokens": 34, "outputTokens": 49, "cachedReadTokens": 9331, "thoughtTokens": 0 });
    let step = session.feed(&acp::prompt_response(2, "end_turn", Some(usage)), 300);
    let AgentEvent::Usage { usage } = &step.events[0].event else {
        panic!("expected usage first, got {:?}", step.events);
    };
    assert_eq!(usage.scope, UsageScope::Turn);
    assert_eq!(usage.quality, UsageQuality::Partial);
    assert_eq!(usage.input, Some(34));
    assert_eq!(usage.output, Some(49));
    assert_eq!(usage.cache_read, Some(9331));
    assert_eq!(usage.cache_write, None);
    // A turn that spent tokens is not empty, even with no words.
    assert!(matches!(
        step.events[1].event,
        AgentEvent::TurnEnd { empty: false, .. }
    ));
}

#[test]
fn a_json_rpc_error_on_the_prompt_ends_the_turn_in_error_and_fails_the_phase() {
    let mut session = ready();
    let step = session.feed(&acp::prompt_error(2, -32000, "auth_required"), 300);
    assert!(matches!(
        events(&step).last(),
        Some(AgentEvent::TurnEnd {
            reason: TurnEndReason::Error { status: None },
            ..
        })
    ));
    assert_eq!(
        session.finish(&exited()).outcome,
        PhaseOutcome::Failed {
            cause: PhaseFailure::Api { status: None }
        }
    );
}

#[test]
fn a_child_that_exits_with_no_prompt_response_is_lost() {
    // S7: closing stdin with a request pending makes the agent exit 0 with no response at all.
    let mut session = ready();
    session.feed(
        &acp::permission_request(7, SESSION, bash_call(), acp::standard_options()),
        200,
    );
    assert_eq!(session.finish(&exited()).outcome, PhaseOutcome::Lost);
}

// --- Messages the session does not map ---

#[test]
fn an_update_it_does_not_know_is_an_other_event_with_the_raw_update() {
    let mut session = ready();
    let update = json!({ "sessionUpdate": "plan", "entries": [] });
    let step = session.feed(&acp::update(SESSION, update.clone()), 200);
    assert_eq!(
        events(&step),
        vec![AgentEvent::Other {
            raw_type: "plan".into(),
            raw: Some(update)
        }]
    );
}

#[test]
fn a_line_that_is_not_json_is_an_other_event_and_not_dropped() {
    let mut session = ready();
    let step = session.feed(b"not json at all\n", 200);
    assert_eq!(
        events(&step),
        vec![AgentEvent::Other {
            raw_type: "unparsed_line".into(),
            raw: Some(json!("not json at all"))
        }]
    );
}

#[test]
fn a_response_to_a_request_the_session_never_wrote_changes_nothing() {
    let mut session = ready();
    let step = session.feed(&acp::prompt_response(99, "end_turn", None), 200);
    assert!(step.events.is_empty() && step.writes.is_empty());
}
