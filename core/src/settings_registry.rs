//! The registry of Collide's own settings (0.7, Settings): one entry per key, with its JSON type and range, its
//! default and the three flags that say how the layers treat it. It is data. The layers that apply it
//! (`settings_layers`), `collide config get` and `collide config set` are Stage 4a's later items.
//!
//! The flags (0.7, Layers and sources):
//! - `overridable`: the user's value wins over the repo's, because the setting is a fact about one machine;
//! - `repo_only`: the key describes the repo and is refused in the user's file and in a user profile;
//! - `per_user`: the key is the user's by nature and is refused in the repo's file.
//!
//! A key is never `repo_only` together with another flag, and a `per_user` key is never `overridable`, as a pack's
//! setting may not be (`repoOnly` with `overridable` or `secret` is refused at load). The tests hold the list.
//!
//! The keys are the ones [`crate::settings::ConfigFile`] can hold in `ask`, `disk` and `claude`. The settings of
//! later stages (the concurrency cap, the metrics level, retention, default aliases) are added by the stage that
//! builds them. `agents.<id>.<field>` has rules of its own and is not here.

use crate::settings::{MAX_TIMEOUT_S, MIN_TIMEOUT_S};
use serde_json::{Value, json};

/// The smallest `disk.floor_gib`, in GiB.
pub const MIN_FLOOR_GIB: u64 = 1;

/// The JSON type of a setting's value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueType {
    String,
    Boolean,
    Integer,
    Number,
}

impl ValueType {
    /// The name JSON Schema gives the type.
    pub fn as_str(self) -> &'static str {
        match self {
            ValueType::String => "string",
            ValueType::Boolean => "boolean",
            ValueType::Integer => "integer",
            ValueType::Number => "number",
        }
    }
}

/// The value a key has when no layer sets it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingDefault {
    Boolean(bool),
    Integer(i64),
}

/// One setting of Collide's own. `min` and `max` are the inclusive range of a whole number, and are `None` for a
/// key that has none. `default` is `None` for a key with no value of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CollideSetting {
    /// `section.key`, as the file nests it.
    pub key: &'static str,
    pub value_type: ValueType,
    pub min: Option<i64>,
    pub max: Option<i64>,
    pub default: Option<SettingDefault>,
    pub overridable: bool,
    pub repo_only: bool,
    pub per_user: bool,
}

impl CollideSetting {
    /// The default as the JSON value a file would hold, or `None`.
    pub fn default_value(&self) -> Option<Value> {
        self.default.map(|default| match default {
            SettingDefault::Boolean(value) => json!(value),
            SettingDefault::Integer(value) => json!(value),
        })
    }
}

/// Collide's own settings, in the order the layers and `collide config get` list them.
pub const COLLIDE_SETTINGS: &[CollideSetting] = &[
    // The agent entry the ask box uses. A person's choice, so never in a repo file.
    CollideSetting {
        key: "ask.agent",
        value_type: ValueType::String,
        min: None,
        max: None,
        default: None,
        overridable: false,
        repo_only: false,
        per_user: true,
    },
    // No default of its own: each agent entry has one (0.7, Agent entries).
    CollideSetting {
        key: "ask.timeout_s",
        value_type: ValueType::Integer,
        min: Some(MIN_TIMEOUT_S as i64),
        max: Some(MAX_TIMEOUT_S as i64),
        default: None,
        overridable: true,
        repo_only: false,
        per_user: false,
    },
    // No cap unless a layer sets one.
    CollideSetting {
        key: "ask.budget_usd",
        value_type: ValueType::Number,
        min: None,
        max: None,
        default: None,
        overridable: false,
        repo_only: false,
        per_user: false,
    },
    // How much free disk a machine keeps, a fact about the machine.
    CollideSetting {
        key: "disk.floor_gib",
        value_type: ValueType::Integer,
        min: Some(MIN_FLOOR_GIB as i64),
        max: None,
        default: Some(SettingDefault::Integer(5)),
        overridable: true,
        repo_only: false,
        per_user: false,
    },
    // Whether to fetch the published `claude` version for the version chip.
    CollideSetting {
        key: "claude.check_published",
        value_type: ValueType::Boolean,
        min: None,
        max: None,
        default: Some(SettingDefault::Boolean(true)),
        overridable: false,
        repo_only: false,
        per_user: true,
    },
];

impl CollideSetting {
    /// The registry's type and range as a JSON Schema, so a value from a profile is checked as a pack's is.
    pub fn schema(&self) -> Value {
        let mut schema = json!({ "type": self.value_type.as_str() });
        if let Some(min) = self.min {
            schema["minimum"] = json!(min);
        }
        if let Some(max) = self.max {
            schema["maximum"] = json!(max);
        }
        schema
    }
}

/// The first part of every key Collide owns: `agents`, `ask`, `disk` and `claude`. A pack may not take one as its
/// name, because `<pack>.<key>` would then name two things.
pub fn collide_roots() -> Vec<&'static str> {
    let mut roots = vec![AGENTS_ROOT];
    for setting in COLLIDE_SETTINGS {
        let root = setting.key.split('.').next().unwrap_or(setting.key);
        if !roots.contains(&root) {
            roots.push(root);
        }
    }
    roots
}

/// The root of the agent entries (`agents.<id>.<field>`), which have rules of their own and are not in the registry.
pub const AGENTS_ROOT: &str = "agents";

/// The entry for `key` (`disk.floor_gib`), or `None` for a key that is not one of Collide's own.
pub fn collide_setting(key: &str) -> Option<&'static CollideSetting> {
    COLLIDE_SETTINGS.iter().find(|setting| setting.key == key)
}

/// The keys the user's value wins for (0.7: a slow local model is a fact about one machine). The others a repo file
/// sets win over the user's.
pub fn overridable_keys() -> Vec<&'static str> {
    COLLIDE_SETTINGS
        .iter()
        .filter(|setting| setting.overridable)
        .map(|setting| setting.key)
        .collect()
}

/// The keys a repo `config.yaml` may set: every one that is not per user.
pub fn repo_file_keys() -> Vec<&'static str> {
    COLLIDE_SETTINGS
        .iter()
        .filter(|setting| !setting.per_user)
        .map(|setting| setting.key)
        .collect()
}
