//! The line of a checkout while git has `HEAD` detached for a rebase or a bisect (ts-0146-se; 0.5, Root busy; 0.9,
//! Untrusted input). The line is the branch git holds, with that branch's own commit, and nothing else. Each test makes
//! its own repository in a temp root, which is also the home, and every git command clears the environment.

mod common;

use collide_host::boards::{Checkout, checkout_of, line_commit};
use common::git_repo::{GitRepo, git_command};
use std::fs;
use std::path::Path;

fn git_env(root: &Path) -> Vec<(String, String)> {
    vec![
        ("HOME".to_string(), root.display().to_string()),
        ("GIT_CONFIG_GLOBAL".to_string(), "/dev/null".to_string()),
        ("GIT_CONFIG_SYSTEM".to_string(), "/dev/null".to_string()),
        ("GIT_CONFIG_NOSYSTEM".to_string(), "1".to_string()),
    ]
}

fn checkout(repo: &GitRepo) -> Checkout {
    checkout_in(repo, repo.work())
}

fn checkout_in(repo: &GitRepo, folder: &Path) -> Checkout {
    checkout_of(folder, &git_env(repo.root().path())).expect("the folder is a checkout")
}

/// `main` and `topic` edit one line each: a rebase of `topic` onto `main` stops on the conflict, in `dir`, which is on
/// `topic`. `flags` go to `git rebase`.
fn stop_in_a_rebase(repo: &GitRepo, dir: &Path, flags: &[&str]) {
    let rebase = git_command(repo.root().path(), dir)
        .arg("rebase")
        .args(flags)
        .arg("main")
        .output()
        .expect("git runs");
    assert!(
        !rebase.status.success(),
        "the rebase should stop on the conflict"
    );
}

/// A repository with `main` and `topic` that conflict on `line.txt`. The checkout is on `topic`.
fn conflicting() -> GitRepo {
    let repo = GitRepo::new();
    repo.write("line.txt", b"one\n");
    repo.commit_all("line");
    repo.git(&["checkout", "--quiet", "-b", "topic"]);
    repo.write("line.txt", b"topic\n");
    repo.commit_all("topic edits the line");
    repo.git(&["checkout", "--quiet", "main"]);
    repo.write("line.txt", b"main\n");
    repo.commit_all("main edits the line");
    repo.git(&["checkout", "--quiet", "topic"]);
    repo
}

#[test]
fn a_rebase_that_stopped_on_a_conflict_keeps_the_branch_being_rebased_as_the_line() {
    let repo = conflicting();
    stop_in_a_rebase(&repo, repo.work(), &[]);
    let found = checkout(&repo);
    assert_eq!(found.branch, "HEAD", "git has HEAD detached");
    assert_eq!(found.line_branch(), "topic");
}

#[test]
fn the_line_commit_in_a_rebase_is_the_branchs_own_and_not_the_rebases_head() {
    let repo = conflicting();
    let topic = repo.git(&["rev-parse", "topic"]);
    stop_in_a_rebase(&repo, repo.work(), &[]);
    let head = repo.git(&["rev-parse", "HEAD"]);
    assert_ne!(head, topic, "the rebase's HEAD is not the branch");
    let found = checkout(&repo);
    let env = git_env(repo.root().path());
    assert_eq!(line_commit(&found, &env), Some(topic));
}

#[test]
fn a_rebase_with_the_apply_backend_keeps_the_branch_too() {
    let repo = conflicting();
    stop_in_a_rebase(&repo, repo.work(), &["--apply"]);
    assert!(repo.git_dir().join("rebase-apply").is_dir());
    assert_eq!(checkout(&repo).line_branch(), "topic");
}

#[test]
fn the_line_is_the_branch_again_when_the_rebase_is_aborted() {
    let repo = conflicting();
    stop_in_a_rebase(&repo, repo.work(), &[]);
    repo.git(&["rebase", "--abort"]);
    let found = checkout(&repo);
    assert_eq!(found.branch, "topic");
    assert_eq!(found.line_branch(), "topic");
}

#[test]
fn a_linked_checkout_reads_its_own_rebase_and_the_main_one_stays_on_its_branch() {
    let repo = conflicting();
    repo.git(&["checkout", "--quiet", "main"]);
    let linked = repo.root().join("linked");
    repo.git(&[
        "worktree",
        "add",
        "--quiet",
        linked.to_str().unwrap(),
        "topic",
    ]);
    stop_in_a_rebase(&repo, &linked, &[]);
    let side = checkout_in(&repo, &linked);
    assert_eq!(side.branch, "HEAD");
    assert_eq!(side.line_branch(), "topic");
    let main = checkout(&repo);
    assert_eq!(main.branch, "main");
    assert_eq!(main.line_branch(), "main");
}

#[test]
fn a_rebase_of_a_detached_head_has_no_branch_so_the_line_stays_head() {
    let repo = conflicting();
    repo.git(&["checkout", "--quiet", "--detach", "topic"]);
    stop_in_a_rebase(&repo, repo.work(), &[]);
    let found = checkout(&repo);
    assert_eq!(found.line_branch(), "HEAD");
    let env = git_env(repo.root().path());
    assert_eq!(
        line_commit(&found, &env),
        Some(repo.git(&["rev-parse", "HEAD"]))
    );
}

#[test]
fn a_head_name_that_names_no_branch_leaves_the_line_as_head() {
    let repo = conflicting();
    stop_in_a_rebase(&repo, repo.work(), &[]);
    let name = repo.git_dir().join("rebase-merge/head-name");
    fs::write(&name, "refs/heads/gone\n").unwrap();
    assert_eq!(checkout(&repo).line_branch(), "HEAD");
    fs::write(&name, "refs/tags/topic\n").unwrap();
    assert_eq!(checkout(&repo).line_branch(), "HEAD");
    fs::remove_file(&name).unwrap();
    assert_eq!(checkout(&repo).line_branch(), "HEAD");
}

#[test]
fn a_detached_head_that_is_in_no_operation_is_the_line_head() {
    let repo = conflicting();
    repo.git(&["checkout", "--quiet", "--detach"]);
    let found = checkout(&repo);
    assert_eq!(found.held, None);
    assert_eq!(found.line_branch(), "HEAD");
}

#[test]
fn a_bisect_keeps_the_branch_it_started_on_as_the_line() {
    let repo = conflicting();
    repo.git(&["checkout", "--quiet", "main"]);
    for step in 0..4 {
        repo.write("steps.txt", format!("{step}\n").as_bytes());
        repo.commit_all("step");
    }
    repo.git(&["bisect", "start"]);
    repo.git(&["bisect", "bad", "main"]);
    repo.git(&["bisect", "good", "main~4"]);
    let found = checkout(&repo);
    assert_eq!(found.branch, "HEAD", "the bisect detached HEAD");
    assert_eq!(found.line_branch(), "main");
    let env = git_env(repo.root().path());
    assert_eq!(
        line_commit(&found, &env),
        Some(repo.git(&["rev-parse", "main"]))
    );
    repo.git(&["bisect", "reset"]);
    assert_eq!(checkout(&repo).line_branch(), "main");
}

#[test]
fn a_bisect_started_on_a_detached_head_leaves_the_line_as_head() {
    let repo = conflicting();
    repo.git(&["checkout", "--quiet", "--detach", "main"]);
    repo.git(&["bisect", "start"]);
    assert_eq!(checkout(&repo).line_branch(), "HEAD");
}
