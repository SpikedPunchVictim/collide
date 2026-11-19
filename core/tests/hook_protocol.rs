//! Tests for the hook protocol (ts-0027): the stdin input, the result file and the validity function that turns
//! an exit status and a result file into the effective status (0.5).

use collide_core::hook::{
    self, HookInput, HookKind, HookResult, HookStatus, MOVE_POINT_TIMEOUT_SECS, OUTPUTS_MAX_BYTES,
    TASK_POINT_TIMEOUT_SECS, effective_result,
};
use collide_core::schema;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::fmt::Debug;

/// Parses `doc` as a `T` and writes it back: the result must be the same JSON.
fn round_trip<T: Serialize + DeserializeOwned + Debug>(doc: Value) -> T {
    let parsed: T =
        serde_json::from_value(doc.clone()).unwrap_or_else(|error| panic!("{error} in {doc}"));
    assert_eq!(serde_json::to_value(&parsed).unwrap(), doc);
    parsed
}

fn refused<T: DeserializeOwned + Debug>(doc: Value) {
    let result = serde_json::from_value::<T>(doc.clone());
    assert!(result.is_err(), "{doc} should be refused, got {result:?}");
}

fn linked_task() -> Value {
    json!({
        "key": "plan:0027",
        "shortId": "ts-0027",
        "kind": "task",
        "state": { "status": "active", "category": "active" },
        "path": ".collide/planning/epics/ep-002-va/tasks/ts-0027-ar.md",
        "templateFields": { "labelId": "t02-027" },
        "fields": { "epic": "ep-002-va" },
        "links": [
            {
                "type": "blocked-by",
                "dir": "out",
                "key": "plan:0009",
                "kind": "task",
                "status": "done",
                "category": "done",
                "stale": false,
                "missing": false
            },
            {
                "type": "blocked-by",
                "dir": "in",
                "key": "plan:0038",
                "kind": "task",
                "status": "ready",
                "category": "ready",
                "stale": true,
                "missing": false
            }
        ]
    })
}

fn change() -> Value {
    json!({
        "op": "setStatus",
        "args": {},
        "from": { "status": "active", "category": "active" },
        "to": { "status": "review", "category": "review" },
        "diff": { "status": { "from": "active", "to": "review" } },
        "categoryMoved": true,
        "via": "ui",
        "actor": { "kind": "person" },
        "batch": null,
        "effects": [],
        "effectOf": null,
        "renamed": [],
        "checkout": "main",
        "files": []
    })
}

/// A hook input outside an experiment and outside a task point.
fn workspace_input() -> Value {
    json!({
        "protocol": 1,
        "hook": "demo/ws-create",
        "pack": "demo",
        "workflow": "demo/loop",
        "board": "main",
        "line": "main",
        "experiment": null,
        "run": "r-77",
        "launch": { "rounds": 3 },
        "config": { "region": "eu" },
        "tasks": [linked_task()],
        "phase": null,
        "workspace": null,
        "history": [],
        "state": { "run": {}, "workspace": {}, "point": {} }
    })
}

/// A hook input inside an experiment, in a phase, with a workspace and a history.
fn phase_input() -> Value {
    let mut doc = workspace_input();
    doc["hook"] = json!("demo/gate-tests");
    doc["experiment"] = json!({ "id": "x-3", "arm": "b", "dir": "/data/x-3/b" });
    doc["phase"] =
        json!({ "name": "build", "kind": "script", "round": 2, "dir": "/data/r-77/build" });
    doc["workspace"] =
        json!({ "kind": "task", "path": "/work/t02-027", "gitDir": "/work/t02-027/.git" });
    doc["history"] = json!([{
        "phase": "plan",
        "kind": "agent",
        "round": 1,
        "startHead": "a1",
        "endHead": "b2",
        "artifacts": ["/data/r-77/plan/plan.md"],
        "verdict": "ok"
    }]);
    doc["state"] = json!({
        "run": { "demo/gate-tests": { "passed": 412 } },
        "workspace": { "lane": "l-1" },
        "point": {}
    });
    doc
}

/// A task point's input: the 0.5 fields plus `change` and `latestRun`.
fn task_point_input() -> Value {
    let mut doc = workspace_input();
    doc["hook"] = json!("demo/gate-review");
    doc["change"] = change();
    doc["latestRun"] = json!({
        "id": "r-70",
        "workflow": "demo/loop",
        "outcome": "done",
        "state": {}
    });
    doc
}

fn result_file(doc: &Value) -> Vec<u8> {
    serde_json::to_vec(doc).unwrap()
}

fn effective(kind: HookKind, file: &Value) -> HookResult {
    effective_result(kind, true, Some(&result_file(file)))
}

const ALL_KINDS: [HookKind; 10] = [
    HookKind::Workspace,
    HookKind::Record,
    HookKind::Integrate,
    HookKind::IntegratePullRequest,
    HookKind::Sync,
    HookKind::Gate,
    HookKind::Script,
    HookKind::Wait,
    HookKind::Decision,
    HookKind::Point,
];

// The input.

#[test]
fn a_hook_input_outside_a_task_point_round_trips() {
    let input: HookInput = round_trip(workspace_input());
    assert_eq!(input.protocol, hook::HOOK_PROTOCOL);
    assert_eq!(input.tasks.len(), 1);
    assert!(input.change.is_none());
}

#[test]
fn a_hook_input_with_an_experiment_phase_workspace_and_history_round_trips() {
    let input: HookInput = round_trip(phase_input());
    assert_eq!(input.experiment.unwrap().arm, "b");
    assert_eq!(input.history[0].artifacts.len(), 1);
}

#[test]
fn a_task_point_input_adds_change_and_latest_run() {
    let input: HookInput = round_trip(task_point_input());
    assert!(input.change.is_some());
    assert_eq!(input.latest_run.unwrap().id, "r-70");
}

#[test]
fn a_tasks_entry_carries_its_links_one_hop_out_in_both_directions() {
    let input: HookInput = round_trip(workspace_input());
    let links = &input.tasks[0].links;
    assert_eq!(links.len(), 2);
    assert_eq!(links[0].key, "plan:0009");
    assert!(!links[0].stale);
    assert!(links[1].stale);
}

#[test]
fn a_link_to_a_key_the_corpus_does_not_hold_says_so() {
    let mut doc = workspace_input();
    doc["tasks"][0]["links"][0] = json!({
        "type": "blocked-by",
        "dir": "out",
        "key": "plan:0099",
        "kind": "",
        "status": "",
        "category": "backlog",
        "stale": false,
        "missing": true
    });
    let input: HookInput = round_trip(doc);
    assert!(input.tasks[0].links[0].missing);
    assert!(!input.tasks[0].links[1].missing);
}

#[test]
fn a_link_written_without_missing_reads_as_present() {
    let mut doc = workspace_input();
    doc["tasks"][0]["links"][0]
        .as_object_mut()
        .unwrap()
        .remove("missing");
    let input: HookInput = serde_json::from_value(doc).unwrap();
    assert!(!input.tasks[0].links[0].missing);
}

#[test]
fn a_link_to_a_feature_names_it_and_a_link_to_an_item_leaves_the_key_out() {
    let mut doc = workspace_input();
    doc["tasks"][0]["links"][0] = json!({
        "type": "implements",
        "dir": "out",
        "key": "plan:0003",
        "kind": "proposal",
        "status": "accepted",
        "category": "done",
        "stale": false,
        "missing": false,
        "feature": "F2"
    });
    let input: HookInput = round_trip(doc);
    assert_eq!(input.tasks[0].links[0].feature.as_deref(), Some("F2"));
    assert_eq!(input.tasks[0].links[1].feature, None);
}

#[test]
fn a_link_written_before_the_feature_key_reads_with_none() {
    let input: HookInput = serde_json::from_value(workspace_input()).unwrap();
    assert!(
        input.tasks[0]
            .links
            .iter()
            .all(|link| link.feature.is_none())
    );
}

#[test]
fn the_link_feature_key_is_in_the_hook_input_schema_and_is_not_required() {
    let files = schema::registry().render();
    let schema: Value = serde_json::from_str(&files["HookInput.input.json"]).unwrap();
    let link = &schema["$defs"]["HookLink"];
    assert!(
        link["properties"]
            .as_object()
            .unwrap()
            .contains_key("feature")
    );
    let required: Vec<&str> = link["required"]
        .as_array()
        .unwrap()
        .iter()
        .map(|key| key.as_str().unwrap())
        .collect();
    assert!(!required.contains(&"feature"), "{required:?}");
    assert!(
        link["properties"]["feature"]["type"]
            .to_string()
            .contains("string")
    );
}

#[test]
fn a_tasks_entry_has_no_body() {
    let mut doc = workspace_input();
    doc["tasks"][0]["body"] = json!("the task text");
    refused::<HookInput>(doc);
}

#[test]
fn a_hook_input_with_an_unknown_key_is_refused() {
    let mut doc = workspace_input();
    doc["extra"] = json!(1);
    refused::<HookInput>(doc);
}

#[test]
fn a_link_with_a_direction_outside_out_and_in_is_refused() {
    let mut doc = workspace_input();
    doc["tasks"][0]["links"][0]["dir"] = json!("sideways");
    refused::<HookInput>(doc);
}

// The result.

#[test]
fn each_status_of_a_result_round_trips() {
    let statuses = [
        ("ok", HookStatus::Ok),
        ("fail", HookStatus::Fail),
        ("unverified", HookStatus::Unverified),
        ("pending", HookStatus::Pending),
        ("refused", HookStatus::Refused),
        ("error", HookStatus::Error),
    ];
    for (name, status) in statuses {
        let result: HookResult =
            round_trip(json!({ "status": name, "message": "m", "outputs": {} }));
        assert_eq!(result.status, status, "{name}");
    }
}

#[test]
fn a_pending_result_with_retry_after_parses() {
    let doc =
        json!({ "status": "pending", "message": "CI running", "outputs": {}, "retryAfter": 45 });
    let result: HookResult = round_trip(doc);
    assert_eq!(result.status, HookStatus::Pending);
    assert_eq!(result.retry_after, Some(45));
}

#[test]
fn a_result_without_retry_after_writes_none() {
    let doc = json!({ "status": "ok", "message": "", "outputs": {} });
    let result: HookResult = round_trip(doc.clone());
    assert_eq!(result.retry_after, None);
    assert!(
        serde_json::to_value(&result)
            .unwrap()
            .get("retryAfter")
            .is_none()
    );
}

#[test]
fn a_result_needs_only_a_status() {
    let result: HookResult = serde_json::from_value(json!({ "status": "ok" })).unwrap();
    assert_eq!(result.message, "");
    assert!(result.outputs.is_empty());
}

#[test]
fn a_result_with_a_bad_status_key_or_retry_after_is_refused() {
    refused::<HookResult>(json!({ "status": "maybe" }));
    refused::<HookResult>(json!({ "message": "no status" }));
    refused::<HookResult>(json!({ "status": "ok", "extra": 1 }));
    refused::<HookResult>(json!({ "status": "pending", "retryAfter": -1 }));
    refused::<HookResult>(json!({ "status": "pending", "retry_after": 1 }));
}

// The validity function.

#[test]
fn pending_is_valid_only_from_a_wait_hook_and_a_pr_integrate() {
    for kind in ALL_KINDS {
        let result = effective(kind, &json!({ "status": "pending", "retryAfter": 30 }));
        let allowed = matches!(kind, HookKind::Wait | HookKind::IntegratePullRequest);
        let expected = if allowed {
            HookStatus::Pending
        } else {
            HookStatus::Error
        };
        assert_eq!(result.status, expected, "{kind:?}");
    }
}

#[test]
fn a_valid_pending_keeps_its_retry_after_and_message() {
    let doc = json!({ "status": "pending", "message": "CI running", "retryAfter": 30 });
    let result = effective(HookKind::Wait, &doc);
    assert_eq!(result.retry_after, Some(30));
    assert_eq!(result.message, "CI running");
}

#[test]
fn a_pending_made_error_says_why() {
    let result = effective(HookKind::Record, &json!({ "status": "pending" }));
    assert!(result.message.contains("pending"), "{}", result.message);
    assert_eq!(result.retry_after, None);
}

#[test]
fn unverified_is_valid_only_from_a_gate_script_wait_or_named_point() {
    for kind in ALL_KINDS {
        let result = effective(
            kind,
            &json!({ "status": "unverified", "message": "docker is down" }),
        );
        let allowed = matches!(
            kind,
            HookKind::Gate | HookKind::Script | HookKind::Wait | HookKind::Point
        );
        let expected = if allowed {
            HookStatus::Unverified
        } else {
            HookStatus::Error
        };
        assert_eq!(result.status, expected, "{kind:?}");
    }
}

#[test]
fn ok_fail_and_refused_and_error_are_valid_from_every_kind() {
    for kind in ALL_KINDS {
        for (name, status) in [
            ("ok", HookStatus::Ok),
            ("fail", HookStatus::Fail),
            ("refused", HookStatus::Refused),
            ("error", HookStatus::Error),
        ] {
            let result = effective(kind, &json!({ "status": name }));
            assert_eq!(result.status, status, "{kind:?} {name}");
        }
    }
}

#[test]
fn a_non_zero_exit_is_error_whatever_the_file_says() {
    for kind in ALL_KINDS {
        let file = result_file(&json!({ "status": "ok", "message": "fine" }));
        let result = effective_result(kind, false, Some(&file));
        assert_eq!(result.status, HookStatus::Error, "{kind:?}");
        assert!(result.message.contains("exit"), "{}", result.message);
    }
}

#[test]
fn a_non_zero_exit_with_no_file_is_error() {
    assert_eq!(
        effective_result(HookKind::Gate, false, None).status,
        HookStatus::Error
    );
}

#[test]
fn a_missing_result_file_is_error() {
    for kind in ALL_KINDS {
        let result = effective_result(kind, true, None);
        assert_eq!(result.status, HookStatus::Error, "{kind:?}");
        assert!(
            result.message.contains("no result file"),
            "{}",
            result.message
        );
    }
}

#[test]
fn an_invalid_result_file_is_error() {
    let invalid: [&[u8]; 8] = [
        b"",
        b"not json",
        b"[]",
        b"{}",
        br#"{"status":"maybe"}"#,
        br#"{"status":"ok","extra":1}"#,
        br#"{"status":"pending","retryAfter":-1}"#,
        &[0xff, 0xfe],
    ];
    for bytes in invalid {
        let result = effective_result(HookKind::Wait, true, Some(bytes));
        assert_eq!(result.status, HookStatus::Error, "{bytes:?}");
        assert!(result.message.contains("invalid"), "{}", result.message);
    }
}

/// A result whose compact `outputs` object is exactly `size` bytes.
fn outputs_of_size(size: usize) -> Value {
    // `{"k":"` and `"}` are eight bytes around the value.
    json!({ "status": "ok", "outputs": { "k": "x".repeat(size - 8) } })
}

#[test]
fn outputs_of_exactly_64_kb_are_valid() {
    assert_eq!(OUTPUTS_MAX_BYTES, 64 * 1024);
    let result = effective(HookKind::Script, &outputs_of_size(OUTPUTS_MAX_BYTES));
    assert_eq!(result.status, HookStatus::Ok);
}

#[test]
fn outputs_one_byte_over_64_kb_are_error() {
    let result = effective(HookKind::Script, &outputs_of_size(OUTPUTS_MAX_BYTES + 1));
    assert_eq!(result.status, HookStatus::Error);
    assert!(result.message.contains("outputs"), "{}", result.message);
    assert!(result.outputs.is_empty());
}

#[test]
fn oversized_outputs_make_a_valid_pending_error() {
    let mut doc = outputs_of_size(OUTPUTS_MAX_BYTES + 1);
    doc["status"] = json!("pending");
    assert_eq!(effective(HookKind::Wait, &doc).status, HookStatus::Error);
}

// Timeouts.

#[test]
fn point_binding_timeouts_default_to_30_seconds_for_task_points_and_120_for_move_points() {
    assert_eq!(TASK_POINT_TIMEOUT_SECS, 30);
    assert_eq!(MOVE_POINT_TIMEOUT_SECS, 120);
}

// Schemas.

#[test]
fn the_registry_lists_the_hook_input_and_result_with_both_schemas() {
    let files = schema::registry().render();
    for name in ["HookInput", "HookResult"] {
        for side in ["input", "output"] {
            let file = format!("{name}.{side}.json");
            assert!(files.contains_key(&file), "{file} is not in the registry");
        }
    }
}

#[test]
fn the_result_schema_has_the_plans_keys_and_six_statuses() {
    let files = schema::registry().render();
    let schema: Value = serde_json::from_str(&files["HookResult.output.json"]).unwrap();
    let keys: Vec<&String> = schema["properties"].as_object().unwrap().keys().collect();
    assert_eq!(keys, ["message", "outputs", "retryAfter", "status"]);
    let status = &schema["$defs"]["HookStatus"];
    assert_eq!(status["enum"].as_array().unwrap().len(), 6);
}
