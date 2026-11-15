//! The `pack.yaml` keys and values that planning adds (0.7, Keys added for planning). This is the start of the
//! manifest type: `ts-0012-ua` adds the other keys beside `points` and `checklists`, and `views:` comes with the
//! view type in `ts-0026-es`.

use crate::item::Category;
use crate::points::DerivedPoint;
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

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PackManifest {
    #[serde(default)]
    pub points: BTreeMap<String, DerivedPoint>,
    #[serde(default)]
    pub checklists: BTreeMap<String, ChecklistTemplate>,
}
