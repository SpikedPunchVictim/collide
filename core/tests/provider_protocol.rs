//! Tests for the provider and observer protocols (ts-0010). The types cover the params, the result and the
//! notification bodies of each message, not the JSON-RPC envelope around them.

use collide_core::item::{Category, LinkTarget};
use collide_core::observer::{ObserverAck, ObserverEvent};
use collide_core::provider::{
    self, ApplyParams, ApplyResult, GetParams, GetResult, InitializeParams, InitializeResult,
    ListParams, ListResult, PreviewResult, ProviderError, ProviderEvent, SourceStale, Task,
    TaskChanged, TaskCollision, TaskInvalid, TaskRemoved, WatchParams, WatchResult,
};
use collide_core::schema;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::fmt::Debug;

/// Parses `doc` as a `T` and writes it back: the result must be the same JSON.
fn round_trip<T: Serialize + DeserializeOwned + Debug>(doc: Value) -> T {
    let parsed: T =
        serde_json::from_value(doc.clone()).unwrap_or_else(|error| panic!("{error} in {doc}"));
    assert_eq!(serde_json::to_value(&parsed).unwrap(), doc);
    parsed
}

/// `doc` with `key` set to a value no type has.
fn with_extra(mut doc: Value) -> Value {
    doc["extra"] = json!(1);
    doc
}

fn refused<T: DeserializeOwned + Debug>(doc: Value) {
    let result = serde_json::from_value::<T>(doc.clone());
    assert!(result.is_err(), "{doc} should be refused, got {result:?}");
}

fn state(status: &str, category: &str) -> Value {
    json!({ "status": status, "category": category })
}

fn task() -> Value {
    json!({
        "key": "plan:0010",
        "id": "ts-0010-yb",
        "aliases": ["ts-0010-yb-provider"],
        "title": "Provider and observer protocols",
        "body": "The contract types.",
        "state": state("active", "active"),
        "version": "9f2c41",
        "checkout": "main",
        "path": ".collide/planning/epics/ep-002-va/tasks/ts-0010-yb.md",
        "kind": "task",
        "links": [{ "type": "blocked-by", "to": "ts-0009-cu" }],
        "owners": ["claude-sonnet"],
        "labels": ["protocol"],
        "collections": ["plan:ep-002-va"],
        "templateFields": { "labelId": "t00-010" },
        "fields": { "epic": "ep-002-va" }
    })
}

fn effect(key: &str) -> Value {
    json!({
        "key": key,
        "version": "a1b2c3",
        "from": state("ready", "ready"),
        "to": state("active", "active")
    })
}

fn initialize_result() -> Value {
    serde_json::from_str(include_str!("fixtures/provider/initialize-one-kind.json")).unwrap()
}

// Provider messages, one round trip each.

#[test]
fn initialize_params_carry_the_packs_config_and_the_root() {
    let params: InitializeParams = round_trip(
        json!({ "config": { "layout": { "root": ".collide/planning" } }, "root": "/work/repo" }),
    );
    assert_eq!(params.root, "/work/repo");
}

#[test]
fn initialize_params_require_the_root() {
    refused::<InitializeParams>(json!({ "config": {} }));
}

#[test]
fn initialize_result_round_trips_with_every_key() {
    let mut doc = initialize_result();
    doc["links"] = json!([{ "name": "blocked-by", "dir": "out", "blocking": true }]);
    let result: InitializeResult = round_trip(doc);
    assert_eq!(result.category_status[&Category::Active], "active");
}

#[test]
fn an_initialize_result_whose_links_declare_no_target_validates_unchanged_and_targets_items() {
    let mut doc = initialize_result();
    doc["links"] = json!([{ "name": "blocked-by", "dir": "out", "blocking": true }]);
    let result: InitializeResult = round_trip(doc);
    assert_eq!(result.links[0].target, LinkTarget::Item);
}

#[test]
fn an_initialize_result_may_declare_a_link_that_targets_a_feature() {
    let mut doc = initialize_result();
    doc["links"] = json!([
        { "name": "blocked-by", "dir": "out", "blocking": true },
        { "name": "implements", "dir": "out", "blocking": false, "target": "feature" }
    ]);
    let result: InitializeResult = round_trip(doc);
    assert_eq!(result.links[1].target, LinkTarget::Feature);
}

#[test]
fn list_params_and_result_round_trip() {
    round_trip::<ListParams>(json!({}));
    round_trip::<ListParams>(json!({ "checkout": "main", "cursor": "c-2" }));
    let result: ListResult = round_trip(json!({
        "tasks": [task()],
        "collections": [{
            "id": "plan:ep-002-va",
            "title": "Skeleton",
            "groups": [{ "name": "contracts", "tasks": ["plan:0009", "plan:0010"] }]
        }],
        "invalid": [],
        "nextCursor": "c-3"
    }));
    assert_eq!(result.tasks.len(), 1);
}

#[test]
fn list_result_carries_invalid_items_in_the_task_invalid_shape() {
    let result: ListResult = round_trip(json!({
        "tasks": [],
        "collections": [],
        "invalid": [
            { "path": ".collide/planning/epics/ep-002-va/tasks/x.md", "checkout": null, "reason": "id does not match" },
            { "path": "a.md", "checkout": "main", "reason": "duplicate" }
        ]
    }));
    assert_eq!(result.invalid.len(), 2);
    assert_eq!(result.invalid[1].checkout.as_deref(), Some("main"));
    let without: ListResult = serde_json::from_value(json!({ "tasks": [] })).unwrap();
    assert!(without.invalid.is_empty());
    refused::<ListResult>(
        json!({ "tasks": [], "invalid": [{ "path": "a.md", "checkout": null }] }),
    );
}

#[test]
fn get_params_and_result_round_trip() {
    round_trip::<GetParams>(json!({ "id": "ts-0010-yb" }));
    round_trip::<GetParams>(json!({ "id": "ts-0010-yb", "checkout": "main" }));
    let found: GetResult = round_trip(json!({ "task": task() }));
    assert!(found.task.is_some());
}

#[test]
fn get_result_without_a_task_round_trips() {
    let missing: GetResult = round_trip(json!({}));
    assert!(missing.task.is_none());
    assert_eq!(serde_json::to_value(&missing).unwrap(), json!({}));
}

#[test]
fn a_task_with_only_its_required_keys_is_valid() {
    let doc = json!({
        "key": "plan:0010",
        "id": "ts-0010-yb",
        "title": "T",
        "state": state("ready", "ready"),
        "version": "v1"
    });
    let task: Task = serde_json::from_value(doc).unwrap();
    assert!(task.links.is_empty() && task.collections.is_empty());
}

fn proposal() -> Value {
    let mut doc = task();
    doc["kind"] = json!("proposal");
    doc["state"] = state("draft", "active");
    doc["features"] = json!([
        { "id": "F1", "title": "Trust prompt", "section": "Packs", "summary": "Asks first." },
        { "id": "F3", "title": "Badge", "section": "", "summary": "The Queue count." }
    ]);
    doc["featureProblems"] = json!([{ "line": 14, "reason": "F1 is used twice" }]);
    doc
}

#[test]
fn a_task_carries_features_and_feature_problems_and_they_round_trip() {
    let parsed: Task = round_trip(proposal());
    assert_eq!(parsed.features.len(), 2);
    assert_eq!(parsed.features[1].section, "");
    assert_eq!(parsed.feature_problems[0].line, 14);
}

#[test]
fn a_task_with_neither_validates_unchanged_and_serializes_neither() {
    let parsed: Task = round_trip(task());
    assert!(parsed.features.is_empty() && parsed.feature_problems.is_empty());
    let value = serde_json::to_value(&parsed).unwrap();
    assert!(value.get("features").is_none() && value.get("featureProblems").is_none());
}

#[test]
fn a_feature_or_a_problem_with_a_missing_or_extra_key_is_refused() {
    let mut no_title = proposal();
    no_title["features"][0]
        .as_object_mut()
        .unwrap()
        .remove("title");
    refused::<Task>(no_title);
    let mut extra = proposal();
    extra["featureProblems"][0]["extra"] = json!(1);
    refused::<Task>(extra);
    let mut snake = proposal();
    snake["feature_problems"] = json!([]);
    refused::<Task>(snake);
}

#[test]
fn watch_params_and_result_round_trip() {
    round_trip::<WatchParams>(json!({ "checkouts": ["main", "wt-1"] }));
    round_trip::<WatchResult>(json!({}));
}

#[test]
fn apply_params_round_trip_for_a_write_and_for_a_create() {
    round_trip::<ApplyParams>(json!({
        "op": "setStatus",
        "id": "ts-0010-yb",
        "args": { "status": "review" },
        "version": "9f2c41",
        "checkout": "main"
    }));
    let create: ApplyParams =
        round_trip(json!({ "op": "create", "args": { "title": "New", "epic": "ep-002-va" } }));
    assert_eq!(create.id, None);
    assert_eq!(create.version, None);
}

#[test]
fn apply_ok_round_trips() {
    round_trip::<ApplyResult>(json!({
        "outcome": "ok",
        "task": task(),
        "version": "d4e5f6",
        "files": ["a/ts-0010-yb.md"],
        "items": [effect("plan:0010"), effect("plan:0009")]
    }));
}

#[test]
fn apply_ok_carries_the_files_touched_and_each_items_version_from_and_to() {
    let result: ApplyResult = serde_json::from_value(json!({
        "outcome": "ok",
        "task": task(),
        "version": "d4e5f6",
        "files": ["a.md", "epic.md"],
        "items": [effect("plan:0010")]
    }))
    .unwrap();
    let ApplyResult::Ok { files, items, .. } = result else {
        panic!("expected ok");
    };
    assert_eq!(files, ["a.md", "epic.md"]);
    assert_eq!(items[0].version, "a1b2c3");
    assert_eq!(items[0].from.as_ref().unwrap().category, Category::Ready);
    assert_eq!(items[0].to.as_ref().unwrap().category, Category::Active);
}

#[test]
fn apply_conflict_carries_the_current_task() {
    let result: ApplyResult = round_trip(json!({ "outcome": "conflict", "current": task() }));
    let ApplyResult::Conflict { current } = result else {
        panic!("expected conflict");
    };
    assert_eq!(current.version, "9f2c41");
}

#[test]
fn apply_refused_carries_the_sources_message() {
    let result: ApplyResult =
        round_trip(json!({ "outcome": "refused", "message": "illegal transition" }));
    let ApplyResult::Refused { message } = result else {
        panic!("expected refused");
    };
    assert_eq!(message, "illegal transition");
}

#[test]
fn apply_results_missing_their_payload_are_refused() {
    refused::<ApplyResult>(json!({ "outcome": "refused" }));
    refused::<ApplyResult>(json!({ "outcome": "conflict" }));
    refused::<ApplyResult>(json!({ "outcome": "ok", "task": task(), "version": "v" }));
    refused::<ApplyResult>(json!({ "outcome": "later", "message": "m" }));
    refused::<ApplyResult>(json!({ "message": "m" }));
}

#[test]
fn preview_ok_carries_items_and_files() {
    let result: PreviewResult =
        round_trip(json!({ "outcome": "ok", "items": [effect("plan:0010")], "files": ["a.md"] }));
    let PreviewResult::Ok { items, files } = result else {
        panic!("expected ok");
    };
    assert_eq!((items.len(), files.len()), (1, 1));
}

#[test]
fn preview_conflict_and_refused_round_trip() {
    let conflict: PreviewResult = round_trip(json!({ "outcome": "conflict", "current": task() }));
    assert!(matches!(conflict, PreviewResult::Conflict { .. }));
    let refusal: PreviewResult =
        round_trip(json!({ "outcome": "refused", "message": "illegal transition" }));
    assert!(matches!(refusal, PreviewResult::Refused { .. }));
}

#[test]
fn an_unknown_key_in_a_preview_result_is_refused() {
    let ok = json!({ "outcome": "ok", "items": [], "files": [] });
    let conflict = json!({ "outcome": "conflict", "current": task() });
    let refusal = json!({ "outcome": "refused", "message": "m" });
    for doc in [ok.clone(), conflict, refusal] {
        refused::<PreviewResult>(with_extra(doc));
    }
    let mut nested = ok;
    nested["items"] = json!([with_extra(effect("plan:0010"))]);
    refused::<PreviewResult>(nested);
    refused::<PreviewResult>(json!({ "items": [], "files": [] }));
    refused::<PreviewResult>(json!({ "outcome": "refused" }));
}

#[test]
fn preview_answers_conflict_and_refused_exactly_as_apply_does() {
    for doc in [
        json!({ "outcome": "conflict", "current": task() }),
        json!({ "outcome": "refused", "message": "illegal transition" }),
    ] {
        let from_apply: ApplyResult = serde_json::from_value(doc.clone()).unwrap();
        let from_preview: PreviewResult = serde_json::from_value(doc.clone()).unwrap();
        assert_eq!(serde_json::to_value(from_apply).unwrap(), doc);
        assert_eq!(serde_json::to_value(from_preview).unwrap(), doc);
    }
    // The schemas of the two shared variants are the same.
    let variant = |root: &str, outcome: &str| {
        let schema = schema_of(root);
        let found = schema["oneOf"]
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["properties"]["outcome"]["const"] == json!(outcome))
            .cloned();
        found.unwrap_or_else(|| panic!("no {outcome} in {root}"))
    };
    for outcome in ["conflict", "refused"] {
        assert_eq!(
            variant("ApplyResult", outcome),
            variant("PreviewResult", outcome),
            "{outcome}"
        );
    }
}

// The watch notifications.

fn notifications() -> Vec<(&'static str, Value)> {
    vec![
        (
            "task.collision",
            json!({ "shortId": "0012", "locations": [
                { "checkout": "main", "path": "a/ts-0012-ua.md" },
                { "checkout": "wt-1", "path": "b/ts-0012-zz.md" }
            ] }),
        ),
        ("source.stale", json!({ "reason": "rate limited" })),
        (
            "task.changed",
            json!({
                "key": "plan:0010",
                "version": "d4e5f6",
                "from": state("ready", "ready"),
                "to": state("active", "active"),
                "diff": { "status": { "from": "ready", "to": "active" } },
                "cause": "worktree"
            }),
        ),
        (
            "task.changed with actor",
            json!({
                "key": "github:acme/edge#212",
                "version": "u1",
                "from": null,
                "to": state("ready", "ready"),
                "diff": { "labels": { "added": ["x"], "removed": [] } },
                "cause": "remote",
                "actor": "octocat",
                "automated": true
            }),
        ),
        (
            "task.changed with checkout",
            json!({
                "key": "plan:0010",
                "version": "d4e5f6",
                "from": state("ready", "ready"),
                "to": state("active", "active"),
                "diff": {},
                "cause": "head",
                "checkout": "wt-1"
            }),
        ),
        (
            "task.renamed",
            json!({ "from": "plan:0010", "to": "plan:0011", "checkout": "main" }),
        ),
        (
            "task.removed",
            json!({ "key": "plan:0010", "cause": "transferred" }),
        ),
        (
            "task.removed with checkout",
            json!({ "key": "plan:0010", "cause": "deleted", "checkout": "wt-1" }),
        ),
        (
            "task.invalid",
            json!({ "path": "a/bad.md", "checkout": "main", "reason": "frontmatter over its cap" }),
        ),
    ]
}

#[test]
fn every_watch_notification_round_trips() {
    for (name, doc) in notifications() {
        let name = name.split(' ').next().unwrap();
        match name {
            "task.collision" => drop(round_trip::<TaskCollision>(doc)),
            "source.stale" => drop(round_trip::<SourceStale>(doc)),
            "task.changed" => drop(round_trip::<TaskChanged>(doc)),
            "task.renamed" => drop(round_trip::<collide_core::change::Rename>(doc)),
            "task.removed" => drop(round_trip::<TaskRemoved>(doc)),
            "task.invalid" => drop(round_trip::<TaskInvalid>(doc)),
            other => panic!("no type for {other}"),
        }
    }
}

#[test]
fn a_notification_with_an_unknown_key_is_refused() {
    for (name, doc) in notifications() {
        let doc = with_extra(doc);
        match name.split(' ').next().unwrap() {
            "task.collision" => refused::<TaskCollision>(doc),
            "source.stale" => refused::<SourceStale>(doc),
            "task.changed" => refused::<TaskChanged>(doc),
            "task.renamed" => refused::<collide_core::change::Rename>(doc),
            "task.removed" => refused::<TaskRemoved>(doc),
            "task.invalid" => refused::<TaskInvalid>(doc),
            other => panic!("no type for {other}"),
        }
    }
}

#[test]
fn every_watch_notification_reads_into_the_event_of_its_method() {
    for (name, doc) in notifications() {
        let method = name.split(' ').next().unwrap();
        let event = ProviderEvent::from_notification(method, doc.clone());
        let read = match (method, &event) {
            ("task.collision", ProviderEvent::Collision(body)) => serde_json::to_value(body),
            ("source.stale", ProviderEvent::Stale(body)) => serde_json::to_value(body),
            ("task.changed", ProviderEvent::Changed(body)) => serde_json::to_value(body),
            ("task.renamed", ProviderEvent::Renamed(body)) => serde_json::to_value(body),
            ("task.removed", ProviderEvent::Removed(body)) => serde_json::to_value(body),
            ("task.invalid", ProviderEvent::Invalid(body)) => serde_json::to_value(body),
            _ => panic!("{name} read as {event:?}"),
        };
        assert_eq!(read.unwrap(), doc, "{name}");
    }
}

#[test]
fn a_notification_of_another_method_is_kept_as_unreadable_with_its_method() {
    let event = ProviderEvent::from_notification("task.exploded", json!({}));
    let ProviderEvent::Unreadable { method, detail } = event else {
        panic!("{event:?}");
    };
    assert_eq!(method, "task.exploded");
    assert!(detail.contains("not a notification"), "{detail}");
}

#[test]
fn a_notification_whose_params_do_not_read_is_kept_as_unreadable_with_the_reason() {
    for (name, doc) in notifications() {
        let method = name.split(' ').next().unwrap();
        let event = ProviderEvent::from_notification(method, with_extra(doc));
        let ProviderEvent::Unreadable {
            method: kept,
            detail,
        } = event
        else {
            panic!("{name} read with an unknown key: {event:?}");
        };
        assert_eq!(kept, method);
        assert!(detail.contains("extra"), "{detail}");
    }
}

#[test]
fn task_invalid_carries_path_checkout_and_reason() {
    let doc = json!({ "path": "a/bad.md", "checkout": "main", "reason": "two files share one id" });
    let invalid: TaskInvalid = round_trip(doc);
    assert_eq!(invalid.path, "a/bad.md");
    assert_eq!(invalid.checkout.as_deref(), Some("main"));
    assert_eq!(invalid.reason, "two files share one id");
    // A global source has no checkout: the key is still written, as null.
    let global = TaskInvalid {
        path: "issue 212".into(),
        checkout: None,
        reason: "no title".into(),
    };
    let written = serde_json::to_value(global).unwrap();
    assert_eq!(written.get("checkout"), Some(&Value::Null));
}

#[test]
fn task_changed_and_task_removed_name_the_checkout_only_when_ref_scoped() {
    let list = notifications();
    for (name, doc) in &list {
        let has = doc.get("checkout").is_some();
        match name.split(' ').next().unwrap() {
            "task.changed" => {
                let changed: TaskChanged = serde_json::from_value(doc.clone()).unwrap();
                assert_eq!(changed.checkout.is_some(), has, "{name}");
            }
            "task.removed" => {
                let removed: TaskRemoved = serde_json::from_value(doc.clone()).unwrap();
                assert_eq!(removed.checkout.is_some(), has, "{name}");
            }
            _ => {}
        }
    }
    assert!(list.iter().any(|(n, _)| *n == "task.changed with checkout"));
    assert!(list.iter().any(|(n, _)| *n == "task.removed with checkout"));
}

#[test]
fn task_invalid_without_a_reason_or_a_path_is_refused() {
    refused::<TaskInvalid>(json!({ "path": "a.md", "checkout": "main" }));
    refused::<TaskInvalid>(json!({ "checkout": "main", "reason": "r" }));
}

#[test]
fn a_change_cause_or_removal_cause_outside_its_list_is_refused() {
    let mut changed = notifications()[2].1.clone();
    for cause in ["head", "worktree", "remote", "journal", "startup"] {
        changed["cause"] = json!(cause);
        assert!(
            serde_json::from_value::<TaskChanged>(changed.clone()).is_ok(),
            "{cause}"
        );
    }
    changed["cause"] = json!("manual");
    refused::<TaskChanged>(changed);
    refused::<TaskRemoved>(json!({ "key": "plan:0010", "cause": "vanished" }));
}

#[test]
fn a_task_changed_diff_entry_with_a_missing_side_is_refused() {
    let mut changed = notifications()[2].1.clone();
    changed["diff"] = json!({ "status": { "to": "active" } });
    refused::<TaskChanged>(changed);
}

#[test]
fn a_task_renamed_names_the_checkout_but_a_change_rename_need_not() {
    let rename: collide_core::change::Rename =
        serde_json::from_value(json!({ "from": "a", "to": "b" })).unwrap();
    assert_eq!(rename.checkout, None);
    assert_eq!(
        serde_json::to_value(&rename).unwrap(),
        json!({ "from": "a", "to": "b" })
    );
}

// An unknown key at every level of an apply result and of initialize.

#[test]
fn an_unknown_key_in_an_apply_result_is_refused() {
    let ok = json!({ "outcome": "ok", "task": task(), "version": "v", "files": [], "items": [] });
    let conflict = json!({ "outcome": "conflict", "current": task() });
    let refusal = json!({ "outcome": "refused", "message": "m" });
    for doc in [ok.clone(), conflict, refusal] {
        refused::<ApplyResult>(with_extra(doc));
    }
    let mut nested = ok.clone();
    nested["items"] = json!([with_extra(effect("plan:0010"))]);
    refused::<ApplyResult>(nested);
    let mut in_task = ok;
    in_task["task"]["extra"] = json!(1);
    refused::<ApplyResult>(in_task);
}

#[test]
fn an_unknown_key_in_initialize_is_refused() {
    refused::<InitializeResult>(with_extra(initialize_result()));
    let mut in_capabilities = initialize_result();
    in_capabilities["capabilities"]["extra"] = json!(true);
    refused::<InitializeResult>(in_capabilities);
    let mut in_op = initialize_result();
    in_op["ops"][0]["extra"] = json!(1);
    refused::<InitializeResult>(in_op);
    refused::<InitializeParams>(json!({ "config": {}, "root": "/work/repo", "extra": 1 }));
}

#[test]
fn a_scope_outside_global_and_ref_is_refused() {
    let mut doc = initialize_result();
    doc["scope"] = json!("local");
    refused::<InitializeResult>(doc);
}

// One kind and no links.

#[test]
fn a_provider_that_declares_one_kind_and_no_links_validates_unchanged() {
    let doc = initialize_result();
    assert!(doc.get("links").is_none());
    let result: InitializeResult = round_trip(doc);
    assert_eq!(result.kinds.len(), 1);
    assert!(result.links.is_empty());
    assert_eq!(provider::check_initialize(&result), Ok(()));
}

#[test]
fn a_provider_that_declares_no_kinds_and_no_links_validates_unchanged() {
    let mut doc = initialize_result();
    doc.as_object_mut().unwrap().remove("kinds");
    let result: InitializeResult = round_trip(doc);
    assert!(result.kinds.is_empty() && result.links.is_empty());
    assert_eq!(provider::check_initialize(&result), Ok(()));
}

#[test]
fn an_op_may_carry_an_argument_schema_and_the_checkouts_it_may_target() {
    let result: InitializeResult = serde_json::from_value(initialize_result()).unwrap();
    assert_eq!(result.ops[0].checkouts, ["main"]);
    assert_eq!(
        result.ops[0].args.as_ref().unwrap()["required"],
        json!(["status"])
    );
    assert!(result.ops[1].args.is_none() && result.ops[1].checkouts.is_empty());
}

// The reserved op name.

fn with_ops(top: &[&str], kind: &[&str]) -> InitializeResult {
    let mut doc = initialize_result();
    doc["ops"] = top.iter().map(|name| json!({ "name": name })).collect();
    doc["kinds"][0]["ops"] = kind.iter().map(|name| json!({ "name": name })).collect();
    serde_json::from_value(doc).unwrap()
}

#[test]
fn a_provider_op_named_move_is_refused() {
    assert_eq!(
        provider::check_initialize(&with_ops(&["setStatus", "move"], &[])),
        Err(vec![ProviderError::ReservedOp {
            op: "move".into(),
            kind: None
        }])
    );
}

#[test]
fn a_kind_op_named_move_is_refused_with_its_kind() {
    assert_eq!(
        provider::check_initialize(&with_ops(&["setStatus"], &["move"])),
        Err(vec![ProviderError::ReservedOp {
            op: "move".into(),
            kind: Some("task".into())
        }])
    );
}

#[test]
fn rehome_and_other_names_are_allowed() {
    let result = with_ops(&["rehome", "moveTo", "Move"], &["rehome"]);
    assert_eq!(provider::check_initialize(&result), Ok(()));
}

#[test]
fn every_refusal_is_reported() {
    let errors = provider::check_initialize(&with_ops(&["move"], &["move"])).unwrap_err();
    assert_eq!(errors.len(), 2);
    assert!(errors[0].to_string().contains("`move`"));
    assert!(errors[0].to_string().contains("Share"));
}

// Framing.

#[test]
fn a_message_with_a_newline_in_its_text_is_still_one_line() {
    let mut doc = task();
    doc["title"] = json!("two\nlines");
    let line = serde_json::to_string(&json!({ "task": doc })).unwrap();
    assert!(!line.contains('\n'));
}

// The observer protocol.

/// One observer line: the event envelope with a body. `body` carries the event's name in its `event` tag.
fn event_line(seq: u64, body: Value) -> Value {
    json!({
        "seq": seq,
        "wall_ms": 1_700_000_000_000u64,
        "actor": { "kind": "workflow", "run": "r-77" },
        "experiment": null,
        "body": body
    })
}

#[test]
fn an_observer_event_has_a_sequence_number_and_a_typed_body() {
    let event: ObserverEvent = round_trip(event_line(
        41,
        json!({ "event": "run.transition", "run": "r-77", "from": null, "to": { "state": "running" } }),
    ));
    assert_eq!(event.0.seq, 41);
    assert_eq!(event.0.body.name(), "run.transition");
}

#[test]
fn an_observer_event_with_an_unknown_key_or_a_missing_one_is_refused() {
    let doc = event_line(
        1,
        json!({ "event": "run.transition", "run": "r-77", "from": null, "to": { "state": "running" } }),
    );
    refused::<ObserverEvent>(with_extra(doc.clone()));
    for key in ["seq", "wall_ms", "actor", "body"] {
        let mut missing = doc.clone();
        missing.as_object_mut().unwrap().remove(key);
        refused::<ObserverEvent>(missing);
    }
    let mut extra_in_body = doc.clone();
    extra_in_body["body"]["extra"] = json!(1);
    refused::<ObserverEvent>(extra_in_body);
}

#[test]
fn an_observer_event_with_an_unknown_event_name_is_refused() {
    refused::<ObserverEvent>(event_line(
        1,
        json!({ "event": "run.started", "run": "r-77" }),
    ));
}

#[test]
fn an_observer_acknowledgement_names_the_sequence_number_it_covers() {
    let ack: ObserverAck = round_trip(json!({ "seq": 41 }));
    assert_eq!(ack.seq, 41);
    refused::<ObserverAck>(json!({}));
    refused::<ObserverAck>(json!({ "seq": 41, "extra": 1 }));
}

#[test]
fn a_sequence_number_is_a_non_negative_integer() {
    refused::<ObserverAck>(json!({ "seq": -1 }));
    refused::<ObserverAck>(json!({ "seq": 1.5 }));
    refused::<ObserverAck>(json!({ "seq": "41" }));
    let mut line = event_line(
        1,
        json!({ "event": "budget.exceeded", "run": "r-77", "scope": "run" }),
    );
    line["seq"] = json!(-1);
    refused::<ObserverEvent>(line);
}

// The schemas.

fn schema_of(root: &str) -> Value {
    let text = &schema::registry().render()[&format!("{root}.input.json")];
    serde_json::from_str(text).unwrap()
}

fn keys_of(root: &str) -> BTreeSet<String> {
    let node = schema_of(root);
    let variants = node["oneOf"].as_array().cloned();
    variants
        .unwrap_or_else(|| vec![node])
        .iter()
        .flat_map(|part| part["properties"].as_object().into_iter().flatten())
        .map(|(key, _)| key.clone())
        .collect()
}

fn set(keys: &[&str]) -> BTreeSet<String> {
    keys.iter().map(|key| key.to_string()).collect()
}

#[test]
fn the_registry_holds_every_protocol_root() {
    let names: BTreeSet<String> = schema::registry().render().into_keys().collect();
    for name in [
        "ApplyParams",
        "ApplyResult",
        "GetParams",
        "GetResult",
        "InitializeParams",
        "InitializeResult",
        "ListParams",
        "ListResult",
        "ObserverAck",
        "ObserverEvent",
        "PreviewResult",
        "Rename",
        "SourceStale",
        "Task",
        "TaskChanged",
        "TaskCollision",
        "TaskInvalid",
        "TaskRemoved",
        "WatchParams",
        "WatchResult",
    ] {
        for side in ["input", "output"] {
            assert!(
                names.contains(&format!("{name}.{side}.json")),
                "{name} {side}"
            );
        }
    }
}

#[test]
fn the_protocol_keys_are_in_the_schema() {
    assert!(keys_of("Task").contains("features") && keys_of("Task").contains("featureProblems"));
    assert_eq!(
        keys_of("InitializeResult"),
        set(&[
            "capabilities",
            "categoryStatus",
            "fields",
            "idPattern",
            "kinds",
            "links",
            "ops",
            "scope",
            "shortIdPattern",
            "statuses",
            "templateFields"
        ])
    );
    assert_eq!(
        keys_of("ApplyResult"),
        set(&[
            "current", "files", "items", "message", "outcome", "task", "version"
        ])
    );
    assert_eq!(keys_of("TaskInvalid"), set(&["checkout", "path", "reason"]));
    assert_eq!(schema_of("ObserverEvent")["$ref"], "#/$defs/Event");
    assert_eq!(
        keys_of("Event"),
        set(&["actor", "body", "experiment", "seq", "wall_ms"])
    );
    assert_eq!(keys_of("ObserverAck"), set(&["seq"]));
    assert_eq!(
        keys_of("TaskChanged"),
        set(&[
            "actor",
            "automated",
            "cause",
            "checkout",
            "diff",
            "from",
            "key",
            "to",
            "version"
        ])
    );
}

#[test]
fn kinds_and_links_are_optional_in_the_initialize_schema() {
    let schema = schema_of("InitializeResult");
    let required: Vec<&str> = schema["required"]
        .as_array()
        .unwrap()
        .iter()
        .map(|key| key.as_str().unwrap())
        .collect();
    assert!(!required.contains(&"kinds"), "{required:?}");
    assert!(!required.contains(&"links"), "{required:?}");
    assert!(required.contains(&"scope"), "{required:?}");
}

#[test]
fn every_apply_result_variant_and_notification_schema_refuses_unknown_keys() {
    let apply = schema_of("ApplyResult");
    let variants = apply["oneOf"].as_array().unwrap();
    assert_eq!(variants.len(), 3);
    for variant in variants {
        assert_eq!(variant["additionalProperties"], json!(false), "{variant}");
    }
    for root in [
        "TaskCollision",
        "SourceStale",
        "TaskChanged",
        "Rename",
        "TaskRemoved",
        "TaskInvalid",
    ] {
        assert_eq!(
            schema_of(root)["additionalProperties"],
            json!(false),
            "{root}"
        );
    }
}
