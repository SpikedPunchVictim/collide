//! The `board.*` methods: open a repo as a board, list the boards, share the board's files with the team, and trust
//! a line's packs. A board is the owner's record of a repo (0.8, Board); the other families name it by `id`.

use super::config::{PackFileChange, PackReviewData};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Whether a line's packs and gate code are trusted (0.9, Trust). `hash` is what trust covers now: the packs, the
/// lock, the pointer file, `board.yaml`, `config.yaml`, the views and the tree hashes of `protectedPaths`. It is the
/// hash an `Untrusted` line would be trusted at, so the person is shown what they trust. (The domain record is plain
/// Rust, so the wire form is this type.)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum TrustState {
    /// Nothing is trusted yet, or the hash changed since.
    Untrusted {
        hash: String,
    },
    Trusted {
        hash: String,
    },
}

/// One line of a board (0.12): a ref the user opened. `snapshot` is the commit its packs are read at. `problems` is
/// why the line did not load (a bad `board.yaml`), one message each, in the first-run style. Such a line is untrusted
/// at an empty hash: there is nothing to trust until the problems are fixed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LineSummary {
    pub ref_name: String,
    pub root: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<String>,
    pub trust: TrustState,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub problems: Vec<String>,
}

/// A board: the repo root and its lines, the default line first. Until lines are built (Stage 14) there is one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BoardSummary {
    pub id: String,
    pub root: String,
    pub lines: Vec<LineSummary>,
    /// Why the daemon's last look at the board could not read its line or its checkouts (a folder that is gone, a
    /// line that does not load). Absent when the last look read them. Only `board.list` fills it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sources_unread: Option<String>,
}

/// `board.list` params: none.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BoardListParams {}

/// `board.list` result: the boards the daemon knows, for the board switcher.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BoardListResult {
    pub boards: Vec<BoardSummary>,
}

/// What a first-run check looks at (0.11, First run works with no pack and no tasks).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CheckId {
    ClaudeInstalled,
    ClaudeLoggedIn,
    GitRepo,
    /// `gh` is optional: a GitHub source needs it, nothing else does.
    GhLoggedIn,
}

/// One first-run check. `message` is Collide's own words and names the fix when `ok` is false.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FirstRunCheck {
    pub id: CheckId,
    pub ok: bool,
    /// Whether a failure stops the board from opening usefully.
    pub required: bool,
    pub message: String,
}

/// `board.open` params. `root` is any folder in the repo or its root. `init_git` says yes to `git init` when the
/// folder is not a repo (0.11 offers it).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BoardOpenParams {
    pub root: String,
    #[serde(default)]
    pub init_git: bool,
}

/// `board.open` result. `board` is absent when the folder is not a repo and `init_git` was not given: the
/// `git_repo` check says so. Opening needs no setup: the built-in packs and the default aliases are in place.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BoardOpenResult {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub board: Option<BoardSummary>,
    pub checks: Vec<FirstRunCheck>,
}

/// `board.init` params: Share with team, `collide init` (0.11). `dry_run` lists the files without writing them,
/// which is the Share preview's list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BoardInitParams {
    pub board: String,
    #[serde(default)]
    pub dry_run: bool,
}

/// `board.init` result: every file written, or with `dry_run` every file that would be, relative to the board root.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BoardInitResult {
    pub files: Vec<String>,
    pub written: bool,
}

/// `board.trust` params: trust a line's packs at `hash`. The daemon computes the hash again and refuses when it
/// differs from this one, so a person trusts what they were shown. Trust is stored outside the repo (0.9).
/// `hosts` and `dev_folders` are what the person approved when the line could not load without them: a host a `git:`
/// pack is fetched from for the first time, and a `dev:` folder never trusted. Each is recorded only when the line
/// needs it. With an empty `hash` the call records these and trusts nothing, and the line can then be loaded and
/// reviewed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BoardTrustParams {
    pub board: String,
    pub line: String,
    pub hash: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hosts: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dev_folders: Vec<String>,
}

/// `board.trust` result: the line's trust after the call.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BoardTrustResult {
    pub trust: TrustState,
}

/// `board.trust_review` params: the line (a ref) of a board that a person is about to trust.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BoardTrustReviewParams {
    pub board: String,
    pub line: String,
}

/// How one thing the trust hash covers differs from what was trusted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TrustChangeKind {
    Added,
    Removed,
    Changed,
}

/// One thing the trust hash covers that is new, gone or different since the person trusted the line: `board.yaml`, a
/// pack's folder, a view. A folder shows its changed `files`; a file shows its changed `lines`. Where neither can be
/// shown, `note` says why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TrustChange {
    pub name: String,
    pub kind: TrustChangeKind,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<PackFileChange>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lines: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// A pack of the line that its repo supplies, with what it runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TrustPackReview {
    pub source: String,
    pub review: PackReviewData,
}

/// `board.trust_review` result: what a person sees before they trust a line. `hash` is the one `board.trust` takes. It
/// is empty when the line did not load: `problems` says why, `hosts` holds the hosts a fetch would contact for the
/// first time and `dev_folders` the `dev:` folders never trusted, and approving them (a `board.trust` with an empty
/// hash) lets the load go on. `changes` are against what was trusted, or everything as added when nothing was.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BoardTrustReviewResult {
    pub trust: TrustState,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub problems: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub changes: Vec<TrustChange>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub packs: Vec<TrustPackReview>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hosts: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dev_folders: Vec<String>,
}
