//! The `workspace.*` methods: the git checkouts the owner made for runs (0.8, Workspace). A client lists them and
//! removes one; the engine makes them.

use crate::hook::WorkspaceKind;
use crate::state::WorkspaceState;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A workspace: a git checkout that lives longer than a run (0.8). The domain record is plain Rust, so this is its
/// wire form, without the state a hook kept in it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceSummary {
    pub id: String,
    pub kind: WorkspaceKind,
    pub path: String,
    pub git_dir: String,
    pub state: WorkspaceState,
    /// The live run that uses it, when one does.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<String>,
}

/// `workspace.list` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceListParams {
    pub board: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<WorkspaceState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<WorkspaceKind>,
}

/// `workspace.list` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceListResult {
    pub workspaces: Vec<WorkspaceSummary>,
}

/// `workspace.remove` params: remove a workspace nobody uses. There is no force: the built-in workspace refuses a
/// dirty or unmerged one (Stage 3a), and the way past is to deal with the work.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceRemoveParams {
    pub id: String,
}

/// Why a workspace was not removed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RemoveRefusal {
    /// It has changes that are not committed.
    Dirty,
    /// Its branch has commits that are not merged.
    Unmerged,
    /// A live run is using it.
    InUse,
}

/// `workspace.remove` result: removed, or refused with the reason and Collide's words for it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "outcome", rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkspaceRemoveResult {
    Removed {},
    Refused {
        reason: RemoveRefusal,
        message: String,
    },
}

/// `workspace.resume` params: resume a `failed` workspace through its implementation's `resume` hook (0.4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceResumeParams {
    pub id: String,
}

/// `workspace.resume` result: the workspace after the hook.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceResumeResult {
    pub workspace: WorkspaceSummary,
}
