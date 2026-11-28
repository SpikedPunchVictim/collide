//! Tests for the ask contract (ts-0034-hh): the methods `ask.start`, `ask.cancel` and `ask.test`, the two shapes of
//! `ask.start`, the `ask.*` events, the outcome, and the result of Test this agent with its confinement probe.

use collide_core::agent::{
    AgentEvent, FinalReportSource, Money, PermissionRequest, ToolKind, TurnEndReason, Usage,
    UsageQuality, UsageScope,
};
use collide_core::ask::{
    AskCancelParams, AskCost, AskEnd, AskEventNotice, AskMethod, AskNotice, AskOutcome,
    AskOutcomeNotice, AskStartDaemonParams, AskStartParams, AskTestDaemonParams, AskTestParams,
    AskTestResult, ConfinementProbe,
};
use collide_core::non_empty::NonEmpty;
use collide_core::owner::{Ack, Method, Request, RequestId, code, route};
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

fn text(s: &str) -> NonEmpty {
    NonEmpty::new(s).unwrap()
}

// Methods.

#[test]
fn the_three_methods_are_start_cancel_and_test() {
    let names: Vec<&str> = AskMethod::ALL.iter().map(|m| m.name()).collect();
    assert_eq!(names, ["ask.start", "ask.cancel", "ask.test"]);
    for method in AskMethod::ALL {
        assert_eq!(AskMethod::from_name(method.name()), Some(*method));
    }
    assert_eq!(AskMethod::from_name("ask.event"), None);
    assert_eq!(AskMethod::from_name("daemon.status"), None);
}

#[test]
fn the_owner_protocol_does_not_know_an_ask_method_yet() {
    for method in AskMethod::ALL {
        let request = Request {
            jsonrpc: Default::default(),
            id: RequestId::Number(1),
            method: method.name().to_string(),
            params: json!({}),
        };
        let error = route(true, &request).unwrap_err();
        assert_eq!(error.code, code::METHOD_NOT_FOUND, "{}", method.name());
        assert!(Method::from_name(method.name()).is_none());
    }
}

#[test]
fn every_method_names_its_schemas_and_each_is_registered() {
    let names: Vec<(String, String, String)> = AskMethod::ALL
        .iter()
        .map(|m| {
            (
                m.window_params_schema(),
                m.params_schema(),
                m.result_schema(),
            )
        })
        .collect();
    let owned = |a: &str, b: &str, c: &str| (a.to_string(), b.to_string(), c.to_string());
    assert_eq!(
        names,
        [
            owned("AskStartParams", "AskStartDaemonParams", "Ack"),
            owned("AskCancelParams", "AskCancelParams", "Ack"),
            owned("AskTestParams", "AskTestDaemonParams", "AskTestResult"),
        ]
    );
    let files = schema::registry().render();
    for (window, daemon, result) in names {
        for name in [window, daemon, result] {
            assert!(files.contains_key(&format!("{name}.input.json")), "{name}");
            assert!(files.contains_key(&format!("{name}.output.json")), "{name}");
        }
    }
    for name in ["AskNotice", "AskOutcome"] {
        assert!(files.contains_key(&format!("{name}.output.json")), "{name}");
    }
}

// The two shapes of ask.start.

#[test]
fn the_windows_start_is_an_ask_an_agent_and_a_request() {
    round_trip(
        AskStartParams {
            ask: text("ask-1"),
            agent: text("claude-haiku"),
            request: text("show me the slowest tracks"),
        },
        json!({ "ask": "ask-1", "agent": "claude-haiku", "request": "show me the slowest tracks" }),
    );
}

#[test]
fn the_windows_start_refuses_a_token_an_endpoint_and_any_unknown_key() {
    let base = json!({ "ask": "a", "agent": "b", "request": "c" });
    assert!(serde_json::from_value::<AskStartParams>(base.clone()).is_ok());
    for key in [
        "token",
        "endpoint",
        "ask_token",
        "endpointToken",
        "actor",
        "x",
    ] {
        let mut wire = base.clone();
        wire[key] = json!("v");
        assert!(
            serde_json::from_value::<AskStartParams>(wire).is_err(),
            "the window's ask.start took `{key}`"
        );
    }
}

#[test]
fn the_daemons_start_adds_the_endpoint_and_the_token() {
    round_trip(
        AskStartDaemonParams {
            ask: text("ask-1"),
            agent: text("claude-haiku"),
            request: text("sort"),
            endpoint: text("http://127.0.0.1:7777/mcp"),
            token: text("secret-ask-token"),
        },
        json!({
            "ask": "ask-1", "agent": "claude-haiku", "request": "sort",
            "endpoint": "http://127.0.0.1:7777/mcp", "token": "secret-ask-token"
        }),
    );
}

#[test]
fn the_daemons_start_needs_both_and_refuses_an_unknown_key() {
    let full = json!({
        "ask": "a", "agent": "b", "request": "c", "endpoint": "http://127.0.0.1:1/mcp", "token": "t"
    });
    assert!(serde_json::from_value::<AskStartDaemonParams>(full.clone()).is_ok());
    for key in ["endpoint", "token"] {
        let mut wire = full.clone();
        wire.as_object_mut().unwrap().remove(key);
        assert!(
            serde_json::from_value::<AskStartDaemonParams>(wire).is_err(),
            "the daemon's ask.start read without `{key}`"
        );
    }
    let mut wire = full;
    wire["x"] = json!(1);
    assert!(serde_json::from_value::<AskStartDaemonParams>(wire).is_err());
}

#[test]
fn an_empty_id_request_endpoint_or_token_is_refused() {
    for key in ["ask", "agent", "request"] {
        let mut wire = json!({ "ask": "a", "agent": "b", "request": "c" });
        wire[key] = json!("");
        assert!(
            serde_json::from_value::<AskStartParams>(wire).is_err(),
            "{key}"
        );
    }
    for key in ["endpoint", "token"] {
        let mut wire = json!({
            "ask": "a", "agent": "b", "request": "c", "endpoint": "e", "token": "t"
        });
        wire[key] = json!("");
        assert!(
            serde_json::from_value::<AskStartDaemonParams>(wire).is_err(),
            "{key}"
        );
    }
}

#[test]
fn the_daemons_start_does_not_print_its_token() {
    let params = AskStartDaemonParams {
        ask: text("ask-1"),
        agent: text("a"),
        request: text("r"),
        endpoint: text("http://127.0.0.1:1/mcp"),
        token: text("secret-ask-token"),
    };
    let shown = format!("{params:?}");
    assert!(!shown.contains("secret-ask-token"), "{shown}");
    let test = AskTestDaemonParams {
        ask: text("ask-2"),
        agent: text("a"),
        endpoint: text("http://127.0.0.1:1/mcp"),
        token: text("secret-ask-token"),
    };
    assert!(!format!("{test:?}").contains("secret-ask-token"));
}

// cancel and test.

#[test]
fn cancel_takes_an_ask_and_answers_an_ack() {
    round_trip(
        AskCancelParams { ask: text("ask-1") },
        json!({ "ask": "ask-1" }),
    );
    round_trip(Ack {}, json!({}));
    assert!(serde_json::from_value::<AskCancelParams>(json!({})).is_err());
    assert!(
        serde_json::from_value::<AskCancelParams>(json!({ "ask": "a", "reason": "x" })).is_err()
    );
}

#[test]
fn test_has_a_window_shape_and_a_daemon_shape_with_the_endpoint_and_token() {
    round_trip(
        AskTestParams {
            ask: text("t-1"),
            agent: text("opencode"),
        },
        json!({ "ask": "t-1", "agent": "opencode" }),
    );
    assert!(
        serde_json::from_value::<AskTestParams>(json!({ "ask": "t", "agent": "a", "token": "x" }))
            .is_err()
    );
    round_trip(
        AskTestDaemonParams {
            ask: text("t-1"),
            agent: text("opencode"),
            endpoint: text("http://127.0.0.1:7777/mcp"),
            token: text("tok"),
        },
        json!({ "ask": "t-1", "agent": "opencode", "endpoint": "http://127.0.0.1:7777/mcp", "token": "tok" }),
    );
    assert!(
        serde_json::from_value::<AskTestDaemonParams>(json!({ "ask": "t", "agent": "a" })).is_err()
    );
}

// The outcome.

fn usd(amount: f64) -> Money {
    Money {
        amount,
        currency: "USD".to_string(),
    }
}

#[test]
fn each_of_the_six_ends_round_trips() {
    round_trip(AskEnd::Finished {}, json!({ "type": "finished" }));
    round_trip(AskEnd::Cancelled {}, json!({ "type": "cancelled" }));
    round_trip(
        AskEnd::TimedOut { after_s: 180 },
        json!({ "type": "timed_out", "after_s": 180 }),
    );
    round_trip(
        AskEnd::StoppedAtBudget { budget: usd(0.05) },
        json!({ "type": "stopped_at_budget", "budget": { "amount": 0.05, "currency": "USD" } }),
    );
    round_trip(
        AskEnd::Failed {
            text: "claude was not found on PATH".to_string(),
        },
        json!({ "type": "failed", "text": "claude was not found on PATH" }),
    );
    round_trip(AskEnd::Lost {}, json!({ "type": "lost" }));
}

#[test]
fn an_end_refuses_an_unknown_key_and_an_unknown_type() {
    assert!(serde_json::from_value::<AskEnd>(json!({ "type": "finished", "x": 1 })).is_err());
    assert!(serde_json::from_value::<AskEnd>(json!({ "type": "lost", "x": 1 })).is_err());
    assert!(serde_json::from_value::<AskEnd>(json!({ "type": "exploded" })).is_err());
}

#[test]
fn the_outcome_carries_the_cost_and_the_usage_quality() {
    round_trip(
        AskOutcome {
            end: AskEnd::Finished {},
            cost: AskCost::Reported { cost: usd(0.0042) },
            quality: UsageQuality::Exact,
        },
        json!({
            "end": { "type": "finished" },
            "cost": { "type": "reported", "cost": { "amount": 0.0042, "currency": "USD" } },
            "quality": "exact"
        }),
    );
}

#[test]
fn a_cost_that_is_not_reported_is_its_own_value_and_never_zero() {
    round_trip(
        AskOutcome {
            end: AskEnd::Finished {},
            cost: AskCost::NotReported {},
            quality: UsageQuality::Unreported,
        },
        json!({
            "end": { "type": "finished" },
            "cost": { "type": "not_reported" },
            "quality": "unreported"
        }),
    );
    let zero = serde_json::to_value(AskCost::Reported { cost: usd(0.0) }).unwrap();
    let unknown = serde_json::to_value(AskCost::NotReported {}).unwrap();
    assert_ne!(
        zero, unknown,
        "a cost of 0 is not a cost that is not reported"
    );
    assert!(serde_json::from_value::<AskCost>(json!(0)).is_err());
    assert!(serde_json::from_value::<AskCost>(json!(null)).is_err());
}

#[test]
fn an_outcome_needs_its_cost_and_quality() {
    assert!(
        serde_json::from_value::<AskOutcome>(json!({ "end": { "type": "finished" } })).is_err()
    );
    assert!(
        serde_json::from_value::<AskOutcome>(json!({
            "end": { "type": "finished" },
            "cost": { "type": "not_reported" },
            "quality": "unreported",
            "token": "t"
        }))
        .is_err()
    );
}

// The events.

fn every_normalized_event() -> Vec<AgentEvent> {
    vec![
        AgentEvent::Message {
            text: "reading".to_string(),
        },
        AgentEvent::ToolCall {
            id: "c1".to_string(),
            tool: Some("Read".to_string()),
            kind: ToolKind::Read,
            input: Some(json!({ "file_path": "a.rs" })),
        },
        AgentEvent::PermissionRequest {
            request: PermissionRequest {
                id: "p1".to_string(),
                tool_call_id: "c1".to_string(),
                tool: None,
                kind: ToolKind::Read,
                input: None,
                reason: None,
                options: vec![],
            },
        },
        AgentEvent::PermissionUnanswerable {
            request_id: "p1".to_string(),
        },
        AgentEvent::SessionBound {
            id: "s1".to_string(),
        },
        AgentEvent::Usage {
            usage: Usage {
                scope: UsageScope::Turn,
                input: Some(10),
                output: None,
                cache_read: None,
                cache_write: None,
                cache_write_1h: None,
                cache_write_5m: None,
                thought: None,
                context_used: None,
                context_size: None,
                cost: Some(usd(0.001)),
                model_time_ms: None,
                quality: UsageQuality::Partial,
            },
        },
        AgentEvent::TurnEnd {
            reason: TurnEndReason::EndTurn {},
            cancel_requested: false,
            empty: false,
        },
        AgentEvent::FinalReport {
            text: "done".to_string(),
            source: FinalReportSource::ResultField,
        },
        AgentEvent::Other {
            raw_type: "newer".to_string(),
            raw: Some(json!({ "k": 1 })),
        },
    ]
}

#[test]
fn an_ask_event_embeds_every_normalized_event() {
    let events = every_normalized_event();
    assert_eq!(events.len(), 9, "one of each AgentEvent variant");
    for event in events {
        let notice = AskNotice::Event(Box::new(AskEventNotice {
            ask: text("ask-1"),
            event: event.clone(),
        }));
        let wire = serde_json::to_value(&notice).unwrap();
        assert_eq!(wire["method"], "ask.event");
        assert_eq!(wire["params"]["ask"], "ask-1");
        assert_eq!(
            wire["params"]["event"],
            serde_json::to_value(&event).unwrap(),
            "the event is embedded as it is written alone"
        );
        assert_eq!(serde_json::from_value::<AskNotice>(wire).unwrap(), notice);
    }
}

#[test]
fn an_ask_outcome_notice_round_trips() {
    let notice = AskNotice::Outcome(Box::new(AskOutcomeNotice {
        ask: text("ask-1"),
        outcome: AskOutcome {
            end: AskEnd::TimedOut { after_s: 180 },
            cost: AskCost::NotReported {},
            quality: UsageQuality::Unreported,
        },
    }));
    round_trip(
        notice,
        json!({
            "method": "ask.outcome",
            "params": {
                "ask": "ask-1",
                "outcome": {
                    "end": { "type": "timed_out", "after_s": 180 },
                    "cost": { "type": "not_reported" },
                    "quality": "unreported"
                }
            }
        }),
    );
}

#[test]
fn a_notice_is_a_notification_with_its_method_name() {
    let notice = AskNotice::Event(Box::new(AskEventNotice {
        ask: text("ask-1"),
        event: AgentEvent::Message {
            text: "hi".to_string(),
        },
    }));
    assert_eq!(notice.method(), "ask.event");
    let notification = notice.to_notification();
    assert_eq!(notification.method, "ask.event");
    assert_eq!(AskNotice::from_notification(&notification).unwrap(), notice);
    let mut unknown = notification;
    unknown.method = "ask.started".to_string();
    assert!(AskNotice::from_notification(&unknown).is_err());
}

#[test]
fn an_ask_event_refuses_a_token_key_and_an_unknown_key() {
    let wire = json!({
        "method": "ask.event",
        "params": { "ask": "a", "event": { "type": "message", "text": "x" }, "token": "t" }
    });
    assert!(serde_json::from_value::<AskNotice>(wire).is_err());
}

#[test]
fn scrubbing_removes_the_ask_token_from_every_string_an_event_holds() {
    let token = text("secret-ask-token");
    let mut notice = AskNotice::Event(Box::new(AskEventNotice {
        ask: text("ask-1"),
        event: AgentEvent::ToolCall {
            id: "c1".to_string(),
            tool: Some("Bash".to_string()),
            kind: ToolKind::Execute,
            input: Some(json!({
                "command": "curl -H 'Authorization: Bearer secret-ask-token' x",
                "nested": [{ "secret-ask-token": "k" }, "secret-ask-token"]
            })),
        },
    }));
    notice.scrub_token(&token);
    let shown = serde_json::to_string(&notice).unwrap();
    assert!(!shown.contains("secret-ask-token"), "{shown}");
    assert!(
        shown.contains("curl -H 'Authorization: Bearer [removed]' x"),
        "{shown}"
    );
}

#[test]
fn scrubbing_leaves_an_event_with_no_token_as_it_was() {
    let token = text("secret-ask-token");
    for event in every_normalized_event() {
        let notice = AskNotice::Event(Box::new(AskEventNotice {
            ask: text("ask-1"),
            event,
        }));
        let mut scrubbed = notice.clone();
        scrubbed.scrub_token(&token);
        assert_eq!(scrubbed, notice);
    }
}

// The type-level test: no ask.* event can hold a token.

/// What one walk of a schema found.
#[derive(Default)]
struct Walk {
    /// Every property name of every object the schema can reach.
    names: BTreeSet<String>,
    /// Every place a value of any shape is allowed, as `Definition.property`: a `true` schema, an empty schema, or an
    /// object that allows extra keys.
    free_form: BTreeSet<String>,
}

fn walk(root: &Value) -> Walk {
    let mut found = Walk::default();
    let mut visited = BTreeSet::new();
    visit(root, root, "<root>", None, &mut visited, &mut found);
    found
}

fn place(def: &str, property: Option<&str>) -> String {
    match property {
        Some(property) => format!("{def}.{property}"),
        None => def.to_string(),
    }
}

fn visit(
    root: &Value,
    node: &Value,
    def: &str,
    property: Option<&str>,
    visited: &mut BTreeSet<String>,
    found: &mut Walk,
) {
    match node {
        Value::Bool(true) => {
            found.free_form.insert(place(def, property));
        }
        Value::Bool(false) => {}
        Value::Object(map) => {
            if let Some(Value::String(reference)) = map.get("$ref") {
                let name = reference
                    .strip_prefix("#/$defs/")
                    .unwrap_or_else(|| panic!("a $ref this walk does not read: {reference}"));
                if visited.insert(name.to_string()) {
                    let target = root["$defs"]
                        .get(name)
                        .unwrap_or_else(|| panic!("$ref to a missing definition: {name}"));
                    visit(root, target, name, None, visited, found);
                }
                return;
            }
            if map.is_empty() {
                found.free_form.insert(place(def, property));
                return;
            }
            let is_object = map.contains_key("properties")
                || map.get("type") == Some(&json!("object"))
                || map
                    .get("type")
                    .and_then(Value::as_array)
                    .is_some_and(|types| types.contains(&json!("object")));
            if is_object {
                match map.get("additionalProperties") {
                    Some(Value::Bool(false)) => {}
                    _ => {
                        found.free_form.insert(place(def, property));
                    }
                }
            }
            if let Some(Value::Object(properties)) = map.get("properties") {
                for (name, child) in properties {
                    found.names.insert(name.clone());
                    visit(root, child, def, Some(name), visited, found);
                }
            }
            for key in ["oneOf", "anyOf", "allOf", "prefixItems"] {
                if let Some(Value::Array(branches)) = map.get(key) {
                    for branch in branches {
                        visit(root, branch, def, property, visited, found);
                    }
                }
            }
            for key in ["items", "additionalProperties"] {
                if let Some(child @ Value::Object(_)) = map.get(key) {
                    visit(root, child, def, property, visited, found);
                }
            }
        }
        _ => {}
    }
}

fn names_a_token(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.contains("token") || lower.contains("endpoint")
}

/// The only places an `ask.*` event may hold a value of any shape: the agent's own tool input, and the raw body of an
/// event this version has no type for. Both are the normalized event's (ts-0036-xt), and `AskNotice::scrub_token`
/// takes the ask's token out of them before the daemon sends the event.
const AGENT_OWN_FREE_FORM: &[&str] = &[
    "AgentEvent.input",
    "AgentEvent.raw",
    "PermissionRequest.input",
];

#[test]
fn no_ask_event_has_a_field_named_for_a_token_or_an_endpoint() {
    for file in ["AskNotice.output.json", "AskNotice.input.json"] {
        let found = walk(&schema_file(file));
        assert!(
            found.names.contains("ask"),
            "{file}: the walk reached nothing"
        );
        assert!(
            found.names.contains("event"),
            "{file}: the walk missed the embedded event"
        );
        assert!(
            found.names.contains("cost"),
            "{file}: the walk missed the outcome"
        );
        let bad: Vec<&String> = found.names.iter().filter(|n| names_a_token(n)).collect();
        assert!(bad.is_empty(), "{file}: {bad:?}");
    }
}

#[test]
fn no_ask_event_has_a_map_that_could_carry_a_token_but_the_agents_own_input() {
    for file in ["AskNotice.output.json", "AskNotice.input.json"] {
        let found = walk(&schema_file(file));
        let extra: Vec<&String> = found
            .free_form
            .iter()
            .filter(|place| !AGENT_OWN_FREE_FORM.contains(&place.as_str()))
            .collect();
        assert!(extra.is_empty(), "{file}: free-form places {extra:?}");
        for allowed in AGENT_OWN_FREE_FORM {
            assert!(
                found.free_form.contains(*allowed),
                "{file}: the allowance for {allowed} is stale; remove it"
            );
        }
    }
}

#[test]
fn the_outcome_alone_has_no_free_form_place_and_no_token_name() {
    let found = walk(&schema_file("AskOutcome.output.json"));
    assert!(found.free_form.is_empty(), "{:?}", found.free_form);
    assert!(
        found.names.iter().all(|n| !names_a_token(n)),
        "{:?}",
        found.names
    );
}

#[test]
fn the_walk_does_see_a_token_and_a_free_form_place_when_there_is_one() {
    let found = walk(&schema_file("AskStartDaemonParams.output.json"));
    assert!(found.names.contains("token") && found.names.contains("endpoint"));
    let found = walk(&json!({
        "type": "object",
        "properties": { "extra": { "type": "object" }, "any": true, "askToken": { "type": "string" } },
        "additionalProperties": false
    }));
    assert!(found.names.iter().any(|n| names_a_token(n)));
    assert!(found.free_form.contains("<root>.extra"));
    assert!(found.free_form.contains("<root>.any"));
}

#[test]
fn the_windows_start_and_test_have_no_token_endpoint_or_free_form_place() {
    for name in ["AskStartParams", "AskTestParams", "AskCancelParams"] {
        for suffix in ["input", "output"] {
            let found = walk(&schema_file(&format!("{name}.{suffix}.json")));
            assert!(
                found.names.iter().all(|n| !names_a_token(n)),
                "{name}.{suffix}: {:?}",
                found.names
            );
            assert!(found.free_form.is_empty(), "{name}.{suffix}");
        }
    }
}

#[test]
fn the_daemons_start_and_test_have_both_fields_and_no_free_form_place() {
    for name in ["AskStartDaemonParams", "AskTestDaemonParams"] {
        let found = walk(&schema_file(&format!("{name}.input.json")));
        assert!(found.names.contains("endpoint"), "{name}");
        assert!(found.names.contains("token"), "{name}");
        assert!(found.free_form.is_empty(), "{name}");
    }
}

// Test this agent.

#[test]
fn the_probe_takes_red_green_and_not_tested() {
    round_trip(ConfinementProbe::Red, json!("red"));
    round_trip(ConfinementProbe::Green, json!("green"));
    round_trip(ConfinementProbe::NotTested, json!("not tested"));
    for wrong in ["not_tested", "Red", "amber", "", "not  tested"] {
        assert!(
            serde_json::from_value::<ConfinementProbe>(json!(wrong)).is_err(),
            "{wrong:?}"
        );
    }
}

#[test]
fn the_probe_values_are_in_the_schema_exactly() {
    let schema = schema_file("AskTestResult.output.json");
    let values = schema["$defs"]["ConfinementProbe"]["enum"].clone();
    assert_eq!(values, json!(["red", "green", "not tested"]));
}

fn full_result() -> (AskTestResult, Value) {
    (
        AskTestResult {
            endpoint_reached: true,
            first_tool_call_ms: Some(2400),
            finish_ms: Some(5100),
            cost: AskCost::Reported { cost: usd(0.0007) },
            readable_folders: vec!["/repo".to_string(), "/repo/docs".to_string()],
            tool_kinds: vec![ToolKind::Read, ToolKind::Search],
            failure: None,
            probe: ConfinementProbe::Green,
        },
        json!({
            "endpoint_reached": true,
            "first_tool_call_ms": 2400,
            "finish_ms": 5100,
            "cost": { "type": "reported", "cost": { "amount": 0.0007, "currency": "USD" } },
            "readable_folders": ["/repo", "/repo/docs"],
            "tool_kinds": ["read", "search"],
            "probe": "green"
        }),
    )
}

#[test]
fn a_test_result_holds_everything_the_plan_lists() {
    let (value, wire) = full_result();
    round_trip(value, wire);
}

#[test]
fn a_failed_test_has_no_times_a_plain_text_and_a_cost_not_reported() {
    round_trip(
        AskTestResult {
            endpoint_reached: false,
            first_tool_call_ms: None,
            finish_ms: None,
            cost: AskCost::NotReported {},
            readable_folders: vec![],
            tool_kinds: vec![],
            failure: Some("claude was not found on PATH".to_string()),
            probe: ConfinementProbe::NotTested,
        },
        json!({
            "endpoint_reached": false,
            "cost": { "type": "not_reported" },
            "readable_folders": [],
            "tool_kinds": [],
            "failure": "claude was not found on PATH",
            "probe": "not tested"
        }),
    );
}

#[test]
fn a_test_result_needs_the_probe_and_the_cost_and_refuses_an_unknown_key() {
    let (_, wire) = full_result();
    for key in ["probe", "cost", "endpoint_reached"] {
        let mut missing = wire.clone();
        missing.as_object_mut().unwrap().remove(key);
        assert!(
            serde_json::from_value::<AskTestResult>(missing).is_err(),
            "read without `{key}`"
        );
    }
    let mut extra = wire;
    extra["token"] = json!("t");
    assert!(serde_json::from_value::<AskTestResult>(extra).is_err());
}

#[test]
fn a_time_that_is_absent_is_not_zero() {
    let (mut value, _) = full_result();
    value.first_tool_call_ms = None;
    let wire = serde_json::to_value(&value).unwrap();
    assert!(wire.get("first_tool_call_ms").is_none(), "{wire}");
    let back: AskTestResult = serde_json::from_value(wire).unwrap();
    assert_eq!(back.first_tool_call_ms, None);
}

#[test]
fn the_test_result_has_no_token_name_and_no_free_form_place() {
    let found = walk(&schema_file("AskTestResult.output.json"));
    // `endpoint_reached` is a result and holds no endpoint, so only the token names are checked.
    assert!(
        found
            .names
            .iter()
            .all(|n| !n.to_lowercase().contains("token")),
        "{:?}",
        found.names
    );
    assert!(found.free_form.is_empty(), "{:?}", found.free_form);
}
