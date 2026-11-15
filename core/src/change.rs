//! What the task points add to a hook's input (0.5): `change` and `latestRun`. Core computes `diff` from the
//! cache before and the `apply` result after (the engine items fill that in); this module is the shape.

use crate::item::Category;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// The route of a write.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Via {
    Ui,
    Workflow,
    Share,
    Integrate,
    Run,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ActorKind {
    Person,
    Workflow,
    Agent,
}

/// Who made the write (0.5). `run` and `phase` are reserved now so that writes a run makes need no later schema
/// change. They are not tied to a kind: a workflow op inside a run may carry `run`, and an agent outside a run,
/// such as one whose actor is read from the process tree, may have neither.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Actor {
    pub kind: ActorKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phase: Option<String>,
}

/// An item's status and its category.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ItemState {
    pub status: String,
    pub category: Category,
}

/// One changed field: `{from, to}`, or `{added, removed}` for a set. `from` is null at create and `to` is null
/// at delete.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged, deny_unknown_fields)]
pub enum FieldDiff {
    Changed {
        from: Value,
        to: Value,
    },
    Set {
        added: Vec<Value>,
        removed: Vec<Value>,
    },
}

/// One call on several items fires per item; `index` counts from 0.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Batch {
    pub id: String,
    pub index: u32,
    pub size: u32,
}

/// Another item that an op changed, from `apply`'s `items[]`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Effect {
    pub key: String,
    pub version: String,
    pub from: Option<ItemState>,
    pub to: Option<ItemState>,
}

/// An id that changed, at Share or `renumber`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Rename {
    pub from: String,
    pub to: String,
}

/// The `change` a task point carries. `op` is null for a write made by a run (`via: run`). `from` is null at
/// create and `to` is null at delete. `diff` is predicted at `before_*` and actual at `after_*`. `effectOf` is the
/// key of the item whose op changed this one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Change {
    pub op: Option<String>,
    #[serde(default)]
    pub args: BTreeMap<String, Value>,
    pub from: Option<ItemState>,
    pub to: Option<ItemState>,
    #[serde(default)]
    pub diff: BTreeMap<String, FieldDiff>,
    pub category_moved: bool,
    pub via: Via,
    pub actor: Actor,
    pub batch: Option<Batch>,
    #[serde(default)]
    pub effects: Vec<Effect>,
    pub effect_of: Option<String>,
    #[serde(default)]
    pub renamed: Vec<Rename>,
    pub checkout: Option<String>,
    #[serde(default)]
    pub files: Vec<String>,
}

/// An item's latest finished run, frozen and read-only. A hook outside a run reads it, for example to find a
/// verify verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LatestRun {
    pub id: String,
    pub workflow: String,
    pub outcome: String,
    pub state: BTreeMap<String, Value>,
}

/// What the five new task points add to the hook input, beside the 0.5 fields that `ts-0012-ua` and `ts-0010-yb`
/// define. `latestRun` is null inside a run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TaskPointPayload {
    pub change: Change,
    pub latest_run: Option<LatestRun>,
}
