//! The state machines of a run, a phase and a workspace (0.8; Stage 3a). [`crate::state`] holds the states and this
//! module holds the moves between them, as pure functions that read no clock and touch nothing. The engine and the
//! owner ask before they change a state, and a refused move is an error that names both states.
//!
//! Each machine has one table, written out below, and every pair of states it does not list is refused:
//!
//! | machine | function | the way in |
//! |---|---|---|
//! | run | [`run_transition`] | [`RunVia::Engine`], or [`RunVia::IntegrateAnswer`] for `integrating` (0.4) |
//! | phase | [`phase_transition`], [`phase_ending`] | none: one table |
//! | workspace | [`workspace_transition`] | [`WorkspaceVia::Engine`], or [`WorkspaceVia::ResumeHook`] out of `failed` |
//!
//! A state that has ended is final. `ended`, `failed` and `stopped` for a run and `ended`, `failed`, `stopped` and
//! `lost` for a phase accept nothing (0.7, Outcomes and terminal states). The one door out of a run's `ended` is
//! accepting it: an `integrate` answer takes `ended` to `integrating` and back (0.4), and that door is
//! [`RunVia::IntegrateAnswer`] only.
//!
//! The table compares the *kind* of each state, so `paused` with any cause is one row. A pause that changes cause
//! goes through `running` first.

use crate::state::{FailCause, PauseCause, PhaseState, RunState, WorkspaceState};
use std::fmt;

/// A move a machine refused. It names the machine and both states, such as `ended (done)` and `running`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransitionError {
    /// `run`, `phase` or `workspace`.
    pub machine: &'static str,
    pub from: String,
    pub to: String,
    /// Why, when the pair is refused for a reason worth saying.
    pub note: Option<&'static str>,
}

impl fmt::Display for TransitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "a {} cannot go from {} to {}",
            self.machine, self.from, self.to
        )?;
        if let Some(note) = self.note {
            write!(f, ": {note}")?;
        }
        Ok(())
    }
}

impl std::error::Error for TransitionError {}

fn pause_name(cause: PauseCause) -> &'static str {
    match cause {
        PauseCause::AskHuman => "ask_human",
        PauseCause::UsageLimit => "usage_limit",
        PauseCause::Disk => "disk",
    }
}

// --- Run ---

/// Who asks for a run's move (0.4). `integrating` is entered and left only by an `integrate` answer, so the engine
/// cannot put a run there or take it out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunVia {
    Engine,
    IntegrateAnswer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RunKind {
    Running,
    Paused,
    Integrating,
    Ended,
    Failed,
    Stopped,
}

impl RunKind {
    fn of(state: &RunState) -> RunKind {
        match state {
            RunState::Running {} => RunKind::Running,
            RunState::Paused { .. } => RunKind::Paused,
            RunState::Integrating {} => RunKind::Integrating,
            RunState::Ended { .. } => RunKind::Ended,
            RunState::Failed {} => RunKind::Failed,
            RunState::Stopped {} => RunKind::Stopped,
        }
    }
}

/// Every move of a run, with the way in. A run pauses and resumes, and ends `ended(<outcome>)` when it reaches an
/// `end`, `failed`, or `stopped`. A paused run can fail or be stopped, and goes back to `running` to carry on; it does
/// not end from a pause, because a human answer is a token the run follows once it runs again (0.7, `ask_human`).
const RUN_TABLE: [(RunKind, RunKind, RunVia); 11] = [
    (RunKind::Running, RunKind::Paused, RunVia::Engine),
    (RunKind::Running, RunKind::Ended, RunVia::Engine),
    (RunKind::Running, RunKind::Failed, RunVia::Engine),
    (RunKind::Running, RunKind::Stopped, RunVia::Engine),
    (RunKind::Paused, RunKind::Running, RunVia::Engine),
    (RunKind::Paused, RunKind::Failed, RunVia::Engine),
    (RunKind::Paused, RunKind::Stopped, RunVia::Engine),
    // Accepting a finished run (0.4): `pending` enters `integrating`, `merged` leaves it. A `merged` straight away, or
    // a `pending` answered again, keeps the state it is in.
    (
        RunKind::Ended,
        RunKind::Integrating,
        RunVia::IntegrateAnswer,
    ),
    (RunKind::Ended, RunKind::Ended, RunVia::IntegrateAnswer),
    (
        RunKind::Integrating,
        RunKind::Integrating,
        RunVia::IntegrateAnswer,
    ),
    (
        RunKind::Integrating,
        RunKind::Ended,
        RunVia::IntegrateAnswer,
    ),
];

fn run_name(state: &RunState) -> String {
    match state {
        RunState::Running {} => "running".to_string(),
        RunState::Paused { cause } => format!("paused ({})", pause_name(*cause)),
        RunState::Integrating {} => "integrating".to_string(),
        RunState::Ended { outcome } => format!("ended ({})", outcome.as_str()),
        RunState::Failed {} => "failed".to_string(),
        RunState::Stopped {} => "stopped".to_string(),
    }
}

fn run_note(from: RunKind, to: RunKind, via: RunVia) -> Option<&'static str> {
    let integrate_door = |kind| matches!(kind, RunKind::Integrating);
    if via == RunVia::Engine && (integrate_door(from) || integrate_door(to)) {
        return Some("`integrating` is entered and left only by an `integrate` answer");
    }
    match from {
        RunKind::Ended if via == RunVia::Engine => Some("an ended run is final"),
        RunKind::Failed => Some("a failed run is final"),
        RunKind::Stopped => Some("a stopped run is final"),
        _ => None,
    }
}

/// Checks a move of a run from `from` to `to` (0.7, 0.8), asked for `via`. Nothing is changed: the caller keeps `to`
/// when this is `Ok`.
pub fn run_transition(from: &RunState, to: &RunState, via: RunVia) -> Result<(), TransitionError> {
    let (from_kind, to_kind) = (RunKind::of(from), RunKind::of(to));
    if RUN_TABLE.contains(&(from_kind, to_kind, via)) {
        return Ok(());
    }
    Err(TransitionError {
        machine: "run",
        from: run_name(from),
        to: run_name(to),
        note: run_note(from_kind, to_kind, via),
    })
}

// --- Phase ---

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PhaseKind {
    Pending,
    Running,
    Paused,
    Ended,
    Failed,
    Stopped,
    Lost,
}

impl PhaseKind {
    fn of(state: &PhaseState) -> PhaseKind {
        match state {
            PhaseState::Pending {} => PhaseKind::Pending,
            PhaseState::Running {} => PhaseKind::Running,
            PhaseState::Paused { .. } => PhaseKind::Paused,
            PhaseState::Ended {} => PhaseKind::Ended,
            PhaseState::Failed { .. } => PhaseKind::Failed,
            PhaseState::Stopped {} => PhaseKind::Stopped,
            PhaseState::Lost {} => PhaseKind::Lost,
        }
    }

    fn is_final(self) -> bool {
        matches!(
            self,
            PhaseKind::Ended | PhaseKind::Failed | PhaseKind::Stopped | PhaseKind::Lost
        )
    }
}

/// Every move of a phase. A phase that has not started can fail to start or be stopped, but not pause or be lost,
/// because no shim owns it yet. `lost` is the move of a shim that died: from `running` or `paused` only (0.2).
const PHASE_TABLE: [(PhaseKind, PhaseKind); 12] = [
    (PhaseKind::Pending, PhaseKind::Running),
    (PhaseKind::Pending, PhaseKind::Failed),
    (PhaseKind::Pending, PhaseKind::Stopped),
    (PhaseKind::Running, PhaseKind::Paused),
    (PhaseKind::Running, PhaseKind::Ended),
    (PhaseKind::Running, PhaseKind::Failed),
    (PhaseKind::Running, PhaseKind::Stopped),
    (PhaseKind::Running, PhaseKind::Lost),
    (PhaseKind::Paused, PhaseKind::Running),
    (PhaseKind::Paused, PhaseKind::Failed),
    (PhaseKind::Paused, PhaseKind::Stopped),
    (PhaseKind::Paused, PhaseKind::Lost),
];

fn phase_name(state: &PhaseState) -> String {
    match state {
        PhaseState::Pending {} => "pending".to_string(),
        PhaseState::Running {} => "running".to_string(),
        PhaseState::Paused { cause } => format!("paused ({})", pause_name(*cause)),
        PhaseState::Ended {} => "ended".to_string(),
        PhaseState::Failed {
            cause: Some(FailCause::Disk),
        } => "failed (disk)".to_string(),
        PhaseState::Failed { cause: None } => "failed".to_string(),
        PhaseState::Stopped {} => "stopped".to_string(),
        PhaseState::Lost {} => "lost".to_string(),
    }
}

/// Checks a move of a phase from `from` to `to` (0.8, Phase). Nothing is changed: the caller keeps `to` when this is
/// `Ok`.
pub fn phase_transition(from: &PhaseState, to: &PhaseState) -> Result<(), TransitionError> {
    let (from_kind, to_kind) = (PhaseKind::of(from), PhaseKind::of(to));
    if PHASE_TABLE.contains(&(from_kind, to_kind)) {
        return Ok(());
    }
    let note = if from_kind.is_final() {
        Some("a phase that has ended is final")
    } else if to_kind == PhaseKind::Lost {
        Some("only a running or paused phase can be lost")
    } else {
        None
    };
    Err(TransitionError {
        machine: "phase",
        from: phase_name(from),
        to: phase_name(to),
        note,
    })
}

/// What the daemon knows when a phase's shim is gone (0.2), as three facts it has read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhaseEnding {
    /// The shim wrote `exit.json`.
    pub exit_recorded: bool,
    /// The agent's stream reached a `result`.
    pub result_seen: bool,
    /// The volume is under the free-space floor, so nothing more could be written.
    pub volume_full: bool,
}

/// Where a phase goes when its shim is gone (0.2, Durability), for [`phase_transition`] to check from `running` or
/// `paused`:
/// - no `result` on a full volume is `failed` with cause `disk`, even though no `exit.json` could be written;
/// - no `exit.json` otherwise is `lost`, whatever the stream reported;
/// - an exit record with a `result` is `ended`;
/// - an exit record with no `result` is `failed`, with no cause known.
pub fn phase_ending(ending: &PhaseEnding) -> PhaseState {
    if ending.volume_full && !ending.result_seen {
        return PhaseState::Failed {
            cause: Some(FailCause::Disk),
        };
    }
    if !ending.exit_recorded {
        return PhaseState::Lost {};
    }
    if ending.result_seen {
        PhaseState::Ended {}
    } else {
        PhaseState::Failed { cause: None }
    }
}

// --- Workspace ---

/// Who asks for a workspace's move (0.8, Workspace). A `failed` workspace leaves only through the `resume` hook.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceVia {
    Engine,
    ResumeHook,
}

/// Every move of a workspace. It goes down `provisioning → ready → in_use → idle → removable → removed` one state at
/// a time, and an `idle` workspace is used again by the next run, because it lives longer than any run (0.8). Any
/// state with work in flight can fail. A `resume` takes `failed` back to `ready`: the hook has made it usable again,
/// and the engine moves it on from there.
const WORKSPACE_TABLE: [(WorkspaceState, WorkspaceState, WorkspaceVia); 12] = [
    (
        WorkspaceState::Provisioning,
        WorkspaceState::Ready,
        WorkspaceVia::Engine,
    ),
    (
        WorkspaceState::Provisioning,
        WorkspaceState::Failed,
        WorkspaceVia::Engine,
    ),
    (
        WorkspaceState::Ready,
        WorkspaceState::InUse,
        WorkspaceVia::Engine,
    ),
    (
        WorkspaceState::Ready,
        WorkspaceState::Failed,
        WorkspaceVia::Engine,
    ),
    (
        WorkspaceState::InUse,
        WorkspaceState::Idle,
        WorkspaceVia::Engine,
    ),
    (
        WorkspaceState::InUse,
        WorkspaceState::Failed,
        WorkspaceVia::Engine,
    ),
    (
        WorkspaceState::Idle,
        WorkspaceState::InUse,
        WorkspaceVia::Engine,
    ),
    (
        WorkspaceState::Idle,
        WorkspaceState::Removable,
        WorkspaceVia::Engine,
    ),
    (
        WorkspaceState::Idle,
        WorkspaceState::Failed,
        WorkspaceVia::Engine,
    ),
    (
        WorkspaceState::Removable,
        WorkspaceState::Removed,
        WorkspaceVia::Engine,
    ),
    (
        WorkspaceState::Removable,
        WorkspaceState::Failed,
        WorkspaceVia::Engine,
    ),
    (
        WorkspaceState::Failed,
        WorkspaceState::Ready,
        WorkspaceVia::ResumeHook,
    ),
];

fn workspace_name(state: WorkspaceState) -> &'static str {
    match state {
        WorkspaceState::Provisioning => "provisioning",
        WorkspaceState::Ready => "ready",
        WorkspaceState::InUse => "in_use",
        WorkspaceState::Idle => "idle",
        WorkspaceState::Removable => "removable",
        WorkspaceState::Removed => "removed",
        WorkspaceState::Failed => "failed",
    }
}

/// Checks a move of a workspace from `from` to `to` (0.8, Workspace), asked for `via`.
pub fn workspace_transition(
    from: WorkspaceState,
    to: WorkspaceState,
    via: WorkspaceVia,
) -> Result<(), TransitionError> {
    if WORKSPACE_TABLE.contains(&(from, to, via)) {
        return Ok(());
    }
    let note = match from {
        WorkspaceState::Failed => Some("a failed workspace leaves only through the `resume` hook"),
        WorkspaceState::Removed => Some("a removed workspace is gone"),
        _ => None,
    };
    Err(TransitionError {
        machine: "workspace",
        from: workspace_name(from).to_string(),
        to: workspace_name(to).to_string(),
        note,
    })
}
