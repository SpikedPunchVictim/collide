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

/// What a link type points at: an item, or a feature of a proposal (`pr-0003-km#F2`, revision 14).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum LinkTarget {
    #[default]
    Item,
    Feature,
}

impl LinkTarget {
    fn is_item(&self) -> bool {
        *self == LinkTarget::Item
    }
}

/// A link type a provider holds: its direction, whether it blocks, and what it targets. A declaration with no
/// `target` targets an item, so every declaration written before revision 14 reads as it did. `inverse` is what the
/// other end calls the link ("blocks" for `blocked-by`); a declaration with none reads as it did.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LinkTypeDecl {
    pub name: String,
    pub dir: LinkDir,
    pub blocking: bool,
    #[serde(default, skip_serializing_if = "LinkTarget::is_item")]
    pub target: LinkTarget,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inverse: Option<String>,
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

/// One feature of a proposal, read from the `## Features` part of its body (0.8, 0.10, "A proposal's features").
/// `id` is `F<n>`, `section` is empty for an entry that is unsorted, and `summary` joins an entry's continuation lines.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Feature {
    pub id: String,
    pub title: String,
    pub section: String,
    pub summary: String,
}

/// What a proposal's features and tasks cover, derived and never written (0.10, "A proposal's quality parts").
/// `featuresWithoutTask` are the `F<n>` that no task implements. `tasksWithoutFeature` are the ids of the tasks the
/// proposal includes, not dropped, that implement no feature.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Coverage {
    pub features_without_task: Vec<String>,
    pub tasks_without_feature: Vec<String>,
}

/// A line of the Features part that did not parse, with why (0.8, 0.10). `line` counts from 1 in the item's file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FeatureProblem {
    pub line: u32,
    pub reason: String,
}

/// A line of a proposal's quality part (the Decision log, the Assumptions or the Findings) that is not right, with why
/// (0.8, 0.10). `part` is the part's name, as `Decision`. `line` counts from 1 in the item's file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PartProblem {
    pub part: String,
    pub line: u32,
    pub reason: String,
}

/// What an `Evidence:` entry names: an item, a feature of a proposal as `pr-0003-km#F2`, or a path in the repo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum EvidenceKind {
    Item,
    Feature,
    Path,
}

/// One entry of an `Evidence:` label. `status` is the status of the item it names, when the scan has it. `proposal`
/// is the proposal of a feature.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Evidence {
    pub target: String,
    pub kind: EvidenceKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proposal: Option<String>,
}

/// One entry `D<n>` of a proposal's Decision log (0.10). The labels' values are text as written; `confidence` is
/// `low`, `medium` or `high`. `line` is the entry's first line in the item's file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Decision {
    pub id: String,
    pub title: String,
    pub text: String,
    #[serde(default)]
    pub passed_on: Vec<String>,
    #[serde(default)]
    pub evidence: Vec<Evidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revisit_if: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<String>,
    pub line: u32,
}

/// One entry `A<n>` of an Assumptions part (0.10). A missing or invalid `state` reads `unverified`.
// `state` is `unverified`, `verified`, `contradicted` or `accepted risk`. `spike` is the spike that would settle
// the assumption, and `spike_status` is its status, when it names a spike.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Assumption {
    pub id: String,
    pub title: String,
    pub text: String,
    pub state: String,
    #[serde(default)]
    pub evidence: Vec<Evidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spike: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spike_status: Option<String>,
    pub line: u32,
}

/// One entry `R<n>` of a Findings part (0.10). `open`: neither `accepted` nor `dismissed`.
// `section` is `Red-team` or `Pre-mortem`, and is left out for an entry under neither heading. `severity` is as
// written, so a bad one is still listed (and is a part problem).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Finding {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub section: Option<String>,
    pub severity: String,
    pub title: String,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accepted: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dismissed: Option<String>,
    pub open: bool,
    pub line: u32,
}

/// One claim of a spike's record, read from a fenced `claim` block (0.10, "A `spike`"). `label` is `measured`,
/// `inferred` or `unmeasured`. `verifiable` is the block's `yes` or `no`, and is left out when the block has none. Only
/// a block with no problem is a claim; the prose is never compared.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Claim {
    pub id: String,
    pub label: String,
    pub load_bearing: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verifiable: Option<bool>,
}

/// A spike's Pre-registration part (0.10, "A `spike`"): the text of each of its four labels, as written, and left out
/// when the label is missing. `amendments` counts the `### Amendment` sections below the original text. `hashMatches`
/// is present only when the frontmatter has `preregistered`: whether it is the hash of the original text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Preregistration {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hypothesis: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cheapest_test: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kill_criterion: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget: Option<String>,
    pub amendments: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hash_matches: Option<bool>,
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
/// themselves, `spike`, and `proposal` with its six statuses. The markdown provider declares the same kinds in
/// `initialize`, with its ops and transitions (left empty here, because they are the provider's). `question` comes
/// with the planning pack.
pub fn planning_kinds() -> Vec<KindDecl> {
    use Category::{Active, Backlog, Done, Dropped, Ready, Review};
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
        kind(
            "proposal",
            &[
                ("draft", Active),
                ("proposed", Review),
                ("accepted", Done),
                ("deferred", Backlog),
                ("rejected", Dropped),
                ("superseded", Dropped),
            ],
        ),
    ]
}
