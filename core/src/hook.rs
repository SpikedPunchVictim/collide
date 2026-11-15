//! The hook protocol (0.5): what a hook reads on stdin, what it writes to the file named by `$COLLIDE_RESULT`, and
//! the one pure function that turns its exit and that file into the status the engine acts on. The runner that
//! starts hooks is a later item; the report line a hook appends to `$COLLIDE_REPORT` is in [`crate::report`].
//!
//! | what | type |
//! |---|---|
//! | stdin | [`HookInput`] |
//! | result file | [`HookResult`] |
//! | status in the file | [`HookStatus`] |

use crate::change::{Change, ItemState, LatestRun};
use crate::item::{Category, LinkDir};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// The `protocol` of a hook input.
pub const HOOK_PROTOCOL: u32 = 1;

/// A point binding's `timeout:` when it has none, for the task points (0.5). The binding struct is `ts-0012-ua`'s.
pub const TASK_POINT_TIMEOUT_SECS: u64 = 30;

/// The same for the move points, `task.before_move` and `task.after_move`, which may scan files (0.5).
pub const MOVE_POINT_TIMEOUT_SECS: u64 = 120;

/// The most bytes in a result's `outputs`, as compact JSON (0.5). Larger data belongs in an artifact.
pub const OUTPUTS_MAX_BYTES: usize = 64 * 1024;

/// A link of a `tasks[]` entry, one hop out, in either direction and resolved across sources (0.5). `stale` says
/// that the source holding the other end is stale, so `kind`, `status` and `category` are the last it reported.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HookLink {
    #[serde(rename = "type")]
    pub link_type: String,
    pub dir: LinkDir,
    pub key: String,
    pub kind: String,
    pub status: String,
    pub category: Category,
    pub stale: bool,
}

/// One entry of `tasks[]`: the fields and the path of a task, never its body, and its links. It is not the
/// provider's `Task`, which has a body, a version and every alias a hook has no use for, and which a hook must
/// not read: task text reaches a hook only through the fields a pack asks for.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HookTask {
    pub key: String,
    pub short_id: String,
    pub kind: String,
    pub state: ItemState,
    pub path: String,
    /// The provider's template fields: safe tokens (0.5).
    #[serde(default)]
    pub template_fields: BTreeMap<String, String>,
    #[serde(default)]
    pub fields: BTreeMap<String, Value>,
    #[serde(default)]
    pub links: Vec<HookLink>,
}

/// Inside an experiment: the experiment, the arm and the arm's directory, so a hook can skip a push (0.14).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HookExperiment {
    pub id: String,
    pub arm: String,
    pub dir: String,
}

/// The phase a hook runs in or for.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HookPhase {
    pub name: String,
    pub kind: String,
    pub round: u32,
    /// The phase's scratch directory, outside the worktree.
    pub dir: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum WorkspaceKind {
    Task,
    Sandbox,
    Line,
    Experiment,
}

/// A workspace: a git checkout (0.8).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HookWorkspace {
    pub kind: WorkspaceKind,
    pub path: String,
    pub git_dir: String,
}

/// An earlier phase of the run (0.8): its kind, round, heads, artifact paths and verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HistoryEntry {
    pub phase: String,
    pub kind: String,
    pub round: u32,
    pub start_head: Option<String>,
    pub end_head: Option<String>,
    #[serde(default)]
    pub artifacts: Vec<String>,
    pub verdict: Option<HookStatus>,
}

/// `state` (0.5): the outputs kept so far. `point` holds the outputs of earlier chain handlers at the same point.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HookState {
    #[serde(default)]
    pub run: BTreeMap<String, Value>,
    #[serde(default)]
    pub workspace: BTreeMap<String, Value>,
    #[serde(default)]
    pub point: BTreeMap<String, Value>,
}

/// What a hook reads on stdin (0.5). Settings travel here, never in the environment. The task points add `change`,
/// and `latestRun` when the task has a finished run; both are absent at every other hook. `workflow` is null
/// outside a run, such as at a UI write: a chain handler uses it, with `pack`, to ignore other packs' runs (0.4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HookInput {
    pub protocol: u32,
    pub hook: String,
    pub pack: String,
    pub workflow: Option<String>,
    pub board: String,
    pub line: String,
    /// Null outside an experiment.
    pub experiment: Option<HookExperiment>,
    /// The run's id, or null outside a run.
    pub run: Option<String>,
    #[serde(default)]
    pub launch: BTreeMap<String, Value>,
    #[serde(default)]
    pub config: BTreeMap<String, Value>,
    pub tasks: Vec<HookTask>,
    pub phase: Option<HookPhase>,
    pub workspace: Option<HookWorkspace>,
    #[serde(default)]
    pub history: Vec<HistoryEntry>,
    pub state: HookState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub change: Option<Change>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_run: Option<LatestRun>,
}

/// The verdict a hook writes. It is `ok`, `fail`, `unverified` (could not check here), `pending` (has not happened
/// yet), `refused` or `error`. `unverified` and `pending` are valid only from some hooks: see [`effective_result`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum HookStatus {
    Ok,
    Fail,
    Unverified,
    Pending,
    Refused,
    Error,
}

/// The result file (0.5). Its status is the verdict. `retryAfter` is a hint in seconds for `pending`; the engine
/// clamps it against the phase's `interval:` ([`crate::run_rules::poll_delay`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HookResult {
    pub status: HookStatus,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub outputs: BTreeMap<String, Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry_after: Option<u32>,
}

/// What kind of hook ran, as far as the result rules care. `Point` is a handler bound to a named point.
/// `IntegratePullRequest` is an `integrate` implementation for a pull request, which answers `pending` until the
/// pull request merges; the runner knows which implementation it ran.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookKind {
    Workspace,
    Record,
    Integrate,
    IntegratePullRequest,
    Sync,
    Gate,
    Script,
    Wait,
    Decision,
    Point,
}

impl HookKind {
    pub fn may_answer_pending(self) -> bool {
        matches!(self, HookKind::Wait | HookKind::IntegratePullRequest)
    }

    fn may_answer_unverified(self) -> bool {
        matches!(
            self,
            HookKind::Gate | HookKind::Script | HookKind::Wait | HookKind::Point
        )
    }
}

/// Why `pending` is an error from this kind of hook (0.5). The run-state rules add where it happened.
pub fn misplaced_pending_message(kind: HookKind) -> String {
    format!(
        "`pending` is valid only from a wait hook or a pull request integrate, not from {kind:?}"
    )
}

fn error(message: String) -> HookResult {
    HookResult {
        status: HookStatus::Error,
        message,
        outputs: BTreeMap::new(),
        retry_after: None,
    }
}

/// The result that counts, from the kind of hook, whether its process exited zero, and the bytes of its result
/// file (`None` when there is none). The file's status is the verdict, and a child tool's exit code is never one,
/// so the result is `error` when:
/// - the process did not exit zero, whatever the file says;
/// - the file is missing, or is not a [`HookResult`];
/// - its `outputs` are over [`OUTPUTS_MAX_BYTES`] as compact JSON;
/// - it is `pending`, and the hook is not a `wait` hook or a pull request `integrate`;
/// - it is `unverified`, and the hook is not a gate, a `script` or `wait` hook, or a handler at a named point.
///
/// An `error` made here carries the reason as its message and has no outputs. Any other result is returned as
/// written.
pub fn effective_result(kind: HookKind, exit_zero: bool, file: Option<&[u8]>) -> HookResult {
    if !exit_zero {
        return error("hook exited non-zero".to_string());
    }
    let Some(file) = file else {
        return error("no result file".to_string());
    };
    let result: HookResult = match serde_json::from_slice(file) {
        Ok(result) => result,
        Err(reason) => return error(format!("invalid result file: {reason}")),
    };
    let outputs_len = serde_json::to_vec(&result.outputs)
        .expect("outputs are JSON")
        .len();
    if outputs_len > OUTPUTS_MAX_BYTES {
        return error(format!(
            "outputs are {outputs_len} bytes, over the {OUTPUTS_MAX_BYTES} byte cap"
        ));
    }
    if result.status == HookStatus::Pending && !kind.may_answer_pending() {
        return error(misplaced_pending_message(kind));
    }
    if result.status == HookStatus::Unverified && !kind.may_answer_unverified() {
        return error(format!(
            "`unverified` is valid only from a gate, script, wait or named-point hook, not from {kind:?}"
        ));
    }
    result
}
