//! The `task.*` methods: the corpus through views, one item, and the source's ops. Everything a source's task
//! carries is the provider protocol's own type ([`Task`], [`ApplyParams`], [`ApplyResult`],
//! [`PreviewResult`](crate::provider::PreviewResult)); these methods add the board they run against and what the owner
//! knows beyond one source: the checkouts, the runs, the views.
//!
//! A `task.list` result never carries a body: `body` is left out of every [`Task`] in it, as the in-app agent's
//! `query` does (0.11). `task.get` carries it.

use crate::change::{ItemState, LatestRun};
use crate::hook::HookLink;
use crate::item::OpDecl;
use crate::provider::{ApplyParams, ApplyResult, Collection, Task, TaskCollision, TaskInvalid};
use crate::view::View;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A view named in a request: one the board holds, or an ad-hoc one the filter bar made (0.11, Filter bar).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ViewRef {
    Named { name: String },
    Inline { view: Box<View> },
}

/// `task.list` params: the corpus through a view (`collide tasks [--view] [--ready] [--kind] [--checkout]`). With no
/// `view` the whole corpus. `ready` keeps the tasks the ready rule lists. `limit` and `cursor` page the answer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TaskListParams {
    pub board: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view: Option<ViewRef>,
    #[serde(default)]
    pub ready: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkout: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
}

/// A source whose last answer may be out of date, and why (`source.stale`, or a provider that is gone).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StaleSource {
    pub source: String,
    pub reason: String,
}

/// `task.list` result. `invalid` holds each item that could not be read, counted on the view (0.10). The board-wide
/// lists (`collections`, `invalid`, `divergent`, `collisions`, `stale_sources`) come with the first page only. They
/// are additive: a reader that does not know them reads the rest.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TaskListResult {
    pub tasks: Vec<Task>,
    #[serde(default)]
    pub collections: Vec<Collection>,
    #[serde(default)]
    pub invalid: Vec<TaskInvalid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
    /// The keys on the board whose copy in a workspace differs from the copy at its root.
    #[serde(default)]
    pub divergent: Vec<String>,
    /// The keys that are at different paths in different checkouts.
    #[serde(default)]
    pub collisions: Vec<TaskCollision>,
    #[serde(default)]
    pub stale_sources: Vec<StaleSource>,
    /// The first listing of a source is still running, so the tasks are what the last run cached, or fewer.
    #[serde(default)]
    pub refreshing: bool,
    /// What the view said of itself, for a request that named or gave one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view: Option<ViewReport>,
}

/// What a view reports for one evaluation (0.10, Views): why it has no members, and what went wrong on the way. A clean
/// view has no `error`, no counts and no unmatched keys. `steps` and `cycle` are the closure's run order.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ViewReport {
    pub name: String,
    /// Why the view has no members, when it was refused. Its other fields are then empty.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// How many tasks are members.
    pub members: u32,
    /// The tasks whose `where` raised an error, by reason.
    #[serde(default)]
    pub errors: BTreeMap<String, u64>,
    #[serde(default)]
    pub over_task_budget: u64,
    #[serde(default)]
    pub pass_budget_exhausted: bool,
    #[serde(default)]
    pub not_reached: u32,
    #[serde(default)]
    pub unmatched_include: Vec<String>,
    #[serde(default)]
    pub unmatched_exclude: Vec<String>,
    /// The closure's goal when it matches no task: the closure is then empty.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub goal_unmatched: Option<String>,
    /// The board's invalid items, which no view can match.
    #[serde(default)]
    pub invalid_items: u32,
    #[serde(default)]
    pub steps: Vec<Vec<String>>,
    #[serde(default)]
    pub cycle: Vec<String>,
}

impl ViewReport {
    /// The report of a view that was refused, with `reason`.
    pub fn failed(name: &str, reason: &str) -> ViewReport {
        ViewReport {
            name: name.to_string(),
            error: Some(reason.to_string()),
            ..ViewReport::default()
        }
    }
}

/// `task.get` params (`collide task <id>`). `id` may be the short form (`12` or `12-ua`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TaskGetParams {
    pub board: String,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkout: Option<String>,
}

/// One copy of a task in one checkout, for the drawer's per-checkout state and divergence (0.11, Task drawer).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TaskCopy {
    pub checkout: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    pub state: ItemState,
    pub version: String,
}

/// `task.get` result. `task` is absent when nothing matches, which is an answer and not an error (as the provider's
/// `get`). When a short id matches several items (two tasks numbered 12) `task` is absent and `ambiguous` lists their
/// keys, so the caller asks for the suffix. `runs` are the ids of the task's runs, newest first. `links` are the task's
/// links one hop out in both directions, as a hook reads them (0.5). `ops` are the ops its source declares for its
/// kind: what `collide task <id> <op>` maps its words to. Both are additive.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TaskGetResult {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<Task>,
    #[serde(default)]
    pub ambiguous: Vec<String>,
    #[serde(default)]
    pub copies: Vec<TaskCopy>,
    #[serde(default)]
    pub collisions: Vec<TaskCollision>,
    #[serde(default)]
    pub runs: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_run: Option<LatestRun>,
    #[serde(default)]
    pub links: Vec<HookLink>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ops: Vec<OpDecl>,
}

/// `task.apply` params (`collide task <id> <op>`, `collide task new`, `collide task reid`, and the drawer's forms):
/// the provider's `apply` call, on one source. `source` is left out when the id names it (`plan:ts-0001`); an op
/// that makes a task has no id, so it names the source. `commit` makes a one-file commit of what the write touched
/// (`--commit`). Who made the write is the connection's actor, never a field here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TaskApplyParams {
    pub board: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    pub apply: ApplyParams,
    #[serde(default)]
    pub commit: bool,
}

/// A write the daemon stored because its checkout is busy (0.4, Root busy). `item` is the Queue item that says
/// why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WriteQueued {
    pub item: String,
    pub reason: String,
}

/// `task.apply` result: the provider's answer, and the commit when `commit` was asked for and the write
/// succeeded. A queued write has no `result`; `queued` says why.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TaskApplyResult {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<ApplyResult>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub queued: Option<WriteQueued>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
}

/// `task.preview` params: what `task.apply` with the same call would do, with nothing written (0.10, `preview`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TaskPreviewParams {
    pub board: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    pub apply: ApplyParams,
}

/// Where a view is kept (0.10, Views).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ViewSource {
    /// `.collide/views/`, committed.
    Repo {},
    /// The user's board settings.
    Personal {},
    Pack {
        pack: String,
    },
}

/// One view of the board, with its counts. `error_count` is the items that failed to read on this view, and `error`
/// is why the view itself did not load, when it did not (one view's error never hides the others).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ViewEntry {
    pub view: View,
    pub source: ViewSource,
    pub task_count: u32,
    pub error_count: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// `task.views` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TaskViewsParams {
    pub board: String,
}

/// `task.views` result: the board's views, for the Field's view menu.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TaskViewsResult {
    pub views: Vec<ViewEntry>,
}

/// Where a saved view is written: the repo's `.collide/views/`, or the user's own board settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ViewTarget {
    Repo,
    Personal,
}

/// `task.save_view` params: Save as View (0.11, Filter bar). The view is checked as a view load checks it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TaskSaveViewParams {
    pub board: String,
    pub view: View,
    pub target: ViewTarget,
    /// Replace the file when there is one. Without it a view of the same name is refused.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub overwrite: bool,
}

/// `task.save_view` result: the file written.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TaskSaveViewResult {
    pub path: String,
}

/// `task.sections` params (`collide sections`). The result is [`crate::section::Sections`], as the CLI prints it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TaskSectionsParams {
    pub board: String,
}
