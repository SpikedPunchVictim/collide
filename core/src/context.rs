//! The Queue item's context record (0.11, Queue; S6). The context view is a data contract: its six sections are the
//! fields of one record that Collide and the pack fill, and the UI shows a section the data lacks as
//! `Not available in this view`, never hides it. The gap shows what people want and cannot get.
//!
//! Every section is required, and each is either filled or explicitly marked `not_available`, so a record that leaves
//! a section out does not parse. The marker is the section's `status`, a value of its own, never an empty string; the
//! UI shows it as [`NOT_AVAILABLE`]. The agent's own words come last (0.9).
//!
//! A filled section is text for now. The structure inside a section (a diff, a flagged command) arrives with the
//! JSON-RPC that carries the record (Stage 2, Contracts).

use crate::non_empty::NonEmpty;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The words the UI shows for a section whose `status` is `not_available`.
pub const NOT_AVAILABLE: &str = "Not available in this view";

/// One section: filled with text, or marked as not available.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "status", deny_unknown_fields)]
pub enum Section {
    #[serde(rename = "filled")]
    Filled { text: NonEmpty },
    #[serde(rename = "not_available")]
    NotAvailable {},
}

/// The six sections, in the order the view shows them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ContextRecord {
    /// The rule that matched, a hook's `decision_reason`, a failed gate check, a budget or a limit.
    pub why_it_asks: Section,
    /// An edit's diff, a command's reach, a fetch's domain, an MCP call's arguments.
    pub what_it_would_do: Section,
    /// The task's title and body.
    pub task: Section,
    /// The phase and round, and what came before.
    pub what_led_here: Section,
    /// The same request in this run and this repo, and how it was answered.
    pub history: Section,
    /// The call's description and the agent's last message before asking. Unverified.
    pub agents_own_words: Section,
}
