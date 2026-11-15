//! Tests for the `core` outlines (ts-0019-vm): the model reference, the traits as trait objects, the signatures of
//! the stubs, and the rule that a role is refused for a capability the adapter cannot promise.

use collide_core::adapter::{AgentAdapter, CapabilityRefusal, refuse_unmet_capabilities};
use collide_core::agent::{Capabilities, ModelRef, Support};
use collide_core::engine::Engine;
use collide_core::pack::PackManifest;
use collide_core::resolve::{ImplementationRef, resolve_implementations, resolve_model_alias};
use collide_core::testing::{FakeAdapter, FakeHookRunner, FakeProvider, sample_launch_request};
use collide_core::workflow::Workflow;
use serde_json::json;
use std::collections::BTreeMap;
use std::path::PathBuf;

// --- The model reference (S7, P7) ---

#[test]
fn a_model_with_no_id_is_known_to_the_adapter_by_its_name() {
    let model = ModelRef {
        name: "sonnet".into(),
        id: None,
    };
    assert_eq!(model.adapter_id(), "sonnet");
}

#[test]
fn a_model_id_is_opaque_and_stays_beside_the_name() {
    let model = ModelRef {
        name: "qwen".into(),
        id: Some("ollama/qwen2.5:7b".into()),
    };
    assert_eq!(model.name, "qwen");
    assert_eq!(model.adapter_id(), "ollama/qwen2.5:7b");
}

#[test]
fn a_model_reference_reads_and_writes_the_id_only_when_there_is_one() {
    let plain: ModelRef = serde_json::from_value(json!({ "name": "opus" })).unwrap();
    assert_eq!(plain.id, None);
    assert_eq!(
        serde_json::to_value(&plain).unwrap(),
        json!({ "name": "opus" })
    );
    let with_id = json!({ "name": "qwen", "id": "ollama/qwen2.5:7b" });
    let read: ModelRef = serde_json::from_value(with_id.clone()).unwrap();
    assert_eq!(serde_json::to_value(&read).unwrap(), with_id);
}

#[test]
fn a_model_reference_with_an_unknown_key_is_refused() {
    let result = serde_json::from_value::<ModelRef>(json!({ "name": "opus", "alias": "fast" }));
    assert!(result.is_err());
}

// --- The traits as trait objects, over the doubles of `collide_core::testing` ---

#[test]
fn an_adapter_is_usable_through_a_trait_object_and_is_stateless() {
    let adapter: Box<dyn AgentAdapter> = Box::new(FakeAdapter::new(Vec::new()));
    let first = adapter.launch(&sample_launch_request()).unwrap();
    let second = adapter.launch(&sample_launch_request()).unwrap();
    assert_eq!(first, second);
    assert_eq!(first.cwd, PathBuf::from("/work"));
}

// --- The stubs: each one has the signature the outline gives, and says which work fills it ---

#[test]
#[should_panic(expected = "launch: capability refusal")]
fn the_capability_refusal_is_a_stub_that_names_its_tag() {
    let required = vec!["session_resume".to_string()];
    let _ = refuse_unmet_capabilities("tool", &Capabilities::default(), &required);
}

#[test]
fn a_capability_refusal_message_names_the_agent_the_capability_and_what_the_adapter_said() {
    let refusal = CapabilityRefusal {
        agent: "tool".into(),
        capability: "mcp_servers.http".into(),
        support: Support::Unknown,
    };
    assert!(refusal.to_string().contains("tool"));
    assert!(refusal.to_string().contains("mcp_servers.http"));
    assert!(refusal.to_string().contains("unknown"));
}

#[test]
fn the_capability_refusals_doc_names_the_rule() {
    let source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/adapter.rs"),
    )
    .unwrap();
    let before = source
        .split("pub fn refuse_unmet_capabilities")
        .next()
        .unwrap();
    let doc: Vec<&str> = before
        .lines()
        .rev()
        .take_while(|line| line.trim_start().starts_with("///"))
        .collect();
    let doc = doc.join("\n");
    assert!(doc.contains("`No`") && doc.contains("`Unknown`"), "{doc}");
    assert!(doc.contains("require"), "{doc}");
}

#[test]
#[should_panic(expected = "resolve: model alias")]
fn the_model_alias_resolution_is_a_stub_that_names_its_tag() {
    let _ = resolve_model_alias("fast", &BTreeMap::new(), None);
}

#[test]
#[should_panic(expected = "resolve: implementations")]
fn the_implementation_resolution_is_a_stub_that_names_its_tag() {
    let workflow: Workflow = serde_json::from_value(json!({ "phases": {} })).unwrap();
    let packs: BTreeMap<String, PackManifest> = BTreeMap::new();
    let _ = resolve_implementations("demo/loop", &workflow, &packs);
}

#[test]
fn the_built_in_implementation_is_a_value_a_run_can_record() {
    assert_ne!(
        ImplementationRef::Builtin,
        ImplementationRef::Pack {
            pack: "demo".into(),
            name: "worktree".into()
        }
    );
}

#[test]
fn the_engine_takes_its_collaborators_as_the_core_traits() {
    let hooks = FakeHookRunner::new();
    let provider = FakeProvider::new();
    let adapter = FakeAdapter::new(Vec::new());
    let engine = Engine {
        adapters: BTreeMap::from([("tool".to_string(), &adapter as &dyn AgentAdapter)]),
        hooks: &hooks,
        provider: &provider,
    };
    assert_eq!(engine.adapters.len(), 1);
}
