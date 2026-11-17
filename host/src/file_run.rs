//! A run of one task file (`run.start_file`; Stage 3b, ts-0065-kx): the daemon makes a worktree of a repository with the
//! built-in workspace, runs the built-in `collide/builder` role in it under a shim, sends the text of the task file as
//! the first turn, and records the run, the phase and the workspace. It is the first run the daemon drives, and the
//! smallest: one phase, no workflow, no board.
//!
//! ## What happens, in order
//!
//! 1. The request is checked: the paths, the role against an empty requirement ([`refuse_unmet_capabilities`]), and the
//!    model alias against the user's `models.yaml`. Every problem is returned together.
//! 2. **The base environment.** The run waits for the first capture of the login shell's environment (ts-0056 decision
//!    5) and takes the newest one. A phase never starts with the daemon's own environment.
//! 3. The run is recorded `running` (`run.transition`). That event's time is the run's start, which the stop sweep reads
//!    ([`crate::phases::Phases::run_start_us`]; ts-0059 decision 5).
//! 4. **The disk floor.** Before the worktree is made and again before the phase starts, the volume of the workspace and
//!    the volume of the run directory are read ([`DiskGuard`]; ts-0055 decision 4). Under the floor nothing is started:
//!    the run goes to `paused: disk` and a `disk_full` Queue item says so ([`FileRuns::queue_items`]).
//! 5. The worktree is provisioned (`workspace.transition` events), the snapshot files are written, and the launch spec
//!    is made by the adapter.
//! 6. **The environment of the child** is the base environment minus what the adapter strips, plus, in this order: the
//!    role's own `env`, the adapter's context variables, and `COLLIDE_RUN` with `COLLIDE_RUN_START`. Later wins, so a
//!    context variable is never overridden by a role's `env` (ts-0065-kx decision 3).
//! 7. The phase starts as an agent phase ([`crate::phases::Phases::start_agent_phase`]) and the task text is sent as the
//!    first turn. From then the phase's watcher feeds the session and settles it ([`crate::agent_feed`]).
//! 8. When the phase ends ([`FileRuns::on_phase_end`]) the workspace goes `in_use → idle` and the run to `ended`,
//!    `failed` or `stopped`. The worktree is left for the owner: nothing removes it.
//!
//! ## What it does not do
//!
//! It stores no agent events (ts-0067) and has no engine, so a budget that runs out is a Queue item that can only stop
//! the run (there is no `ask_human` phase to resume into). The rest of the phase's life is below.
//!
//! ## A prompt run (ts-0078-ye)
//!
//! A run of `collide/quick` on a scratch task shares all of this: `FileRuns::begin` is the start both kinds go through,
//! and the end hook goes on to the review for a run that has a `QuickRun` in its context. Its own part, the
//! checks, the task's moves, the leftovers' commit and the Review item, is in [`quick_run`].
//!
//! ## Pauses, answers and a restart (ts-0066-zb)
//!
//! - A phase that settles on a usage limit (a `limited` event, a 429) or on its budget (`error_max_budget_usd`) is
//!   `paused` and its shim is finished ([`FileRuns::on_phase_pause`]); the run is `paused` with the same cause and a
//!   Queue item says what it waits for ([`crate::queue`]).
//! - A limit has a timer ([`FileRuns::recover`], `arm_timer`): it reads the **wall** clock for the reset time, every
//!   poll, so a machine that slept past it resumes on waking; the pause's length is the **continuous** clock's. It
//!   resumes the session by its id with the budget it has left, in the phase's folder as a new attempt
//!   ([`crate::phases::Phases::resume_agent_phase`]).
//! - [`FileRuns::answer`] is `queue.answer`: allow or deny for a permission request, Resume, and Stop, and for a
//!   prompt run's review (`quick_run`): accept, re-prompt (`reprompt`) and discard (`discard`).
//! - Each run's context is in the store ([`crate::store::Store::set_run_context`]), so a daemon that starts again goes
//!   on with the runs it finds: it rebuilds a running phase's session from its journal, keeps its pending requests in
//!   the Queue, and resumes a limit that reset while no daemon ran.

mod accept;
mod discard;
pub mod quick_run;
mod reprompt;

use crate::adapters::claude::session::ClaudeSession;
use crate::adapters::claude::user_config::{
    endpoint_in_environment, read_user_config, user_config_fingerprint,
};
use crate::adapters::claude::version::{CANARY_FAILURE, ClaudeVersion};
use crate::adapters::claude::{
    ClaudeAdapter, PROGRAM, capabilities, env_strip, snapshot::snapshot_files,
};
use crate::agent_feed::{AgentFeed, SETTINGS_IGNORED_FAILURE};
use crate::base_env::BaseEnvironment;
use crate::builtin_roles::{BUILDER_DEFINITION, BUILDER_ROLE_NAME, builder_role};
use crate::claude_versions::{ClaudeVersions, Gate};
use crate::clock::{Clock, Clocks};
use crate::disk::{Before, DiskError, DiskGuard, DiskPause, FloorSource, FreeSpace};
use crate::metrics::Metrics;
use crate::metrics_store::WaitKind;
use crate::models::load_user_models;
use crate::phases::{PhaseEnd, PhaseKey, PhasePause, PhaseSpec, Phases};
use crate::queue::Queue;
use crate::shim::ChildStdio;
use crate::store::Store;
use crate::workspace::{BuiltinWorkspace, WorkspaceError, WorkspaceSpec};
use collide_core::adapter::{AgentAdapter, LaunchRequest, refuse_unmet_capabilities};
use collide_core::agent::{ModelRef, SessionStart};
use collide_core::change::{Actor, ActorKind};
use collide_core::events::{BudgetScope, EventBody};
use collide_core::hook::WorkspaceKind;
use collide_core::non_empty::NonEmpty;
use collide_core::owner::{
    AnswerKind, LaunchProblem, QueueAnswer, QueueAnswerResult, QueueItem, QueueKind,
    RunStartFileParams,
};
use collide_core::profile::RoleConfig;
use collide_core::resolve::resolve_model_alias;
use collide_core::state::{PauseCause, PhaseState, RunState, WorkspaceState};
use collide_core::workflow::{AgentPhase, OnCold, Prompt};
use quick_run::QuickRun;
use serde::{Deserialize, Serialize};
use serde_json::Map;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::Read;
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// The name of the one phase of a file-task run.
pub const PHASE_NAME: &str = "build";

/// The variable that carries the run's start, in microseconds since the epoch, beside [`crate::stop::RUN_MARKER`]: the
/// stop sweep passes over a process that began before the run did.
pub const RUN_START_MARKER: &str = "COLLIDE_RUN_START";

/// Where the base environment comes from: a wait of up to the given time for the first capture, then the newest one.
pub type BaseEnvSource = Arc<dyn Fn(Duration) -> Option<BaseEnvironment> + Send + Sync>;

/// What [`FileRuns`] is made of.
pub struct FileRunConfig {
    pub runs_dir: PathBuf,
    /// The folder the runs' worktrees are made in, one folder for each run.
    pub workspaces_dir: PathBuf,
    /// The home directory `models.yaml` is read from: given, never read from the environment here.
    pub home: Option<String>,
    pub clocks: Arc<dyn Clocks + Send + Sync>,
    pub free_space: Arc<dyn FreeSpace + Send + Sync>,
    /// The floor for a run's board, read when the run is checked ([`crate::disk::FloorSource`]).
    pub floor: FloorSource,
    /// Added to the environment of every git command ([`BuiltinWorkspace::with_git_env`]).
    pub git_env: Vec<(String, String)>,
    pub base_env: BaseEnvSource,
    /// How long a run waits for the first capture of the base environment.
    pub capture_wait: Duration,
    pub log: Arc<dyn Fn(&str) + Send + Sync>,
    /// Where a run's context is kept for a restart.
    pub store: Arc<Store>,
    /// The Queue the runs' pauses put their items in, and the answers take them out of.
    pub queue: Arc<Queue>,
    /// How a limit's timer waits between two looks at the wall clock (`thread::sleep` in a running daemon). A test
    /// passes one that moves its clocks, as a sleeping machine does.
    pub sleep: Arc<dyn Fn(Duration) + Send + Sync>,
    /// How often a limit's timer looks at the wall clock.
    pub limit_poll: Duration,
    /// The least a run stays paused for a limit before the reset time resumes it, on the continuous clock.
    pub limit_min_pause: Duration,
    /// The always-recorded metrics: the run's span, its waits, its workspace's size and its fingerprint.
    pub metrics: Arc<Metrics>,
    /// The version of `claude` and its smoke check: a phase starts only on a binary that passes it.
    pub claude: Arc<ClaudeVersions>,
}

/// The text of the turn that goes first to a session that was resumed: the stream ended mid-work, and the tool waits for
/// a user turn before it goes on (S1).
pub const RESUME_TURN: &str = "Continue where you left off.";

/// What a file-task run keeps in the store, so a restart can go on with it: what it was asked, the session of its agent,
/// where its worktree is, and what the session has cost. It is the daemon's own text, not a wire type.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RunContext {
    run: String,
    repo: String,
    task_file: Option<String>,
    alias: Option<String>,
    budget_usd: Option<f64>,
    task_text: String,
    model: Option<ModelRef>,
    /// The session's id, which the first attempt assigns and every later one resumes.
    session_id: String,
    /// How many attempts the phase has had after the first: `0` until it is resumed.
    attempt: u32,
    /// What the session had cost when the phase last paused, in US dollars. The tool counts a session's cost across
    /// `--resume`, so this is the whole of it and never a sum of processes.
    spent_usd: f64,
    /// The run's state as the daemon last recorded it, which the next `run.transition` event is from.
    run_state: RunState,
    workspace_state: WorkspaceState,
    workspace_path: String,
    git_dir: Option<String>,
    branch: String,
    /// The `provisioning` event was written, so a launch that is tried again does not write it twice.
    workspace_recorded: bool,
    /// What a prompt run (`collide/quick`) records besides: its board, its task and the branch and commit it started
    /// from. Absent for a run of a task file. It stays in the store after the run ends, because accept, re-prompt and
    /// discard need the worktree, the branch and the session.
    #[serde(default)]
    quick: Option<QuickRun>,
    /// The folder's name under the workspaces folder, and the `workspace` of the workspace's events: the id of the run
    /// that made the worktree. `None` for a run that made its own. A re-prompt takes over the worktree of the run it
    /// follows, and keeps that name, so the worktree is one workspace.
    #[serde(default)]
    workspace_name: Option<String>,
}

impl RunContext {
    /// The workspace's name: the run's own id unless the run took over another run's worktree.
    fn workspace_name(&self) -> &str {
        self.workspace_name.as_deref().unwrap_or(&self.run)
    }

    /// The context of the run that takes `prior`'s place after a re-prompt: the same repository, model, session, branch
    /// and worktree, a new run and `quick` record, and `task_text` as the first turn. It is the session's next attempt,
    /// so it resumes the session by its id.
    fn continuing(prior: &RunContext, run: &str, task_text: String, quick: QuickRun) -> RunContext {
        RunContext {
            run: run.to_string(),
            repo: prior.repo.clone(),
            task_file: None,
            alias: prior.alias.clone(),
            budget_usd: prior.budget_usd,
            task_text,
            model: prior.model.clone(),
            session_id: prior.session_id.clone(),
            attempt: prior.attempt + 1,
            spent_usd: prior.spent_usd,
            run_state: RunState::Running {},
            workspace_state: prior.workspace_state,
            workspace_path: prior.workspace_path.clone(),
            git_dir: prior.git_dir.clone(),
            branch: prior.branch.clone(),
            workspace_recorded: true,
            quick: Some(quick),
            workspace_name: Some(prior.workspace_name().to_string()),
        }
    }

    /// The first attempt assigns the session's id and every later one resumes it.
    fn session_start(&self) -> SessionStart {
        if self.attempt == 0 {
            SessionStart::Assign {
                uuid: self.session_id.clone(),
            }
        } else {
            SessionStart::Resume {
                id: self.session_id.clone(),
            }
        }
    }

    fn budget_left(&self) -> Option<f64> {
        self.budget_usd.map(|budget| budget - self.spent_usd)
    }
}

/// A run the daemon drives: its worktree and what it keeps of it. One lock for both, held while the run starts, resumes
/// or ends, so the phase's end hook waits for a start that has not finished.
struct Live {
    workspace: BuiltinWorkspace,
    ctx: RunContext,
}

/// Why [`FileRuns::answer`] did not answer an item.
#[derive(Debug, PartialEq, Eq)]
pub enum AnswerError {
    /// The Queue holds no such item.
    NotFound(String),
    /// The item does not take that kind of answer.
    Invalid(String),
}

/// The file-task runs of a daemon: their pauses, their resumes and the Queue items the pauses made.
pub struct FileRuns {
    config: FileRunConfig,
    live: Mutex<HashMap<String, Arc<Mutex<Live>>>>,
    /// A cancel flag for each run that has a timer waiting for its limit's reset.
    timers: Mutex<HashMap<String, Arc<AtomicBool>>>,
    timer_threads: Mutex<Vec<JoinHandle<()>>>,
    closed: AtomicBool,
    counter: AtomicU64,
    /// Held while a prompt run claims its task, so two starts on one task cannot both find it ready.
    claims: Mutex<()>,
    /// Held while accept merges into a checkout, so two accepts never meet in git's own lock.
    merging: Mutex<()>,
}

fn locked<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

fn problem(message: impl Into<String>) -> LaunchProblem {
    LaunchProblem {
        task: None,
        message: message.into(),
    }
}

/// Where a start stopped: refused with its problems, or paused for disk (the run exists and is `paused: disk`).
enum Halt {
    Refused(Vec<LaunchProblem>),
    Disk(DiskPause),
}

impl From<LaunchProblem> for Halt {
    fn from(problem: LaunchProblem) -> Halt {
        Halt::Refused(vec![problem])
    }
}

/// What a run is made of once its request is checked, whatever the request was.
struct Begin {
    repo: String,
    task_file: Option<String>,
    alias: Option<String>,
    budget_usd: Option<f64>,
    task_text: String,
    model: Option<ModelRef>,
    quick: Option<QuickRun>,
    /// The context of the run this one re-prompts: the new run takes over its worktree and resumes its session.
    follows: Option<RunContext>,
}

fn limit_item_id(run: &str) -> String {
    format!("{run}-limit")
}

fn budget_item_id(run: &str) -> String {
    format!("{run}-budget")
}

fn disk_item_id(run: &str) -> String {
    format!("{run}-disk")
}

fn agent_setup_item_id(run: &str) -> String {
    format!("{run}-agent-setup")
}

fn answer_kind(answer: &QueueAnswer) -> AnswerKind {
    match answer {
        QueueAnswer::Permission { .. } => AnswerKind::Permission,
        QueueAnswer::Text { .. } => AnswerKind::Text,
        QueueAnswer::Proceed {} => AnswerKind::Proceed,
        QueueAnswer::Stop {} => AnswerKind::Stop,
        QueueAnswer::Retry {} => AnswerKind::Retry,
        QueueAnswer::RaiseCap { .. } => AnswerKind::RaiseCap,
        QueueAnswer::Accept {} => AnswerKind::Accept,
        QueueAnswer::Discard {} => AnswerKind::Discard,
        QueueAnswer::Reprompt { .. } => AnswerKind::Reprompt,
        QueueAnswer::Acknowledge {} => AnswerKind::Acknowledge,
        QueueAnswer::Resume {} => AnswerKind::Resume,
        QueueAnswer::Dismiss {} => AnswerKind::Dismiss,
    }
}

impl FileRuns {
    /// The runs of an earlier daemon are read back from the store, so the end of a phase that finished while no daemon
    /// ran still ends its run. A context that cannot be read is logged and left out.
    pub fn new(config: FileRunConfig) -> FileRuns {
        let mut live = HashMap::new();
        match config.store.run_contexts() {
            Ok(rows) => {
                for (run, json) in rows {
                    match serde_json::from_str::<RunContext>(&json) {
                        Ok(ctx) => {
                            let workspace = restored_workspace(&config, &ctx);
                            live.insert(run, Arc::new(Mutex::new(Live { workspace, ctx })));
                        }
                        Err(error) => {
                            (config.log)(&format!(
                                "run {run}: its context is not readable: {error}"
                            ));
                        }
                    }
                }
            }
            Err(error) => (config.log)(&format!("the runs' contexts could not be read: {error}")),
        }
        FileRuns {
            config,
            live: Mutex::new(live),
            timers: Mutex::new(HashMap::new()),
            timer_threads: Mutex::new(Vec::new()),
            closed: AtomicBool::new(false),
            counter: AtomicU64::new(0),
            claims: Mutex::new(()),
            merging: Mutex::new(()),
        }
    }

    /// The worktree of `run`, when it has been made: the folder the run's workspace is in. `None` for a name that is no
    /// run's (the name is a path part, so a name that is not a plain one is refused here).
    pub fn workspace_path(&self, run: &str) -> Option<PathBuf> {
        PhaseKey::new(run, PHASE_NAME).ok()?;
        // A run that took over another's worktree has that run's folder.
        let name = self.live_of(run).map_or_else(
            || run.to_string(),
            |live| locked(&live).ctx.workspace_name().to_string(),
        );
        let path = self.config.workspaces_dir.join(name);
        path.is_dir().then_some(path)
    }

    /// The Queue items, those of `run` when it is given.
    pub fn queue_items(&self, run: Option<&str>) -> Vec<QueueItem> {
        self.config.queue.list(run)
    }

    fn log(&self, text: &str) {
        (self.config.log)(text);
    }

    fn now_ms(&self) -> u64 {
        self.config.clocks.read(Clock::Wall).nanos / 1_000_000
    }

    fn live_of(&self, run: &str) -> Option<Arc<Mutex<Live>>> {
        locked(&self.live).get(run).cloned()
    }

    /// Ends the timers: each stops within one poll, and this waits for them. The runs go on in their shims.
    pub fn shutdown(&self) {
        self.closed.store(true, Ordering::SeqCst);
        let handles = std::mem::take(&mut *locked(&self.timer_threads));
        for handle in handles {
            let _ = handle.join();
        }
    }

    /// Starts a run. `Ok` is the run's id, for a run that is running and for one that is paused for disk; `Err` is every
    /// problem that kept it from starting (the run is then recorded `failed` if it got as far as being recorded).
    pub fn start(
        &self,
        phases: &Phases,
        params: &RunStartFileParams,
    ) -> Result<String, Vec<LaunchProblem>> {
        let mut problems = Vec::new();
        let checked = self.check(params, &mut problems);
        let Some((task_text, model)) = checked.filter(|_| problems.is_empty()) else {
            return Err(problems);
        };
        let begin = Begin {
            repo: params.repo.clone(),
            task_file: Some(params.task_file.clone()),
            alias: params.alias.clone(),
            budget_usd: params.budget_usd,
            task_text,
            model,
            quick: None,
            follows: None,
        };
        self.begin(phases, begin, &|_| Ok(()))
    }

    /// What every kind of run does once its request is checked: waits for the base environment, records the run, calls
    /// `linked` (which a prompt run uses to link its board and task and to claim the task), then makes the worktree
    /// and starts the phase. A `linked` that refuses ends the run `failed`, as a start that is refused later does.
    fn begin(
        &self,
        phases: &Phases,
        begin: Begin,
        linked: &dyn Fn(&str) -> Result<(), Vec<LaunchProblem>>,
    ) -> Result<String, Vec<LaunchProblem>> {
        let Some(base) = (self.config.base_env)(self.config.capture_wait) else {
            return Err(vec![problem(
                "the base environment was not captured in time, so no phase can start",
            )]);
        };
        let session_id =
            new_uuid().map_err(|error| vec![problem(format!("making a session id: {error}"))])?;
        let run = format!(
            "run-{}-{}",
            self.now_ms(),
            self.counter.fetch_add(1, Ordering::SeqCst)
        );
        let workspace_path = self.config.workspaces_dir.join(&run);
        let ctx = match (&begin.follows, &begin.quick) {
            (Some(prior), Some(quick)) => {
                RunContext::continuing(prior, &run, begin.task_text, quick.clone())
            }
            _ => RunContext {
                run: run.clone(),
                repo: begin.repo,
                task_file: begin.task_file,
                alias: begin.alias,
                budget_usd: begin.budget_usd,
                task_text: begin.task_text,
                model: begin.model,
                session_id,
                attempt: 0,
                spent_usd: 0.0,
                run_state: RunState::Running {},
                workspace_state: WorkspaceState::Provisioning,
                workspace_path: workspace_path.to_string_lossy().into_owned(),
                git_dir: None,
                branch: format!("collide/{run}"),
                workspace_recorded: false,
                quick: begin.quick,
                workspace_name: None,
            },
        };
        if let Err(error) = self.record_run(phases, &run, None, RunState::Running {}) {
            return Err(vec![problem(format!("recording the run: {error}"))]);
        }
        let workspace = restored_workspace(&self.config, &ctx);
        let live = Arc::new(Mutex::new(Live { workspace, ctx }));
        // In the table before the phase starts, so a phase that ends at once finds its run.
        locked(&self.live).insert(run.clone(), live.clone());
        let mut live = locked(&live);
        let halted = linked(&run)
            .map_err(Halt::Refused)
            .and_then(|()| self.launch(phases, &run, &mut live, &base));
        self.sync(&mut live);
        match halted {
            Ok(()) => Ok(run),
            Err(Halt::Disk(pause)) => {
                self.pause_for_disk(phases, &mut live, &pause);
                Ok(run)
            }
            Err(Halt::Refused(problems)) => {
                self.fail(phases, &mut live);
                locked(&self.live).remove(&run);
                for each in &problems {
                    self.log(&format!("run {run}: did not start: {}", each.message));
                }
                Err(problems)
            }
        }
    }

    /// The checks that need no daemon state: the paths, the role and the alias. Returns the task text and the model.
    fn check(
        &self,
        params: &RunStartFileParams,
        problems: &mut Vec<LaunchProblem>,
    ) -> Option<(String, Option<ModelRef>)> {
        if !Path::new(&params.repo).is_absolute() || !Path::new(&params.repo).is_dir() {
            problems.push(problem(format!(
                "the repository {:?} is not a folder (give an absolute path)",
                params.repo
            )));
        }
        let text = match read_task(&params.task_file) {
            Ok(text) => Some(text),
            Err(message) => {
                problems.push(problem(message));
                None
            }
        };
        let model = self.check_agent(params.alias.as_deref(), params.budget_usd, problems);
        Some((text?, model))
    }

    /// The checks every run's agent shares: the role against an empty requirement, the model alias and the budget.
    /// Returns the model, which is `None` for a role with no model and for an alias that did not resolve (the problem
    /// is then in `problems`).
    fn check_agent(
        &self,
        alias: Option<&str>,
        budget_usd: Option<f64>,
        problems: &mut Vec<LaunchProblem>,
    ) -> Option<ModelRef> {
        if let Err(refusals) = refuse_unmet_capabilities("claude", &capabilities(), &[]) {
            for refusal in refusals {
                problems.push(problem(refusal.to_string()));
            }
        }
        let role = builder_role();
        let alias = alias.or(role.model.as_deref());
        let model = match alias {
            None => None,
            Some(alias) => match load_user_models(self.config.home.as_deref()) {
                Err(error) => {
                    problems.push(problem(error.to_string()));
                    None
                }
                Ok(models) => {
                    match resolve_model_alias(alias, &models.mapping_for("claude"), None) {
                        Ok(model) => Some(model),
                        Err(error) => {
                            problems.push(problem(error.to_string()));
                            None
                        }
                    }
                }
            },
        };
        if let Some(usd) = budget_usd
            && (!usd.is_finite() || usd <= 0.0)
        {
            problems.push(problem(format!("the budget {usd} is not above zero")));
        }
        model
    }

    /// Copies the workspace's state into the context and stores it.
    fn sync(&self, live: &mut Live) {
        live.ctx.workspace_state = live.workspace.state();
        if let Some(location) = live.workspace.location() {
            live.ctx.workspace_path = location.path;
            live.ctx.git_dir = Some(location.git_dir);
        }
        self.save(&live.ctx);
    }

    fn save(&self, ctx: &RunContext) {
        match serde_json::to_string(ctx) {
            Ok(json) => {
                if let Err(error) = self.config.store.set_run_context(&ctx.run, &json) {
                    self.log(&format!(
                        "run {}: its context was not stored: {error}",
                        ctx.run
                    ));
                }
            }
            Err(error) => self.log(&format!(
                "run {}: its context is not JSON: {error}",
                ctx.run
            )),
        }
    }

    /// Makes the worktree if it is not made, starts the workspace's phase, and starts the agent. It is also what a
    /// run that paused for disk does again when it is resumed: each step is skipped when the workspace is past it.
    fn launch(
        &self,
        phases: &Phases,
        run: &str,
        live: &mut Live,
        base: &BaseEnvironment,
    ) -> Result<(), Halt> {
        let board = live.ctx.quick.as_ref().map(|quick| quick.board.as_str());
        let floor = (self.config.floor)(board);
        let guard = DiskGuard::new(self.config.free_space.as_ref(), floor);
        let Live { workspace, ctx } = live;
        let workspace_path = self.config.workspaces_dir.join(ctx.workspace_name());
        let name = ctx.workspace_name().to_string();
        if workspace.state() == WorkspaceState::Provisioning {
            self.disk_check(&guard, &workspace_path, Before::Workspace)?;
            self.disk_check(&guard, &self.config.runs_dir, Before::Workspace)?;
            if !ctx.workspace_recorded {
                self.record_workspace(phases, run, &name, None, WorkspaceState::Provisioning);
                ctx.workspace_recorded = true;
            }
            workspace.provision(&guard).map_err(|error| match error {
                WorkspaceError::Disk(DiskError::Paused(pause)) => Halt::Disk(pause),
                other => {
                    self.record_workspace(
                        phases,
                        run,
                        &name,
                        Some(WorkspaceState::Provisioning),
                        workspace.state(),
                    );
                    Halt::from(problem(format!("making the worktree: {other}")))
                }
            })?;
            self.record_workspace(
                phases,
                run,
                &name,
                Some(WorkspaceState::Provisioning),
                WorkspaceState::Ready,
            );
        }

        self.disk_check(&guard, &self.config.runs_dir, Before::Phase)?;
        let workspace_before = workspace.state();
        workspace.start_phase(&guard).map_err(|error| match error {
            WorkspaceError::Disk(DiskError::Paused(pause)) => Halt::Disk(pause),
            other => Halt::from(problem(format!("starting the phase: {other}"))),
        })?;
        // A workspace that is `idle` (a worktree a re-prompt takes over) goes in use from there.
        self.record_workspace(
            phases,
            run,
            &name,
            Some(workspace_before),
            WorkspaceState::InUse,
        );
        let first_turn = ctx.task_text.clone();
        self.start_attempt(phases, run, workspace, ctx, base, &first_turn, false)
    }

    /// Starts the phase's agent: the first attempt (`resume` false) or a later one, which resumes the session.
    /// `first_turn` is what the session is told first.
    #[allow(clippy::too_many_arguments)]
    fn start_attempt(
        &self,
        phases: &Phases,
        run: &str,
        workspace: &BuiltinWorkspace,
        ctx: &RunContext,
        base: &BaseEnvironment,
        first_turn: &str,
        resume: bool,
    ) -> Result<(), Halt> {
        let location = workspace
            .location()
            .ok_or_else(|| problem("the worktree has no location"))?;
        let run_dir = self.config.runs_dir.join(run);
        let snapshot_dir = run_dir.join("snapshot");
        let budget_left = if resume {
            ctx.budget_left()
        } else {
            ctx.budget_usd
        };
        let request = LaunchRequest {
            role_name: BUILDER_ROLE_NAME.to_string(),
            role: builder_role(),
            phase_name: PHASE_NAME.to_string(),
            phase: AgentPhase {
                role: BUILDER_ROLE_NAME.to_string(),
                prompt: Prompt::Name("task".to_string()),
                label: None,
                gates: Vec::new(),
                outputs: Vec::new(),
                record: false,
                sandbox: None,
                on_cold: OnCold::default(),
                fanout: None,
                then: None,
            },
            model: ctx.model.clone(),
            session: ctx.session_start(),
            cwd: PathBuf::from(&location.path),
            snapshot_dir: snapshot_dir.clone(),
            prompt: first_turn.to_string(),
            budget_left_usd: budget_left,
        };
        write_snapshot(&request, &snapshot_dir)?;
        let adapter = ClaudeAdapter;
        let launch = adapter
            .launch(&request)
            .map_err(|error| problem(format!("the launch line: {error}")))?;

        // The base environment, minus the adapter's strip, then the role's env, the adapter's context variables and
        // the run's markers, each layer over the one before.
        let start_us = phases.run_start_us(run).unwrap_or_default();
        let extra = layered_env(&request.role.env, &launch.env, run, start_us);
        let env = base.for_child(&launch.env_strip, &extra);

        let program = find_on_path(PROGRAM, env.get("PATH").map(String::as_str)).ok_or_else(|| {
            problem(format!(
                "`{PROGRAM}` is not on the PATH of the base environment, so the phase cannot start"
            ))
        })?;
        let key = PhaseKey::new(run, PHASE_NAME)
            .map_err(|error| problem(format!("the phase's name: {error}")))?;
        // The version is read before each phase (cached by the binary's path and time); one outside the tested range
        // gets a smoke check, here and not in the phase, so a failed one refuses the phase before it starts.
        let version = match self.config.claude.gate(&program, &run_dir, &env) {
            Gate::Ready { version, .. } => version,
            Gate::Refused { message, .. } | Gate::Unreadable { message } => {
                self.push_agent_setup(run, None, &message);
                return Err(problem(message).into());
            }
        };
        self.record_agent_version(run, version);
        self.record_fingerprint(run, request.role.config, &env);
        let feed =
            AgentFeed::new(adapter.session(&request), &run_dir.join(PHASE_NAME)).close_after_turn();
        let spec = PhaseSpec {
            program,
            args: launch.argv,
            env,
            cwd: launch.cwd,
            stdio: ChildStdio::Pipes,
        };
        let started = if resume {
            phases.resume_agent_phase(&key, &spec, feed)
        } else {
            phases.start_agent_phase(&key, &spec, feed)
        };
        started.map_err(|error| problem(format!("starting the phase: {error}")))?;
        if let Err(error) = phases.send_turn(&key, first_turn) {
            // A phase that was never told its task would wait for it for good.
            let _ = phases.stop_run(run, Some(Duration::from_secs(2)));
            return Err(problem(format!("sending the task to the phase: {error}")).into());
        }
        Ok(())
    }

    /// The `agent_version` part of the run's fingerprint (0.13): the version the phase runs on, whether it is in the tested
    /// range, and whether a smoke check passed for it.
    fn record_agent_version(&self, run: &str, version: ClaudeVersion) {
        let status = self.config.claude.status().agents.remove(0);
        let part = serde_json::json!({
            "agent": status.agent,
            "version": version.to_string(),
            "in_tested_range": status.in_tested_range,
            "smoke_passed": status.smoke.map(|smoke| smoke.passed),
        });
        self.config
            .metrics
            .fingerprint_part(run, "agent_version", &part.to_string());
    }

    /// A Queue item that says the agent tool was not ready (a version that failed its smoke check, or settings it
    /// ignored). It waits to be dismissed.
    fn push_agent_setup(&self, run: &str, phase: Option<&str>, title: &str) {
        self.log(&format!("run {run}: {title}"));
        self.config.queue.push(QueueItem {
            id: agent_setup_item_id(run),
            kind: QueueKind::AgentSetup,
            run: Some(run.to_string()),
            phase: phase.map(String::from),
            task: None,
            since_ms: self.now_ms(),
            title: title.to_string(),
            permission: None,
            untrusted_input: false,
            cache: None,
            reset_ms: None,
            show: Vec::new(),
            answers: vec![AnswerKind::Dismiss],
        });
    }

    /// Reads the version of the `claude` the base environment finds, for `daemon.agent_versions` and the daemon's start.
    /// It starts nothing else, and waits for the first capture for up to `wait`: before it there is nothing to read.
    pub fn read_claude_version(&self, wait: Duration) {
        let Some(base) = (self.config.base_env)(wait) else {
            return;
        };
        let env = base.for_child(&env_strip(), &BTreeMap::new());
        match find_on_path(PROGRAM, env.get("PATH").map(String::as_str)) {
            Some(program) => {
                if let Err(error) = self.config.claude.read(&program, &env) {
                    self.log(&error.to_string());
                }
            }
            None => self.config.claude.note_no_binary(format!(
                "`{PROGRAM}` is not on the PATH of the base environment"
            )),
        }
    }

    /// Writes the user-config part of the run's fingerprint (0.13): the user's hooks and flagged `env` by name and the
    /// endpoint's host, read as the phase's launch line will load them. A settings file that cannot be read is logged and
    /// leaves the part out: a missing part is not an empty config.
    fn record_fingerprint(
        &self,
        run: &str,
        config: Option<RoleConfig>,
        env: &BTreeMap<String, String>,
    ) {
        let mut user = match read_user_config(self.config.home.as_deref(), config) {
            Ok(user) => user,
            Err(error) => {
                self.log(&format!(
                    "run {run}: the user config was not read for its fingerprint: {error}"
                ));
                return;
            }
        };
        if user.endpoint.is_none() {
            user.endpoint = endpoint_in_environment(env);
        }
        self.config.metrics.fingerprint_part(
            run,
            "user_config",
            &user_config_fingerprint(&user).to_string(),
        );
    }

    /// Checks the floor for `path`. A volume under it is a pause; one that cannot be read is a refusal.
    fn disk_check(&self, guard: &DiskGuard, path: &Path, before: Before) -> Result<(), Halt> {
        match guard.check(path, before) {
            Ok(()) => Ok(()),
            Err(DiskError::Paused(pause)) => Err(Halt::Disk(pause)),
            Err(error) => Err(problem(error.to_string()).into()),
        }
    }

    fn pause_for_disk(&self, phases: &Phases, live: &mut Live, pause: &DiskPause) {
        let run = live.ctx.run.clone();
        let item = pause.queue_item();
        self.log(&format!("run {run}: {}", item.title));
        self.record_run_state(phases, live, pause.run_state());
        self.config.metrics.wait_started(
            &run,
            PHASE_NAME,
            WaitKind::Disk,
            Some(&disk_item_id(&run)),
        );
        self.config.queue.push(QueueItem {
            id: disk_item_id(&run),
            kind: QueueKind::DiskFull,
            run: Some(run),
            phase: None,
            task: None,
            since_ms: self.now_ms(),
            title: format!("{} ({})", item.title, item.detail),
            permission: None,
            untrusted_input: false,
            cache: None,
            reset_ms: None,
            show: Vec::new(),
            answers: item.answers,
        });
    }

    /// A start that failed after the run was recorded: the workspace is released and the run is `failed`.
    fn fail(&self, phases: &Phases, live: &mut Live) {
        if live.workspace.state() == WorkspaceState::InUse {
            self.end_workspace_phase(phases, live);
        }
        self.record_run_state(phases, live, RunState::Failed {});
        self.config.store.delete_run_context(&live.ctx.run).ok();
    }

    fn end_workspace_phase(&self, phases: &Phases, live: &mut Live) {
        let run = live.ctx.run.as_str();
        match live.workspace.end_phase() {
            Ok(bytes) => {
                self.log(&format!("run {run}: its worktree uses {bytes} bytes"));
                self.config.metrics.workspace_bytes(run, PHASE_NAME, bytes);
            }
            Err(error) => self.log(&format!(
                "run {run}: its worktree was not measured: {error}"
            )),
        }
        self.record_workspace(
            phases,
            run,
            live.ctx.workspace_name(),
            Some(WorkspaceState::InUse),
            WorkspaceState::Idle,
        );
    }

    /// The daemon's end hook ([`crate::phases::PhaseEndHook`]) for a file-task run's phase: the workspace goes idle and
    /// the run ends as its phase did, and what the run was waiting for is dropped (its timer, its Queue items). A phase
    /// of another kind of run is none of this.
    pub fn on_phase_end(&self, phases: &Phases, end: &PhaseEnd) {
        let run = &end.key.run;
        let Some(live) = self.live_of(run) else {
            return;
        };
        let mut live = locked(&live);
        if live.workspace.state() == WorkspaceState::InUse {
            self.end_workspace_phase(phases, &mut live);
        }
        self.cancel_timer(run);
        self.config.queue.clear_run(run, None);
        // A phase that ended its turn without the canary ran without Collide's settings: the Queue says so, before the
        // run shows as failed, so a client that sees the failure finds the item.
        let record = self.config.store.phase_record(run, &end.key.phase);
        if matches!(&record, Ok(Some(record)) if record.failure.as_deref() == Some(SETTINGS_IGNORED_FAILURE))
        {
            self.push_agent_setup(run, Some(&end.key.phase), CANARY_FAILURE);
        }
        let to = match end.state {
            PhaseState::Ended {} => RunState::Ended {
                outcome: NonEmpty::new("the phase ended").expect("the text is not empty"),
            },
            PhaseState::Stopped {} => RunState::Stopped {},
            _ => RunState::Failed {},
        };
        // A prompt run goes on to its review before it shows as final, so a client that sees it final finds the item.
        let to = self.finish_quick(phases, &mut live, to);
        self.record_run_state(phases, &mut live, to);
        self.sync(&mut live);
        self.forget_context(&live);
    }

    /// An ended run needs nothing from a restart, except a prompt run, which review, accept and discard still need.
    fn forget_context(&self, live: &Live) {
        if live.ctx.quick.is_some() {
            return;
        }
        let run = &live.ctx.run;
        if let Err(error) = self.config.store.delete_run_context(run) {
            self.log(&format!("run {run}: its context was not deleted: {error}"));
        }
    }

    /// The daemon's pause hook ([`crate::phases::PhasePauseHook`]): the run is `paused` with the phase's cause and a
    /// Queue item says what it waits for. A usage limit starts a timer for the reset time; a budget that ran out waits
    /// for the person, who can only stop it in 3b.
    pub fn on_phase_pause(self: &Arc<Self>, phases: &Phases, paused: &PhasePause) {
        let run = &paused.key.run;
        let Some(live) = self.live_of(run) else {
            return;
        };
        let mut live = locked(&live);
        if let Some(spent) = paused.pause.spent_usd {
            live.ctx.spent_usd = spent;
        }
        let cause = paused.pause.cause;
        self.record_run_state(phases, &mut live, RunState::Paused { cause });
        self.sync(&mut live);
        let since_ms = self.now_ms();
        match cause {
            PauseCause::UsageLimit => {
                let reset_ms = paused.pause.reset_at_ms;
                self.config.metrics.wait_started(
                    run,
                    &paused.key.phase,
                    WaitKind::UsageLimit,
                    Some(&limit_item_id(run)),
                );
                self.config.queue.push(QueueItem {
                    id: limit_item_id(run),
                    kind: QueueKind::UsageLimit,
                    run: Some(run.clone()),
                    phase: Some(paused.key.phase.clone()),
                    task: None,
                    since_ms,
                    title: "The agent reached its usage limit".to_string(),
                    permission: None,
                    untrusted_input: false,
                    cache: None,
                    reset_ms,
                    show: Vec::new(),
                    answers: vec![AnswerKind::Resume, AnswerKind::Stop],
                });
                self.log(&format!(
                    "run {run}: paused for a usage limit{}",
                    reset_ms.map_or(String::new(), |ms| format!(", which resets at {ms} ms"))
                ));
                self.arm_timer(phases, run, reset_ms, self.config.limit_min_pause);
            }
            PauseCause::AskHuman => {
                let item = budget_item_id(run);
                self.config.metrics.wait_started(
                    run,
                    &paused.key.phase,
                    WaitKind::Budget,
                    Some(&item),
                );
                self.append(
                    phases,
                    run,
                    EventBody::BudgetExceeded {
                        run: run.clone(),
                        scope: BudgetScope::Phase,
                    },
                );
                self.append(
                    phases,
                    run,
                    EventBody::AskHumanAsked {
                        run: run.clone(),
                        cause: "budget".to_string(),
                        queue_item: item.clone(),
                    },
                );
                self.config.queue.push(QueueItem {
                    id: item,
                    kind: QueueKind::BudgetExceeded,
                    run: Some(run.clone()),
                    phase: Some(paused.key.phase.clone()),
                    task: None,
                    since_ms,
                    title: "The phase used its whole budget".to_string(),
                    permission: None,
                    untrusted_input: false,
                    cache: None,
                    reset_ms: None,
                    show: Vec::new(),
                    answers: vec![AnswerKind::Stop],
                });
                self.log(&format!("run {run}: paused, its phase used its budget"));
            }
            PauseCause::Disk => {}
        }
    }

    fn append(&self, phases: &Phases, run: &str, body: EventBody) {
        let written = phases.append_event(
            Actor {
                kind: ActorKind::Workflow,
                run: Some(run.to_string()),
                phase: None,
            },
            body,
        );
        if let Err(error) = written {
            self.log(&format!("run {run}: an event was not stored: {error}"));
        }
    }

    /// After a start: the runs that were paused for a usage limit when the last daemon ended get their timers, which
    /// resume them at the reset time or at once when it passed while no daemon ran. A pause that this start already
    /// found (a phase that paused while no daemon ran) has its timer.
    pub fn recover(self: &Arc<Self>, phases: &Phases) {
        let runs: Vec<Arc<Mutex<Live>>> = locked(&self.live).values().cloned().collect();
        for live in runs {
            let (run, state) = {
                let live = locked(&live);
                (live.ctx.run.clone(), live.ctx.run_state.clone())
            };
            if state
                == (RunState::Paused {
                    cause: PauseCause::UsageLimit,
                })
                && !locked(&self.timers).contains_key(&run)
            {
                let reset_ms = self
                    .config
                    .queue
                    .get(&limit_item_id(&run))
                    .and_then(|item| item.reset_ms);
                self.log(&format!("run {run}: still paused for a usage limit"));
                self.arm_timer(phases, &run, reset_ms, Duration::ZERO);
            }
        }
        self.recover_accepts(phases);
        self.recover_discards(phases);
    }

    /// Starts a thread that resumes the run when its limit has reset. A limit with no reset time has no timer: only the
    /// person's Resume answer resumes it.
    ///
    /// The thread looks at the wall clock every [`FileRunConfig::limit_poll`], so a machine that slept past the reset
    /// resumes on its first look after waking, and a clock set forward does too. It does not count down. The pause is
    /// at least [`FileRunConfig::limit_min_pause`] long on the continuous clock, which keeps a reset time that is
    /// already past from resuming into the same limit over and over. The length of the pause is read on the
    /// continuous clock too, which keeps counting while the machine sleeps. A pause that an earlier daemon began has no
    /// floor: it has already lasted.
    fn arm_timer(
        self: &Arc<Self>,
        phases: &Phases,
        run: &str,
        reset_ms: Option<u64>,
        floor: Duration,
    ) {
        let Some(reset_ms) = reset_ms else {
            return;
        };
        let cancel = Arc::new(AtomicBool::new(false));
        locked(&self.timers).insert(run.to_string(), cancel.clone());
        let runs = self.clone();
        let phases = phases.clone();
        let named = run.to_string();
        let spawned = thread::Builder::new()
            .name(format!("collide-limit-{run}"))
            .spawn(move || runs.wait_for_reset(&phases, &named, reset_ms, floor, &cancel));
        match spawned {
            Ok(handle) => locked(&self.timer_threads).push(handle),
            Err(error) => self.log(&format!(
                "run {run}: the thread that waits for the reset could not start: {error}"
            )),
        }
    }

    fn wait_for_reset(
        &self,
        phases: &Phases,
        run: &str,
        reset_ms: u64,
        floor: Duration,
        cancel: &AtomicBool,
    ) {
        let paused_at = self.config.clocks.read(Clock::Continuous).nanos;
        let floor = u64::try_from(floor.as_nanos()).unwrap_or(u64::MAX);
        loop {
            if self.closed.load(Ordering::SeqCst) || cancel.load(Ordering::SeqCst) {
                return;
            }
            let paused_for = self
                .config
                .clocks
                .read(Clock::Continuous)
                .nanos
                .saturating_sub(paused_at);
            if self.now_ms() >= reset_ms && paused_for >= floor {
                self.timers_forget(run);
                if let Some(item) = self.config.queue.take(&limit_item_id(run)) {
                    self.log(&format!(
                        "run {run}: its limit reset after a pause of {} s; resuming",
                        paused_for / 1_000_000_000
                    ));
                    if let Err(message) = self.resume_limit(phases, item) {
                        self.log(&format!("run {run}: not resumed: {message}"));
                    }
                }
                return;
            }
            (self.config.sleep)(self.config.limit_poll);
        }
    }

    fn timers_forget(&self, run: &str) {
        locked(&self.timers).remove(run);
    }

    fn cancel_timer(&self, run: &str) {
        if let Some(cancel) = locked(&self.timers).remove(run) {
            cancel.store(true, Ordering::SeqCst);
        }
    }

    /// Resumes a run that paused for a usage limit: the run goes `running`, and the phase's session is resumed by its
    /// id with the budget it has left. `item` is the limit's Queue item, taken out by the caller, so one resume acts.
    /// A resume that could not start puts the run back to `paused` and the item back in the Queue.
    fn resume_limit(&self, phases: &Phases, item: QueueItem) -> Result<(), String> {
        let run = item.run.clone().unwrap_or_default();
        self.cancel_timer(&run);
        let restore = |message: String| {
            self.config.queue.restore(item.clone());
            message
        };
        let Some(live) = self.live_of(&run) else {
            return Err(restore(format!("the daemon has no run {run:?}")));
        };
        let Some(base) = (self.config.base_env)(self.config.capture_wait) else {
            return Err(restore(
                "the base environment was not captured in time, so no phase can resume".to_string(),
            ));
        };
        let mut live = locked(&live);
        let paused = live.ctx.run_state.clone();
        self.record_run_state(phases, &mut live, RunState::Running {});
        live.ctx.attempt += 1;
        let first_turn = RESUME_TURN;
        let Live { workspace, ctx } = &mut *live;
        let started = self.start_attempt(phases, &run, workspace, ctx, &base, first_turn, true);
        if let Err(halt) = started {
            let message = match halt {
                Halt::Refused(problems) => problems
                    .into_iter()
                    .map(|each| each.message)
                    .collect::<Vec<_>>()
                    .join("; "),
                Halt::Disk(pause) => pause.queue_item().title,
            };
            live.ctx.attempt -= 1;
            self.record_run_state(phases, &mut live, paused);
            self.sync(&mut live);
            return Err(restore(message));
        }
        self.sync(&mut live);
        self.log(&format!(
            "run {run}: resumed (attempt {})",
            live.ctx.attempt + 1
        ));
        Ok(())
    }

    /// Resumes a run that paused for disk: the start goes on from the step the floor stopped. A floor that is still
    /// there pauses it again with a new item.
    fn resume_disk(&self, phases: &Phases, item: QueueItem) -> Result<(), String> {
        let run = item.run.clone().unwrap_or_default();
        let restore = |message: String| {
            self.config.queue.restore(item.clone());
            message
        };
        let Some(live) = self.live_of(&run) else {
            return Err(restore(format!("the daemon has no run {run:?}")));
        };
        let Some(base) = (self.config.base_env)(self.config.capture_wait) else {
            return Err(restore(
                "the base environment was not captured in time, so no phase can start".to_string(),
            ));
        };
        let mut live = locked(&live);
        self.record_run_state(phases, &mut live, RunState::Running {});
        let halted = self.launch(phases, &run, &mut live, &base);
        self.sync(&mut live);
        match halted {
            Ok(()) => Ok(()),
            Err(Halt::Disk(pause)) => {
                self.pause_for_disk(phases, &mut live, &pause);
                Ok(())
            }
            Err(Halt::Refused(problems)) => {
                self.fail(phases, &mut live);
                Err(problems
                    .into_iter()
                    .map(|each| each.message)
                    .collect::<Vec<_>>()
                    .join("; "))
            }
        }
    }

    /// Stops a run that paused for disk before it had a phase: there is nothing to signal, so the run is recorded
    /// `stopped` and its Queue items are dropped. `false` for any other run, which [`Phases::stop_run`] stops. The
    /// worktree is left as it was (`provisioning` means none was made; `ready` can be removed).
    pub fn stop_unstarted(&self, phases: &Phases, run: &str) -> bool {
        let Some(live) = self.live_of(run) else {
            return false;
        };
        let mut live = locked(&live);
        if live.ctx.run_state
            != (RunState::Paused {
                cause: PauseCause::Disk,
            })
        {
            return false;
        }
        self.config.queue.clear_run(run, None);
        let to = self.finish_quick(phases, &mut live, RunState::Stopped {});
        self.record_run_state(phases, &mut live, to);
        self.forget_context(&live);
        true
    }

    /// Answers a Queue item, once: the item is taken out of the Queue before anything is done, so two answers never
    /// both act. A permission request goes to the agent (allow or deny, never "always"); Resume resumes a run paused
    /// for a limit or for disk; Stop stops the run. An answer that could not be carried out is `refused` with its
    /// reason and the item stays.
    pub fn answer(
        self: &Arc<Self>,
        phases: &Phases,
        item_id: &str,
        answer: &QueueAnswer,
    ) -> Result<QueueAnswerResult, AnswerError> {
        let item =
            self.config.queue.get(item_id).ok_or_else(|| {
                AnswerError::NotFound(format!("the Queue has no item {item_id:?}"))
            })?;
        let kind = answer_kind(answer);
        if !item.answers.contains(&kind) {
            return Err(AnswerError::Invalid(format!(
                "item {item_id:?} takes {:?}, not {kind:?}",
                item.answers
            )));
        }
        let refused = |message: String| Ok(QueueAnswerResult::Refused { message });
        let outcome = match answer {
            QueueAnswer::Permission { reply } => {
                let (Some(run), Some(phase), Some(request)) =
                    (&item.run, &item.phase, &item.permission)
                else {
                    return refused(format!("item {item_id:?} holds no request"));
                };
                let key = PhaseKey::new(run, phase)
                    .map_err(|error| AnswerError::Invalid(error.to_string()))?;
                phases.answer_permission(&key, &request.id, reply.clone())
            }
            QueueAnswer::Resume {} => match self.config.queue.take(item_id) {
                None => Err(format!("item {item_id:?} was answered already")),
                Some(taken) if taken.kind == QueueKind::DiskFull => self.resume_disk(phases, taken),
                Some(taken) => self.resume_limit(phases, taken),
            },
            QueueAnswer::Stop {} => {
                let run = item.run.clone().unwrap_or_default();
                if !phases.has_run(&run) && self.stop_unstarted(phases, &run) {
                    Ok(())
                } else {
                    phases
                        .stop_run(&run, None)
                        .map(|_| ())
                        .map_err(|error| error.to_string())
                }
            }
            QueueAnswer::Dismiss {} => match self.config.queue.take(item_id) {
                Some(taken) => {
                    if taken.kind == QueueKind::OutcomeUnknown {
                        self.dismiss_unknown(phases, &taken);
                    }
                    Ok(())
                }
                None => Err(format!("item {item_id:?} was answered already")),
            },
            QueueAnswer::Accept {} => return Ok(self.accept(phases, item_id)),
            QueueAnswer::Reprompt { prompt } => return Ok(self.reprompt(phases, item_id, prompt)),
            QueueAnswer::Discard {} => return Ok(self.discard(phases, item_id)),
            _ => Err(format!("an item of this kind does not take {kind:?} yet")),
        };
        match outcome {
            Ok(()) => Ok(QueueAnswerResult::Done {}),
            Err(message) => refused(message),
        }
    }

    /// Records the run's move to `to`, from the state the context holds, and keeps `to` in the context.
    fn record_run_state(&self, phases: &Phases, live: &mut Live, to: RunState) {
        let run = live.ctx.run.clone();
        let from = std::mem::replace(&mut live.ctx.run_state, to.clone());
        if let Err(error) = self.record_run(phases, &run, Some(from), to) {
            self.log(&format!("run {run}: a move was not recorded: {error}"));
        }
    }

    fn record_run(
        &self,
        phases: &Phases,
        run: &str,
        from: Option<RunState>,
        to: RunState,
    ) -> Result<(), crate::store::StoreError> {
        let metrics = &self.config.metrics;
        match (&from, &to) {
            (None, RunState::Running {}) => metrics.run_started(run),
            (_, RunState::Ended { .. }) => metrics.run_ended(run, "ended"),
            (_, RunState::Failed {}) => metrics.run_ended(run, "failed"),
            (_, RunState::Stopped {}) => metrics.run_ended(run, "stopped"),
            _ => {}
        }
        phases.append_event(
            Actor {
                kind: ActorKind::Workflow,
                run: Some(run.to_string()),
                phase: None,
            },
            EventBody::RunTransition {
                run: run.to_string(),
                from,
                to,
            },
        )
    }

    /// Records a move of the workspace named `workspace`, made by `run`: the two are the same for a run that made its
    /// worktree, and not for one that took it over.
    fn record_workspace(
        &self,
        phases: &Phases,
        run: &str,
        workspace: &str,
        from: Option<WorkspaceState>,
        to: WorkspaceState,
    ) {
        let written = phases.append_event(
            Actor {
                kind: ActorKind::Workflow,
                run: Some(run.to_string()),
                phase: None,
            },
            EventBody::WorkspaceTransition {
                workspace: workspace.to_string(),
                from,
                to,
            },
        );
        if let Err(error) = written {
            self.log(&format!(
                "run {run}: a workspace event was not stored: {error}"
            ));
        }
    }

    /// Rebuilds the feed of a phase the daemon did not start, from its journal ([`crate::phases::FeedRebuilder`]): the
    /// session starts as the run's attempt did, so a permission request asked before a restart is pending again.
    pub fn rebuild_feed(&self, key: &PhaseKey, dir: &Path) -> Option<AgentFeed> {
        let live = self.live_of(&key.run)?;
        let start = locked(&live).ctx.session_start();
        match AgentFeed::rebuild(Box::new(ClaudeSession::new(start).expecting_canary()), dir) {
            Ok(feed) => Some(feed.close_after_turn()),
            Err(error) => {
                self.log(&format!(
                    "phase {key}: its session could not be rebuilt from the journal: {error}"
                ));
                None
            }
        }
    }
}

/// The workspace a context describes, as the daemon that stored it left it.
fn restored_workspace(config: &FileRunConfig, ctx: &RunContext) -> BuiltinWorkspace {
    BuiltinWorkspace::restored(
        WorkspaceSpec {
            kind: WorkspaceKind::Task,
            repo: PathBuf::from(&ctx.repo),
            path: PathBuf::from(&ctx.workspace_path),
            branch: ctx.branch.clone(),
            base: ctx
                .quick
                .as_ref()
                .map_or_else(|| "HEAD".to_string(), |quick| quick.base_commit.clone()),
        },
        ctx.git_dir.as_ref().map(PathBuf::from),
        ctx.workspace_state,
    )
    .with_git_env(config.git_env.clone())
}

/// What is added to the base environment after the adapter's strip, later over earlier: the role's own `env`, the
/// adapter's context variables (`LaunchSpec::env`), then the run's markers. A context variable is set after the role's
/// `env`, so a role cannot override one (ts-0065-kx decision 3).
pub fn layered_env(
    role_env: &BTreeMap<String, String>,
    context_env: &BTreeMap<String, String>,
    run: &str,
    run_start_us: u64,
) -> BTreeMap<String, String> {
    let mut extra = role_env.clone();
    extra.extend(context_env.clone());
    extra.insert(crate::stop::RUN_MARKER.to_string(), run.to_string());
    extra.insert(RUN_START_MARKER.to_string(), run_start_us.to_string());
    extra
}

/// The text of the task file: a file, not empty, and not larger than [`MAX_TASK_BYTES`].
fn read_task(path: &str) -> Result<String, String> {
    if !Path::new(path).is_absolute() {
        return Err(format!("the task file {path:?} is not an absolute path"));
    }
    let file = fs::File::open(path).map_err(|error| format!("the task file {path}: {error}"))?;
    let mut text = String::new();
    file.take(MAX_TASK_BYTES + 1)
        .read_to_string(&mut text)
        .map_err(|error| format!("the task file {path} is not readable text: {error}"))?;
    if text.len() as u64 > MAX_TASK_BYTES {
        return Err(format!(
            "the task file {path} is over {MAX_TASK_BYTES} bytes"
        ));
    }
    if text.trim().is_empty() {
        return Err(format!("the task file {path} is empty"));
    }
    Ok(text)
}

/// The most a task file may hold: it is one message to the agent.
pub const MAX_TASK_BYTES: u64 = 256 * 1024;

fn write_snapshot(request: &LaunchRequest, dir: &Path) -> Result<(), Halt> {
    let files = snapshot_files(request, BUILDER_DEFINITION, &Map::new())
        .map_err(|error| problem(format!("the snapshot: {error}")))?;
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)
        .map_err(|error| problem(format!("making {}: {error}", dir.display())))?;
    for (name, text) in files.named() {
        let path = dir.join(name);
        fs::write(&path, text)
            .map_err(|error| problem(format!("writing {}: {error}", path.display())))?;
    }
    Ok(())
}

/// The first file called `program` in a folder of `path` that is a file with an execute bit.
fn find_on_path(program: &str, path: Option<&str>) -> Option<PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    path?
        .split(':')
        .filter(|dir| !dir.is_empty())
        .map(|dir| Path::new(dir).join(program))
        .find(|candidate| {
            fs::metadata(candidate)
                .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
        })
}

/// A random version 4 UUID, from the system's random source.
fn new_uuid() -> std::io::Result<String> {
    let mut bytes = [0u8; 16];
    fs::File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
    Ok(format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_uuid_is_version_4_in_the_usual_layout() {
        let uuid = new_uuid().unwrap();
        assert_eq!(uuid.len(), 36);
        assert_eq!(uuid.as_bytes()[14], b'4');
        assert!(matches!(uuid.as_bytes()[19], b'8' | b'9' | b'a' | b'b'));
        assert_ne!(uuid, new_uuid().unwrap());
    }

    #[test]
    fn a_program_is_found_in_the_first_folder_that_holds_an_executable_of_that_name() {
        assert_eq!(
            find_on_path("sh", Some("/nonexistent:/bin:/usr/bin")),
            Some(PathBuf::from("/bin/sh"))
        );
        assert_eq!(find_on_path("sh", Some("/nonexistent")), None);
        assert_eq!(find_on_path("sh", None), None);
    }
}
