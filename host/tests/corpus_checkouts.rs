//! The checkouts of a board for the corpus (ts-0116-wh; 0.10, Ref-scoped sources): the entries of `git worktree list`
//! that are folders, and the board's `checkout_filter` over them. Every repository is a temp one, and git runs with
//! an empty global and system config.

mod common;

use collide_host::boards::{Worktree, checkout_excluded, git_dir_of, worktrees};
use common::git_repo::GitRepo;
use std::path::PathBuf;

fn env() -> Vec<(String, String)> {
    vec![
        ("GIT_CONFIG_GLOBAL".to_string(), "/dev/null".to_string()),
        ("GIT_CONFIG_SYSTEM".to_string(), "/dev/null".to_string()),
    ]
}

fn repo_with_commit() -> GitRepo {
    let repo = GitRepo::new();
    repo.write("README.md", b"hello\n");
    repo.commit_all("first");
    repo
}

fn canonical(path: &std::path::Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap()
}

#[test]
fn a_repository_with_no_other_worktree_lists_its_root_and_its_branch() {
    let repo = repo_with_commit();
    let found = worktrees(repo.work(), &env()).unwrap();
    assert_eq!(
        found,
        [Worktree {
            path: canonical(repo.work()),
            branch: Some("main".to_string())
        }]
    );
}

#[test]
fn an_added_worktree_is_listed_with_its_branch_and_a_detached_one_with_none() {
    let repo = repo_with_commit();
    let linked = repo.root().join("linked");
    let detached = repo.root().join("detached");
    repo.git(&[
        "worktree",
        "add",
        "-q",
        "-b",
        "feature/x",
        linked.to_str().unwrap(),
    ]);
    repo.git(&[
        "worktree",
        "add",
        "-q",
        "--detach",
        detached.to_str().unwrap(),
    ]);
    let mut found = worktrees(repo.work(), &env()).unwrap();
    found.sort_by(|a, b| a.path.cmp(&b.path));
    let mut want = vec![
        Worktree {
            path: canonical(repo.work()),
            branch: Some("main".to_string()),
        },
        Worktree {
            path: canonical(&linked),
            branch: Some("feature/x".to_string()),
        },
        Worktree {
            path: canonical(&detached),
            branch: None,
        },
    ];
    want.sort_by(|a, b| a.path.cmp(&b.path));
    assert_eq!(found, want);
}

#[test]
fn a_worktree_whose_folder_is_gone_is_left_out() {
    let repo = repo_with_commit();
    let linked = repo.root().join("linked");
    repo.git(&[
        "worktree",
        "add",
        "-q",
        "-b",
        "gone",
        linked.to_str().unwrap(),
    ]);
    std::fs::remove_dir_all(&linked).unwrap();
    let found = worktrees(repo.work(), &env()).unwrap();
    assert_eq!(found.len(), 1);
}

#[test]
fn a_folder_that_is_no_repository_is_an_error_with_a_message() {
    let root = common::TempRoot::new();
    let message = worktrees(root.path(), &env()).unwrap_err();
    assert!(message.contains("worktree"), "{message}");
}

/// A folder that was there and is gone: the temp root's `deleted` folder, never made.
fn gone_folder(root: &common::TempRoot) -> PathBuf {
    root.join("deleted")
}

fn assert_says_the_folder_is_gone(message: &str, folder: &std::path::Path) {
    assert!(message.contains(&folder.display().to_string()), "{message}");
    assert!(message.contains("gone"), "{message}");
    assert!(!message.contains("install git"), "{message}");
}

#[test]
fn git_in_a_folder_that_is_gone_says_so_and_does_not_say_to_install_git() {
    let root = common::TempRoot::new();
    let folder = gone_folder(&root);
    let message = git_dir_of(&folder, &env()).unwrap_err();
    assert_says_the_folder_is_gone(&message, &folder);
}

#[test]
fn the_worktree_list_of_a_folder_that_is_gone_says_so_and_does_not_say_to_install_git() {
    let root = common::TempRoot::new();
    let folder = gone_folder(&root);
    let message = worktrees(&folder, &env()).unwrap_err();
    assert_says_the_folder_is_gone(&message, &folder);
}

#[test]
fn git_that_is_not_on_the_path_still_says_to_install_git() {
    let root = common::TempRoot::new();
    let no_tools = root.join("no-tools");
    std::fs::create_dir(&no_tools).unwrap();
    // The path goes in the environment the helper gives its own `Command`, not in this process's.
    let env = vec![("PATH".to_string(), no_tools.display().to_string())];
    let message = git_dir_of(root.path(), &env).unwrap_err();
    assert!(message.contains("install git"), "{message}");
}

fn tree(path: &str, branch: Option<&str>) -> Worktree {
    Worktree {
        path: PathBuf::from(path),
        branch: branch.map(str::to_string),
    }
}

#[test]
fn a_glob_is_matched_against_the_branch_and_against_the_path() {
    let globs = |list: &[&str]| list.iter().map(|g| g.to_string()).collect::<Vec<_>>();
    let scratch = tree("/work/scratch-1", Some("scratch/one"));
    let other = tree("/work/other", Some("feature/y"));
    assert!(checkout_excluded(&globs(&["scratch/*"]), &scratch));
    assert!(!checkout_excluded(&globs(&["scratch/*"]), &other));
    assert!(checkout_excluded(&globs(&["/work/oth?r"]), &other));
    assert!(checkout_excluded(&globs(&["*/scratch-*"]), &scratch));
    assert!(!checkout_excluded(&[], &scratch));
}

#[test]
fn a_detached_worktree_is_matched_by_its_path_alone() {
    let detached = tree("/work/d", None);
    assert!(checkout_excluded(&["/work/*".to_string()], &detached));
    assert!(!checkout_excluded(&["feature/*".to_string()], &detached));
}
