//! A small, realistic slice of the pack manifest (0.7): every serde feature the plan's manifest uses.
//! The same types carry all three generators' derives, behind features, so each pipeline can be built alone.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[cfg(feature = "schemars")]
use schemars::JsonSchema;
#[cfg(feature = "ts")]
use ts_rs::TS;

/// Top level of a `pack.yaml` (a slice of it).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(JsonSchema))]
#[cfg_attr(feature = "ts", derive(TS), ts(export, optional_fields))]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PackManifest {
    pub name: String,
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    #[cfg_attr(feature = "ts-fix", ts(as = "Option<BTreeMap<String, String>>"))]
    pub requires: BTreeMap<String, String>,
    pub workflows: BTreeMap<String, Workflow>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_workflow: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protected_paths: Option<ProtectedPaths>,
    /// One JSON Schema per setting key (0.7 `config:`), so the values are arbitrary JSON.
    #[serde(default)]
    #[cfg_attr(feature = "ts-fix", ts(as = "Option<BTreeMap<String, serde_json::Value>>"))]
    #[cfg_attr(feature = "specta", specta(type = BTreeMap<String, specta_typescript::Unknown>))]
    pub config: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(JsonSchema))]
#[cfg_attr(feature = "ts", derive(TS), ts(export, optional_fields))]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Workflow {
    pub phases: Vec<Phase>,
    #[serde(default)]
    #[cfg_attr(feature = "ts-fix", ts(as = "Option<u32>"))]
    pub max_rounds: u32,
    /// u64 on purpose: ts-rs and specta treat it as a bigint by default; JSON carries a number.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub timeout_ms: Option<u64>,
}

/// A workflow phase: an internally tagged enum, with a struct variant using renamed fields, a unit variant and a
/// recursive variant.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(JsonSchema))]
#[cfg_attr(feature = "ts", derive(TS), ts(export, optional_fields))]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Phase {
    #[serde(rename_all = "camelCase")]
    Agent {
        agent: String,
        prompt: String,
        #[serde(default)]
        #[cfg_attr(feature = "ts-fix", ts(as = "Option<bool>"))]
        read_only: bool,
    },
    #[serde(rename_all = "camelCase")]
    Gate {
        command: Vec<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        timeout_s: Option<u32>,
    },
    Hook {
        script: String,
    },
    Review,
    /// An empty struct variant: the form that `deny_unknown_fields` does enforce.
    Wait {},
    Fanout {
        over: String,
        phase: Box<Phase>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(JsonSchema))]
#[cfg_attr(feature = "ts", derive(TS), ts(export, optional_fields))]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct ProtectedPaths {
    pub paths: Vec<String>,
    pub at: When,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(JsonSchema))]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "kebab-case")]
pub enum When {
    PhaseEnd,
    Commit,
}
