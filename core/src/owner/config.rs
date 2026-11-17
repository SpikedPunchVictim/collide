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

/// One setting: its effective value and the layer it came from. A secret has no `value`: it lives in the OS
/// keychain, and no method returns it (0.7, Secrets).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConfigEntry {
    pub key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<Value>,
    pub layer: SettingLayer,
    pub secret: bool,
}

/// `config.get` params (`collide config get <key> [--profile <name>]`). With no `key`, every setting. `board` picks
/// the board whose repo layer applies, and is left out for the user's own layers. `profile` is applied as the top
/// layer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConfigGetParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub board: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
}

/// `config.get` result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConfigGetResult {
    pub entries: Vec<ConfigEntry>,
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
}

/// One pack of a board. `dev` is a pack run from a folder (`dev:`), which is badged everywhere (0.7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PackEntry {
    pub name: String,
    pub source: String,
    pub origin: PackOrigin,
    pub enabled: bool,
    pub dev: bool,
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConfigPackAddParams {
    pub source: String,
}

/// The result of `config.pack_add`, `config.pack_link` and `config.pack_enable`: the pack after the change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConfigPackResult {
    pub pack: PackEntry,
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
