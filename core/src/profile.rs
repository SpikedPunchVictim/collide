//! A profile (0.7, Profiles): a named setup for a kind of job, the top settings layer. A profile is a YAML file in
//! `.collide/profiles/`, in `~/.config/collide/profiles/`, or an entry of a pack's `profiles:`.
//!
//! The types are the file. Which keys a profile may set depends on where it lives (a repo profile any key, a pack's
//! profile only that pack's own settings, a user profile only user packs and `overridable` keys). This module holds
//! the part of that rule which needs no pack or settings: [`user_profile_refusals`] names the keys a user profile
//! may not set at all, as data. Applying it at load, and checking `packs:` and `config:` against the user's packs
//! and the packs' `overridable` keys, is Stage 4a's.

use crate::view::is_namespaced_name;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// The smallest `compact_at` a role may set (0.3, Context options): a smaller window is ignored by the agent
/// tool without a warning, so Collide refuses it.
pub const MIN_COMPACT_AT: u64 = 100_000;

/// `packs: { on: [...], off: [...] }`: packs this profile switches on and off. A repo profile switches repo
/// packs and a user profile user packs.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProfilePacks {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub on: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub off: Vec<String>,
}

/// `config: user | none`: which settings the agent tool reads (0.3, Context options).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RoleConfig {
    User,
    None,
}

/// `repo_instructions: worktree | none`: whether the agent tool loads the repository's own instructions (`CLAUDE.md`,
/// skills, commands) from the worktree (0.3, Context options; revision 21). Absent means `worktree`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RepoInstructions {
    Worktree,
    None,
}

/// A role's context options (0.3), as a profile overrides them.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RoleOptions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compact_at: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config: Option<RoleConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repo_instructions: Option<RepoInstructions>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bash_output_max: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mcp_output_max: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "ProfilePacks::is_empty")]
    pub packs: ProfilePacks,
    /// The default workflow under this profile, as `pack/workflow`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workflow: Option<String>,
    /// Launch parameters (`maxRounds`, ...). The workflow declares which exist.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub launch: BTreeMap<String, Value>,
    /// Model alias to model name. The model source and account stay the user's.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub models: BTreeMap<String, String>,
    /// Context options by role, as `pack/role`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub roles: BTreeMap<String, RoleOptions>,
    /// Pack settings: pack name, then key, then value. A pack's schema checks each value.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub config: BTreeMap<String, BTreeMap<String, Value>>,
}

impl ProfilePacks {
    fn is_empty(&self) -> bool {
        self.on.is_empty() && self.off.is_empty()
    }
}

/// A top-level key of a profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileKey {
    Description,
    Packs,
    Workflow,
    Launch,
    Models,
    Roles,
    Config,
}

impl ProfileKey {
    pub fn as_str(self) -> &'static str {
        match self {
            ProfileKey::Description => "description",
            ProfileKey::Packs => "packs",
            ProfileKey::Workflow => "workflow",
            ProfileKey::Launch => "launch",
            ProfileKey::Models => "models",
            ProfileKey::Roles => "roles",
            ProfileKey::Config => "config",
        }
    }
}

impl fmt::Display for ProfileKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The keys a user profile may set at all (0.7: "a user profile switches only user packs and sets only
/// `overridable` keys"). `packs` and `config` stay narrowed by what only the load knows: the user's packs and
/// the `overridable` keys.
pub const USER_PROFILE_KEYS: [ProfileKey; 3] = [
    ProfileKey::Description,
    ProfileKey::Packs,
    ProfileKey::Config,
];

impl Profile {
    /// The top-level keys this profile sets, in the order of [`ProfileKey`]. A key that holds nothing is not set.
    pub fn keys_set(&self) -> Vec<ProfileKey> {
        let mut keys = Vec::new();
        if self.description.is_some() {
            keys.push(ProfileKey::Description);
        }
        if !self.packs.is_empty() {
            keys.push(ProfileKey::Packs);
        }
        if self.workflow.is_some() {
            keys.push(ProfileKey::Workflow);
        }
        if !self.launch.is_empty() {
            keys.push(ProfileKey::Launch);
        }
        if !self.models.is_empty() {
            keys.push(ProfileKey::Models);
        }
        if !self.roles.is_empty() {
            keys.push(ProfileKey::Roles);
        }
        if !self.config.is_empty() {
            keys.push(ProfileKey::Config);
        }
        keys
    }
}

/// A key a user profile set and may not: the load refuses the profile and names the key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UserProfileRefusal {
    pub key: ProfileKey,
}

impl fmt::Display for UserProfileRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "a user profile may not set `{}`", self.key)
    }
}

impl std::error::Error for UserProfileRefusal {}

/// The keys of `profile` that a user profile may not set, one refusal each, in the order of [`ProfileKey`].
pub fn user_profile_refusals(profile: &Profile) -> Vec<UserProfileRefusal> {
    profile
        .keys_set()
        .into_iter()
        .filter(|key| !USER_PROFILE_KEYS.contains(key))
        .map(|key| UserProfileRefusal { key })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileError {
    /// A pack named in both `packs.on` and `packs.off`.
    PackSwitchedBothWays { pack: String },
    /// An empty pack name in `packs.on` or `packs.off`. `key` is the list's name.
    EmptyPackName { key: &'static str },
    /// A `workflow:` that is not written `pack/workflow`.
    BadWorkflowName { name: String },
    /// A role key that is not written `pack/role`.
    BadRoleName { name: String },
    /// A `compact_at` under [`MIN_COMPACT_AT`].
    CompactAtBelowFloor { role: String, value: u64 },
    /// A model alias or a model name that is empty.
    EmptyModel { alias: String },
    /// A `config:` entry whose pack name is empty.
    EmptyConfigPack,
}

impl fmt::Display for ProfileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProfileError::PackSwitchedBothWays { pack } => {
                write!(f, "pack `{pack}` is in both `packs.on` and `packs.off`")
            }
            ProfileError::EmptyPackName { key } => write!(f, "`packs.{key}` has an empty name"),
            ProfileError::BadWorkflowName { name } => {
                write!(f, "`workflow: {name}` is not written `pack/workflow`")
            }
            ProfileError::BadRoleName { name } => {
                write!(f, "role `{name}` is not written `pack/role`")
            }
            ProfileError::CompactAtBelowFloor { role, value } => write!(
                f,
                "role `{role}`: `compact_at: {value}` is under the floor of {MIN_COMPACT_AT}"
            ),
            ProfileError::EmptyModel { alias } => {
                write!(f, "`models` entry `{alias}` is empty")
            }
            ProfileError::EmptyConfigPack => write!(f, "`config:` has an entry with no pack name"),
        }
    }
}

impl std::error::Error for ProfileError {}

/// Checks a profile on its own: pack lists, names and role options. Returns every refusal. Which keys the
/// profile's place allows is [`user_profile_refusals`] and Stage 4a.
pub fn check_profile(profile: &Profile) -> Result<(), Vec<ProfileError>> {
    let mut errors = Vec::new();
    for (key, names) in [("on", &profile.packs.on), ("off", &profile.packs.off)] {
        if names.iter().any(|name| name.trim().is_empty()) {
            errors.push(ProfileError::EmptyPackName { key });
        }
    }
    let on: BTreeSet<&String> = profile.packs.on.iter().collect();
    for pack in profile.packs.off.iter().filter(|name| on.contains(name)) {
        errors.push(ProfileError::PackSwitchedBothWays { pack: pack.clone() });
    }
    if let Some(workflow) = &profile.workflow
        && !is_namespaced_name(workflow)
    {
        errors.push(ProfileError::BadWorkflowName {
            name: workflow.clone(),
        });
    }
    for (alias, model) in &profile.models {
        if alias.trim().is_empty() || model.trim().is_empty() {
            errors.push(ProfileError::EmptyModel {
                alias: alias.clone(),
            });
        }
    }
    for (role, options) in &profile.roles {
        if !is_namespaced_name(role) {
            errors.push(ProfileError::BadRoleName { name: role.clone() });
        }
        if let Some(value) = options.compact_at
            && value < MIN_COMPACT_AT
        {
            errors.push(ProfileError::CompactAtBelowFloor {
                role: role.clone(),
                value,
            });
        }
    }
    if profile.config.keys().any(|pack| pack.trim().is_empty()) {
        errors.push(ProfileError::EmptyConfigPack);
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Where a named profile was found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileSource {
    /// A file of the snapshot's `.collide/profiles/`.
    Repo,
    /// A file of `~/.config/collide/profiles/`.
    User,
    /// An entry of the named pack's `profiles:`.
    Pack(String),
}

/// The folder a repo's profiles are in, and the user's: where a lookup says it looked.
pub const REPO_PROFILES_DIR: &str = ".collide/profiles";
pub const USER_PROFILES_DIR: &str = "~/.config/collide/profiles";

/// Why a profile name found no single profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileLookupError {
    /// Nothing is called `name`. `looked` lists the places searched.
    NotFound { name: String, looked: Vec<String> },
    /// More than one place has `name`, and Collide will not pick one.
    InTwoPlaces { name: String, places: Vec<String> },
}

impl fmt::Display for ProfileLookupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProfileLookupError::NotFound { name, looked } => write!(
                f,
                "no profile is named `{name}`; looked in {} (a pack's profile is written `<pack>/<profile>`)",
                looked.join(", ")
            ),
            ProfileLookupError::InTwoPlaces { name, places } => write!(
                f,
                "the profile `{name}` is in {}: rename one, so that the name says which is meant",
                places.join(" and ")
            ),
        }
    }
}

impl std::error::Error for ProfileLookupError {}

/// A profile and where it was found.
#[derive(Debug, Clone)]
pub struct FoundProfile<'a> {
    pub source: ProfileSource,
    pub profile: &'a Profile,
}

/// Finds `name` among the repo's profiles, the user's, and the packs' (`<pack>/<profile>`). A name in two places is
/// an error naming both, and one in none is an error naming where it looked.
pub fn find_profile<'a>(
    name: &str,
    repo: &'a BTreeMap<String, Profile>,
    user: &'a BTreeMap<String, Profile>,
    packs: &'a [(String, BTreeMap<String, Profile>)],
) -> Result<FoundProfile<'a>, ProfileLookupError> {
    let mut found: Vec<(String, FoundProfile<'a>)> = Vec::new();
    if let Some(profile) = repo.get(name) {
        found.push((
            format!("{REPO_PROFILES_DIR} (repo)"),
            FoundProfile {
                source: ProfileSource::Repo,
                profile,
            },
        ));
    }
    if let Some(profile) = user.get(name) {
        found.push((
            format!("{USER_PROFILES_DIR} (user)"),
            FoundProfile {
                source: ProfileSource::User,
                profile,
            },
        ));
    }
    for (pack, profiles) in packs {
        let Some(own) = name
            .strip_prefix(pack.as_str())
            .and_then(|rest| rest.strip_prefix('/'))
        else {
            continue;
        };
        if let Some(profile) = profiles.get(own) {
            found.push((
                format!("the pack `{pack}`"),
                FoundProfile {
                    source: ProfileSource::Pack(pack.clone()),
                    profile,
                },
            ));
        }
    }
    match found.len() {
        1 => Ok(found.remove(0).1),
        0 => {
            let mut looked = vec![REPO_PROFILES_DIR.to_string(), USER_PROFILES_DIR.to_string()];
            looked.extend(packs.iter().map(|(pack, _)| format!("the pack `{pack}`")));
            Err(ProfileLookupError::NotFound {
                name: name.to_string(),
                looked,
            })
        }
        _ => Err(ProfileLookupError::InTwoPlaces {
            name: name.to_string(),
            places: found.into_iter().map(|(place, _)| place).collect(),
        }),
    }
}
