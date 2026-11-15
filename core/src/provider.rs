//! The provider protocol v0 (0.10, Protocol): the messages between core and a task source, which is a process that
//! speaks JSON-RPC 2.0 over stdio.
//!
//! **Framing.** Newline-delimited JSON: each message is one JSON value on one line, ended by `\n`. JSON escapes the
//! newlines inside strings, so a line never splits a message. It is the framing the observer protocol already uses,
//! and a provider in any language reads it with no length parser. Stdout carries messages only; a provider logs
//! to stderr.
//!
//! **Envelope.** These types are the `params` of each request, the `result` of each response and the `params` of
//! each notification. The JSON-RPC envelope (`jsonrpc`, `id`, `method`, `error`) is the same for every method and
//! is the transport's, so it is not typed here.
//!
//! | method | params | result |
//! |---|---|---|
//! | `initialize` | [`InitializeParams`] | [`InitializeResult`] |
//! | `list` | [`ListParams`] | [`ListResult`] |
//! | `get` | [`GetParams`] | [`GetResult`] |
//! | `watch` | [`WatchParams`] | [`WatchResult`] |
//! | `apply` | [`ApplyParams`] | [`ApplyResult`] |
//! | `preview` (if declared) | [`ApplyParams`] | [`PreviewResult`] |
//!
//! | notification (from `watch`) | params |
//! |---|---|
//! | `task.collision` | [`TaskCollision`] |
//! | `source.stale` | [`SourceStale`] |
//! | `task.changed` | [`TaskChanged`] |
//! | `task.renamed` | [`Rename`](crate::change::Rename) |
//! | `task.removed` | [`TaskRemoved`] |
//! | `task.invalid` | [`TaskInvalid`] |

use crate::change::{Effect, FieldDiff, ItemState};
use crate::item::{Category, Checklist, KindDecl, Link, LinkTypeDecl, OpDecl, StatusDecl};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fmt;

/// Op names that a provider may not declare. `move` is Share's (0.10); plan's `move --epic` is declared `rehome`.
const RESERVED_OPS: &[&str] = &["move"];

/// `initialize` params: the pack's settings (`config`, 0.7), already checked against the pack's own schema, and
/// `root`, the main checkout's absolute path (owner, 2026-11-14). A provider never infers the root from its working
/// directory.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct InitializeParams {
    #[serde(default)]
    pub config: BTreeMap<String, Value>,
    pub root: String,
}

/// Whether one task has one state (`global`) or one state per checkout (`ref`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    Global,
    Ref,
}

/// A field that templates may use (0.5): its name and the regex every value must match.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TemplateFieldDecl {
    pub name: String,
    pub pattern: String,
}

/// A source field. `type` is the provider's own name for the type, such as `string` or `checklist`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FieldDecl {
    pub id: String,
    #[serde(rename = "type")]
    pub field_type: String,
    #[serde(default)]
    pub filterable: bool,
}

/// What the provider can do beyond `initialize`, `list`, `get` and `apply`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Capabilities {
    #[serde(default)]
    pub watch: bool,
    #[serde(default)]
    pub create: bool,
    #[serde(default)]
    pub collections: bool,
    #[serde(default)]
    pub preview: bool,
}

/// `initialize` result. `kinds` and `links` may be left out: a provider with one kind and no links validates
/// without them. `categoryStatus` gives, for each category, the status that a category write produces (plan:
/// `active` is `doing`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InitializeResult {
    pub scope: Scope,
    pub id_pattern: String,
    pub short_id_pattern: String,
    #[serde(default)]
    pub template_fields: Vec<TemplateFieldDecl>,
    pub statuses: Vec<StatusDecl>,
    #[serde(default)]
    pub category_status: BTreeMap<Category, String>,
    #[serde(default)]
    pub fields: Vec<FieldDecl>,
    #[serde(default)]
    pub ops: Vec<OpDecl>,
    pub capabilities: Capabilities,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub kinds: Vec<KindDecl>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub links: Vec<LinkTypeDecl>,
}

/// One task as a provider reports it. `key` is `source:shortId`; `id` is the long form that `get` and `apply`
/// take. `checkout` and `path` are set by a ref-scoped source. `kind` may be left out when the provider has one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Task {
    pub key: String,
    pub id: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    pub state: ItemState,
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkout: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default)]
    pub links: Vec<Link>,
    #[serde(default)]
    pub owners: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checklist: Option<Checklist>,
    #[serde(default)]
    pub labels: Vec<String>,
    #[serde(default)]
    pub collections: Vec<String>,
    #[serde(default)]
    pub template_fields: BTreeMap<String, String>,
    #[serde(default)]
    pub fields: BTreeMap<String, Value>,
}

/// One ordered group of a collection, such as a slice of an epic: the keys of its tasks, in order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CollectionGroup {
    pub name: String,
    pub tasks: Vec<String>,
}

/// A grouping the source holds itself, such as an epic or a milestone (0.10). Each task names the collections it
/// belongs to in `Task.collections`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Collection {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub groups: Vec<CollectionGroup>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ListParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkout: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
}

/// `list` result. `nextCursor` is set when there is another page. `invalid` holds each item the source could not
/// read as a task, in the shape of `task.invalid` (owner, 2026-11-14).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ListResult {
    pub tasks: Vec<Task>,
    #[serde(default)]
    pub collections: Vec<Collection>,
    #[serde(default)]
    pub invalid: Vec<TaskInvalid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GetParams {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkout: Option<String>,
}

/// `get` result. `task` is absent when no item has the id: a missing id is an answer, not an error (owner,
/// 2026-11-14).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GetResult {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<Task>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WatchParams {
    pub checkouts: Vec<String>,
}

/// `watch` is answered with an empty object, and the notifications follow.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WatchResult {}

/// `apply` params, and `preview`'s. `id` is left out for an op that makes a task, and `version` is the version the
/// caller last read: the source answers `conflict` when it has moved on. Only `args` is op-specific; its schema is
/// the op's `args` in `initialize`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ApplyParams {
    pub op: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default)]
    pub args: BTreeMap<String, Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkout: Option<String>,
}

/// `apply` result. `ok` carries the new task and its version, the `files` the write touched (what `record`
/// commits) and `items[]`: every item the op changed, with its version, from and to. `conflict` carries the
/// current task. `refused` carries the source's own message, such as plan's "illegal transition".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "outcome", rename_all = "lowercase", deny_unknown_fields)]
pub enum ApplyResult {
    Ok {
        task: Task,
        version: String,
        files: Vec<String>,
        items: Vec<Effect>,
    },
    Conflict {
        current: Task,
    },
    Refused {
        message: String,
    },
}

/// `preview` result: the same three outcomes as [`ApplyResult`], with nothing written. `ok` carries the `items[]`
/// and `files` that `apply` would return. `conflict` and `refused` are `apply`'s, and a test pins them in step.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "outcome", rename_all = "lowercase", deny_unknown_fields)]
pub enum PreviewResult {
    Ok {
        items: Vec<Effect>,
        files: Vec<String>,
    },
    Conflict {
        current: Box<Task>,
    },
    Refused {
        message: String,
    },
}

/// Where one copy of a task is.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Location {
    pub checkout: String,
    pub path: String,
}

/// `task.collision`: one short id at different paths in several checkouts. It is a collision, never a divergence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TaskCollision {
    pub short_id: String,
    pub locations: Vec<Location>,
}

/// `source.stale`: the source could not refresh, and what core holds may be out of date.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SourceStale {
    pub reason: String,
}

/// What made a `task.changed` notification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ChangeCause {
    Head,
    Worktree,
    Remote,
    Journal,
    Startup,
}

/// `task.changed`. `from` is null at create and `to` is null at delete, as in `change`. `actor` is the source's
/// own, when it knows it (GitHub events, the beads journal); `automated` is GitHub's `wasAutomated`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TaskChanged {
    pub key: String,
    pub version: String,
    pub from: Option<ItemState>,
    pub to: Option<ItemState>,
    #[serde(default)]
    pub diff: BTreeMap<String, FieldDiff>,
    pub cause: ChangeCause,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub automated: Option<bool>,
    /// The checkout whose copy changed, for a ref-scoped source; absent for a global source (such as GitHub).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkout: Option<String>,
}

/// Why a task is gone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum RemovedCause {
    Deleted,
    Transferred,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TaskRemoved {
    pub key: String,
    pub cause: RemovedCause,
    /// The checkout whose copy was removed, for a ref-scoped source; absent for a global source (such as GitHub).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkout: Option<String>,
}

/// `task.invalid`: something the source could not read as a task. Core turns it into a Queue item. `checkout` is
/// null for a source that is not ref-scoped.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TaskInvalid {
    pub path: String,
    pub checkout: Option<String>,
    pub reason: String,
}

/// Why an `initialize` result is refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderError {
    /// An op named `move`, in the provider's `ops` (`kind` is `None`) or in a kind's.
    ReservedOp { op: String, kind: Option<String> },
}

impl fmt::Display for ProviderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProviderError::ReservedOp { op, kind: None } => {
                write!(
                    f,
                    "op `{op}` is reserved for Share; declare it under another name"
                )
            }
            ProviderError::ReservedOp {
                op,
                kind: Some(kind),
            } => write!(
                f,
                "op `{op}` of kind `{kind}` is reserved for Share; declare it under another name"
            ),
        }
    }
}

impl std::error::Error for ProviderError {}

/// The refusals serde cannot express in an `initialize` result: every op, the provider's and each kind's, must
/// avoid the reserved names. Returns all of them.
pub fn check_initialize(result: &InitializeResult) -> Result<(), Vec<ProviderError>> {
    let reserved = |op: &OpDecl, kind: Option<&str>| {
        RESERVED_OPS
            .contains(&op.name.as_str())
            .then(|| ProviderError::ReservedOp {
                op: op.name.clone(),
                kind: kind.map(str::to_string),
            })
    };
    let mut errors: Vec<ProviderError> = result
        .ops
        .iter()
        .filter_map(|op| reserved(op, None))
        .collect();
    for kind in &result.kinds {
        errors.extend(
            kind.ops
                .iter()
                .filter_map(|op| reserved(op, Some(&kind.name))),
        );
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}
