//! An id taken by a second file, with the daemon running (ts-0161-ta; 0.10, Identity and Corpus): a write the daemon
//! made, and a file that then takes the item's id, leave the corpus as a full listing does: one row for the key (the
//! first file in scan order) and the other file set aside with its reason. A write that ends the clash clears the set-
//! aside entry at once. The provider is the real built-in `collide/plan` pack from a built bundle, run by `node`,
//! which names no checkout on the tasks it answers a write with.
//!
//! Every path is below the test's own root under `/tmp`, which is also the home. Nothing here touches the user's
//! home, keychain or a real daemon.

mod common;

use collide_core::owner::{
    TaskApplyResult, TaskCreateParams, TaskListResult, TaskReidParams, TaskReidResult,
};
use collide_core::provider::{ApplyResult, Task};
use collide_host::task_cli::{exit, parse_task};
use collide_host::task_daemon::run_with;
use common::plan_rig::{EPIC, Rig, list_until, rig_with, rpc, task, trust};
use serde_json::json;
use std::time::Duration;

const EPIC_DIR: &str = ".collide/planning/epics/ep-001-aa-alpha";
/// The daemon's look at its sources: short for a test that waits for one, too long for a test that must not see one.
const SHORT: Duration = Duration::from_millis(300);
const LONG: Duration = Duration::from_secs(60);

fn board_over(recheck: Duration, seed: impl FnOnce(&Rig)) -> (Rig, String) {
    let rig = rig_with("daemon-plan-clash", move |options| {
        options.corpus_recheck = recheck;
    });
    rig.write(&format!("{EPIC_DIR}/epic.md"), EPIC);
    rig.write(
        &format!("{EPIC_DIR}/tasks/ts-0001-aa-first.md"),
        &task("ts-0001-aa"),
    );
    seed(&rig);
    rig.commit();
    let board = rig.board();
    trust(&rig, &board);
    (rig, board)
}

fn rows_of<'a>(list: &'a TaskListResult, key: &str) -> Vec<&'a Task> {
    list.tasks.iter().filter(|task| task.key == key).collect()
}

fn list(rig: &Rig, board: &str) -> TaskListResult {
    rpc(rig, "task.list", &json!({ "board": board })).unwrap()
}

/// A board whose daemon made one item (`ts-0002-ra`, file `ts-0002-ra-probe.md`) with `task.create`, and then saw a
/// copy of that file with the same id, named so that it sorts first, so that the item the daemon wrote is the one set
/// aside. Returns once a listing says so.
fn clash_after_a_write() -> (Rig, String) {
    let (rig, board) = board_over(SHORT, |_| {});
    list_until(&rig, &board, "the first listing", |list| {
        !list.refreshing && list.tasks.len() == 2
    });
    let made: TaskApplyResult = rpc(
        &rig,
        "task.create",
        &TaskCreateParams {
            board: board.clone(),
            title: "Probe".to_string(),
            kind: None,
            epic: Some("ep-001-aa".to_string()),
            seed: Some(1),
            root: None,
            args: Default::default(),
            checkout: None,
        },
    )
    .unwrap();
    let Some(ApplyResult::Ok { task: made, .. }) = made.result else {
        panic!("the create was applied");
    };
    assert_eq!(made.id, "ts-0002-ra");
    rig.write(
        &format!("{EPIC_DIR}/tasks/ts-0002-ra-acopy.md"),
        &task("ts-0002-ra"),
    );
    rig.commit();
    list_until(&rig, &board, "the copy is set aside", |list| {
        !list.invalid.is_empty()
    });
    (rig, board)
}

#[test]
fn a_file_that_takes_the_id_of_an_item_the_daemon_wrote_leaves_one_row_and_one_skipped_entry() {
    let (rig, board) = clash_after_a_write();

    let listed = list(&rig, &board);

    let rows = rows_of(&listed, "plan:ts-0002-ra");
    assert_eq!(rows.len(), 1, "one row for the key: {rows:?}");
    assert_eq!(
        rows[0].path.as_deref(),
        Some(".collide/planning/epics/ep-001-aa-alpha/tasks/ts-0002-ra-acopy.md"),
        "the first file in scan order keeps the id"
    );
    assert_eq!(listed.invalid.len(), 1, "{:?}", listed.invalid);
    assert_eq!(
        listed.invalid[0].path,
        ".collide/planning/epics/ep-001-aa-alpha/tasks/ts-0002-ra-probe.md"
    );
    assert!(
        listed.invalid[0].reason.contains(
            "is already used by .collide/planning/epics/ep-001-aa-alpha/tasks/ts-0002-ra-acopy.md"
        ),
        "{}",
        listed.invalid[0].reason
    );
}

#[test]
fn a_bare_id_reid_of_the_clash_works_with_the_daemon_running() {
    let (rig, _board) = clash_after_a_write();
    let words: Vec<String> = ["reid", "ts-0002-ra"]
        .iter()
        .map(|word| word.to_string())
        .collect();
    let command = parse_task(&words).unwrap();
    let (mut out, mut err) = (Vec::new(), Vec::new());

    let code = run_with(
        rig.daemon.paths(),
        &command,
        rig.repo.work(),
        &mut out,
        &mut err,
    );

    let (out, err) = (
        String::from_utf8(out).unwrap(),
        String::from_utf8(err).unwrap(),
    );
    assert_eq!(code, exit::OK, "{out}\n{err}");
}

#[test]
fn the_skipped_entry_is_gone_from_the_next_listing_once_a_reid_ends_the_clash() {
    // The look at the sources is a minute away, so only the write can have cleared the entry.
    let (rig, board) = board_over(LONG, |rig| {
        rig.write(
            &format!("{EPIC_DIR}/tasks/ts-0001-aa-zcopy.md"),
            &task("ts-0001-aa"),
        );
    });
    let first = list_until(&rig, &board, "the first listing", |list| {
        !list.refreshing && !list.invalid.is_empty()
    });
    assert_eq!(
        first.invalid.len(),
        1,
        "the later file is set aside: {:?}",
        first.invalid
    );

    let reid: TaskReidResult = rpc(
        &rig,
        "task.reid",
        &TaskReidParams {
            board: board.clone(),
            id: "ts-0001-aa".to_string(),
            seed: Some(1),
            checkout: None,
        },
    )
    .unwrap();
    let Some(ApplyResult::Ok { task: renamed, .. }) = reid.result else {
        panic!("the reid was applied: {reid:?}");
    };

    let listed = list(&rig, &board);
    assert!(
        listed.invalid.is_empty(),
        "no skipped entry is left: {:?}",
        listed.invalid
    );
    assert_eq!(rows_of(&listed, "plan:ts-0001-aa").len(), 1);
    let rows = rows_of(&listed, &renamed.key);
    assert_eq!(
        rows.len(),
        1,
        "the renamed item has its row: {:?}",
        listed.tasks
    );
    let kept = rows_of(&listed, "plan:ts-0001-aa")[0];
    assert_eq!(
        rows[0].checkout, kept.checkout,
        "both are copies in the checkout the listing names"
    );
}
