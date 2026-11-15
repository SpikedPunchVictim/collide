//! What core knows about an item (0.8, 0.10): its kind, the status categories, typed links, owners and the
//! checklist field. Core knows kinds, links and a checklist field, never roles. The kinds themselves are data
//! that a provider declares in `initialize`.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;

/// The category every source status maps to, so views and columns work across sources (0.10).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    Backlog,
    Ready,
    Active,
    Review,
    Done,
    Dropped,
}

/// One status of a kind and the category it maps to.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StatusDecl {
    pub name: String,
    pub category: Category,
}

/// A status change the provider allows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TransitionDecl {
    pub from: String,
    pub to: String,
}

/// An op, as a provider declares it. Its effect is the category it moves the item to, if any, so that
/// `task.before_status` can fire before the op runs (0.4). `args` is the JSON Schema of its arguments, which the UI
/// renders as a form (0.10), and `checkouts` names the checkouts it may target (empty: any).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OpDecl {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub moves_to: Option<Category>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub args: Option<Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub checkouts: Vec<String>,
}

/// One item kind, as a provider declares it (0.10, Protocol). `roles` are plain tags that core never reads.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct KindDecl {
    pub name: String,
    pub statuses: Vec<StatusDecl>,
    #[serde(default)]
    pub transitions: Vec<TransitionDecl>,
    #[serde(default)]
    pub ops: Vec<OpDecl>,
    #[serde(default)]
    pub roles: Vec<String>,
}

/// Which end of a link the item that stores it holds: `out` for `blocked-by`, which points at the blocker.
/// Collide derives the other direction (0.10).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum LinkDir {
    Out,
    In,
}

/// A link type a provider holds: its direction and whether it blocks.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LinkTypeDecl {
    pub name: String,
    pub dir: LinkDir,
    pub blocking: bool,
}

/// A typed link on an item (0.8).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Link {
    #[serde(rename = "type")]
    pub link_type: String,
    pub to: String,
    /// Absent means the link type's `blocking` applies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocking: Option<bool>,
}

/// The state of one checklist item. Only a person marks `done` or `n/a`; the gate is a pack's (ts-0012).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum ChecklistState {
    #[serde(rename = "open")]
    Open,
    #[serde(rename = "suggested")]
    Suggested,
    #[serde(rename = "done")]
    Done,
    #[serde(rename = "n/a")]
    NotApplicable,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ChecklistItem {
    pub id: String,
    pub state: ChecklistState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence: Option<String>,
}

/// The checklist field type: the state of one template's items, pinned as `name@n`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Checklist {
    pub template: String,
    pub items: Vec<ChecklistItem>,
}

/// The fields core adds to an item for planning (0.8): `kind`, typed `links`, `owners` (the people or agents who
/// claimed it) and a checklist.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanningFields {
    pub kind: String,
    #[serde(default)]
    pub links: Vec<Link>,
    #[serde(default)]
    pub owners: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checklist: Option<Checklist>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KindError {
    UnknownKind(String),
    UnknownStatus { kind: String, status: String },
}

impl fmt::Display for KindError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KindError::UnknownKind(kind) => write!(f, "unknown kind `{kind}`"),
            KindError::UnknownStatus { kind, status } => {
                write!(f, "status `{status}` is not a status of kind `{kind}`")
            }
        }
    }
}

impl std::error::Error for KindError {}

/// The category of `status` under `kind`, or why it is not one. A status outside its kind's list fails.
pub fn category_of(kinds: &[KindDecl], kind: &str, status: &str) -> Result<Category, KindError> {
    let decl = kinds
        .iter()
        .find(|decl| decl.name == kind)
        .ok_or_else(|| KindError::UnknownKind(kind.to_string()))?;
    decl.statuses
        .iter()
        .find(|decl| decl.name == status)
        .map(|decl| decl.category)
        .ok_or_else(|| KindError::UnknownStatus {
            kind: kind.to_string(),
            status: status.to_string(),
        })
}

fn kind(name: &str, statuses: &[(&str, Category)]) -> KindDecl {
    KindDecl {
        name: name.to_string(),
        statuses: statuses
            .iter()
            .map(|(status, category)| StatusDecl {
                name: status.to_string(),
                category: *category,
            })
            .collect(),
        transitions: Vec::new(),
        ops: Vec::new(),
        roles: Vec::new(),
    }
}

/// The kinds of Collide's own planning items (0.10): `epic` and `task`, whose statuses are five of the categories
/// themselves, and `spike`. The markdown provider declares the same kinds in `initialize`, with its ops and
/// transitions (left empty here, because they are the provider's). `proposal` and `question` come with the
/// planning pack.
pub fn planning_kinds() -> Vec<KindDecl> {
    use Category::{Active, Done, Dropped, Ready, Review};
    let own_categories = [
        ("ready", Ready),
        ("active", Active),
        ("review", Review),
        ("done", Done),
        ("dropped", Dropped),
    ];
    vec![
        kind("epic", &own_categories),
        kind("task", &own_categories),
        kind(
            "spike",
            &[
                ("planned", Ready),
                ("running", Active),
                ("results", Review),
                ("concluded", Done),
                ("dropped", Dropped),
            ],
        ),
    ]
}
