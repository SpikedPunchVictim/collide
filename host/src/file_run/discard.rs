//! Discard on a prompt run's `Review` item (ts-0080-tq; Stage 3c's pull from Stage 9a). **Stage 7's workspace hooks and
//! Stage 9a replace this**, with the work that never lands (0.4).
//!
//! `queue.answer` with `discard` deletes what the run made and drops the task:
//!
//! 1. **The step** (0.2). A `step.intent` event says what will go, with the branch's last commit; then the worktree's
//!    folder and the branch are deleted ([`crate::workspace::BuiltinWorkspace::discard`]), with whatever the worktree
//!    holds, committed or not; then a `step.outcome` event with the same last commit as its detail. A person can make a
//!    branch of that commit (`git branch <name> <commit>`) until git collects it. The step is `collide/discard`.
//! 2. **The task** goes to the category [`QUICK`] names for a discard, by category and never by a status name.
//!
//! The board's checkout is never touched, and nothing here reads its state: a checkout in the middle of a merge, or on
//! another branch, changes nothing. The run's context stays in the store, so `run.get` still knows the run, with its
//! workspace `removed`. The run's own state is as it was (`ended`, `failed` or `stopped`).
//!
//! ## A restart in the middle of a discard
//!
//! The item is taken out of the Queue before anything is done, so after a kill it is gone, and the log may hold an
//! intent with no outcome and a worktree that is half deleted. [`FileRuns::recover_discards`] reads the log at
//! start (0.2) and puts the review item back for each run whose discard did not finish: the newest
//! `collide/discard` event is an intent, or an outcome that `succeeded` while the run's context does not say the
//! worktree is removed. It never deletes anything, and never moves the task: a discard is safe to repeat, so the
//! person answers discard again, and that finishes the deletion (a folder that is gone, a worktree git still knows,
//! a branch that is still there) and writes its own intent and outcome. An open intent is closed first with a
//! `failed` outcome that says so. An item answered with a discard that had dropped the task already works, since
//! dropping a dropped task changes nothing.
//!
//! ## The folder, and a repository that is gone (ts-0160-th)
//!
//! The folder is deleted by the path the run recorded, so [`FileRuns::check_folder`] looks first: once every link in
//! the path is resolved, it must be `workspaces/<name>` of the daemon's own `workspaces` folder, the run's own, and the
//! recorded path must not be a link itself. Otherwise the discard is refused, with words that name the path, before
//! any event is written and before anything is deleted. A link inside the worktree is deleted as a link and never
//! followed.
//!
//! A run whose repository's folder is gone (the clone was deleted) has no branch to read and none to delete: the
//! step's intent says so, only the folder is deleted, and the answer is `discarded` with a note that the branch went
//! with the repository. That holds for a run of an earlier daemon process too, which a restart reads back from the
//! store like any other.
//!
//! The item is taken first and put back when the removal fails (a file that cannot be deleted): the outcome is `failed`
//! with the cause, nothing else changes, and the same item is answered again once the cause is gone. A run whose
//! worktree was never made (stopped for disk before it) has nothing to delete: it is dropped with no step.

use super::quick_run::QUICK;
use super::{FileRuns, Live, locked};
use crate::phases::Phases;
use collide_core::events::{Event, EventBody, StepKind, StepResult};
use collide_core::owner::{QueueAnswerResult, QueueItem};
use collide_core::state::{RunState, WorkspaceState};
use std::fs;
use std::io;
use std::path::{Component, Path};
use std::sync::{Arc, Mutex};

/// The name of the discard step in the event log.
const STEP: &str = "collide/discard";

impl FileRuns {
    /// `queue.answer` with `discard` on a run's `Review` item. The item is taken out first, so two discards never both
    /// delete, and is put back when the discard is refused.
    pub(super) fn discard(&self, phases: &Phases, item_id: &str) -> QueueAnswerResult {
        let Some(item) = self.config.queue.take(item_id) else {
            return QueueAnswerResult::Refused {
                message: format!("item {item_id:?} was answered already"),
            };
        };
        match self.try_discard(phases, &item) {
            Ok(None) => QueueAnswerResult::Done {},
            Ok(Some(note)) => QueueAnswerResult::Discarded { note },
            Err(message) => {
                self.config.queue.restore(item);
                QueueAnswerResult::Refused { message }
            }
        }
    }

    /// The step, then the task. An `Err` is a refusal: the worktree and the task are as they were. An `Ok` holds the
    /// note for the person, when there is one.
    fn try_discard(&self, phases: &Phases, item: &QueueItem) -> Result<Option<String>, String> {
        let run = item.run.clone().unwrap_or_default();
        let live = self
            .live_of(&run)
            .ok_or_else(|| format!("the daemon has no run {run:?}"))?;
        let mut live = locked(&live);
        let quick = live.ctx.quick.clone().ok_or_else(|| {
            format!("run {run} is not a prompt run, so there is nothing to discard")
        })?;
        if let Some(next) = &quick.followed_by {
            return Err(format!(
                "run {run} was re-prompted, and {next} holds its worktree now: discard that run"
            ));
        }
        if !matches!(
            live.ctx.run_state,
            RunState::Ended { .. } | RunState::Failed {} | RunState::Stopped {}
        ) {
            return Err(format!(
                "run {run} has not ended, so it cannot be discarded"
            ));
        }
        // `provisioning` and `failed` mean no worktree was made, and `removed` that it is gone.
        let mut note = None;
        if !matches!(
            live.workspace.state(),
            WorkspaceState::Provisioning | WorkspaceState::Failed | WorkspaceState::Removed
        ) {
            note = self.delete_work(phases, &mut live)?;
        }
        if let Err(message) = self.move_task(phases, &quick, QUICK.on_discarded, &run) {
            self.log(&format!(
                "run {run}: discarded, but {} was not dropped: {message}",
                quick.task
            ));
        }
        Ok(note)
    }

    /// Refuses a folder that is not this run's own inside the daemon's `workspaces` folder. The path is the one the
    /// run recorded, and a store that was edited, or a folder that was swapped for a link, must never make the daemon
    /// delete something else. The folder must be, once every link in its path is resolved, `workspaces/<name>` and
    /// nothing above or beside it; a link as the folder itself is refused without being followed. A folder that is
    /// not there has nothing to delete.
    fn check_folder(&self, live: &Live) -> Result<(), String> {
        let recorded = Path::new(&live.ctx.workspace_path);
        let refuse = |why: String| {
            Err(format!(
                "{} was not deleted, because {why}. Nothing was changed.",
                recorded.display()
            ))
        };
        match fs::symlink_metadata(recorded) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return refuse(format!("it could not be read: {error}")),
            Ok(meta) if meta.file_type().is_symlink() => {
                return refuse("it is a symbolic link, which is never followed".to_string());
            }
            Ok(_) => {}
        }
        let workspaces = &self.config.workspaces_dir;
        let name = live.ctx.workspace_name();
        let plain = matches!(
            Path::new(name).components().collect::<Vec<_>>().as_slice(),
            [Component::Normal(_)]
        );
        if !plain {
            return refuse(format!(
                "the run's folder name {name:?} is not a plain name"
            ));
        }
        let (Ok(folder), Ok(inside)) = (fs::canonicalize(recorded), fs::canonicalize(workspaces))
        else {
            return refuse(format!(
                "it or the daemon's workspaces folder {} could not be resolved",
                workspaces.display()
            ));
        };
        if folder != inside.join(name) {
            return refuse(format!(
                "it is not the folder {name} inside the daemon's workspaces folder {}",
                inside.display()
            ));
        }
        Ok(())
    }

    /// The step: its intent with the branch's last commit, the deletion, and its outcome. The note is for a repository
    /// that is gone: its branch went with it, and only the folder was deleted.
    fn delete_work(&self, phases: &Phases, live: &mut Live) -> Result<Option<String>, String> {
        let run = live.ctx.run.clone();
        let branch = live.ctx.branch.clone();
        self.check_folder(live)?;
        let gone = live.workspace.repository_gone();
        let head = if gone {
            None
        } else {
            live.workspace
                .branch_head()
                .map_err(|error| format!("Git could not read the branch `{branch}`: {error}"))?
        };
        let folder = live.ctx.workspace_path.clone();
        let repo = live.ctx.repo.clone();
        let action = match &head {
            _ if gone => format!(
                "delete the worktree {folder}; the repository {repo} is gone, and the branch {branch} with it"
            ),
            Some(commit) => format!(
                "delete the worktree {folder} and the branch {branch}, whose last commit is {commit}"
            ),
            None => format!("delete the worktree {folder}; the branch {branch} is not there"),
        };
        self.write_event(
            phases,
            &run,
            EventBody::StepIntent {
                step: STEP.to_string(),
                kind: StepKind::Discard,
                action,
            },
        )
        .map_err(|error| {
            format!("nothing was deleted, because the discard's intent was not recorded: {error}")
        })?;

        let before = live.workspace.state();
        let discarded = live.workspace.discard();
        self.record_removal(phases, live, before);
        let (result, detail) = match &discarded {
            Ok(()) => (StepResult::Succeeded, head.unwrap_or_default()),
            Err(error) => (StepResult::Failed, error.to_string()),
        };
        let written = self.write_event(
            phases,
            &run,
            EventBody::StepOutcome {
                step: STEP.to_string(),
                kind: StepKind::Discard,
                result,
                detail,
            },
        );
        if let Err(error) = written {
            self.log(&format!(
                "run {run}: the discard's outcome was not recorded: {error}"
            ));
        }
        // The context is stored after the outcome, so a kill between the two leaves a context that says the worktree is
        // not removed, and the next start gives the item back ([`FileRuns::recover_discards`]).
        self.sync(live);
        discarded.map_err(|error| {
            format!(
                "The run's work was not deleted: {error}. Put the cause right and discard again."
            )
        })?;
        Ok(gone.then(|| {
            format!(
                "The repository {repo} is gone, so the branch {branch} went with it. \
                 The agent's copy of the project was deleted."
            )
        }))
    }
}

/// What the log says about a run's last discard.
enum Unsettled {
    /// An intent with no outcome.
    Open,
    /// An outcome that `succeeded`, and a context that does not say the worktree is removed.
    NotRecorded,
}

fn unsettled(own: &[Event], recorded_removed: bool) -> Option<Unsettled> {
    // The newest event of the step decides.
    let newest = own.iter().rev().find(|event| {
        matches!(
            event.body,
            EventBody::StepIntent {
                kind: StepKind::Discard,
                ..
            } | EventBody::StepOutcome {
                kind: StepKind::Discard,
                ..
            }
        )
    })?;
    match &newest.body {
        EventBody::StepIntent { .. } => Some(Unsettled::Open),
        EventBody::StepOutcome { result, .. } => (*result == StepResult::Succeeded
            && !recorded_removed)
            .then_some(Unsettled::NotRecorded),
        _ => None,
    }
}

impl FileRuns {
    /// After a start: puts back the review item of each prompt run whose discard did not finish (see the module notes).
    /// The log is read once, and only when a run that has a worktree is waiting on its review.
    pub(super) fn recover_discards(&self, phases: &Phases) {
        let waiting: Vec<Arc<Mutex<Live>>> = locked(&self.live)
            .values()
            .filter(|live| {
                let live = locked(live);
                live.ctx
                    .quick
                    .as_ref()
                    .is_some_and(|quick| quick.followed_by.is_none())
                    && matches!(
                        live.ctx.run_state,
                        RunState::Ended { .. } | RunState::Failed {} | RunState::Stopped {}
                    )
                    && !matches!(
                        live.ctx.workspace_state,
                        WorkspaceState::Provisioning | WorkspaceState::Failed
                    )
            })
            .cloned()
            .collect();
        if waiting.is_empty() {
            return;
        }
        let events = match self.config.store.events_since(0) {
            Ok(events) => events,
            Err(error) => {
                self.log(&format!(
                    "the log could not be read to settle discards: {error}"
                ));
                return;
            }
        };
        for live in waiting {
            let live = locked(&live);
            let run = live.ctx.run.clone();
            let own: Vec<Event> = events
                .iter()
                .filter(|event| event.actor.run.as_deref() == Some(run.as_str()))
                .cloned()
                .collect();
            let removed = live.ctx.workspace_state == WorkspaceState::Removed;
            let Some(found) = unsettled(&own, removed) else {
                continue;
            };
            let Some(quick) = live.ctx.quick.clone() else {
                continue;
            };
            self.log(&format!(
                "run {run}: its discard did not finish; the review is back"
            ));
            if matches!(found, Unsettled::Open) {
                let closed = self.write_event(
                    phases,
                    &run,
                    EventBody::StepOutcome {
                        step: STEP.to_string(),
                        kind: StepKind::Discard,
                        result: StepResult::Failed,
                        detail: "the daemon stopped before the discard finished; discard again to finish it"
                            .to_string(),
                    },
                );
                if let Err(error) = closed {
                    self.log(&format!("run {run}: the outcome was not recorded: {error}"));
                }
            }
            let title = self.review_title(&run, &live.ctx.run_state);
            let done = matches!(live.ctx.run_state, RunState::Ended { .. });
            self.push_review(&run, &quick.task, done, &title);
        }
    }
}
