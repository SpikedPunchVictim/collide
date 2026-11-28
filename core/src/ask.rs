//! The ask contract (0.2, The UI-tools endpoint; 0.3, The ask agent; 0.7, Test this agent; 0.11, The ask box;
//! ts-0034-hh): the three methods the window calls to start, cancel and test an ask, the events a running ask sends
//! back, the ask's outcome, and the result of Test this agent. It is types and pure checks only. The daemon that
//! answers the methods is Stage 6b's, and so is the shell code that adds the endpoint and the token.
//!
//! An ask is a request typed in the ask box. The agent reads what it needs and changes the screen through the UI
//! tools. It is not a run: it has no task, workspace or phase, and nothing resumes it (0.3).
//!
//! ## The connection rule
//!
//! **An ask is tied to the connection that started it.** The daemon cancels the ask when that connection drops, so a
//! crashed shell or a closed window leaves no model running with nobody to read it (0.2, R5). An ask's events go to
//! that connection only. The daemon builds this in Stage 6b; the rule is stated here so that the types and the daemon
//! agree on it.
//!
//! ## The methods
//!
//! They are not in [`crate::owner::Method`]: the owner protocol answers [`crate::owner::code::METHOD_NOT_FOUND`] to
//! them until the daemon takes them in Stage 6b. [`AskMethod`] is their list.
//!
//! | method | window sends | daemon receives | result |
//! |---|---|---|---|
//! | `ask.start` | [`AskStartParams`] | [`AskStartDaemonParams`] | [`crate::owner::Ack`] |
//! | `ask.cancel` | [`AskCancelParams`] | [`AskCancelParams`] | [`crate::owner::Ack`] |
//! | `ask.test` | [`AskTestParams`] | [`AskTestDaemonParams`] | [`AskTestResult`] |
//!
//! ## Two shapes, so the window never holds a token
//!
//! The window's `ask.start` and `ask.test` name the ask, the agent and (for a start) the request, and nothing else.
//! The shell makes the ask's token, registers it with its endpoint, adds the endpoint URL and the token, and forwards
//! the daemon's shape ([`AskStartDaemonParams`], [`AskTestDaemonParams`]). Each type refuses a key it does not list,
//! so a window that sends a `token`, or a daemon that gets a start with no `endpoint`, is refused.
//!
//! The ask's id is the window's, so the window can show lines and Cancel before the daemon has answered. The shell
//! refuses an id it cannot register (0.2: empty, `external`, or one already in use).
//!
//! `ask.start` answers when the daemon has accepted the ask. Everything after that, including a refusal the ask box
//! shows as plain text (`claude was not found on PATH`, `this agent is not confined; acknowledge it in Agents
//! first`), arrives as the ask's [`AskOutcome`], so the box has one place that writes a failure. A request that cannot
//! be read, or an agent id that does not exist, is an owner error of the usual codes.
//!
//! `ask.cancel` answers [`crate::owner::Ack`] for an ask that has already ended, because the window and the daemon
//! race at the end of every ask.
//!
//! `ask.test` runs the smallest ask and answers with [`AskTestResult`]. It sends no `ask.event` and no `ask.outcome`.
//!
//! ## The events
//!
//! The daemon sends [`AskNotice`]s as notifications: `ask.event` carries one normalized event of the ask's stream
//! ([`crate::agent::AgentEvent`], whole, so the ask box can write a line for each call), and `ask.outcome` carries
//! the end. No field of an event holds a token. The only places an event holds a value of any shape are the agent's
//! own tool input and the raw body of an event this version has no type for, both from [`crate::agent::AgentEvent`],
//! and [`AskNotice::scrub_token`] takes the ask's token out of them before the daemon sends the event.

use crate::agent::{AgentEvent, Money, ToolKind, UsageQuality};
use crate::non_empty::NonEmpty;
use crate::owner::{Ack, Notification};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::fmt;

/// What [`AskNotice::scrub_token`] writes in place of the token.
pub const REMOVED_TEXT: &str = "[removed]";

// ---- the methods ------------------------------------------------------------------------------------------------

/// A method of the ask contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AskMethod {
    Start,
    Cancel,
    Test,
}

impl AskMethod {
    /// Every method, in the order of the table above.
    pub const ALL: &'static [AskMethod] = &[AskMethod::Start, AskMethod::Cancel, AskMethod::Test];

    /// The name on the wire.
    pub fn name(self) -> &'static str {
        match self {
            AskMethod::Start => "ask.start",
            AskMethod::Cancel => "ask.cancel",
            AskMethod::Test => "ask.test",
        }
    }

    pub fn from_name(name: &str) -> Option<AskMethod> {
        AskMethod::ALL.iter().copied().find(|m| m.name() == name)
    }

    /// The name of the request type's schema as the window sends it.
    pub fn window_params_schema(self) -> String {
        match self {
            AskMethod::Start => <AskStartParams as JsonSchema>::schema_name(),
            AskMethod::Cancel => <AskCancelParams as JsonSchema>::schema_name(),
            AskMethod::Test => <AskTestParams as JsonSchema>::schema_name(),
        }
        .into_owned()
    }

    /// The name of the request type's schema as the daemon receives it.
    pub fn params_schema(self) -> String {
        match self {
            AskMethod::Start => <AskStartDaemonParams as JsonSchema>::schema_name(),
            AskMethod::Cancel => <AskCancelParams as JsonSchema>::schema_name(),
            AskMethod::Test => <AskTestDaemonParams as JsonSchema>::schema_name(),
        }
        .into_owned()
    }

    /// The name of the result type's schema.
    pub fn result_schema(self) -> String {
        match self {
            AskMethod::Start | AskMethod::Cancel => <Ack as JsonSchema>::schema_name(),
            AskMethod::Test => <AskTestResult as JsonSchema>::schema_name(),
        }
        .into_owned()
    }
}

/// `ask.start` params as the window sends them: the ask's id, the agent entry's id (0.7) and the request as typed.
/// No token and no endpoint: the shell adds them ([`AskStartDaemonParams`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AskStartParams {
    pub ask: NonEmpty,
    pub agent: NonEmpty,
    pub request: NonEmpty,
}

/// `ask.start` params as the shell forwards them to the daemon: the window's three fields, the endpoint URL and the
/// ask's token (0.2). The token works for this ask only and is dead when the ask ends. The daemon puts it in the
/// child's MCP configuration file of mode 0600 (Claude) or in `session/new` (ACP), never in argv or the environment.
///
/// The ask is tied to the connection that carried this request: when the connection drops, the daemon cancels the
/// ask (Stage 6b). `Debug` leaves the token out.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AskStartDaemonParams {
    pub ask: NonEmpty,
    pub agent: NonEmpty,
    pub request: NonEmpty,
    /// The URL of the shell's UI-tools endpoint.
    pub endpoint: NonEmpty,
    pub token: NonEmpty,
}

impl fmt::Debug for AskStartDaemonParams {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AskStartDaemonParams")
            .field("ask", &self.ask)
            .field("agent", &self.agent)
            .field("request", &self.request)
            .field("endpoint", &self.endpoint)
            .field("token", &"<hidden>")
            .finish()
    }
}

/// `ask.cancel` params: the ask to stop. The shell sends it for each live ask when the window closes or reloads
/// (0.2); the ask box sends it for Cancel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AskCancelParams {
    pub ask: NonEmpty,
}

/// `ask.test` params as the window sends them: an id for the test's ask, and the agent entry to test (0.7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AskTestParams {
    pub ask: NonEmpty,
    pub agent: NonEmpty,
}

/// `ask.test` params as the shell forwards them: the test is an ask like any other, so the daemon needs the endpoint
/// and the ask's token to start the child. The same rules as [`AskStartDaemonParams`] hold, and `Debug` leaves the
/// token out.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AskTestDaemonParams {
    pub ask: NonEmpty,
    pub agent: NonEmpty,
    pub endpoint: NonEmpty,
    pub token: NonEmpty,
}

impl fmt::Debug for AskTestDaemonParams {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AskTestDaemonParams")
            .field("ask", &self.ask)
            .field("agent", &self.agent)
            .field("endpoint", &self.endpoint)
            .field("token", &"<hidden>")
            .finish()
    }
}

// ---- the outcome ------------------------------------------------------------------------------------------------

/// What an ask cost. A cost that the agent did not report is [`AskCost::NotReported`], never 0: an ACP agent reports
/// no cost (0.3, measured S6), and 0 would read as free.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum AskCost {
    Reported { cost: Money },
    NotReported {},
}

/// How an ask ended (0.11, The ask box). The ask box writes the plain text of each from these: `timed out after N
/// s`, `stopped at the $X budget`, `cancelled`, `agent error: <text>`, `the ask was lost; ask again`. A refusal
/// before launch is `failed` with its text.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum AskEnd {
    /// The agent's turn ended. Whether it changed the screen is the window's to say: it ran the calls.
    Finished {},
    /// Stopped on request, or because the connection that started the ask dropped.
    Cancelled {},
    /// Collide's timeout stopped the child (0.3, Budget and timeout). `after_s` is the timeout that applied.
    TimedOut { after_s: u64 },
    /// Claude Code's `--max-budget-usd` stopped it. Only Claude Code enforces a budget (0.3).
    StoppedAtBudget { budget: Money },
    /// The ask did not run or did not finish. `text` is plain text for the ask box.
    Failed { text: String },
    /// The ask's shim died with no word of how the child ended, or the daemon restarted under it. The ask box says
    /// `the ask was lost; ask again` (0.3, The ask agent is a separate role).
    Lost {},
}

/// The outcome of an ask: how it ended, what it cost, and how far the figures of the usage can be trusted
/// ([`UsageQuality`], 0.3). The ask's spend is shown on the Agents page and is never added to a run (0.3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AskOutcome {
    pub end: AskEnd,
    pub cost: AskCost,
    pub quality: UsageQuality,
}

// ---- the events -------------------------------------------------------------------------------------------------

/// The body of `ask.event`: one normalized event of the ask's stream. It has no field for a token.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AskEventNotice {
    pub ask: NonEmpty,
    pub event: AgentEvent,
}

/// The body of `ask.outcome`: the last message of an ask.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AskOutcomeNotice {
    pub ask: NonEmpty,
    pub outcome: AskOutcome,
}

/// What the daemon sends for a running ask, to the connection that started it. On the wire a notice is a
/// [`Notification`] whose `method` is the name below and whose `params` is the payload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "method", content = "params", deny_unknown_fields)]
pub enum AskNotice {
    /// One normalized event of the ask's stream.
    #[serde(rename = "ask.event")]
    Event(Box<AskEventNotice>),
    /// The ask ended. Nothing follows it for that ask.
    #[serde(rename = "ask.outcome")]
    Outcome(Box<AskOutcomeNotice>),
}

impl AskNotice {
    /// The method of an [`AskNotice::Event`].
    pub const EVENT_METHOD: &'static str = "ask.event";
    /// The method of an [`AskNotice::Outcome`].
    pub const OUTCOME_METHOD: &'static str = "ask.outcome";

    pub fn method(&self) -> &'static str {
        match self {
            AskNotice::Event(_) => AskNotice::EVENT_METHOD,
            AskNotice::Outcome(_) => AskNotice::OUTCOME_METHOD,
        }
    }

    pub fn to_notification(&self) -> Notification {
        let mut value = serde_json::to_value(self).expect("a notice is JSON");
        let params = value["params"].take();
        Notification {
            jsonrpc: Default::default(),
            method: self.method().to_string(),
            params,
        }
    }

    /// Reads a notification as a notice. An unknown method, or params the notice's type does not take, is an error.
    pub fn from_notification(notification: &Notification) -> Result<AskNotice, serde_json::Error> {
        serde_json::from_value(serde_json::json!({
            "method": notification.method,
            "params": notification.params,
        }))
    }

    /// Replaces the ask's token with [`REMOVED_TEXT`] in every string of an `ask.event`, keys of an object included.
    /// The daemon calls it on each event before it sends or stores it, because the agent's tool input is free-form
    /// and an agent could echo a token it was given. An `ask.outcome` holds no free-form value and is left alone.
    pub fn scrub_token(&mut self, token: &NonEmpty) {
        let AskNotice::Event(notice) = self else {
            return;
        };
        let written = serde_json::to_value(&notice.event).expect("a normalized event is JSON");
        let scrubbed = scrub_value(written, token.as_str());
        notice.event = serde_json::from_value(scrubbed)
            .expect("an event with some text replaced is still a normalized event");
    }
}

fn scrub_value(value: Value, token: &str) -> Value {
    match value {
        Value::String(text) => Value::String(text.replace(token, REMOVED_TEXT)),
        Value::Array(items) => Value::Array(
            items
                .into_iter()
                .map(|item| scrub_value(item, token))
                .collect(),
        ),
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(key, item)| (key.replace(token, REMOVED_TEXT), scrub_value(item, token)))
                .collect::<Map<String, Value>>(),
        ),
        other => other,
    }
}

// ---- Test this agent --------------------------------------------------------------------------------------------

/// The confinement probe of Test this agent (0.7, R5; 0.3). The test asks the agent to write a file and to run `true`
/// in a temporary directory made for it, and checks whether either happened. `red` means one did: the agent is not
/// confined, and is refused as an ask agent until the person acknowledges it. `not tested` is an entry the probe has
/// not run for, and is never read as `green`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum ConfinementProbe {
    #[serde(rename = "red")]
    Red,
    #[serde(rename = "green")]
    Green,
    #[serde(rename = "not tested")]
    NotTested,
}

/// `ask.test` result: what Test this agent shows (0.7). A time or failure that does not apply is left out, never 0
/// or empty text.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AskTestResult {
    /// Whether the endpoint was reached with the ask's token. The daemon sets it from the stream (a tool call that
    /// the endpoint did not refuse); the shell, which sees the endpoint's requests, may replace it.
    pub endpoint_reached: bool,
    /// Milliseconds from the start to the first tool call. Absent when the agent made none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_tool_call_ms: Option<u64>,
    /// Milliseconds from the start to the end of the test. Absent when it did not end.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finish_ms: Option<u64>,
    /// The cost (Claude), or "not reported" (ACP).
    pub cost: AskCost,
    /// The folders the agent can read, as the entry names them.
    pub readable_folders: Vec<String>,
    /// The tool kinds in effect: `read` and `search` in v0 (0.7).
    pub tool_kinds: Vec<ToolKind>,
    /// What went wrong, in plain text, such as `claude was not found on PATH` or `the agent did not call a tool
    /// within 180 s`. Absent when nothing did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure: Option<String>,
    pub probe: ConfinementProbe,
}
