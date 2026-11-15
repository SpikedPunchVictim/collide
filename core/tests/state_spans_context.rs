//! Tests for the run, phase and workspace states, the spans and the Queue item's context record (ts-0037-vg):
//! the span validity cases, each context section missing in turn, the state round trips, and the schemas.

use collide_core::context::{ContextRecord, NOT_AVAILABLE, Section};
use collide_core::events::{Event, EventBody};
use collide_core::non_empty::NonEmpty;
use collide_core::schema;
use collide_core::span::Span;
use collide_core::state::{FailCause, PauseCause, PhaseState, RunState, WorkspaceState};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

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

fn refused<T: DeserializeOwned + std::fmt::Debug>(wire: Value) {
    let shown = wire.to_string();
    let parsed = serde_json::from_value::<T>(wire);
    assert!(parsed.is_err(), "{shown} should be refused, got {parsed:?}");
}

fn text(value: &str) -> NonEmpty {
    NonEmpty::new(value).unwrap()
}

fn schema_file(name: &str) -> Value {
    serde_json::from_str(&schema::registry().render()[name]).unwrap()
}

// States.

#[test]
fn every_run_state_round_trips() {
    round_trip(RunState::Running {}, json!({ "state": "running" }));
    round_trip(RunState::Integrating {}, json!({ "state": "integrating" }));
    round_trip(RunState::Failed {}, json!({ "state": "failed" }));
    round_trip(RunState::Stopped {}, json!({ "state": "stopped" }));
    round_trip(
        RunState::Ended {
            outcome: text("done"),
        },
        json!({ "state": "ended", "outcome": "done" }),
    );
}

#[test]
fn a_run_pauses_with_each_cause() {
    for (cause, name) in [
        (PauseCause::AskHuman, "ask_human"),
        (PauseCause::UsageLimit, "usage_limit"),
        (PauseCause::Disk, "disk"),
    ] {
        round_trip(
            RunState::Paused { cause },
            json!({ "state": "paused", "cause": name }),
        );
    }
}

#[test]
fn a_run_state_with_no_cause_or_outcome_is_refused() {
    refused::<RunState>(json!({ "state": "paused" }));
    refused::<RunState>(json!({ "state": "ended" }));
    refused::<RunState>(json!({ "state": "ended", "outcome": "" }));
    refused::<RunState>(json!({ "state": "paused", "cause": "tired" }));
}

#[test]
fn a_run_state_with_an_unknown_name_or_key_is_refused() {
    refused::<RunState>(json!({ "state": "done" }));
    refused::<RunState>(json!({}));
    refused::<RunState>(json!("running"));
    refused::<RunState>(json!({ "state": "running", "extra": 1 }));
    refused::<RunState>(json!({ "state": "integrating", "url": "u" }));
}

#[test]
fn every_phase_state_round_trips() {
    for name in ["pending", "running", "ended", "stopped", "lost"] {
        let value: PhaseState = serde_json::from_value(json!({ "state": name })).unwrap();
        assert_eq!(
            serde_json::to_value(&value).unwrap(),
            json!({ "state": name })
        );
    }
    round_trip(
        PhaseState::Paused {
            cause: PauseCause::UsageLimit,
        },
        json!({ "state": "paused", "cause": "usage_limit" }),
    );
    round_trip(
        PhaseState::Failed {
            cause: Some(FailCause::Disk),
        },
        json!({ "state": "failed", "cause": "disk" }),
    );
}

#[test]
fn a_phase_can_be_lost_and_a_full_disk_fails_it_with_cause_disk() {
    assert_eq!(
        serde_json::from_value::<PhaseState>(json!({ "state": "lost" })).unwrap(),
        PhaseState::Lost {}
    );
    let failed: PhaseState = serde_json::from_value(json!({ "state": "failed" })).unwrap();
    assert_eq!(failed, PhaseState::Failed { cause: None });
}

#[test]
fn a_phase_state_with_an_unknown_name_or_key_is_refused() {
    refused::<PhaseState>(json!({ "state": "done" }));
    refused::<PhaseState>(json!({ "state": "lost", "extra": 1 }));
    refused::<PhaseState>(json!({ "state": "failed", "cause": "boredom" }));
    refused::<PhaseState>(json!({ "state": "integrating" }));
}

#[test]
fn every_workspace_state_round_trips() {
    for (state, name) in [
        (WorkspaceState::Provisioning, "provisioning"),
        (WorkspaceState::Ready, "ready"),
        (WorkspaceState::InUse, "in_use"),
        (WorkspaceState::Idle, "idle"),
        (WorkspaceState::Removable, "removable"),
        (WorkspaceState::Removed, "removed"),
        (WorkspaceState::Failed, "failed"),
    ] {
        round_trip(state, json!(name));
    }
}

#[test]
fn a_workspace_state_outside_the_lifecycle_is_refused() {
    refused::<WorkspaceState>(json!("lost"));
    refused::<WorkspaceState>(json!("in-use"));
    refused::<WorkspaceState>(json!({ "state": "ready" }));
}

#[test]
fn the_transition_events_carry_the_typed_states() {
    let line = |body: Value| json!({ "seq": 1, "wall_ms": 1, "actor": { "kind": "person" }, "experiment": null, "body": body });
    let run = line(json!({
        "event": "run.transition", "run": "r-1",
        "from": { "state": "running" }, "to": { "state": "paused", "cause": "disk" }
    }));
    let event: Event = serde_json::from_value(run.clone()).unwrap();
    assert!(matches!(
        event.body,
        EventBody::RunTransition {
            to: RunState::Paused {
                cause: PauseCause::Disk
            },
            ..
        }
    ));
    assert_eq!(serde_json::to_value(&event).unwrap(), run);

    let workspace = line(json!({
        "event": "workspace.transition", "workspace": "w-1", "from": "ready", "to": "in_use"
    }));
    let event: Event = serde_json::from_value(workspace).unwrap();
    assert!(matches!(
        event.body,
        EventBody::WorkspaceTransition {
            to: WorkspaceState::InUse,
            ..
        }
    ));

    refused::<Event>(line(
        json!({ "event": "run.transition", "run": "r-1", "from": null, "to": "running" }),
    ));
}

// Spans.

#[test]
fn a_root_span_has_an_id_and_no_parent() {
    let root = Span::Run { id: text("s-1") };
    round_trip(root.clone(), json!({ "kind": "run", "id": "s-1" }));
    assert!(root.parent().is_none());
}

#[test]
fn every_other_kind_round_trips_with_its_parent() {
    for kind in [
        "phase",
        "agent_turn",
        "tool_call",
        "hook",
        "gate",
        "provisioning",
        "subprocess",
        "wait",
    ] {
        let wire = json!({ "kind": kind, "id": "s-2", "parent": "s-1" });
        let span: Span = serde_json::from_value(wire.clone()).unwrap();
        assert_eq!(serde_json::to_value(&span).unwrap(), wire, "{kind}");
        assert_eq!(span.id().as_str(), "s-2");
        assert_eq!(span.parent().unwrap().as_str(), "s-1");
        assert!(span.queue_item().is_none());
    }
}

#[test]
fn a_span_without_a_kind_is_invalid() {
    refused::<Span>(json!({ "id": "s-2", "parent": "s-1" }));
}

#[test]
fn a_span_with_an_unknown_kind_is_invalid() {
    refused::<Span>(json!({ "kind": "lunch", "id": "s-2", "parent": "s-1" }));
}

#[test]
fn a_span_without_an_id_is_invalid() {
    refused::<Span>(json!({ "kind": "phase", "parent": "s-1" }));
    refused::<Span>(json!({ "kind": "run" }));
    refused::<Span>(json!({ "kind": "phase", "id": "", "parent": "s-1" }));
}

#[test]
fn a_span_that_is_not_the_root_needs_a_parent() {
    for kind in [
        "phase",
        "agent_turn",
        "tool_call",
        "hook",
        "gate",
        "provisioning",
        "subprocess",
        "wait",
    ] {
        refused::<Span>(json!({ "kind": kind, "id": "s-2" }));
        refused::<Span>(json!({ "kind": kind, "id": "s-2", "parent": null }));
        refused::<Span>(json!({ "kind": kind, "id": "s-2", "parent": "" }));
    }
    refused::<Span>(json!({ "kind": "user_wait", "id": "s-2", "queue_item": "q-1" }));
}

#[test]
fn the_root_takes_no_parent() {
    refused::<Span>(json!({ "kind": "run", "id": "s-1", "parent": "s-0" }));
}

#[test]
fn a_wait_on_the_user_names_the_queue_item() {
    let wire = json!({ "kind": "user_wait", "id": "s-3", "parent": "s-2", "queue_item": "q-9" });
    let span: Span = serde_json::from_value(wire.clone()).unwrap();
    assert_eq!(serde_json::to_value(&span).unwrap(), wire);
    assert_eq!(span.queue_item().unwrap().as_str(), "q-9");
}

#[test]
fn a_wait_on_the_user_without_a_queue_item_is_invalid() {
    refused::<Span>(json!({ "kind": "user_wait", "id": "s-3", "parent": "s-2" }));
    refused::<Span>(
        json!({ "kind": "user_wait", "id": "s-3", "parent": "s-2", "queue_item": null }),
    );
    refused::<Span>(json!({ "kind": "user_wait", "id": "s-3", "parent": "s-2", "queue_item": "" }));
}

#[test]
fn only_a_wait_on_the_user_takes_a_queue_item() {
    refused::<Span>(json!({ "kind": "wait", "id": "s-3", "parent": "s-2", "queue_item": "q-9" }));
    refused::<Span>(json!({ "kind": "phase", "id": "s-3", "parent": "s-2", "extra": 1 }));
}

// The context record.

const SECTIONS: [&str; 6] = [
    "why_it_asks",
    "what_it_would_do",
    "task",
    "what_led_here",
    "history",
    "agents_own_words",
];

fn filled(value: &str) -> Value {
    json!({ "status": "filled", "text": value })
}

fn marker() -> Value {
    json!({ "status": "not_available" })
}

fn record(each: impl Fn(&str) -> Value) -> Value {
    let mut doc = serde_json::Map::new();
    for name in SECTIONS {
        doc.insert(name.to_string(), each(name));
    }
    Value::Object(doc)
}

#[test]
fn a_record_with_every_section_filled_round_trips() {
    let wire = record(|name| filled(&format!("text of {name}")));
    let parsed: ContextRecord = serde_json::from_value(wire.clone()).unwrap();
    assert_eq!(
        parsed.history,
        Section::Filled {
            text: text("text of history")
        }
    );
    assert_eq!(serde_json::to_value(&parsed).unwrap(), wire);
}

#[test]
fn a_record_with_every_section_marked_not_available_round_trips() {
    let wire = record(|_| marker());
    let parsed: ContextRecord = serde_json::from_value(wire.clone()).unwrap();
    assert_eq!(parsed.task, Section::NotAvailable {});
    assert_eq!(serde_json::to_value(&parsed).unwrap(), wire);
}

#[test]
fn the_marker_is_snake_case_on_the_wire_and_the_view_shows_its_words() {
    assert_eq!(NOT_AVAILABLE, "Not available in this view");
    assert_eq!(
        serde_json::to_value(Section::NotAvailable {}).unwrap(),
        json!({ "status": "not_available" })
    );
}

#[test]
fn a_record_with_a_missing_section_is_invalid_whichever_section_it_is() {
    for missing in SECTIONS {
        let mut doc = record(|_| filled("x"));
        doc.as_object_mut().unwrap().remove(missing);
        refused::<ContextRecord>(doc);
    }
}

#[test]
fn a_section_left_null_or_blank_is_invalid_whichever_section_it_is() {
    for name in SECTIONS {
        for blank in [
            json!(null),
            json!(""),
            json!({}),
            json!({ "status": "filled" }),
        ] {
            let mut doc = record(|_| filled("x"));
            doc[name] = blank;
            refused::<ContextRecord>(doc);
        }
    }
}

#[test]
fn a_filled_section_with_empty_text_is_invalid() {
    let mut doc = record(|_| filled("x"));
    doc["history"] = filled("");
    refused::<ContextRecord>(doc);
}

#[test]
fn a_not_available_section_takes_no_text() {
    let mut doc = record(|_| filled("x"));
    doc["task"] = json!({ "status": "not_available", "text": "something" });
    refused::<ContextRecord>(doc);
}

#[test]
fn a_record_with_an_extra_section_is_invalid() {
    let mut doc = record(|_| filled("x"));
    doc["seventh"] = filled("y");
    refused::<ContextRecord>(doc);
}

// The schemas.

#[test]
fn the_new_types_are_registered_in_both_forms() {
    let files = schema::registry().render();
    for name in [
        "RunState",
        "PhaseState",
        "WorkspaceState",
        "Span",
        "ContextRecord",
    ] {
        assert!(
            files.contains_key(&format!("{name}.input.json")),
            "{name} input"
        );
        assert!(
            files.contains_key(&format!("{name}.output.json")),
            "{name} output"
        );
    }
}

#[test]
fn the_span_schema_is_closed_and_requires_the_parent_of_every_non_root_kind() {
    let schema = schema_file("Span.input.json");
    let variants = schema["oneOf"].as_array().expect("one variant per kind");
    assert_eq!(variants.len(), 10);
    for variant in variants {
        let kind = variant["properties"]["kind"]["const"].as_str().unwrap();
        assert_eq!(variant["additionalProperties"], json!(false), "{kind}");
        let required: Vec<&str> = variant["required"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(required.contains(&"parent"), kind != "run", "{kind}");
        assert_eq!(
            required.contains(&"queue_item"),
            kind == "user_wait",
            "{kind}"
        );
        assert!(
            required.contains(&"kind") && required.contains(&"id"),
            "{kind}"
        );
    }
}

#[test]
fn the_context_schema_requires_all_six_sections_and_names_the_marker() {
    let schema = schema_file("ContextRecord.input.json");
    let mut required: Vec<&str> = schema["required"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    required.sort_unstable();
    let mut expected = SECTIONS.to_vec();
    expected.sort_unstable();
    assert_eq!(required, expected);
    assert_eq!(schema["additionalProperties"], json!(false));
    assert!(
        schema_file("ContextRecord.input.json")
            .to_string()
            .contains("\"not_available\""),
        "the marker is in the schema"
    );
}

#[test]
fn the_run_state_schema_lists_integrating_and_the_disk_pause() {
    let text = schema_file("RunState.input.json").to_string();
    assert!(text.contains("\"integrating\""));
    assert!(text.contains("\"disk\""));
    assert!(text.contains("\"ended\""));
}
