//! Sample contract types for testing the schema pipeline. They are not part of `collide-core`'s API and are
//! never shipped. Together they cover each shape the contracts use: a tagged enum with a `Wait {}` variant, an
//! optional field, a defaulted field, a map, nested structs three deep, and integer fields.

use collide_core::schema::Registry;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The fixture registry. It is built with the same `Registry` that `collide_core::schema::registry` uses.
pub fn registry() -> Registry {
    let mut registry = Registry::new();
    registry.add::<Sample>();
    registry.add::<Step>();
    registry
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Sample {
    pub name: String,
    pub retries: u32,
    pub budget_bytes: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(default)]
    pub labels: BTreeMap<String, String>,
    pub steps: Vec<Step>,
    pub level1: Level1,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Level1 {
    pub level2: Level2,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Level2 {
    pub level3: Level3,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Level3 {
    pub value: String,
}

/// An internally tagged enum. `Wait {}` is an empty struct variant, never a unit variant, because serde does not
/// enforce `deny_unknown_fields` on a unit variant of an internally tagged enum.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Step {
    #[serde(rename_all = "camelCase")]
    Agent {
        prompt: String,
        #[serde(default)]
        read_only: bool,
    },
    Wait {},
}
