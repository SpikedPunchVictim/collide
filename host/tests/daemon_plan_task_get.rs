//! `task.get` for the Field's drawer through the daemon (ts-0180-ad; 0.11, Task drawer): the names of the board's views
//! that hold the task, and the title of each linked task. The provider is the real built-in `collide/plan` pack from a
//! built bundle, run by `node`, over a temporary repository.
//!
//! Every path is below the test's own root under `/tmp`, which is also the home. Nothing here touches the user's
//! home, keychain or a real daemon.

mod common;

use collide_core::owner::{TaskGetParams, TaskGetResult};
use common::plan_rig::{EPIC, Rig, list_until, rig, rpc, trust};

const PLANNING: &str = ".collide/planning/epics/ep-001-aa-alpha";

fn doc(kind: &str, id: &str, status: &str, title: &str, links: &str) -> String {
    format!(
        "---\nid: {id}\nkind: {kind}\nstatus: {status}\ntitle: \"{title}\"\nowners: []\n{links}---\n\n## Goal\n"
    )
}

/// A trusted board: a ready task and a spike (both in `ready`; the spike also in the plan pack's `spikes`), a task
/// that is blocked by the first and by an id that names nothing, and a repo view that holds the spike too.
fn board() -> (Rig, String) {
    let rig = rig("daemon-plan-task-get");
    rig.write(&format!("{PLANNING}/epic.md"), EPIC);
    let blocked = "links:\n  - { type: blocked-by, to: ts-0001-aa }\n  - { type: blocked-by, to: ts-9999-zz }\n";
    for (path, text) in [
        (
            format!("{PLANNING}/tasks/ts-0001-aa-work.md"),
            doc("task", "ts-0001-aa", "ready", "Lay the keel", "links: []\n"),
        ),
        (
            format!("{PLANNING}/tasks/ts-0002-bb-work.md"),
            doc("task", "ts-0002-bb", "ready", "Raise the mast", blocked),
        ),
        (
            ".collide/planning/spikes/sp-0001-aa-probe/record.md".to_string(),
            doc("spike", "sp-0001-aa", "planned", "Probe the tide", ""),
        ),
        (
            ".collide/views/mine.yaml".to_string(),
            "name: mine\ninclude: [plan:sp-0001-aa]\n".to_string(),
        ),
    ] {
        rig.write(&path, &text);
    }
    rig.commit();
    let board = rig.board();
    trust(&rig, &board);
    list_until(&rig, &board, "the plan source lists", |list| {
        !list.refreshing && list.tasks.len() == 4
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
fn a_plan_task_held_by_two_views_lists_both_and_not_the_ones_that_do_not_hold_it() {
    let (rig, board) = board();

    let spike = get(&rig, &board, "plan:sp-0001-aa");

    assert_eq!(spike.views, ["ready", "mine", "collide/plan/spikes"]);
}

#[test]
fn a_task_no_view_holds_has_no_views() {
    let (rig, board) = board();

    // It waits on `ts-0001-aa`, so `ready` does not hold it, and no other view names it.
    let blocked = get(&rig, &board, "plan:ts-0002-bb");

    assert!(blocked.views.is_empty(), "{:?}", blocked.views);
}

#[test]
fn a_link_carries_the_other_tasks_title_and_a_missing_one_carries_none() {
    let (rig, board) = board();

    let task = get(&rig, &board, "plan:ts-0002-bb");

    let titled = task
        .links
        .iter()
        .find(|link| link.key == "plan:ts-0001-aa" && link.dir == collide_core::item::LinkDir::Out)
        .expect("the link to the keel");
    assert_eq!(titled.title.as_deref(), Some("Lay the keel"));
    assert!(!titled.missing);
    let missing = task
        .links
        .iter()
        .find(|link| link.missing)
        .expect("the link to a task that is not there");
    assert_eq!(missing.title, None);
    // The keel sees the same link from its side, and names the task that waits on it.
    let keel = get(&rig, &board, "plan:ts-0001-aa");
    let back = keel
        .links
        .iter()
        .find(|link| link.key == "plan:ts-0002-bb")
        .expect("the link seen from the other side");
    assert_eq!(back.title.as_deref(), Some("Raise the mast"));
}
