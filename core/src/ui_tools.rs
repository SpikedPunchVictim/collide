//! The shell's `ui_tools.*` contract (0.2, The UI-tools endpoint; 0.11, The five tools; ts-0035-re): the three methods
//! the window calls on the shell, the event the shell sends the window, and the file format of the tool manifest.
//! It is types and pure checks only. The handler that answers an agent's HTTP request is `host`'s (`ui_tools`
//! module), and the listener is Stage 6b's.
//!
//! ## The shell handles these itself
//!
//! `ui_tools.reply`, `ui_tools.token` and `ui_tools.rotate` come from the window over the same `rpc` command as the
//! owner's methods, and the shell answers them and does not forward them to the daemon, so they are not in
//! [`crate::owner::Method`] (the daemon answers [`crate::owner::code::METHOD_NOT_FOUND`] to them). [`UiToolsMethod`]
//! is their list.
//!
//! | method | params | result | what it does |
//! |---|---|---|---|
//! | `ui_tools.reply` | [`UiToolsReplyParams`] | [`crate::owner::Ack`] | the window's answer to a [`UiToolsRequest`] |
//! | `ui_tools.token` | [`UiToolsTokenParams`] | [`UiToolsTokenResult`] | the endpoint token, for Settings to show |
//! | `ui_tools.rotate` | [`UiToolsRotateParams`] | [`UiToolsRotateResult`] | a new endpoint token, which ends every client of the old one |
//!
//! ## The event
//!
//! `ui_tools.request` ([`UiToolsRequest`]) is sent to the window for each `tools/call` an agent makes. It holds the
//! agent's `tools/call` message as it came, which is untrusted (0.9): the window validates it.
//!
//! ## The manifest
//!
//! The shell answers `tools/list` from a file generated at build time from the window's TypeScript tool definitions
//! (0.11), so a client can list the tools with the window closed. The file is a JSON array of [`ToolDefinition`], the
//! entries MCP's `tools/list` returns, and [`ToolManifest`] is its type and schema. The generator is Stage 6's.

use crate::owner::Ack;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;
use std::fmt;

/// The name of the event the shell sends the window, as the plan writes it.
pub const REQUEST_EVENT: &str = "ui_tools.request";

/// How long the shell waits for the window to answer a [`UiToolsRequest`], in seconds (0.2).
pub const REQUEST_DEADLINE_SECS: u64 = 10;

/// How many screen-changing calls one ask may make (0.9). The shell counts them, so a window reload cannot reset the
/// count.
pub const MAX_SCREEN_CHANGING_CALLS: u32 = 20;

/// What a `tools/call` fails with when no window is open (0.2, A closed or silent window).
pub const NO_WINDOW_TEXT: &str = "Collide has no window open; open Collide and ask again";

/// What a `tools/call` fails with when the window does not answer in [`REQUEST_DEADLINE_SECS`] (0.2).
pub const TIMEOUT_TEXT: &str = "the Collide window did not answer within 10 s";

/// The `caller` of a [`UiToolsRequest`] that is not an ask: a client of the endpoint token (0.7). An ask's id never
/// equals it.
pub const EXTERNAL_CALLER: &str = "external";

// ---- the methods ------------------------------------------------------------------------------------------------

/// A method the window calls on the shell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiToolsMethod {
    Reply,
    Token,
    Rotate,
}

impl UiToolsMethod {
    /// Every method, in the order of the table above.
    pub const ALL: &'static [UiToolsMethod] = &[
        UiToolsMethod::Reply,
        UiToolsMethod::Token,
        UiToolsMethod::Rotate,
    ];

    /// The name on the wire.
    pub fn name(self) -> &'static str {
        match self {
            UiToolsMethod::Reply => "ui_tools.reply",
            UiToolsMethod::Token => "ui_tools.token",
            UiToolsMethod::Rotate => "ui_tools.rotate",
        }
    }

    pub fn from_name(name: &str) -> Option<UiToolsMethod> {
        UiToolsMethod::ALL
            .iter()
            .copied()
            .find(|m| m.name() == name)
    }

    /// The name of the request type's schema.
    pub fn params_schema(self) -> String {
        match self {
            UiToolsMethod::Reply => <UiToolsReplyParams as JsonSchema>::schema_name(),
            UiToolsMethod::Token => <UiToolsTokenParams as JsonSchema>::schema_name(),
            UiToolsMethod::Rotate => <UiToolsRotateParams as JsonSchema>::schema_name(),
        }
        .into_owned()
    }

    /// The name of the result type's schema.
    pub fn result_schema(self) -> String {
        match self {
            UiToolsMethod::Reply => <Ack as JsonSchema>::schema_name(),
            UiToolsMethod::Token => <UiToolsTokenResult as JsonSchema>::schema_name(),
            UiToolsMethod::Rotate => <UiToolsRotateResult as JsonSchema>::schema_name(),
        }
        .into_owned()
    }
}

/// One piece of a tool's answer. MCP has more kinds; the five tools answer in text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum ToolContent {
    #[serde(rename = "text")]
    Text { text: String },
}

/// `ui_tools.reply` params: the window's answer to the [`UiToolsRequest`] with this `requestId`, in the shape of an MCP
/// tool result. `isError` is true when the tool refused the call; the text then says what to do (0.9). A reply to a
/// request the shell has dropped is ignored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiToolsReplyParams {
    pub request_id: String,
    pub content: Vec<ToolContent>,
    #[serde(default)]
    pub is_error: bool,
}

/// `ui_tools.token` params: none.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UiToolsTokenParams {}

/// `ui_tools.token` result: the endpoint token (0.2), for Settings to show.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UiToolsTokenResult {
    pub token: String,
}

/// `ui_tools.rotate` params: none.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UiToolsRotateParams {}

/// `ui_tools.rotate` result: the new endpoint token. The old one stops working at once.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UiToolsRotateResult {
    pub token: String,
}

// ---- the event --------------------------------------------------------------------------------------------------

/// The event `ui_tools.request` (0.2): an agent's `tools/call`, for the window to run against its own state and answer
/// with `ui_tools.reply`. No field holds a token.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiToolsRequest {
    /// Names this request in the window's `ui_tools.reply`.
    pub request_id: String,
    /// The ask's id, or [`EXTERNAL_CALLER`].
    pub caller: String,
    /// When the shell gives up and answers the agent that the window did not answer: Unix milliseconds on the wall
    /// clock, so the window can skip a request it cannot answer in time. The shell's own timeout runs on the uptime
    /// clock (0.2, Clocks) and does not depend on this value.
    pub deadline: u64,
    /// The `tools/call` message as the agent sent it. It is untrusted (0.9).
    pub message: Value,
}

// ---- the manifest -----------------------------------------------------------------------------------------------

/// MCP's tool annotations, of which the manifest takes one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolAnnotations {
    /// True for a tool that changes nothing on screen. The shell does not count its calls against
    /// [`MAX_SCREEN_CHANGING_CALLS`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub read_only_hint: Option<bool>,
}

/// One tool of the manifest: what MCP's `tools/list` returns for it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolDefinition {
    #[schemars(length(min = 1))]
    pub name: String,
    pub description: String,
    /// A JSON Schema for the tool's arguments, whose `type` is `object` ([`ToolManifest::check`]).
    #[schemars(schema_with = "object_schema")]
    pub input_schema: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub annotations: Option<ToolAnnotations>,
}

fn object_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({ "type": "object" })
}

/// The tool manifest: a JSON array of [`ToolDefinition`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ToolManifest(pub Vec<ToolDefinition>);

/// What a manifest can be wrong in that its schema does not say.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestProblem {
    EmptyName,
    DuplicateName(String),
    InputSchemaNotAnObject(String),
}

impl fmt::Display for ManifestProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ManifestProblem::EmptyName => write!(f, "a tool of the manifest has an empty name"),
            ManifestProblem::DuplicateName(name) => {
                write!(f, "the manifest names the tool `{name}` twice")
            }
            ManifestProblem::InputSchemaNotAnObject(name) => write!(
                f,
                "the inputSchema of the tool `{name}` must be a JSON Schema whose type is `object`"
            ),
        }
    }
}

impl std::error::Error for ManifestProblem {}

impl ToolManifest {
    /// Reads manifest text. The error says what is wrong and that it is the tool manifest.
    pub fn from_json(text: &str) -> Result<ToolManifest, String> {
        serde_json::from_str(text).map_err(|error| format!("not a tool manifest: {error}"))
    }

    /// The rules a schema cannot say: a name is not empty and appears once, and an `inputSchema` is an object with
    /// `type: object`.
    pub fn check(&self) -> Result<(), ManifestProblem> {
        let mut seen = BTreeSet::new();
        for tool in &self.0 {
            if tool.name.is_empty() {
                return Err(ManifestProblem::EmptyName);
            }
            if !seen.insert(tool.name.as_str()) {
                return Err(ManifestProblem::DuplicateName(tool.name.clone()));
            }
            if tool.input_schema.get("type") != Some(&Value::String("object".to_string())) {
                return Err(ManifestProblem::InputSchemaNotAnObject(tool.name.clone()));
            }
        }
        Ok(())
    }

    /// Whether the manifest marks `name` read-only. A tool the manifest does not hold is not, so a call to it counts
    /// against the cap.
    pub fn is_read_only(&self, name: &str) -> bool {
        self.0.iter().any(|tool| {
            tool.name == name
                && tool
                    .annotations
                    .as_ref()
                    .and_then(|a| a.read_only_hint)
                    .unwrap_or(false)
        })
    }
}
