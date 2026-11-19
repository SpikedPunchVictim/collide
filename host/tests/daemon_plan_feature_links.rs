//! Feature links, link types and coverage through the daemon (ts-0133-kg; 0.8, 0.10 "A proposal's features" and
//! "A proposal's quality parts"): a task's link to `pr-0001-aa#F1` resolves in the corpus, the proposal sees it from
//! its side, `task.get` carries the source's link types with what the other end calls each, and the proposal reports
//! its coverage. The provider is the real built-in `collide/plan` pack from a built bundle, run by `node`, over a
//! temporary repository.
//!
//! Every path is below the test's own root under `/tmp`, which is also the home. Nothing here touches the user's
//! home, keychain or a real daemon.

mod common;

use collide_core::item::LinkDir;
use collide_core::owner::{TaskGetParams, TaskGetResult};
use common::plan_rig::{EPIC, Rig, list_until, rig, rpc, trust};

const PROPOSAL: &str = "pr-0001-aa";
const IMPLEMENTER: &str = "ts-0001-aa";
const BROKEN: &str = "ts-0002-bb";
const UNLINKED: &str = "ts-0003-cc";

const PROPOSAL_FILE: &str = ".collide/planning/epics/ep-001-aa-alpha/proposals/pr-0001-aa-idea.md";

fn doc(kind: &str, id: &str, status: &str, links: &str, body: &str) -> String {
    format!(
        "---\nid: {id}\nkind: {kind}\nstatus: {status}\ntitle: \"Made-up {kind}\"\nowners: []\n{links}---\n\n{body}"
    )
}

fn task_file(id: &str) -> String {
    format!(".collide/planning/epics/ep-001-aa-alpha/tasks/{id}-work.md")
}

fn implements(to: &str) -> String {
    format!("links:\n  - {{ type: implements, to: {to} }}\n")
}

/// A trusted board: a proposal with F1 and F2, a task that implements F1, one that implements an F9 the proposal
/// lacks, and one that implements nothing. The proposal includes the third.
fn board() -> (Rig, String) {
    let rig = rig("daemon-plan-feature-links");
    rig.write(".collide/planning/epics/ep-001-aa-alpha/epic.md", EPIC);
    let features = "## Features\n\n- **F1 First**: one.\n- **F2 Second**: two.\n";
    let includes = format!("links:\n  - {{ type: includes, to: {UNLINKED} }}\n");
    rig.write(
        PROPOSAL_FILE,
        &doc("proposal", PROPOSAL, "accepted", &includes, features),
    );
    for (id, links) in [
        (IMPLEMENTER, implements(&format!("{PROPOSAL}#F1"))),
        (BROKEN, implements(&format!("{PROPOSAL}#F9"))),
        (UNLINKED, "links: []\n".to_string()),
    ] {
        rig.write(
            &task_file(id),
            &doc("task", id, "ready", &links, "## Goal\n"),
        );
    }
    rig.commit();
    let board = rig.board();
    trust(&rig, &board);
    list_until(&rig, &board, "the plan source is listed", |list| {
        !list.refreshing && list.tasks.len() == 5
    });
    (rig, board)
}

fn get(rig: &Rig, board: &str, id: &str) -> TaskGetResult {
    rpc(
        rig,
        "task.get",
        &TaskGetParams {
            board: board.to_string(),
            id: id.to_string(),
            checkout: None,
        },
    )
    .unwrap()
}

#[test]
fn a_tasks_feature_link_names_the_feature_and_the_proposal_sees_it_from_its_side() {
    let (rig, board) = board();

    let task = get(&rig, &board, &format!("plan:{IMPLEMENTER}"));
    let link = task
        .links
        .iter()
        .find(|link| link.link_type == "implements")
        .expect("the implements link");
    assert_eq!(link.dir, LinkDir::Out);
    assert_eq!(link.key, format!("plan:{PROPOSAL}"));
    assert_eq!(link.feature.as_deref(), Some("F1"));
    assert_eq!(
        (link.kind.as_str(), link.status.as_str()),
        ("proposal", "accepted")
    );
    assert!(!link.missing);

    let broken = get(&rig, &board, &format!("plan:{BROKEN}"));
    let missing = broken
        .links
        .iter()
        .find(|link| link.link_type == "implements")
        .unwrap();
    assert_eq!(missing.feature.as_deref(), Some("F9"));
    assert!(missing.missing, "the proposal has no F9");
    assert_eq!(
        missing.status, "accepted",
        "it still carries the proposal's state"
    );

    let proposal = get(&rig, &board, &format!("plan:{PROPOSAL}"));
    let incoming: Vec<(String, Option<String>)> = proposal
        .links
        .iter()
        .filter(|link| link.dir == LinkDir::In && link.link_type == "implements")
        .map(|link| (link.key.clone(), link.feature.clone()))
        .collect();
    assert_eq!(
        incoming,
        [
            (format!("plan:{IMPLEMENTER}"), Some("F1".to_string())),
            (format!("plan:{BROKEN}"), Some("F9".to_string())),
        ]
    );
}

#[test]
fn task_get_carries_the_link_types_of_the_plan_source_with_their_inverse() {
    let (rig, board) = board();
    let got = get(&rig, &board, &format!("plan:{IMPLEMENTER}"));
    let inverse: Vec<(&str, Option<&str>)> = got
        .link_types
        .iter()
        .map(|decl| (decl.name.as_str(), decl.inverse.as_deref()))
        .collect();
    assert_eq!(
        inverse,
        [
            ("blocked-by", Some("blocks")),
            ("part-of", Some("has part")),
            ("implements", Some("implemented by")),
            ("informs", Some("informed by")),
            ("includes", Some("included in")),
            ("supersedes", Some("superseded by")),
            ("raised-by", Some("raised")),
        ]
    );
}

#[test]
fn the_proposal_reports_its_coverage_in_the_list_and_in_task_get() {
    let (rig, board) = board();
    let from_get = get(&rig, &board, &format!("plan:{PROPOSAL}")).task.unwrap();
    let from_list = list_until(&rig, &board, "the list has the proposal", |list| {
        list.tasks.iter().any(|task| task.id == PROPOSAL)
    })
    .tasks
    .into_iter()
    .find(|task| task.id == PROPOSAL)
    .unwrap();
    for task in [from_get, from_list] {
        let coverage = task
            .coverage
            .expect("a proposal with features has coverage");
        assert_eq!(coverage.features_without_task, ["F2"]);
        assert_eq!(coverage.tasks_without_feature, [UNLINKED]);
    }
    let other = get(&rig, &board, &format!("plan:{IMPLEMENTER}"))
        .task
        .unwrap();
    assert!(other.coverage.is_none(), "only a proposal has coverage");
}
