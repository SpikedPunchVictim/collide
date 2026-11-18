//! The plan pack's `layout` setting through the daemon (ts-0124-xm; 0.10, Planning sources): a repository whose
//! committed `.collide/config.yaml` sets `layout.root` is listed from that folder by `task.list`, and a layout the
//! provider refuses fails the source's start with the reason. The provider is the real built-in `collide/plan` pack
//! from a built bundle, run by `node`.
//!
//! Every path is below the test's own root under `/tmp`, which is also the home. The daemon's captured environment is
//! made by a login shell that clears the environment, so the provider gets only `HOME` and `COLLIDE_DAEMON_ROOT` of
//! that root and a `PATH`. Nothing here touches the user's home, keychain or a real daemon.

mod common;

use collide_core::owner::{TaskGetParams, TaskGetResult};
use common::config_fixture::Fixture;
use common::plan_rig::{EPIC, keys, list_until, rig, rpc, task, trust};

/// A trusted board over a repository with `config` as its `.collide/config.yaml`, items at `docs/plan`, and one item
/// left at the default root.
fn board_with(fixture: &Fixture, config: &str) -> String {
    fixture.write(".collide/config.yaml", config);
    fixture.write("docs/plan/epics/ep-001-aa-alpha/epic.md", EPIC);
    for id in ["ts-0001-aa", "ts-0002-bb"] {
        fixture.write(
            &format!("docs/plan/epics/ep-001-aa-alpha/tasks/{id}-in-docs.md"),
            &task(id),
        );
    }
    fixture.write(
        ".collide/planning/epics/ep-009-zz-left/tasks/ts-0090-zz-left.md",
        &task("ts-0090-zz"),
    );
    fixture.commit();
    let board = fixture.board();
    trust(fixture, &board);
    board
}

#[test]
fn task_list_shows_the_items_of_the_root_the_repos_layout_names() {
    let fixture = rig("daemon-plan-layout");
    let board = board_with(
        &fixture,
        "config:\n  collide/plan:\n    layout:\n      root: docs/plan\n",
    );
    let list = list_until(&fixture, &board, "the plan source lists", |list| {
        !list.refreshing && !list.tasks.is_empty()
    });
    assert_eq!(
        keys(&list),
        ["plan:ep-001-aa", "plan:ts-0001-aa", "plan:ts-0002-bb"]
    );
    assert!(list.stale_sources.is_empty(), "{:?}", list.stale_sources);
}

#[test]
fn a_layout_the_provider_refuses_is_an_error_on_the_source_naming_the_rule_and_the_file() {
    let fixture = rig("daemon-plan-layout");
    let board = board_with(
        &fixture,
        "config:\n  collide/plan:\n    layout:\n      root: .collide/views\n",
    );
    let error = rpc::<TaskGetResult>(
        &fixture,
        "task.get",
        &TaskGetParams {
            board,
            id: "plan:ts-0001-aa".to_string(),
            checkout: None,
        },
    )
    .unwrap_err();
    assert!(error.message.contains("layout.root"), "{}", error.message);
    assert!(
        error.message.contains("would cover .collide/views"),
        "{}",
        error.message
    );
}
