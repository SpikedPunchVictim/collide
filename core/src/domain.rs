//! The domain records of 0.8, Data model (core): board, line, task, run, phase, workspace and experiment. They are
//! the shapes the engine, the store and the UI's queries share, written in full because a type is a definition and
//! not behaviour (0.15, Tagged stubs).
//!
//! | record | what it is | state enum |
//! |---|---|---|
//! | [`BoardRoot`] | a board: repo root, lines, source configuration, saved views | |
//! | [`Line`] | a ref the user opened, with its own root, packs and trust | |
//! | [`CachedTask`] | a task from a source, with its category and provenance | [`crate::item::Category`] |
//! | [`Run`] | one workflow over one or more tasks | [`RunState`] |
//! | [`PhaseRun`] | one phase of a run, with its instances | [`PhaseState`] |
//! | [`Workspace`] | a git checkout, longer-lived than a run | [`WorkspaceState`] |
//! | [`ExperimentRecord`] | one task under two arms, and the runs it produced | |
//!
//! These are plain Rust records, with no serde or schema yet: none crosses a process boundary until the owner's
//! JSON-RPC methods (`ts-0032-pp`) and the store (Stage 3a) fix their wire form, and `PhaseKind` is not readable from
//! JSON. Ids are strings. Paths are strings, as they are in [`crate::hook`], so that a record can be stored as it is.
//! The state enums are the ones of [`crate::state`]; a transition between them is the state machines' (Stage 3a).

use crate::board::{Board, BoardPack};
use crate::hook::{HookStatus, WorkspaceKind};
use crate::item::Category;
use crate::provider::Task;
use crate::resolve::ResolvedImplementations;
use crate::state::{PhaseState, RunState, WorkspaceState};
use crate::view::View;
use crate::workflow::PhaseKind;
use serde_json::Value;
use std::collections::BTreeMap;

/// A board: the repo root, its lines, the source configuration and the saved views (0.8, Board). `config` is
/// `board.yaml`; the board-level keys come from the default line (0.12).
#[derive(Debug, Clone, PartialEq)]
pub struct BoardRoot {
    pub id: String,
    /// The repo root: the board root, which is the default line's root.
    pub root: String,
    pub config: Board,
    /// The default line first.
    pub lines: Vec<Line>,
    pub views: Vec<View>,
}

/// Whether the repo's packs and gate code of a line are trusted (0.9, Trust). Trust is stored outside the repo,
/// keyed by a hash that covers the packs, the lock, the pointer file, `board.yaml`, `config.yaml`, the views and the
/// tree hashes of `protectedPaths`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrustEntry {
    /// Nothing trusted yet, or the hash changed since.
    Untrusted,
    Trusted {
        hash: String,
    },
}

/// A line: a ref the user has opened on a board (0.12). `snapshot` is the commit the line's packs, workflows, roles,
/// prompts, gates and hooks are read at; it follows `ref_name` between runs, and a run pins the commit it launched
/// from.
#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    pub ref_name: String,
    /// The checkout where the line's hooks run (kind `line`). For the default line it is the board root.
    pub root: String,
    pub snapshot: Option<String>,
    /// This line's packs, in order. A line may reorder or replace the board's list.
    pub packs: Vec<BoardPack>,
    /// The default workflow, as `pack/workflow`.
    pub workflow: Option<String>,
    pub trust: TrustEntry,
    /// The path of the line's lock file, which serializes `record` and `integrate` at the root (0.4).
    pub lock: String,
}

/// How far a task's author is trusted (0.9): task text from an author outside the repo's trust list needs a
/// confirmation before a run, and never runs in `bypassPermissions`. A draft item has no author association and
/// counts as untrusted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrustLevel {
    Trusted,
    Untrusted,
}

/// Who wrote a task and how far to trust them (0.8, Task).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Provenance {
    pub author: Option<String>,
    pub trust: TrustLevel,
}

/// A task as the corpus caches it (0.8, Task cache): what its source reported, the status category, and the
/// provenance. The key is `source:shortId`.
#[derive(Debug, Clone, PartialEq)]
pub struct CachedTask {
    pub source: String,
    pub task: Task,
    pub category: Category,
    pub provenance: Provenance,
}

/// One task of a run, with its own membership (0.8, Run): its path at launch and its status when the run started.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunTask {
    pub key: String,
    pub path: Option<String>,
    pub status: String,
}

/// The arm of an experiment that a run belongs to (0.14).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArmRef {
    pub experiment: String,
    pub arm: String,
}

/// A run (0.8, Run): one workflow over one or more tasks. There is one live run per task key, except inside an
/// experiment. `state` carries its terminal state (`ended(<outcome>)`, `failed`, `stopped`) and any pause with its
/// cause.
#[derive(Debug, Clone, PartialEq)]
pub struct Run {
    pub id: String,
    pub tasks: Vec<RunTask>,
    /// The line's ref the run launched on.
    pub line: String,
    /// The commit the run pinned at launch.
    pub snapshot_commit: String,
    /// `pack/workflow`.
    pub workflow: String,
    pub launch: BTreeMap<String, Value>,
    /// The workspace id, once one exists.
    pub workspace: Option<String>,
    pub implementations: ResolvedImplementations,
    /// The commit a chain of rounds builds on (`chain.base`).
    pub chain_base: Option<String>,
    /// The round each phase has reached, counted on re-entry.
    pub rounds: BTreeMap<String, u32>,
    pub state: RunState,
    /// The fingerprint of 0.13: what was true of the run's setup. Its fields are fixed by Stage 3b.
    pub fingerprint: BTreeMap<String, Value>,
    pub annotations: Vec<String>,
    pub experiment: Option<ArmRef>,
}

/// Why a phase waits for a person (0.8, Phase; 0.7): the decision token that led to it, a budget, or an agent's
/// stop-and-report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AskHumanCause {
    Decision { token: String },
    Budget,
    AgentReport,
}

/// One `ask_human` event of a phase, with its cause.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AskHuman {
    pub cause: AskHumanCause,
}

/// One instance of a phase: the only one for an `agent` phase, one per model or instance for a `fanout` (0.8, Phase).
#[derive(Debug, Clone, PartialEq)]
pub struct PhaseInstance {
    pub id: String,
    /// The model alias it runs on, if the phase names one.
    pub model: Option<String>,
    /// The agent session id, once the session is bound (`SessionBound`).
    pub session_id: Option<String>,
    pub state: PhaseState,
}

/// A phase of a run (0.8, Phase). `dir` is a scratch directory outside the worktree. `gate_results` keeps the latest
/// verdict of each gate.
#[derive(Debug, Clone, PartialEq)]
pub struct PhaseRun {
    pub run: String,
    pub name: String,
    pub kind: PhaseKind,
    pub round: u32,
    pub role: Option<String>,
    pub instances: Vec<PhaseInstance>,
    pub start_head: Option<String>,
    pub end_head: Option<String>,
    pub dir: String,
    /// The ids of the sandbox workspaces made for the phase's instances.
    pub sandboxes: Vec<String>,
    /// The declared outputs that exist, as paths under `dir`.
    pub outputs: Vec<String>,
    pub final_reports: Vec<String>,
    pub gate_results: BTreeMap<String, HookStatus>,
    pub labels: Vec<String>,
    pub ask_human: Vec<AskHuman>,
}

/// A workspace: a git checkout (0.8, Workspace). It lives longer than any run and keeps its own state, merged key by
/// key (0.5). `kind` is `task`, `sandbox`, `line` or `experiment`.
#[derive(Debug, Clone, PartialEq)]
pub struct Workspace {
    pub id: String,
    pub kind: WorkspaceKind,
    pub path: String,
    pub git_dir: String,
    pub state: WorkspaceState,
    pub kept: BTreeMap<String, Value>,
}

/// The order of an experiment's runs (0.14, Design choices).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExperimentOrder {
    /// Each pair runs A, then B.
    Ab,
    /// Pairs alternate: A B, then B A.
    Abba,
}

/// An arm: a line plus launch parameters, env overrides, a profile and pack overrides (0.14).
#[derive(Debug, Clone, PartialEq)]
pub struct Arm {
    pub name: String,
    pub line: String,
    pub launch: BTreeMap<String, Value>,
    pub env: BTreeMap<String, String>,
    pub profile: Option<String>,
    /// A pack's name to the source it is pinned at in this arm.
    pub pack_overrides: BTreeMap<String, String>,
}

/// The design the user chose when creating the experiment (0.14): the order, its seed and the fixed pair count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExperimentDesign {
    pub order: ExperimentOrder,
    pub seed: u64,
    pub pairs: u32,
}

/// What a before or after inventory found: `git for-each-ref` and `git worktree list`, plus what the pack and the
/// adapter declared (0.14, Isolation). Each entry is a name and its value.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Inventory {
    pub entries: BTreeMap<String, String>,
}

/// An experiment (0.8, Experiment): the task, its arms, the primary metric, the design, the repetitions, the before
/// and after inventory, and the runs it produced.
#[derive(Debug, Clone, PartialEq)]
pub struct ExperimentRecord {
    pub id: String,
    pub task: String,
    pub arms: Vec<Arm>,
    pub primary_metric: String,
    pub design: ExperimentDesign,
    pub repetitions: u32,
    pub before: Option<Inventory>,
    pub after: Option<Inventory>,
    /// The ids of the runs the experiment produced.
    pub runs: Vec<String>,
}
