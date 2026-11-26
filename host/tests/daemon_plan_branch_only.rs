//! `branch_only` on `task.list` (ts-0195-ju; Stage 6a, Planning): a proposal that is only on a branch, checked out as a
//! worktree, is named with its checkout and its branch, and a task at the default line's root is not. The provider is
//! the real built-in `collide/plan` pack from a built bundle, run by `node`, over a temporary repository.
//!
//! Every path is below the test's own root under `/tmp`, which is also the home. Nothing here touches the user's
//! home, keychain or a real daemon.

mod common;

use collide_core::owner::TaskBranchOnly;
use common::git_repo::git_text;
use common::plan_rig::{EPIC, list_until, rig, task, trust};
use std::fs;

const BRANCH: &str = "plan/ui-first";
const PROPOSAL: &str = "pr-0001-aa";
const PROPOSAL_FILE: &str = ".collide/planning/epics/ep-001-aa-alpha/proposals/pr-0001-aa-idea.md";
const BODY: &str = "---\nid: pr-0001-aa\nkind: proposal\nstatus: draft\ntitle: \"Made-up proposal\"\nowners: []\n\
                    links: []\n---\n\n## Features\n\n- **F1 First**: one.\n";

#[test]
fn a_proposal_only_on_a_branch_worktree_is_branch_only_with_its_branch() {
    let rig = rig("daemon-plan-branch-only");
    rig.write(".collide/planning/epics/ep-001-aa-alpha/epic.md", EPIC);
    rig.write(
        ".collide/planning/epics/ep-001-aa-alpha/tasks/ts-0001-aa-work.md",
        &task("ts-0001-aa"),
    );
    rig.commit();
    let home = rig.home().to_path_buf();
    let worktree = home.join("wt-plan");
    rig.repo.git(&[
        "worktree",
        "add",
        "-q",
        "-b",
        BRANCH,
        worktree.to_str().unwrap(),
    ]);
    let file = worktree.join(PROPOSAL_FILE);
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(&file, BODY).unwrap();
    git_text(&home, &worktree, &["add", "--all"]);
    git_text(
        &home,
        &worktree,
        &["commit", "--quiet", "-m", "the proposal"],
    );
    let board = rig.board();
    trust(&rig, &board);

    let list = list_until(&rig, &board, "the proposal is listed", |list| {
        !list.refreshing
            && list
                .tasks
                .iter()
                .any(|task| task.key == format!("plan:{PROPOSAL}"))
    });

    let checkout = fs::canonicalize(&worktree).unwrap().display().to_string();
    assert_eq!(
        list.branch_only,
        [TaskBranchOnly {
            key: format!("plan:{PROPOSAL}"),
            checkout,
            branch: Some(BRANCH.to_string()),
        }],
        "the epic and the task are at the root, so only the proposal is branch only"
    );
}
