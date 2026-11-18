//! A prompt run of `collide/quick`, up to review (`run.start`, `run.diff`; ts-0078-ye; Stage 3c "From Stage 8").
//! **Stage 8's engine replaces this sequence**, and Stage 9a's launch replaces its start: it is the thinnest run the
//! window's prompt box needs, written as a sequence and not as an engine, and [`QUICK`] is held to what
//! `packs/collide/pack.yaml` declares as `quick` by a test.
//!
//! It is a file-task run ([`super`]) with a board and a scratch task. It shares the file run's worktree, phase, pause
//! and answer code, and adds:
//!
//! - **The start.** `run.start` takes one scratch key and the workflow `collide/quick` or none. It is refused, in
//!   Collide's words, for another workflow (not built yet), for a checkout whose `HEAD` is detached, and for a task
//!   that is not `ready`. The base branch and its commit are read once and recorded in the run's context
//!   (`QuickRun`); the worktree is made from that commit, never from the symbolic `HEAD`, so a checkout that moves
//!   on changes nothing. The worktree is on `collide/<run>`.
//! - **The task moves by category.** `active` at the start and `review` at the end ([`QUICK`]), with the category
//!   name's own status looked up by the source: no status name is written here (plan revision 23). The start is under
//!   `FileRuns::claims`, so two starts cannot both take one ready task; a start that fails after the claim puts the
//!   task back to `ready`.
//! - **The end.** The phase's end hook (`FileRuns::finish_quick`) commits what the agent left uncommitted, in a
//!   commit of Collide's own that says so, so that the diff the person reviews is exactly what accept merges. The
//!   run ends `ended(done)`, `failed` or `stopped`, the task goes to `review`, and one `Review` item is in the Queue
//!   before the run shows as final: with accept, re-prompt and discard for a run that is done, and with re-prompt and
//!   discard alone for one that is not, so no worktree is left with no way to remove it. A failed or stopped run's
//!   leftovers are committed too, so its diff shows them; only a run that is done can fail on that commit (a hook of
//!   the repository that refuses it).
//! - **The context stays.** A prompt run's context is kept in the store after the run ends, because accept,
//!   re-prompt and discard (ts-0079, ts-0080) need its worktree, branch, base and session. Accept (the `accept` module)
//!   removes the worktree and keeps the context, with the workspace `removed`, so `run.get` still knows the run.
//! - **Re-prompt and discard** are in the `reprompt` and `discard` modules: a re-prompt is a new run that takes over
//!   this run's worktree, and a discard deletes the worktree and the branch.
//! - **The diff.** [`FileRuns::diff`] is [`crate::run_diff`] between the recorded commit and the branch's head.

use super::{Begin, FileRuns, Live, locked, problem};
use crate::boards;
use crate::builtin_roles::BUILDER_ROLE_NAME;
use crate::run_diff::{self, DiffError};
use crate::run_store::RunLaunch;
use crate::scratch::{self, ScratchSource};
use crate::sources::changed_event;
use crate::workspace::Git;
use collide_core::change::{Actor, ActorKind};
use collide_core::item::Category;
use collide_core::non_empty::NonEmpty;
use collide_core::owner::{
    AnswerKind, LaunchProblem, QueueItem, QueueKind, RunDiffResult, RunMode, RunStartParams,
    RunTaskRef,
};
use collide_core::ports::TaskProvider;
use collide_core::provider::{ApplyParams, ApplyResult, ChangeCause, GetParams, Task};
use collide_core::state::RunState;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::Path;

/// What the sequence does, as `packs/collide/pack.yaml` declares `quick`.
pub struct QuickSequence {
    pub workflow: &'static str,
    pub phase: &'static str,
    pub role: &'static str,
    /// The category the task moves to when the run starts.
    pub on_start: Category,
    /// The category the task moves to when the run ends done (and, here, however it ends: the review is where the
    /// person decides).
    pub on_end: Category,
    /// The category the task moves to when accept merges the run. In the engine this is the task source's operation
    /// that follows an `integrate` (0.4); the pack declares none yet, so it is written here.
    pub on_merged: Category,
    /// The category the task moves to when the person discards the run's work.
    pub on_discarded: Category,
}

pub const QUICK: QuickSequence = QuickSequence {
    workflow: "collide/quick",
    phase: super::PHASE_NAME,
    role: BUILDER_ROLE_NAME,
    on_start: Category::Active,
    on_end: Category::Review,
    on_merged: Category::Done,
    on_discarded: Category::Dropped,
};

/// The Review item's title for a run that is done.
pub(super) const READY_TITLE: &str = "The agent finished and its changes are ready to review";

/// The Review item's title for a run that was stopped.
const STOPPED_TITLE: &str = "The run was stopped; re-prompt the agent or discard its work";

/// The Review item's title for a run that failed, with the cause the phase recorded when there is one.
fn failed_title(cause: Option<String>) -> String {
    match cause {
        Some(cause) => format!("The run failed ({cause}); re-prompt the agent or discard its work"),
        None => "The run failed; re-prompt the agent or discard its work".to_string(),
    }
}

/// The subject of the commit Collide makes of what the agent left uncommitted.
pub const LEFTOVER_COMMIT: &str = "Commit what the agent left uncommitted";

/// What a prompt run keeps beside a file run's context: the board and the task, and where the run started from. The
/// branch is the one the checkout was on, and the commit is the one it pointed at, both when the run started.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct QuickRun {
    pub(super) board: String,
    /// `scratch:<n>`.
    pub(super) task: String,
    pub(super) base_branch: String,
    pub(super) base_commit: String,
    /// The run this one re-prompts, when it does: it took over that run's worktree and resumed its session.
    #[serde(default)]
    pub(super) follows: Option<String>,
    /// The run a re-prompt of this one started, once it has started.
    #[serde(default)]
    pub(super) followed_by: Option<String>,
}

/// Why [`FileRuns::diff`] gave no diff.
#[derive(Debug, PartialEq, Eq)]
pub enum RunDiffError {
    /// No such prompt run, or the run's branch or the file is gone.
    NotFound(String),
    /// Git failed.
    Failed(String),
}

fn task_problem(key: &str, message: impl Into<String>) -> LaunchProblem {
    LaunchProblem {
        task: Some(key.to_string()),
        message: message.into(),
    }
}

impl FileRuns {
    /// `run.start` for a prompt: checks everything that needs no side effect and returns every problem together, then
    /// starts the run like a file run, with the task claimed once the run is recorded. `board_root` is the folder of
    /// the board's checkout.
    pub fn start_quick(
        &self,
        phases: &crate::phases::Phases,
        params: &RunStartParams,
        board_root: &str,
    ) -> Result<String, Vec<LaunchProblem>> {
        let mut problems = Vec::new();
        let checked = self.check_quick(params, board_root, &mut problems);
        let Some(begin) = checked.filter(|_| problems.is_empty()) else {
            return Err(problems);
        };
        let quick = begin.quick.clone().expect("a prompt run has its record");
        let claimed = RefCell::new(None);
        let result = self.begin(phases, begin, &|run| {
            self.claim(phases, run, &quick)?;
            *claimed.borrow_mut() = Some(run.to_string());
            Ok(())
        });
        if result.is_err()
            && let Some(run) = claimed.into_inner()
            && let Err(message) = self.move_task(phases, &quick, Category::Ready, &run)
        {
            self.log(&format!(
                "run {run}: {} was not put back to ready: {message}",
                quick.task
            ));
        }
        result
    }

    fn check_quick(
        &self,
        params: &RunStartParams,
        board_root: &str,
        problems: &mut Vec<LaunchProblem>,
    ) -> Option<Begin> {
        for (given, what) in [
            (params.line.is_some(), "choosing a line"),
            (params.profile.is_some(), "choosing a profile"),
            (!params.launch.is_empty(), "launch parameters"),
            (params.mode == RunMode::Interactive, "an interactive run"),
        ] {
            if given {
                problems.push(problem(format!("{what} is not built yet")));
            }
        }
        if let Some(workflow) = params.workflow.as_deref()
            && workflow != QUICK.workflow
        {
            problems.push(problem(format!(
                "the workflow {workflow} is not built yet (Stage 8); only {} runs now",
                QUICK.workflow
            )));
        }
        let key = match params.tasks.as_slice() {
            [key] => Some(key.clone()),
            [] => {
                problems.push(problem("name the task to run"));
                None
            }
            _ => {
                problems.push(problem(format!(
                    "{} runs one task at a time; name one task",
                    QUICK.workflow
                )));
                None
            }
        };
        let task = key.as_deref().and_then(|key| {
            self.ready_task(&params.board, key)
                .map_err(|found| problems.push(found))
                .ok()
        });
        let base = self.checkout_base(board_root, problems);
        let model = self.check_agent(None, None, problems);
        let (task, (root, base_branch, base_commit)) = (task?, base?);
        Some(Begin {
            repo: root,
            task_file: None,
            alias: None,
            budget_usd: None,
            task_text: task.body.unwrap_or(task.title),
            model,
            quick: Some(QuickRun {
                board: params.board.clone(),
                task: key?,
                base_branch,
                base_commit,
                follows: None,
                followed_by: None,
            }),
            follows: None,
            config: None,
        })
    }

    /// The checkout's top level, the branch it is on and the commit it is at. A detached `HEAD` and a repository with
    /// no commit are problems.
    fn checkout_base(
        &self,
        board_root: &str,
        problems: &mut Vec<LaunchProblem>,
    ) -> Option<(String, String, String)> {
        let checkout = match boards::checkout_of(Path::new(board_root), &self.config.git_env) {
            Ok(checkout) => checkout,
            Err(message) => {
                problems.push(problem(message));
                return None;
            }
        };
        let root = checkout.root.display().to_string();
        if checkout.branch == "HEAD" {
            problems.push(problem(format!(
                "The checkout at {root} is not on a branch (its HEAD is detached). Check out a branch and start the \
                 run again: the run's changes are merged back into the branch it started from."
            )));
            return None;
        }
        let git = Git::with_env(self.config.git_env.clone());
        match git.text(&checkout.root, &["rev-parse", "--verify", "HEAD^{commit}"]) {
            Ok(commit) => Some((root, checkout.branch, commit)),
            Err(_) => {
                problems.push(problem(format!(
                    "The checkout at {root} has no commits yet, so a run has nothing to start from. Make a first \
                     commit and start the run again."
                )));
                None
            }
        }
    }

    /// The scratch task `key` of `board`, when it is there and `ready`.
    fn ready_task(&self, board: &str, key: &str) -> Result<Task, LaunchProblem> {
        let task = self.task_now(board, key)?;
        if task.state.category != Category::Ready {
            return Err(task_problem(
                key,
                format!(
                    "{key} is {}, and only a ready task can be started",
                    task.state.status
                ),
            ));
        }
        Ok(task)
    }

    /// The scratch task `key` of `board` as it is now, whatever its state.
    pub(super) fn task_now(&self, board: &str, key: &str) -> Result<Task, LaunchProblem> {
        let (source, id) = scratch::split_id(key);
        if source != Some(scratch::SOURCE) {
            return Err(task_problem(
                key,
                format!("{key} is not a scratch task, and only scratch tasks run so far"),
            ));
        }
        let found = ScratchSource::new(self.config.store.clone(), board)
            .get(&GetParams {
                id: id.to_string(),
                checkout: None,
            })
            .map_err(|error| task_problem(key, format!("reading {key}: {error}")))?;
        found
            .task
            .ok_or_else(|| task_problem(key, format!("the board has no task {key}")))
    }

    /// Once the run is recorded: links it to its board and task, and takes the task `active`. Under the claims lock,
    /// the task is read again, so a task that another start took first is refused. The run of a re-prompt takes a task
    /// that is in review, and the person's answer is what claims it.
    pub(super) fn claim(
        &self,
        phases: &crate::phases::Phases,
        run: &str,
        quick: &QuickRun,
    ) -> Result<(), Vec<LaunchProblem>> {
        let _claims = locked(&self.claims);
        let task = if quick.follows.is_some() {
            self.task_now(&quick.board, &quick.task)
        } else {
            self.ready_task(&quick.board, &quick.task)
        }
        .map_err(|found| vec![found])?;
        let launch = RunLaunch {
            board: Some(quick.board.clone()),
            tasks: vec![RunTaskRef {
                key: quick.task.clone(),
                status: task.state.status,
            }],
            line: quick.base_branch.clone(),
            workflow: QUICK.workflow.to_string(),
        };
        match self.config.store.set_run_launch(run, &launch) {
            Ok(true) => {}
            Ok(false) => return Err(vec![problem("the run was not recorded")]),
            Err(error) => return Err(vec![problem(format!("linking the run: {error}"))]),
        }
        self.move_task(phases, quick, QUICK.on_start, run)
            .map_err(|message| vec![task_problem(&quick.task, message)])
    }

    /// Moves the task to the status its source declares for `category`, and records the change as the run's.
    pub(super) fn move_task(
        &self,
        phases: &crate::phases::Phases,
        quick: &QuickRun,
        category: Category,
        run: &str,
    ) -> Result<(), String> {
        let (_, id) = scratch::split_id(&quick.task);
        let applied = ScratchSource::new(self.config.store.clone(), &quick.board)
            .apply(&ApplyParams {
                op: "setStatus".to_string(),
                id: Some(id.to_string()),
                args: BTreeMap::from([("category".to_string(), json!(category))]),
                version: None,
                checkout: None,
            })
            .map_err(|error| error.to_string())?;
        match applied {
            ApplyResult::Ok { items, .. } => {
                for effect in &items {
                    let written = phases.append_event(
                        Actor {
                            kind: ActorKind::Workflow,
                            run: Some(run.to_string()),
                            phase: None,
                        },
                        changed_event(scratch::SOURCE, effect, ChangeCause::Store, None),
                    );
                    if let Err(error) = written {
                        self.log(&format!(
                            "run {run}: the change of {} was written but its event was not: {error}",
                            quick.task
                        ));
                    }
                }
                Ok(())
            }
            ApplyResult::Refused { message } => Err(message),
            ApplyResult::Conflict { .. } => Err(format!("{} changed under the run", quick.task)),
        }
    }

    /// The end of a prompt run, before its final state is recorded: commits what the agent left, moves the task to
    /// review and puts the Review item in the Queue. Returns the state to record, which is `ended(done)` for a phase
    /// that ended and its leftovers were committed. Anything else is a run that is not a prompt run: `to` as it was.
    pub(super) fn finish_quick(
        &self,
        phases: &crate::phases::Phases,
        live: &mut Live,
        to: RunState,
    ) -> RunState {
        let Some(quick) = live.ctx.quick.clone() else {
            return to;
        };
        let run = live.ctx.run.clone();
        let committed = match live.workspace.location() {
            Some(location) => self.commit_leftovers(Path::new(&location.path)),
            None => Ok(()),
        };
        if let Err(message) = &committed {
            self.log(&format!("run {run}: {message}"));
        }
        let (to, title) = match (to, committed) {
            (RunState::Ended { .. }, Ok(())) => (
                RunState::Ended {
                    outcome: NonEmpty::new("done").expect("the text is not empty"),
                },
                READY_TITLE.to_string(),
            ),
            (RunState::Ended { .. }, Err(_)) => (
                RunState::Failed {},
                "Collide could not commit what the agent left; re-prompt the agent or discard its work"
                    .to_string(),
            ),
            (RunState::Stopped {}, _) => (RunState::Stopped {}, STOPPED_TITLE.to_string()),
            (other, _) => (other, failed_title(self.failure_of(&run))),
        };
        if let Err(message) = self.move_task(phases, &quick, QUICK.on_end, &run) {
            self.log(&format!(
                "run {run}: {} was not moved to review: {message}",
                quick.task
            ));
        }
        self.push_review(
            &run,
            &quick.task,
            matches!(to, RunState::Ended { .. }),
            &title,
        );
        to
    }

    /// The Review item's title for a run in `state`, as `finish_quick` words it when the run ends.
    pub(super) fn review_title(&self, run: &str, state: &RunState) -> String {
        match state {
            RunState::Ended { .. } => READY_TITLE.to_string(),
            RunState::Stopped {} => STOPPED_TITLE.to_string(),
            _ => failed_title(self.failure_of(run)),
        }
    }

    /// The cause the run's phase failed with, as the store has it.
    fn failure_of(&self, run: &str) -> Option<String> {
        match self.config.store.phase_record(run, super::PHASE_NAME) {
            Ok(record) => record?.failure,
            Err(error) => {
                self.log(&format!(
                    "run {run}: its phase's record was not read: {error}"
                ));
                None
            }
        }
    }

    /// Puts the run's `Review` item in the Queue: accept, re-prompt and discard for a run that is done, and re-prompt
    /// and discard alone for one that is not, so no worktree is left with no way to remove it. The id is the run's, so
    /// pushing it again replaces the item.
    pub(super) fn push_review(&self, run: &str, task: &str, done: bool, title: &str) {
        let answers = if done {
            vec![
                AnswerKind::Accept,
                AnswerKind::Reprompt,
                AnswerKind::Discard,
            ]
        } else {
            vec![AnswerKind::Reprompt, AnswerKind::Discard]
        };
        self.config.queue.push(QueueItem {
            id: format!("{run}-review"),
            kind: QueueKind::Review,
            run: Some(run.to_string()),
            phase: None,
            task: Some(task.to_string()),
            since_ms: self.now_ms(),
            title: title.to_string(),
            permission: None,
            untrusted_input: false,
            cache: None,
            reset_ms: None,
            show: Vec::new(),
            answers,
        });
    }

    /// Commits what the worktree holds that is not committed, as one commit of Collide's own. Nothing is made when
    /// there is nothing to commit. The repository's hooks run, and signing is off, so a commit never waits for a key.
    fn commit_leftovers(&self, tree: &Path) -> Result<(), String> {
        let git = Git::with_env(self.config.git_env.clone());
        let failed = |error: crate::workspace::WorkspaceError| {
            format!("what the agent left uncommitted was not committed: {error}")
        };
        let status = git
            .run(
                tree,
                &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
            )
            .map_err(failed)?;
        if status.is_empty() {
            return Ok(());
        }
        git.run(tree, &["add", "-A"]).map_err(failed)?;
        git.run(
            tree,
            &[
                "-c",
                "user.name=Collide",
                "-c",
                "user.email=collide@localhost",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "-q",
                "-m",
                LEFTOVER_COMMIT,
                "-m",
                "The agent's phase ended with changes it had not committed. Collide committed them, so the diff \
                 under review is exactly what accepting the run merges.",
            ],
        )
        .map_err(failed)?;
        Ok(())
    }

    /// Whether `run` is a prompt run this daemon drives or drove.
    pub fn is_prompt_run(&self, run: &str) -> bool {
        self.live_of(run)
            .is_some_and(|live| locked(&live).ctx.quick.is_some())
    }

    /// `run.diff`: the recorded base commit against the head of the run's branch, and with `path` that file's patch.
    pub fn diff(&self, run: &str, path: Option<&str>) -> Result<RunDiffResult, RunDiffError> {
        let not_a_prompt_run = || {
            RunDiffError::NotFound(format!(
                "{run:?} is not a prompt run, so it has no review diff"
            ))
        };
        let live = self.live_of(run).ok_or_else(not_a_prompt_run)?;
        let (repo, base, branch) = {
            let live = locked(&live);
            let quick = live.ctx.quick.as_ref().ok_or_else(not_a_prompt_run)?;
            (
                live.ctx.repo.clone(),
                quick.base_commit.clone(),
                live.ctx.branch.clone(),
            )
        };
        let git = Git::with_env(self.config.git_env.clone());
        run_diff::diff(&git, Path::new(&repo), &base, &branch, path).map_err(|error| match error {
            DiffError::Git(message) => RunDiffError::Failed(message),
            other => RunDiffError::NotFound(other.to_string()),
        })
    }
}
