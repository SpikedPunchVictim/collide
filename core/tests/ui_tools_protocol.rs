//! Tests for the shell's `ui_tools.*` contract (ts-0035-re): the three methods, the `ui_tools.request` event, the
//! tool manifest's file format and its schema. The handler that uses them is `host`'s (`host/tests/ui_tools.rs`).

mod schema_validate;

use collide_core::owner::{Ack, Method, Request, RequestId, code, route};
use collide_core::schema;
use collide_core::ui_tools::{
    ManifestProblem, NO_WINDOW_TEXT, REQUEST_DEADLINE_SECS, TIMEOUT_TEXT, ToolManifest,
    UiToolsMethod, UiToolsReplyParams, UiToolsRequest, UiToolsRotateParams, UiToolsRotateResult,
    UiToolsTokenParams, UiToolsTokenResult,
};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};

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

fn repo(path: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join(path)
}

fn schema_file(name: &str) -> Value {
    serde_json::from_str(&schema::registry().render()[name]).unwrap()
}

fn fixture(path: &str) -> Value {
    let text = fs::read_to_string(repo(path)).unwrap_or_else(|e| panic!("{path}: {e}"));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{path}: {e}"))
}

// Methods.

#[test]
fn the_three_methods_are_reply_token_and_rotate() {
    let names: Vec<&str> = UiToolsMethod::ALL.iter().map(|m| m.name()).collect();
    assert_eq!(
        names,
        ["ui_tools.reply", "ui_tools.token", "ui_tools.rotate"]
    );
    for method in UiToolsMethod::ALL {
        assert_eq!(UiToolsMethod::from_name(method.name()), Some(*method));
    }
    assert_eq!(UiToolsMethod::from_name("ui_tools.request"), None);
    assert_eq!(UiToolsMethod::from_name("daemon.status"), None);
}

#[test]
fn the_owner_protocol_does_not_know_a_ui_tools_method() {
    for method in UiToolsMethod::ALL {
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
fn every_method_names_its_params_and_result_schema() {
    let names: Vec<(String, String)> = UiToolsMethod::ALL
        .iter()
        .map(|m| (m.params_schema(), m.result_schema()))
        .collect();
    assert_eq!(
        names,
        [
            ("UiToolsReplyParams".to_string(), "Ack".to_string()),
            (
                "UiToolsTokenParams".to_string(),
                "UiToolsTokenResult".to_string()
            ),
            (
                "UiToolsRotateParams".to_string(),
                "UiToolsRotateResult".to_string()
            ),
        ]
    );
    let files = schema::registry().render();
    for (params, result) in names {
        for name in [params, result] {
            assert!(files.contains_key(&format!("{name}.input.json")), "{name}");
            assert!(files.contains_key(&format!("{name}.output.json")), "{name}");
        }
    }
}

// Wire shapes.

#[test]
fn reply_params_carry_the_request_id_and_the_tool_result() {
    round_trip(
        UiToolsReplyParams {
            request_id: "ui-1".to_string(),
            content: vec![collide_core::ui_tools::ToolContent::Text {
                text: "shown".to_string(),
            }],
            is_error: false,
        },
        json!({ "requestId": "ui-1", "content": [{ "type": "text", "text": "shown" }], "isError": false }),
    );
    let read: UiToolsReplyParams =
        serde_json::from_value(json!({ "requestId": "ui-2", "content": [] })).unwrap();
    assert!(!read.is_error, "isError defaults to false");
}

#[test]
fn reply_params_refuse_a_missing_request_id_and_an_unknown_key() {
    assert!(serde_json::from_value::<UiToolsReplyParams>(json!({ "content": [] })).is_err());
    assert!(
        serde_json::from_value::<UiToolsReplyParams>(
            json!({ "requestId": "ui-1", "content": [], "actor": "x" })
        )
        .is_err()
    );
}

#[test]
fn token_and_rotate_take_nothing_and_answer_a_token() {
    round_trip(UiToolsTokenParams {}, json!({}));
    round_trip(UiToolsRotateParams {}, json!({}));
    round_trip(
        UiToolsTokenResult {
            token: "t".to_string(),
        },
        json!({ "token": "t" }),
    );
    round_trip(
        UiToolsRotateResult {
            token: "u".to_string(),
        },
        json!({ "token": "u" }),
    );
    assert!(serde_json::from_value::<UiToolsTokenParams>(json!({ "x": 1 })).is_err());
}

#[test]
fn the_request_event_has_request_id_caller_deadline_and_message() {
    let message =
        json!({ "jsonrpc": "2.0", "id": 7, "method": "tools/call", "params": { "name": "open" } });
    round_trip(
        UiToolsRequest {
            request_id: "ui-3".to_string(),
            caller: "external".to_string(),
            deadline: 1_700_000_010_000,
            message: message.clone(),
        },
        json!({ "requestId": "ui-3", "caller": "external", "deadline": 1_700_000_010_000_u64, "message": message }),
    );
    assert!(
        serde_json::from_value::<UiToolsRequest>(
            json!({ "requestId": "x", "caller": "a", "deadline": 1, "message": {}, "token": "t" })
        )
        .is_err()
    );
}

#[test]
fn the_event_method_name_and_the_failure_texts_are_the_plans() {
    assert_eq!(collide_core::ui_tools::REQUEST_EVENT, "ui_tools.request");
    assert_eq!(REQUEST_DEADLINE_SECS, 10);
    assert_eq!(
        NO_WINDOW_TEXT,
        "Collide has no window open; open Collide and ask again"
    );
    assert_eq!(
        TIMEOUT_TEXT,
        "the Collide window did not answer within 10 s"
    );
}

#[test]
fn the_ack_result_of_reply_is_the_owners() {
    round_trip(Ack {}, json!({}));
}

// The manifest.

#[test]
fn the_valid_fixture_validates_against_the_manifest_schema() {
    let doc = fixture("fixtures/ui-tools/manifest.json");
    for name in ["ToolManifest.input.json", "ToolManifest.output.json"] {
        assert_eq!(
            schema_validate::violations(&schema_file(name), &doc),
            Vec::<String>::new(),
            "{name}"
        );
    }
}

#[test]
fn the_valid_fixture_loads_and_checks_and_has_the_five_tools() {
    let text = fs::read_to_string(repo("fixtures/ui-tools/manifest.json")).unwrap();
    let manifest = ToolManifest::from_json(&text).unwrap();
    let names: Vec<&str> = manifest.0.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(
        names,
        ["query", "set_view", "show_groups", "highlight", "open"]
    );
    assert_eq!(manifest.check(), Ok(()));
    assert!(manifest.is_read_only("query"));
    assert!(!manifest.is_read_only("set_view"));
    assert!(
        !manifest.is_read_only("no_such_tool"),
        "an unknown tool is not read-only"
    );
}

#[test]
fn the_valid_fixture_round_trips_through_the_type() {
    let doc = fixture("fixtures/ui-tools/manifest.json");
    let manifest: ToolManifest = serde_json::from_value(doc.clone()).unwrap();
    assert_eq!(serde_json::to_value(&manifest).unwrap(), doc);
}

#[test]
fn each_invalid_fixture_fails_the_manifest_schema() {
    let mut count = 0;
    for entry in fs::read_dir(repo("fixtures/ui-tools/invalid")).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let doc: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert!(
            !schema_validate::violations(&schema_file("ToolManifest.input.json"), &doc).is_empty(),
            "{name} passed the schema"
        );
        count += 1;
    }
    assert!(count >= 6, "only {count} invalid fixtures");
}

#[test]
fn each_invalid_fixture_also_fails_to_load_as_the_type_or_the_check() {
    for entry in fs::read_dir(repo("fixtures/ui-tools/invalid")).unwrap() {
        let path = entry.unwrap().path();
        let text = fs::read_to_string(&path).unwrap();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let loaded =
            ToolManifest::from_json(&text).and_then(|m| m.check().map_err(|p| p.to_string()));
        assert!(loaded.is_err(), "{name} loaded");
    }
}

#[test]
fn the_check_refuses_what_a_schema_cannot_say() {
    let duplicate = ToolManifest::from_json(
        r#"[{"name":"a","description":"d","inputSchema":{"type":"object"}},
            {"name":"a","description":"d","inputSchema":{"type":"object"}}]"#,
    )
    .unwrap();
    assert_eq!(
        duplicate.check(),
        Err(ManifestProblem::DuplicateName("a".to_string()))
    );
    let array = ToolManifest::from_json(
        r#"[{"name":"a","description":"d","inputSchema":{"type":"array"}}]"#,
    )
    .unwrap();
    assert_eq!(
        array.check(),
        Err(ManifestProblem::InputSchemaNotAnObject("a".to_string()))
    );
    let untyped =
        ToolManifest::from_json(r#"[{"name":"a","description":"d","inputSchema":{}}]"#).unwrap();
    assert!(untyped.check().is_err());
}

#[test]
fn a_manifest_problem_names_the_tool() {
    let text = ManifestProblem::DuplicateName("open".to_string()).to_string();
    assert!(text.contains("open"), "{text}");
}

#[test]
fn from_json_says_what_is_wrong_with_text_that_is_not_a_manifest() {
    let error = ToolManifest::from_json("{").unwrap_err();
    assert!(error.contains("tool manifest"), "{error}");
}
