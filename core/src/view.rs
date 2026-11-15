//! A view (0.10, Views): a named subset of the corpus, written as a YAML file in `.collide/views/`, in the user's
//! board settings, or under a pack's `views:`. Membership is `(where ∪ include ∪ closure) − exclude`.
//!
//! `where:` is CEL held as text. This module caps its length (0.9, a 1 KB source cap) and nothing else: parsing,
//! the static cost estimate and evaluation are the CEL stage's (`ts-0044-xq`, Stage 4b). [`check_view`] is the
//! pure check a view load runs, with every refusal in one pass.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt;

/// The longest CEL source a view may hold, in bytes (0.9: "a 1 KB source cap"). A source of exactly this many
/// bytes is accepted.
pub const MAX_CEL_SOURCE_BYTES: usize = 1024;

/// `closure:` follows one kind of edge, from a goal task: the prototype board's run order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Follow {
    DependsOn,
}

/// `closure: { goal: plan:T-01-042, follow: depends_on }`: the open tasks the goal depends on, and the goal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Closure {
    pub goal: String,
    pub follow: Follow,
}

/// How the Field lays a view out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum LayoutKind {
    Board,
    RunOrder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SortDir {
    Asc,
    Desc,
}

/// `layout.sort` (0.11): a sort key and a direction. `by` is a field the surface can sort on (the Field's `id`,
/// `title`, `epic`, `labels` and `status`; Tracks adds more), which the surface checks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Sort {
    pub by: String,
    pub dir: SortDir,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Layout {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<LayoutKind>,
    #[serde(rename = "groupBy", default, skip_serializing_if = "Option::is_none")]
    pub group_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sort: Option<Sort>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct View {
    pub name: String,
    /// CEL over one task, as text (0.10). Its length is capped at [`MAX_CEL_SOURCE_BYTES`].
    #[serde(rename = "where", default, skip_serializing_if = "Option::is_none")]
    pub where_: Option<String>,
    /// Task keys such as `plan:T-01-042`, hand-picked into the view.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub include: Vec<String>,
    /// Task keys taken out of the view, after the other three are joined.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exclude: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub closure: Option<Closure>,
    /// The default workflow for runs launched from this view, as `pack/workflow`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workflow: Option<String>,
    /// The profile runs launched from this view get (0.7, Profiles).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout: Option<Layout>,
    /// Feeds a Queue lane (0.10, Planning sources).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub queue: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViewError {
    EmptyName,
    /// `where:` is empty or only white space.
    EmptyWhere,
    /// `where:` is longer than [`MAX_CEL_SOURCE_BYTES`].
    WhereTooLong {
        bytes: usize,
    },
    /// An empty entry in `include:` or `exclude:`. `key` is the list's name.
    EmptyTaskKey {
        key: &'static str,
        index: usize,
    },
    EmptyClosureGoal,
    /// A `workflow:` or `profile:` that is empty. A workflow is named `pack/workflow`.
    BadName {
        key: &'static str,
        name: String,
    },
    /// A `layout.sort.by` or `layout.groupBy` that is empty.
    EmptyLayoutField {
        key: &'static str,
    },
}

impl fmt::Display for ViewError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ViewError::EmptyName => write!(f, "`name:` is empty"),
            ViewError::EmptyWhere => write!(f, "`where:` is empty"),
            ViewError::WhereTooLong { bytes } => write!(
                f,
                "`where:` is {bytes} bytes; a CEL source is capped at {MAX_CEL_SOURCE_BYTES}"
            ),
            ViewError::EmptyTaskKey { key, index } => {
                write!(f, "`{key}:` entry {index} is empty")
            }
            ViewError::EmptyClosureGoal => write!(f, "`closure.goal:` is empty"),
            ViewError::BadName { key, name } => {
                write!(f, "`{key}: {name}` is not a usable name")
            }
            ViewError::EmptyLayoutField { key } => write!(f, "`{key}` is empty"),
        }
    }
}

impl std::error::Error for ViewError {}

/// A name written `pack/name` (0.4): two non-empty parts and no white space.
pub fn is_namespaced_name(name: &str) -> bool {
    match name.split_once('/') {
        Some((pack, rest)) => {
            !pack.is_empty()
                && !rest.is_empty()
                && !rest.contains('/')
                && !name.chars().any(char::is_whitespace)
        }
        None => false,
    }
}

/// Checks a view: its names, task keys and the length of `where:`. Returns every refusal, in the order the keys
/// are written. CEL syntax and cost are not checked here.
pub fn check_view(view: &View) -> Result<(), Vec<ViewError>> {
    let mut errors = Vec::new();
    if view.name.trim().is_empty() {
        errors.push(ViewError::EmptyName);
    }
    if let Some(source) = &view.where_ {
        if source.trim().is_empty() {
            errors.push(ViewError::EmptyWhere);
        } else if source.len() > MAX_CEL_SOURCE_BYTES {
            errors.push(ViewError::WhereTooLong {
                bytes: source.len(),
            });
        }
    }
    for (key, keys) in [("include", &view.include), ("exclude", &view.exclude)] {
        for (index, task_key) in keys.iter().enumerate() {
            if task_key.trim().is_empty() {
                errors.push(ViewError::EmptyTaskKey { key, index });
            }
        }
    }
    if let Some(closure) = &view.closure
        && closure.goal.trim().is_empty()
    {
        errors.push(ViewError::EmptyClosureGoal);
    }
    if let Some(workflow) = &view.workflow
        && !is_namespaced_name(workflow)
    {
        errors.push(ViewError::BadName {
            key: "workflow",
            name: workflow.clone(),
        });
    }
    if let Some(profile) = &view.profile
        && profile.trim().is_empty()
    {
        errors.push(ViewError::BadName {
            key: "profile",
            name: profile.clone(),
        });
    }
    if let Some(layout) = &view.layout {
        if let Some(group_by) = &layout.group_by
            && group_by.trim().is_empty()
        {
            errors.push(ViewError::EmptyLayoutField {
                key: "layout.groupBy",
            });
        }
        if let Some(sort) = &layout.sort
            && sort.by.trim().is_empty()
        {
            errors.push(ViewError::EmptyLayoutField {
                key: "layout.sort.by",
            });
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}
