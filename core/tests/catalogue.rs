//! Tests for the point and event catalogues (ts-0018): the points of 0.4's table with their objects and stages, the
//! events with their stages, the intent and outcome events, and the generated schema of the catalogue.

use collide_core::change::{Actor, ActorKind};
use collide_core::events::{
    BudgetScope, Event, EventBody, EventKind, PermissionAnswer, StepKind, StepResult,
};
use collide_core::points::{self, DerivedPoint, Object, Point};
use collide_core::schema;
use collide_core::stage::Stage;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

/// One observer line or event-log line around `body`, from a workflow actor in run `r-77`, phase `build`.
fn line(seq: u64, body: Value) -> Value {
    json!({
        "seq": seq,
        "wall_ms": 1_700_000_000_000u64,
        "actor": { "kind": "workflow", "run": "r-77", "phase": "build" },
        "experiment": null,
        "body": body
    })
}

fn change() -> Value {
    json!({
        "op": "setStatus",
        "args": { "status": "active" },
        "from": { "status": "ready", "category": "ready" },
        "to": { "status": "active", "category": "active" },
        "diff": { "status": { "from": "ready", "to": "active" } },
        "categoryMoved": true,
        "via": "ui",
        "actor": { "kind": "person" },
        "batch": null,
        "effects": [],
        "effectOf": null,
        "renamed": [],
        "checkout": "main",
        "files": ["ts-0018-dd.md"]
    })
}

/// One body of each event, in the catalogue's order.
fn samples() -> Vec<(EventKind, Value)> {
    vec![
        (
            EventKind::RunTransition,
            json!({ "event": "run.transition", "run": "r-77", "from": null, "to": { "state": "running" } }),
        ),
        (
            EventKind::PhaseTransition,
            json!({
                "event": "phase.transition", "run": "r-77", "phase": "build", "round": 1, "from": { "state": "running" },
                "to": { "state": "ended" }, "log": "logs/build.log", "transcript": "transcripts/build.jsonl"
            }),
        ),
        (
            EventKind::GateVerdict,
            json!({
                "event": "gate.verdict", "pack": "collide", "gate": "lint", "status": "fail", "run": "r-77",
                "phase": null, "point": "task.before_status", "message": "two items open"
            }),
        ),
        (
            EventKind::PermissionRequested,
            json!({
                "event": "permission.requested", "run": "r-77", "phase": "build",
                "request": "p-1", "tool": "Bash"
            }),
        ),
        (
            EventKind::PermissionAnswered,
            json!({
                "event": "permission.answered", "run": "r-77", "phase": "build",
                "request": "p-1", "answer": "allow"
            }),
        ),
        (
            EventKind::AskHumanAsked,
            json!({ "event": "ask_human.asked", "run": "r-77", "cause": "budget", "queue_item": "q-4" }),
        ),
        (
            EventKind::AskHumanAnswered,
            json!({ "event": "ask_human.answered", "run": "r-77", "answer": "continue", "queue_item": "q-4" }),
        ),
        (
            EventKind::BudgetExceeded,
            json!({ "event": "budget.exceeded", "run": "r-77", "scope": "run" }),
        ),
        (
            EventKind::TaskChanged,
            json!({
                "event": "task.changed", "source": "plan",
                "task": {
                    "key": "ts-0018-dd", "version": "v2",
                    "from": { "status": "ready", "category": "ready" },
                    "to": { "status": "active", "category": "active" },
                    "diff": {}, "cause": "worktree"
                }
            }),
        ),
        (
            EventKind::TaskRenamed,
            json!({
                "event": "task.renamed", "source": "plan",
                "rename": { "from": "ts-0012-ua", "to": "ts-0013-ua", "checkout": "main" }
            }),
        ),
        (
            EventKind::TaskRemoved,
            json!({
                "event": "task.removed", "source": "github",
                "task": { "key": "issue-12", "cause": "deleted" }
            }),
        ),
        (
            EventKind::TaskInvalid,
            json!({
                "event": "task.invalid", "source": "plan",
                "invalid": { "path": "epics/x/tasks/ts-1.md", "checkout": "main", "reason": "conflict markers" }
            }),
        ),
        (
            EventKind::TaskWritten,
            json!({ "event": "task.written", "source": "plan", "key": "ts-0018-dd", "change": change() }),
        ),
        (
            EventKind::WorkspaceTransition,
            json!({ "event": "workspace.transition", "workspace": "w-1", "from": "ready", "to": "in_use" }),
        ),
        (
            EventKind::MetricReported,
            json!({
                "event": "metric.reported", "run": "r-77",
                "line": { "kind": "metric", "v": 1, "t": "2026-11-14T10:00:00Z", "name": "tokens", "value": 12.0 }
            }),
        ),
        (
            EventKind::ExperimentTransition,
            json!({ "event": "experiment.transition", "id": "e-3", "from": "running", "to": "done" }),
        ),
        (
            EventKind::StepIntent,
            json!({
                "event": "step.intent", "step": "s-4", "kind": "integrate",
                "action": "merge the run branch into main"
            }),
        ),
        (
            EventKind::StepOutcome,
            json!({ "event": "step.outcome", "step": "s-4", "kind": "integrate", "result": "succeeded", "detail": "" }),
        ),
    ]
}

/// The points of 0.4's table, in order, with their object and stage.
const POINT_TABLE: [(&str, Object, &str); 16] = [
    ("run.before_start", Object::Run, "7"),
    ("run.after_end", Object::Run, "7"),
    ("run.before_integrate", Object::Run, "8"),
    ("run.after_integrate", Object::Run, "8"),
    ("workspace.after_create", Object::Workspace, "7"),
    ("workspace.before_remove", Object::Workspace, "7"),
    ("phase.before_start", Object::Phase, "8"),
    ("phase.after_end", Object::Phase, "8"),
    ("task.before_status", Object::Task, "8"),
    ("task.after_status", Object::Task, "8"),
    ("task.before_op", Object::Task, "8"),
    ("task.after_op", Object::Task, "8"),
    ("task.before_move", Object::Task, "12"),
    ("task.after_move", Object::Task, "12"),
    ("experiment.before_arm", Object::Experiment, "15"),
    ("experiment.after_arm", Object::Experiment, "15"),
];

/// Every event, with the stage that makes it fire.
const EVENT_TABLE: [(&str, &str); 18] = [
    ("run.transition", "3a"),
    ("phase.transition", "3a"),
    ("gate.verdict", "8"),
    ("permission.requested", "3b"),
    ("permission.answered", "3b"),
    ("ask_human.asked", "8"),
    ("ask_human.answered", "8"),
    ("budget.exceeded", "8"),
    ("task.changed", "4b"),
    ("task.renamed", "4b"),
    ("task.removed", "4b"),
    ("task.invalid", "4b"),
    ("task.written", "8"),
    ("workspace.transition", "3a"),
    ("metric.reported", "7"),
    ("experiment.transition", "15"),
    ("step.intent", "8"),
    ("step.outcome", "8"),
];

fn plan_stage_ids() -> BTreeSet<String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../IMPLEMENTATION_PLAN.md");
    let plan =
        fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    plan.lines()
        .filter_map(|line| {
            let rest = line
                .strip_prefix("## Stage ")
                .or_else(|| line.strip_prefix("### Stage "))?;
            rest.split(':').next().map(str::to_string)
        })
        .collect()
}

/// The values an enum schema allows: its `enum` list, or the `const` of each `oneOf` branch.
fn enum_values(name: &str) -> BTreeSet<String> {
    let text = &schema::registry().render()[&format!("{name}.input.json")];
    let node: Value = serde_json::from_str(text).unwrap();
    if let Some(values) = node["enum"].as_array() {
        return values
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();
    }
    node["oneOf"]
        .as_array()
        .unwrap_or_else(|| panic!("{name} is not an enum: {node}"))
        .iter()
        .map(|branch| branch["const"].as_str().unwrap().to_string())
        .collect()
}

// Points.

#[test]
fn the_catalogue_holds_the_sixteen_points_of_the_table_in_order() {
    let names: Vec<&str> = Point::ALL.iter().map(|point| point.name()).collect();
    let expected: Vec<&str> = POINT_TABLE.iter().map(|(name, _, _)| *name).collect();
    assert_eq!(names, expected);
    assert_eq!(names.len(), 16);
}

#[test]
fn each_point_has_the_object_and_the_stage_the_plan_gives_it() {
    for point in Point::ALL {
        let row = POINT_TABLE
            .iter()
            .find(|(name, _, _)| *name == point.name())
            .unwrap_or_else(|| panic!("{} is not in the table", point.name()));
        assert_eq!(point.object(), row.1, "object of {}", point.name());
        assert_eq!(point.stage().id(), row.2, "stage of {}", point.name());
    }
}

#[test]
fn a_point_serializes_as_its_dotted_name() {
    for point in Point::ALL {
        assert_eq!(serde_json::to_value(point).unwrap(), json!(point.name()));
        assert_eq!(
            serde_json::from_value::<Point>(json!(point.name())).unwrap(),
            point
        );
    }
}

#[test]
fn a_binding_to_a_point_outside_the_catalogue_is_refused_and_the_error_names_it() {
    let error = Point::parse("task.before_nothing").unwrap_err();
    assert!(error.to_string().contains("task.before_nothing"), "{error}");
    assert!(serde_json::from_value::<Point>(json!("task.before_nothing")).is_err());
}

#[test]
fn a_derived_point_on_a_point_outside_the_catalogue_is_refused_and_names_it() {
    let mut points = BTreeMap::new();
    points.insert(
        "pack.before_x".to_string(),
        DerivedPoint {
            on: "run.before_nothing".to_string(),
            matches: None,
            when: None,
        },
    );
    let errors = points::check_derived_points(&points, &Point::ALL).unwrap_err();
    assert_eq!(errors.len(), 1);
    assert!(
        errors[0].to_string().contains("run.before_nothing"),
        "{}",
        errors[0]
    );
}

#[test]
fn every_name_in_the_catalogue_parses_to_its_point() {
    for point in Point::ALL {
        assert_eq!(Point::parse(point.name()), Ok(point));
    }
}

#[test]
fn the_point_schema_lists_exactly_the_sixteen_names() {
    let listed = enum_values("Point");
    let names: BTreeSet<String> = Point::ALL.iter().map(|p| p.name().to_string()).collect();
    assert_eq!(listed, names);
    assert_eq!(listed.len(), 16);
}

// Stages.

#[test]
fn the_stage_ids_are_the_plan_stages_in_order() {
    let ids: Vec<&str> = Stage::ALL.iter().map(|stage| stage.id()).collect();
    assert_eq!(
        ids,
        [
            "1", "2", "3a", "3b", "3c", "3d", "4a", "4b", "4c", "5", "6a", "6b", "6c", "7", "8",
            "9a", "9b", "9c", "9d", "10", "11", "12", "13", "14", "15"
        ]
    );
}

#[test]
fn a_stage_serializes_as_its_id() {
    for stage in Stage::ALL {
        assert_eq!(serde_json::to_value(stage).unwrap(), json!(stage.id()));
    }
}

#[test]
fn every_stage_named_by_a_point_or_an_event_is_a_heading_of_the_plan() {
    let headings = plan_stage_ids();
    for stage in Stage::ALL {
        assert!(
            headings.contains(stage.id()),
            "no `Stage {}:` heading in the plan",
            stage.id()
        );
    }
    for point in Point::ALL {
        assert!(headings.contains(point.stage().id()), "{}", point.name());
    }
    for kind in EventKind::ALL {
        assert!(headings.contains(kind.stage().id()), "{}", kind.name());
    }
}

// Events.

#[test]
fn the_catalogue_holds_the_eighteen_events_in_order_with_their_stages() {
    let names: Vec<&str> = EventKind::ALL.iter().map(|kind| kind.name()).collect();
    let expected: Vec<&str> = EVENT_TABLE.iter().map(|(name, _)| *name).collect();
    assert_eq!(names, expected);
    for kind in EventKind::ALL {
        let row = EVENT_TABLE
            .iter()
            .find(|(name, _)| *name == kind.name())
            .unwrap();
        assert_eq!(kind.stage().id(), row.1, "stage of {}", kind.name());
    }
}

#[test]
fn the_samples_cover_every_event_kind_once() {
    let kinds: Vec<EventKind> = samples().into_iter().map(|(kind, _)| kind).collect();
    assert_eq!(kinds, EventKind::ALL);
}

#[test]
fn every_event_variant_round_trips_through_json_with_its_sequence_number() {
    for (kind, body) in samples() {
        let doc = line(9, body);
        let parsed: Event = serde_json::from_value(doc.clone())
            .unwrap_or_else(|error| panic!("{}: {error}", kind.name()));
        assert_eq!(parsed.seq, 9, "{}", kind.name());
        assert_eq!(parsed.body.kind(), kind);
        assert_eq!(parsed.body.name(), kind.name());
        assert_eq!(parsed.body.stage(), kind.stage());
        assert_eq!(
            serde_json::to_value(&parsed).unwrap(),
            doc,
            "{}",
            kind.name()
        );
    }
}

#[test]
fn an_event_carries_the_experiment_it_ran_in_or_null() {
    let mut doc = line(
        4,
        json!({ "event": "run.transition", "run": "r-77", "from": { "state": "running" }, "to": { "state": "ended", "outcome": "done" } }),
    );
    let outside: Event = serde_json::from_value(doc.clone()).unwrap();
    assert!(outside.experiment.is_none());

    doc["experiment"] = json!({ "id": "e-3", "arm": "B", "dir": "/work/e-3/arm-b" });
    let inside: Event = serde_json::from_value(doc.clone()).unwrap();
    assert_eq!(inside.experiment.as_ref().unwrap().arm, "B");
    assert_eq!(serde_json::to_value(&inside).unwrap(), doc);
}

#[test]
fn an_event_with_an_unknown_key_is_refused() {
    for (kind, body) in samples() {
        let mut doc = line(1, body);
        doc["body"]["extra"] = json!(1);
        assert!(
            serde_json::from_value::<Event>(doc).is_err(),
            "{} with extra key",
            kind.name()
        );
    }
}

#[test]
fn an_event_with_an_unknown_tag_is_refused() {
    let doc = line(1, json!({ "event": "run.started", "run": "r-77" }));
    assert!(serde_json::from_value::<Event>(doc).is_err());
}

#[test]
fn an_event_body_with_a_missing_key_is_refused() {
    let doc = line(
        1,
        json!({ "event": "run.transition", "run": "r-77", "from": null }),
    );
    assert!(serde_json::from_value::<Event>(doc).is_err());
}

#[test]
fn the_event_schema_is_registered() {
    let names: BTreeSet<String> = schema::registry().render().into_keys().collect();
    assert!(names.contains("Event.input.json"));
    assert!(names.contains("Event.output.json"));
    assert!(names.contains("Point.input.json"));
    assert!(names.contains("Stage.output.json"));
}

// Intent and outcome.

fn step_kind(kind: StepKind) -> &'static str {
    match kind {
        StepKind::Integrate => "integrate",
        StepKind::SourceOp => "source_op",
        StepKind::AfterIntegrateHook => "after_integrate_hook",
        StepKind::RemoteWrite => "remote_write",
        StepKind::Discard => "discard",
    }
}

#[test]
fn a_step_kind_is_one_of_the_side_effect_steps_of_the_plan_and_a_discard() {
    for kind in [
        StepKind::Integrate,
        StepKind::SourceOp,
        StepKind::AfterIntegrateHook,
        StepKind::RemoteWrite,
        StepKind::Discard,
    ] {
        assert_eq!(serde_json::to_value(kind).unwrap(), json!(step_kind(kind)));
    }
}

#[test]
fn an_intent_names_its_step_its_kind_and_what_it_will_do() {
    let doc = line(
        3,
        json!({ "event": "step.intent", "step": "s-4", "kind": "remote_write", "action": "set label done" }),
    );
    let parsed: Event = serde_json::from_value(doc).unwrap();
    match parsed.body {
        EventBody::StepIntent { step, kind, action } => {
            assert_eq!(step, "s-4");
            assert_eq!(kind, StepKind::RemoteWrite);
            assert_eq!(action, "set label done");
        }
        other => panic!("expected step.intent, got {other:?}"),
    }
}

#[test]
fn an_outcome_names_the_same_step_and_kind_and_its_result() {
    let doc = line(
        4,
        json!({ "event": "step.outcome", "step": "s-4", "kind": "integrate", "result": "failed", "detail": "PR closed unmerged" }),
    );
    let parsed: Event = serde_json::from_value(doc).unwrap();
    match parsed.body {
        EventBody::StepOutcome {
            step, kind, result, ..
        } => {
            assert_eq!(step, "s-4");
            assert_eq!(kind, StepKind::Integrate);
            assert_eq!(result, StepResult::Failed);
        }
        other => panic!("expected step.outcome, got {other:?}"),
    }
}

#[test]
fn an_intent_and_an_outcome_both_carry_the_actor() {
    for (name, body) in [
        (
            "step.intent",
            json!({ "event": "step.intent", "step": "s-4", "kind": "integrate", "action": "merge" }),
        ),
        (
            "step.outcome",
            json!({ "event": "step.outcome", "step": "s-4", "kind": "integrate", "result": "succeeded", "detail": "" }),
        ),
    ] {
        let doc = json!({
            "seq": 5,
            "wall_ms": 1_700_000_000_000u64,
            "actor": { "kind": "workflow", "run": "r-77" },
            "experiment": null,
            "body": body
        });
        let parsed: Event = serde_json::from_value(doc.clone()).unwrap();
        assert_eq!(
            parsed.actor,
            Actor {
                kind: ActorKind::Workflow,
                run: Some("r-77".to_string()),
                phase: None,
            },
            "{name}"
        );
        assert_eq!(serde_json::to_value(&parsed).unwrap(), doc, "{name}");
    }
}

// The typed fields: lowercase enums, the phase's round and optional paths, gate scope, queue items, step detail.

fn event_of(body: Value) -> Event {
    serde_json::from_value(line(2, body)).unwrap()
}

#[test]
fn a_permission_answer_is_lowercase_and_a_capitalized_one_is_refused() {
    for (text, answer) in [
        ("allow", PermissionAnswer::Allow),
        ("deny", PermissionAnswer::Deny),
        ("cancel", PermissionAnswer::Cancel),
    ] {
        assert_eq!(
            serde_json::from_value::<PermissionAnswer>(json!(text)).unwrap(),
            answer
        );
    }
    assert!(serde_json::from_value::<PermissionAnswer>(json!("Allow")).is_err());
    let doc = line(
        1,
        json!({ "event": "permission.answered", "run": "r-77", "phase": "build", "request": "p-1", "answer": "Allow" }),
    );
    assert!(serde_json::from_value::<Event>(doc).is_err());
}

#[test]
fn a_phase_entering_its_first_state_has_no_log_or_transcript_yet() {
    let doc = line(
        3,
        json!({
            "event": "phase.transition", "run": "r-77", "phase": "review", "round": 2, "from": null,
            "to": { "state": "running" }, "log": null, "transcript": null
        }),
    );
    let parsed: Event = serde_json::from_value(doc.clone()).unwrap();
    assert_eq!(serde_json::to_value(&parsed).unwrap(), doc);
    match parsed.body {
        EventBody::PhaseTransition {
            round,
            log,
            transcript,
            ..
        } => {
            assert_eq!(round, 2);
            assert!(log.is_none() && transcript.is_none());
        }
        other => panic!("expected phase.transition, got {other:?}"),
    }
}

#[test]
fn a_phase_round_is_a_number() {
    let doc = line(
        3,
        json!({
            "event": "phase.transition", "run": "r-77", "phase": "review", "round": "2", "from": null,
            "to": { "state": "running" }, "log": null, "transcript": null
        }),
    );
    assert!(serde_json::from_value::<Event>(doc).is_err());
}

#[test]
fn a_gate_verdict_names_its_pack_and_its_point_or_phase() {
    let on_point = json!({
        "event": "gate.verdict", "pack": "collide", "gate": "lint", "status": "fail", "run": null,
        "phase": null, "point": "task.before_status", "message": "two items open"
    });
    let parsed = event_of(on_point.clone());
    match &parsed.body {
        EventBody::GateVerdict { point, phase, .. } => {
            assert_eq!(*point, Some(Point::TaskBeforeStatus));
            assert!(phase.is_none());
        }
        other => panic!("expected gate.verdict, got {other:?}"),
    }
    assert_eq!(serde_json::to_value(&parsed).unwrap()["body"], on_point);

    let on_phase = json!({
        "event": "gate.verdict", "pack": "demo", "gate": "review", "status": "ok", "run": "r-77",
        "phase": "review", "point": null, "message": ""
    });
    assert_eq!(
        serde_json::to_value(event_of(on_phase.clone())).unwrap()["body"],
        on_phase
    );
}

#[test]
fn a_gate_verdict_without_a_message_reads_as_an_empty_one() {
    let parsed = event_of(json!({
        "event": "gate.verdict", "pack": "collide", "gate": "lint", "status": "ok", "run": "r-77",
        "phase": "build", "point": null
    }));
    match parsed.body {
        EventBody::GateVerdict { message, .. } => assert_eq!(message, ""),
        other => panic!("expected gate.verdict, got {other:?}"),
    }
}

#[test]
fn a_gate_point_must_be_in_the_catalogue() {
    let doc = line(
        1,
        json!({
            "event": "gate.verdict", "pack": "collide", "gate": "lint", "status": "ok", "run": null,
            "phase": null, "point": "task.before_nothing", "message": ""
        }),
    );
    assert!(serde_json::from_value::<Event>(doc).is_err());
}

#[test]
fn ask_human_events_carry_the_queue_item_the_run_waits_on() {
    for body in [
        json!({ "event": "ask_human.asked", "run": "r-77", "cause": "budget", "queue_item": "q-4" }),
        json!({ "event": "ask_human.answered", "run": "r-77", "answer": "stop", "queue_item": "q-4" }),
    ] {
        let parsed = event_of(body.clone());
        assert_eq!(serde_json::to_value(&parsed).unwrap()["body"], body);
        let mut missing = body.clone();
        missing.as_object_mut().unwrap().remove("queue_item");
        assert!(serde_json::from_value::<Event>(line(1, missing)).is_err());
    }
}

#[test]
fn a_budget_scope_is_run_phase_or_role_in_lowercase() {
    for (text, scope) in [
        ("run", BudgetScope::Run),
        ("phase", BudgetScope::Phase),
        ("role", BudgetScope::Role),
    ] {
        assert_eq!(
            serde_json::from_value::<BudgetScope>(json!(text)).unwrap(),
            scope
        );
    }
    assert!(serde_json::from_value::<BudgetScope>(json!("Run")).is_err());
    assert!(serde_json::from_value::<BudgetScope>(json!("pack")).is_err());
    let doc = line(
        1,
        json!({ "event": "budget.exceeded", "run": "r-77", "scope": "Run" }),
    );
    assert!(serde_json::from_value::<Event>(doc).is_err());
}

#[test]
fn a_step_result_is_succeeded_or_failed_and_detail_may_be_empty() {
    assert_eq!(
        serde_json::to_value(StepResult::Succeeded).unwrap(),
        json!("succeeded")
    );
    assert_eq!(
        serde_json::to_value(StepResult::Failed).unwrap(),
        json!("failed")
    );
    assert!(serde_json::from_value::<StepResult>(json!("merged")).is_err());
    let parsed = event_of(json!({
        "event": "step.outcome", "step": "s-4", "kind": "source_op", "result": "succeeded"
    }));
    match parsed.body {
        EventBody::StepOutcome {
            result,
            detail,
            kind,
            ..
        } => {
            assert_eq!(result, StepResult::Succeeded);
            assert_eq!(detail, "");
            assert_eq!(kind, StepKind::SourceOp);
        }
        other => panic!("expected step.outcome, got {other:?}"),
    }
}
