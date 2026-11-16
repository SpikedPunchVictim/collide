//! The hard-coded sequence of a prompt run is what `packs/collide/pack.yaml` declares as `quick` (ts-0078-ye; Stage 3c,
//! "From Stage 8"): the same phase, the same role and the same status moves. Stage 8 replaces the sequence with the
//! engine reading the table, and this test goes with it.

mod common;

use collide_core::item::Category;
use collide_core::pack::PackManifest;
use collide_core::workflow::Phase;
use collide_core::yaml::load_yaml_file;
use collide_host::file_run::quick_run::QUICK;
use serde_json::json;

fn manifest() -> PackManifest {
    load_yaml_file(&common::repo_root().join("packs/collide/pack.yaml")).unwrap()
}

#[test]
fn the_sequence_runs_the_phase_and_the_role_the_quick_workflow_declares() {
    let manifest = manifest();
    let quick = &manifest.workflows["quick"];
    let phases: Vec<&str> = quick.phases.iter().map(|(name, _)| name).collect();
    assert_eq!(phases, [QUICK.phase]);
    let Some(Phase::Agent(agent)) = quick.phases.get(QUICK.phase) else {
        panic!("the phase is an agent phase");
    };
    assert_eq!(format!("collide/{}", agent.role), QUICK.role);
}

#[test]
fn the_sequence_moves_the_task_to_the_categories_the_quick_workflow_declares() {
    let manifest = manifest();
    let quick = &manifest.workflows["quick"];
    assert_eq!(quick.on_start.len(), 1);
    assert_eq!(quick.on_start[0].op, "setStatus");
    assert_eq!(quick.on_start[0].args["category"], json!(QUICK.on_start));
    assert_eq!(quick.on_end.len(), 1);
    assert_eq!(quick.on_end[0].op, "setStatus");
    assert_eq!(quick.on_end[0].args["category"], json!(QUICK.on_end));
    assert_eq!(
        (QUICK.on_start, QUICK.on_end),
        (Category::Active, Category::Review)
    );
}

#[test]
fn the_sequence_is_named_as_the_workflow_is() {
    assert_eq!(QUICK.workflow, "collide/quick");
}
