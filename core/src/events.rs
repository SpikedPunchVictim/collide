//! The event catalogue (0.4, Observers; 0.8, Event log). Each line of the event log is an [`Event`]: a sequence
//! number, the wall clock, the actor, the experiment, and one [`EventBody`]. Observers receive the same lines
//! (see [`crate::observer::ObserverEvent`]). Every event names the stage that makes it fire, through [`EventKind`].
//!
//! The states of runs, phases, workspaces and experiments are plain strings for now; `ts-0037-vg` types them.

use crate::change::{Actor, Change, Rename};
use crate::hook::{HookExperiment, HookStatus};
use crate::points::Point;
use crate::provider::{TaskChanged, TaskInvalid, TaskRemoved};
use crate::report::ReportLine;
use crate::stage::Stage;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// One line of the event log. `wall_ms` is the wall clock in milliseconds since the Unix epoch. `experiment` is null
/// outside an experiment (0.4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Event {
    pub seq: u64,
    pub wall_ms: u64,
    pub actor: Actor,
    pub experiment: Option<HookExperiment>,
    pub body: EventBody,
}

/// The answer to a permission request (0.3, S7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum PermissionAnswer {
    Allow,
    Deny,
    Cancel,
}

/// The budget that stopped a run: the run's total, a phase's, or a role's (0.7, Budgets).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum BudgetScope {
    Run,
    Phase,
    Role,
}

/// How a side-effect step ended. A merge that finds its change already at the target is `succeeded` (0.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum StepResult {
    Succeeded,
    Failed,
}

/// The side-effect steps that write an intent before and an outcome after (0.2): an integrate, a source op, an
/// `after_integrate` hook and a remote write.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StepKind {
    Integrate,
    SourceOp,
    AfterIntegrateHook,
    RemoteWrite,
}

/// What happened. Each variant is one event name, and `event` is the tag. Unknown keys are refused.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "event", deny_unknown_fields)]
pub enum EventBody {
    /// `from` is null for the first state.
    #[serde(rename = "run.transition")]
    RunTransition {
        run: String,
        from: Option<String>,
        to: String,
    },
    /// The phase's log and transcript paths are carried, because a registry needs them (0.4). A `human` phase has no
    /// log, only agent phases have a transcript, and a phase entering its first state has neither yet, so both are
    /// null until they exist. `round` counts the phase's repeats, as `HookPhase.round` does.
    #[serde(rename = "phase.transition")]
    PhaseTransition {
        run: String,
        phase: String,
        round: u32,
        from: Option<String>,
        to: String,
        log: Option<String>,
        transcript: Option<String>,
    },
    /// `run` is null for a gate on a named point that no run is running. A gate sits on a phase (`phase`) or on a
    /// named point (`point`, 0.4 Gates). `message` is the hook's own message, as in `HookResult.message`.
    #[serde(rename = "gate.verdict")]
    GateVerdict {
        pack: String,
        gate: String,
        status: HookStatus,
        run: Option<String>,
        phase: Option<String>,
        point: Option<Point>,
        #[serde(default)]
        message: String,
    },
    #[serde(rename = "permission.requested")]
    PermissionRequested {
        run: String,
        phase: String,
        request: String,
        tool: String,
    },
    #[serde(rename = "permission.answered")]
    PermissionAnswered {
        run: String,
        phase: String,
        request: String,
        answer: PermissionAnswer,
    },
    /// `cause` names the decision token, the budget, or the agent's stop-and-report that led to the question.
    /// `queue_item` is the Queue item the run waits on (0.13), so an answer matches its question.
    #[serde(rename = "ask_human.asked")]
    AskHumanAsked {
        run: String,
        cause: String,
        queue_item: String,
    },
    #[serde(rename = "ask_human.answered")]
    AskHumanAnswered {
        run: String,
        answer: String,
        queue_item: String,
    },
    #[serde(rename = "budget.exceeded")]
    BudgetExceeded { run: String, scope: BudgetScope },
    #[serde(rename = "task.changed")]
    TaskChanged { source: String, task: TaskChanged },
    #[serde(rename = "task.renamed")]
    TaskRenamed { source: String, rename: Rename },
    #[serde(rename = "task.removed")]
    TaskRemoved { source: String, task: TaskRemoved },
    #[serde(rename = "task.invalid")]
    TaskInvalid {
        source: String,
        invalid: TaskInvalid,
    },
    /// A write Collide made. `key` names the item; the change says what it did.
    #[serde(rename = "task.written")]
    TaskWritten {
        source: String,
        key: String,
        change: Change,
    },
    #[serde(rename = "workspace.transition")]
    WorkspaceTransition {
        workspace: String,
        from: Option<String>,
        to: String,
    },
    #[serde(rename = "metric.reported")]
    MetricReported { run: String, line: ReportLine },
    #[serde(rename = "experiment.transition")]
    ExperimentTransition {
        id: String,
        from: Option<String>,
        to: String,
    },
    /// Written before the step. `action` says what the step will do. After a restart, an intent with no outcome is
    /// checked against the world, never repeated blind (0.2).
    #[serde(rename = "step.intent")]
    StepIntent {
        step: String,
        kind: StepKind,
        action: String,
    },
    /// Written after the step, with the same `step` and `kind`. `result` says how it ended; `detail` says more, such as
    /// the merge commit or the PR url.
    #[serde(rename = "step.outcome")]
    StepOutcome {
        step: String,
        kind: StepKind,
        result: StepResult,
        #[serde(default)]
        detail: String,
    },
}

/// The name of each event, and the stage that makes it fire (0.4, Observers).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventKind {
    RunTransition,
    PhaseTransition,
    GateVerdict,
    PermissionRequested,
    PermissionAnswered,
    AskHumanAsked,
    AskHumanAnswered,
    BudgetExceeded,
    TaskChanged,
    TaskRenamed,
    TaskRemoved,
    TaskInvalid,
    TaskWritten,
    WorkspaceTransition,
    MetricReported,
    ExperimentTransition,
    StepIntent,
    StepOutcome,
}

impl EventKind {
    /// Every event kind, in the catalogue's order.
    pub const ALL: [EventKind; 18] = [
        EventKind::RunTransition,
        EventKind::PhaseTransition,
        EventKind::GateVerdict,
        EventKind::PermissionRequested,
        EventKind::PermissionAnswered,
        EventKind::AskHumanAsked,
        EventKind::AskHumanAnswered,
        EventKind::BudgetExceeded,
        EventKind::TaskChanged,
        EventKind::TaskRenamed,
        EventKind::TaskRemoved,
        EventKind::TaskInvalid,
        EventKind::TaskWritten,
        EventKind::WorkspaceTransition,
        EventKind::MetricReported,
        EventKind::ExperimentTransition,
        EventKind::StepIntent,
        EventKind::StepOutcome,
    ];

    /// The event's name, which is its `event` tag: `run.transition`.
    pub fn name(self) -> &'static str {
        match self {
            EventKind::RunTransition => "run.transition",
            EventKind::PhaseTransition => "phase.transition",
            EventKind::GateVerdict => "gate.verdict",
            EventKind::PermissionRequested => "permission.requested",
            EventKind::PermissionAnswered => "permission.answered",
            EventKind::AskHumanAsked => "ask_human.asked",
            EventKind::AskHumanAnswered => "ask_human.answered",
            EventKind::BudgetExceeded => "budget.exceeded",
            EventKind::TaskChanged => "task.changed",
            EventKind::TaskRenamed => "task.renamed",
            EventKind::TaskRemoved => "task.removed",
            EventKind::TaskInvalid => "task.invalid",
            EventKind::TaskWritten => "task.written",
            EventKind::WorkspaceTransition => "workspace.transition",
            EventKind::MetricReported => "metric.reported",
            EventKind::ExperimentTransition => "experiment.transition",
            EventKind::StepIntent => "step.intent",
            EventKind::StepOutcome => "step.outcome",
        }
    }

    /// The stage that makes this event fire.
    pub fn stage(self) -> Stage {
        match self {
            EventKind::GateVerdict
            | EventKind::AskHumanAsked
            | EventKind::AskHumanAnswered
            | EventKind::BudgetExceeded
            | EventKind::TaskWritten
            | EventKind::StepIntent
            | EventKind::StepOutcome => Stage::S8,
            EventKind::RunTransition
            | EventKind::PhaseTransition
            | EventKind::WorkspaceTransition => Stage::S3a,
            EventKind::PermissionRequested | EventKind::PermissionAnswered => Stage::S3b,
            EventKind::TaskChanged
            | EventKind::TaskRenamed
            | EventKind::TaskRemoved
            | EventKind::TaskInvalid => Stage::S4b,
            EventKind::MetricReported => Stage::S7,
            EventKind::ExperimentTransition => Stage::S15,
        }
    }
}

impl EventBody {
    /// Which event this body is.
    pub fn kind(&self) -> EventKind {
        match self {
            EventBody::RunTransition { .. } => EventKind::RunTransition,
            EventBody::PhaseTransition { .. } => EventKind::PhaseTransition,
            EventBody::GateVerdict { .. } => EventKind::GateVerdict,
            EventBody::PermissionRequested { .. } => EventKind::PermissionRequested,
            EventBody::PermissionAnswered { .. } => EventKind::PermissionAnswered,
            EventBody::AskHumanAsked { .. } => EventKind::AskHumanAsked,
            EventBody::AskHumanAnswered { .. } => EventKind::AskHumanAnswered,
            EventBody::BudgetExceeded { .. } => EventKind::BudgetExceeded,
            EventBody::TaskChanged { .. } => EventKind::TaskChanged,
            EventBody::TaskRenamed { .. } => EventKind::TaskRenamed,
            EventBody::TaskRemoved { .. } => EventKind::TaskRemoved,
            EventBody::TaskInvalid { .. } => EventKind::TaskInvalid,
            EventBody::TaskWritten { .. } => EventKind::TaskWritten,
            EventBody::WorkspaceTransition { .. } => EventKind::WorkspaceTransition,
            EventBody::MetricReported { .. } => EventKind::MetricReported,
            EventBody::ExperimentTransition { .. } => EventKind::ExperimentTransition,
            EventBody::StepIntent { .. } => EventKind::StepIntent,
            EventBody::StepOutcome { .. } => EventKind::StepOutcome,
        }
    }

    /// The event's name, as its `event` tag.
    pub fn name(&self) -> &'static str {
        self.kind().name()
    }

    /// The stage that makes this event fire.
    pub fn stage(&self) -> Stage {
        self.kind().stage()
    }
}
