//! `.collide/board.yaml` and the `.collide` pointer file (0.7, The Collide directory; 0.12; Stage 6c).
//!
//! - [`Board`] is `board.yaml`: the packs in order, the default workflow, the profiles (`profiles.default` and
//!   `profiles.rules`), source configuration, the status mapping and the checkout filter.
//! - [`CollidePointer`] is a `.collide` **file** holding `dir: <path>`, for a repo that keeps its Collide directory
//!   elsewhere.
//!
//! A pack source is the text of [`crate::lock::PackSource`]; a board refuses `dev:` (the mirror of
//! [`crate::lock::UserPacksError::RepoSource`]). A rule's `when:` is CEL kept as text with the length cap of
//! [`MAX_CEL_SOURCE_BYTES`], as a view's `where:` is.
//!
//! The Projects v2 keys (`sources.github.projects`, `status_mapping.github`) are frozen here and built in Stage 6c.
//! The checks are pure: they read the parsed file. Loading packs, resolving names against them, and CEL parsing are
//! Stage 4a's and 4b's.

use crate::item::Category;
use crate::lock::{PackSource, SourceError};
use crate::view::{MAX_CEL_SOURCE_BYTES, is_namespaced_name};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

/// One pack of the board's list. A line may reorder or replace the list (0.12).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BoardPack {
    /// `<scheme>:<rest>`: a `repo:` or `git:` source. `dev:` is refused.
    #[schemars(regex(pattern = "^[a-z][a-z0-9+.-]*:.+$"))]
    pub source: String,
}

/// `profiles.rules[]`: `{ when: <CEL over the task>, profile: <name> }`. The first rule that matches picks the
/// run's profile (0.7, Profiles).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProfileRule {
    /// CEL over one task, as text. Its length is capped at [`MAX_CEL_SOURCE_BYTES`].
    pub when: String,
    pub profile: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BoardProfiles {
    /// The profile a run gets when nothing earlier picks one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<ProfileRule>,
}

impl BoardProfiles {
    fn is_empty(&self) -> bool {
        *self == BoardProfiles::default()
    }
}

/// A GitHub Projects v2 project: its owner (a user or an organization) and its number.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GithubProject {
    pub owner: String,
    #[schemars(range(min = 1))]
    pub number: u64,
}

/// The GitHub source: repos for Issues, projects for Projects v2. The account is per user, in the user's
/// settings, never here (0.7).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GithubSource {
    /// Each `owner/name`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub repos: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub projects: Vec<GithubProject>,
}

/// `sources:` (0.10: configuration lives in `board.yaml`).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BoardSources {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub github: Option<GithubSource>,
}

impl BoardSources {
    fn is_empty(&self) -> bool {
        *self == BoardSources::default()
    }
}

/// The GitHub Projects v2 Status field mapped to the categories (0.10, Status categories; Stage 6c).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GithubStatusMapping {
    /// The Projects field that holds the status. When it is absent the field is named `Status`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status_field: Option<String>,
    /// Each category and the Projects statuses that map to it. The first status of a category is the one a
    /// category write sets (`setStatus` with `done` sets the first status listed under `done`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub categories: BTreeMap<Category, Vec<String>>,
}

/// `status_mapping:`, by source.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StatusMapping {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub github: Option<GithubStatusMapping>,
}

impl StatusMapping {
    fn is_empty(&self) -> bool {
        *self == StatusMapping::default()
    }
}

/// `checkout_filter:` (0.10, Ref-scoped sources): which `git worktree list` entries are not checkouts of the
/// board.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CheckoutFilter {
    /// Globs, such as `scratch/*`. What a glob is matched against is the checkout code's (Stage 4).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exclude: Vec<String>,
}

impl CheckoutFilter {
    fn is_empty(&self) -> bool {
        self.exclude.is_empty()
    }
}

/// `.collide/board.yaml`. Every key is optional: `collide init` writes a short file.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Board {
    /// The packs, in order. A pack in the repo is `repo:`; `git:` ones are pinned in `.collide/packs.lock`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub packs: Vec<BoardPack>,
    /// The default workflow, as `pack/workflow`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workflow: Option<String>,
    #[serde(default, skip_serializing_if = "BoardProfiles::is_empty")]
    pub profiles: BoardProfiles,
    #[serde(default, skip_serializing_if = "BoardSources::is_empty")]
    pub sources: BoardSources,
    #[serde(default, skip_serializing_if = "StatusMapping::is_empty")]
    pub status_mapping: StatusMapping,
    #[serde(default, skip_serializing_if = "CheckoutFilter::is_empty")]
    pub checkout_filter: CheckoutFilter,
}

/// A top-level key of `board.yaml`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BoardKey {
    Packs,
    Workflow,
    Profiles,
    Sources,
    StatusMapping,
    CheckoutFilter,
}

impl BoardKey {
    pub fn as_str(self) -> &'static str {
        match self {
            BoardKey::Packs => "packs",
            BoardKey::Workflow => "workflow",
            BoardKey::Profiles => "profiles",
            BoardKey::Sources => "sources",
            BoardKey::StatusMapping => "status_mapping",
            BoardKey::CheckoutFilter => "checkout_filter",
        }
    }
}

impl fmt::Display for BoardKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The keys a line may override (0.12, Per line): its `packs` list, the default workflow and the profiles.
/// Everything else in `board.yaml` is board-level and comes from the default line.
pub const LINE_KEYS: [BoardKey; 3] = [BoardKey::Packs, BoardKey::Workflow, BoardKey::Profiles];

impl Board {
    /// The top-level keys this file sets, in the order of [`BoardKey`]. A key that holds nothing is not set.
    pub fn keys_set(&self) -> Vec<BoardKey> {
        let mut keys = Vec::new();
        if !self.packs.is_empty() {
            keys.push(BoardKey::Packs);
        }
        if self.workflow.is_some() {
            keys.push(BoardKey::Workflow);
        }
        if !self.profiles.is_empty() {
            keys.push(BoardKey::Profiles);
        }
        if !self.sources.is_empty() {
            keys.push(BoardKey::Sources);
        }
        if !self.status_mapping.is_empty() {
            keys.push(BoardKey::StatusMapping);
        }
        if !self.checkout_filter.is_empty() {
            keys.push(BoardKey::CheckoutFilter);
        }
        keys
    }
}

/// The board-level keys that a non-default line's `board.yaml` sets. The board takes these from the default line
/// (0.12), so the line's value is not used; the caller shows a notice that names each key.
pub fn line_ignored_keys(board: &Board) -> Vec<BoardKey> {
    board
        .keys_set()
        .into_iter()
        .filter(|key| !LINE_KEYS.contains(key))
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoardError {
    /// A `packs` source that does not parse.
    BadSource { index: usize, error: SourceError },
    /// A `dev:` source: a working folder is per user and never in a committed file (0.7, Pack sources).
    DevSource { index: usize },
    DuplicateSource {
        source: String,
        first: usize,
        second: usize,
    },
    /// A `workflow:` that is not written `pack/workflow`.
    BadWorkflowName { name: String },
    /// `profiles.default` or a rule's `profile:` that is empty.
    EmptyProfileName { key: &'static str },
    /// A rule's `when:` that is empty or only white space.
    EmptyWhen { index: usize },
    /// A rule's `when:` over [`MAX_CEL_SOURCE_BYTES`].
    WhenTooLong { index: usize, bytes: usize },
    /// A `sources.github.repos` entry that is not `owner/name`.
    BadRepo { name: String },
    /// A project with an empty owner or a number under 1.
    BadProject { index: usize },
    /// A `status_mapping.github.status_field` that is empty.
    EmptyStatusField,
    /// A category listed with no statuses, or a status that is empty.
    EmptyStatus { category: Category },
    /// One status listed under two categories.
    StatusInTwoCategories {
        status: String,
        first: Category,
        second: Category,
    },
    /// An empty `checkout_filter.exclude` glob.
    EmptyGlob { index: usize },
}

impl fmt::Display for BoardError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BoardError::BadSource { index, error } => write!(f, "`packs` entry {index}: {error}"),
            BoardError::DevSource { index } => write!(
                f,
                "`packs` entry {index}: `board.yaml` refuses `dev:`; a folder being developed is per user"
            ),
            BoardError::DuplicateSource {
                source,
                first,
                second,
            } => write!(
                f,
                "`packs` entries {first} and {second} both list `{source}`"
            ),
            BoardError::BadWorkflowName { name } => {
                write!(f, "`workflow: {name}` is not written `pack/workflow`")
            }
            BoardError::EmptyProfileName { key } => write!(f, "`{key}` is empty"),
            BoardError::EmptyWhen { index } => {
                write!(f, "`profiles.rules` entry {index}: `when` is empty")
            }
            BoardError::WhenTooLong { index, bytes } => write!(
                f,
                "`profiles.rules` entry {index}: `when` is {bytes} bytes; a CEL source is capped at {MAX_CEL_SOURCE_BYTES}"
            ),
            BoardError::BadRepo { name } => {
                write!(
                    f,
                    "`sources.github.repos`: `{name}` is not written `owner/name`"
                )
            }
            BoardError::BadProject { index } => write!(
                f,
                "`sources.github.projects` entry {index} needs an owner and a number of 1 or more"
            ),
            BoardError::EmptyStatusField => {
                write!(f, "`status_mapping.github.status_field` is empty")
            }
            BoardError::EmptyStatus { category } => write!(
                f,
                "`status_mapping.github.categories.{}` lists no status, or an empty one",
                category_name(*category)
            ),
            BoardError::StatusInTwoCategories {
                status,
                first,
                second,
            } => write!(
                f,
                "`status_mapping.github`: status `{status}` is under `{}` and `{}`",
                category_name(*first),
                category_name(*second)
            ),
            BoardError::EmptyGlob { index } => {
                write!(f, "`checkout_filter.exclude` entry {index} is empty")
            }
        }
    }
}

impl std::error::Error for BoardError {}

fn category_name(category: Category) -> &'static str {
    match category {
        Category::Backlog => "backlog",
        Category::Ready => "ready",
        Category::Active => "active",
        Category::Review => "review",
        Category::Done => "done",
        Category::Dropped => "dropped",
    }
}

fn is_owner_name(text: &str) -> bool {
    !text.is_empty() && !text.contains('/') && !text.chars().any(char::is_whitespace)
}

/// Checks a board on its own: every source parses and none is `dev:` or listed twice, names, rule sources and
/// their length, the GitHub names and the status mapping. Returns every refusal, in the order the keys are written.
/// An unknown source scheme passes (a package manager's, later). Whether a pack, workflow or profile exists is the
/// load's (Stage 4a), and CEL syntax is Stage 4b's.
pub fn check_board(board: &Board) -> Result<(), Vec<BoardError>> {
    let mut errors = Vec::new();
    let mut seen: Vec<(&str, usize)> = Vec::new();
    for (index, pack) in board.packs.iter().enumerate() {
        match PackSource::parse(&pack.source) {
            Err(error) => errors.push(BoardError::BadSource { index, error }),
            Ok(PackSource::Dev { .. }) => errors.push(BoardError::DevSource { index }),
            Ok(_) => {}
        }
        match seen.iter().find(|(source, _)| *source == pack.source) {
            Some((_, first)) => errors.push(BoardError::DuplicateSource {
                source: pack.source.clone(),
                first: *first,
                second: index,
            }),
            None => seen.push((&pack.source, index)),
        }
    }
    if let Some(workflow) = &board.workflow
        && !is_namespaced_name(workflow)
    {
        errors.push(BoardError::BadWorkflowName {
            name: workflow.clone(),
        });
    }
    if let Some(default) = &board.profiles.default
        && default.trim().is_empty()
    {
        errors.push(BoardError::EmptyProfileName {
            key: "profiles.default",
        });
    }
    for (index, rule) in board.profiles.rules.iter().enumerate() {
        if rule.when.trim().is_empty() {
            errors.push(BoardError::EmptyWhen { index });
        } else if rule.when.len() > MAX_CEL_SOURCE_BYTES {
            errors.push(BoardError::WhenTooLong {
                index,
                bytes: rule.when.len(),
            });
        }
        if rule.profile.trim().is_empty() {
            errors.push(BoardError::EmptyProfileName {
                key: "profiles.rules.profile",
            });
        }
    }
    if let Some(github) = &board.sources.github {
        for name in &github.repos {
            let ok = name
                .split_once('/')
                .is_some_and(|(owner, repo)| is_owner_name(owner) && is_owner_name(repo));
            if !ok {
                errors.push(BoardError::BadRepo { name: name.clone() });
            }
        }
        for (index, project) in github.projects.iter().enumerate() {
            if !is_owner_name(&project.owner) || project.number < 1 {
                errors.push(BoardError::BadProject { index });
            }
        }
    }
    if let Some(github) = &board.status_mapping.github {
        if github
            .status_field
            .as_deref()
            .is_some_and(|field| field.trim().is_empty())
        {
            errors.push(BoardError::EmptyStatusField);
        }
        let mut owners: BTreeMap<&str, Category> = BTreeMap::new();
        for (category, statuses) in &github.categories {
            if statuses.is_empty() || statuses.iter().any(|status| status.trim().is_empty()) {
                errors.push(BoardError::EmptyStatus {
                    category: *category,
                });
            }
            for status in statuses.iter().filter(|status| !status.trim().is_empty()) {
                match owners.get(status.as_str()) {
                    Some(first) if first != category => {
                        errors.push(BoardError::StatusInTwoCategories {
                            status: status.clone(),
                            first: *first,
                            second: *category,
                        })
                    }
                    _ => {
                        owners.insert(status, *category);
                    }
                }
            }
        }
    }
    for (index, glob) in board.checkout_filter.exclude.iter().enumerate() {
        if glob.trim().is_empty() {
            errors.push(BoardError::EmptyGlob { index });
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// The `.collide` pointer file: a file in place of the directory, as git's `.git` file points a worktree at its git
/// directory (0.7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CollidePointer {
    /// The Collide directory, relative to the repo root.
    pub dir: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PointerError {
    EmptyDir,
    /// A `dir` that is not relative to the repo root.
    NotRelative {
        dir: String,
    },
    /// A `dir` with a `..` part, which would leave the repo.
    LeavesRepo {
        dir: String,
    },
}

impl fmt::Display for PointerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PointerError::EmptyDir => write!(f, "`dir` is empty"),
            PointerError::NotRelative { dir } => {
                write!(f, "`dir: {dir}` is not relative to the repo root")
            }
            PointerError::LeavesRepo { dir } => {
                write!(f, "`dir: {dir}` has a `..` part and would leave the repo")
            }
        }
    }
}

impl std::error::Error for PointerError {}

/// Checks the pointer's `dir`: not empty, relative to the repo root, and with no `..` part. Whether the folder
/// exists is the load's.
pub fn check_pointer(pointer: &CollidePointer) -> Result<(), PointerError> {
    let dir = pointer.dir.as_str();
    if dir.trim().is_empty() {
        return Err(PointerError::EmptyDir);
    }
    if dir.starts_with('/') || dir.starts_with('\\') || dir.starts_with('~') || dir.contains(':') {
        return Err(PointerError::NotRelative { dir: dir.into() });
    }
    if dir.split(['/', '\\']).any(|part| part == "..") {
        return Err(PointerError::LeavesRepo { dir: dir.into() });
    }
    Ok(())
}
