//! The four settings layers of 0.7 as one pure function (ts-0098-nx): the schemas' defaults, the user's files
//! (the per-board file wins within them), the repo's file, and the run's profile on top. The repo's value wins over
//! the user's, except for a key its schema marks `overridable`, where the user's wins. The profile wins over both.
//!
//! Nothing is read here: the caller passes the loaded files and packs. Every refusal is a [`SettingProblem`] that
//! names the key, and a refused value is not applied, so the layer below stays in force and `collide config get` can
//! still print the rest. No message holds a value: a secret that reached a file by mistake must not be echoed.

use crate::owner::{ConfigEntry, SettingLayer};
use crate::pack::SchemaObject;
use crate::profile::{Profile, ProfileSource, user_profile_refusals};
use crate::setting_value::check_value;
use crate::settings::{ConfigFile, repo_file_refusals};
use crate::settings_registry::{AGENTS_ROOT, COLLIDE_SETTINGS, collide_roots};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fmt;

/// A loaded pack's name and its `config:` schemas.
#[derive(Debug, Clone, Copy)]
pub struct PackSettings<'a> {
    pub pack: &'a str,
    pub config: &'a BTreeMap<String, SchemaObject>,
}

/// The profile applied as the top layer, with the name it was asked for and where it was found.
#[derive(Debug, Clone, Copy)]
pub struct ProfileUse<'a> {
    pub name: &'a str,
    pub source: &'a ProfileSource,
    pub profile: &'a Profile,
}

/// Everything the layers read.
#[derive(Debug, Clone, Copy)]
pub struct LayerInputs<'a> {
    pub packs: &'a [PackSettings<'a>],
    /// `~/.config/collide/config.yaml`.
    pub user: &'a ConfigFile,
    /// `~/.config/collide/boards/<board-id>/config.yaml`: wins over `user`.
    pub user_board: &'a ConfigFile,
    /// The line snapshot's `.collide/config.yaml`.
    pub repo: &'a ConfigFile,
    pub profile: Option<ProfileUse<'a>>,
}

/// One refusal, with the key it is about (`ferry.lanePool`; a section or a profile key when no single setting is).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingProblem {
    pub key: String,
    pub message: String,
}

impl fmt::Display for SettingProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

/// The settings of a board: one entry per key, Collide's own first in the registry's order, then each pack's keys
/// by name. And every problem found, none of them stopping the rest.
#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    pub entries: Vec<ConfigEntry>,
    pub problems: Vec<SettingProblem>,
}

/// What the layers know about one key.
struct KeyInfo {
    key: String,
    /// The pack the key belongs to, or `None` for one of Collide's own.
    pack: Option<String>,
    schema: Value,
    overridable: bool,
    repo_only: bool,
    per_user: bool,
    secret: bool,
}

impl KeyInfo {
    fn default_value(&self) -> Option<Value> {
        if self.secret {
            return None;
        }
        match &self.pack {
            None => crate::settings_registry::collide_setting(&self.key)?.default_value(),
            Some(_) => self.schema.get("default").cloned(),
        }
    }
}

/// Where a value was written.
#[derive(Clone, Copy)]
enum Origin<'a> {
    User,
    UserBoard,
    Repo,
    Profile(ProfileUse<'a>),
}

impl Origin<'_> {
    /// The place, as a message names it.
    fn label(&self) -> String {
        match self {
            Origin::User => "the user's config".into(),
            Origin::UserBoard => "the user's config for this board".into(),
            Origin::Repo => "the repo's config".into(),
            Origin::Profile(profile) => format!("profile `{}`", profile.name),
        }
    }
}

fn key_infos(packs: &[PackSettings]) -> Vec<KeyInfo> {
    let mut infos: Vec<KeyInfo> = COLLIDE_SETTINGS
        .iter()
        .map(|setting| KeyInfo {
            key: setting.key.to_string(),
            pack: None,
            schema: setting.schema(),
            overridable: setting.overridable,
            repo_only: setting.repo_only,
            per_user: setting.per_user,
            secret: false,
        })
        .collect();
    for pack in packs {
        for (key, schema) in pack.config {
            let flag = |name: &str| schema.0.get(name) == Some(&Value::Bool(true));
            infos.push(KeyInfo {
                key: format!("{}.{key}", pack.pack),
                pack: Some(pack.pack.to_string()),
                schema: schema.0.clone(),
                overridable: flag("overridable"),
                repo_only: flag("repoOnly"),
                per_user: false,
                secret: flag("secret"),
            });
        }
    }
    infos
}

/// Resolves the layers. See the module docs.
pub fn settings_layers(inputs: &LayerInputs) -> Settings {
    let infos = key_infos(inputs.packs);
    let mut problems = Vec::new();

    // Every value a file or the profile sets, in layer order, with the problems of reading them.
    let mut sets: Vec<Set> = Vec::new();
    let packs = inputs.packs;
    read_file(
        inputs.user,
        Origin::User,
        &infos,
        packs,
        &mut problems,
        &mut sets,
    );
    read_file(
        inputs.user_board,
        Origin::UserBoard,
        &infos,
        packs,
        &mut problems,
        &mut sets,
    );
    problems.extend(
        repo_file_refusals(inputs.repo)
            .into_iter()
            .map(|refusal| SettingProblem {
                key: refusal.key.clone(),
                message: refusal.to_string(),
            }),
    );
    read_file(
        inputs.repo,
        Origin::Repo,
        &infos,
        packs,
        &mut problems,
        &mut sets,
    );
    if let Some(profile) = inputs.profile {
        read_profile(profile, &infos, packs, &mut problems, &mut sets);
    }

    // Keep the values that pass; the others are problems and leave the layer below in force.
    let mut kept: BTreeMap<&str, Vec<(Origin, Value)>> = BTreeMap::new();
    for set in &sets {
        let info = infos.iter().find(|info| info.key == set.key);
        let Some(info) = info else { continue };
        match check_set(info, set) {
            Ok(()) => kept
                .entry(info.key.as_str())
                .or_default()
                .push((set.origin, set.value.clone())),
            Err(message) => problems.push(SettingProblem {
                key: info.key.clone(),
                message,
            }),
        }
    }

    let entries = infos
        .iter()
        .map(|info| {
            entry(
                info,
                kept.get(info.key.as_str())
                    .map(Vec::as_slice)
                    .unwrap_or(&[]),
            )
        })
        .collect();
    Settings { entries, problems }
}

/// One value a layer sets, before it is checked.
struct Set<'a> {
    key: String,
    value: Value,
    origin: Origin<'a>,
}

/// The layer of the key's value: the last one that set it, but the user's beats the repo's for an overridable key.
fn entry(info: &KeyInfo, values: &[(Origin, Value)]) -> ConfigEntry {
    if info.secret {
        return ConfigEntry {
            key: info.key.clone(),
            value: None,
            layer: SettingLayer::User,
            secret: true,
        };
    }
    let mut winner: Option<(SettingLayer, &Value)> = None;
    for (origin, value) in values {
        let layer = match origin {
            Origin::User | Origin::UserBoard => SettingLayer::User,
            Origin::Repo => SettingLayer::Repo,
            Origin::Profile(_) => SettingLayer::Profile,
        };
        let repo_over_user = layer == SettingLayer::Repo
            && info.overridable
            && winner.is_some_and(|(l, _)| l == SettingLayer::User);
        if !repo_over_user {
            winner = Some((layer, value));
        }
    }
    match winner {
        Some((layer, value)) => ConfigEntry {
            key: info.key.clone(),
            value: Some(value.clone()),
            layer,
            secret: false,
        },
        None => ConfigEntry {
            key: info.key.clone(),
            value: info.default_value(),
            layer: SettingLayer::Default,
            secret: false,
        },
    }
}

/// The refusals that depend on the key and where it was written, then the schema.
fn check_set(info: &KeyInfo, set: &Set) -> Result<(), String> {
    let key = &info.key;
    let place = set.origin.label();
    if info.secret {
        return Err(format!(
            "`{key}` is a secret and may not be in {place}: it lives in the OS keychain, so set it with \
             `collide config set --secret`"
        ));
    }
    match set.origin {
        Origin::User | Origin::UserBoard if info.repo_only => {
            return Err(format!(
                "`{key}` is repoOnly: it describes the repo, so it is set in the repo's config and not in {place}"
            ));
        }
        Origin::Profile(profile) => check_profile_key(info, profile)?,
        _ => {}
    }
    let violations = check_value(&info.schema, &set.value);
    if violations.is_empty() {
        return Ok(());
    }
    let found: Vec<String> = violations.iter().map(ToString::to_string).collect();
    Err(format!("`{key}` in {place} {}", found.join("; ")))
}

/// What a profile of each source may set (0.7, Profiles).
fn check_profile_key(info: &KeyInfo, profile: ProfileUse) -> Result<(), String> {
    let key = &info.key;
    let name = profile.name;
    match profile.source {
        ProfileSource::Repo => {
            if info.per_user {
                return Err(format!(
                    "`{key}` is per user and may not be set in profile `{name}` of the repo"
                ));
            }
        }
        ProfileSource::User => {
            if info.repo_only {
                return Err(format!(
                    "`{key}` is repoOnly and may not be set in profile `{name}`, which is a user profile"
                ));
            }
            if !info.overridable {
                return Err(format!(
                    "`{key}` is not marked overridable, so profile `{name}`, which is a user profile, may not set it"
                ));
            }
        }
        ProfileSource::Pack(pack) => {
            if info.pack.as_deref() != Some(pack.as_str()) {
                return Err(format!(
                    "`{key}` is not a setting of the pack `{pack}`, and profile `{name}` of that pack may set only \
                     its own"
                ));
            }
        }
    }
    Ok(())
}

/// The values of one settings file: Collide's keys by name, then `config:` by pack.
fn read_file<'a>(
    file: &ConfigFile,
    origin: Origin<'a>,
    infos: &[KeyInfo],
    packs: &[PackSettings],
    problems: &mut Vec<SettingProblem>,
    sets: &mut Vec<Set<'a>>,
) {
    for setting in COLLIDE_SETTINGS {
        // A per-user key in the repo's file is already a problem of `repo_file_refusals`, and is not applied.
        if matches!(origin, Origin::Repo) && setting.per_user {
            continue;
        }
        if let Some(value) = file.collide_value(setting.key) {
            sets.push(Set {
                key: setting.key.to_string(),
                value,
                origin,
            });
        }
    }
    for (section, keys) in &file.config {
        if collide_roots().contains(&section.as_str()) {
            problems.push(SettingProblem {
                key: section.clone(),
                message: format!(
                    "`config.{section}` is a name Collide's own settings use, so it is no pack: write \
                     `{section}:` at the top of the file"
                ),
            });
            continue;
        }
        read_section(section, keys, origin, infos, packs, problems, sets);
    }
}

/// A profile's `config:`, where a section may also be one of Collide's own (`ask`, `disk`, `claude`), and the
/// top-level keys a user profile may not set.
fn read_profile<'a>(
    profile: ProfileUse<'a>,
    infos: &[KeyInfo],
    packs: &[PackSettings],
    problems: &mut Vec<SettingProblem>,
    sets: &mut Vec<Set<'a>>,
) {
    let origin = Origin::Profile(profile);
    if *profile.source == ProfileSource::User {
        for refusal in user_profile_refusals(profile.profile) {
            problems.push(SettingProblem {
                key: refusal.key.to_string(),
                message: format!("profile `{}`: {refusal}", profile.name),
            });
        }
    }
    for (section, keys) in &profile.profile.config {
        if section == AGENTS_ROOT {
            problems.push(SettingProblem {
                key: section.clone(),
                message: format!(
                    "profile `{}` sets `agents`, which a profile may not: agent entries belong to the user's \
                     config",
                    profile.name
                ),
            });
            continue;
        }
        read_section(section, keys, origin, infos, packs, problems, sets);
    }
}

/// The keys of one `config.<section>`, a pack's or (in a profile) one of Collide's own.
fn read_section<'a>(
    section: &str,
    keys: &BTreeMap<String, Value>,
    origin: Origin<'a>,
    infos: &[KeyInfo],
    packs: &[PackSettings],
    problems: &mut Vec<SettingProblem>,
    sets: &mut Vec<Set<'a>>,
) {
    let collide = collide_roots().contains(&section);
    // A pack the board does not load is left alone: its settings may belong to another board.
    if !collide && !packs.iter().any(|pack| pack.pack == section) {
        return;
    }
    for (name, value) in keys {
        let key = format!("{section}.{name}");
        if infos.iter().any(|info| info.key == key) {
            sets.push(Set {
                key,
                value: value.clone(),
                origin,
            });
            continue;
        }
        let has: Vec<&str> = infos
            .iter()
            .filter(|info| info.key.starts_with(&format!("{section}.")))
            .map(|info| &info.key[section.len() + 1..])
            .collect();
        let owner = if collide {
            "Collide's own settings".to_string()
        } else {
            format!("the pack `{section}`")
        };
        problems.push(SettingProblem {
            key: key.clone(),
            message: format!(
                "`{key}` in {} is not a setting of {owner}; it has {}",
                origin.label(),
                if has.is_empty() {
                    "none".to_string()
                } else {
                    has.join(", ")
                }
            ),
        });
    }
}
