//! The `task.*` methods: the corpus through views, one item, and the source's ops. Everything a source's task
//! carries is the provider protocol's own type ([`Task`], [`ApplyParams`], [`ApplyResult`],
//! [`PreviewResult`](crate::provider::PreviewResult)); these methods add the board they run against and what the owner
//! knows beyond one source: the checkouts, the runs, the views.
//!
//! A `task.list` result never carries a body: `body` is left out of every [`Task`] in it, as the in-app agent's
//! `query` does (0.11). `task.get` carries it.

use crate::change::{ItemState, LatestRun};
use crate::hook::HookLink;
use crate::item::{Category, LinkTypeDecl, OpDecl, StatusDecl};
use crate::owner::run::{CollapsedFolder, DiffFile, FilePatch};
use crate::provider::{
    ApplyParams, ApplyResult, Collection, FieldDecl, Scope, Task, TaskCollision, TaskInvalid,
};
use crate::ready::UnknownTarget;
use crate::state::RunState;
use crate::view::View;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// A view named in a request: one the board holds, or an ad-hoc one the filter bar made (0.11, Filter bar).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ViewRef {
    Named { name: String },
    Inline { view: Box<View> },
}

/// `task.list` params: the corpus through a view (`collide tasks [--view] [--ready] [--kind] [--checkout]`). With no
/// `view` the whole corpus. `ready` keeps the tasks the ready rule lists. `filter` is a CEL expression, checked,
/// compiled and budgeted as a view's `where` is: it keeps the members of the view asked for (the whole corpus when
/// there is none) that it matches, in the view's order, whatever made them members. `limit` and `cursor` page the
/// answer.
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
    pub filter: Option<String>,
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

/// A source of the board that is not running, and why: its pack did not load, was not trusted, or its provider did not
/// start. It never declared anything this run, so it is not in `stale_sources`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NotStartedSource {
    pub source: String,
    pub reason: String,
}

/// What the Field draws from one started source of the board (0.10, Task sources; 0.11, the Field): its statuses in
/// the order the source declared them, each with its category, the status a category write produces, its fields with
/// their labels, and its id patterns. `pack` is the pack that provides it, left out for the built-in scratch source.
/// `blocking_links` says whether any of its link types blocks: a source with none has no Waits on and Unblocks.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BoardSource {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pack: Option<String>,
    pub scope: Scope,
    pub statuses: Vec<StatusDecl>,
    #[serde(default)]
    pub category_status: BTreeMap<Category, String>,
    #[serde(default)]
    pub fields: Vec<FieldDecl>,
    #[serde(default)]
    pub blocking_links: bool,
    pub id_pattern: String,
    pub short_id_pattern: String,
}

/// One copy of a task, in a workspace, that differs from the copy at the root it was forked from. `run` is the run
/// whose workspace the checkout is, when it is one: a checkout that no run made (a worktree of the person's own) has
/// none.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DivergentCopy {
    pub checkout: String,
    pub status: String,
    pub category: Category,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<String>,
}

/// A key whose copy in a workspace differs from the copy at its root, with each such copy ("Review in Run R-121").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TaskDivergence {
    pub key: String,
    pub copies: Vec<DivergentCopy>,
}

/// A key whose only copy is in a worktree, with its checkout and the branch that checkout is on, if any.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TaskBranchOnly {
    pub key: String,
    pub checkout: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
}

/// `task.list` result. `invalid` holds each item that could not be read, counted on the view (0.10). The board-wide
/// lists (`collections`, `invalid`, `divergent`, `divergence`, `branch_only`, `collisions`, `stale_sources`,
/// `not_started`, `sources`), `sources_started` and `sources_unread` come with the first page only. They are
/// additive: a reader that does not know them reads the rest. `divergent` is the keys alone, which the CLI prints;
/// `divergence` says how each differs.
/// `sources` are the sources of the board that started, in name order: a source that did not is in `not_started`.
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
    /// The same keys, each with the copies that differ: their status, category and checkout, and their run.
    #[serde(default)]
    pub divergence: Vec<TaskDivergence>,
    /// The keys whose only copy is off the default line's root, each with its checkout and branch.
    #[serde(default)]
    pub branch_only: Vec<TaskBranchOnly>,
    /// What each started source declared, for the Field's columns and the drawer's fields.
    #[serde(default)]
    pub sources: Vec<BoardSource>,
    /// The keys that are at different paths in different checkouts.
    #[serde(default)]
    pub collisions: Vec<TaskCollision>,
    #[serde(default)]
    pub stale_sources: Vec<StaleSource>,
    /// The sources of the board that could not start and have no declaration to show, with the reason.
    #[serde(default)]
    pub not_started: Vec<NotStartedSource>,
    /// How many of the board's sources are running, the scratch source apart.
    #[serde(default)]
    pub sources_started: u32,
    /// Why the board's line or its checkouts could not be read at the last look, so its sources are not known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sources_unread: Option<String>,
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
    /// The problems in the quality parts, claim blocks and Pre-registration of the members (`Task.part_problems`), one
    /// for each problem entry. Left out when there are none. A member that is two files counts once.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub part_problems: u32,
    #[serde(default)]
    pub steps: Vec<Vec<String>>,
    #[serde(default)]
    pub cycle: Vec<String>,
    /// The `blocked-by` links of the built-in `ready` view that name no item: their tasks are not ready. Left out when
    /// there are none, and always for a view other than `ready`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unknown_blockers: Vec<UnknownTarget>,
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
/// kind: what `collide task <id> <op>` maps its words to. `link_types` are the link types its source declares, so a
/// reader can name the other end of a link (`inverse`). `views` are the names of the board's views that hold the task,
/// as `task.list` evaluates them. All four are additive.
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub link_types: Vec<LinkTypeDecl>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub views: Vec<String>,
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
    /// A person-only write: nothing was written, and a person answers in the app. No `result`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confirmation: Option<WriteQueued>,
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
    /// A view core holds, which no expression can say (`ready`).
    Builtin {},
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
    /// The problems in the parts of the members, as `ViewReport.part_problems` counts them. Left out when none.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub problem_count: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

fn is_zero(count: &u32) -> bool {
    *count == 0
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
    /// The board's line has a Collide folder, so a personal view can be shared: saved to `.collide/views/` for the
    /// person to commit (`task.save_view`, target `repo`).
    #[serde(default)]
    pub shared: bool,
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

/// `task.create` params (`collide task create`): the planning source's `create`, with the new item's title and the
/// epic and kind it is made in. `epic` is the id the source's `create` takes. `seed` makes the id's suffix repeatable,
/// `root` names the private root, and `checkout` names the checkout to write in (the board's main one when left out).
/// `args` holds the other arguments the source's `create` declares, each as the text typed: the daemon checks every
/// name against the declaration and reads the text as the type it declares. The answer is a [`TaskApplyResult`]: the
/// write is queued when its checkout is busy, as any write is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TaskCreateParams {
    pub board: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub epic: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub args: BTreeMap<String, Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkout: Option<String>,
}

/// `task.reid` params (`collide task reid`): gives the item `id` a new suffix, in the planning source, and rewrites the
/// links that name the old id. `id` is a key, or an id of the planning source. Nothing is committed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TaskReidParams {
    pub board: String,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkout: Option<String>,
}

/// `task.reid` result. `from` is the key the item had. `result` is the provider's answer, with the renamed item as its
/// `task`; it is absent when the write was queued (`queued` says why). `rewritten` are the keys of the other items
/// whose links, or whose `resolved_by`, were rewritten to the new id.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TaskReidResult {
    pub from: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<ApplyResult>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub queued: Option<WriteQueued>,
    #[serde(default)]
    pub rewritten: Vec<String>,
}

/// `task.sections` params (`collide sections`). The result is [`crate::section::Sections`], as the CLI prints it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TaskSectionsParams {
    pub board: String,
}

/// `task.changes` params: the task's key, as `task.get` gives it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TaskChangesParams {
    pub board: String,
    pub task: String,
}

/// Where a run's changes were made.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ChangesPlace {
    /// A worktree the run has to itself.
    Worktree,
    /// The person's own checkout, where the person's edits are in the working tree too.
    Checkout,
}

/// Where a list of changes comes from, and how far it reaches, in words a window shows as they are (`place` and `how`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ChangeWords {
    pub place: String,
    pub how: String,
}

/// One run's changes for `task.changes`: the files from its first phase's start head to its last phase's end head, or
/// to the working tree while a phase is active. A run with no heads has no files and says so in `note`. `files` and
/// `collapsed` are capped as `run.diff`'s are, and `more` counts what they left out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RunChanges {
    pub run: String,
    pub state: RunState,
    /// The folder the run works in, while it is there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
    pub place: ChangesPlace,
    pub words: ChangeWords,
    #[serde(default)]
    pub files: Vec<DiffFile>,
    #[serde(default)]
    pub collapsed: Vec<CollapsedFolder>,
    #[serde(default)]
    pub more: u32,
    /// Why `files` is empty, for a run that has no changes to show: `no changes recorded`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// One commit that names the task, outside every run's span. `date_ms` is the commit's time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CommitChanges {
    pub id: String,
    pub subject: String,
    pub date_ms: u64,
    #[serde(default)]
    pub files: Vec<DiffFile>,
    #[serde(default)]
    pub collapsed: Vec<CollapsedFolder>,
    #[serde(default)]
    pub more: u32,
}

/// `task.changes` result: the task's runs, newest first, then the commits on the board's line that name its id, newest
/// first and at most 200. A commit inside a run's span is under the run. `commits_words` says where the commits are
/// and how they were found. A task of the `scratch` source has no commits: its short ids name nothing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TaskChangesResult {
    pub runs: Vec<RunChanges>,
    pub commits: Vec<CommitChanges>,
    pub commits_words: ChangeWords,
}

/// `task.diff` params: one file of a run's changes (`run`) or of a commit's (`commit`); exactly one is given. With
/// `whole` the result is the whole file after the changes, not the patch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TaskDiffParams {
    pub board: String,
    pub task: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    pub path: String,
    #[serde(default)]
    pub whole: bool,
}

/// One line of a file after the changes: its number, from 1, its text, and whether the changes added or changed it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NumberedLine {
    pub n: u32,
    pub text: String,
    pub changed: bool,
}

/// A whole file after the changes. `binary` says it is not text and has no `lines`. `truncated` says `lines` stops at
/// the patch cap, and `bytes` is the file's full size.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WholeFile {
    pub lines: Vec<NumberedLine>,
    pub bytes: u64,
    pub truncated: bool,
    pub binary: bool,
}

/// `task.diff` result: `patch` for a patch, `whole` for the whole file. `note` says what was cut or left out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TaskDiffResult {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patch: Option<FilePatch>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub whole: Option<WholeFile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}
