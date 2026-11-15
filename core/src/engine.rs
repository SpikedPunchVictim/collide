//! The workflow engine as signatures (0.7, 0.4; Stage 8): the transition table of a workflow runs against its
//! collaborators, which it takes as the core traits (0.15, Dependency injection), so the engine suite runs both
//! fixture packs with a fake adapter and fake hooks and no process.
//!
//! Every function here is a stub tagged `engine: <work>`, and Stage 8 is complete when none is left. The checks the
//! engine builds on are already real: [`crate::workflow_check`] (structure, launch refusals, adoption),
//! [`crate::run_rules`] (the `pending` poll, `integrate` and `integrating`, recovery after a restart) and
//! [`crate::state`] (the state enums).
//!
//! | tag | function | what it does |
//! |---|---|---|
//! | `engine: run preview` | [`Engine::preview`] | the refusals and the resolved values, before anything starts |
//! | `engine: start run` | [`Engine::start_run`] | the run, its first phase and the `on_start` ops |
//! | `engine: transition` | [`Engine::advance`] | what follows a phase's end |
//! | `engine: fan-out join` | [`Engine::join_fanout`] | when a fan-out is done and which instances failed |
//! | `engine: read-only check` | [`check_read_only`] | the `readOnly` rule over two snapshots of a workspace |
//! | `engine: adopt run` | [`Engine::adopt`] | a run that starts at a named phase and round |
//! | `engine: workflow ops` | [`Engine::run_workflow_ops`] | `on_start` and `on_end` through the provider |

use crate::adapter::{AgentAdapter, PhaseOutcome};
use crate::agent::ModelRef;
use crate::domain::{AskHumanCause, CachedTask, PhaseRun, Run};
use crate::non_empty::NonEmpty;
use crate::pack::PackManifest;
use crate::ports::{HookRunError, HookRunner, ProviderError, TaskProvider};
use crate::provider::ApplyResult;
use crate::resolve::{ResolveError, ResolvedImplementations};
use crate::workflow::{Adoption, Workflow, WorkflowOp};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fmt;

/// The engine and what it calls (0.15). `adapters` is keyed by the agent kind a role names (`claude`). The engine
/// holds no process, store or clock: time and storage come in through the values it is given, so a test drives it
/// with fakes.
pub struct Engine<'a> {
    pub adapters: BTreeMap<String, &'a dyn AgentAdapter>,
    pub hooks: &'a dyn HookRunner,
    pub provider: &'a dyn TaskProvider,
}

/// Why the engine refused or could not go on.
#[derive(Debug)]
pub enum EngineError {
    /// A launch or a step is refused, and the message names the field, token, alias or capability.
    Refused {
        message: String,
    },
    Resolve(ResolveError),
    Hook(HookRunError),
    Provider(ProviderError),
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EngineError::Refused { message } => f.write_str(message),
            EngineError::Resolve(error) => error.fmt(f),
            EngineError::Hook(error) => error.fmt(f),
            EngineError::Provider(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for EngineError {}

impl From<ResolveError> for EngineError {
    fn from(error: ResolveError) -> Self {
        EngineError::Resolve(error)
    }
}

impl From<HookRunError> for EngineError {
    fn from(error: HookRunError) -> Self {
        EngineError::Hook(error)
    }
}

impl From<ProviderError> for EngineError {
    fn from(error: ProviderError) -> Self {
        EngineError::Provider(error)
    }
}

/// A request to start a run (0.7, `launch` parameters; 0.12, Launch on a line).
#[derive(Debug, Clone, PartialEq)]
pub struct RunRequest {
    pub tasks: Vec<CachedTask>,
    /// `pack/workflow`.
    pub workflow: String,
    /// The line's ref.
    pub line: String,
    pub launch: BTreeMap<String, Value>,
    pub profile: Option<String>,
    /// Starts the run at a named phase and round, in an existing workspace (0.7, Adopting).
    pub adoption: Option<Adoption>,
}

/// What the run preview shows before anything starts (0.4, 0.11): the implementations a run will use and the model
/// each role's alias resolved to.
#[derive(Debug, Clone, PartialEq)]
pub struct RunPreview {
    pub implementations: ResolvedImplementations,
    /// Role name (`pack/role`) to the model it runs on.
    pub models: BTreeMap<String, ModelRef>,
}

/// A phase that has ended, as the engine is told. `token` is the decision script's answer, when the phase has a
/// decision.
#[derive(Debug, Clone, PartialEq)]
pub struct PhaseEnded {
    pub phase: String,
    pub round: u32,
    pub outcome: PhaseOutcome,
    pub token: Option<String>,
}

/// What follows a phase (0.7, Outcomes and terminal states).
#[derive(Debug, Clone, PartialEq)]
pub enum Advance {
    /// Another phase, in its round.
    Phase {
        name: String,
        round: u32,
    },
    /// The run ended, with the `end` token it reached.
    Ended {
        outcome: NonEmpty,
    },
    /// The run waits for a person.
    AskHuman {
        cause: AskHumanCause,
    },
    Failed,
    Stopped,
}

/// Where a fan-out stands (0.7, fan-out phases).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FanoutJoin {
    /// Some instances have not ended.
    Waiting { pending: Vec<String> },
    /// Every instance ended. `failed` names the ones that failed; the rest ended.
    Joined {
        ended: Vec<String>,
        failed: Vec<String>,
    },
}

/// A workspace as the `readOnly` check sees it: the head and each tracked or untracked-but-not-ignored file with its
/// hash. Ignored files are out of scope, because a reviewer's own build rebuilds them (0.7, `readOnly`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceSnapshot {
    pub head: String,
    pub files: BTreeMap<String, String>,
}

/// How a `readOnly` phase broke its rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadOnlyViolation {
    HeadMoved {
        before: String,
        after: String,
    },
    /// Paths that changed, were added or were removed outside the phase's directory.
    FilesChanged {
        paths: Vec<String>,
    },
}

/// What one `on_start` or `on_end` op did (0.4, Lifecycle order): the op and the provider's answer.
#[derive(Debug, Clone, PartialEq)]
pub struct OpResult {
    pub op: String,
    pub result: ApplyResult,
}

impl Engine<'_> {
    /// Everything a launch refuses, before anything starts (0.7, 0.3). It runs [`crate::workflow_check::check_launch`],
    /// resolves each role's model alias and the workflow's implementations, checks each role's required capabilities
    /// against its adapter ([`crate::adapter::refuse_unmet_capabilities`]), checks every task against `accepts` and
    /// names the field or operation it lacks, refuses a workflow that uses `task.*` for several tasks, refuses a
    /// fan-out whose aliases resolve to one model, and refuses a second launch of a task with a live run.
    pub fn preview(
        &self,
        request: &RunRequest,
        workflow: &Workflow,
        packs: &BTreeMap<String, PackManifest>,
    ) -> Result<RunPreview, Vec<EngineError>> {
        let _ = (request, workflow, packs);
        todo!("engine: run preview")
    }

    /// Starts a run: the `Run` record, its first phase, and `on_start` through [`Engine::run_workflow_ops`]. A phase
    /// that provisions a workspace goes through the hook runner.
    pub fn start_run(
        &self,
        request: &RunRequest,
        workflow: &Workflow,
        preview: &RunPreview,
    ) -> Result<Run, EngineError> {
        let _ = (request, workflow, preview);
        todo!("engine: start run")
    }

    /// Reads the transition table (0.7): `then:` to a phase, an end, `ask_human` or a decision script. A token that the
    /// decision does not declare makes the decision `error`, and the message names the token. A phase re-entered
    /// counts a round. A failed or stopped phase makes the run `failed` or `stopped`.
    pub fn advance(
        &self,
        workflow: &Workflow,
        run: &Run,
        ended: &PhaseEnded,
    ) -> Result<Advance, EngineError> {
        let _ = (workflow, run, ended);
        todo!("engine: transition")
    }

    /// Joins a fan-out's instances (0.7): done when each has ended or failed, with the failed ones named, so one
    /// failing instance does not hide the others' results.
    pub fn join_fanout(&self, phase: &PhaseRun) -> Result<FanoutJoin, EngineError> {
        let _ = phase;
        todo!("engine: fan-out join")
    }

    /// Starts a run at a named phase and round in an existing workspace (0.7, Adopting), after
    /// [`crate::workflow_check::check_adoption`] has accepted the phase and round.
    pub fn adopt(
        &self,
        request: &RunRequest,
        workflow: &Workflow,
        preview: &RunPreview,
    ) -> Result<Run, EngineError> {
        let _ = (request, workflow, preview);
        todo!("engine: adopt run")
    }

    /// Runs `on_start` or `on_end` ops by status category through the provider's `apply` (0.4, Lifecycle order),
    /// each firing `task.before_status` first and then `record`. `outcome` is the `end` token an `on_end` op's `when:`
    /// reads. A refusal skips only that item.
    pub fn run_workflow_ops(
        &self,
        ops: &[WorkflowOp],
        run: &Run,
        outcome: Option<&str>,
    ) -> Result<Vec<OpResult>, EngineError> {
        let _ = (ops, run, outcome);
        todo!("engine: workflow ops")
    }
}

/// The `readOnly` rule (0.7, 0.9): the head is unchanged, and no tracked or untracked-but-not-ignored file changed
/// outside `phase_dir`. A path under `phase_dir` is not compared: the directory is a scratch place that is usually
/// outside the worktree, so none appears in a snapshot, and one that is inside it is the phase's to write.
pub fn check_read_only(
    before: &WorkspaceSnapshot,
    after: &WorkspaceSnapshot,
    phase_dir: &str,
) -> Result<(), ReadOnlyViolation> {
    let _ = (before, after, phase_dir);
    todo!("engine: read-only check")
}
