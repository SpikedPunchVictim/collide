//! The `pack.yaml` keys and values that planning adds (0.7, Keys added for planning), and the few identity keys
//! the built-in packs need to load (`apiVersion`, `name`, `requires.tools`, `providers`). `workflows:` holds the
//! workflow types of [`crate::workflow`]. This is the start of the manifest type: `ts-0012-ua` adds the other
//! keys beside these, makes the identity keys required and checks them (and runs the workflow checks over each
//! entry of `workflows:`). `views:` and `profiles:` hold the types of [`crate::view`] and [`crate::profile`]
//! (`ts-0026-es`).

use crate::item::Category;
use crate::points::DerivedPoint;
use crate::profile::Profile;
use crate::view::View;
use crate::workflow::Workflow;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A run's total budget. At the total, the run goes to `ask_human` with cause budget.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RunBudget {
    pub usd: f64,
}

/// One line of a checklist template. `when:` is CEL over the item.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ChecklistTemplateItem {
    pub id: String,
    pub text: String,
    #[serde(default)]
    pub required: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<String>,
}

/// A checklist template. `gates` is the category whose status write the pack's gate blocks. The template's name
/// is its key in `checklists:`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ChecklistTemplate {
    pub version: u32,
    pub gates: Category,
    pub items: Vec<ChecklistTemplateItem>,
}

/// A tool the pack needs on `PATH` (0.7, `requires.tools`). `version` is a range such as `">=22.18"`; it is
/// optional, as in `- { name: pnpm }`. Checking it is Stage 4.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ToolRequirement {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

/// `requires:` in a manifest. Only `tools` is read so far; `sdk` and `agents` come with `ts-0012-ua`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Requires {
    #[serde(default)]
    pub tools: Vec<ToolRequirement>,
}

/// One entry of `providers:`: the script that starts the provider, relative to the pack directory.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProviderEntry {
    pub script: String,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PackManifest {
    #[serde(
        rename = "apiVersion",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub api_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requires: Option<Requires>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub providers: BTreeMap<String, ProviderEntry>,
    #[serde(default)]
    pub points: BTreeMap<String, DerivedPoint>,
    #[serde(default)]
    pub checklists: BTreeMap<String, ChecklistTemplate>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub workflows: BTreeMap<String, Workflow>,
    /// Named views a board may adopt by name (0.7, Keys added for planning).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub views: BTreeMap<String, View>,
    /// Named profiles, which cover only this pack's own settings and workflows (0.7, Profiles).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub profiles: BTreeMap<String, Profile>,
}
