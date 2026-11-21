//! Discard on a quick run's `Review` item (ts-0080-tq; Stage 3c): `queue.answer` with `discard` removes the run's
//! worktree even with uncommitted changes and deletes its branch, moves the scratch task to dropped, and leaves the
//! board's checkout alone. The step's events keep the branch's last commit, so a person can restore it by hand. Each
//! test has a daemon of its own under a temp root and fake Claude as `claude` (`quick_run.rs`, `accept.rs` and
//! `reprompt.rs` have the helpers): git reads no configuration, every command the test runs clears its environment, and
//! nothing here reads the owner's files.

use super::accept::{
    Reviewed, actor, adds, commit_file, review_back_after_an_unknown_outcome, reviewed, status,
    step_events,
};
use super::common::TempRoot;
use super::file_run::{FakeFree, Fixture, Setup};
use super::quick_run::{
    answer, git_out, quick_in, report_with, rpc_error, run_get, started, status_moves, wait_item,
    workspace_moves,
};
use super::reprompt::{failed_run, stopped_run};
use collide_core::events::{Event, EventBody, StepKind, StepResult};
use collide_core::item::Category;
use collide_core::owner::{QueueAnswer, QueueAnswerParams, QueueAnswerResult, QueueKind, code};
use collide_core::state::{RunState, WorkspaceState};
use collide_host::store::Store;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

// ---- helpers ------------------------------------------------------------------------------------------------------

impl Reviewed {
    fn discard(&mut self) -> QueueAnswerResult {
        answer(&mut self.fx, &self.item, QueueAnswer::Discard {})
    }

    fn discarded(&mut self) {
        let result = self.discard();
        assert_eq!(result, QueueAnswerResult::Done {});
    }

    fn tree(&self) -> PathBuf {
        self.fx.workspace_dir(&self.run)
    }

    /// What the checkout looks like: its status, its head, its log, and the branches that are not the runs'.
    fn checkout(&self) -> (String, String, String, Vec<String>) {
        let repo = &self.fx.repo;
        let branches = git_out(repo, &["for-each-ref", "--format=%(refname)", "refs/heads"]);
        (
            status(repo),
            git_out(repo, &["rev-parse", "HEAD"]),
            git_out(repo, &["log", "--format=%H %s", "HEAD"]),
            branches
                .lines()
                .filter(|name| !name.contains("collide/"))
                .map(str::to_string)
                .collect(),
        )
    }

    /// The `collide/discard` events: the intent and the outcome.
    fn discard_steps(&self) -> Vec<EventBody> {
        step_events(&self.fx.events())
            .into_iter()
            .filter(|body| {
                matches!(
                    body,
                    EventBody::StepIntent {
                        kind: StepKind::Discard,
                        ..
                    } | EventBody::StepOutcome {
                        kind: StepKind::Discard,
                        ..
                    }
                )
            })
            .collect()
    }
}

/// A run stopped for disk before the agent started, with its `Review` item. `readings` are the free space the
/// daemon reads: the first run under the floor leaves no worktree, a later one leaves a worktree and no phase.
pub(super) fn stopped_for_disk(root: &TempRoot, readings: &[u64]) -> Reviewed {
    let setup = Setup {
        free: Some(FakeFree::new(readings)),
        ..Setup::default()
    };
    let (mut fx, board, key) = quick_in(Fixture::new(&report_with(root, &[]), setup));
    let run = started(&mut fx, &board, &key);
    let disk = wait_item(&mut fx, &run, QueueKind::DiskFull);
    let stopped = answer(&mut fx, &disk, QueueAnswer::Stop {});
    assert_eq!(stopped, QueueAnswerResult::Done {});
    let item = wait_item(&mut fx, &run, QueueKind::Review);
    Reviewed {
        fx,
        board,
        key,
        run,
        item,
    }
}

// ---- discard ------------------------------------------------------------------------------------------------------

#[test]
fn discard_removes_the_worktree_and_the_branch_and_leaves_the_checkout_as_it_was() {
    let root = TempRoot::new();
    let mut run = reviewed(&root, &adds("a.txt"));
    // The checkout has changes of the person's own, and discard does not touch them.
    fs::write(run.fx.repo.join("README"), "my local edit\n").unwrap();
    fs::write(run.fx.repo.join("notes.txt"), "mine\n").unwrap();
    let before = run.checkout();
    assert!(before.0.contains("M README") && before.0.contains("?? notes.txt"));

    run.discarded();

    run.assert_removed();
    assert_eq!(run.checkout(), before);
    assert!(!run.fx.repo.join("a.txt").exists());
    assert_eq!(
        fs::read_to_string(run.fx.repo.join("README")).unwrap(),
        "my local edit\n"
    );
    assert_eq!(
        git_out(&run.fx.repo, &["rev-parse", "--abbrev-ref", "HEAD"]),
        "main"
    );
}

#[test]
fn discard_moves_the_task_to_dropped_and_the_item_leaves_the_queue() {
    let root = TempRoot::new();
    let mut run = reviewed(&root, &adds("a.txt"));

    run.discarded();

    assert_eq!(run.category(), Category::Dropped);
    assert_eq!(
        status_moves(&run.fx.events()),
        ["ready", "active", "review", "dropped"]
    );
    assert!(run.fx.queue(&run.run).is_empty());
    assert!(run.fx.queue_all().is_empty());
}

#[test]
fn the_run_and_its_context_stay_after_a_discard() {
    let root = TempRoot::new();
    let mut run = reviewed(&root, &adds("a.txt"));

    run.discarded();

    let got = run_get(&mut run.fx, &run.run);
    assert!(
        matches!(&got.summary.state, RunState::Ended { outcome } if outcome.as_str() == "done")
    );
    assert_eq!(got.implementations["workspace"], "builtin");
    assert_eq!(got.summary.workspace, None, "the folder is gone");
    let moves = workspace_moves(&run.fx.events());
    assert_eq!(
        moves[moves.len() - 2..],
        [WorkspaceState::Removable, WorkspaceState::Removed]
    );
    // A restart reads the context back and does not look for a half-made accept.
    run.fx.stop_daemon();
    run.fx.start_daemon();
    let again = run_get(&mut run.fx, &run.run);
    assert_eq!(again.implementations["workspace"], "builtin");
    assert!(run.fx.queue(&run.run).is_empty());
}

#[test]
fn the_events_keep_the_last_commit_of_the_branch_so_it_can_be_restored_by_hand() {
    let root = TempRoot::new();
    let mut run = reviewed(&root, &adds("a.txt"));
    let last = run.branch_head();

    run.discarded();

    let steps = run.discard_steps();
    assert_eq!(steps.len(), 2, "{steps:?}");
    match &steps[0] {
        EventBody::StepIntent { step, action, .. } => {
            assert_eq!(step, "collide/discard");
            assert!(
                action.contains(&run.branch()) && action.contains(&last),
                "{action}"
            );
        }
        other => panic!("the intent comes first: {other:?}"),
    }
    match &steps[1] {
        EventBody::StepOutcome { result, detail, .. } => {
            assert_eq!(*result, StepResult::Succeeded);
            assert_eq!(detail, &last);
        }
        other => panic!("the outcome comes last: {other:?}"),
    }
    // Git has not collected the commit, so a branch made of it brings the work back.
    git_out(&run.fx.repo, &["branch", "restored", &last]);
    assert_eq!(
        git_out(&run.fx.repo, &["log", "-n", "1", "--format=%s", "restored"]),
        "Add a.txt"
    );
}

#[test]
fn a_worktree_with_committed_uncommitted_and_untracked_changes_is_removed() {
    let root = TempRoot::new();
    let mut run = reviewed(&root, &adds("a.txt"));
    let tree = run.tree();
    let before = run.checkout();
    // After the review: an edit to a tracked file, a new file that is staged, one that is not, and an ignored one.
    fs::write(tree.join("README"), "edited after the review\n").unwrap();
    fs::write(tree.join("staged.txt"), "staged\n").unwrap();
    git_out(&tree, &["add", "staged.txt"]);
    fs::write(tree.join("untracked.txt"), "untracked\n").unwrap();
    fs::write(tree.join(".gitignore"), "ignored.log\n").unwrap();
    fs::write(tree.join("ignored.log"), "ignored\n").unwrap();
    assert!(status(&tree).lines().count() >= 4, "{}", status(&tree));
    let last = run.branch_head();

    run.discarded();

    run.assert_removed();
    assert_eq!(run.checkout(), before);
    assert!(matches!(
        run.discard_steps().last(),
        Some(EventBody::StepOutcome { detail, .. }) if *detail == last
    ));
    assert_eq!(run.category(), Category::Dropped);
}

#[test]
fn a_worktree_with_commits_the_base_does_not_have_is_removed_too() {
    let root = TempRoot::new();
    let mut run = reviewed(&root, &adds("a.txt"));
    let tree = run.tree();
    fs::write(tree.join("more.txt"), "more\n").unwrap();
    git_out(&tree, &["add", "."]);
    git_out(&tree, &["commit", "-q", "-m", "a commit by hand"]);
    let last = run.branch_head();

    run.discarded();

    run.assert_removed();
    assert!(matches!(
        run.discard_steps().last(),
        Some(EventBody::StepOutcome { detail, .. }) if *detail == last
    ));
}

#[test]
fn a_worktree_folder_that_is_gone_is_discarded_all_the_same() {
    let root = TempRoot::new();
    let mut run = reviewed(&root, &adds("a.txt"));
    fs::remove_dir_all(run.tree()).unwrap();

    run.discarded();

    run.assert_removed();
    assert_eq!(run.category(), Category::Dropped);
}

// ---- runs that did not end well -----------------------------------------------------------------------------------

#[test]
fn a_failed_run_is_discarded_with_its_worktree() {
    let root = TempRoot::new();
    let mut run = failed_run(&root, None);

    run.discarded();

    run.assert_removed();
    assert_eq!(run.category(), Category::Dropped);
}

#[test]
fn a_stopped_run_is_discarded_with_its_worktree() {
    let root = TempRoot::new();
    let mut run = stopped_run(&root, None);

    run.discarded();

    run.assert_removed();
    assert_eq!(run.category(), Category::Dropped);
}

#[test]
fn a_run_stopped_for_disk_with_a_worktree_and_no_phase_is_discarded() {
    let root = TempRoot::new();
    // The readings: the workspace's volume, the run directory's, the workspace's own check, then the run directory
    // before the phase, which is under the floor.
    let mut run = stopped_for_disk(&root, &[u64::MAX, u64::MAX, u64::MAX, 0]);
    assert!(run.tree().is_dir());

    run.discarded();

    run.assert_removed();
    assert_eq!(run.category(), Category::Dropped);
}

#[test]
fn a_run_stopped_for_disk_before_any_worktree_was_made_is_discarded_with_nothing_to_remove() {
    let root = TempRoot::new();
    let mut run = stopped_for_disk(&root, &[0]);
    assert!(!run.tree().exists());

    run.discarded();

    assert!(run.fx.queue(&run.run).is_empty());
    assert_eq!(run.category(), Category::Dropped);
    assert_eq!(
        git_out(&run.fx.repo, &["branch", "--list", &run.branch()]),
        ""
    );
    assert!(
        run.discard_steps().is_empty(),
        "there was nothing to delete"
    );
}

// ---- what is refused ----------------------------------------------------------------------------------------------

#[test]
fn a_second_answer_to_a_discarded_item_is_refused_and_changes_nothing() {
    let root = TempRoot::new();
    let mut run = reviewed(&root, &adds("a.txt"));
    run.discarded();
    let before = (run.checkout(), run.fx.events().len());

    for reply in [
        QueueAnswer::Discard {},
        QueueAnswer::Accept {},
        QueueAnswer::Reprompt {
            prompt: "again".to_string(),
        },
    ] {
        let params = QueueAnswerParams {
            item: run.item.id.clone(),
            answer: reply,
        };
        let (refused, _) = rpc_error(&mut run.fx, "queue.answer", &params);
        assert_eq!(refused, code::NOT_FOUND);
    }

    assert_eq!((run.checkout(), run.fx.events().len()), before);
    assert_eq!(run.category(), Category::Dropped);
}

/// Makes a folder in the worktree that git cannot delete, until [`unlock`] is called on it.
fn lock(tree: &Path) -> PathBuf {
    let folder = tree.join("locked");
    fs::create_dir(&folder).unwrap();
    fs::write(folder.join("file"), "held\n").unwrap();
    fs::set_permissions(&folder, fs::Permissions::from_mode(0o500)).unwrap();
    folder
}

fn unlock(folder: &Path) {
    fs::set_permissions(folder, fs::Permissions::from_mode(0o700)).unwrap();
}

#[test]
fn a_removal_that_fails_is_a_refusal_with_the_item_back_and_nothing_else_changed() {
    let root = TempRoot::new();
    let mut run = reviewed(&root, &adds("a.txt"));
    let locked = lock(&run.tree());

    let result = run.discard();

    let QueueAnswerResult::Refused { message } = result else {
        unlock(&locked);
        panic!("the discard should be refused, and gave {result:?}");
    };
    assert!(!message.is_empty());
    assert_eq!(run.fx.queue(&run.run), std::slice::from_ref(&run.item));
    assert_eq!(run.category(), Category::Review);
    assert!(
        matches!(
            run.discard_steps().as_slice(),
            [
                EventBody::StepIntent { .. },
                EventBody::StepOutcome {
                    result: StepResult::Failed,
                    ..
                }
            ]
        ),
        "{:?}",
        run.discard_steps()
    );
    // The cause goes away, and the same item discards.
    unlock(&locked);
    run.discarded();
    run.assert_removed();
    assert_eq!(run.category(), Category::Dropped);
}

#[test]
fn a_dismissed_outcome_unknown_gives_back_a_review_item_that_can_be_discarded() {
    let root = TempRoot::new();
    let mut run = reviewed(&root, &adds("a.txt"));

    run.item = review_back_after_an_unknown_outcome(&mut run);
    run.discarded();

    run.assert_removed();
    assert_eq!(run.category(), Category::Dropped);
}

#[test]
fn discard_does_not_look_at_the_checkout() {
    let root = TempRoot::new();
    let mut run = reviewed(&root, &adds("a.txt"));
    // A merge in progress in the checkout refuses an accept, and does not matter to a discard.
    commit_file(&run.fx.repo, "README", "from main\n", "Edit README on main");
    git_out(&run.fx.repo, &["checkout", "-q", "-b", "other"]);
    let before = run.checkout();

    run.discarded();

    run.assert_removed();
    assert_eq!(run.checkout(), before);
    assert_eq!(
        git_out(&run.fx.repo, &["rev-parse", "--abbrev-ref", "HEAD"]),
        "other"
    );
}

// ---- a kill in the middle of a discard ----------------------------------------------------------------------------

/// Stops the daemon as a kill in the middle of a discard would leave it: the item is out of the Queue, the intent is in
/// the log, and the worktree's folder is deleted while its branch and git's record of it are not. With `outcome`, the
/// outcome of the step is in the log too.
fn killed_while_discarding(run: &mut Reviewed, outcome: Option<(StepResult, &str)>) {
    run.fx.stop_daemon();
    let store = Store::open(&run.fx.paths().store_dir).unwrap();
    store.delete_queue_item(&run.item.id).unwrap();
    let mut bodies = vec![EventBody::StepIntent {
        step: "collide/discard".to_string(),
        kind: StepKind::Discard,
        action: format!("delete the worktree and the branch {}", run.branch()),
    }];
    if let Some((result, detail)) = outcome {
        bodies.push(EventBody::StepOutcome {
            step: "collide/discard".to_string(),
            kind: StepKind::Discard,
            result,
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
    fs::remove_dir_all(run.tree()).unwrap();
}

impl Reviewed {
    /// What git still holds of the run's work: its branch, and its worktree's record.
    fn kept_by_git(&self) -> (String, bool) {
        (
            git_out(&self.fx.repo, &["branch", "--list", &self.branch()]),
            git_out(&self.fx.repo, &["worktree", "list"]).contains(&self.run),
        )
    }
}

#[test]
fn a_discard_a_kill_cut_short_has_its_review_item_back_at_the_next_start_and_nothing_is_deleted() {
    let root = TempRoot::new();
    let mut run = reviewed(&root, &adds("a.txt"));
    let before = run.kept_by_git();
    assert!(!before.0.is_empty() && before.1);
    killed_while_discarding(&mut run, None);

    run.fx.start_daemon();

    let back = wait_item(&mut run.fx, &run.run, QueueKind::Review);
    assert_eq!(
        (&back.id, &back.answers, &back.task),
        (&run.item.id, &run.item.answers, &run.item.task)
    );
    assert_eq!(run.fx.queue(&run.run), [back]);
    assert_eq!(run.kept_by_git(), before, "the start deleted nothing");
    assert_eq!(run.category(), Category::Review);
    assert!(matches!(
        run.discard_steps().as_slice(),
        [
            EventBody::StepIntent { .. },
            EventBody::StepOutcome {
                result: StepResult::Failed,
                ..
            }
        ]
    ));
}

#[test]
fn discard_on_the_item_a_kill_gave_back_finishes_the_deletion_and_writes_the_outcome() {
    let root = TempRoot::new();
    let mut run = reviewed(&root, &adds("a.txt"));
    let last = run.branch_head();
    killed_while_discarding(&mut run, None);
    run.fx.start_daemon();
    run.item = wait_item(&mut run.fx, &run.run, QueueKind::Review);

    run.discarded();

    run.assert_removed();
    assert_eq!(run.category(), Category::Dropped);
    let steps = run.discard_steps();
    assert_eq!(steps.len(), 4, "{steps:?}");
    assert!(matches!(
        &steps[3],
        EventBody::StepOutcome { result: StepResult::Succeeded, detail, .. } if *detail == last
    ));
    assert!(run.fx.queue(&run.run).is_empty());
}

#[test]
fn a_kill_after_the_outcome_but_before_the_run_was_recorded_removed_also_gives_the_item_back() {
    let root = TempRoot::new();
    let mut run = reviewed(&root, &adds("a.txt"));
    let last = run.branch_head();
    killed_while_discarding(&mut run, Some((StepResult::Succeeded, &last)));

    run.fx.start_daemon();

    run.item = wait_item(&mut run.fx, &run.run, QueueKind::Review);
    assert_eq!(run.category(), Category::Review);
    run.discarded();
    run.assert_removed();
    assert_eq!(run.category(), Category::Dropped);
}

#[test]
fn a_discard_that_finished_is_left_alone_at_the_next_start() {
    let root = TempRoot::new();
    let mut run = reviewed(&root, &adds("a.txt"));
    run.discarded();
    let before = (run.discard_steps(), run.fx.events().len());

    run.fx.stop_daemon();
    run.fx.start_daemon();

    assert!(run.fx.queue(&run.run).is_empty());
    assert_eq!((run.discard_steps(), run.fx.events().len()), before);
    assert_eq!(run.category(), Category::Dropped);
}

#[test]
fn a_discard_that_failed_is_left_alone_at_the_next_start() {
    let root = TempRoot::new();
    let mut run = reviewed(&root, &adds("a.txt"));
    let locked = lock(&run.tree());
    let refused = run.discard();
    assert!(matches!(refused, QueueAnswerResult::Refused { .. }));
    let before = (run.discard_steps(), run.fx.events().len());

    run.fx.stop_daemon();
    run.fx.start_daemon();

    assert_eq!(run.fx.queue(&run.run), std::slice::from_ref(&run.item));
    assert_eq!((run.discard_steps(), run.fx.events().len()), before);
    unlock(&locked);
}

#[test]
fn an_item_given_back_still_discards_when_the_task_is_already_dropped() {
    let root = TempRoot::new();
    let mut run = reviewed(&root, &adds("a.txt"));
    run.discarded();
    assert_eq!(run.category(), Category::Dropped);
    // As if the kill came before the run was recorded removed: the context says the worktree is idle, the log has the
    // step's outcome, and the item is gone.
    run.fx.stop_daemon();
    let store = Store::open(&run.fx.paths().store_dir).unwrap();
    let (name, json) = store.run_contexts().unwrap().remove(0);
    let stale = json.replace(
        "\"workspace_state\":\"removed\"",
        "\"workspace_state\":\"idle\"",
    );
    assert_ne!(stale, json);
    store.set_run_context(&name, &stale).unwrap();

    run.fx.start_daemon();

    run.item = wait_item(&mut run.fx, &run.run, QueueKind::Review);
    run.discarded();
    run.assert_removed();
    assert_eq!(run.category(), Category::Dropped);
}

// ---- a repository that is gone, and a run of an earlier daemon (ts-0160-th) ----------------------------------------

impl Reviewed {
    /// What the owner's check left: the repository's folder deleted, and the run's worktree under the daemon's own
    /// `workspaces` folder as it was.
    fn delete_the_repository(&self) {
        fs::remove_dir_all(&self.fx.repo).unwrap();
    }

    /// Stops the daemon and starts it again, as a run of an earlier daemon process meets the next one.
    fn restart(&mut self) {
        self.fx.stop_daemon();
        self.fx.start_daemon();
    }

    /// Stops the daemon, changes the folder its store records for the run's worktree, and starts it again.
    fn record_the_folder_as(&mut self, folder: &Path) {
        self.fx.stop_daemon();
        let store = Store::open(&self.fx.paths().store_dir).unwrap();
        let (name, json) = store.run_contexts().unwrap().remove(0);
        let mut context: serde_json::Value = serde_json::from_str(&json).unwrap();
        context["workspace_path"] = folder.to_str().unwrap().into();
        store.set_run_context(&name, &context.to_string()).unwrap();
        self.fx.start_daemon();
    }

    /// The refusal of a discard, with the item still in the Queue and no step written.
    fn refused_at_the_door(&mut self) -> String {
        let QueueAnswerResult::Refused { message } = self.discard() else {
            panic!("the discard should be refused");
        };
        assert_eq!(self.fx.queue(&self.run), std::slice::from_ref(&self.item));
        assert!(
            self.discard_steps().is_empty(),
            "{:?}",
            self.discard_steps()
        );
        message
    }

    /// A folder of the owner's own, outside the daemon's `workspaces` folder, with one file in it.
    fn somewhere_else(&self) -> (PathBuf, PathBuf) {
        let folder = self.fx.root.join("somewhere-else");
        fs::create_dir(&folder).unwrap();
        let file = folder.join("mine.txt");
        fs::write(&file, "not the run's\n").unwrap();
        (folder, file)
    }
}

#[test]
fn a_review_item_whose_repository_is_gone_is_discarded_and_its_folder_deleted() {
    let root = TempRoot::new();
    let mut run = reviewed(&root, &adds("a.txt"));
    let branch = run.branch();
    run.delete_the_repository();

    let result = run.discard();

    let QueueAnswerResult::Discarded { note } = result else {
        panic!("the discard should go through with a note, and gave {result:?}");
    };
    assert!(
        note.contains("repository") && note.contains(&branch),
        "{note}"
    );
    assert!(!run.tree().exists());
    assert!(run.fx.queue(&run.run).is_empty());
    assert_eq!(run_get(&mut run.fx, &run.run).summary.workspace, None);
    assert_eq!(run.category(), Category::Dropped);
}

#[test]
fn a_discard_with_the_repository_gone_writes_its_intent_and_outcome() {
    let root = TempRoot::new();
    let mut run = reviewed(&root, &adds("a.txt"));
    let folder = run.tree();
    run.delete_the_repository();

    let result = run.discard();

    assert!(
        matches!(result, QueueAnswerResult::Discarded { .. }),
        "{result:?}"
    );
    match run.discard_steps().as_slice() {
        [
            EventBody::StepIntent { action, .. },
            EventBody::StepOutcome { result, .. },
        ] => {
            assert!(action.contains(folder.to_str().unwrap()), "{action}");
            assert!(
                action.contains("repository") && action.contains("gone"),
                "{action}"
            );
            assert_eq!(*result, StepResult::Succeeded);
        }
        other => panic!("an intent and an outcome were expected: {other:?}"),
    }
    let moves = workspace_moves(&run.fx.events());
    assert_eq!(
        moves[moves.len() - 2..],
        [WorkspaceState::Removable, WorkspaceState::Removed]
    );
}

#[test]
fn a_run_of_an_earlier_daemon_whose_repository_is_gone_is_discarded_after_a_restart() {
    let root = TempRoot::new();
    let mut run = reviewed(&root, &adds("a.txt"));
    run.fx.stop_daemon();
    run.delete_the_repository();
    run.fx.start_daemon();

    let result = run.discard();

    assert!(
        matches!(result, QueueAnswerResult::Discarded { .. }),
        "{result:?}"
    );
    assert!(!run.tree().exists());
    assert!(run.fx.queue_all().is_empty());
}

#[test]
fn a_run_of_an_earlier_daemon_is_discarded_after_a_restart_with_the_repository_in_place() {
    let root = TempRoot::new();
    let mut run = reviewed(&root, &adds("a.txt"));
    run.restart();

    run.discarded();

    run.assert_removed();
    assert_eq!(run.category(), Category::Dropped);
    assert!(run.fx.queue_all().is_empty());
}

#[test]
fn a_worktree_folder_that_is_gone_with_the_repository_is_discarded_all_the_same() {
    let root = TempRoot::new();
    let mut run = reviewed(&root, &adds("a.txt"));
    fs::remove_dir_all(run.tree()).unwrap();
    run.delete_the_repository();

    let result = run.discard();

    assert!(
        matches!(result, QueueAnswerResult::Discarded { .. }),
        "{result:?}"
    );
    assert!(run.fx.queue_all().is_empty());
}

// ---- deleting a folder by a recorded path -------------------------------------------------------------------------

#[test]
fn a_recorded_folder_outside_the_workspaces_folder_is_refused_by_name_and_left_alone() {
    let root = TempRoot::new();
    let mut run = reviewed(&root, &adds("a.txt"));
    let (elsewhere, file) = run.somewhere_else();
    run.delete_the_repository();
    run.record_the_folder_as(&elsewhere);

    let message = run.refused_at_the_door();

    assert!(message.contains(elsewhere.to_str().unwrap()), "{message}");
    assert!(message.contains("workspaces"), "{message}");
    assert!(file.is_file(), "the folder was deleted");
}

#[test]
fn a_recorded_folder_outside_the_workspaces_folder_is_refused_with_the_repository_in_place_too() {
    let root = TempRoot::new();
    let mut run = reviewed(&root, &adds("a.txt"));
    let (elsewhere, file) = run.somewhere_else();
    run.record_the_folder_as(&elsewhere);

    let message = run.refused_at_the_door();

    assert!(message.contains(elsewhere.to_str().unwrap()), "{message}");
    assert!(file.is_file(), "the folder was deleted");
    assert!(!git_out(&run.fx.repo, &["branch", "--list", &run.branch()]).is_empty());
}

#[test]
fn the_workspaces_folder_itself_and_another_runs_folder_are_refused() {
    let root = TempRoot::new();
    let mut run = reviewed(&root, &adds("a.txt"));
    run.delete_the_repository();
    let workspaces = run.tree().parent().unwrap().to_path_buf();
    let other = workspaces.join("run-not-this-one");
    fs::create_dir(&other).unwrap();
    fs::write(other.join("theirs.txt"), "another run's\n").unwrap();

    for folder in [workspaces.clone(), other.clone()] {
        run.record_the_folder_as(&folder);
        let message = run.refused_at_the_door();
        assert!(message.contains(folder.to_str().unwrap()), "{message}");
    }

    assert!(other.join("theirs.txt").is_file());
    assert!(run.tree().is_dir());
}

#[cfg(unix)]
fn link_the_folder_to(run: &Reviewed, target: &Path) {
    fs::remove_dir_all(run.tree()).unwrap();
    std::os::unix::fs::symlink(target, run.tree()).unwrap();
}

#[test]
fn a_run_folder_that_is_a_symlink_out_of_the_workspaces_folder_is_refused_and_not_followed() {
    let root = TempRoot::new();
    let mut run = reviewed(&root, &adds("a.txt"));
    let (elsewhere, file) = run.somewhere_else();
    run.fx.stop_daemon();
    link_the_folder_to(&run, &elsewhere);
    run.delete_the_repository();
    run.fx.start_daemon();

    let message = run.refused_at_the_door();

    assert!(message.contains(run.tree().to_str().unwrap()), "{message}");
    assert!(file.is_file(), "the link was followed");
    assert!(fs::symlink_metadata(run.tree()).unwrap().is_symlink());
}

#[test]
fn a_run_folder_that_is_a_symlink_out_of_the_workspaces_folder_is_refused_with_the_repository_in_place_too()
 {
    let root = TempRoot::new();
    let mut run = reviewed(&root, &adds("a.txt"));
    let (elsewhere, file) = run.somewhere_else();
    run.fx.stop_daemon();
    link_the_folder_to(&run, &elsewhere);
    run.fx.start_daemon();

    let message = run.refused_at_the_door();

    assert!(message.contains(run.tree().to_str().unwrap()), "{message}");
    assert!(file.is_file(), "the link was followed");
    assert!(fs::symlink_metadata(run.tree()).unwrap().is_symlink());
}

#[test]
fn a_symlink_inside_the_worktree_that_points_out_is_removed_and_not_followed() {
    let root = TempRoot::new();
    let mut run = reviewed(&root, &adds("a.txt"));
    let (elsewhere, file) = run.somewhere_else();
    std::os::unix::fs::symlink(&elsewhere, run.tree().join("out")).unwrap();
    run.delete_the_repository();

    let result = run.discard();

    assert!(
        matches!(result, QueueAnswerResult::Discarded { .. }),
        "{result:?}"
    );
    assert!(!run.tree().exists());
    assert!(file.is_file(), "the link was followed");
}
