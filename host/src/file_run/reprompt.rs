//! Re-prompt on a prompt run's `Review` item (ts-0080-tq; Stage 3c's pull from Stage 9a). **Stage 8's engine and Stage
//! 9a's launch replace this**, as they do the rest of the quick sequence ([`super::quick_run`]).
//!
//! `queue.answer` with `reprompt` and the person's words starts **a new run** of the same scratch task. Core lets
//! nothing but an integrate answer out of `ended`, and the quick run is over before its review, so the run cannot go
//! on: the new run follows it (0.4, Work that never lands: a re-prompt is a new run of the task in that workspace).
//!
//! - **The worktree and the branch** are the old run's. The new run's context is a copy of the old run's
//!   ([`super::RunContext::continuing`]), so it has the same worktree, branch, base commit and session, and it names
//!   its workspace for the run that made it: the worktree is one workspace, and its events carry that name. The new run
//!   is the one that acts on it from here on, in `run.diff`, accept and discard.
//! - **The session** is the old run's, resumed by its id with the person's words as the first turn: the new run's
//!   attempt is the next one, so its launch line has `--resume <id>`, and its phase is a new phase record.
//! - **The task** goes back to `active`, and to `review` when the new run ends, as any run of it does. The old run is
//!   as it was, and records the run that followed it ([`super::quick_run::QuickRun`]'s `followed_by`), so a restart
//!   does not settle its worktree as an accept left half-way.
//! - **The item** is taken first and put back when the re-prompt cannot start, so two answers never both start a run.
//!   A start that is refused after the new run was recorded (no `claude` on the path, a session `claude` cannot find)
//!   is a failed run, as any start that is refused later is, and the task goes back to `review`. A session that
//!   `claude` starts and then cannot resume fails the new phase, and the new run ends with its own `Review` item,
//!   with re-prompt and discard, so the worktree is not left with no way to remove it.
//! - **A run whose agent never started** (stopped for disk before its phase) has no session to resume. It is refused,
//!   and discard is the answer for it.

use super::quick_run::{QUICK, QuickRun};
use super::{Begin, FileRuns, locked};
use crate::phases::Phases;
use collide_core::owner::{QueueAnswerResult, QueueItem};
use collide_core::state::{RunState, WorkspaceState};
use std::cell::RefCell;
use std::path::Path;

impl FileRuns {
    /// `queue.answer` with `reprompt` on a run's `Review` item. Words that say nothing are refused before anything
    /// else, and the item is put back when the re-prompt is refused.
    pub(super) fn reprompt(
        &self,
        phases: &Phases,
        item_id: &str,
        words: &str,
    ) -> QueueAnswerResult {
        if words.trim().is_empty() {
            return QueueAnswerResult::Refused {
                message: "a re-prompt needs words to tell the agent: say what it should do next"
                    .to_string(),
            };
        }
        let Some(item) = self.config.queue.take(item_id) else {
            return QueueAnswerResult::Refused {
                message: format!("item {item_id:?} was answered already"),
            };
        };
        match self.try_reprompt(phases, &item, words) {
            Ok(()) => QueueAnswerResult::Done {},
            Err(message) => {
                self.config.queue.restore(item);
                QueueAnswerResult::Refused { message }
            }
        }
    }

    fn try_reprompt(&self, phases: &Phases, item: &QueueItem, words: &str) -> Result<(), String> {
        let run = item.run.clone().unwrap_or_default();
        let live = self
            .live_of(&run)
            .ok_or_else(|| format!("the daemon has no run {run:?}"))?;
        let (prior, quick) = {
            let live = locked(&live);
            let quick = live.ctx.quick.clone().ok_or_else(|| {
                format!("run {run} is not a prompt run, so there is nothing to re-prompt")
            })?;
            if let Some(next) = &quick.followed_by {
                return Err(format!("run {run} was re-prompted already, as {next}"));
            }
            if !matches!(
                live.ctx.run_state,
                RunState::Ended { .. } | RunState::Failed {} | RunState::Stopped {}
            ) {
                return Err(format!(
                    "run {run} has not ended, so it cannot be re-prompted"
                ));
            }
            match live.workspace.state() {
                WorkspaceState::Idle => {}
                WorkspaceState::Removable | WorkspaceState::Removed => {
                    return Err(format!(
                        "the worktree of run {run} is removed or is being removed, so there is nothing to re-prompt \
                         in: discard the run"
                    ));
                }
                _ => {
                    return Err(format!(
                        "the agent of run {run} never started, so there is no session to resume: discard the run"
                    ));
                }
            }
            if !Path::new(&live.ctx.workspace_path).is_dir() {
                return Err(format!(
                    "the worktree of run {run} is gone ({}), so there is nothing to re-prompt in: discard the run",
                    live.ctx.workspace_path
                ));
            }
            (live.ctx.clone(), quick)
        };
        let next = QuickRun {
            follows: Some(run.clone()),
            followed_by: None,
            ..quick
        };
        let begin = Begin {
            repo: prior.repo.clone(),
            task_file: None,
            alias: prior.alias.clone(),
            budget_usd: prior.budget_usd,
            task_text: words.to_string(),
            model: prior.model.clone(),
            quick: Some(next.clone()),
            follows: Some(prior),
        };
        let claimed = RefCell::new(None);
        let started = self.begin(phases, begin, &|new_run| {
            self.claim(phases, new_run, &next)?;
            *claimed.borrow_mut() = Some(new_run.to_string());
            Ok(())
        });
        match started {
            Ok(new_run) => {
                let mut live = locked(&live);
                if let Some(quick) = live.ctx.quick.as_mut() {
                    quick.followed_by = Some(new_run.clone());
                }
                self.save(&live.ctx);
                self.log(&format!("run {run}: re-prompted, as {new_run}"));
                Ok(())
            }
            Err(problems) => {
                if let Some(new_run) = claimed.into_inner()
                    && let Err(message) = self.move_task(phases, &next, QUICK.on_end, &new_run)
                {
                    self.log(&format!(
                        "run {new_run}: {} was not put back to review: {message}",
                        next.task
                    ));
                }
                Err(problems
                    .into_iter()
                    .map(|each| each.message)
                    .collect::<Vec<_>>()
                    .join("; "))
            }
        }
    }
}
