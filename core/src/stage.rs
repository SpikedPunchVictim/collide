//! The stages of `IMPLEMENTATION_PLAN.md` (0.15). Each point and each event names the stage that makes it fire. The
//! catalogue test checks that every stage named here is a heading of the plan.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A stage, by the id its heading carries in the plan: `## Stage 2:`, `### Stage 3a:`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum Stage {
    #[serde(rename = "1")]
    S1,
    #[serde(rename = "2")]
    S2,
    #[serde(rename = "3a")]
    S3a,
    #[serde(rename = "3b")]
    S3b,
    #[serde(rename = "3c")]
    S3c,
    #[serde(rename = "3d")]
    S3d,
    #[serde(rename = "4a")]
    S4a,
    #[serde(rename = "4b")]
    S4b,
    #[serde(rename = "4c")]
    S4c,
    #[serde(rename = "5")]
    S5,
    #[serde(rename = "6a")]
    S6a,
    #[serde(rename = "6b")]
    S6b,
    #[serde(rename = "6c")]
    S6c,
    #[serde(rename = "7")]
    S7,
    #[serde(rename = "8")]
    S8,
    #[serde(rename = "9a")]
    S9a,
    #[serde(rename = "9b")]
    S9b,
    #[serde(rename = "9c")]
    S9c,
    #[serde(rename = "9d")]
    S9d,
    #[serde(rename = "10")]
    S10,
    #[serde(rename = "11")]
    S11,
    #[serde(rename = "12")]
    S12,
    #[serde(rename = "13")]
    S13,
    #[serde(rename = "14")]
    S14,
    #[serde(rename = "15")]
    S15,
}

impl Stage {
    /// Every stage, in the plan's order.
    pub const ALL: [Stage; 25] = [
        Stage::S1,
        Stage::S2,
        Stage::S3a,
        Stage::S3b,
        Stage::S3c,
        Stage::S3d,
        Stage::S4a,
        Stage::S4b,
        Stage::S4c,
        Stage::S5,
        Stage::S6a,
        Stage::S6b,
        Stage::S6c,
        Stage::S7,
        Stage::S8,
        Stage::S9a,
        Stage::S9b,
        Stage::S9c,
        Stage::S9d,
        Stage::S10,
        Stage::S11,
        Stage::S12,
        Stage::S13,
        Stage::S14,
        Stage::S15,
    ];

    /// The id as the plan's heading writes it: `2`, `3a`. The heading is `Stage <id>:`.
    pub fn id(self) -> &'static str {
        match self {
            Stage::S1 => "1",
            Stage::S2 => "2",
            Stage::S3a => "3a",
            Stage::S3b => "3b",
            Stage::S3c => "3c",
            Stage::S3d => "3d",
            Stage::S4a => "4a",
            Stage::S4b => "4b",
            Stage::S4c => "4c",
            Stage::S5 => "5",
            Stage::S6a => "6a",
            Stage::S6b => "6b",
            Stage::S6c => "6c",
            Stage::S7 => "7",
            Stage::S8 => "8",
            Stage::S9a => "9a",
            Stage::S9b => "9b",
            Stage::S9c => "9c",
            Stage::S9d => "9d",
            Stage::S10 => "10",
            Stage::S11 => "11",
            Stage::S12 => "12",
            Stage::S13 => "13",
            Stage::S14 => "14",
            Stage::S15 => "15",
        }
    }
}
