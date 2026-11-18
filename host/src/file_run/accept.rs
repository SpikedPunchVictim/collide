//! Accept on a prompt run's `Review` item: `collide/merge-local` in its Stage 3c form (ts-0079-pu). **Stage 7's
//! `merge-local` hook and Stage 9a's accept replace this**, with the engine, the gates and the source's operations.
//!
//! `queue.answer` with `accept` merges the run's branch into the board's checkout, in this order:
//!
//! 1. **The check.** `git merge-tree --write-tree` of the base branch and the run's branch ([`MergeCheck`]). A conflict
//!    refuses the accept and names the files. A clean merge that makes the base's own tree means the base already has
//!    the change (merged, squash merged, rebased, or no change): nothing is merged, and it is accepted as merged.
//! 2. **The checkout.** It must be on the run's base branch, with no rebase, merge, cherry-pick, revert or bisect in
//!    progress, or the accept is refused and says what to do. (0.4 says the merge goes into the base ref, and Stage 3c
//!    into the checkout. They agree only for a checkout on the base branch, so another is refused with the branch to
//!    switch to: owner decision, 2026-11-15.) The run's worktree must be clean too, so what is merged is what was
//!    reviewed.
//! 3. **The step** (0.2). A `step.intent` event, then `git merge --no-edit` in the checkout, then a `step.outcome`
//!    event. The merge is git's own, so the repository's hooks run and nothing bypasses them; a hook that refuses is
//!    a refusal, and the half-made merge is undone. A merge git refuses (a local change it would overwrite) is a
//!    refusal too, with git's words, and a `failed` outcome.
//! 4. **The run.** The answer is applied through `collide_core::run_rules::apply_integrate_answer`, and what it
//!    releases follows: the run's `after_integrate` handlers (none until Stage 7) and the source's operations, here
//!    the task's move to done ([`QUICK`]).
//! 5. **The worktree** is removed and its branch deleted, with the removal checked against the base *branch*: the
//!    recorded base is a commit (ts-0078-ye), and the merged commits are not ahead of the branch.
//!
//! A refused accept puts the item back, so the person can fix the cause, re-prompt or discard.
//!
//! ## A restart between the intent and the outcome
//!
//! The item is taken out of the Queue before anything is done, so after a kill it is gone and the log holds an intent
//! with no outcome. [`FileRuns::recover_accepts`] reads the checkout with the step's own check, and never repeats the
//! merge before that (0.2):
//!
//! | the checkout shows | what happens |
//! |---|---|
//! | the base already has the run's change | a `succeeded` outcome, and the removal finishes |
//! | `MERGE_HEAD` (a merge was cut short) | an `OutcomeUnknown` item; nothing is touched |
//! | anything else | a `failed` outcome, and the `Review` item is back |
//!
//! A kill between the outcome and the removal finishes the removal. The person answers an `OutcomeUnknown` item with
//! Dismiss after fixing the checkout, which puts the review item back.

use super::quick_run::{QUICK, QuickRun, READY_TITLE};
use super::{FileRuns, Live, locked};
use crate::checkout::root_busy;
use crate::phases::Phases;
use crate::workspace::{Git, MergeCheck, WorkspaceError};
use collide_core::change::{Actor, ActorKind};
use collide_core::events::{Event, EventBody, StepKind, StepResult};
use collide_core::non_empty::NonEmpty;
use collide_core::owner::{AnswerKind, QueueAnswerResult, QueueItem, QueueKind};
use collide_core::run_rules::{
    AfterIntegrate, CheckWorld, Finding, IntegrateAnswer, Settled, apply_integrate_answer, recover,
    settle,
};
use collide_core::state::{RunState, WorkspaceState};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// The name of the integrate step in the event log.
const STEP: &str = "collide/merge-local";

/// The refs a run's merge is about.
struct Refs {
    /// `refs/heads/<base branch>`, so a tag of the same name is never read for it.
    base: String,
    branch: String,
}

impl Refs {
    fn of(live: &Live, quick: &QuickRun) -> Refs {
        Refs {
            base: format!("refs/heads/{}", quick.base_branch),
            branch: format!("refs/heads/{}", live.ctx.branch),
        }
    }
}

fn git_failed(error: &WorkspaceError) -> String {
    format!("Git could not be asked about the checkout: {error}")
}

impl FileRuns {
    /// `queue.answer` with `accept` on a run's `Review` item. The item is taken out first, so two accepts never both
    /// merge, and is put back when the accept is refused.
    pub(super) fn accept(&self, phases: &Phases, item_id: &str) -> QueueAnswerResult {
        let Some(item) = self.config.queue.take(item_id) else {
            return QueueAnswerResult::Refused {
                message: format!("item {item_id:?} was answered already"),
            };
        };
        match self.try_accept(phases, &item) {
            Ok(commit) => QueueAnswerResult::Merged { commit },
            Err(message) => {
                self.config.queue.restore(item);
                QueueAnswerResult::Refused { message }
            }
        }
    }

    /// The merge, up to and including the run's end. An `Err` is a refusal: nothing in the checkout changed.
    fn try_accept(&self, phases: &Phases, item: &QueueItem) -> Result<String, String> {
        let run = item.run.clone().unwrap_or_default();
        let live = self
            .live_of(&run)
            .ok_or_else(|| format!("the daemon has no run {run:?}"))?;
        let mut live = locked(&live);
        let quick = live.ctx.quick.clone().ok_or_else(|| {
            format!("run {run} is not a prompt run, so there is nothing to merge")
        })?;
        if !matches!(live.ctx.run_state, RunState::Ended { .. }) {
            return Err(format!("run {run} has not ended, so it cannot be accepted"));
        }
        // One merge at a time: two runs of one checkout would otherwise meet in git's own lock.
        let _merging = locked(&self.merging);
        let git = Git::with_env(self.config.git_env.clone());
        let checkout = PathBuf::from(&live.ctx.repo);
        let refs = Refs::of(&live, &quick);

        let already = self.check_merge(&git, &checkout, &live, &quick, &refs)?;
        live.workspace
            .refuse_dirty()
            .map_err(|error| format!("{error}. Nothing was merged."))?;

        let action = format!("merge {} into {}", live.ctx.branch, quick.base_branch);
        self.write_event(
            phases,
            &run,
            EventBody::StepIntent {
                step: STEP.to_string(),
                kind: StepKind::Integrate,
                action,
            },
        )
        .map_err(|error| {
            format!("the merge was not started, because its intent was not recorded: {error}")
        })?;

        let merged = if already {
            git.text(&checkout, &["rev-parse", "--verify", &refs.base])
                .map_err(|error| git_failed(&error))
        } else {
            self.merge(&git, &checkout, &live, &quick)
        };
        let commit = match merged {
            Ok(commit) => commit,
            Err(message) => {
                self.write_outcome(phases, &run, StepResult::Failed, &message);
                return Err(message);
            }
        };
        self.write_outcome(phases, &run, StepResult::Succeeded, &commit);
        self.finish_merged(phases, &mut live, &commit);
        Ok(commit)
    }

    /// Steps 1 and 2: the merge check, then the checkout. `Ok(true)` when the base already has the change.
    fn check_merge(
        &self,
        git: &Git,
        checkout: &Path,
        live: &Live,
        quick: &QuickRun,
        refs: &Refs,
    ) -> Result<bool, String> {
        let branch = &live.ctx.branch;
        let base = &quick.base_branch;
        let found = git
            .merge_check(checkout, &refs.base, &refs.branch)
            .map_err(|error| {
                format!("Git could not check the merge of `{branch}` into `{base}`: {error}")
            })?;
        if let MergeCheck::Conflict { files } = &found {
            return Err(format!(
                "Merging `{branch}` into `{base}` would conflict in {}. Nothing was merged: re-prompt the agent to \
                 resolve it, or discard the run.",
                files.join(", ")
            ));
        }
        let root = checkout.display();
        if let Some(what) =
            root_busy(&self.config.git_env, checkout).map_err(|error| git_failed(&error))?
        {
            return Err(format!(
                "The checkout at {root} is in the middle of {what}. Finish it or abort it, then accept again. \
                 Nothing was merged."
            ));
        }
        let current = crate::boards::checkout_of(checkout, &self.config.git_env)?.branch;
        if current != *base {
            let on = if current == "HEAD" {
                "is not on a branch (its HEAD is detached)".to_string()
            } else {
                format!("is on `{current}`")
            };
            return Err(format!(
                "The checkout at {root} {on}, and this run's changes are merged into `{base}`. Switch it with \
                 `git switch {base}`, then accept again. Nothing was merged."
            ));
        }
        Ok(found == MergeCheck::InBase)
    }

    /// `git merge --no-edit` in the checkout. A merge that stops with the merge state written (a hook that refused
    /// the commit) is aborted, so the checkout is as it was.
    fn merge(
        &self,
        git: &Git,
        checkout: &Path,
        live: &Live,
        quick: &QuickRun,
    ) -> Result<String, String> {
        let branch = &live.ctx.branch;
        if let Err(error) = git.run(checkout, &["merge", "--no-edit", branch]) {
            let WorkspaceError::Git { stderr, .. } = &error else {
                return Err(error.to_string());
            };
            if matches!(merge_head_exists(git, checkout), Ok(true))
                && let Err(abort) = git.run(checkout, &["merge", "--abort"])
            {
                self.log(&format!(
                    "run {}: the merge stopped and could not be aborted: {abort}",
                    live.ctx.run
                ));
            }
            return Err(format!(
                "Git did not merge `{branch}` into `{}`: {stderr}\nThe checkout is as it was.",
                quick.base_branch
            ));
        }
        git.text(checkout, &["rev-parse", "--verify", "HEAD"])
            .map_err(|error| git_failed(&error))
    }

    /// What follows a merge, whether this accept made it or an earlier daemon did: the integrate answer applied to the
    /// run, the task's move, and the worktree's removal. Nothing here refuses: the merge is done, so a failure is
    /// logged.
    fn finish_merged(&self, phases: &Phases, live: &mut Live, commit: &str) {
        let run = live.ctx.run.clone();
        let Some(quick) = live.ctx.quick.clone() else {
            return;
        };
        let (RunState::Ended { outcome }, Ok(commit)) =
            (live.ctx.run_state.clone(), NonEmpty::new(commit))
        else {
            self.log(&format!("run {run}: merged, but the run is not ended"));
            return;
        };
        let answer = IntegrateAnswer::Merged { commit };
        match apply_integrate_answer(&live.ctx.run_state, &answer, &outcome) {
            Ok(step) => {
                if step.run != live.ctx.run_state {
                    self.record_run_state(phases, live, step.run);
                }
                for after in step.release {
                    match after {
                        // The run's `after_integrate` handlers are Stage 7's: there are none to run.
                        AfterIntegrate::RunAfterIntegrate => {}
                        AfterIntegrate::SourceOps => {
                            if let Err(message) =
                                self.move_task(phases, &quick, QUICK.on_merged, &run)
                            {
                                self.log(&format!(
                                    "run {run}: {} was not moved to done: {message}",
                                    quick.task
                                ));
                            }
                        }
                    }
                }
            }
            Err(error) => self.log(&format!("run {run}: {error}")),
        }
        self.remove_worktree(phases, live, &quick);
    }

    /// Removes the worktree and the branch. The base it is checked against is the base branch.
    fn remove_worktree(&self, phases: &Phases, live: &mut Live, quick: &QuickRun) {
        let run = live.ctx.run.clone();
        let before = live.workspace.state();
        live.workspace
            .set_base(&format!("refs/heads/{}", quick.base_branch));
        let removed = live.workspace.remove();
        self.record_removal(phases, live, before);
        if let Err(error) = removed {
            self.log(&format!(
                "run {run}: merged, but its worktree was not removed: {error}"
            ));
        }
        self.sync(live);
    }

    /// Records the workspace's move from `before` to where a removal left it. `idle` and `ready` go through
    /// `removable`, which is two moves.
    pub(super) fn record_removal(&self, phases: &Phases, live: &Live, before: WorkspaceState) {
        let run = &live.ctx.run;
        let name = live.ctx.workspace_name();
        let after = live.workspace.state();
        if after == before {
            return;
        }
        if after == WorkspaceState::Removed && before != WorkspaceState::Removable {
            self.record_workspace(phases, run, name, Some(before), WorkspaceState::Removable);
            self.record_workspace(
                phases,
                run,
                name,
                Some(WorkspaceState::Removable),
                WorkspaceState::Removed,
            );
        } else {
            self.record_workspace(phases, run, name, Some(before), after);
        }
    }

    pub(super) fn write_event(
        &self,
        phases: &Phases,
        run: &str,
        body: EventBody,
    ) -> Result<(), String> {
        phases
            .append_event(
                Actor {
                    kind: ActorKind::Workflow,
                    run: Some(run.to_string()),
                    phase: None,
                },
                body,
            )
            .map_err(|error| error.to_string())
    }

    fn write_outcome(&self, phases: &Phases, run: &str, result: StepResult, detail: &str) {
        let written = self.write_event(
            phases,
            run,
            EventBody::StepOutcome {
                step: STEP.to_string(),
                kind: StepKind::Integrate,
                result,
                detail: detail.to_string(),
            },
        );
        if let Err(error) = written {
            self.log(&format!(
                "run {run}: the merge's outcome was not recorded: {error}"
            ));
        }
    }

    /// Puts the review item back, for a run that is done.
    fn reopen_review(&self, run: &str, task: &str) {
        self.push_review(run, task, true, READY_TITLE);
    }

    /// `queue.answer` with Dismiss on an `OutcomeUnknown` item: the person has looked at the checkout, so the open
    /// step is closed as failed and the review item comes back. The next accept checks the checkout again.
    pub(super) fn dismiss_unknown(&self, phases: &Phases, item: &QueueItem) {
        let Some(run) = item.run.as_deref() else {
            return;
        };
        let Some(task) = self.live_of(run).and_then(|live| {
            locked(&live)
                .ctx
                .quick
                .as_ref()
                .map(|quick| quick.task.clone())
        }) else {
            return;
        };
        self.write_outcome(
            phases,
            run,
            StepResult::Failed,
            "the outcome was unknown, and the person dismissed it",
        );
        self.reopen_review(run, &task);
    }

    /// After a start: settles each accepted run an earlier daemon left half-way (see the module notes). The log is read
    /// once, and only when a run is waiting on its review.
    pub(super) fn recover_accepts(&self, phases: &Phases) {
        let waiting: Vec<Arc<Mutex<Live>>> = locked(&self.live)
            .values()
            .filter(|live| {
                let live = locked(live);
                // A run a re-prompt followed gave its worktree to the new run, which settles it.
                live.ctx
                    .quick
                    .as_ref()
                    .is_some_and(|quick| quick.followed_by.is_none())
                    && matches!(live.ctx.run_state, RunState::Ended { .. })
                    && live.ctx.workspace_state != WorkspaceState::Removed
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
                    "the log could not be read to settle merges: {error}"
                ));
                return;
            }
        };
        for live in waiting {
            let mut live = locked(&live);
            let own: Vec<Event> = events
                .iter()
                .filter(|event| event.actor.run.as_deref() == Some(live.ctx.run.as_str()))
                .cloned()
                .collect();
            self.settle_merge(phases, &mut live, &own);
        }
    }

    /// One run's open step, if it has one: the world is checked, and the step is never run again before that.
    fn settle_merge(&self, phases: &Phases, live: &mut Live, own: &[Event]) {
        let run = live.ctx.run.clone();
        let Some(quick) = live.ctx.quick.clone() else {
            return;
        };
        let open = recover(own)
            .into_iter()
            .find(|check| check.kind == StepKind::Integrate);
        let Some(check) = open else {
            // No step is open. A merge that succeeded and whose removal never happened finishes the removal.
            if let Some(commit) = last_merge_commit(own) {
                self.log(&format!("run {run}: finishing the removal after its merge"));
                self.finish_merged(phases, live, &commit);
            }
            return;
        };
        let git = Git::with_env(self.config.git_env.clone());
        let refs = Refs::of(live, &quick);
        let finding = self.look_at_checkout(&git, live, &refs);
        match settle(&check, finding) {
            Settled::Succeeded { outcome } => {
                let commit = match &*outcome {
                    EventBody::StepOutcome { detail, .. } => detail.clone(),
                    _ => String::new(),
                };
                self.log(&format!("run {run}: its merge is in the base; finishing"));
                if let Err(error) = self.write_event(phases, &run, *outcome) {
                    self.log(&format!("run {run}: the outcome was not recorded: {error}"));
                }
                self.finish_merged(phases, live, &commit);
            }
            Settled::RunStep => {
                self.log(&format!(
                    "run {run}: its merge did not happen; the review is back"
                ));
                self.write_outcome(
                    phases,
                    &run,
                    StepResult::Failed,
                    "the daemon stopped before the merge was made, and the checkout has none of it",
                );
                self.reopen_review(&run, &quick.task);
            }
            Settled::OutcomeUnknown(unknown) => {
                self.log(&format!("run {run}: {unknown}"));
                self.push_outcome_unknown(&run, &quick.task, &check, &unknown.reason);
            }
        }
    }

    /// The step's own check (0.2): is the run's change in the base branch.
    fn look_at_checkout(&self, git: &Git, live: &Live, refs: &Refs) -> Finding {
        let checkout = Path::new(&live.ctx.repo);
        match merge_head_exists(git, checkout) {
            Ok(true) => {
                return Finding::Inconclusive {
                    reason: "the checkout holds a merge that was cut short (MERGE_HEAD)"
                        .to_string(),
                };
            }
            Ok(false) => {}
            Err(error) => {
                return Finding::Inconclusive {
                    reason: error.to_string(),
                };
            }
        }
        match git.merge_check(checkout, &refs.base, &refs.branch) {
            Ok(MergeCheck::InBase) => {
                match git.text(checkout, &["rev-parse", "--verify", &refs.base]) {
                    Ok(commit) => Finding::Done { detail: commit },
                    Err(error) => Finding::Inconclusive {
                        reason: error.to_string(),
                    },
                }
            }
            Ok(_) => Finding::NotDone,
            Err(error) => Finding::Inconclusive {
                reason: error.to_string(),
            },
        }
    }

    fn push_outcome_unknown(&self, run: &str, task: &str, check: &CheckWorld, reason: &str) {
        self.config.queue.push(QueueItem {
            id: format!("{run}-outcome-unknown"),
            kind: QueueKind::OutcomeUnknown,
            run: Some(run.to_string()),
            phase: None,
            task: Some(task.to_string()),
            since_ms: self.now_ms(),
            title: format!(
                "Collide stopped while it was to {}, and cannot tell whether it happened: {reason}. Look at the \
                 checkout, put it right, then dismiss this to review the run again.",
                check.action
            ),
            permission: None,
            untrusted_input: false,
            cache: None,
            reset_ms: None,
            show: Vec::new(),
            answers: vec![AnswerKind::Dismiss],
        });
    }
}

/// The commit of the run's last integrate, when its outcome was `succeeded`.
fn last_merge_commit(own: &[Event]) -> Option<String> {
    own.iter().rev().find_map(|event| match &event.body {
        EventBody::StepOutcome {
            kind: StepKind::Integrate,
            result,
            detail,
            ..
        } => Some((*result == StepResult::Succeeded).then(|| detail.clone())),
        _ => None,
    })?
}

fn merge_head_exists(git: &Git, checkout: &Path) -> Result<bool, WorkspaceError> {
    let path = git.text(checkout, &["rev-parse", "--git-path", "MERGE_HEAD"])?;
    Ok(checkout.join(path).exists())
}
