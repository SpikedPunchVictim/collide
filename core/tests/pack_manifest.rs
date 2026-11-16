//! Tests for the `pack.yaml` contract (ts-0012): the five real manifests against the generated schema, an unknown
//! key at any depth, `minLength: 1`, a gate on an `after_*` point, paths, the manifest checks, and the invalid
//! fixtures in `fixtures/manifests/invalid/`, read through the hardened YAML loader.

mod schema_validate;

use collide_core::pack::{PackManifest, ProtectedAt, ProtectedPaths};
use collide_core::pack_check::{
    BindingProblem, ManifestError, PackPath, PathProblem, check_manifest, parse_pack_path,
};
use collide_core::schema;
use collide_core::workflow::Phase;
use collide_core::workflow_check::WorkflowError;
use collide_core::yaml::{load_yaml, load_yaml_file};
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn repo(path: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join(path)
}

fn schema_file(name: &str) -> Value {
    serde_json::from_str(&schema::registry().render()[name]).unwrap()
}

fn load(text: &str) -> PackManifest {
    load_yaml(text.as_bytes(), "test.yaml").unwrap_or_else(|error| panic!("{error}\n{text}"))
}

/// A manifest with the two required keys and `body` after them.
fn manifest(body: &str) -> String {
    format!("apiVersion: collide/v0\nname: demo\n{body}")
}

fn refusals(body: &str) -> Vec<ManifestError> {
    check_manifest(&load(&manifest(body))).expect_err(body)
}

fn refusal_text(body: &str) -> String {
    refusals(body)
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}

// The five manifests.

/// The two built-in packs and the three fixture packs (the reference pack's copy, and the two others), found by
/// listing the pack roots, as `host/tests/layout.rs` does.
fn five() -> Vec<String> {
    let mut found = vec![
        "packs/collide/pack.yaml".to_string(),
        "packs/plan/pack.yaml".to_string(),
    ];
    let mut fixtures: Vec<String> = std::fs::read_dir(repo("fixtures/packs"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    fixtures.sort();
    found.extend(
        fixtures
            .iter()
            .map(|dir| format!("fixtures/packs/{dir}/pack.yaml")),
    );
    assert_eq!(found.len(), 5, "{found:?}");
    found
}

/// The reference pack's fixture copy: the one fixture manifest that has a `record` key.
fn reference_manifest() -> PackManifest {
    five()
        .iter()
        .map(|path| load_yaml_file::<PackManifest>(&repo(path)).unwrap())
        .find(|manifest| manifest.record.is_some())
        .expect("one fixture pack is the reference pack")
}

#[test]
fn the_five_manifests_load_and_pass_the_manifest_check() {
    for path in five() {
        let path = path.as_str();
        let manifest: PackManifest =
            load_yaml_file(&repo(path)).unwrap_or_else(|error| panic!("{path}: {error}"));
        assert_eq!(check_manifest(&manifest), Ok(()), "{path}");
    }
}

#[test]
fn the_five_manifests_validate_against_the_generated_schema() {
    let input = schema_file("PackManifest.input.json");
    for path in five() {
        let path = path.as_str();
        let doc: Value =
            load_yaml_file(&repo(path)).unwrap_or_else(|error| panic!("{path}: {error}"));
        assert_eq!(
            schema_validate::violations(&input, &doc),
            Vec::<String>::new(),
            "{path}"
        );
    }
}

#[test]
fn the_five_manifests_round_trip_and_the_output_schema_accepts_what_is_written() {
    let output = schema_file("PackManifest.output.json");
    for path in five() {
        let path = path.as_str();
        let manifest: PackManifest = load_yaml_file(&repo(path)).unwrap();
        // Through JSON text, not `Value`: a `Value` map is sorted and would lose the phases' declaration order.
        let written = serde_json::to_string(&manifest).unwrap();
        let back: PackManifest = serde_json::from_str(&written).unwrap();
        assert_eq!(back, manifest, "{path}");
        assert_eq!(serde_json::to_string(&back).unwrap(), written, "{path}");
        let doc: Value = serde_json::from_str(&written).unwrap();
        assert_eq!(
            schema_validate::violations(&output, &doc),
            Vec::<String>::new(),
            "{path}"
        );
    }
}

#[test]
fn the_reference_manifest_holds_every_key_of_the_plans_example() {
    let manifest = reference_manifest();
    assert!(!manifest.name.is_empty());
    assert_eq!(manifest.version.as_deref(), Some("0.1.0"));
    let requires = manifest.requires.as_ref().unwrap();
    assert_eq!(requires.sdk.as_deref(), Some(">=0.1 <0.2"));
    assert_eq!(requires.agents, ["claude"]);
    assert_eq!(requires.model_name.as_deref(), Some("^[a-z]+$"));
    assert_eq!(requires.tools.len(), 3);
    assert!(manifest.launch.contains_key("review_budget"));
    let worktree = &manifest.workspaces["worktree"];
    assert_eq!(worktree.create.timeout.unwrap().as_secs(), 7200);
    assert_eq!(worktree.create.grace.unwrap().as_secs(), 60);
    assert!(worktree.create.outputs.contains_key("mapCommit"));
    assert!(worktree.cleanup.is_some() && worktree.discard.is_some());
    assert_eq!(
        manifest.integrations["merge-local"].script,
        "hooks/merge-local.ts"
    );
    assert_eq!(manifest.record.as_ref().unwrap().paths.len(), 2);
    assert_eq!(manifest.protected_paths.as_ref().unwrap().paths().len(), 3);
    assert!(manifest.metrics.as_ref().unwrap().import.is_some());
    let experiment = manifest.experiment.as_ref().unwrap();
    assert_eq!(
        experiment.branch_prefix.as_deref(),
        Some("scratch/collide-exp/")
    );
    assert_eq!(experiment.append_only.len(), 3);
    assert_eq!(manifest.roles.len(), 3);
    assert!(manifest.roles["reviewer"].read_only);
    assert_eq!(manifest.roles["builder"].budget.as_ref().unwrap().usd, 20.0);
    assert_eq!(manifest.prompts.len(), 3);
    assert_eq!(
        manifest.gates["iter-evidence"].script,
        "gates/iter-evidence.ts"
    );
    assert!(manifest.workflows.contains_key("loop"));
    assert_eq!(manifest.providers["plan"].script, "providers/plan.ts");
}

#[test]
fn the_other_fixtures_hold_a_binding_a_gate_an_observer_and_a_setting_of_each_flag() {
    let pr_ci: PackManifest = load_yaml_file(&repo("fixtures/packs/pr-ci/pack.yaml")).unwrap();
    assert_eq!(pr_ci.bindings.len(), 1);
    assert_eq!(pr_ci.bindings[0].gate.as_deref(), Some("ci-green"));
    assert_eq!(pr_ci.bindings[0].timeout.unwrap().as_secs(), 30);
    assert_eq!(pr_ci.observers["notify"].events, ["run.*", "ask_human.*"]);
    let review_loop: PackManifest = load_yaml_file(&repo("fixtures/packs/loop/pack.yaml")).unwrap();
    assert_eq!(
        review_loop.config["notifyHook"].0["secret"],
        Value::Bool(true)
    );
    assert_eq!(
        review_loop.config["lanePool"].0["overridable"],
        Value::Bool(true)
    );
}

#[test]
fn the_minimal_manifest_is_the_two_required_keys() {
    let minimal: PackManifest = load_yaml_file(&repo("fixtures/manifests/minimal.yaml")).unwrap();
    assert_eq!(check_manifest(&minimal), Ok(()));
    assert_eq!(
        serde_json::to_value(&minimal).unwrap(),
        serde_json::json!({
            "apiVersion": "collide/v0", "name": "demo"
        })
    );
}

// The invalid fixtures.

#[derive(Clone, Copy, PartialEq)]
enum Fails {
    /// The YAML loader or serde refuses the file.
    AtLoad,
    /// The file loads and the manifest check refuses it.
    AtCheck,
}

/// Every file of `fixtures/manifests/invalid/`, where it fails, and a word its message must hold. The test below
/// fails for a file that is not in this table.
const INVALID: &[(&str, Fails, &str)] = &[
    ("unknown-key-top.yaml", Fails::AtLoad, "flavor"),
    ("unknown-key-in-role.yaml", Fails::AtLoad, "thinks"),
    (
        "unknown-key-in-workspace-hook.yaml",
        Fails::AtLoad,
        "retries",
    ),
    ("unknown-key-in-phase.yaml", Fails::AtLoad, "speed"),
    ("unknown-key-in-experiment.yaml", Fails::AtLoad, "armPrefix"),
    ("missing-name.yaml", Fails::AtLoad, "name"),
    ("missing-api-version.yaml", Fails::AtLoad, "apiVersion"),
    ("empty-name.yaml", Fails::AtCheck, "`name` is empty"),
    (
        "empty-role-agent.yaml",
        Fails::AtCheck,
        "roles.builder.agent",
    ),
    ("gate-on-after-point.yaml", Fails::AtCheck, "run.after_end"),
    (
        "gate-on-derived-after-point.yaml",
        Fails::AtCheck,
        "proposal.after_share",
    ),
    (
        "binding-unknown-point.yaml",
        Fails::AtCheck,
        "task.before_statuz",
    ),
    (
        "binding-gate-and-script.yaml",
        Fails::AtCheck,
        "both a `gate:` and a `script:`",
    ),
    (
        "absolute-script-path.yaml",
        Fails::AtCheck,
        "/usr/local/bin/check",
    ),
    (
        "climbing-hook-path.yaml",
        Fails::AtCheck,
        "../outside/merge.ts",
    ),
    (
        "other-scheme-path.yaml",
        Fails::AtCheck,
        "starts with `dev:`",
    ),
    (
        "repo-path-climbs.yaml",
        Fails::AtCheck,
        "repo:../agents/builder.md",
    ),
    (
        "wait-path-absolute.yaml",
        Fails::AtCheck,
        "workflows.pr.phases.ci.wait",
    ),
    (
        "record-glob-absolute.yaml",
        Fails::AtCheck,
        "record.paths[0]",
    ),
    (
        "zero-hook-timeout.yaml",
        Fails::AtCheck,
        "gates.evidence.timeout",
    ),
    (
        "view-name-differs-from-key.yaml",
        Fails::AtCheck,
        "`mvp` has `name: MVP`",
    ),
    (
        "profile-reaches-other-pack.yaml",
        Fails::AtCheck,
        "reaches past this pack",
    ),
    ("workflow-dangling-then.yaml", Fails::AtCheck, "nowhere"),
    (
        "derived-point-on-unknown-core-point.yaml",
        Fails::AtCheck,
        "task.before_statuz",
    ),
    (
        "setting-secret-not-boolean.yaml",
        Fails::AtCheck,
        "config.notifyHook",
    ),
    ("compact-at-below-floor.yaml", Fails::AtCheck, "100000"),
    (
        "observer-without-events.yaml",
        Fails::AtCheck,
        "observers.notify.events",
    ),
    ("duplicate-key.yaml", Fails::AtLoad, "duplicate"),
    ("alias-bomb.yaml", Fails::AtLoad, "anchor"),
    ("merge-key.yaml", Fails::AtLoad, "merge"),
    ("unquoted-version.yaml", Fails::AtLoad, "version"),
    (
        "unquoted-template.yaml",
        Fails::AtLoad,
        "quote the template",
    ),
    ("deep-nesting.yaml", Fails::AtLoad, "deep"),
];

fn invalid_files() -> BTreeSet<String> {
    std::fs::read_dir(repo("fixtures/manifests/invalid"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect()
}

#[test]
fn every_invalid_fixture_is_in_the_table() {
    let listed: BTreeSet<String> = INVALID
        .iter()
        .map(|(file, _, _)| file.to_string())
        .collect();
    assert_eq!(invalid_files(), listed);
}

#[test]
fn every_invalid_fixture_fails_where_the_table_says_and_names_what_is_wrong() {
    for (file, fails, word) in INVALID {
        let path = repo(&format!("fixtures/manifests/invalid/{file}"));
        let text = match (load_yaml_file::<PackManifest>(&path), fails) {
            (Err(error), Fails::AtLoad) => error.to_string(),
            (Ok(manifest), Fails::AtCheck) => check_manifest(&manifest)
                .err()
                .unwrap_or_else(|| panic!("{file} passes the manifest check"))
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n"),
            (Err(error), Fails::AtCheck) => panic!("{file} should load: {error}"),
            (Ok(_), Fails::AtLoad) => panic!("{file} should not load"),
        };
        assert!(text.contains(word), "{file}: `{word}` is not in: {text}");
    }
}

#[test]
fn an_error_names_the_file_and_the_line_of_a_load_failure() {
    let path = repo("fixtures/manifests/invalid/unknown-key-in-role.yaml");
    let text = load_yaml_file::<PackManifest>(&path)
        .unwrap_err()
        .to_string();
    assert!(text.contains("unknown-key-in-role.yaml:6"), "{text}");
    assert!(text.contains("roles.builder"), "{text}");
}

#[test]
fn the_schema_rejects_what_serde_rejects_for_the_keys_and_values_that_load() {
    // Files that load as `Value` (so the loader does not stop them) but not as a manifest: the schema must too.
    let input = schema_file("PackManifest.input.json");
    for file in [
        "unknown-key-top.yaml",
        "unknown-key-in-role.yaml",
        "unknown-key-in-workspace-hook.yaml",
        "unknown-key-in-phase.yaml",
        "unknown-key-in-experiment.yaml",
        "missing-name.yaml",
        "missing-api-version.yaml",
        "empty-name.yaml",
        "empty-role-agent.yaml",
    ] {
        let doc: Value =
            load_yaml_file(&repo(&format!("fixtures/manifests/invalid/{file}"))).unwrap();
        assert!(
            !schema_validate::violations(&input, &doc).is_empty(),
            "{file}"
        );
    }
}

// Unknown keys at any depth.

#[test]
fn an_unknown_key_fails_at_every_depth_of_the_manifest() {
    // Each is a valid fragment plus one `extra:` key, placed at one depth.
    let cases = [
        "requires: { extra: 1 }\n",
        "requires: { tools: [{ name: node, extra: 1 }] }\n",
        "workspaces: { w: { create: { script: a.ts, extra: 1 } } }\n",
        "workspaces: { w: { create: { script: a.ts, outputs: { x: { type: string } } }, extra: 1 } }\n",
        "integrations: { i: { script: a.ts, extra: 1 } }\n",
        "syncs: { s: { script: a.ts, extra: 1 } }\n",
        "record: { paths: [a], script: a.ts, extra: 1 }\n",
        "protectedPaths: { paths: [a], at: phase-end, extra: 1 }\n",
        "metrics: { extra: 1 }\n",
        "metrics: { import: { script: a.ts, extra: 1 } }\n",
        "experiment: { extra: 1 }\n",
        "experiment: { prepare: { script: a.ts, extra: 1 } }\n",
        "roles: { r: { agent: claude, budget: { usd: 1, extra: 1 } } }\n",
        "roles: { r: { agent: claude, extra: 1 } }\n",
        "gates: { g: { script: a.ts, extra: 1 } }\n",
        "bindings: [{ point: task.before_status, gate: g, extra: 1 }]\n",
        "observers: { o: { script: a.ts, events: ['*'], extra: 1 } }\n",
        "providers: { p: { script: a.ts, extra: 1 } }\n",
        "points: { a.before_b: { on: task.before_status, extra: 1 } }\n",
        "checklists: { c: { version: 1, gates: done, items: [], extra: 1 } }\n",
        "checklists: { c: { version: 1, gates: done, items: [{ id: a, text: b, extra: 1 }] } }\n",
        "workflows: { w: { extra: 1, phases: { a: { script: a.ts } } } }\n",
        "workflows: { w: { phases: { a: { script: a.ts, extra: 1 } } } }\n",
        "workflows: { w: { phases: { a: { role: r, prompt: p, then: { script: a.ts, on: { go: { end: x, extra: 1 } } } } } } }\n",
        "views: { v: { name: v, extra: 1 } }\n",
        "views: { v: { name: v, layout: { extra: 1 } } }\n",
        "profiles: { p: { extra: 1 } }\n",
        "profiles: { p: { roles: { demo/r: { extra: 1 } } } }\n",
    ];
    let input = schema_file("PackManifest.input.json");
    for body in cases {
        let text = manifest(body);
        let error = load_yaml::<PackManifest>(text.as_bytes(), "test.yaml")
            .err()
            .unwrap_or_else(|| panic!("`extra` was accepted in:\n{body}"));
        assert!(error.to_string().contains("extra"), "{body}: {error}");
        let doc: Value = load_yaml(text.as_bytes(), "test.yaml").unwrap();
        assert!(
            schema_validate::violations(&input, &doc)
                .iter()
                .any(|violation| violation.contains("extra") || violation.contains("matches 0")),
            "the schema accepted `extra` in:\n{body}"
        );
    }
}

#[test]
fn a_setting_is_a_json_schema_and_its_own_keywords_are_not_closed() {
    let loaded = load(&manifest(
        "config:\n  lanePool: { type: integer, minimum: 1, default: 2, overridable: true, x-note: fine }\n",
    ));
    assert_eq!(check_manifest(&loaded), Ok(()));
}

// minLength.

#[test]
fn a_required_string_has_min_length_one_in_the_generated_schema() {
    let input = schema_file("PackManifest.input.json");
    let output = schema_file("PackManifest.output.json");
    for (side, schema) in [("input", &input), ("output", &output)] {
        assert_eq!(schema["properties"]["name"]["minLength"], 1, "{side} name");
        assert_eq!(
            schema["properties"]["apiVersion"]["minLength"], 1,
            "{side} apiVersion"
        );
        for (def, property) in [
            ("ToolRequirement", "name"),
            ("ProviderEntry", "script"),
            ("HookEntry", "script"),
            ("WorkspaceImpl", "create"),
            ("Record", "script"),
            ("Role", "agent"),
            ("Binding", "point"),
            ("ObserverEntry", "script"),
            ("ChecklistTemplateItem", "id"),
            ("ChecklistTemplateItem", "text"),
            ("DerivedPoint", "on"),
            ("View", "name"),
            ("AgentPhase", "role"),
            ("ScriptPhase", "script"),
            ("WaitPhase", "wait"),
            ("HumanPhase", "human"),
            ("Decision", "script"),
            ("EndTarget", "end"),
            ("ArtifactPrompt", "artifact"),
            ("WorkflowOp", "op"),
        ] {
            let node = &schema["$defs"][def]["properties"][property];
            if property == "create" {
                assert_eq!(node["$ref"], "#/$defs/HookEntry", "{side} {def}.{property}");
            } else {
                assert_eq!(node["minLength"], 1, "{side} {def}.{property}");
            }
        }
    }
}

#[test]
fn the_schema_and_the_check_both_refuse_an_empty_name() {
    let input = schema_file("PackManifest.input.json");
    let doc: Value = load_yaml(b"apiVersion: collide/v0\nname: ''\n", "test.yaml").unwrap();
    assert!(
        schema_validate::violations(&input, &doc)
            .iter()
            .any(|v| v.contains("shorter than 1")),
    );
    let empty = load("apiVersion: collide/v0\nname: ''\n");
    assert_eq!(
        check_manifest(&empty).unwrap_err(),
        [ManifestError::Empty { at: "name".into() }]
    );
}

#[test]
fn white_space_alone_is_empty_to_the_check() {
    let loaded = load("apiVersion: '  '\nname: demo\n");
    assert_eq!(
        check_manifest(&loaded).unwrap_err(),
        [ManifestError::Empty {
            at: "apiVersion".into()
        }]
    );
}

// Gates and bindings.

#[test]
fn a_gate_bound_to_an_after_point_is_refused_naming_the_point() {
    for point in [
        "run.after_end",
        "run.after_integrate",
        "workspace.after_create",
        "phase.after_end",
        "task.after_status",
        "task.after_op",
        "task.after_move",
        "experiment.after_arm",
    ] {
        let text = refusal_text(&format!(
            "gates: {{ g: {{ script: g.ts }} }}\nbindings: [{{ point: {point}, gate: g }}]\n"
        ));
        assert!(text.contains(&format!("`{point}`")), "{text}");
        assert!(text.contains("an `after_*` point"), "{text}");
    }
}

#[test]
fn a_gate_on_a_before_point_and_a_script_on_any_point_are_accepted() {
    let loaded = load(&manifest(
        "gates: { g: { script: g.ts } }\n\
         points: { decision.before_decide: { on: task.before_status, match: [done] } }\n\
         bindings:\n\
         \x20 - { point: task.before_status, match: [review], gate: g, timeout: 30s }\n\
         \x20 - { point: run.before_integrate, gate: g }\n\
         \x20 - { point: decision.before_decide, gate: g, when: 'task.kind == \"decision\"' }\n\
         \x20 - { point: other/decision.before_decide, gate: g }\n\
         \x20 - { point: run.after_end, script: hooks/after.ts }\n\
         \x20 - { point: task.after_status, script: hooks/after.ts }\n\
         \x20 - { point: other/spike.after_conclude, script: hooks/after.ts }\n",
    ));
    assert_eq!(check_manifest(&loaded), Ok(()));
}

#[test]
fn a_gate_on_an_after_point_of_a_derived_point_or_another_pack_is_refused() {
    for (points, point) in [
        (
            "points: { proposal.after_share: { on: task.after_status } }\n",
            "proposal.after_share",
        ),
        ("", "other/decision.after_decide"),
    ] {
        let errors = refusals(&format!(
            "{points}gates: {{ g: {{ script: g.ts }} }}\nbindings: [{{ point: '{point}', gate: g }}]\n"
        ));
        assert_eq!(
            errors,
            [ManifestError::Binding {
                index: 0,
                problem: BindingProblem::GateOnAfterPoint {
                    point: point.to_string(),
                    gate: "g".to_string()
                }
            }]
        );
    }
}

#[test]
fn a_binding_names_one_target_and_a_known_point() {
    let cases = [
        ("{ point: task.before_status }", BindingProblem::NoTarget),
        (
            "{ point: task.before_status, gate: g, script: a.ts }",
            BindingProblem::BothTargets,
        ),
        (
            "{ point: task.before_statuz, script: a.ts }",
            BindingProblem::UnknownPoint {
                point: "task.before_statuz".into(),
            },
        ),
        (
            "{ point: nodot, script: a.ts }",
            BindingProblem::UnknownPoint {
                point: "nodot".into(),
            },
        ),
        (
            "{ point: other/nosides, script: a.ts }",
            BindingProblem::UnknownPoint {
                point: "other/nosides".into(),
            },
        ),
        (
            "{ point: task.before_status, script: a.ts, match: [] }",
            BindingProblem::EmptyMatch,
        ),
    ];
    for (binding, problem) in cases {
        assert_eq!(
            refusals(&format!("bindings: [{binding}]\n")),
            [ManifestError::Binding { index: 0, problem }],
            "{binding}"
        );
    }
}

#[test]
fn a_binding_when_is_checked_for_length() {
    let long = "x".repeat(1025);
    let text = refusal_text(&format!(
        "bindings: [{{ point: task.before_status, script: a.ts, when: '{long}' }}]\n"
    ));
    assert!(
        text.contains("bindings[0].when") && text.contains("1025 bytes"),
        "{text}"
    );
    let exact = "x".repeat(1024);
    let loaded = load(&manifest(&format!(
        "bindings: [{{ point: task.before_status, script: a.ts, when: '{exact}' }}]\n"
    )));
    assert_eq!(check_manifest(&loaded), Ok(()));
}

// Paths.

#[test]
fn a_path_is_in_the_pack_unless_it_starts_with_repo() {
    assert_eq!(
        parse_pack_path("hooks/a.ts"),
        Ok(PackPath::InPack("hooks/a.ts"))
    );
    assert_eq!(
        parse_pack_path("./hooks/a.ts"),
        Ok(PackPath::InPack("./hooks/a.ts"))
    );
    assert_eq!(
        parse_pack_path("repo:.claude/agents/builder.md"),
        Ok(PackPath::InRepo(".claude/agents/builder.md"))
    );
}

#[test]
fn a_path_that_leaves_the_pack_or_the_snapshot_is_refused() {
    for (path, problem) in [
        ("", PathProblem::Empty),
        ("  ", PathProblem::Empty),
        ("repo:", PathProblem::Empty),
        ("/etc/passwd", PathProblem::Absolute),
        ("\\share\\x", PathProblem::Absolute),
        ("repo:/etc/passwd", PathProblem::Absolute),
        ("../x.ts", PathProblem::Climbs),
        ("hooks/../../x.ts", PathProblem::Climbs),
        ("repo:../x.md", PathProblem::Climbs),
        ("repo:a\\..\\b", PathProblem::Climbs),
        (
            "dev:~/x.ts",
            PathProblem::Scheme {
                scheme: "dev".into(),
            },
        ),
        (
            "git:github.com/org/x",
            PathProblem::Scheme {
                scheme: "git".into(),
            },
        ),
        ("C:\\x.ts", PathProblem::Scheme { scheme: "C".into() }),
        (
            "repo:repo:x",
            PathProblem::Scheme {
                scheme: "repo".into(),
            },
        ),
    ] {
        assert_eq!(parse_pack_path(path), Err(problem), "{path:?}");
    }
}

#[test]
fn a_dotted_name_is_not_a_climb() {
    assert!(parse_pack_path("hooks/..a/b..ts").is_ok());
}

#[test]
fn every_script_wait_hook_prompt_and_definition_path_is_checked() {
    let body = "\
providers: { p: { script: /p } }\n\
workspaces: { w: { create: { script: /ws }, remove: { script: ../rm } } }\n\
integrations: { i: { script: /i } }\n\
syncs: { s: { script: /s } }\n\
record: { paths: [a], script: /record }\n\
gates: { g: { script: /g } }\n\
bindings: [{ point: run.after_end, script: /b }]\n\
observers: { o: { script: /o, events: ['*'] } }\n\
metrics: { import: { script: /m }, processes: { script: /mp } }\n\
experiment: { prepare: { script: /e } }\n\
roles: { r: { agent: claude, definition: /d } }\n\
prompts: { p: /prompt }\n\
workflows:\n\
\x20 w:\n\
\x20   phases:\n\
\x20     a: { script: /script }\n\
\x20     b: { wait: /wait, timeout: 5m }\n\
\x20     c: { role: r, prompt: p, then: { script: /decide, on: { go: { end: x } } } }\n";
    let text = refusal_text(body);
    for at in [
        "providers.p.script",
        "workspaces.w.create.script",
        "workspaces.w.remove.script",
        "integrations.i.script",
        "syncs.s.script",
        "record.script",
        "gates.g.script",
        "bindings[0].script",
        "observers.o.script",
        "metrics.import.script",
        "metrics.processes.script",
        "experiment.prepare.script",
        "roles.r.definition",
        "prompts.p",
        "workflows.w.phases.a.script",
        "workflows.w.phases.b.wait",
        "workflows.w.phases.c.then.script",
    ] {
        assert!(
            text.contains(&format!("`{at}`")),
            "{at} not refused in:\n{text}"
        );
    }
}

#[test]
fn a_repo_path_is_accepted_for_a_definition_and_a_script() {
    let loaded = load(&manifest(
        "roles: { r: { agent: claude, definition: 'repo:.claude/agents/r.md' } }\n\
         gates: { g: { script: 'repo:tools/check.ts' } }\n",
    ));
    assert_eq!(check_manifest(&loaded), Ok(()));
}

#[test]
fn record_and_protected_globs_are_repo_paths_that_cannot_leave_the_repo() {
    let text = refusal_text(
        "record: { paths: ['/abs', 'ok/**', '../up'], script: r.ts }\n\
         protectedPaths: ['/also-abs']\n",
    );
    assert!(
        text.contains("record.paths[0]") && text.contains("record.paths[2]"),
        "{text}"
    );
    assert!(!text.contains("record.paths[1]"), "{text}");
    assert!(text.contains("protectedPaths[0]"), "{text}");
}

#[test]
fn protected_paths_is_a_list_or_an_object_with_at_phase_end() {
    let list = load(&manifest("protectedPaths: ['.agents/**', 'tools/**']\n"));
    assert!(matches!(list.protected_paths, Some(ProtectedPaths::Paths(ref p)) if p.len() == 2));
    let object = load(&manifest(
        "protectedPaths: { paths: ['.agents/**'], at: phase-end }\n",
    ));
    match object.protected_paths {
        Some(ProtectedPaths::AtPhaseEnd(ref at)) => assert_eq!(at.at, ProtectedAt::PhaseEnd),
        other => panic!("{other:?}"),
    }
    let error = load_yaml::<PackManifest>(
        manifest("protectedPaths: { paths: ['a'], at: commit }\n").as_bytes(),
        "test.yaml",
    );
    assert!(error.is_err());
}

// Other checks.

#[test]
fn a_hook_timeout_of_zero_is_refused_and_a_duration_needs_its_unit() {
    let text = refusal_text("gates: { g: { script: g.ts, timeout: 0m } }\n");
    assert!(text.contains("gates.g.timeout"), "{text}");
    let error = load_yaml::<PackManifest>(
        manifest("gates: { g: { script: g.ts, timeout: 120 } }\n").as_bytes(),
        "test.yaml",
    )
    .unwrap_err()
    .to_string();
    assert!(error.contains("quoted"), "{error}");
}

#[test]
fn names_are_one_word() {
    let text = refusal_text(
        "roles: { 'a/b': { agent: claude }, 'a b': { agent: claude } }\n\
         gates: { '': { script: g.ts } }\n",
    );
    assert!(
        text.contains("role `a/b`") && text.contains("role `a b`"),
        "{text}"
    );
    assert!(text.contains("gate ``"), "{text}");
    let spaced = check_manifest(&load("apiVersion: collide/v0\nname: has space\n")).unwrap_err();
    assert_eq!(
        spaced,
        [ManifestError::BadName {
            kind: "pack",
            name: "has space".into()
        }]
    );
}

#[test]
fn a_pack_name_may_hold_one_slash_like_the_built_in_plan_pack() {
    let loaded = load("apiVersion: collide/v0\nname: collide/plan\n");
    assert_eq!(check_manifest(&loaded), Ok(()));
}

#[test]
fn a_setting_flag_must_be_a_boolean_and_a_setting_must_be_a_mapping() {
    let text = refusal_text(
        "config: { a: { type: string, secret: yes_please }, b: { overridable: 1 } }\n",
    );
    assert!(
        text.contains("config.a") && text.contains("config.b"),
        "{text}"
    );
    let error = load_yaml::<PackManifest>(manifest("config: { a: string }\n").as_bytes(), "t.yaml");
    assert!(error.is_ok(), "a scalar loads as data");
    assert_eq!(
        check_manifest(&error.unwrap()).unwrap_err(),
        [ManifestError::NotASchema {
            at: "config.a".into()
        }]
    );
}

#[test]
fn a_view_key_is_its_name() {
    let errors = refusals("views: { mvp: { name: MVP } }\n");
    assert_eq!(
        errors,
        [ManifestError::ViewNameMismatch {
            key: "mvp".into(),
            name: "MVP".into()
        }]
    );
    let text = errors[0].to_string();
    assert!(text.contains("mvp") && text.contains("MVP"), "{text}");
    let same = load(&manifest("views: { MVP: { name: MVP, where: 'true' } }\n"));
    assert_eq!(check_manifest(&same), Ok(()));
}

#[test]
fn a_view_is_checked_as_a_view() {
    let errors = refusals("views: { v: { name: v, workflow: not-namespaced } }\n");
    assert!(matches!(&errors[..], [ManifestError::View { key, .. }] if key == "v"));
}

#[test]
fn a_pack_profile_covers_only_its_own_settings_and_workflows() {
    let own = load(&manifest(
        "profiles:\n  small: { workflow: demo/review, config: { demo: { lanePool: 1 } } }\n",
    ));
    assert_eq!(check_manifest(&own), Ok(()));
    let slashed = load(
        "apiVersion: collide/v0\nname: collide/plan\nprofiles:\n  p: { config: { collide/plan: { layout: x } } }\n",
    );
    assert_eq!(check_manifest(&slashed), Ok(()));
    let text = refusal_text(
        "profiles:\n  wide:\n    packs: { off: [x] }\n    workflow: other/review\n    config: { other: { k: 1 } }\n",
    );
    for key in ["packs", "workflow: other/review", "config.other"] {
        assert!(text.contains(&format!("`{key}` reaches")), "{key}: {text}");
    }
}

#[test]
fn a_profile_is_checked_as_a_profile() {
    let errors = refusals("profiles: { p: { packs: { on: [a], off: [a] } } }\n");
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, ManifestError::Profile { key, .. } if key == "p"))
    );
}

#[test]
fn the_workflow_checks_run_over_every_workflow_entry() {
    let errors = refusals(
        "workflows:\n\
         \x20 one: { phases: { a: { role: r, prompt: p, then: nowhere } } }\n\
         \x20 two: { start: ghost, phases: { a: { role: r, prompt: p } } }\n",
    );
    assert_eq!(
        errors,
        [
            ManifestError::Workflow {
                name: "one".into(),
                error: WorkflowError::DanglingThen {
                    phase: "a".into(),
                    at: "then".into(),
                    target: "nowhere".into()
                }
            },
            ManifestError::Workflow {
                name: "two".into(),
                error: WorkflowError::UnknownStart {
                    start: "ghost".into()
                }
            },
        ]
    );
}

#[test]
fn empty_workflow_strings_are_refused_where_the_schema_has_min_length() {
    let text = refusal_text(
        "workflows:\n\
         \x20 w:\n\
         \x20   on_start: [{ op: '' }]\n\
         \x20   phases:\n\
         \x20     a: { role: '', prompt: { artifact: '' } }\n\
         \x20     b: { human: '', on: { go: { end: x } } }\n\
         \x20     c: { script: '' }\n",
    );
    for at in [
        "workflows.w.on_start[0].op",
        "workflows.w.phases.a.role",
        "workflows.w.phases.a.prompt.artifact",
        "workflows.w.phases.b.human",
        "workflows.w.phases.c.script",
    ] {
        assert!(
            text.contains(&format!("`{at}` is empty")) || text.contains(at),
            "{at}: {text}"
        );
    }
}

#[test]
fn derived_points_are_checked_against_the_core_points() {
    let errors = refusals("points: { spike.before_conclude: { on: task.before_statuz } }\n");
    assert!(matches!(&errors[..], [ManifestError::Point(_)]));
}

#[test]
fn empty_checklist_strings_and_event_patterns_are_refused() {
    let text = refusal_text(
        "checklists: { c: { version: 1, gates: done, items: [{ id: '', text: '' }] } }\n\
         observers: { o: { script: o.ts, events: ['run *', ''] } }\n",
    );
    for at in [
        "checklists.c.items[0].id",
        "checklists.c.items[0].text",
        "observers.o.events[1]",
    ] {
        assert!(text.contains(at), "{at}: {text}");
    }
    assert!(text.contains("event pattern `run *`"), "{text}");
}

#[test]
fn every_refusal_of_a_manifest_comes_back_in_one_pass() {
    let errors = refusals(
        "gates: { g: { script: /abs } }\n\
         bindings: [{ point: run.after_end, gate: g }]\n\
         views: { a: { name: b } }\n\
         roles: { r: { agent: '' } }\n",
    );
    assert!(errors.len() >= 4, "{errors:?}");
}

#[test]
fn a_phase_kind_survives_the_manifest() {
    let manifest: PackManifest = load_yaml_file(&repo("fixtures/packs/pr-ci/pack.yaml")).unwrap();
    let ci = manifest.workflows["pr"].phases.get("ci").unwrap();
    assert!(matches!(ci, Phase::Wait(wait) if wait.wait == "hooks/ci-checks.ts"));
}

// The hazards of S5, read through the loader.

#[test]
fn no_and_a_quoted_version_stay_strings_in_a_manifest() {
    let loaded = load(&manifest(
        "version: \"0.10\"\nroles: { r: { agent: claude, model: no } }\nlaunch: { flag: { default: no } }\n",
    ));
    assert_eq!(loaded.version.as_deref(), Some("0.10"));
    assert_eq!(loaded.roles["r"].model.as_deref(), Some("no"));
    assert_eq!(
        loaded.launch["flag"].0["default"],
        Value::String("no".into())
    );
    assert_eq!(check_manifest(&loaded), Ok(()));
}

#[test]
fn an_unquoted_ambiguous_scalar_in_a_string_field_is_an_error() {
    for text in ["version: 0.10\n", "version: 1e10\n", "version: true\n"] {
        let error = load_yaml::<PackManifest>(manifest(text).as_bytes(), "t.yaml");
        assert!(error.is_err(), "{text}");
    }
}

// The registry and the key list.

#[test]
fn the_registry_lists_the_manifest_keys() {
    let schema = schema_file("PackManifest.input.json");
    let keys: BTreeSet<&str> = schema["properties"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    let expected: BTreeSet<&str> = [
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
        "workspaces",
    ]
    .into_iter()
    .collect();
    assert_eq!(keys, expected);
    assert_eq!(
        schema["required"],
        serde_json::json!(["apiVersion", "name"])
    );
}

#[test]
fn the_collide_pack_declares_quick_as_one_builder_phase_that_moves_the_task_by_category() {
    let manifest: PackManifest = load_yaml_file(&repo("packs/collide/pack.yaml")).unwrap();
    let quick = manifest
        .workflows
        .get("quick")
        .expect("the collide pack declares quick");
    let phases: Vec<&str> = quick.phases.iter().map(|(name, _)| name).collect();
    assert_eq!(phases, ["build"]);
    let Some(Phase::Agent(build)) = quick.phases.get("build") else {
        panic!("build is an agent phase");
    };
    assert_eq!(build.role, "builder");
    assert!(build.then.is_none(), "the last phase ends the run, done");
    assert_eq!(quick.integrate.as_deref(), Some("collide/merge-local"));
    let ops = |ops: &[collide_core::workflow::WorkflowOp]| -> Vec<(String, Value, Option<String>)> {
        ops.iter()
            .map(|op| (op.op.clone(), op.args["category"].clone(), op.when.clone()))
            .collect()
    };
    assert_eq!(
        ops(&quick.on_start),
        [("setStatus".to_string(), "active".into(), None)]
    );
    assert_eq!(
        ops(&quick.on_end),
        [("setStatus".to_string(), "review".into(), None)],
        "every end of a quick run goes to review, failed and stopped too"
    );
}
