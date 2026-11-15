//! Tests for the workflow transition-table types (ts-0024): each phase kind, the structural checks as pure
//! functions, closed structs at every depth, and the fixture workflows.

use collide_core::duration::Duration;
use collide_core::pack::PackManifest;
use collide_core::schema;
use collide_core::workflow::{
    ASK_HUMAN, Adoption, DEFAULT_WAIT_INTERVAL, Phase, PhaseKind, Target, Then, Workflow,
};
use collide_core::workflow_check::{
    AdoptionError, LaunchRefusal, WorkflowError, check_adoption, check_launch, identical_instances,
    validate,
};
use collide_core::yaml::{load_yaml, load_yaml_file};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn workflow(text: &str) -> Workflow {
    load_yaml(text.as_bytes(), "workflow.yaml").unwrap_or_else(|error| panic!("{error}\n{text}"))
}

fn parse_error(text: &str) -> String {
    load_yaml::<Workflow>(text.as_bytes(), "workflow.yaml")
        .expect_err("the workflow should not parse")
        .to_string()
}

fn refusals(text: &str) -> Vec<WorkflowError> {
    validate(&workflow(text)).expect_err("the workflow should be refused")
}

fn phase_of(text: &str, name: &str) -> Phase {
    workflow(text).phases.get(name).unwrap().clone()
}

// Each phase kind.

#[test]
fn an_agent_phase_has_a_role_and_a_prompt() {
    let phase = phase_of(
        "phases:\n  build: { role: builder, prompt: build, label: build, gates: [evidence] }\n",
        "build",
    );
    assert_eq!(phase.kind(), PhaseKind::Agent);
    let Phase::Agent(agent) = phase else { panic!() };
    assert_eq!(agent.role, "builder");
    assert_eq!(agent.gates, ["evidence"]);
}

#[test]
fn an_agent_phase_prompt_can_be_an_earlier_artifact() {
    let phase = phase_of(
        "phases:\n  fix: { role: builder, prompt: { artifact: adjudicate/fix-brief.md } }\n",
        "fix",
    );
    let Phase::Agent(agent) = phase else { panic!() };
    assert_eq!(
        serde_json::to_value(&agent.prompt).unwrap(),
        json!({ "artifact": "adjudicate/fix-brief.md" })
    );
}

#[test]
fn on_cold_defaults_to_resume_and_can_be_fresh() {
    let doc = workflow(
        "phases:\n  a: { role: r, prompt: p }\n  b: { role: r, prompt: p, on_cold: fresh }\n",
    );
    let on_cold = |name: &str| match doc.phases.get(name).unwrap() {
        Phase::Agent(agent) => serde_json::to_value(agent.on_cold).unwrap(),
        _ => panic!(),
    };
    assert_eq!(on_cold("a"), json!("resume"));
    assert_eq!(on_cold("b"), json!("fresh"));
}

#[test]
fn on_cold_is_refused_on_a_script_phase() {
    let error = parse_error("phases:\n  push: { script: hooks/push.ts, on_cold: fresh }\n");
    assert!(
        error.contains("`on_cold` is not a key of a script phase"),
        "{error}"
    );
}

#[test]
fn a_fanout_phase_lists_models() {
    let phase = phase_of(
        "phases:\n  review:\n    role: reviewer\n    prompt: review\n    sandbox: { models: [strong] }\n    fanout: { models: [strong, fast], when: \"round == 1\" }\n",
        "review",
    );
    assert_eq!(phase.kind(), PhaseKind::Fanout);
    let Phase::Agent(agent) = phase else { panic!() };
    let fanout = agent.fanout.unwrap();
    assert_eq!(fanout.models, ["strong", "fast"]);
    assert_eq!(fanout.when.as_deref(), Some("round == 1"));
    assert_eq!(agent.sandbox.unwrap().models, ["strong"]);
}

const THREE_INSTANCES: &str = "phases:
  plan:
    role: planner
    prompt: plan
    fanout:
      instances:
        - { model: strong, prompt: plan-a }
        - { definition: agents/skeptic.md, profile: cheap }
        - { prompt: plan-c }
";

#[test]
fn a_fanout_instances_list_parses() {
    let phase = phase_of(THREE_INSTANCES, "plan");
    assert_eq!(phase.kind(), PhaseKind::Fanout);
    let Phase::Agent(agent) = phase else { panic!() };
    let instances = agent.fanout.unwrap().instances;
    assert_eq!(instances.len(), 3);
    assert_eq!(instances[0].model.as_deref(), Some("strong"));
    assert_eq!(
        instances[1].definition.as_deref(),
        Some("agents/skeptic.md")
    );
    assert_eq!(instances[1].profile.as_deref(), Some("cheap"));
    assert!(instances[2].prompt.is_some());
    assert_eq!(validate(&workflow(THREE_INSTANCES)), Ok(()));
    assert_eq!(check_launch(&workflow(THREE_INSTANCES)), Ok(()));
}

#[test]
fn two_identical_instances_are_marked_for_refusal_at_launch() {
    let doc = workflow(
        "phases:
  plan:
    role: planner
    prompt: plan
    fanout:
      instances:
        - { model: strong, prompt: plan-a }
        - { model: fast }
        - { model: strong, prompt: plan-a }
",
    );
    assert_eq!(validate(&doc), Ok(()), "a table with them is still a table");
    assert_eq!(
        check_launch(&doc),
        Err(vec![LaunchRefusal::IdenticalInstances {
            phase: "plan".into(),
            first: 0,
            second: 2
        }])
    );
}

#[test]
fn identical_instances_names_each_repeat_against_its_first() {
    let doc = workflow(
        "phases:\n  p:\n    role: r\n    prompt: p\n    fanout: { instances: [{ model: a }, { model: a }, { model: a }] }\n",
    );
    let Phase::Agent(agent) = doc.phases.get("p").unwrap() else {
        panic!()
    };
    let instances = &agent.fanout.as_ref().unwrap().instances;
    assert_eq!(identical_instances(instances), [(0, 1), (0, 2)]);
}

#[test]
fn a_model_listed_twice_in_a_fanout_is_refused_at_launch() {
    let doc = workflow(
        "phases:\n  review: { role: r, prompt: p, fanout: { models: [strong, strong] } }\n",
    );
    assert_eq!(
        check_launch(&doc),
        Err(vec![LaunchRefusal::RepeatedModel {
            phase: "review".into(),
            model: "strong".into()
        }])
    );
}

#[test]
fn a_fanout_needs_models_or_instances_and_not_both() {
    for fanout in [
        "{ when: \"round == 1\" }",
        "{ models: [a], instances: [{ model: b }] }",
    ] {
        let errors = refusals(&format!(
            "phases:\n  p: {{ role: r, prompt: p, fanout: {fanout} }}\n"
        ));
        assert_eq!(
            errors,
            [WorkflowError::FanoutSource { phase: "p".into() }],
            "{fanout}"
        );
    }
}

#[test]
fn an_instance_that_names_nothing_is_refused() {
    let errors = refusals("phases:\n  p: { role: r, prompt: p, fanout: { instances: [{}] } }\n");
    assert_eq!(
        errors,
        [WorkflowError::EmptyInstance {
            phase: "p".into(),
            index: 0
        }]
    );
}

#[test]
fn a_script_phase_can_mark_agent_work() {
    let phase = phase_of(
        "phases:\n  run: { script: hooks/run.ts, agentWork: true, timeout: 2h, outputs: [report.md] }\n",
        "run",
    );
    assert_eq!(phase.kind(), PhaseKind::Script);
    let Phase::Script(script) = phase else {
        panic!()
    };
    assert!(script.agent_work);
    assert_eq!(script.timeout, Some(Duration::from_secs(7200)));
}

#[test]
fn a_script_phase_is_not_agent_work_unless_it_says_so() {
    let Phase::Script(script) = phase_of("phases:\n  push: { script: hooks/push.ts }\n", "push")
    else {
        panic!()
    };
    assert!(!script.agent_work);
}

#[test]
fn a_wait_phase_interval_defaults_to_30_seconds() {
    let phase = phase_of("phases:\n  ci: { wait: hooks/ci.ts, timeout: 60m }\n", "ci");
    assert_eq!(phase.kind(), PhaseKind::Wait);
    let Phase::Wait(wait) = phase else { panic!() };
    assert_eq!(wait.interval, DEFAULT_WAIT_INTERVAL);
    assert_eq!(wait.interval.as_secs(), 30);
    assert_eq!(wait.timeout.as_secs(), 3600);
}

#[test]
fn a_wait_phase_reads_its_interval_and_timeout_as_durations() {
    let Phase::Wait(wait) = phase_of(
        "phases:\n  ci: { wait: hooks/ci.ts, interval: 10s, timeout: 2h }\n",
        "ci",
    ) else {
        panic!()
    };
    assert_eq!(wait.interval, Duration::from_secs(10));
    assert_eq!(wait.timeout, Duration::from_secs(7200));
}

#[test]
fn a_wait_phase_without_a_timeout_fails() {
    let error = parse_error("phases:\n  ci: { wait: hooks/ci.ts, interval: 10s }\n");
    assert!(error.contains("a wait phase needs `timeout:`"), "{error}");
}

#[test]
fn a_wait_phase_timeout_that_is_not_a_duration_fails() {
    for bad in ["60", "1.5h", "soon", "5 minutes", "-1m"] {
        let error = parse_error(&format!(
            "phases:\n  ci: {{ wait: hooks/ci.ts, timeout: \"{bad}\" }}\n"
        ));
        assert!(error.contains("duration"), "{bad}: {error}");
    }
}

#[test]
fn a_wait_phase_that_ends_at_once_or_polls_too_fast_is_refused() {
    let errors = refusals("phases:\n  ci: { wait: hooks/ci.ts, interval: 0s, timeout: 0s }\n");
    assert_eq!(
        errors,
        [
            WorkflowError::ZeroTimeout { phase: "ci".into() },
            WorkflowError::IntervalTooShort {
                phase: "ci".into(),
                interval: Duration::from_secs(0)
            }
        ]
    );
}

#[test]
fn a_human_phase_picks_one_of_its_tokens() {
    let phase = phase_of(
        "phases:\n  build: { role: b, prompt: p }\n  ok:\n    human: Approve it.\n    on: { approve: { end: approved }, changes: build, stop: ask_human }\n    show: [ci/summary.md]\n",
        "ok",
    );
    assert_eq!(phase.kind(), PhaseKind::Human);
    let Phase::Human(human) = phase else { panic!() };
    assert_eq!(
        human.on["approve"],
        Target::End(collide_core::workflow::EndTarget {
            end: "approved".into()
        })
    );
    assert_eq!(human.on["changes"], Target::Name("build".into()));
    assert!(human.on["stop"].is_ask_human());
    assert_eq!(human.show, ["ci/summary.md"]);
}

#[test]
fn a_human_phase_with_no_tokens_is_refused() {
    let errors = refusals("phases:\n  ok: { human: Approve it., on: {} }\n");
    assert_eq!(errors, [WorkflowError::NoTokens { phase: "ok".into() }]);
}

#[test]
fn a_phase_must_say_what_kind_it_is() {
    let error = parse_error("phases:\n  nothing: { label: x }\n");
    assert!(error.contains("needs `role:`"), "{error}");
    let error = parse_error("phases:\n  both: { role: r, prompt: p, script: s.ts }\n");
    assert!(error.contains("one kind"), "{error}");
}

// then:

#[test]
fn then_is_a_phase_an_end_ask_human_or_a_decision() {
    let doc = workflow(
        "phases:
  a: { role: r, prompt: p, then: b }
  b: { role: r, prompt: p, then: { end: closed } }
  c: { role: r, prompt: p, then: ask_human }
  d:
    role: r
    prompt: p
    then: { script: next.ts, on: { go: a, stop: ask_human, done: { end: ok } } }
",
    );
    let then = |name: &str| doc.phases.get(name).unwrap().then().unwrap().clone();
    assert_eq!(then("a"), Then::Target(Target::Name("b".into())));
    assert_eq!(
        then("b"),
        Then::Target(Target::End(collide_core::workflow::EndTarget {
            end: "closed".into()
        }))
    );
    assert!(matches!(then("c"), Then::Target(target) if target.is_ask_human()));
    let Then::Decision(decision) = then("d") else {
        panic!()
    };
    assert_eq!(decision.script, "next.ts");
    assert_eq!(decision.target_for("go"), Some(&Target::Name("a".into())));
    assert_eq!(decision.target_for("other"), None);
    assert_eq!(ASK_HUMAN, "ask_human");
    assert_eq!(validate(&doc), Ok(()));
}

#[test]
fn without_then_the_next_phase_in_declaration_order_follows() {
    let doc = workflow(
        "phases:\n  zeta: { role: r, prompt: p }\n  alpha: { role: r, prompt: p }\n  mid: { role: r, prompt: p }\n",
    );
    let names: Vec<&str> = doc.phases.iter().map(|(name, _)| name).collect();
    assert_eq!(names, ["zeta", "alpha", "mid"]);
    assert_eq!(doc.phases.next_after("zeta"), Some("alpha"));
    assert_eq!(doc.phases.next_after("mid"), None);
    assert_eq!(doc.start_phase(), Some("zeta"));
}

#[test]
fn a_then_that_names_no_phase_fails() {
    let errors = refusals("phases:\n  a: { role: r, prompt: p, then: nowhere }\n");
    assert_eq!(
        errors,
        [WorkflowError::DanglingThen {
            phase: "a".into(),
            at: "then".into(),
            target: "nowhere".into()
        }]
    );
    assert_eq!(
        errors[0].to_string(),
        "phase `a`: `then` goes to `nowhere`, which is not a phase of this workflow"
    );
}

#[test]
fn a_decision_target_or_a_human_target_that_names_no_phase_fails() {
    let errors = refusals(
        "phases:
  a: { role: r, prompt: p, then: { script: n.ts, on: { go: missing } } }
  h: { human: Pick., on: { back: gone } }
",
    );
    let at: Vec<(&str, &str)> = errors
        .iter()
        .map(|error| match error {
            WorkflowError::DanglingThen { at, target, .. } => (at.as_str(), target.as_str()),
            other => panic!("{other}"),
        })
        .collect();
    assert_eq!(at, [("then.on.go", "missing"), ("on.back", "gone")]);
}

#[test]
fn a_decision_token_that_tokens_does_not_declare_fails() {
    let errors = refusals(
        "phases:
  a:
    role: r
    prompt: p
    then: { script: n.ts, tokens: [fix, stop], on: { fix: a, stop: ask_human, close: a } }
",
    );
    assert_eq!(
        errors,
        [WorkflowError::UndeclaredToken {
            phase: "a".into(),
            token: "close".into()
        }]
    );
}

#[test]
fn a_declared_token_with_no_target_fails() {
    let errors = refusals(
        "phases:\n  a: { role: r, prompt: p, then: { script: n.ts, tokens: [fix, stop], on: { fix: a } } }\n",
    );
    assert_eq!(
        errors,
        [WorkflowError::UnhandledToken {
            phase: "a".into(),
            token: "stop".into()
        }]
    );
}

#[test]
fn a_decision_with_no_tokens_fails() {
    let errors = refusals("phases:\n  a: { role: r, prompt: p, then: { script: n.ts, on: {} } }\n");
    assert_eq!(errors, [WorkflowError::NoTokens { phase: "a".into() }]);
}

#[test]
fn a_decision_needs_a_script_and_an_end_is_not_a_decision() {
    let error = parse_error("phases:\n  a: { role: r, prompt: p, then: { on: { go: a } } }\n");
    assert!(
        error.contains("a decision needs `script:` and `on:`"),
        "{error}"
    );
    let error = parse_error(
        "phases:\n  a: { role: r, prompt: p, then: { end: x, script: n.ts, on: { go: a } } }\n",
    );
    assert!(error.contains("`end` cannot be combined"), "{error}");
}

#[test]
fn an_end_with_no_outcome_is_refused() {
    let errors = refusals("phases:\n  a: { role: r, prompt: p, then: { end: \"\" } }\n");
    assert_eq!(
        errors,
        [WorkflowError::EmptyOutcome {
            phase: "a".into(),
            at: "then".into()
        }]
    );
}

#[test]
fn start_must_be_a_phase_and_a_phase_cannot_be_named_ask_human() {
    let errors = refusals("start: nope\nphases:\n  ask_human: { role: r, prompt: p }\n");
    assert_eq!(
        errors,
        [
            WorkflowError::UnknownStart {
                start: "nope".into()
            },
            WorkflowError::BadPhaseName {
                name: "ask_human".into()
            }
        ]
    );
}

#[test]
fn a_workflow_with_no_phases_is_refused() {
    assert_eq!(refusals("phases: {}\n"), [WorkflowError::NoPhases]);
}

#[test]
fn implementation_names_are_pack_slash_name() {
    let doc = "workspace: worktree\nintegrate: a/b/c\nphases:\n  a: { role: r, prompt: p }\n";
    assert_eq!(
        refusals(doc),
        [
            WorkflowError::BadImplementationName {
                key: "workspace",
                name: "worktree".into()
            },
            WorkflowError::BadImplementationName {
                key: "integrate",
                name: "a/b/c".into()
            }
        ]
    );
    let good = workflow(
        "workspace: pack/worktree\nintegrate: pack/merge-local\nphases:\n  a: { role: r, prompt: p }\n",
    );
    assert_eq!(validate(&good), Ok(()));
    assert_eq!(good.workspace.as_deref(), Some("pack/worktree"));
}

#[test]
fn a_phase_declared_twice_fails() {
    let error = parse_error("phases:\n  a: { role: r, prompt: p }\n  a: { role: r, prompt: p }\n");
    assert!(error.contains("a"), "{error}");
}

// accepts, ops and adopting.

#[test]
fn accepts_and_ops_parse() {
    let doc = workflow(
        "accepts: { sources: [plan], fields: [labelId] }
on_start: [{ op: setStatus, args: { category: active } }]
on_end: [{ op: setStatus, args: { category: review }, when: 'outcome == \"closed\"' }]
phases:
  a: { role: r, prompt: p }
",
    );
    let accepts = doc.accepts.unwrap();
    assert_eq!(accepts.sources, ["plan"]);
    assert_eq!(accepts.fields, ["labelId"]);
    assert_eq!(doc.on_start[0].op, "setStatus");
    assert_eq!(doc.on_start[0].args["category"], json!("active"));
    assert_eq!(doc.on_end[0].when.as_deref(), Some("outcome == \"closed\""));
}

#[test]
fn adopting_names_a_phase_and_a_round() {
    let doc =
        workflow("phases:\n  build: { role: r, prompt: p }\n  review: { role: r, prompt: p }\n");
    let adoption = |phase: &str, round: u32| Adoption {
        phase: phase.into(),
        round,
    };
    assert_eq!(check_adoption(&doc, &adoption("review", 3)), Ok(()));
    assert_eq!(
        check_adoption(&doc, &adoption("fix", 1)),
        Err(AdoptionError::UnknownPhase {
            phase: "fix".into()
        })
    );
    assert_eq!(
        check_adoption(&doc, &adoption("review", 0)),
        Err(AdoptionError::RoundZero)
    );
}

// Closed structs, at every depth.

#[test]
fn an_unknown_key_fails_in_a_phase() {
    let error = parse_error("phases:\n  a: { role: r, prompt: p, colour: red }\n");
    assert!(error.contains("colour"), "{error}");
    assert!(error.starts_with("workflow.yaml:2:"), "{error}");
}

#[test]
fn a_key_of_another_kind_fails_in_a_phase_and_is_named() {
    for (phase, key) in [
        ("{ role: r, prompt: p, interval: 5s }", "interval"),
        ("{ role: r, prompt: p, show: [x] }", "show"),
        ("{ script: s.ts, role: r }", "one kind"),
        ("{ script: s.ts, interval: 5s }", "interval"),
        ("{ wait: w.ts, timeout: 1m, agentWork: true }", "agentWork"),
        ("{ human: Pick., on: { a: ask_human }, then: x }", "then"),
    ] {
        let error = parse_error(&format!("phases:\n  a: {phase}\n"));
        assert!(error.contains(key), "{phase}: {error}");
    }
}

#[test]
fn an_unknown_key_fails_at_every_depth() {
    for text in [
        "extra: 1\nphases:\n  a: { role: r, prompt: p }\n",
        "accepts: { extra: 1 }\nphases:\n  a: { role: r, prompt: p }\n",
        "on_start: [{ op: x, extra: 1 }]\nphases:\n  a: { role: r, prompt: p }\n",
        "phases:\n  a: { role: r, prompt: { artifact: x, extra: 1 } }\n",
        "phases:\n  a: { role: r, prompt: p, sandbox: { models: [a], extra: 1 } }\n",
        "phases:\n  a: { role: r, prompt: p, fanout: { models: [a], extra: 1 } }\n",
        "phases:\n  a: { role: r, prompt: p, fanout: { instances: [{ model: a, extra: 1 }] } }\n",
        "phases:\n  a: { role: r, prompt: p, then: { end: x, extra: 1 } }\n",
        "phases:\n  a: { role: r, prompt: p, then: { script: s, on: { go: { end: x, extra: 1 } }, extra: 1 } }\n",
        "phases:\n  a: { human: Pick., on: { go: { end: x, extra: 1 } } }\n",
    ] {
        let error = parse_error(text);
        assert!(error.contains("extra"), "{text}: {error}");
    }
}

// Fixtures.

fn repo(path: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join(path)
}

fn fixture_workflows(path: &str) -> BTreeMap<String, Workflow> {
    load_yaml_file::<PackManifest>(&repo(path))
        .unwrap_or_else(|error| panic!("{error}"))
        .workflows
}

fn assert_valid_and_round_trips(name: &str, workflow: &Workflow) {
    assert_eq!(validate(workflow), Ok(()), "{name}");
    assert_eq!(check_launch(workflow), Ok(()), "{name}");
    // Through JSON text, not `Value`: a `Value` map is sorted and would lose the phases' declaration order.
    let written = serde_json::to_string(workflow).unwrap();
    let back: Workflow = serde_json::from_str(&written).unwrap();
    assert_eq!(&back, workflow, "{name}");
    assert_eq!(serde_json::to_string(&back).unwrap(), written, "{name}");
}

#[test]
fn the_loop_fixture_workflow_is_a_transition_table() {
    let workflows = fixture_workflows("fixtures/packs/loop/pack.yaml");
    let review_loop = &workflows["loop"];
    assert_valid_and_round_trips("loop", review_loop);
    let kinds: Vec<(&str, PhaseKind)> = review_loop
        .phases
        .iter()
        .map(|(name, phase)| (name, phase.kind()))
        .collect();
    assert_eq!(
        kinds,
        [
            ("build", PhaseKind::Agent),
            ("review", PhaseKind::Fanout),
            ("adjudicate", PhaseKind::Agent),
            ("fix", PhaseKind::Agent),
            ("close", PhaseKind::Agent),
        ]
    );
}

#[test]
fn the_pr_ci_fixture_workflow_is_a_transition_table() {
    let workflows = fixture_workflows("fixtures/packs/pr-ci/pack.yaml");
    let pr = &workflows["pr"];
    assert_valid_and_round_trips("pr", pr);
    let kinds: Vec<PhaseKind> = pr.phases.iter().map(|(_, phase)| phase.kind()).collect();
    assert_eq!(
        kinds,
        [
            PhaseKind::Agent,
            PhaseKind::Script,
            PhaseKind::Wait,
            PhaseKind::Human
        ]
    );
}

#[test]
fn the_collide_review_workflow_is_a_transition_table() {
    let workflows = fixture_workflows("packs/collide/pack.yaml");
    assert_valid_and_round_trips("review", &workflows["review"]);
}

/// Only the `workflows:` key of a pack file.
#[derive(Deserialize)]
struct WorkflowsOnly {
    #[serde(default)]
    workflows: BTreeMap<String, Workflow>,
}

#[test]
fn the_workflows_of_every_fixture_pack_read_as_transition_tables() {
    let mut read = 0;
    for entry in std::fs::read_dir(repo("fixtures/packs")).unwrap() {
        let path = entry.unwrap().path().join("pack.yaml");
        let doc: WorkflowsOnly = load_yaml_file(&path).unwrap_or_else(|error| panic!("{error}"));
        for (name, workflow) in &doc.workflows {
            assert_valid_and_round_trips(&format!("{}:{name}", path.display()), workflow);
            read += 1;
        }
    }
    assert!(read >= 3, "{read} workflows read");
}

// Durations and schemas.

#[test]
fn durations_write_in_their_largest_whole_unit() {
    for (text, secs, written) in [
        ("30s", 30, "30s"),
        ("120m", 7200, "2h"),
        ("90m", 5400, "90m"),
        ("1d", 86_400, "1d"),
        ("0s", 0, "0s"),
    ] {
        let duration = Duration::parse(text).unwrap();
        assert_eq!(duration.as_secs(), secs, "{text}");
        assert_eq!(duration.to_string(), written, "{text}");
    }
}

#[test]
fn a_bare_number_is_not_a_duration() {
    let error = Duration::parse("120").unwrap_err().to_string();
    assert!(error.contains("no unit"), "{error}");
}

fn schema_file(name: &str) -> Value {
    serde_json::from_str(&schema::registry().render()[name]).unwrap()
}

#[test]
fn the_registry_has_the_workflow_and_adoption_schemas() {
    let names: Vec<String> = schema::registry().render().into_keys().collect();
    for name in [
        "Workflow.input.json",
        "Workflow.output.json",
        "Adoption.input.json",
    ] {
        assert!(names.contains(&name.to_string()), "{name}");
    }
}

#[test]
fn the_wait_phase_schema_requires_a_timeout_and_not_an_interval() {
    let schema = schema_file("Workflow.input.json");
    let wait = &schema["$defs"]["WaitPhase"];
    let required: Vec<&str> = wait["required"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(required.contains(&"timeout"), "{wait}");
    assert!(!required.contains(&"interval"), "{wait}");
    assert_eq!(wait["properties"]["interval"]["default"], json!("30s"));
    assert_eq!(wait["additionalProperties"], json!(false));
}

#[test]
fn the_pack_manifest_schema_has_workflows() {
    let schema = schema_file("PackManifest.input.json");
    assert!(schema["properties"]["workflows"].is_object());
}
