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
//! It stores no agent events (ts-0067), answers no permission request (ts-0066: a phase that asks hangs until it
//! does), and has no resume for a run paused for disk (ts-0066 owns pause and resume). The Queue is in memory.

use crate::adapters::claude::{ClaudeAdapter, PROGRAM, capabilities, snapshot::snapshot_files};
use crate::agent_feed::AgentFeed;
use crate::base_env::BaseEnvironment;
use crate::builtin_roles::{BUILDER_DEFINITION, BUILDER_ROLE_NAME, builder_role};
use crate::clock::{Clock, Clocks};
use crate::disk::{Before, DiskError, DiskGuard, DiskPause, Floor, FreeSpace};
use crate::models::load_user_models;
use crate::phases::{PhaseEnd, PhaseKey, PhaseSpec, Phases};
use crate::shim::ChildStdio;
use crate::workspace::{BuiltinWorkspace, WorkspaceSpec};
use collide_core::adapter::{AgentAdapter, LaunchRequest, refuse_unmet_capabilities};
use collide_core::agent::SessionStart;
use collide_core::change::{Actor, ActorKind};
use collide_core::events::EventBody;
use collide_core::hook::WorkspaceKind;
use collide_core::non_empty::NonEmpty;
use collide_core::owner::{LaunchProblem, QueueItem, QueueKind, RunStartFileParams};
use collide_core::resolve::resolve_model_alias;
use collide_core::state::{PhaseState, RunState, WorkspaceState};
use collide_core::workflow::{AgentPhase, OnCold, Prompt};
use serde_json::Map;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::Read;
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
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
    pub floor: Floor,
    /// Added to the environment of every git command ([`BuiltinWorkspace::with_git_env`]).
    pub git_env: Vec<(String, String)>,
    pub base_env: BaseEnvSource,
    /// How long a run waits for the first capture of the base environment.
    pub capture_wait: Duration,
    pub log: Arc<dyn Fn(&str) + Send + Sync>,
}

/// The file-task runs of a daemon, and the Queue items their pauses made.
pub struct FileRuns {
    config: FileRunConfig,
    live: Mutex<HashMap<String, BuiltinWorkspace>>,
    queue: Mutex<Vec<QueueItem>>,
    counter: AtomicU64,
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

impl FileRuns {
    pub fn new(config: FileRunConfig) -> FileRuns {
        FileRuns {
            config,
            live: Mutex::new(HashMap::new()),
            queue: Mutex::new(Vec::new()),
            counter: AtomicU64::new(0),
        }
    }

    /// The Queue items the runs made, those of `run` when it is given.
    pub fn queue_items(&self, run: Option<&str>) -> Vec<QueueItem> {
        locked(&self.queue)
            .iter()
            .filter(|item| run.is_none_or(|run| item.run.as_deref() == Some(run)))
            .cloned()
            .collect()
    }

    fn log(&self, text: &str) {
        (self.config.log)(text);
    }

    fn now_ms(&self) -> u64 {
        self.config.clocks.read(Clock::Wall).nanos / 1_000_000
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
        let Some(base) = (self.config.base_env)(self.config.capture_wait) else {
            return Err(vec![problem(
                "the base environment was not captured in time, so no phase can start",
            )]);
        };
        let run = format!(
            "run-{}-{}",
            self.now_ms(),
            self.counter.fetch_add(1, Ordering::SeqCst)
        );
        if let Err(error) = self.record_run(phases, &run, None, RunState::Running {}) {
            return Err(vec![problem(format!("recording the run: {error}"))]);
        }
        let mut workspace = BuiltinWorkspace::new(WorkspaceSpec {
            kind: WorkspaceKind::Task,
            repo: PathBuf::from(&params.repo),
            path: self.config.workspaces_dir.join(&run),
            branch: format!("collide/{run}"),
            base: "HEAD".to_string(),
        })
        .with_git_env(self.config.git_env.clone());
        let halted = self.launch(
            phases,
            &run,
            &mut workspace,
            params,
            &task_text,
            model.as_ref(),
            &base,
        );
        match halted {
            Ok(()) => {
                locked(&self.live).insert(run.clone(), workspace);
                Ok(run)
            }
            Err(Halt::Disk(pause)) => {
                self.pause_for_disk(phases, &run, &pause);
                locked(&self.live).insert(run.clone(), workspace);
                Ok(run)
            }
            Err(Halt::Refused(problems)) => {
                self.fail(phases, &run, &mut workspace);
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
    ) -> Option<(String, Option<collide_core::agent::ModelRef>)> {
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
        if let Err(refusals) = refuse_unmet_capabilities("claude", &capabilities(), &[]) {
            for refusal in refusals {
                problems.push(problem(refusal.to_string()));
            }
        }
        let role = builder_role();
        let alias = params.alias.as_deref().or(role.model.as_deref());
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
        if let Some(usd) = params.budget_usd
            && (!usd.is_finite() || usd <= 0.0)
        {
            problems.push(problem(format!("the budget {usd} is not above zero")));
        }
        Some((text?, model))
    }

    #[allow(clippy::too_many_arguments)]
    fn launch(
        &self,
        phases: &Phases,
        run: &str,
        workspace: &mut BuiltinWorkspace,
        params: &RunStartFileParams,
        task_text: &str,
        model: Option<&collide_core::agent::ModelRef>,
        base: &BaseEnvironment,
    ) -> Result<(), Halt> {
        let guard = DiskGuard::new(self.config.free_space.as_ref(), self.config.floor);
        let workspace_path = self.config.workspaces_dir.join(run);
        self.disk_check(&guard, &workspace_path, Before::Workspace)?;
        self.disk_check(&guard, &self.config.runs_dir, Before::Workspace)?;

        self.record_workspace(phases, run, None, WorkspaceState::Provisioning);
        workspace.provision(&guard).map_err(|error| match error {
            crate::workspace::WorkspaceError::Disk(DiskError::Paused(pause)) => Halt::Disk(pause),
            other => {
                self.record_workspace(
                    phases,
                    run,
                    Some(WorkspaceState::Provisioning),
                    workspace.state(),
                );
                Halt::from(problem(format!("making the worktree: {other}")))
            }
        })?;
        self.record_workspace(
            phases,
            run,
            Some(WorkspaceState::Provisioning),
            WorkspaceState::Ready,
        );

        self.disk_check(&guard, &self.config.runs_dir, Before::Phase)?;
        workspace.start_phase(&guard).map_err(|error| match error {
            crate::workspace::WorkspaceError::Disk(DiskError::Paused(pause)) => Halt::Disk(pause),
            other => Halt::from(problem(format!("starting the phase: {other}"))),
        })?;
        self.record_workspace(
            phases,
            run,
            Some(WorkspaceState::Ready),
            WorkspaceState::InUse,
        );

        let location = workspace
            .location()
            .ok_or_else(|| problem("the worktree has no location"))?;
        let run_dir = self.config.runs_dir.join(run);
        let snapshot_dir = run_dir.join("snapshot");
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
            model: model.cloned(),
            session: SessionStart::Assign {
                uuid: new_uuid()
                    .map_err(|error| problem(format!("making a session id: {error}")))?,
            },
            cwd: PathBuf::from(&location.path),
            snapshot_dir: snapshot_dir.clone(),
            prompt: task_text.to_string(),
            budget_left_usd: params.budget_usd,
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
        let feed =
            AgentFeed::new(adapter.session(&request), &run_dir.join(PHASE_NAME)).close_after_turn();
        let spec = PhaseSpec {
            program,
            args: launch.argv,
            env,
            cwd: launch.cwd,
            stdio: ChildStdio::Pipes,
        };
        phases
            .start_agent_phase(&key, &spec, feed)
            .map_err(|error| problem(format!("starting the phase: {error}")))?;
        if let Err(error) = phases.send_turn(&key, task_text) {
            // A phase that was never told its task would wait for it for good.
            let _ = phases.stop_run(run, Some(Duration::from_secs(2)));
            return Err(problem(format!("sending the task to the phase: {error}")).into());
        }
        Ok(())
    }

    /// Checks the floor for `path`. A volume under it is a pause; one that cannot be read is a refusal.
    fn disk_check(&self, guard: &DiskGuard, path: &Path, before: Before) -> Result<(), Halt> {
        match guard.check(path, before) {
            Ok(()) => Ok(()),
            Err(DiskError::Paused(pause)) => Err(Halt::Disk(pause)),
            Err(error) => Err(problem(error.to_string()).into()),
        }
    }

    fn pause_for_disk(&self, phases: &Phases, run: &str, pause: &DiskPause) {
        let item = pause.queue_item();
        self.log(&format!("run {run}: {}", item.title));
        if let Err(error) =
            self.record_run(phases, run, Some(RunState::Running {}), pause.run_state())
        {
            self.log(&format!("run {run}: the pause was not recorded: {error}"));
        }
        locked(&self.queue).push(QueueItem {
            id: format!("{run}-disk"),
            kind: QueueKind::DiskFull,
            run: Some(run.to_string()),
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
    fn fail(&self, phases: &Phases, run: &str, workspace: &mut BuiltinWorkspace) {
        if workspace.state() == WorkspaceState::InUse {
            self.end_workspace_phase(phases, run, workspace);
        }
        if let Err(error) =
            self.record_run(phases, run, Some(RunState::Running {}), RunState::Failed {})
        {
            self.log(&format!("run {run}: the failure was not recorded: {error}"));
        }
    }

    fn end_workspace_phase(&self, phases: &Phases, run: &str, workspace: &mut BuiltinWorkspace) {
        match workspace.end_phase() {
            Ok(bytes) => self.log(&format!("run {run}: its worktree uses {bytes} bytes")),
            Err(error) => self.log(&format!(
                "run {run}: its worktree was not measured: {error}"
            )),
        }
        self.record_workspace(
            phases,
            run,
            Some(WorkspaceState::InUse),
            WorkspaceState::Idle,
        );
    }

    /// The daemon's end hook ([`crate::phases::PhaseEndHook`]) for a file-task run's phase: the workspace goes idle and
    /// the run ends as its phase did. A phase of another kind of run is none of this.
    pub fn on_phase_end(&self, phases: &Phases, end: &PhaseEnd) {
        let Some(mut workspace) = locked(&self.live).remove(&end.key.run) else {
            return;
        };
        let run = &end.key.run;
        if workspace.state() == WorkspaceState::InUse {
            self.end_workspace_phase(phases, run, &mut workspace);
        }
        let to = match end.state {
            PhaseState::Ended {} => RunState::Ended {
                outcome: NonEmpty::new("the phase ended").expect("the text is not empty"),
            },
            PhaseState::Stopped {} => RunState::Stopped {},
            _ => RunState::Failed {},
        };
        if let Err(error) = self.record_run(phases, run, Some(RunState::Running {}), to) {
            self.log(&format!("run {run}: its end was not recorded: {error}"));
        }
        locked(&self.live).insert(run.clone(), workspace);
    }

    fn record_run(
        &self,
        phases: &Phases,
        run: &str,
        from: Option<RunState>,
        to: RunState,
    ) -> Result<(), crate::store::StoreError> {
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

    fn record_workspace(
        &self,
        phases: &Phases,
        run: &str,
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
                workspace: run.to_string(),
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
