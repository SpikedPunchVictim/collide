//! Tests for the normalized agent events and the capability table (ts-0036): round-trips, `None` that is never 0,
//! the all-`unknown` table, the unknown-key cases, `Other`, and the generated schemas.

use collide_core::agent::{
    AgentEvent, CAPABILITY_NAMES, Capabilities, FinalReportSource, HookDecision, HookFailure,
    HookOutcome, Money, PermissionOption, PermissionOptionKind, PermissionReply, PermissionRequest,
    RefusalCause, SessionStart, ToolKind, TurnEndReason, Usage, UsageQuality, UsageScope,
};
use collide_core::schema;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::collections::BTreeSet;

fn round_trip<T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug>(
    value: T,
    wire: Value,
) {
    assert_eq!(serde_json::to_value(&value).unwrap(), wire, "written form");
    assert_eq!(
        serde_json::from_value::<T>(wire).unwrap(),
        value,
        "read form"
    );
}

fn schema_file(name: &str) -> Value {
    serde_json::from_str(&schema::registry().render()[name]).unwrap()
}

fn request() -> PermissionRequest {
    PermissionRequest {
        id: "r1".into(),
        tool_call_id: "c1".into(),
        tool: Some("Bash".into()),
        kind: ToolKind::Execute,
        input: Some(json!({ "command": "ls" })),
        reason: Some("outside the worktree".into()),
        options: vec![PermissionOption {
            id: "o1".into(),
            name: "Allow once".into(),
            kind: PermissionOptionKind::AllowOnce,
        }],
    }
}

fn usage() -> Usage {
    Usage {
        scope: UsageScope::RunningTotal,
        input: Some(10),
        output: Some(20),
        cache_read: Some(30),
        cache_write: Some(40),
        cache_write_1h: Some(40),
        cache_write_5m: Some(0),
        thought: Some(5),
        context_used: Some(1000),
        context_size: Some(200_000),
        cost: Some(Money {
            amount: 0.25,
            currency: "USD".into(),
        }),
        model_time_ms: Some(1300),
        quality: UsageQuality::Exact,
    }
}

// Round-trips.

#[test]
fn a_session_start_round_trips() {
    round_trip(
        SessionStart::Assign { uuid: "u-1".into() },
        json!({ "type": "assign", "uuid": "u-1" }),
    );
    round_trip(
        SessionStart::AgentAssigned {},
        json!({ "type": "agent_assigned" }),
    );
    round_trip(
        SessionStart::Resume { id: "s-9".into() },
        json!({ "type": "resume", "id": "s-9" }),
    );
}

#[test]
fn a_permission_request_round_trips() {
    round_trip(
        request(),
        json!({
            "id": "r1", "tool_call_id": "c1", "tool": "Bash", "kind": "execute",
            "input": { "command": "ls" }, "reason": "outside the worktree",
            "options": [{ "id": "o1", "name": "Allow once", "kind": "allow_once" }]
        }),
    );
}

#[test]
fn a_permission_request_without_input_has_none() {
    let read: PermissionRequest = serde_json::from_value(
        json!({ "id": "r1", "tool_call_id": "c1", "kind": "execute", "options": [] }),
    )
    .unwrap();
    assert_eq!(read.input, None);
    assert_eq!(read.tool, None);
    assert_eq!(read.reason, None);
    assert!(
        !serde_json::to_value(&read)
            .unwrap()
            .as_object()
            .unwrap()
            .contains_key("input")
    );
}

#[test]
fn the_permission_answers_round_trip() {
    round_trip(PermissionReply::Allow {}, json!({ "type": "allow" }));
    round_trip(PermissionReply::Cancel {}, json!({ "type": "cancel" }));
    round_trip(
        PermissionReply::Deny {
            message: Some("not here".into()),
        },
        json!({ "type": "deny", "message": "not here" }),
    );
    round_trip(
        PermissionReply::Deny { message: None },
        json!({ "type": "deny" }),
    );
}

#[test]
fn a_cancel_answer_parses() {
    let reply: PermissionReply = serde_json::from_str(r#"{"type":"cancel"}"#).unwrap();
    assert_eq!(reply, PermissionReply::Cancel {});
}

#[test]
fn every_agent_event_round_trips() {
    round_trip(
        AgentEvent::Message { text: "hi".into() },
        json!({ "type": "message", "text": "hi" }),
    );
    round_trip(
        AgentEvent::ToolCall {
            id: "c1".into(),
            tool: Some("Read".into()),
            kind: ToolKind::Read,
            input: Some(json!({ "path": "a" })),
        },
        json!({ "type": "tool_call", "id": "c1", "tool": "Read", "kind": "read", "input": { "path": "a" } }),
    );
    round_trip(
        AgentEvent::ToolCall {
            id: "c2".into(),
            tool: None,
            kind: ToolKind::Execute,
            input: None,
        },
        json!({ "type": "tool_call", "id": "c2", "kind": "execute" }),
    );
    round_trip(
        AgentEvent::PermissionRequest { request: request() },
        json!({ "type": "permission_request", "request": serde_json::to_value(request()).unwrap() }),
    );
    round_trip(
        AgentEvent::PermissionUnanswerable {
            request_id: "r1".into(),
        },
        json!({ "type": "permission_unanswerable", "request_id": "r1" }),
    );
    round_trip(
        AgentEvent::SessionBound { id: "s-1".into() },
        json!({ "type": "session_bound", "id": "s-1" }),
    );
    round_trip(
        AgentEvent::Usage { usage: usage() },
        json!({ "type": "usage", "usage": serde_json::to_value(usage()).unwrap() }),
    );
    round_trip(
        AgentEvent::TurnEnd {
            reason: TurnEndReason::EndTurn {},
            cancel_requested: false,
            empty: false,
        },
        json!({ "type": "turn_end", "reason": { "type": "end_turn" }, "cancel_requested": false, "empty": false }),
    );
    round_trip(
        AgentEvent::FinalReport {
            text: "done".into(),
            source: FinalReportSource::Reconstructed,
        },
        json!({ "type": "final_report", "text": "done", "source": "reconstructed" }),
    );
    round_trip(
        AgentEvent::Other {
            raw_type: "plan".into(),
            raw: Some(json!({ "steps": 2 })),
        },
        json!({ "type": "other", "raw_type": "plan", "raw": { "steps": 2 } }),
    );
}

#[test]
fn a_limited_event_round_trips_with_and_without_a_reset_time() {
    round_trip(
        AgentEvent::Limited {
            reset_at_ms: Some(1_763_000_000_000),
        },
        json!({ "type": "limited", "reset_at_ms": 1_763_000_000_000u64 }),
    );
    round_trip(
        AgentEvent::Limited { reset_at_ms: None },
        json!({ "type": "limited" }),
    );
}

#[test]
fn a_hook_call_round_trips_and_leaves_out_an_exit_code_it_was_not_given() {
    round_trip(
        AgentEvent::HookCall {
            name: "PreToolUse:Bash".into(),
            event: "PreToolUse".into(),
            outcome: HookOutcome::Error,
            exit_code: Some(2),
            decision: None,
        },
        json!({ "type": "hook_call", "name": "PreToolUse:Bash", "event": "PreToolUse", "outcome": "error", "exit_code": 2 }),
    );
    round_trip(
        AgentEvent::HookCall {
            name: "Stop".into(),
            event: "Stop".into(),
            outcome: HookOutcome::Cancelled,
            exit_code: None,
            decision: None,
        },
        json!({ "type": "hook_call", "name": "Stop", "event": "Stop", "outcome": "cancelled" }),
    );
}

#[test]
fn a_hook_call_has_no_place_for_the_hooks_output() {
    // A hook's stdout and stderr are untrusted text that may hold secrets: the event cannot carry them (0.3).
    for key in ["stdout", "stderr", "output"] {
        let mut wire =
            json!({ "type": "hook_call", "name": "n", "event": "e", "outcome": "success" });
        wire[key] = json!("secret");
        let error = serde_json::from_value::<AgentEvent>(wire)
            .expect_err(key)
            .to_string();
        assert!(error.contains(key), "{error}");
    }
}

#[test]
fn a_hook_call_carries_the_decision_its_output_gave_and_nothing_else_of_it() {
    for (decision, wire) in [
        (HookDecision::Allow, "allow"),
        (HookDecision::Deny, "deny"),
        (HookDecision::Ask, "ask"),
        (HookDecision::Block, "block"),
    ] {
        round_trip(
            AgentEvent::HookCall {
                name: "PreToolUse:Bash".into(),
                event: "PreToolUse".into(),
                outcome: HookOutcome::Success,
                exit_code: Some(0),
                decision: Some(decision),
            },
            json!({ "type": "hook_call", "name": "PreToolUse:Bash", "event": "PreToolUse",
                    "outcome": "success", "exit_code": 0, "decision": wire }),
        );
        round_trip(decision, json!(wire));
    }
}

#[test]
fn a_hook_that_failed_open_round_trips_with_each_cause() {
    for (cause, wire) in [
        (HookFailure::ExitError, "exit_error"),
        (HookFailure::NoVerdict, "no_verdict"),
    ] {
        round_trip(
            AgentEvent::HookFailedOpen {
                name: "PreToolUse:Bash".into(),
                event: "PreToolUse".into(),
                tool_call_id: "toolu_01".into(),
                cause,
            },
            json!({ "type": "hook_failed_open", "name": "PreToolUse:Bash", "event": "PreToolUse",
                    "tool_call_id": "toolu_01", "cause": wire }),
        );
        round_trip(cause, json!(wire));
    }
}

#[test]
fn a_hook_that_failed_open_has_no_place_for_the_hooks_output() {
    for key in ["stdout", "stderr", "output"] {
        let mut wire = json!({ "type": "hook_failed_open", "name": "n", "event": "e",
                               "tool_call_id": "c", "cause": "exit_error" });
        wire[key] = json!("secret");
        let error = serde_json::from_value::<AgentEvent>(wire)
            .expect_err(key)
            .to_string();
        assert!(error.contains(key), "{error}");
    }
}

#[test]
fn every_hook_outcome_round_trips() {
    round_trip(HookOutcome::Success, json!("success"));
    round_trip(HookOutcome::Error, json!("error"));
    round_trip(HookOutcome::Cancelled, json!("cancelled"));
}

#[test]
fn a_refused_tool_call_round_trips_with_every_cause() {
    for (cause, wire) in [
        (RefusalCause::Hook, "hook"),
        (RefusalCause::Host, "host"),
        (RefusalCause::Rule, "rule"),
        (RefusalCause::MissingTool, "missing_tool"),
        (RefusalCause::Unknown, "unknown"),
    ] {
        round_trip(
            AgentEvent::ToolRefused {
                id: "c1".into(),
                cause,
            },
            json!({ "type": "tool_refused", "id": "c1", "cause": wire }),
        );
        round_trip(cause, json!(wire));
    }
}

#[test]
fn the_size_of_a_tool_result_round_trips_and_has_no_place_for_its_content() {
    round_trip(
        AgentEvent::ToolResultSize {
            tool_call_id: "c1".into(),
            bytes: 2048,
        },
        json!({ "type": "tool_result_size", "tool_call_id": "c1", "bytes": 2048 }),
    );
    let error = serde_json::from_value::<AgentEvent>(
        json!({ "type": "tool_result_size", "tool_call_id": "c1", "bytes": 1, "content": "text" }),
    )
    .unwrap_err()
    .to_string();
    assert!(error.contains("content"), "{error}");
}

#[test]
fn a_turn_blocked_by_a_hook_round_trips_with_its_reason() {
    round_trip(
        TurnEndReason::Blocked {
            reason: Some("not on a Friday".into()),
        },
        json!({ "type": "blocked", "reason": "not on a Friday" }),
    );
    round_trip(
        TurnEndReason::Blocked { reason: None },
        json!({ "type": "blocked" }),
    );
}

#[test]
fn a_compaction_call_is_a_usage_with_its_own_scope() {
    round_trip(UsageScope::Compaction, json!("compaction"));
    let compaction = Usage {
        scope: UsageScope::Compaction,
        input: Some(90_000),
        output: Some(2_000),
        ..usage()
    };
    round_trip(
        AgentEvent::Usage {
            usage: compaction.clone(),
        },
        json!({ "type": "usage", "usage": serde_json::to_value(&compaction).unwrap() }),
    );
}

#[test]
fn every_turn_end_reason_round_trips() {
    round_trip(TurnEndReason::EndTurn {}, json!({ "type": "end_turn" }));
    round_trip(TurnEndReason::MaxTokens {}, json!({ "type": "max_tokens" }));
    round_trip(TurnEndReason::MaxTurns {}, json!({ "type": "max_turns" }));
    round_trip(TurnEndReason::Refusal {}, json!({ "type": "refusal" }));
    round_trip(TurnEndReason::Cancelled {}, json!({ "type": "cancelled" }));
    round_trip(
        TurnEndReason::Error { status: Some(429) },
        json!({ "type": "error", "status": 429 }),
    );
    round_trip(
        TurnEndReason::Error { status: None },
        json!({ "type": "error" }),
    );
}

#[test]
fn both_final_report_sources_round_trip() {
    round_trip(FinalReportSource::ResultField, json!("result_field"));
    round_trip(FinalReportSource::Reconstructed, json!("reconstructed"));
}

#[test]
fn every_usage_scope_and_quality_round_trips() {
    round_trip(UsageScope::Turn, json!("turn"));
    round_trip(UsageScope::Step, json!("step"));
    round_trip(UsageScope::RunningTotal, json!("running_total"));
    round_trip(UsageScope::Compaction, json!("compaction"));
    round_trip(UsageScope::OtherCalls, json!("other_calls"));
    round_trip(UsageQuality::Exact, json!("exact"));
    round_trip(UsageQuality::Partial, json!("partial"));
    round_trip(UsageQuality::Unreported, json!("unreported"));
}

// Usage: a number that is absent is None, never 0.

#[test]
fn an_absent_usage_number_reads_as_none() {
    let read: Usage =
        serde_json::from_value(json!({ "scope": "turn", "quality": "unreported" })).unwrap();
    assert_eq!(read.input, None);
    assert_eq!(read.output, None);
    assert_eq!(read.cache_read, None);
    assert_eq!(read.cache_write, None);
    assert_eq!(read.thought, None);
    assert_eq!(read.context_used, None);
    assert_eq!(read.context_size, None);
    assert_eq!(read.cost, None);
    assert_eq!(read.model_time_ms, None);
}

#[test]
fn an_absent_usage_number_is_written_as_absent_not_zero() {
    let usage = Usage {
        scope: UsageScope::Turn,
        input: None,
        output: Some(0),
        cache_read: None,
        cache_write: None,
        cache_write_1h: None,
        cache_write_5m: None,
        thought: None,
        context_used: None,
        context_size: None,
        cost: None,
        model_time_ms: None,
        quality: UsageQuality::Partial,
    };
    assert_eq!(
        serde_json::to_value(&usage).unwrap(),
        json!({ "scope": "turn", "output": 0, "quality": "partial" }),
        "a reported 0 is kept, an absent number is left out, the model time included"
    );
}

#[test]
fn a_null_usage_number_reads_as_none() {
    let read: Usage = serde_json::from_value(
        json!({ "scope": "step", "input": null, "cost": null, "quality": "partial" }),
    )
    .unwrap();
    assert_eq!((read.input, read.cost), (None, None));
}

#[test]
fn the_usage_schema_makes_every_number_optional_and_nullable_with_no_zero_default() {
    for file in ["Usage.input.json", "Usage.output.json"] {
        let schema = schema_file(file);
        let required: BTreeSet<&str> = schema["required"]
            .as_array()
            .unwrap()
            .iter()
            .map(|name| name.as_str().unwrap())
            .collect();
        assert_eq!(required, BTreeSet::from(["scope", "quality"]), "{file}");
        for number in [
            "input",
            "output",
            "cache_read",
            "cache_write",
            "cache_write_1h",
            "cache_write_5m",
            "thought",
            "context_used",
            "context_size",
            "model_time_ms",
        ] {
            let property = &schema["properties"][number];
            assert_eq!(
                property["type"],
                json!(["integer", "null"]),
                "{file} {number}"
            );
            assert_eq!(property["minimum"], json!(0), "{file} {number}");
            assert!(
                property.get("default").is_none_or(Value::is_null),
                "{file} {number}"
            );
        }
        assert!(
            schema["properties"]["cost"]
                .get("default")
                .is_none_or(Value::is_null),
            "{file} cost"
        );
    }
}

// Unknown keys, and Other.

#[test]
fn an_unknown_key_fails_in_every_known_event() {
    let events = [
        json!({ "type": "message", "text": "hi" }),
        json!({ "type": "tool_call", "id": "c", "kind": "read" }),
        json!({ "type": "permission_request", "request": serde_json::to_value(request()).unwrap() }),
        json!({ "type": "permission_unanswerable", "request_id": "r" }),
        json!({ "type": "session_bound", "id": "s" }),
        json!({ "type": "usage", "usage": serde_json::to_value(usage()).unwrap() }),
        json!({ "type": "turn_end", "reason": { "type": "end_turn" }, "cancel_requested": false, "empty": true }),
        json!({ "type": "final_report", "text": "t", "source": "result_field" }),
        json!({ "type": "limited", "reset_at_ms": 1 }),
        json!({ "type": "hook_call", "name": "n", "event": "e", "outcome": "success", "exit_code": 0 }),
        json!({ "type": "tool_refused", "id": "c", "cause": "hook" }),
        json!({ "type": "tool_result_size", "tool_call_id": "c", "bytes": 3 }),
        json!({ "type": "hook_failed_open", "name": "n", "event": "e", "tool_call_id": "c", "cause": "no_verdict" }),
        json!({ "type": "other", "raw_type": "x" }),
    ];
    for mut event in events {
        assert!(
            serde_json::from_value::<AgentEvent>(event.clone()).is_ok(),
            "{event}"
        );
        event["surprise"] = json!(1);
        let error = serde_json::from_value::<AgentEvent>(event.clone())
            .expect_err(&format!("should not parse: {event}"))
            .to_string();
        assert!(error.contains("surprise"), "{error}");
    }
}

#[test]
fn an_unknown_key_fails_inside_a_nested_type() {
    let mut nested = serde_json::to_value(request()).unwrap();
    nested["options"][0]["surprise"] = json!(1);
    let event = json!({ "type": "permission_request", "request": nested });
    assert!(serde_json::from_value::<AgentEvent>(event).is_err());

    let mut usage = serde_json::to_value(usage()).unwrap();
    usage["cost"]["surprise"] = json!(1);
    assert!(serde_json::from_value::<Usage>(usage).is_err());

    let reason = json!({ "type": "error", "status": 500, "surprise": 1 });
    assert!(serde_json::from_value::<TurnEndReason>(reason).is_err());
}

#[test]
fn an_unknown_key_fails_on_an_empty_variant() {
    assert!(serde_json::from_value::<PermissionReply>(json!({ "type": "allow", "x": 1 })).is_err());
    assert!(
        serde_json::from_value::<PermissionReply>(json!({ "type": "cancel", "x": 1 })).is_err()
    );
    assert!(
        serde_json::from_value::<SessionStart>(json!({ "type": "agent_assigned", "x": 1 }))
            .is_err()
    );
    assert!(
        serde_json::from_value::<TurnEndReason>(json!({ "type": "end_turn", "x": 1 })).is_err()
    );
}

#[test]
fn an_unknown_key_fails_in_the_other_types() {
    assert!(serde_json::from_value::<Capabilities>(json!({ "surprise": "yes" })).is_err());
    assert!(
        serde_json::from_value::<Capabilities>(json!({ "permission_host": { "surprise": "yes" } }))
            .is_err()
    );
    assert!(
        serde_json::from_value::<Usage>(
            json!({ "scope": "turn", "quality": "exact", "tokens": 1 })
        )
        .is_err()
    );
    assert!(
        serde_json::from_value::<PermissionRequest>(
            json!({ "id": "r", "tool_call_id": "c", "kind": "read", "surprise": 1 })
        )
        .is_err()
    );
}

#[test]
fn an_unrecognized_event_reads_as_other_with_its_raw_type_name() {
    let read: AgentEvent = serde_json::from_value(
        json!({ "type": "compact_boundary", "pre_tokens": 120000, "trigger": "auto" }),
    )
    .unwrap();
    assert_eq!(
        read,
        AgentEvent::Other {
            raw_type: "compact_boundary".into(),
            raw: Some(json!({ "pre_tokens": 120000, "trigger": "auto" })),
        }
    );
}

#[test]
fn an_unrecognized_event_with_no_other_keys_reads_as_other() {
    let read: AgentEvent = serde_json::from_value(json!({ "type": "ping" })).unwrap();
    assert_eq!(
        read,
        AgentEvent::Other {
            raw_type: "ping".into(),
            raw: Some(json!({})),
        }
    );
}

#[test]
fn other_is_stable_when_written_and_read_again() {
    let first: AgentEvent = serde_json::from_value(json!({ "type": "ping", "n": 1 })).unwrap();
    let written = serde_json::to_value(&first).unwrap();
    assert_eq!(written["type"], "other");
    assert_eq!(written["raw_type"], "ping");
    let second: AgentEvent = serde_json::from_value(written).unwrap();
    assert_eq!(first, second);
}

#[test]
fn a_malformed_event_fails_with_a_message() {
    for bad in [
        json!({}),
        json!({ "type": 3 }),
        json!("message"),
        json!(null),
    ] {
        assert!(
            serde_json::from_value::<AgentEvent>(bad.clone()).is_err(),
            "{bad}"
        );
    }
    let known_with_missing_field = json!({ "type": "message" });
    let error = serde_json::from_value::<AgentEvent>(known_with_missing_field)
        .unwrap_err()
        .to_string();
    assert!(error.contains("text"), "{error}");
}

#[test]
fn every_known_event_type_in_the_schema_reads_as_itself_not_as_other() {
    // A type added to the enum but left out of the reader's list would read as `Other`.
    let schema = schema_file("AgentEvent.input.json");
    let tags: BTreeSet<String> = schema["oneOf"]
        .as_array()
        .unwrap()
        .iter()
        .map(|variant| {
            variant["properties"]["type"]["const"]
                .as_str()
                .unwrap()
                .to_string()
        })
        .collect();
    assert_eq!(tags.len(), 14);
    for tag in &tags {
        let bare = json!({ "type": tag });
        if let Ok(AgentEvent::Other { raw_type, .. }) = serde_json::from_value::<AgentEvent>(bare) {
            assert_eq!(tag, "other", "`{tag}` reads as Other (raw_type {raw_type})");
        }
    }
}

// The capability table.

#[test]
fn the_default_table_is_all_unknown_and_lists_all_18_capabilities() {
    let written = serde_json::to_value(Capabilities::default()).unwrap();
    let keys: BTreeSet<&str> = written
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(keys, BTreeSet::from(CAPABILITY_NAMES));
    assert_eq!(CAPABILITY_NAMES.len(), 18);

    fn leaves(value: &Value, path: &str, found: &mut Vec<(String, Value)>) {
        match value {
            Value::Object(map) => map
                .iter()
                .for_each(|(key, child)| leaves(child, &format!("{path}.{key}"), found)),
            other => found.push((path.to_string(), other.clone())),
        }
    }
    let mut found = Vec::new();
    leaves(&written, "", &mut found);
    for (path, leaf) in found {
        assert!(
            leaf == json!("unknown") || leaf == json!([]) || leaf.is_null(),
            "{path} is {leaf}"
        );
    }
}

#[test]
fn an_empty_table_reads_as_all_unknown() {
    let read: Capabilities = serde_json::from_value(json!({})).unwrap();
    assert_eq!(read, Capabilities::default());
}

#[test]
fn a_capability_that_is_left_out_is_unknown() {
    let read: Capabilities = serde_json::from_value(json!({
        "headless_stream": "yes",
        "permission_host": { "support": "yes", "deny_message": "no" },
        "session_resume": "silent"
    }))
    .unwrap();
    assert_eq!(read.headless_stream, collide_core::agent::Support::Yes);
    assert_eq!(
        read.permission_host.deny_message,
        collide_core::agent::Support::No
    );
    assert_eq!(
        read.permission_host.input_at_request,
        collide_core::agent::Support::Unknown
    );
    assert_eq!(read.background, collide_core::agent::Support::Unknown);
    assert_eq!(
        read.session_resume,
        collide_core::agent::SessionResume::Silent
    );
    assert_eq!(
        read.system_prompt,
        collide_core::agent::SystemPrompt::Unknown
    );
}

#[test]
fn a_full_table_round_trips() {
    let table: Capabilities = serde_json::from_value(json!({
        "headless_stream": "yes",
        "permission_host": { "support": "yes", "deny_message": "yes", "input_at_request": "yes", "request_reason": "no" },
        "session_resume": "replay",
        "background": "no",
        "budget_native": "no",
        "system_prompt": "append",
        "tool_allowlist": "yes",
        "named_agent": "no",
        "usage_report": { "support": "yes", "granularity": ["turn", "step"], "cost": "no",
            "cache_write_tiers": "no", "context": "yes", "quality": "partial" },
        "limit_signal": "no",
        "compaction_events": "unknown",
        "hook_events": "no",
        "refusal_source": "no",
        "strict_config": "unknown",
        "context_options": { "support": "no", "compact_at": "no", "bash_output_max": "no", "mcp_output_max": "no" },
        "model_select": "session",
        "take_over": "yes",
        "mcp_servers": { "support": "yes", "stdio": "yes", "http": "yes", "sse": "yes" }
    }))
    .unwrap();
    let written = serde_json::to_value(&table).unwrap();
    assert_eq!(
        serde_json::from_value::<Capabilities>(written).unwrap(),
        table
    );
}

#[test]
fn a_value_outside_the_capability_vocabulary_fails() {
    assert!(serde_json::from_value::<Capabilities>(json!({ "background": "maybe" })).is_err());
    assert!(serde_json::from_value::<Capabilities>(json!({ "session_resume": "yes" })).is_err());
    assert!(serde_json::from_value::<Capabilities>(json!({ "model_select": "Launch" })).is_err());
}

// The generated schemas.

#[test]
fn the_registry_has_the_new_types_and_each_object_is_closed() {
    let files = schema::registry().render();
    for name in [
        "AgentEvent",
        "Capabilities",
        "PermissionReply",
        "PermissionRequest",
        "SessionStart",
        "Usage",
    ] {
        for side in ["input", "output"] {
            assert!(
                files.contains_key(&format!("{name}.{side}.json")),
                "{name}.{side}"
            );
        }
    }
}

#[test]
fn the_capabilities_schema_lists_the_18_names() {
    let schema = schema_file("Capabilities.input.json");
    let names: BTreeSet<&str> = schema["properties"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(names, BTreeSet::from(CAPABILITY_NAMES));
    assert_eq!(schema["additionalProperties"], json!(false));
    assert!(
        schema
            .get("required")
            .is_none_or(|required| required.as_array().unwrap().is_empty())
    );
}

#[test]
fn an_empty_variant_is_an_empty_object_in_the_schema() {
    let schema = schema_file("PermissionReply.input.json");
    let allow = schema["oneOf"]
        .as_array()
        .unwrap()
        .iter()
        .find(|variant| variant["properties"]["type"]["const"] == "allow")
        .unwrap();
    assert_eq!(allow["additionalProperties"], json!(false));
    assert_eq!(allow["properties"].as_object().unwrap().len(), 1);
}
