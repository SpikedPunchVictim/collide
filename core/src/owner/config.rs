//! The `config.*` methods: settings with the layer each value came from, and packs (`collide config`, `collide
//! pack`, the Settings page).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The layer a setting's value came from, lowest first (0.7, Values come from four layers).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SettingLayer {
    /// The schema's defaults, and the built-in agent entry.
    Default,
    /// `~/.config/collide/config.yaml`.
    User,
    /// The line snapshot's `.collide/config.yaml`.
    Repo,
    /// The run's profile, the top layer.
    Profile,
}

/// The layers a write can reach: the user's file and the repo's. The defaults are the schema's, and a profile is a
/// file of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ConfigWriteLayer {
    User,
    Repo,
}

/// A value a higher layer replaced: what the layer below it said (the Settings page shows it struck out).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OverriddenValue {
    pub value: Value,
    pub layer: SettingLayer,
}

/// One setting: its effective value and the layer it came from. A secret has no `value`: it lives in the OS
/// keychain, and no method returns it (0.7, Secrets). `overridden` is what the effective value replaced, when a
/// layer that sets the key was passed over (never for a secret). `is_set` is for a secret only: whether the keychain
/// holds an item for it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConfigEntry {
    pub key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<Value>,
    pub layer: SettingLayer,
    pub secret: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overridden: Option<OverriddenValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_set: Option<bool>,
}

/// `config.get` params (`collide config get <key> [--profile <name>]`). With no `key`, every setting. `board` picks
/// the board whose repo layer applies, and is left out for the user's own layers. `profile` is applied as the top
/// layer. With `with_problems` the problems of single settings come back in the result's `problems`, and the
/// entries are still given; without it they refuse the request, as they always have. A file or a pack that does
/// not load refuses it either way.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConfigGetParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub board: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    #[serde(default)]
    pub with_problems: bool,
}

/// One problem with a setting: the key it is about (a section or a profile key when no single setting is), the layer
/// that holds the bad value, and the words. The words never hold a value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConfigProblem {
    pub key: String,
    pub layer: SettingLayer,
    pub message: String,
}

/// A settings file of one layer, where the daemon looks for it. `for_board` marks the user's file for this board,
/// which wins over the user's own. `exists` is false when there is no such file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConfigLayerFile {
    pub layer: SettingLayer,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub for_board: bool,
    pub path: String,
    pub exists: bool,
}

/// `config.get` result. `problems` is empty unless the request asked for them. `files` lists the files the layers
/// are read from: the user's, the board's own of the user's, and the repo's.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConfigGetResult {
    pub entries: Vec<ConfigEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub problems: Vec<ConfigProblem>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<ConfigLayerFile>,
    /// What is worth saying and is no error, one line each (a default budget an agent cannot enforce).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notices: Vec<String>,
}

/// `config.set` params (`collide config set <key> <value> [--secret]`). With `secret` true the value goes to the OS
/// keychain, `layer` is ignored, and the daemon never logs, journals or echoes the request's `value`. The key is
/// checked against the setting's schema, and a secret in either settings file is refused (0.7).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConfigSetParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub board: Option<String>,
    pub key: String,
    pub value: Value,
    pub layer: ConfigWriteLayer,
    #[serde(default)]
    pub secret: bool,
}

/// `config.set` result: the setting after the write, so a repo value that shadows the user's is shown as the repo's.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConfigSetResult {
    pub entry: ConfigEntry,
}

/// Where a pack comes from (0.7, Repo packs and user packs).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum PackOrigin {
    /// Compiled into the bundle, so it needs no trust prompt (0.4).
    Builtin,
    /// Listed in the board's `board.yaml`, shared with the team.
    Repo,
    /// Added with `collide pack add`, switched on per board.
    User,
    /// A new folder under `.collide/packs/` in the user's checkout that `board.yaml` does not list: a working copy
    /// that can be linked (0.7, Working copies).
    Offered,
}

/// One script or provider a pack runs, with the manifest key that names it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PackReviewLine {
    pub key: String,
    pub script: String,
}

/// What a person is shown before trusting a pack (0.9, Per pack source): where it is from, the hooks, providers and
/// observers it runs, and what it requires.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PackReviewData {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// The commit a `git:` source resolved to. Absent for a `dev:` folder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    /// The tree id the pack is trusted at.
    pub tree: String,
    pub hooks: Vec<PackReviewLine>,
    pub providers: Vec<PackReviewLine>,
    /// A key holds the events the observer gets.
    pub observers: Vec<PackReviewLine>,
    pub requires: Vec<String>,
}

/// One pack of a board. `dev` is a pack run from a folder (`dev:`), which is badged everywhere (0.7). `working_copy`
/// marks a repo pack whose folder in the user's checkout differs from the line's ref, so `pack link` can use it.
/// `review` is what `pack add` asks the person to trust.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PackEntry {
    pub name: String,
    pub source: String,
    pub origin: PackOrigin,
    pub enabled: bool,
    pub dev: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub working_copy: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review: Option<PackReviewData>,
}

/// `config.pack_list` params (`collide pack list`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConfigPackListParams {
    pub board: String,
}

/// `config.pack_list` result, in pack order: the board's packs, then the user's.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConfigPackListResult {
    pub packs: Vec<PackEntry>,
}

/// `config.pack_add` params (`collide pack add <source>`): a `git:` or `dev:` source, added to the user's packs.
/// Without `confirm` nothing is written and the result carries the pack's `review`. With it the source is added, locked
/// and trusted at the tree id the review showed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConfigPackAddParams {
    pub source: String,
    #[serde(default)]
    pub confirm: bool,
    /// Hosts the person has said yes to fetching from for the first time; one that is not the host this fetch needs
    /// is ignored, as for `config.pack_update`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub approve_hosts: Vec<String>,
}

/// The result of `config.pack_add`, `config.pack_link` and `config.pack_enable`: the pack after the change.
/// `written` is false when nothing changed (a preview, or a change that was already made), and `note` says why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConfigPackResult {
    pub pack: PackEntry,
    #[serde(default)]
    pub written: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// A host never fetched from that the add needs a yes for: nothing was fetched or written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub needs_host: Option<String>,
}

/// `config.pack_link` params (`collide pack link <name> [<folder>]`). With no `folder` the pack's folder in the
/// user's own checkout is used (0.7, Working copies).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConfigPackLinkParams {
    pub board: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
}

/// `config.pack_enable` params: `collide pack enable <name>` is `enabled: true` and `disable` is `false`. A user
/// pack switches per board; a repo pack switches through `board.yaml`, so this refuses it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConfigPackEnableParams {
    pub board: String,
    pub name: String,
    pub enabled: bool,
}

/// What a `git:` source resolved to: the commit, and the tree at the pack's path in it (0.7, Pack sources).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PackPin {
    pub commit: String,
    pub tree: String,
}

/// How a file of the pack differs between two pins.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PackFileChangeKind {
    Added,
    Deleted,
    Modified,
    /// A file that became a folder, a link or a submodule, or the other way round.
    TypeChanged,
}

/// One file that differs between two pins of a pack: the manifest, a hook, a script.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PackFileChange {
    pub path: String,
    pub kind: PackFileChangeKind,
    /// The changed lines of a text file: `@@`, `-` and `+` lines, no context.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lines: Vec<String>,
    /// Why there are no lines, when there are none to show.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// `config.pack_update` params (`collide pack update <name> [--to <ref>]`): fetch the pack's repository, resolve its
/// ref (or `to`: a branch, a tag or a commit), and show what changed. Only with `confirm` is the lock written.
/// `approve_hosts` are hosts the person has said yes to for a first fetch: each is recorded when this fetch needs it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConfigPackUpdateParams {
    pub board: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    #[serde(default)]
    pub confirm: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub approve_hosts: Vec<String>,
}

/// `config.pack_update` result. `old` is the pin now; `new` is the pin the ref resolves to. With `needs_host` set the
/// fetch has not run (the first fetch from that host needs the person's yes), `new` is absent and nothing was written.
/// `written` is true when `confirm` was set and the lock changed, in the file `lock_file` names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConfigPackUpdateResult {
    pub pack: PackEntry,
    pub old: PackPin,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new: Option<PackPin>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub needs_host: Option<String>,
    /// The files that differ, in path order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub changes: Vec<PackFileChange>,
    /// What cannot be shown of the difference (the old version is no longer in the mirror).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    pub written: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lock_file: Option<String>,
}

/// `config.reset` params (Start with defaults, Reset): the board whose own folder is moved aside with the global files.
/// With no board only the global files move.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConfigResetParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub board: Option<String>,
}

/// `config.reset` result: the folder the files were moved into, and each file or folder moved, as a path under it.
/// Nothing is deleted. A file that was not there is not listed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConfigResetResult {
    pub folder: String,
    pub moved: Vec<String>,
}
