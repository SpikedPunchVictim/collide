//! Accept on a quick run's `Review` item (ts-0079-pu; Stage 3c): `queue.answer` with `accept` merges the run's branch
//! into the board's checkout, removes the worktree and its branch, and moves the scratch task to done. Each test has
//! a daemon of its own under a temp root and fake Claude as `claude` (`quick_run.rs` has the helpers): git reads no
//! configuration, every command the test runs clears its environment, and nothing here reads the owner's files.
//!
//! A daemon killed between the intent and the outcome is made by stopping the daemon and writing what a kill leaves:
//! the intent event and no Queue item, since accept takes the item out before it acts. The restarted daemon then
//! checks the checkout.

use super::common::{TempRoot, wait_until};
use super::file_run::Fixture;
use super::quick_run::{
    PROMPT, answer, create, git_out, open, quick, rpc_error, run_get, started, status_moves, task,
    wait_ended, wait_item, workspace_moves, write_file,
};
use collide_core::change::{Actor, ActorKind};
use collide_core::events::{Event, EventBody, StepKind, StepResult};
use collide_core::item::Category;
use collide_core::owner::{AnswerKind, QueueAnswer, QueueAnswerResult, QueueItem, QueueKind, code};
use collide_core::state::{RunState, WorkspaceState};
use collide_host::store::Store;
use serde_json::{Value, json};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

// ---- helpers ------------------------------------------------------------------------------------------------------

/// What the agent does: writes `file` and commits it.
pub(super) fn adds(file: &str) -> Vec<Value> {
    vec![
        write_file(file, "from the agent\n"),
        json!({"git_commit": format!("Add {file}")}),
    ]
}

/// Runs git in `dir` as `git_out` does, and says whether it worked. For a command that is meant to stop half-way.
pub(super) fn git_stops(dir: &Path, args: &[&str]) -> bool {
    Command::new("git")
        .args([
            "-c",
            "user.name=test",
            "-c",
            "user.email=test@example.invalid",
        ])
        .args(["-c", "commit.gpgsign=false", "-c", "core.editor=true"])
        .args(args)
        .current_dir(dir)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", "/nonexistent")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .expect("git runs")
        .status
        .success()
}

/// Commits `file` with `text` on the branch the checkout is on.
pub(super) fn commit_file(dir: &Path, file: &str, text: &str, message: &str) {
    fs::write(dir.join(file), text).unwrap();
    git_out(dir, &["add", "."]);
    git_out(dir, &["commit", "-q", "-m", message]);
}

pub(super) struct Reviewed {
    pub(super) fx: Fixture,
    pub(super) board: String,
    pub(super) key: String,
    pub(super) run: String,
    pub(super) item: QueueItem,
}

/// A run whose agent did `steps`, ended, with its `Review` item.
pub(super) fn reviewed(root: &TempRoot, steps: &[Value]) -> Reviewed {
    reviewed_in(quick(root, steps))
}

/// The run of the task of `quick_in`, started and ended, with its `Review` item.
pub(super) fn reviewed_in((mut fx, board, key): (Fixture, String, String)) -> Reviewed {
    let run = started(&mut fx, &board, &key);
    let item = wait_item(&mut fx, &run, QueueKind::Review);
    wait_ended(&mut fx, &run);
    Reviewed {
        fx,
        board,
        key,
        run,
        item,
    }
}

impl Reviewed {
    pub(super) fn accept(&mut self) -> QueueAnswerResult {
        answer(&mut self.fx, &self.item, QueueAnswer::Accept {})
    }

    /// The message of a refused accept. The item is still in the Queue, and the run still has its worktree.
    pub(super) fn refused(&mut self) -> String {
        match self.accept() {
            QueueAnswerResult::Refused { message } => {
                assert_eq!(
                    self.fx.queue(&self.run),
                    std::slice::from_ref(&self.item),
                    "{message}"
                );
                assert!(self.fx.workspace_dir(&self.run).is_dir(), "{message}");
                message
            }
            other => panic!("the accept should be refused, and gave {other:?}"),
        }
    }

    pub(super) fn merged(&mut self) -> String {
        match self.accept() {
            QueueAnswerResult::Merged { commit } => commit,
            other => panic!("the accept should merge, and gave {other:?}"),
        }
    }

    pub(super) fn branch(&self) -> String {
        format!("collide/{}", self.run)
    }

    pub(super) fn branch_head(&self) -> String {
        git_out(&self.fx.repo, &["rev-parse", &self.branch()])
    }

    pub(super) fn head(&self) -> String {
        git_out(&self.fx.repo, &["rev-parse", "HEAD"])
    }

    pub(super) fn category(&mut self) -> Category {
        task(&mut self.fx, &self.board, &self.key).state.category
    }

    /// The worktree and the branch are gone, and git no longer lists the worktree.
    pub(super) fn assert_removed(&self) {
        assert!(!self.fx.workspace_dir(&self.run).exists());
        assert_eq!(
            git_out(&self.fx.repo, &["branch", "--list", &self.branch()]),
            ""
        );
        assert!(!git_out(&self.fx.repo, &["worktree", "list"]).contains(&self.run));
    }
}

pub(super) fn step_events(events: &[EventBody]) -> Vec<EventBody> {
    events
        .iter()
        .filter(|body| {
            matches!(
                body,
                EventBody::StepIntent { .. } | EventBody::StepOutcome { .. }
            )
        })
        .cloned()
        .collect()
}

pub(super) fn status(dir: &Path) -> String {
    git_out(dir, &["status", "--porcelain"])
}

// ---- accept -------------------------------------------------------------------------------------------------------

#[test]
fn accept_merges_the_branch_into_the_checkout_and_removes_the_worktree() {
    let root = TempRoot::new();
    let mut run = reviewed(&root, &adds("a.txt"));
    let branch_head = run.branch_head();

    let commit = run.merged();

    assert_eq!(
        commit,
        run.head(),
        "the answer names the checkout's new head"
    );
    assert_eq!(
        commit, branch_head,
        "the base had not moved, so it fast-forwards"
    );
    assert_eq!(
        git_out(&run.fx.repo, &["log", "-n", "1", "--format=%s"]),
        "Add a.txt"
    );
    assert!(run.fx.repo.join("a.txt").is_file());
    run.assert_removed();
    assert_eq!(run.category(), Category::Done);
    assert_eq!(
        status_moves(&run.fx.events()),
        ["ready", "active", "review", "done"]
    );
    assert!(run.fx.queue(&run.run).is_empty(), "the item left the Queue");
    let moves = workspace_moves(&run.fx.events());
    assert_eq!(
        moves[moves.len() - 2..],
        [WorkspaceState::Removable, WorkspaceState::Removed]
    );
    let state = run_get(&mut run.fx, &run.run).summary.state;
    assert!(matches!(state, RunState::Ended { outcome } if outcome.as_str() == "done"));
}

#[test]
fn accept_writes_the_intent_before_the_merge_and_the_outcome_after_it() {
    let root = TempRoot::new();
    let mut run = reviewed(&root, &adds("a.txt"));
    let commit = run.merged();

    let steps = step_events(&run.fx.events());
    assert_eq!(steps.len(), 2, "{steps:?}");
    match &steps[0] {
        EventBody::StepIntent { step, kind, action } => {
            assert_eq!(step, "collide/merge-local");
            assert_eq!(*kind, StepKind::Integrate);
            assert!(
                action.contains(&run.branch()) && action.contains("main"),
                "{action}"
            );
        }
        other => panic!("the intent comes first: {other:?}"),
    }
    match &steps[1] {
        EventBody::StepOutcome {
            step,
            kind,
            result,
            detail,
        } => {
            assert_eq!(step, "collide/merge-local");
            assert_eq!(*kind, StepKind::Integrate);
            assert_eq!(*result, StepResult::Succeeded);
            assert!(detail.contains(&commit), "{detail}");
        }
        other => panic!("the outcome comes second: {other:?}"),
    }
}

#[test]
fn a_base_that_moved_on_gets_a_merge_commit_with_both_sides() {
    let root = TempRoot::new();
    let mut run = reviewed(&root, &adds("a.txt"));
    let branch_head = run.branch_head();
    commit_file(&run.fx.repo, "b.txt", "later\n", "later");
    let later = run.head();

    let commit = run.merged();

    assert_eq!(git_out(&run.fx.repo, &["rev-parse", "HEAD"]), commit);
    assert_eq!(git_out(&run.fx.repo, &["rev-parse", "HEAD^1"]), later);
    assert_eq!(git_out(&run.fx.repo, &["rev-parse", "HEAD^2"]), branch_head);
    assert!(run.fx.repo.join("a.txt").is_file() && run.fx.repo.join("b.txt").is_file());
    run.assert_removed();
}

#[test]
fn a_second_answer_to_an_accepted_item_is_refused_and_changes_nothing() {
    let root = TempRoot::new();
    let mut run = reviewed(&root, &adds("a.txt"));
    let commit = run.merged();

    let params = collide_core::owner::QueueAnswerParams {
        item: run.item.id.clone(),
        answer: QueueAnswer::Accept {},
    };
    let (refused, _) = rpc_error(&mut run.fx, "queue.answer", &params);

    assert_eq!(refused, code::NOT_FOUND);
    assert_eq!(run.head(), commit);
}

// ---- what refuses the accept --------------------------------------------------------------------------------------

#[test]
fn a_conflict_refuses_the_accept_names_the_files_and_changes_nothing() {
    let root = TempRoot::new();
    let steps = [
        write_file("README", "from the agent\n"),
        json!({"git_commit": "Edit README"}),
    ];
    let mut run = reviewed(&root, &steps);
    commit_file(&run.fx.repo, "README", "from main\n", "Edit README on main");
    let head = run.head();

    let message = run.refused();

    assert!(message.contains("README"), "{message}");
    assert!(message.to_lowercase().contains("conflict"), "{message}");
    assert_eq!(status(&run.fx.repo), "");
    assert_eq!(run.head(), head);
    assert!(!run.fx.repo.join(".git/MERGE_HEAD").exists());
    assert_eq!(run.category(), Category::Review);
    assert!(
        step_events(&run.fx.events()).is_empty(),
        "nothing was started"
    );
}

#[test]
fn a_checkout_on_another_branch_is_refused_and_names_the_branch_to_switch_to() {
    let root = TempRoot::new();
    let mut run = reviewed(&root, &adds("a.txt"));
    git_out(&run.fx.repo, &["checkout", "-q", "-b", "other"]);
    let main = git_out(&run.fx.repo, &["rev-parse", "main"]);

    let message = run.refused();

    assert!(
        message.contains("`other`") && message.contains("`main`"),
        "{message}"
    );
    assert_eq!(
        git_out(&run.fx.repo, &["rev-parse", "--abbrev-ref", "HEAD"]),
        "other"
    );
    assert_eq!(git_out(&run.fx.repo, &["rev-parse", "main"]), main);
    assert!(step_events(&run.fx.events()).is_empty());

    // The item stayed, so switching back and accepting again works.
    git_out(&run.fx.repo, &["checkout", "-q", "main"]);
    run.merged();
    run.assert_removed();
}

/// A checkout in the middle of `what`, which `start` puts it in with a conflict on `README`.
fn refused_while(what: &str, start: impl FnOnce(&Path)) {
    let root = TempRoot::new();
    let mut run = reviewed(&root, &adds("a.txt"));
    start(&run.fx.repo);
    let head = run.head();

    let message = run.refused();

    assert!(message.contains(what), "{message}");
    assert_eq!(run.head(), head);
    assert!(!run.fx.repo.join("a.txt").exists());
    assert!(step_events(&run.fx.events()).is_empty());
}

/// `topic` and `main` both change `README`, so `git` stops when it brings one to the other.
fn diverge_on_readme(repo: &Path) {
    git_out(repo, &["branch", "topic"]);
    commit_file(repo, "README", "from main\n", "Edit README on main");
    git_out(repo, &["checkout", "-q", "topic"]);
    commit_file(repo, "README", "from topic\n", "Edit README on topic");
    git_out(repo, &["checkout", "-q", "main"]);
}

#[test]
fn a_rebase_in_progress_is_refused() {
    refused_while("rebase", |repo| {
        diverge_on_readme(repo);
        git_out(repo, &["checkout", "-q", "topic"]);
        assert!(
            !git_stops(repo, &["rebase", "main"]),
            "the rebase should stop"
        );
    });
}

#[test]
fn a_merge_in_progress_is_refused() {
    refused_while("merge", |repo| {
        diverge_on_readme(repo);
        assert!(
            !git_stops(repo, &["merge", "topic"]),
            "the merge should stop"
        );
    });
}

#[test]
fn a_cherry_pick_in_progress_is_refused() {
    refused_while("cherry-pick", |repo| {
        diverge_on_readme(repo);
        assert!(
            !git_stops(repo, &["cherry-pick", "topic"]),
            "the cherry-pick should stop"
        );
    });
}

#[test]
fn a_bisect_in_progress_is_refused() {
    refused_while("bisect", |repo| {
        assert!(git_stops(repo, &["bisect", "start"]));
    });
}

#[test]
fn a_local_change_the_merge_would_overwrite_is_refused_by_git_and_nothing_changes() {
    let root = TempRoot::new();
    let steps = [
        write_file("README", "from the agent\n"),
        json!({"git_commit": "Edit README"}),
    ];
    let mut run = reviewed(&root, &steps);
    fs::write(run.fx.repo.join("README"), "my local edit\n").unwrap();
    let head = run.head();

    let message = run.refused();

    assert!(
        message.contains("README") && message.contains("overwritten"),
        "{message}"
    );
    assert_eq!(
        fs::read_to_string(run.fx.repo.join("README")).unwrap(),
        "my local edit\n"
    );
    assert_eq!(run.head(), head);
    assert_eq!(run.category(), Category::Review);
    // The step was started and failed, so the log says what was tried.
    let steps = step_events(&run.fx.events());
    assert!(
        matches!(
            steps.as_slice(),
            [
                EventBody::StepIntent { .. },
                EventBody::StepOutcome {
                    result: StepResult::Failed,
                    ..
                }
            ]
        ),
        "{steps:?}"
    );
}

#[test]
fn a_local_change_the_merge_does_not_touch_stays_as_it_was() {
    let root = TempRoot::new();
    let mut run = reviewed(&root, &adds("a.txt"));
    fs::write(run.fx.repo.join("README"), "my local edit\n").unwrap();
    fs::write(run.fx.repo.join("notes.txt"), "mine\n").unwrap();

    run.merged();

    assert_eq!(
        fs::read_to_string(run.fx.repo.join("README")).unwrap(),
        "my local edit\n"
    );
    let left = status(&run.fx.repo);
    assert!(
        left.contains("M README") && left.contains("?? notes.txt"),
        "{left}"
    );
    assert!(run.fx.repo.join("a.txt").is_file());
    run.assert_removed();
}

#[test]
fn a_hook_of_the_repository_that_refuses_the_merge_is_a_refusal_and_the_checkout_is_as_it_was() {
    let root = TempRoot::new();
    let mut run = reviewed(&root, &adds("a.txt"));
    commit_file(&run.fx.repo, "b.txt", "later\n", "later");
    let hook = run.fx.repo.join(".git/hooks/pre-merge-commit");
    fs::write(&hook, "#!/bin/sh\necho 'no merges today' >&2\nexit 1\n").unwrap();
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
    let head = run.head();

    let message = run.refused();

    assert!(message.contains("no merges today"), "{message}");
    assert_eq!(status(&run.fx.repo), "");
    assert_eq!(run.head(), head);
    assert!(
        !run.fx.repo.join(".git/MERGE_HEAD").exists(),
        "the half-made merge is undone"
    );
    assert_eq!(run.category(), Category::Review);

    // With the hook gone the same item merges.
    fs::remove_file(&hook).unwrap();
    run.merged();
    run.assert_removed();
}

#[test]
fn a_worktree_with_uncommitted_changes_refuses_the_accept_before_anything_is_merged() {
    let root = TempRoot::new();
    let mut run = reviewed(&root, &adds("a.txt"));
    let tree = run.fx.workspace_dir(&run.run);
    fs::write(tree.join("extra.txt"), "added after the review\n").unwrap();
    let head = run.head();

    let message = run.refused();

    assert!(
        message.contains("uncommitted") || message.contains("untracked"),
        "{message}"
    );
    assert_eq!(run.head(), head);
    assert!(tree.join("extra.txt").is_file());
    assert!(step_events(&run.fx.events()).is_empty());
}

// ---- a branch whose change is already in the base -----------------------------------------------------------------

/// Accepts a run whose change `land` put in the base, and checks nothing new was merged.
fn accepted_as_already_merged(land: impl FnOnce(&Path, &str)) {
    let root = TempRoot::new();
    let mut run = reviewed(&root, &adds("a.txt"));
    land(&run.fx.repo, &run.branch());
    let head = run.head();

    let commit = run.merged();

    assert_eq!(commit, head, "no commit is made: the base has the change");
    assert_eq!(run.head(), head);
    run.assert_removed();
    assert_eq!(run.category(), Category::Done);
    let steps = step_events(&run.fx.events());
    assert!(
        matches!(
            steps.as_slice(),
            [
                EventBody::StepIntent { .. },
                EventBody::StepOutcome {
                    result: StepResult::Succeeded,
                    ..
                }
            ]
        ),
        "{steps:?}"
    );
}

#[test]
fn a_branch_already_merged_into_the_base_is_accepted_as_merged() {
    accepted_as_already_merged(|repo, branch| {
        git_out(repo, &["merge", "-q", "--ff-only", branch]);
    });
}

#[test]
fn a_branch_squashed_into_the_base_is_accepted_as_merged() {
    accepted_as_already_merged(|repo, branch| {
        git_out(repo, &["merge", "-q", "--squash", branch]);
        git_out(repo, &["commit", "-q", "-m", "squash of the run"]);
    });
}

#[test]
fn a_branch_rebased_onto_the_base_is_accepted_as_merged() {
    accepted_as_already_merged(|repo, branch| {
        commit_file(repo, "b.txt", "later\n", "later");
        git_out(repo, &["cherry-pick", branch]);
    });
}

// ---- a board whose root is a linked worktree ----------------------------------------------------------------------

#[test]
fn a_board_whose_root_is_a_linked_worktree_merges_into_the_branch_it_is_on() {
    let root = TempRoot::new();
    let (mut fx, _, _) = quick(&root, &adds("a.txt"));
    let linked = fx.root.join("linked");
    git_out(
        &fx.repo,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "feature",
            linked.to_str().unwrap(),
        ],
    );
    let linked = fs::canonicalize(linked).unwrap();
    let main = git_out(&fx.repo, &["rev-parse", "main"]);

    let board = open(&mut fx, &linked);
    let key = create(&mut fx, &board, PROMPT);
    let run = started(&mut fx, &board, &key);
    let item = wait_item(&mut fx, &run, QueueKind::Review);
    wait_ended(&mut fx, &run);

    let result = answer(&mut fx, &item, QueueAnswer::Accept {});

    let QueueAnswerResult::Merged { commit } = result else {
        panic!("should merge: {result:?}");
    };
    assert_eq!(git_out(&fx.repo, &["rev-parse", "feature"]), commit);
    assert_eq!(git_out(&linked, &["rev-parse", "HEAD"]), commit);
    assert!(linked.join("a.txt").is_file());
    assert_eq!(
        git_out(&fx.repo, &["rev-parse", "main"]),
        main,
        "main is not touched"
    );
    assert!(!fx.workspace_dir(&run).exists());
}

// ---- a daemon killed between the intent and the outcome -----------------------------------------------------------

pub(super) fn actor(run: &str) -> Actor {
    Actor {
        kind: ActorKind::Workflow,
        run: Some(run.to_string()),
        phase: None,
    }
}

/// Stops the daemon as a kill after the intent would leave it: the item is out of the Queue, the intent is in the log.
/// With `outcome`, the outcome is written too, as a kill between the outcome and the removal leaves it.
pub(super) fn killed_after_intent(run: &mut Reviewed, outcome: Option<&str>) {
    run.fx.stop_daemon();
    let store = Store::open(&run.fx.paths().store_dir).unwrap();
    store.delete_queue_item(&run.item.id).unwrap();
    let mut bodies = vec![EventBody::StepIntent {
        step: "collide/merge-local".to_string(),
        kind: StepKind::Integrate,
        action: format!("merge {} into main", run.branch()),
    }];
    if let Some(detail) = outcome {
        bodies.push(EventBody::StepOutcome {
            step: "collide/merge-local".to_string(),
            kind: StepKind::Integrate,
            result: StepResult::Succeeded,
            detail: detail.to_string(),
        });
    }
    for body in bodies {
        store
            .append_event(&Event {
                seq: store.last_seq().unwrap() + 1,
                wall_ms: 1,
                actor: actor(&run.run),
                experiment: None,
                body,
            })
            .unwrap();
    }
}

fn merge_by_hand(run: &Reviewed) {
    git_out(
        &run.fx.repo,
        &["merge", "-q", "--no-ff", "-m", "by hand", &run.branch()],
    );
}

fn merges(run: &Reviewed) -> String {
    git_out(&run.fx.repo, &["rev-list", "--count", "--merges", "main"])
}

#[test]
fn after_a_kill_a_change_at_the_base_is_a_success_and_the_removal_finishes_with_no_second_merge() {
    let root = TempRoot::new();
    let mut run = reviewed(&root, &adds("a.txt"));
    merge_by_hand(&run);
    let head = run.head();
    killed_after_intent(&mut run, None);

    run.fx.start_daemon();

    assert_eq!(run.head(), head, "the merge is not repeated");
    assert_eq!(merges(&run), "1");
    run.assert_removed();
    assert_eq!(run.category(), Category::Done);
    assert!(run.fx.queue(&run.run).is_empty());
    let steps = step_events(&run.fx.events());
    assert!(
        matches!(steps.last(), Some(EventBody::StepOutcome { result: StepResult::Succeeded, detail, .. })
            if detail.contains(&head)),
        "{steps:?}"
    );
}

#[test]
fn after_a_kill_an_interrupted_merge_is_an_outcome_unknown_item_and_nothing_is_touched() {
    let root = TempRoot::new();
    let mut run = reviewed(&root, &adds("a.txt"));
    commit_file(&run.fx.repo, "b.txt", "later\n", "later");
    assert!(git_stops(
        &run.fx.repo,
        &["merge", "--no-commit", "--no-ff", &run.branch()]
    ));
    assert!(run.fx.repo.join(".git/MERGE_HEAD").exists());
    killed_after_intent(&mut run, None);

    run.fx.start_daemon();

    let items = run.fx.queue(&run.run);
    assert_eq!(items.len(), 1, "{items:?}");
    assert_eq!(items[0].kind, QueueKind::OutcomeUnknown);
    assert_eq!(items[0].task.as_deref(), Some(run.key.as_str()));
    assert!(
        run.fx.repo.join(".git/MERGE_HEAD").exists(),
        "the checkout is as it was left"
    );
    assert!(run.fx.workspace_dir(&run.run).is_dir());
    assert_eq!(run.category(), Category::Review);

    // The person undoes the merge and says so: the review item comes back, and accept works.
    git_out(&run.fx.repo, &["merge", "--abort"]);
    let unknown = items[0].clone();
    assert_eq!(unknown.answers, [AnswerKind::Dismiss]);
    answer(&mut run.fx, &unknown, QueueAnswer::Dismiss {});
    let back = wait_item(&mut run.fx, &run.run, QueueKind::Review);
    assert_eq!(
        back.answers,
        [
            AnswerKind::Accept,
            AnswerKind::Reprompt,
            AnswerKind::Discard
        ]
    );
    run.item = back;
    run.merged();
    run.assert_removed();
}

/// A run whose accept a kill cut short in the middle of a merge in the checkout: the restarted daemon asks about the
/// outcome, the person undoes the merge and dismisses the item, and the review item is back. Returns that item.
pub(super) fn review_back_after_an_unknown_outcome(run: &mut Reviewed) -> QueueItem {
    commit_file(&run.fx.repo, "b.txt", "later\n", "later");
    assert!(git_stops(
        &run.fx.repo,
        &["merge", "--no-commit", "--no-ff", &run.branch()]
    ));
    killed_after_intent(run, None);
    run.fx.start_daemon();
    let unknown = wait_item(&mut run.fx, &run.run, QueueKind::OutcomeUnknown);
    git_out(&run.fx.repo, &["merge", "--abort"]);
    answer(&mut run.fx, &unknown, QueueAnswer::Dismiss {});
    wait_item(&mut run.fx, &run.run, QueueKind::Review)
}

#[test]
fn after_a_kill_before_the_merge_the_review_item_is_back_and_the_merge_is_made_when_accepted() {
    let root = TempRoot::new();
    let mut run = reviewed(&root, &adds("a.txt"));
    let head = run.head();
    killed_after_intent(&mut run, None);

    run.fx.start_daemon();

    let back = wait_item(&mut run.fx, &run.run, QueueKind::Review);
    assert_eq!(
        back.answers,
        [
            AnswerKind::Accept,
            AnswerKind::Reprompt,
            AnswerKind::Discard
        ]
    );
    assert_eq!(run.head(), head);
    assert!(run.fx.workspace_dir(&run.run).is_dir());
    assert_eq!(run.category(), Category::Review);
    let steps = step_events(&run.fx.events());
    assert!(
        matches!(
            steps.last(),
            Some(EventBody::StepOutcome {
                result: StepResult::Failed,
                ..
            })
        ),
        "the open intent is closed: {steps:?}"
    );

    run.item = back;
    run.merged();
    run.assert_removed();
}

#[test]
fn after_a_kill_between_the_outcome_and_the_removal_the_removal_finishes() {
    let root = TempRoot::new();
    let mut run = reviewed(&root, &adds("a.txt"));
    merge_by_hand(&run);
    let head = run.head();
    killed_after_intent(&mut run, Some(&head));

    run.fx.start_daemon();

    run.assert_removed();
    assert_eq!(run.category(), Category::Done);
    assert_eq!(merges(&run), "1");
    assert!(run.fx.queue(&run.run).is_empty());
    wait_until("the one outcome", Duration::from_secs(5), || {
        step_events(&run.fx.events()).len() == 2
    });
}
