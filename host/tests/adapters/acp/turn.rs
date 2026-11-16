//! A prompt turn through the ACP session (ts-0087-mf): what each `session/update` becomes, how the `session/prompt`
//! response ends the turn, and one whole turn of the fake child under the real shim.

use super::fake::{play, recording};
use super::session::{SESSION, events_of, exited, line, ready, written};
use collide_core::adapter::{AgentSession, PhaseFailure, PhaseOutcome, StampedEvent, Step};
use collide_core::agent::{AgentEvent, FinalReportSource, ToolKind, TurnEndReason};
use collide_core::journal::{Direction, read_journal};
use collide_core::testing::acp::{
    message_chunk, permission_request, prompt_error, prompt_response, standard_options, tool_call,
    update,
};
use collide_host::adapters::acp::options::AcpOptions;
use collide_host::adapters::acp::session::AcpSession;
use serde_json::{Value, json};

fn turn_end_of(step: &Step) -> (TurnEndReason, bool, bool) {
    match events_of(step).last() {
        Some(AgentEvent::TurnEnd {
            reason,
            cancel_requested,
            empty,
        }) => (reason.clone(), *cancel_requested, *empty),
        other => panic!("expected a turn end last, got {other:?}"),
    }
}

fn feed_update(session: &mut AcpSession, body: Value) -> Vec<AgentEvent> {
    events_of(&session.feed(&update(SESSION, body), 300))
}

// ---- session/update ------------------------------------------------------------------------------------------------

#[test]
fn a_message_chunk_gives_a_message_with_the_stored_time() {
    let mut session = ready();
    let step = session.feed(&message_chunk(SESSION, "Reading the notes. "), 777);
    assert_eq!(
        step.events,
        vec![StampedEvent {
            wall_ns: Some(777),
            event: AgentEvent::Message {
                text: "Reading the notes. ".into()
            }
        }]
    );
    assert!(step.writes.is_empty());
}

#[test]
fn a_message_chunk_that_holds_no_text_is_an_other_with_no_content() {
    let mut session = ready();
    let events = feed_update(
        &mut session,
        json!({
            "sessionUpdate": "agent_message_chunk",
            "content": { "type": "image", "data": "SECRET-STAND-IN", "mimeType": "image/png" }
        }),
    );
    assert_eq!(
        events,
        vec![AgentEvent::Other {
            raw_type: "agent_message_chunk".into(),
            raw: None
        }]
    );
}

#[test]
fn every_other_update_is_an_other_named_by_its_session_update_with_no_content() {
    for kind in [
        "tool_call_update",
        "agent_thought_chunk",
        "plan",
        "available_commands_update",
        "config_option_update",
        "user_message_chunk",
        "an_update_of_a_later_version",
    ] {
        let mut session = ready();
        let events = feed_update(
            &mut session,
            json!({
                "sessionUpdate": kind, "toolCallId": "call-stand-in-1",
                "content": { "type": "text", "text": "SECRET-STAND-IN" },
                "rawOutput": "SECRET-STAND-IN"
            }),
        );
        assert_eq!(
            events,
            vec![AgentEvent::Other {
                raw_type: kind.into(),
                raw: None
            }],
            "{kind}"
        );
    }
}

#[test]
fn an_update_with_no_kind_is_an_other_named_session_update() {
    let mut session = ready();
    let events = feed_update(&mut session, json!({ "content": "SECRET-STAND-IN" }));
    assert_eq!(
        events,
        vec![AgentEvent::Other {
            raw_type: "session/update".into(),
            raw: None
        }]
    );
}

#[test]
fn a_tool_call_is_classified_by_its_kind_and_never_by_its_title() {
    // The titles say another tool than the kind does, as an agent's titles do (S7, O12).
    let cases = [
        ("read", ToolKind::Read, "bash"),
        ("edit", ToolKind::Edit, "Read notes.txt"),
        ("delete", ToolKind::Delete, "Read notes.txt"),
        ("move", ToolKind::Move, "Read notes.txt"),
        ("search", ToolKind::Search, "Read notes.txt"),
        ("execute", ToolKind::Execute, "Read notes.txt"),
        ("think", ToolKind::Think, "Read notes.txt"),
        ("fetch", ToolKind::Fetch, "Read notes.txt"),
        ("switch_mode", ToolKind::Other, "Read notes.txt"),
        (
            "a_kind_of_a_later_version",
            ToolKind::Other,
            "Read notes.txt",
        ),
    ];
    for (word, kind, title) in cases {
        let mut session = ready();
        let step = session.feed(&tool_call(SESSION, "call-stand-in-1", title, word), 400);
        assert_eq!(
            step.events,
            vec![StampedEvent {
                wall_ns: Some(400),
                event: AgentEvent::ToolCall {
                    id: "call-stand-in-1".into(),
                    tool: Some(title.into()),
                    kind,
                    input: None
                }
            }],
            "{word}"
        );
    }
}

#[test]
fn a_tool_call_with_no_kind_is_of_kind_other_whatever_its_title_says() {
    let mut session = ready();
    let events = feed_update(
        &mut session,
        json!({ "sessionUpdate": "tool_call", "toolCallId": "c1", "title": "Read notes.txt" }),
    );
    let [AgentEvent::ToolCall { kind, .. }] = events.as_slice() else {
        panic!("{events:?}");
    };
    assert_eq!(*kind, ToolKind::Other);
}

#[test]
fn a_tool_calls_input_is_none_while_the_agent_sends_none_or_an_empty_object() {
    let input_of = |fields: Value| {
        let mut session = ready();
        let mut body = json!({
            "sessionUpdate": "tool_call", "toolCallId": "c1", "title": "bash", "kind": "execute"
        });
        body.as_object_mut()
            .unwrap()
            .extend(fields.as_object().unwrap().clone());
        match feed_update(&mut session, body).as_slice() {
            [AgentEvent::ToolCall { input, .. }] => input.clone(),
            other => panic!("{other:?}"),
        }
    };
    assert_eq!(input_of(json!({})), None);
    assert_eq!(input_of(json!({ "rawInput": null })), None);
    assert_eq!(input_of(json!({ "rawInput": {} })), None);
    assert_eq!(
        input_of(json!({ "rawInput": { "command": "ls" } })),
        Some(json!({ "command": "ls" }))
    );
}

#[test]
fn a_permission_request_before_its_tool_call_is_not_an_error() {
    // S7, O6: the agent asks before it announces the call it asks about.
    let mut session = ready();
    let call =
        json!({ "toolCallId": "call-1", "title": "bash", "kind": "execute", "rawInput": {} });
    let asked = session.feed(
        &permission_request(7, SESSION, call, standard_options()),
        500,
    );
    assert!(matches!(
        events_of(&asked).as_slice(),
        [AgentEvent::PermissionRequest { .. }]
    ));
    let announced = session.feed(&tool_call(SESSION, "call-1", "bash", "execute"), 510);
    assert!(matches!(
        events_of(&announced).as_slice(),
        [AgentEvent::ToolCall { .. }]
    ));
}

// ---- the end of a turn ---------------------------------------------------------------------------------------------

#[test]
fn each_stop_reason_ends_the_turn_with_the_reason_of_the_same_meaning() {
    for (word, reason) in [
        ("end_turn", TurnEndReason::EndTurn {}),
        ("max_tokens", TurnEndReason::MaxTokens {}),
        ("max_turn_requests", TurnEndReason::MaxTurns {}),
        ("refusal", TurnEndReason::Refusal {}),
        ("cancelled", TurnEndReason::Cancelled {}),
    ] {
        let mut session = ready();
        let step = session.feed(&prompt_response(2, word, None), 600);
        assert_eq!(turn_end_of(&step), (reason, false, true), "{word}");
        assert_eq!(step.events.last().unwrap().wall_ns, Some(600));
        assert!(step.writes.is_empty());
    }
}

#[test]
fn an_unknown_stop_reason_a_missing_one_and_an_error_response_are_each_an_error_turn_end() {
    let error = TurnEndReason::Error { status: None };
    for response in [
        prompt_response(2, "a_reason_of_a_later_version", None),
        prompt_response(2, "", None),
        line(json!({ "jsonrpc": "2.0", "id": 2, "result": {} })),
        line(json!({ "jsonrpc": "2.0", "id": 2, "result": { "stopReason": 7 } })),
        prompt_error(2, -32603, "Internal error SECRET-STAND-IN"),
    ] {
        let mut session = ready();
        let step = session.feed(&response, 600);
        assert_eq!(
            turn_end_of(&step),
            (error.clone(), false, true),
            "{response:?}"
        );
        let kept = format!("{:?}", step.events);
        assert!(!kept.contains("SECRET-STAND-IN"), "{kept}");
    }
}

#[test]
fn finish_says_what_the_turn_end_says() {
    let failed = |cause| PhaseOutcome::Failed { cause };
    for (response, outcome) in [
        (prompt_response(2, "end_turn", None), PhaseOutcome::Ended),
        (
            prompt_response(2, "max_tokens", None),
            failed(PhaseFailure::MaxTokens),
        ),
        (
            prompt_response(2, "max_turn_requests", None),
            failed(PhaseFailure::MaxTurns),
        ),
        (
            prompt_response(2, "refusal", None),
            failed(PhaseFailure::Refusal),
        ),
        (prompt_response(2, "cancelled", None), PhaseOutcome::Stopped),
        (
            prompt_response(2, "a_reason_of_a_later_version", None),
            failed(PhaseFailure::Api { status: None }),
        ),
        (
            prompt_error(2, -32603, "Internal error"),
            failed(PhaseFailure::Api { status: None }),
        ),
    ] {
        let mut session = ready();
        session.feed(&message_chunk(SESSION, "words"), 500);
        session.feed(&response, 600);
        assert_eq!(
            session.finish(&exited()).outcome,
            outcome,
            "{}",
            String::from_utf8_lossy(&response)
        );
    }
}

#[test]
fn the_final_report_holds_the_words_after_the_last_tool_call_and_comes_before_the_turn_end() {
    let mut session = ready();
    session.feed(&message_chunk(SESSION, "Let me look. "), 500);
    session.feed(&tool_call(SESSION, "c1", "Read a", "read"), 510);
    session.feed(&message_chunk(SESSION, "And again. "), 520);
    session.feed(&tool_call(SESSION, "c2", "Read b", "read"), 530);
    session.feed(&message_chunk(SESSION, "The notes "), 540);
    session.feed(&message_chunk(SESSION, "say hello."), 550);
    let step = session.feed(&prompt_response(2, "end_turn", None), 560);
    let events = events_of(&step);
    assert_eq!(events.len(), 3, "{events:?}");
    assert!(matches!(events[0], AgentEvent::Usage { .. }));
    assert_eq!(
        events[1],
        AgentEvent::FinalReport {
            text: "The notes say hello.".into(),
            source: FinalReportSource::Reconstructed
        }
    );
    assert!(matches!(events[2], AgentEvent::TurnEnd { .. }));
}

#[test]
fn a_turn_with_no_words_after_its_last_tool_call_has_no_final_report() {
    let mut session = ready();
    session.feed(&message_chunk(SESSION, "Let me look. "), 500);
    session.feed(&tool_call(SESSION, "c1", "Read a", "read"), 510);
    let step = session.feed(&prompt_response(2, "end_turn", None), 560);
    assert!(matches!(
        events_of(&step).as_slice(),
        [AgentEvent::Usage { .. }, AgentEvent::TurnEnd { .. }]
    ));
}

#[test]
fn a_turn_with_no_tool_call_reports_all_its_words() {
    let mut session = ready();
    session.feed(&message_chunk(SESSION, "All "), 500);
    session.feed(&message_chunk(SESSION, "of it."), 510);
    let step = session.feed(&prompt_response(2, "end_turn", None), 560);
    assert!(matches!(
        &events_of(&step)[1],
        AgentEvent::FinalReport { text, .. } if text == "All of it."
    ));
}

#[test]
fn an_error_response_gives_no_final_report_and_the_next_turn_starts_with_no_words() {
    let mut session = ready();
    session.feed(&message_chunk(SESSION, "half an answer"), 500);
    let step = session.feed(&prompt_error(2, -32603, "Internal error"), 560);
    assert!(matches!(
        events_of(&step).as_slice(),
        [AgentEvent::Usage { .. }, AgentEvent::TurnEnd { .. }]
    ));

    let next = session.send_turn("again");
    assert_eq!(written(&next)[0]["id"], json!(3));
    session.feed(&message_chunk(SESSION, "fresh"), 600);
    let step = session.feed(&prompt_response(3, "end_turn", None), 610);
    assert!(matches!(
        &events_of(&step)[1],
        AgentEvent::FinalReport { text, .. } if text == "fresh"
    ));
}

// ---- one turn of the fake child under the real shim ----------------------------------------------------------------

#[test]
fn a_prompt_turn_under_the_real_shim_gives_its_events_in_order_with_their_stored_times() {
    let mut played = play(
        &recording("prompt-turn"),
        AcpOptions::new("/work/repo", "read the notes"),
        &[],
        |_, _| Step::default(),
    );

    let kinds: Vec<&str> = played
        .agent_events()
        .iter()
        .map(|event| match event {
            AgentEvent::SessionBound { .. } => "bound",
            AgentEvent::Message { .. } => "message",
            AgentEvent::ToolCall { .. } => "tool_call",
            AgentEvent::Usage { .. } => "usage",
            AgentEvent::FinalReport { .. } => "final_report",
            AgentEvent::TurnEnd { .. } => "turn_end",
            other => panic!("an event the turn does not make: {other:?}"),
        })
        .collect();
    assert_eq!(
        kinds,
        [
            "bound",
            "message",
            "tool_call",
            "message",
            "usage",
            "final_report",
            "turn_end"
        ]
    );

    let events = played.agent_events();
    assert_eq!(
        events[0],
        AgentEvent::SessionBound {
            id: "ses-stand-in-1".into()
        }
    );
    assert!(matches!(
        &events[2],
        AgentEvent::ToolCall {
            kind: ToolKind::Read,
            ..
        }
    ));
    assert_eq!(
        events[5],
        AgentEvent::FinalReport {
            text: "The notes say hello.".into(),
            source: FinalReportSource::Reconstructed
        },
        "only the words after the tool call"
    );
    assert!(matches!(
        &events[6],
        AgentEvent::TurnEnd {
            reason: TurnEndReason::EndTurn {},
            cancel_requested: false,
            ..
        }
    ));

    // Each event carries the stored time of the journal record of the line that made it.
    let journal = read_journal(&played.journal);
    let stored: Vec<u64> = journal
        .records
        .iter()
        .filter(|record| record.direction == Direction::Out)
        .map(|record| record.wall_ns)
        .collect();
    let times: Vec<u64> = played
        .fed_events()
        .iter()
        .map(|stamped| stamped.wall_ns.unwrap())
        .collect();
    assert_eq!(times.len(), 7);
    assert!(times.iter().all(|time| stored.contains(time)), "{times:?}");
    assert!(times.windows(2).all(|pair| pair[0] <= pair[1]), "{times:?}");
    assert_eq!(
        times[0], stored[1],
        "the time of the `session/new` response"
    );

    assert_eq!(
        played.session.finish(&played.exit).outcome,
        PhaseOutcome::Ended
    );
}

#[test]
fn the_journal_of_a_prompt_turn_holds_the_three_requests_the_session_wrote() {
    let played = play(
        &recording("prompt-turn"),
        AcpOptions::new("/work/repo", "read the notes"),
        &[],
        |_, _| Step::default(),
    );
    let written: Vec<Value> = read_journal(&played.journal)
        .records
        .iter()
        .filter(|record| record.direction == Direction::In)
        .map(|record| serde_json::from_slice(&record.bytes).unwrap())
        .collect();
    let methods: Vec<(&str, i64)> = written
        .iter()
        .map(|m| (m["method"].as_str().unwrap(), m["id"].as_i64().unwrap()))
        .collect();
    assert_eq!(
        methods,
        [("initialize", 0), ("session/new", 1), ("session/prompt", 2)]
    );
    assert_eq!(written[1]["params"]["cwd"], "/work/repo");
    assert_eq!(
        written[2]["params"]["prompt"][0]["text"], "read the notes",
        "the first prompt is the options' own"
    );
}
