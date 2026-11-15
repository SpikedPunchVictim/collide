//! The `run.*` methods: start a run, watch it, steer it and stop it (`collide run`, `collide stop`, Launch, Tracks,
//! Output, Review and the take-over of 0.11). The decisions of a run are Queue items (`queue.*`), and the run's own
//! events come on the event channel.

use crate::agent::AgentEvent;
use crate::hook::HookStatus;
use crate::state::{PhaseState, RunState};
use crate::workflow::PhaseKind;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// A phase's kind: the five names of [`PhaseKind`]. That enum is written to JSON but not read from it, so this is the
/// wire form, and a test pins the names to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum PhaseKindWire {
    Agent,
    Fanout,
    Script,
    Wait,
    Human,
}

impl From<PhaseKind> for PhaseKindWire {
    fn from(kind: PhaseKind) -> Self {
        match kind {
            PhaseKind::Agent => PhaseKindWire::Agent,
            PhaseKind::Fanout => PhaseKindWire::Fanout,
            PhaseKind::Script => PhaseKindWire::Script,
            PhaseKind::Wait => PhaseKindWire::Wait,
            PhaseKind::Human => PhaseKindWire::Human,
        }
    }
}

/// Whether a run's agent is driven by Collide or sat in by a person in a terminal (0.11, New session).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RunMode {
    #[default]
    Headless,
    /// A New session or a Plan… session: interactive Claude in a fresh worktree, taken over from the first moment. The
    /// caller makes the scratch task or the draft proposal first, with `task.apply`, and names it in `tasks`.
    Interactive,
}

/// `run.start` params (`collide run`, Launch, the prompt box): one workflow over one or more tasks. Every field the
/// caller leaves out has the default of 0.11's run preview: the board's default workflow, its default line, the
/// view's or the board's profile. `launch` is the pack's `launch` parameters. There is no actor field: the connection
/// is who started it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RunStartParams {
    pub board: String,
    /// Task keys, `source:shortId`.
    pub tasks: Vec<String>,
    /// `pack/workflow`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workflow: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<String>,
    #[serde(default)]
    pub launch: BTreeMap<String, Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    #[serde(default)]
    pub mode: RunMode,
}

/// `run.preview` params: the same call as `run.start`, so the preview is what the start would do.
pub type RunPreviewParams = RunStartParams;

/// A reason a run was not started: one task's, or the launch's own when `task` is absent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LaunchProblem {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
    pub message: String,
}

/// `run.start` result: started, or refused with every problem (a task the workflow does not accept, a live run on
/// the task, an alias with no mapping).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "outcome", rename_all = "snake_case", deny_unknown_fields)]
pub enum RunStartResult {
    Started { run: String },
    Refused { problems: Vec<LaunchProblem> },
}

/// One phase of the workflow, as the preview lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PreviewPhase {
    pub name: String,
    pub kind: PhaseKindWire,
}

/// One task of the preview: its status at the line's ref, and whether the workflow accepts it. `lacks` names the
/// field or operation a refused task lacks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PreviewTask {
    pub key: String,
    pub status: String,
    pub accepted: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lacks: Option<String>,
}

/// Why a profile applies (0.11, Launch shows a run preview).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProfileReason {
    Picked,
    View,
    Rule,
    BoardDefault,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PreviewProfile {
    pub name: String,
    pub reason: ProfileReason,
}

/// One resolved setting of the preview with its layer (0.7).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PreviewSetting {
    pub key: String,
    pub value: Value,
    pub layer: crate::owner::SettingLayer,
}

/// A user hook or plugin a phase will load, with the flags on those that change behaviour (0.3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PreviewHook {
    pub phase: String,
    pub name: String,
    #[serde(default)]
    pub flags: Vec<String>,
}

/// A badge of the preview (0.11).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PreviewBadge {
    /// A pack loaded from a `dev:` folder.
    DevPack,
    /// A phase with `agentWork`.
    AgentWork,
}

/// `run.preview` result: what 0.11 lists before anything starts. `implementations` maps `workspace` and `integrate`
/// to `builtin` or `pack/name`; a workflow with no `integrate` has no such key, so it cannot be accepted. The
/// expected duration is absent until three earlier runs of the workflow have timings, and the UI says "unknown".
/// `model_endpoint_host` is the host of a redirected model endpoint, and never a path, a query or a key (0.3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RunPreviewResult {
    pub line: String,
    pub workflow: String,
    pub phases: Vec<PreviewPhase>,
    pub implementations: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget_usd: Option<f64>,
    #[serde(default)]
    pub launch: BTreeMap<String, Value>,
    pub tasks: Vec<PreviewTask>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<PreviewProfile>,
    #[serde(default)]
    pub settings: Vec<PreviewSetting>,
    #[serde(default)]
    pub user_hooks: Vec<PreviewHook>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_endpoint_host: Option<String>,
    #[serde(default)]
    pub badges: Vec<PreviewBadge>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_duration_secs: Option<u64>,
}

/// One task of a run, with its status when the run started.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RunTaskRef {
    pub key: String,
    pub status: String,
}

/// Whether a take-over's terminal is open (0.11, Talking to an agent): closing the view detaches, and the session
/// goes on in its shim.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum TakeOverState {
    Attached {},
    Detached {},
}

/// One instance of a phase: the only one of an `agent` phase, one for each model or instance of a `fanout`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct InstanceSummary {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// The agent session's id, once bound.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    pub state: PhaseState,
}

/// One phase of a run, for a track's segment: its instances' states, its real times and its gate verdicts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PhaseSummary {
    pub name: String,
    pub kind: PhaseKindWire,
    pub round: u32,
    pub instances: Vec<InstanceSummary>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ended_ms: Option<u64>,
    #[serde(default)]
    pub gate_results: BTreeMap<String, HookStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub take_over: Option<TakeOverState>,
}

/// A run for a track and for a list: the domain record's wire form (the record is plain Rust). Times are on the wall
/// clock; the spans and metrics of a run are Stage 9c's.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RunSummary {
    pub id: String,
    pub tasks: Vec<RunTaskRef>,
    pub line: String,
    /// `pack/workflow`.
    pub workflow: String,
    pub state: RunState,
    pub started_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ended_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
    pub phases: Vec<PhaseSummary>,
}

/// Which runs `run.list` returns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum RunScope {
    Live,
    Finished,
    #[default]
    All,
}

/// `run.list` params: the runs for Tracks, and for an app that reattaches to the daemon's runs after it restarts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RunListParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub board: Option<String>,
    #[serde(default)]
    pub scope: RunScope,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
}

/// `run.list` result, newest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RunListResult {
    pub runs: Vec<RunSummary>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

/// `run.get` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RunGetParams {
    pub run: String,
}

/// `run.get` result: the summary and what the run pinned at launch (0.8, Run). `implementations` maps `workspace` and
/// `integrate` to `builtin` or `pack/name`. `rounds` counts how often each phase was entered.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RunGetResult {
    pub summary: RunSummary,
    pub snapshot_commit: String,
    #[serde(default)]
    pub launch: BTreeMap<String, Value>,
    #[serde(default)]
    pub implementations: BTreeMap<String, String>,
    #[serde(default)]
    pub rounds: BTreeMap<String, u32>,
    #[serde(default)]
    pub fingerprint: BTreeMap<String, Value>,
    #[serde(default)]
    pub annotations: Vec<String>,
}

/// `run.stop` params (`collide stop`): the stop of 0.5. `grace_secs` is the time between SIGTERM and SIGKILL; the
/// default is the daemon's.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RunStopParams {
    pub run: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grace_secs: Option<u32>,
}

/// A process still alive after the final rescan (0.5, Stopping). One its hook recorded is stopped by the cleanup
/// hook; one it did not record is listed here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Survivor {
    pub pid: u32,
    pub command: String,
}

/// `run.stop` result: the run's state after the stop, how many processes the first scan signalled, and the
/// survivors.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RunStopResult {
    pub state: RunState,
    pub signalled: u32,
    #[serde(default)]
    pub survivors: Vec<Survivor>,
}

/// `run.diff` params: a run's diff for Review (0.11, Drill-down). With `path`, that file's patch is in the result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RunDiffParams {
    pub run: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DiffStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
}

/// One changed file. The counts are absent for a binary file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DiffFile {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub old_path: Option<String>,
    pub status: DiffStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub additions: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deletions: Option<u32>,
}

/// A folder the diff collapses (dependency stores and build output that `.gitignore` does not hide, 0.11): its file
/// count and size are shown instead of its files. Lock files are never collapsed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CollapsedFolder {
    pub path: String,
    pub files: u32,
    pub bytes: u64,
}

/// One file's patch as text. It is untrusted (0.9). `truncated` says the text was cut, and `bytes` is the patch's
/// full size.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FilePatch {
    pub path: String,
    pub text: String,
    pub bytes: u64,
    pub truncated: bool,
}

/// `run.diff` result, from `base` to `head`. `files` is capped, and `more` counts the files left out (the stored
/// list is capped with the rest counted, 0.11).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RunDiffResult {
    pub base: String,
    pub head: String,
    pub files: Vec<DiffFile>,
    #[serde(default)]
    pub collapsed: Vec<CollapsedFolder>,
    pub more: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patch: Option<FilePatch>,
}

/// One normalized event of an agent's stream, with its place in the phase's log and the time the shim saw it (from
/// the journal's `wall_ns`, so a replayed log gives the same time). `index` counts from 0 in the phase instance's log.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OutputEntry {
    pub index: u64,
    pub wall_ms: u64,
    pub event: AgentEvent,
}

/// `run.output` params: one page of a phase's normalized stream, for the Output pane (0.11, Agent output). `from` is
/// the first `index`, or the start. `instance` picks one of a fan-out's instances, and is left out for a phase with
/// one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RunOutputParams {
    pub run: String,
    pub phase: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instance: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

/// `run.output` result. `next` is the `index` to ask for after this page, absent when the page reached the end of
/// the log so far. New entries after that come as `run.output` notifications.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RunOutputResult {
    pub entries: Vec<OutputEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next: Option<u64>,
}

/// The params of the `run.output` notification: one new entry of a phase's stream, sent to a connection that has
/// subscribed to events (`daemon.subscribe`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RunOutputNotice {
    pub run: String,
    pub phase: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instance: Option<String>,
    pub entry: OutputEntry,
}

/// `run.intervene` params: send a note to a running phase (0.11, Talking to an agent is the exception). The note is
/// the person's words and goes to the agent as a message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RunIntervenParams {
    pub run: String,
    pub phase: String,
    pub note: String,
}

/// `run.take_over` params: open the phase's session in a terminal through the owner's PTY, or attach again to one
/// that was detached. `cols` and `rows` are the terminal's size.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RunTakeOverParams {
    pub run: String,
    pub phase: String,
    pub cols: u16,
    pub rows: u16,
}

/// `run.take_over` result. `replay` is the last 1 MiB of the terminal's output as base64, and the daemon sends it
/// atomically with the attach (0.2): `run.terminal_output` notifications for `terminal` follow it with no gap and no
/// repeat. Terminal bytes are not always UTF-8, so every byte string of this protocol is base64.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RunTakeOverResult {
    pub terminal: String,
    pub session_id: String,
    pub replay: String,
}

/// `run.terminal_input` params: keys for the terminal, as base64.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RunTerminalInputParams {
    pub terminal: String,
    pub data: String,
}

/// `run.terminal_resize` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RunTerminalResizeParams {
    pub terminal: String,
    pub cols: u16,
    pub rows: u16,
}

/// `run.detach` params: close the terminal's view. The session goes on in its shim (0.11).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RunDetachParams {
    pub terminal: String,
}

/// `run.end_take_over` params: End take-over. The phase's gates then run as usual (0.11).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RunEndTakeOverParams {
    pub run: String,
    pub phase: String,
}

/// The params of the `run.terminal_output` notification: bytes the take-over's terminal wrote, as base64.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RunTerminalOutput {
    pub terminal: String,
    pub data: String,
}
