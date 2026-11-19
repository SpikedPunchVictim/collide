//! `task.create`, `task.reid` and `task.sections` through the daemon (ts-0136-rr; 0.10, Planning sources and Identity
//! and `reid`): the plan provider's `create` and `reid` as typed requests with the daemon's write guard, and the
//! sections of the proposals' Features parts over the corpus. The provider is the real built-in `collide/plan` pack
//! from a built bundle, run by `node`.
//!
//! Every path is below the test's own root under `/tmp`, which is also the home. Nothing here touches the user's
//! home, keychain or a real daemon.

mod common;

use collide_core::owner::{
    TaskApplyResult, TaskCreateParams, TaskListParams, TaskListResult, TaskReidParams,
    TaskReidResult, TaskSectionsParams, code,
};
use collide_core::provider::ApplyResult;
use collide_core::section::Sections;
use common::plan_rig::{EPIC, Rig, list_until, rig, rig_with, rpc, task, trust};
use common::wait_until;
use std::time::Duration;

const EPIC_DIR: &str = ".collide/planning/epics/ep-001-aa-alpha";
const DEADLINE: Duration = Duration::from_secs(30);

/// A trusted board over a repository with one epic, a task `ts-0001-aa` and a task `ts-0002-bb` that is blocked by
/// it. Returns the rig and the board id.
fn board(name: &str) -> (Rig, String) {
    let rig = rig(name);
    seed(&rig);
    let board = rig.board();
    trust(&rig, &board);
    (rig, board)
}

fn seed(rig: &Rig) {
    rig.write(&format!("{EPIC_DIR}/epic.md"), EPIC);
    rig.write(
        &format!("{EPIC_DIR}/tasks/ts-0001-aa-first.md"),
        &task("ts-0001-aa"),
    );
    rig.write(
        &format!("{EPIC_DIR}/tasks/ts-0002-bb-second.md"),
        "---\nid: ts-0002-bb\nkind: task\nstatus: ready\ntitle: \"Second\"\nowners: []\n\
         links:\n  - { type: blocked-by, to: ts-0001-aa }\n---\n\n## Goal\n",
    );
    rig.commit();
}

fn create(title: &str, epic: Option<&str>, board: &str) -> TaskCreateParams {
    TaskCreateParams {
        board: board.to_string(),
        title: title.to_string(),
        kind: None,
        epic: epic.map(str::to_string),
        seed: Some(1),
        root: None,
        checkout: None,
    }
}

fn reid(id: &str, board: &str) -> TaskReidParams {
    TaskReidParams {
        board: board.to_string(),
        id: id.to_string(),
        seed: Some(1),
        checkout: None,
    }
}

fn sections(rig: &Rig, board: &str) -> Sections {
    // The first listing of the board has to be done before its features are counted.
    list_until(rig, board, "the plan source lists", |list| {
        !list.refreshing && !list.tasks.is_empty()
    });
    rpc(
        rig,
        "task.sections",
        &TaskSectionsParams {
            board: board.to_string(),
        },
    )
    .unwrap()
}

fn proposal(id: &str, status: &str, features: &str) -> String {
    format!(
        "---\nid: {id}\nkind: proposal\nstatus: {status}\ntitle: \"Proposal {id}\"\nowners: []\nlinks: []\n---\n\n\
         ## Features\n\n{features}"
    )
}

// ---- task.create ---------------------------------------------------------------------------------------------------

#[test]
fn task_create_makes_the_item_in_the_epics_folder_and_answers_with_the_applied_result() {
    let (rig, board) = board("daemon-plan-write");

    let answer: TaskApplyResult = rpc(
        &rig,
        "task.create",
        &create("Made by create", Some("ep-001-aa"), &board),
    )
    .unwrap();

    let Some(ApplyResult::Ok { task, files, .. }) = answer.result else {
        panic!("the create was applied: {answer:?}")
    };
    assert_eq!(task.id, "ts-0003-ra");
    assert_eq!(
        files,
        [format!("{EPIC_DIR}/tasks/ts-0003-ra-made-by-create.md")]
    );
    assert!(rig.repo.work().join(&files[0]).exists());
}

#[test]
fn task_create_of_a_task_with_no_epic_is_refused_with_the_providers_message_and_writes_nothing() {
    let (rig, board) = board("daemon-plan-write");

    let answer: TaskApplyResult =
        rpc(&rig, "task.create", &create("No epic", None, &board)).unwrap();

    let Some(ApplyResult::Refused { message }) = answer.result else {
        panic!("the create was refused: {answer:?}")
    };
    assert!(message.contains("a task needs epic"), "{message}");
    assert_eq!(rig.repo.git(&["status", "--porcelain"]), "");
}

#[test]
fn task_create_for_a_board_the_daemon_does_not_have_is_an_error() {
    let (rig, _) = board("daemon-plan-write");
    let error = rpc::<TaskApplyResult>(
        &rig,
        "task.create",
        &create("x", Some("ep-001-aa"), "no-such-board"),
    )
    .unwrap_err();
    assert!(!error.message.is_empty());
}

#[test]
fn task_create_in_a_busy_checkout_is_queued_with_the_reason_and_applies_when_the_phase_ends() {
    let (rig, board) = board("daemon-plan-write");
    let live = rig.daemon.live_phases();
    live.started("R-1", &rig.repo.work().canonicalize().unwrap(), false);

    let answer: TaskApplyResult = rpc(
        &rig,
        "task.create",
        &create("Queued", Some("ep-001-aa"), &board),
    )
    .unwrap();

    let queued = answer.queued.expect("the create was queued");
    assert!(
        queued.reason.contains("a phase of run R-1 is running"),
        "{}",
        queued.reason
    );
    assert!(answer.result.is_none());
    let file = rig
        .repo
        .work()
        .join(EPIC_DIR)
        .join("tasks/ts-0003-ra-queued.md");
    assert!(!file.exists());
    live.ended("R-1");
    wait_until("the queued create is applied", DEADLINE, || file.exists());
}

// ---- task.reid -----------------------------------------------------------------------------------------------------

#[test]
fn task_reid_renames_the_item_and_names_each_other_item_whose_links_it_rewrote() {
    let (rig, board) = board("daemon-plan-write");

    let answer: TaskReidResult = rpc(&rig, "task.reid", &reid("plan:ts-0001-aa", &board)).unwrap();

    assert_eq!(answer.from, "plan:ts-0001-aa");
    assert_eq!(answer.rewritten, ["plan:ts-0002-bb"]);
    let Some(ApplyResult::Ok { task, files, .. }) = answer.result else {
        panic!("the reid was applied: {answer:?}")
    };
    assert_eq!(task.id, "ts-0001-ra");
    assert!(
        files
            .iter()
            .any(|file| file.ends_with("ts-0001-ra-first.md")),
        "{files:?}"
    );
    let second = std::fs::read_to_string(
        rig.repo
            .work()
            .join(EPIC_DIR)
            .join("tasks/ts-0002-bb-second.md"),
    )
    .unwrap();
    assert!(second.contains("to: ts-0001-ra"), "{second}");
    assert!(!second.contains("ts-0001-aa"), "{second}");
}

#[test]
fn task_reid_takes_a_bare_id_as_an_id_of_the_plan_source() {
    let (rig, board) = board("daemon-plan-write");
    let answer: TaskReidResult = rpc(&rig, "task.reid", &reid("ts-0001-aa", &board)).unwrap();
    assert_eq!(answer.from, "plan:ts-0001-aa");
    assert!(
        matches!(answer.result, Some(ApplyResult::Ok { .. })),
        "{answer:?}"
    );
}

#[test]
fn task_reid_of_an_id_nobody_has_is_refused_and_rewrites_nothing() {
    let (rig, board) = board("daemon-plan-write");

    let answer: TaskReidResult = rpc(&rig, "task.reid", &reid("plan:ts-0099-zz", &board)).unwrap();

    assert!(answer.rewritten.is_empty());
    assert!(
        matches!(answer.result, Some(ApplyResult::Refused { .. })),
        "{answer:?}"
    );
    assert_eq!(rig.repo.git(&["status", "--porcelain"]), "");
}

#[test]
fn task_reid_in_a_busy_checkout_is_queued_and_renames_nothing_until_the_phase_ends() {
    let (rig, board) = board("daemon-plan-write");
    let live = rig.daemon.live_phases();
    live.started("R-2", &rig.repo.work().canonicalize().unwrap(), false);

    let answer: TaskReidResult = rpc(&rig, "task.reid", &reid("plan:ts-0001-aa", &board)).unwrap();

    assert!(answer.queued.is_some(), "{answer:?}");
    assert!(answer.result.is_none());
    let first = rig
        .repo
        .work()
        .join(EPIC_DIR)
        .join("tasks/ts-0001-aa-first.md");
    assert!(first.exists());
    live.ended("R-2");
    wait_until("the queued reid is applied", DEADLINE, || !first.exists());
}

// ---- task.sections -------------------------------------------------------------------------------------------------

#[test]
fn task_sections_counts_the_features_of_the_proposals_that_are_not_dropped() {
    let (rig, board) = board("daemon-plan-write");
    rig.write(
        &format!("{EPIC_DIR}/proposals/pr-0003-km-a.md"),
        &proposal(
            "pr-0003-km",
            "draft",
            "### Pack\n- **F1 One**: a\n### Other\n- **F2 Two**: b\n",
        ),
    );
    rig.write(
        &format!("{EPIC_DIR}/proposals/pr-0004-lm-b.md"),
        &proposal("pr-0004-lm", "accepted", "### pack\n- **F1 Three**: c\n"),
    );
    rig.write(
        &format!("{EPIC_DIR}/proposals/pr-0005-mn-c.md"),
        &proposal("pr-0005-mn", "rejected", "### Gone\n- **F1 Four**: d\n"),
    );
    rig.commit();

    let found = wait_for_sections(&rig, &board, |found| found.sections.len() == 2);

    let counts: Vec<(&str, u32, u32)> = found
        .sections
        .iter()
        .map(|section| {
            (
                section.name.as_str(),
                section.feature_count,
                section.proposal_count,
            )
        })
        .collect();
    assert_eq!(counts, [("Pack", 2, 2), ("Other", 1, 1)]);
}

#[test]
fn task_sections_of_a_board_with_no_proposal_has_no_section() {
    let (rig, board) = board("daemon-plan-write");
    let found = sections(&rig, &board);
    assert!(found.sections.is_empty(), "{found:?}");
    assert!(found.near_duplicates.is_empty());
}

#[test]
fn task_sections_follows_the_layout_the_repo_sets() {
    let rig = rig_with("daemon-plan-write", |_| {});
    rig.write(
        ".collide/config.yaml",
        "config:\n  collide/plan:\n    layout:\n      root: docs/plan\n",
    );
    rig.write("docs/plan/epics/ep-001-aa-alpha/epic.md", EPIC);
    rig.write(
        "docs/plan/epics/ep-001-aa-alpha/proposals/pr-0003-km-a.md",
        &proposal("pr-0003-km", "draft", "### Docs\n- **F1 One**: a\n"),
    );
    rig.commit();
    let board = rig.board();
    trust(&rig, &board);

    let found = wait_for_sections(&rig, &board, |found| !found.sections.is_empty());

    assert_eq!(found.sections[0].name, "Docs");
}

/// `task.sections` until `done` holds: the corpus takes a moment to see files written after the daemon started.
fn wait_for_sections(rig: &Rig, board: &str, done: impl Fn(&Sections) -> bool) -> Sections {
    let mut last = None;
    wait_until("the sections are counted", DEADLINE, || {
        let found = sections(rig, board);
        let ok = done(&found);
        last = Some(found);
        ok
    });
    last.unwrap()
}

// ---- task.list with a kind -----------------------------------------------------------------------------------------

fn list_of_kind(kind: &str, board: &str) -> TaskListParams {
    TaskListParams {
        board: board.to_string(),
        view: None,
        ready: false,
        kind: Some(kind.to_string()),
        checkout: None,
        limit: None,
        cursor: None,
    }
}

#[test]
fn task_list_with_a_kind_no_source_declares_is_refused_and_names_the_kinds_the_board_declares() {
    let (rig, board) = board("daemon-plan-write");
    list_until(&rig, &board, "the plan source lists", |list| {
        !list.refreshing && !list.tasks.is_empty()
    });

    let error =
        rpc::<TaskListResult>(&rig, "task.list", &list_of_kind("widget", &board)).unwrap_err();

    assert_eq!(error.code, code::INVALID_PARAMS);
    assert_eq!(
        error.message,
        "unknown kind `widget`; the board's sources declare: task, epic, proposal, spike, question"
    );
}

#[test]
fn task_list_with_a_kind_a_source_declares_lists_only_that_kind() {
    let (rig, board) = board("daemon-plan-write");
    list_until(&rig, &board, "the plan source lists", |list| {
        !list.refreshing && !list.tasks.is_empty()
    });

    let epics: TaskListResult = rpc(&rig, "task.list", &list_of_kind("epic", &board)).unwrap();

    assert_eq!(epics.tasks.len(), 1);
    assert_eq!(epics.tasks[0].id, "ep-001-aa");
}
