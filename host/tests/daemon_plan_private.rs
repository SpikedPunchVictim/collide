//! The plan pack's private root through the daemon (ts-0125-uh; 0.10, Planning sources): the daemon gives the built-in
//! `collide/plan` provider its pack data folder, the provider keeps the private root in `planning/` there, and an item
//! in it lists, links and changes like a shared one while `git status` of the repository stays empty. The provider is
//! the real built-in pack from a built bundle, run by `node`.
//!
//! The pack data folder is below the test's own root under `/tmp`, which is also the home: never the real Application
//! Support folder. Nothing here touches the user's home, keychain or a real daemon.

mod common;

use collide_core::item::LinkDir;
use collide_core::owner::{
    TaskApplyParams, TaskApplyResult, TaskGetParams, TaskGetResult, TaskListResult,
};
use collide_core::provider::{ApplyParams, ApplyResult};
use collide_host::board_load::pack_data_dir;
use collide_host::task_cli::{exit, parse_task};
use collide_host::task_daemon::run_with;
use common::plan_rig::{EPIC, Rig, list_until, rig, rpc, task, trust};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::PathBuf;

const SHARED_TASK: &str = "ts-0001-aa";
const SHARED_FILE: &str = ".collide/planning/epics/ep-001-aa-alpha/tasks/ts-0001-aa-in-repo.md";

/// A trusted board over a repository whose shared root has one epic and one task. Returns the rig and the board id.
fn board() -> (Rig, String) {
    let rig = rig("daemon-plan-private");
    rig.write(".collide/planning/epics/ep-001-aa-alpha/epic.md", EPIC);
    rig.write(SHARED_FILE, &task(SHARED_TASK));
    rig.commit();
    let board = rig.board();
    trust(&rig, &board);
    (rig, board)
}

/// Where the daemon keeps the private root of the plan pack on `board`.
fn private_root(rig: &Rig, board: &str) -> PathBuf {
    let data_root = rig.daemon.paths().runs_dir.with_file_name("pack-data");
    pack_data_dir(&data_root, board, "collide/plan")
        .unwrap()
        .join("planning")
}

fn args(value: Value) -> BTreeMap<String, Value> {
    serde_json::from_value(value).unwrap()
}

/// One `task.apply` on the plan source; the provider's own answer.
fn apply(rig: &Rig, board: &str, apply: ApplyParams) -> ApplyResult {
    let answer: TaskApplyResult = rpc(
        rig,
        "task.apply",
        &TaskApplyParams {
            board: board.to_string(),
            source: Some("plan".to_string()),
            apply,
            commit: false,
        },
    )
    .unwrap();
    answer.result.expect("the write was applied, not queued")
}

fn create_private(
    rig: &Rig,
    board: &str,
    kind: &str,
    title: &str,
) -> (String, String, ApplyResult) {
    let result = apply(
        rig,
        board,
        ApplyParams {
            op: "create".to_string(),
            id: None,
            args: args(json!({
                "kind": kind, "epic": "ep-001-aa", "title": title, "root": "private", "seed": 1,
            })),
            version: None,
            checkout: None,
        },
    );
    let ApplyResult::Ok { task, version, .. } = &result else {
        panic!("the create was not applied: {result:?}")
    };
    (task.id.clone(), version.clone(), result)
}

fn write(rig: &Rig, board: &str, id: &str, version: &str, op: &str, with: Value) -> ApplyResult {
    apply(
        rig,
        board,
        ApplyParams {
            op: op.to_string(),
            id: Some(id.to_string()),
            args: args(with),
            version: Some(version.to_string()),
            checkout: None,
        },
    )
}

fn version_of(result: &ApplyResult) -> String {
    match result {
        ApplyResult::Ok { version, .. } => version.clone(),
        other => panic!("expected an ok write: {other:?}"),
    }
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

fn status(rig: &Rig) -> String {
    rig.repo.git(&["status", "--porcelain"])
}

#[test]
fn a_private_item_lists_with_its_epic_and_a_write_to_it_leaves_git_status_empty() {
    let (rig, board) = board();
    let (id, version, created) = create_private(&rig, &board, "proposal", "Private idea");
    let ApplyResult::Ok { task, files, .. } = &created else {
        unreachable!()
    };
    assert!(files.is_empty(), "{files:?}");
    assert!(task.path.as_deref().unwrap().starts_with("private:"));
    let folder = private_root(&rig, &board).join("epics/ep-001-aa/proposals");
    assert_eq!(std::fs::read_dir(folder).unwrap().count(), 1);

    let proposed = write(
        &rig,
        &board,
        &id,
        &version,
        "setStatus",
        json!({"status": "proposed"}),
    );
    let linked = write(
        &rig,
        &board,
        &id,
        &version_of(&proposed),
        "link",
        json!({"to": SHARED_TASK}),
    );
    let claimed = write(
        &rig,
        &board,
        &id,
        &version_of(&linked),
        "claim",
        json!({"claimant": "me"}),
    );
    for result in [&proposed, &linked, &claimed] {
        let ApplyResult::Ok { files, .. } = result else {
            panic!("not applied: {result:?}")
        };
        assert!(files.is_empty(), "{files:?}");
    }
    assert_eq!(status(&rig), "");

    let key = format!("plan:{id}");
    let list: TaskListResult =
        list_until(&rig, &board, "the private item lists as claimed", |list| {
            !list.refreshing
                && list
                    .tasks
                    .iter()
                    .any(|task| task.key == key && task.owners == ["me"])
        });
    let listed = list.tasks.iter().find(|task| task.key == key).unwrap();
    assert_eq!(listed.epic(), Some("ep-001-aa"));
    assert_eq!(listed.fields.get("root"), Some(&json!("private")));
    assert!(listed.path.as_deref().unwrap().starts_with("private:"));
    assert_eq!(listed.state.status, "proposed");
}

#[test]
fn a_private_item_and_a_shared_one_that_link_to_each_other_show_both_links_in_the_corpus() {
    let (rig, board) = board();
    let (id, version, _) = create_private(&rig, &board, "task", "Private task");
    write(
        &rig,
        &board,
        &id,
        &version,
        "link",
        json!({"to": SHARED_TASK}),
    );
    let shared_key = format!("plan:{SHARED_TASK}");
    let shared_version = get(&rig, &board, &shared_key).task.unwrap().version;
    let back = write(
        &rig,
        &board,
        &shared_key,
        &shared_version,
        "link",
        json!({"to": id}),
    );
    let ApplyResult::Ok { files, .. } = &back else {
        panic!("not applied: {back:?}")
    };
    assert_eq!(files, &[SHARED_FILE.to_string()]);
    // `git_text` trims, so the leading space of the porcelain code is gone.
    assert_eq!(status(&rig), format!("M {SHARED_FILE}"));

    let private_key = format!("plan:{id}");
    list_until(&rig, &board, "both links are in the corpus", |list| {
        let has = |key: &str, other: &str| {
            let links = get(&rig, &board, key).links;
            [LinkDir::Out, LinkDir::In].iter().all(|dir| {
                links.iter().any(|link| {
                    link.link_type == "blocked-by" && link.dir == *dir && link.key == other
                })
            })
        };
        !list.refreshing && has(&private_key, &shared_key) && has(&shared_key, &private_key)
    });
}

#[test]
fn commit_on_a_write_to_a_private_item_is_refused_and_changes_nothing_in_the_repository() {
    let (rig, board) = board();
    let (id, ..) = create_private(&rig, &board, "task", "Private task");
    let head = rig.repo.git(&["rev-parse", "HEAD"]);
    let words: Vec<String> = [&format!("plan:{id}"), "claim", "claimant=me", "--commit"]
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
    let err = String::from_utf8(err).unwrap();
    assert_eq!(code, exit::FAILED, "{err}");
    assert!(err.contains("--commit refused"), "{err}");
    assert!(err.contains("private root"), "{err}");
    assert_eq!(rig.repo.git(&["rev-parse", "HEAD"]), head);
    assert_eq!(status(&rig), "");
}
