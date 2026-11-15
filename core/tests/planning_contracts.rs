//! Tests for the planning contracts (ts-0009): item kinds and their statuses, the derived-point check, the
//! `change` payload of the task points, and the schemas of the new keys.

use collide_core::change::{Change, TaskPointPayload};
use collide_core::item::{self, Category, KindDecl, KindError, Link, PlanningFields};
use collide_core::pack::{PackManifest, RunBudget};
use collide_core::points::{self, DerivedPoint, PointError};
use collide_core::schema;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

// Kinds and statuses.

fn category_of(kind: &str, status: &str) -> Result<Category, KindError> {
    item::category_of(&item::planning_kinds(), kind, status)
}

#[test]
fn a_spike_status_maps_to_its_category() {
    let expected = [
        ("planned", Category::Ready),
        ("running", Category::Active),
        ("results", Category::Review),
        ("concluded", Category::Done),
        ("dropped", Category::Dropped),
    ];
    for (status, category) in expected {
        assert_eq!(category_of("spike", status), Ok(category), "{status}");
    }
}

#[test]
fn a_task_and_an_epic_use_the_five_categories_as_statuses() {
    let expected = [
        ("ready", Category::Ready),
        ("active", Category::Active),
        ("review", Category::Review),
        ("done", Category::Done),
        ("dropped", Category::Dropped),
    ];
    for kind in ["task", "epic"] {
        for (status, category) in expected {
            assert_eq!(category_of(kind, status), Ok(category), "{kind} {status}");
        }
    }
}

#[test]
fn a_status_outside_its_kinds_list_fails() {
    for (kind, status) in [
        ("spike", "active"),
        ("spike", "ready"),
        ("task", "concluded"),
        ("task", "backlog"),
        ("epic", "running"),
    ] {
        assert_eq!(
            category_of(kind, status),
            Err(KindError::UnknownStatus {
                kind: kind.into(),
                status: status.into()
            }),
            "{kind} {status}"
        );
    }
}

#[test]
fn an_unknown_kind_fails() {
    assert_eq!(
        category_of("proposal", "draft"),
        Err(KindError::UnknownKind("proposal".into()))
    );
}

#[test]
fn a_kind_declares_statuses_transitions_ops_and_roles() {
    let doc = json!({
        "name": "question",
        "statuses": [
            { "name": "open", "category": "ready" },
            { "name": "resolved", "category": "done" }
        ],
        "transitions": [{ "from": "open", "to": "resolved" }],
        "ops": [{ "name": "resolve", "movesTo": "done" }, { "name": "link" }],
        "roles": ["decision"]
    });
    let kind: KindDecl = serde_json::from_value(doc.clone()).unwrap();
    assert_eq!(kind.ops[0].moves_to, Some(Category::Done));
    assert_eq!(kind.ops[1].moves_to, None);
    let mut back = serde_json::to_value(&kind).unwrap();
    // `movesTo` is left out when an op moves nothing.
    assert_eq!(back["ops"][1], json!({ "name": "link" }));
    back["ops"][1] = json!({ "name": "link" });
    assert_eq!(back, doc);
}

#[test]
fn a_kind_with_only_a_name_and_statuses_is_valid() {
    let doc = json!({ "name": "task", "statuses": [{ "name": "ready", "category": "ready" }] });
    assert!(serde_json::from_value::<KindDecl>(doc).is_ok());
}

#[test]
fn a_status_with_an_unknown_category_is_rejected() {
    let doc = json!({ "name": "task", "statuses": [{ "name": "ready", "category": "soon" }] });
    assert!(serde_json::from_value::<KindDecl>(doc).is_err());
}

#[test]
fn an_unknown_key_on_a_kind_or_an_op_is_rejected() {
    let kind = json!({ "name": "task", "statuses": [], "extra": 1 });
    assert!(serde_json::from_value::<KindDecl>(kind).is_err());
    let op = json!({ "name": "task", "statuses": [], "ops": [{ "name": "x", "extra": 1 }] });
    assert!(serde_json::from_value::<KindDecl>(op).is_err());
}

// Links, owners and the checklist field.

#[test]
fn planning_fields_hold_kind_links_owners_and_a_checklist() {
    let doc = json!({
        "kind": "spike",
        "links": [{ "type": "blocked-by", "to": "ts-0008-mx", "blocking": true }],
        "owners": ["claude-sonnet"],
        "checklist": {
            "template": "collide/plan/spike@1",
            "items": [
                { "id": "design", "state": "open" },
                { "id": "evidence", "state": "suggested", "evidence": "sp-0010-bn" },
                { "id": "scope", "state": "n/a", "by": "human", "date": "2026-11-14", "reason": "none" }
            ]
        }
    });
    let fields: PlanningFields = serde_json::from_value(doc.clone()).unwrap();
    assert_eq!(
        serde_json::to_value(&fields).unwrap()["checklist"],
        doc["checklist"]
    );
    assert_eq!(fields.owners, ["claude-sonnet"]);
}

#[test]
fn a_link_without_blocking_leaves_it_to_the_link_type() {
    let doc = json!({ "type": "blocked-by", "to": "ts-0008-mx" });
    let link: Link = serde_json::from_value(doc.clone()).unwrap();
    assert_eq!(link.blocking, None);
    assert_eq!(serde_json::to_value(&link).unwrap(), doc);
    let explicit: Link =
        serde_json::from_value(json!({ "type": "informs", "to": "x", "blocking": false })).unwrap();
    assert_eq!(explicit.blocking, Some(false));
}

#[test]
fn a_checklist_state_outside_the_four_is_rejected() {
    let doc = json!({ "kind": "task", "checklist": { "template": "t@1", "items": [{ "id": "a", "state": "maybe" }] } });
    assert!(serde_json::from_value::<PlanningFields>(doc).is_err());
}

#[test]
fn an_unknown_key_in_planning_fields_is_rejected() {
    let doc = json!({ "kind": "task", "assignee": "x" });
    assert!(serde_json::from_value::<PlanningFields>(doc).is_err());
}

#[test]
fn a_run_budget_is_a_total_in_dollars() {
    let budget: RunBudget = serde_json::from_value(json!({ "usd": 20 })).unwrap();
    assert_eq!(budget.usd, 20.0);
    assert!(serde_json::from_value::<RunBudget>(json!({ "usd": 20, "tokens": 1 })).is_err());
    assert!(serde_json::from_value::<RunBudget>(json!({})).is_err());
}

// Derived points.

const KNOWN: &[&str] = &["task.before_status", "task.after_status"];

fn derived(entries: &[(&str, &str)]) -> BTreeMap<String, DerivedPoint> {
    entries
        .iter()
        .map(|(name, on)| {
            let point = json!({ "on": on });
            (name.to_string(), serde_json::from_value(point).unwrap())
        })
        .collect()
}

#[test]
fn a_derived_point_on_a_known_core_point_of_its_side_is_accepted() {
    let points = derived(&[
        ("decision.before_decide", "task.before_status"),
        ("decision.after_decide", "task.after_status"),
    ]);
    assert_eq!(points::check_derived_points(&points, KNOWN), Ok(()));
}

#[test]
fn a_derived_point_naming_an_unknown_core_point_is_refused() {
    let points = derived(&[("decision.before_decide", "task.before_nothing")]);
    assert_eq!(
        points::check_derived_points(&points, KNOWN),
        Err(vec![PointError::UnknownCorePoint {
            point: "decision.before_decide".into(),
            on: "task.before_nothing".into()
        }])
    );
}

#[test]
fn a_derived_point_from_another_derived_point_is_refused() {
    let points = derived(&[
        ("decision.before_decide", "task.before_status"),
        ("decision.before_again", "decision.before_decide"),
    ]);
    assert_eq!(
        points::check_derived_points(&points, KNOWN),
        Err(vec![PointError::DerivedFromDerived {
            point: "decision.before_again".into(),
            on: "decision.before_decide".into()
        }])
    );
}

#[test]
fn a_derived_point_on_the_other_side_of_its_core_point_is_refused() {
    let points = derived(&[
        ("decision.before_decide", "task.after_status"),
        ("decision.after_decide", "task.before_status"),
    ]);
    let refused = |point: &str, on: &str| PointError::SideMismatch {
        point: point.into(),
        on: on.into(),
    };
    assert_eq!(
        points::check_derived_points(&points, KNOWN),
        Err(vec![
            refused("decision.after_decide", "task.before_status"),
            refused("decision.before_decide", "task.after_status"),
        ])
    );
}

#[test]
fn a_derived_point_name_with_no_side_is_refused() {
    let points = derived(&[("decision.decide", "task.before_status")]);
    assert_eq!(
        points::check_derived_points(&points, KNOWN),
        Err(vec![PointError::NoSide {
            point: "decision.decide".into()
        }])
    );
}

#[test]
fn the_known_core_points_are_an_input_to_the_check() {
    let points = derived(&[("item.before_rename", "item.before_op")]);
    assert!(points::check_derived_points(&points, KNOWN).is_err());
    assert_eq!(
        points::check_derived_points(&points, &["item.before_op"]),
        Ok(())
    );
}

#[test]
fn the_core_points_this_item_defines_pass_their_own_check() {
    for core in points::CORE_POINTS {
        let side = if core.contains(".before_") {
            "before"
        } else {
            "after"
        };
        let name = format!("pack.{side}_x");
        let points = derived(&[(name.as_str(), core)]);
        assert_eq!(
            points::check_derived_points(&points, points::CORE_POINTS),
            Ok(()),
            "{core}"
        );
    }
}

#[test]
fn the_five_new_task_points_are_core_points() {
    for point in [
        "task.after_status",
        "task.before_op",
        "task.after_op",
        "task.before_move",
        "task.after_move",
    ] {
        assert!(points::CORE_POINTS.contains(&point), "{point}");
    }
}

#[test]
fn a_derived_point_reads_as_the_plan_writes_it() {
    let doc = json!({
        "on": "task.before_status",
        "match": ["done", "dropped"],
        "when": "task.kind in config.roles.decision && change.op != \"supersede\""
    });
    let point: DerivedPoint = serde_json::from_value(doc.clone()).unwrap();
    assert_eq!(serde_json::to_value(&point).unwrap(), doc);
    let extra = json!({ "on": "task.before_status", "extra": 1 });
    assert!(serde_json::from_value::<DerivedPoint>(extra).is_err());
}

#[test]
fn a_pack_manifest_holds_points_and_checklists_and_both_are_optional() {
    let doc = json!({
        "points": { "spike.before_conclude": { "on": "task.before_status", "match": ["done"] } },
        "checklists": {
            "spike": {
                "version": 1,
                "gates": "ready",
                "items": [{ "id": "design", "text": "Designed", "required": true, "when": "kind == \"spike\"" }]
            }
        }
    });
    let manifest: PackManifest = serde_json::from_value(doc.clone()).unwrap();
    assert_eq!(
        points::check_derived_points(&manifest.points, KNOWN),
        Ok(())
    );
    assert_eq!(serde_json::to_value(&manifest).unwrap(), doc);
    let empty: PackManifest = serde_json::from_value(json!({})).unwrap();
    assert_eq!(empty, PackManifest::default());
    assert!(serde_json::from_value::<PackManifest>(json!({ "views": {} })).is_err());
}

// The `change` payload.

fn base_change() -> Value {
    json!({
        "op": "setStatus",
        "args": { "status": "review" },
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
        "files": ["tasks/ts-0009.md"]
    })
}

fn parse_change(doc: &Value) -> Change {
    serde_json::from_value(doc.clone()).unwrap()
}

#[test]
fn change_at_update_maps_a_field_to_from_and_to_and_a_set_to_added_and_removed() {
    let mut doc = base_change();
    doc["diff"]["labels"] = json!({ "added": ["urgent"], "removed": ["later"] });
    let change = parse_change(&doc);
    assert_eq!(change.diff.len(), 2);
    assert_eq!(serde_json::to_value(&change).unwrap(), doc);
}

#[test]
fn change_at_create_has_a_null_from_and_a_null_from_in_each_field() {
    let mut doc = base_change();
    doc["op"] = json!("create");
    doc["from"] = Value::Null;
    doc["diff"] = json!({ "title": { "from": null, "to": "New" } });
    let change = parse_change(&doc);
    assert_eq!(change.from, None);
    assert!(change.to.is_some());
    let out = serde_json::to_value(&change).unwrap();
    assert_eq!(out["from"], Value::Null);
    assert_eq!(out["diff"]["title"]["from"], Value::Null);
    assert_eq!(out, doc);
}

#[test]
fn change_at_delete_has_a_null_to_and_a_null_to_in_each_field() {
    let mut doc = base_change();
    doc["op"] = json!("delete");
    doc["to"] = Value::Null;
    doc["diff"] = json!({ "title": { "from": "Old", "to": null } });
    let change = parse_change(&doc);
    assert!(change.from.is_some());
    assert_eq!(change.to, None);
    let out = serde_json::to_value(&change).unwrap();
    assert_eq!(out["to"], Value::Null);
    assert_eq!(out["diff"]["title"]["to"], Value::Null);
    assert_eq!(out, doc);
}

#[test]
fn a_diff_entry_with_a_missing_side_or_mixed_keys_is_rejected() {
    for entry in [
        json!({ "to": "x" }),
        json!({ "from": "x" }),
        json!({ "added": [] }),
        json!({ "from": "x", "to": "y", "added": [] }),
    ] {
        let mut doc = base_change();
        doc["diff"]["field"] = entry.clone();
        assert!(serde_json::from_value::<Change>(doc).is_err(), "{entry}");
    }
}

#[test]
fn a_run_made_write_has_via_run_a_null_op_and_an_agent_actor() {
    let mut doc = base_change();
    doc["op"] = Value::Null;
    doc["via"] = json!("run");
    doc["actor"] = json!({ "kind": "agent", "run": "r-77", "phase": "build" });
    assert_eq!(serde_json::to_value(parse_change(&doc)).unwrap(), doc);
}

#[test]
fn an_actor_is_a_kind_with_an_optional_run_and_phase() {
    for actor in [
        json!({ "kind": "person" }),
        json!({ "kind": "workflow", "run": "r-77" }),
        json!({ "kind": "agent" }),
        json!({ "kind": "agent", "run": "r-77", "phase": "build" }),
    ] {
        let mut doc = base_change();
        doc["actor"] = actor.clone();
        assert_eq!(
            serde_json::to_value(parse_change(&doc)).unwrap(),
            doc,
            "{actor}"
        );
    }
}

#[test]
fn an_actor_with_an_unknown_kind_or_key_is_refused() {
    for actor in [
        json!({ "kind": "robot" }),
        json!({ "kind": "person", "extra": 1 }),
        json!({ "run": "r-77" }),
    ] {
        let mut doc = base_change();
        doc["actor"] = actor.clone();
        assert!(serde_json::from_value::<Change>(doc).is_err(), "{actor}");
    }
}

#[test]
fn via_is_one_of_the_five_routes() {
    for via in ["ui", "workflow", "share", "integrate", "run"] {
        let mut doc = base_change();
        doc["via"] = json!(via);
        assert!(serde_json::from_value::<Change>(doc).is_ok(), "{via}");
    }
    let mut doc = base_change();
    doc["via"] = json!("cli");
    assert!(serde_json::from_value::<Change>(doc).is_err());
}

#[test]
fn change_carries_a_batch_effects_effect_of_and_renamed() {
    let mut doc = base_change();
    doc["batch"] = json!({ "id": "b-1", "index": 0, "size": 3 });
    doc["effects"] = json!([{
        "key": "plan:ts-0010-yb",
        "version": "9f2c41",
        "from": { "status": "ready", "category": "ready" },
        "to": { "status": "done", "category": "done" }
    }]);
    doc["effectOf"] = json!("plan:ts-0009-cu");
    doc["renamed"] = json!([{ "from": "plan:ts-0009-cu", "to": "plan:ts-0009-xx" }]);
    assert_eq!(serde_json::to_value(parse_change(&doc)).unwrap(), doc);
}

#[test]
fn a_batch_index_and_size_cannot_be_negative() {
    let mut doc = base_change();
    doc["batch"] = json!({ "id": "b-1", "index": -1, "size": 3 });
    assert!(serde_json::from_value::<Change>(doc).is_err());
}

#[test]
fn an_unknown_key_in_change_or_a_nested_object_is_rejected() {
    let mut doc = base_change();
    doc["extra"] = json!(1);
    assert!(serde_json::from_value::<Change>(doc).is_err());
    let mut doc = base_change();
    doc["to"]["extra"] = json!(1);
    assert!(serde_json::from_value::<Change>(doc).is_err());
}

#[test]
fn a_task_point_payload_adds_change_and_the_latest_run() {
    let doc = json!({
        "change": base_change(),
        "latestRun": { "id": "r-77", "workflow": "collide/plan/spike", "outcome": "concluded", "state": {} }
    });
    let payload: TaskPointPayload = serde_json::from_value(doc.clone()).unwrap();
    assert!(payload.latest_run.is_some());
    assert_eq!(serde_json::to_value(&payload).unwrap(), doc);
    let mut inside_a_run = doc;
    inside_a_run["latestRun"] = Value::Null;
    let payload: TaskPointPayload = serde_json::from_value(inside_a_run).unwrap();
    assert_eq!(payload.latest_run, None);
}

// The schemas of the new keys.

/// The keys of a type in the real registry's input schemas: the root type or one in `$defs`.
fn keys_of(root: &str, def: Option<&str>) -> BTreeSet<String> {
    let text = &schema::registry().render()[&format!("{root}.input.json")];
    let mut node: Value = serde_json::from_str(text).unwrap();
    if let Some(def) = def {
        node = node["$defs"][def].clone();
    }
    let mut keys = BTreeSet::new();
    let variants = node["oneOf"]
        .as_array()
        .or(node["anyOf"].as_array())
        .cloned();
    for part in variants.unwrap_or_else(|| vec![node.clone()]) {
        let Some(properties) = part["properties"].as_object() else {
            continue;
        };
        keys.extend(properties.keys().cloned());
    }
    keys
}

fn set(keys: &[&str]) -> BTreeSet<String> {
    keys.iter().map(|key| key.to_string()).collect()
}

#[test]
fn the_registry_holds_the_planning_types() {
    let names: BTreeSet<String> = schema::registry().render().into_keys().collect();
    for name in [
        "Change",
        "KindDecl",
        "LinkTypeDecl",
        "PackManifest",
        "PlanningFields",
        "RunBudget",
        "TaskPointPayload",
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
fn the_pack_keys_points_and_checklists_are_in_the_schema() {
    assert_eq!(
        keys_of("PackManifest", None),
        set(&["checklists", "points"])
    );
    assert_eq!(
        keys_of("PackManifest", Some("DerivedPoint")),
        set(&["match", "on", "when"])
    );
    assert_eq!(
        keys_of("PackManifest", Some("ChecklistTemplate")),
        set(&["gates", "items", "version"])
    );
    assert_eq!(
        keys_of("PackManifest", Some("ChecklistTemplateItem")),
        set(&["id", "required", "text", "when"])
    );
}

#[test]
fn the_change_keys_are_in_the_schema() {
    let change = [
        "actor",
        "args",
        "batch",
        "categoryMoved",
        "checkout",
        "diff",
        "effectOf",
        "effects",
        "files",
        "from",
        "op",
        "renamed",
        "to",
        "via",
    ];
    assert_eq!(keys_of("Change", None), set(&change));
    assert_eq!(
        keys_of("TaskPointPayload", None),
        set(&["change", "latestRun"])
    );
    assert_eq!(
        keys_of("TaskPointPayload", Some("LatestRun")),
        set(&["id", "outcome", "state", "workflow"])
    );
    assert_eq!(
        keys_of("Change", Some("Actor")),
        set(&["kind", "phase", "run"])
    );
    assert_eq!(
        keys_of("Change", Some("FieldDiff")),
        set(&["added", "from", "removed", "to"])
    );
}

#[test]
fn the_kind_link_owner_checklist_and_budget_keys_are_in_the_schema() {
    assert_eq!(
        keys_of("KindDecl", None),
        set(&["name", "ops", "roles", "statuses", "transitions"])
    );
    assert_eq!(
        keys_of("KindDecl", Some("OpDecl")),
        set(&["movesTo", "name"])
    );
    assert_eq!(
        keys_of("LinkTypeDecl", None),
        set(&["blocking", "dir", "name"])
    );
    assert_eq!(
        keys_of("PlanningFields", None),
        set(&["checklist", "kind", "links", "owners"])
    );
    assert_eq!(
        keys_of("PlanningFields", Some("Link")),
        set(&["blocking", "to", "type"])
    );
    assert_eq!(keys_of("RunBudget", None), set(&["usd"]));
}
