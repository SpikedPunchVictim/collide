//! Tests for `collide schema` (ts-0009): the first real subcommand of the `collide` binary.

use std::collections::{BTreeMap, BTreeSet};
use std::process::{Command, Output};

fn collide(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_collide"))
        .args(args)
        .output()
        .expect("collide should run")
}

fn schema_stdout() -> String {
    let output = collide(&["schema"]);
    assert!(
        output.status.success(),
        "collide schema exited with {}",
        output.status
    );
    String::from_utf8(output.stdout).expect("stdout should be UTF-8")
}

/// The lines of a section that starts with `name` and ends at a blank line, without the header.
fn section_lines(text: &str, name: &str) -> Vec<String> {
    let header = format!("{name}\n");
    let section = text
        .split("\n\n")
        .find(|s| s.starts_with(&header))
        .unwrap_or_else(|| panic!("a {name} section"));
    section
        .lines()
        .skip(1)
        .map(|line| line.trim().to_string())
        .collect()
}

/// The `points` section: each point's name, then its stage, as `  <name>  stage <id>`.
fn listed_points(text: &str) -> Vec<String> {
    section_lines(text, "points")
        .iter()
        .map(|line| line.split_whitespace().next().unwrap().to_string())
        .collect()
}

/// The `events` section, in the same form as the points.
fn listed_events(text: &str) -> Vec<String> {
    section_lines(text, "events")
        .iter()
        .map(|line| line.split_whitespace().next().unwrap().to_string())
        .collect()
}

/// The `types` section: each type at two spaces, its keys at four.
fn listed_types(text: &str) -> BTreeMap<String, BTreeSet<String>> {
    let section = text
        .split("\n\n")
        .find(|s| s.starts_with("types\n"))
        .expect("a types section");
    let mut types: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut current = String::new();
    for line in section.lines().skip(1) {
        if let Some(key) = line.strip_prefix("    ") {
            types.get_mut(&current).unwrap().insert(key.to_string());
        } else {
            current = line.trim().to_string();
            types.insert(current.clone(), BTreeSet::new());
        }
    }
    types
}

fn keys(types: &BTreeMap<String, BTreeSet<String>>, name: &str) -> Vec<String> {
    types
        .get(name)
        .unwrap_or_else(|| panic!("no type {name}"))
        .iter()
        .cloned()
        .collect()
}

#[test]
fn schema_lists_the_five_new_task_points() {
    let points = listed_points(&schema_stdout());
    for point in [
        "task.after_status",
        "task.before_op",
        "task.after_op",
        "task.before_move",
        "task.after_move",
    ] {
        assert!(points.contains(&point.to_string()), "{point} in {points:?}");
    }
}

#[test]
fn schema_lists_the_points_in_catalogue_order() {
    assert_eq!(
        listed_points(&schema_stdout()),
        [
            "run.before_start",
            "run.after_end",
            "run.before_integrate",
            "run.after_integrate",
            "workspace.after_create",
            "workspace.before_remove",
            "phase.before_start",
            "phase.after_end",
            "task.before_status",
            "task.after_status",
            "task.before_op",
            "task.after_op",
            "task.before_move",
            "task.after_move",
            "experiment.before_arm",
            "experiment.after_arm"
        ]
    );
}

#[test]
fn schema_lists_each_point_and_event_with_its_stage() {
    let text = schema_stdout();
    for line in [
        "  run.before_start  stage 7",
        "  run.before_integrate  stage 8",
        "  task.before_move  stage 12",
        "  experiment.after_arm  stage 15",
        "  permission.requested  stage 3b",
        "  task.changed  stage 4b",
        "  step.outcome  stage 8",
    ] {
        assert!(text.lines().any(|l| l == line), "{line} in\n{text}");
    }
}

#[test]
fn schema_lists_the_events_in_catalogue_order() {
    assert_eq!(
        listed_events(&schema_stdout()),
        [
            "run.transition",
            "phase.transition",
            "gate.verdict",
            "permission.requested",
            "permission.answered",
            "ask_human.asked",
            "ask_human.answered",
            "budget.exceeded",
            "task.changed",
            "task.renamed",
            "task.removed",
            "task.invalid",
            "task.written",
            "workspace.transition",
            "metric.reported",
            "experiment.transition",
            "step.intent",
            "step.outcome"
        ]
    );
}

#[test]
fn schema_lists_the_change_keys_and_latest_run() {
    let types = listed_types(&schema_stdout());
    assert_eq!(
        keys(&types, "Change"),
        [
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
            "via"
        ]
    );
    assert_eq!(keys(&types, "TaskPointPayload"), ["change", "latestRun"]);
    assert_eq!(keys(&types, "Actor"), ["kind", "phase", "run"]);
}

#[test]
fn schema_lists_the_pack_keys_points_and_checklists() {
    let types = listed_types(&schema_stdout());
    assert_eq!(
        keys(&types, "PackManifest"),
        [
            "apiVersion",
            "bindings",
            "checklists",
            "config",
            "experiment",
            "gates",
            "integrations",
            "launch",
            "metrics",
            "name",
            "observers",
            "points",
            "profiles",
            "prompts",
            "protectedPaths",
            "providers",
            "record",
            "requires",
            "roles",
            "syncs",
            "version",
            "views",
            "workflows",
            "workspaces"
        ]
    );
    assert_eq!(
        keys(&types, "Requires"),
        ["agents", "modelName", "sdk", "tools"]
    );
    assert_eq!(
        keys(&types, "HookEntry"),
        ["grace", "outputs", "script", "timeout"]
    );
    assert_eq!(
        keys(&types, "Binding"),
        ["gate", "match", "point", "script", "timeout", "when"]
    );
    assert_eq!(keys(&types, "ObserverEntry"), ["events", "script"]);
    assert_eq!(keys(&types, "ToolRequirement"), ["name", "version"]);
    assert_eq!(keys(&types, "ProviderEntry"), ["script"]);
    assert_eq!(keys(&types, "DerivedPoint"), ["match", "on", "when"]);
}

#[test]
fn schema_lists_the_board_config_agents_ask_and_pointer_keys() {
    let types = listed_types(&schema_stdout());
    assert_eq!(
        keys(&types, "Board"),
        [
            "checkout_filter",
            "packs",
            "profiles",
            "sources",
            "status_mapping",
            "workflow"
        ]
    );
    assert_eq!(keys(&types, "BoardProfiles"), ["default", "rules"]);
    assert_eq!(keys(&types, "ProfileRule"), ["profile", "when"]);
    assert_eq!(keys(&types, "GithubSource"), ["projects", "repos"]);
    assert_eq!(keys(&types, "GithubProject"), ["number", "owner"]);
    assert_eq!(
        keys(&types, "GithubStatusMapping"),
        ["categories", "status_field"]
    );
    assert_eq!(keys(&types, "CollidePointer"), ["dir"]);
    assert_eq!(keys(&types, "ConfigFile"), ["agents", "ask", "config"]);
    assert_eq!(
        keys(&types, "AgentEntry"),
        [
            "args",
            "budget_usd",
            "command",
            "config",
            "env",
            "kind",
            "model",
            "read",
            "secrets",
            "timeout_s",
            "tools"
        ]
    );
    assert_eq!(keys(&types, "Ask"), ["agent", "budget_usd", "timeout_s"]);
}

#[test]
fn schema_lists_kinds_op_effects_links_owners_checklist_and_budget() {
    let types = listed_types(&schema_stdout());
    assert_eq!(
        keys(&types, "KindDecl"),
        ["name", "ops", "roles", "statuses", "transitions"]
    );
    assert_eq!(
        keys(&types, "OpDecl"),
        ["args", "checkouts", "movesTo", "name"]
    );
    assert_eq!(
        keys(&types, "LinkTypeDecl"),
        ["blocking", "dir", "name", "target"]
    );
    assert_eq!(
        keys(&types, "PlanningFields"),
        ["checklist", "kind", "links", "owners"]
    );
    assert_eq!(keys(&types, "RunBudget"), ["usd"]);
}

#[test]
fn schema_lists_the_provider_and_observer_protocol_types() {
    let types = listed_types(&schema_stdout());
    assert_eq!(
        keys(&types, "InitializeResult"),
        [
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
        ]
    );
    assert_eq!(
        keys(&types, "ApplyResult"),
        [
            "current", "files", "items", "message", "outcome", "task", "version"
        ]
    );
    assert_eq!(
        keys(&types, "PreviewResult"),
        ["current", "files", "items", "message", "outcome"]
    );
    assert_eq!(keys(&types, "TaskInvalid"), ["checkout", "path", "reason"]);
    assert_eq!(keys(&types, "TaskRemoved"), ["cause", "checkout", "key"]);
    assert_eq!(
        keys(&types, "Event"),
        ["actor", "body", "experiment", "seq", "wall_ms"]
    );
    assert_eq!(keys(&types, "ObserverAck"), ["seq"]);
}

#[test]
fn schema_lists_the_hook_input_result_and_report_line() {
    let types = listed_types(&schema_stdout());
    assert_eq!(
        keys(&types, "HookInput"),
        [
            "board",
            "change",
            "config",
            "experiment",
            "history",
            "hook",
            "latestRun",
            "launch",
            "line",
            "pack",
            "phase",
            "protocol",
            "run",
            "state",
            "tasks",
            "workflow",
            "workspace"
        ]
    );
    assert_eq!(
        keys(&types, "HookResult"),
        ["message", "outputs", "retryAfter", "status"]
    );
    assert_eq!(
        keys(&types, "HookTask"),
        [
            "fields",
            "key",
            "kind",
            "links",
            "path",
            "shortId",
            "state",
            "templateFields"
        ]
    );
    assert_eq!(
        keys(&types, "HookLink"),
        ["category", "dir", "key", "kind", "stale", "status", "type"]
    );
    // `ReportLine` is one object per kind, so the listing shows the keys of all six together.
    assert_eq!(
        keys(&types, "ReportLine"),
        [
            "attrs",
            "cacheRead",
            "cacheWrite",
            "cacheWrite1h",
            "cacheWrite5m",
            "done",
            "id",
            "kind",
            "model",
            "name",
            "parent",
            "resetsAt",
            "status",
            "t",
            "tokensIn",
            "tokensOut",
            "total",
            "unit",
            "v",
            "value"
        ]
    );
}

#[test]
fn schema_output_is_the_same_on_every_run() {
    assert_eq!(schema_stdout(), schema_stdout());
}

#[test]
fn schema_lists_types_in_sorted_order() {
    let text = schema_stdout();
    let names: Vec<String> = listed_types(&text).into_keys().collect();
    let in_text: Vec<&str> = text
        .lines()
        .filter(|line| line.starts_with("  ") && !line.starts_with("    "))
        .skip(listed_points(&text).len() + listed_events(&text).len())
        .map(str::trim)
        .collect();
    assert_eq!(in_text, names);
}

#[test]
fn an_unknown_subcommand_exits_non_zero_with_a_usage_line() {
    let output = collide(&["frobnicate"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert_eq!(stderr.lines().count(), 1, "{stderr}");
    assert!(stderr.starts_with("usage: collide"), "{stderr}");
}

#[test]
fn schema_with_an_argument_it_does_not_take_is_a_usage_error() {
    for extra in ["--path", "Change"] {
        let output = collide(&["schema", extra]);
        assert_eq!(output.status.code(), Some(2), "schema {extra}");
        assert!(output.stdout.is_empty(), "schema {extra}");
    }
}
