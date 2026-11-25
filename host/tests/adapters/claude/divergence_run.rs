//! A worktree that a run made, in the Field's divergence (ts-0180-ad; 0.10, Corpus): the agent of a prompt run changes
//! a task file in its worktree, and the `divergence` entry of `task.list` for that task names the run whose workspace
//! the checkout is. The source is a repo pack over files (`corpus_pack_provider.mjs`), run by `node`. Each test has a
//! daemon of its own under a temp root, and every process it starts has an environment of its own.

use super::common::{TempRoot, wait_until};
use super::file_run::{Fixture, Setup, git};
use super::quick_run::{call, create, open, report_with, started, wait_ended, write_file};
use collide_core::owner::{
    BoardTrustParams, BoardTrustResult, BoardTrustReviewParams, BoardTrustReviewResult,
    TaskListParams, TaskListResult, TrustState,
};
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

const PROVIDER: &str = include_str!("../../fixtures/corpus_pack_provider.mjs");

/// The folder `node` is in, which fake Claude's shell adds to the `PATH` the daemon captures.
fn node_dir() -> String {
    let path = std::env::var_os("PATH").expect("the tests run with a PATH");
    std::env::split_paths(&path)
        .find(|dir| dir.join("node").is_file())
        .expect("node is on the PATH")
        .display()
        .to_string()
}

fn task_file(status: &str) -> String {
    format!("{{\"short\":\"1\",\"title\":\"Rename\",\"status\":\"{status}\",\"version\":\"v1\"}}\n")
}

fn listed(fx: &mut Fixture, board: &str) -> TaskListResult {
    call(
        fx,
        "task.list",
        &TaskListParams {
            board: board.to_string(),
            ready: false,
            kind: None,
            checkout: None,
            view: None,
            filter: None,
            limit: None,
            cursor: None,
        },
    )
}

#[test]
fn a_task_file_changed_in_a_runs_worktree_is_a_divergent_copy_that_names_the_run() {
    let root = TempRoot::new();
    let agent = [write_file("tasks/1.json", &task_file("doing"))];
    let mut fx = Fixture::new(
        &report_with(&root, &agent),
        Setup {
            profile: format!("export PATH=\"$PATH:{}\"", node_dir()),
            corpus_recheck: Some(Duration::from_millis(200)),
            ..Setup::default()
        },
    );
    let repo: PathBuf = fx.repo.clone();
    fs::create_dir_all(repo.join("tasks")).unwrap();
    fs::write(repo.join("tasks/1.json"), task_file("todo")).unwrap();
    fs::create_dir_all(repo.join(".collide/packs/quay/providers")).unwrap();
    fs::write(
        repo.join(".collide/packs/quay/pack.yaml"),
        "apiVersion: collide/v0\nname: quay\nversion: 1.0.0\nproviders:\n  files: { script: providers/files.mjs }\n",
    )
    .unwrap();
    fs::write(
        repo.join(".collide/packs/quay/providers/files.mjs"),
        PROVIDER
            .replace("__SOURCE__", "files")
            .replace("__WATCH__", "false")
            .replace("__PAGE__", "100"),
    )
    .unwrap();
    fs::write(
        repo.join(".collide/board.yaml"),
        "packs:\n  - source: repo:.collide/packs/quay\n",
    )
    .unwrap();
    git(&repo, &["config", "user.name", "test"]);
    git(&repo, &["config", "user.email", "test@example.invalid"]);
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-q", "-m", "a pack and a task"]);
    let board = open(&mut fx, &repo);
    trust(&mut fx, &board);
    wait_until("the source lists its task", Duration::from_secs(30), || {
        listed(&mut fx, &board)
            .tasks
            .iter()
            .any(|task| task.key == "files:1")
    });

    // A scratch task is what the run is about; the file task is what its agent changes in the worktree.
    let key = create(&mut fx, &board, "Rename the widget");
    let run = started(&mut fx, &board, &key);
    wait_ended(&mut fx, &run);

    let mut found = None;
    wait_until(
        "the worktree's copy is divergent",
        Duration::from_secs(30),
        || {
            found = listed(&mut fx, &board).divergence.into_iter().next();
            found.is_some()
        },
    );
    let entry = found.expect("waited for");
    assert_eq!(entry.key, "files:1");
    assert_eq!(entry.copies.len(), 1);
    assert_eq!(entry.copies[0].status, "doing");
    assert_eq!(entry.copies[0].run.as_deref(), Some(run.as_str()));
    assert!(
        entry.copies[0].checkout.ends_with(&run),
        "{}",
        entry.copies[0].checkout
    );
}

fn trust(fx: &mut Fixture, board: &str) {
    let review: BoardTrustReviewResult = call(
        fx,
        "board.trust_review",
        &BoardTrustReviewParams {
            board: board.to_string(),
            line: "main".to_string(),
        },
    );
    let hash = match review.trust {
        TrustState::Trusted { hash } | TrustState::Untrusted { hash } => hash,
    };
    assert!(!hash.is_empty(), "{:?}", review.problems);
    let _: BoardTrustResult = call(
        fx,
        "board.trust",
        &BoardTrustParams {
            board: board.to_string(),
            line: "main".to_string(),
            hash,
            hosts: Vec::new(),
            dev_folders: Vec::new(),
        },
    );
}
