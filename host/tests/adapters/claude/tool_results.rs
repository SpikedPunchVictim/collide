//! The size of each tool result (ts-0064-hw; 0.13, Token and context metrics: tool-result bytes). The event holds the
//! size and the call's id, never the content, which is untrusted and may be large.

use super::fake::{assigned, play};
use super::session::{allow_all, events, line, named, session};
use collide_core::adapter::AgentSession;
use collide_core::agent::AgentEvent;
use serde_json::{Value, json};

fn result_of(id: &str, content: Value) -> Vec<u8> {
    line(
        json!({"type": "user", "message": {"role": "user", "content": [
        {"type": "tool_result", "tool_use_id": id, "is_error": false, "content": content}]}}),
    )
}

fn sizes(events: &[AgentEvent]) -> Vec<(String, u64)> {
    events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::ToolResultSize {
                tool_call_id,
                bytes,
            } => Some((tool_call_id.clone(), *bytes)),
            _ => None,
        })
        .collect()
}

#[test]
fn a_text_result_is_its_length_in_bytes_not_in_characters() {
    let mut session = session();
    let step = session.feed(&result_of("toolu_1", json!("héllo")), 1);
    assert_eq!(sizes(&events(&step)), [("toolu_1".to_string(), 6)]);
}

#[test]
fn a_result_of_content_blocks_is_the_text_of_its_text_blocks_and_the_json_of_any_other() {
    let mut session = session();
    let image = json!({"type": "image", "source": {"type": "base64", "data": "AAAA"}});
    let expected = 3 + 2 + serde_json::to_string(&image).unwrap().len() as u64;
    let content = json!([{"type": "text", "text": "abc"}, {"type": "text", "text": "de"}, image]);
    let step = session.feed(&result_of("toolu_1", content), 1);
    assert_eq!(sizes(&events(&step)), [("toolu_1".to_string(), expected)]);
}

#[test]
fn an_empty_result_is_zero_bytes_and_a_result_with_no_content_has_no_size() {
    let mut session = session();
    let empty = session.feed(&result_of("toolu_1", json!("")), 1);
    assert_eq!(sizes(&events(&empty)), [("toolu_1".to_string(), 0)]);
    let none = line(json!({"type": "user", "message": {"content": [
        {"type": "tool_result", "tool_use_id": "toolu_2", "is_error": false}]}}));
    assert!(sizes(&events(&session.feed(&none, 1))).is_empty());
}

#[test]
fn several_results_in_one_line_each_have_a_size_in_order() {
    let mut session = session();
    let user = line(json!({"type": "user", "message": {"content": [
        {"type": "tool_result", "tool_use_id": "a", "content": "xx"},
        {"type": "tool_result", "tool_use_id": "b", "content": "yyyy"}]}}));
    assert_eq!(
        sizes(&events(&session.feed(&user, 1))),
        [("a".to_string(), 2), ("b".to_string(), 4)]
    );
}

#[test]
fn a_refused_calls_result_has_a_size_too_before_the_refusal() {
    let mut session = session();
    let user = line(json!({"type": "user", "message": {"content": [
        {"type": "tool_result", "tool_use_id": "a", "is_error": true, "content": "Refused."}]},
        "tool_result_meta": [{"id": "a", "non_execution_kind": "permission_denied",
            "permission_decision": {"decision": "reject", "source": "hook"}}]}));
    let all = events(&session.feed(&user, 1));
    assert!(matches!(
        &all[..],
        [
            AgentEvent::ToolResultSize { bytes: 8, .. },
            AgentEvent::ToolRefused { .. }
        ]
    ));
}

#[test]
fn a_line_with_no_tool_result_has_no_size() {
    let mut session = session();
    let user =
        line(json!({"type": "user", "message": {"content": [{"type": "text", "text": "hi"}]}}));
    assert!(sizes(&events(&session.feed(&user, 1))).is_empty());
}

#[test]
fn the_event_has_no_place_for_the_content() {
    let mut session = session();
    let step = session.feed(&result_of("toolu_1", json!("SECRET-CONTENT-MARKER")), 1);
    let written = serde_json::to_string(&events(&step)).unwrap();
    assert!(!written.contains("SECRET"), "{written}");
}

#[test]
fn the_report_recording_gives_the_size_of_each_of_its_two_results() {
    let played = play("report", assigned(), &["go"], allow_all);
    let all = named(&played);
    assert_eq!(
        sizes(&all).len(),
        played
            .agent_events()
            .iter()
            .filter(|e| matches!(e, AgentEvent::ToolCall { .. }))
            .count()
    );
    assert!(!sizes(&all).is_empty());
}
