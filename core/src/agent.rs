//! The normalized agent events and the capability table (0.3, Agents and model sources). An agent tool's own stream
//! (Claude's stream-json, ACP's JSON-RPC) is turned into [`AgentEvent`]s by an adapter, and the rest of Collide reads
//! only these. The traits that produce them (`AgentAdapter`, `AgentSession`) are in [`crate::adapter`]; this module
//! holds the data only, and names no process, Tauri or SQLite type.
//!
//! | what | type |
//! |---|---|
//! | one event of a turn | [`AgentEvent`] |
//! | how a session starts | [`SessionStart`] |
//! | a request to run a tool | [`PermissionRequest`] |
//! | the answer to it | [`PermissionReply`] |
//! | what a turn spent | [`Usage`] |
//! | what an adapter can do | [`Capabilities`] |
//! | the model a role runs on | [`ModelRef`] |
//! | the user's aliases for it | [`UserModels`] |
//!
//! The wire form of every tagged type is an object with a `type` key in `snake_case`. A number that an agent does
//! not report is absent, never 0.

use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

/// How a session starts (0.3). Claude supports `assign` and `resume`; ACP supports `agent_assigned` and `resume`.
/// The id is then recorded on [`AgentEvent::SessionBound`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum SessionStart {
    /// Collide picks the session id (a UUID) and passes it to the tool.
    Assign { uuid: String },
    /// The tool picks the id and reports it.
    AgentAssigned {},
    /// Continue the session with this id.
    Resume { id: String },
}

/// The model a role runs on, as one mapping of `models.yaml` gives it (0.3, Who picks what). `name` is the safe
/// label that reaches env values, paths, arguments and a pack's `requires.modelName`. `id` is opaque: only the
/// adapter reads it, and it never reaches env, paths or argv, because an ACP model id such as `ollama/qwen2.5:7b`
/// fails the safe token. When there is no `id`, the adapter's id is the `name` (Claude: `model.id` is `model.name`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ModelRef {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

impl ModelRef {
    /// The id the adapter hands to its tool: `id` when the mapping names one, else `name`.
    pub fn adapter_id(&self) -> &str {
        self.id.as_deref().unwrap_or(&self.name)
    }
}

/// `~/.config/collide/models.yaml` (0.3, Who picks what): for each tool, the aliases a pack's roles name (`fast`,
/// `strong`) and the model each one maps to. An alias never switches the tool, so the table is keyed by tool first. A
/// tool or an alias that is not here is unmapped, and launch refuses a role that names it. The model source and the
/// account stay the user's own.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UserModels {
    #[serde(default)]
    pub tools: BTreeMap<String, BTreeMap<String, ModelRef>>,
}

impl UserModels {
    /// The aliases mapped for `tool`: empty when the user mapped none.
    pub fn mapping_for(&self, tool: &str) -> BTreeMap<String, ModelRef> {
        self.tools.get(tool).cloned().unwrap_or_default()
    }
}

/// What a tool call does, as the agent classifies it. Adapters classify by `kind` and input, never by a call's title
/// (S7, O12).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ToolKind {
    Read,
    Edit,
    Delete,
    Move,
    Search,
    Execute,
    Think,
    Fetch,
    Other,
}

/// The kind of one of the agent's own options in a permission request. Collide picks only the `*_once` kinds: the
/// agent owns the scope and storage of an "always" choice, so no adapter selects one (0.3, Permissions).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PermissionOptionKind {
    AllowOnce,
    AllowAlways,
    RejectOnce,
    RejectAlways,
}

/// One of the agent's own options in a permission request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PermissionOption {
    pub id: String,
    pub name: String,
    pub kind: PermissionOptionKind,
}

/// A request to run a tool (0.3, Permissions). `input` is `None` when the agent has not sent it yet (opencode sends
/// `rawInput: {}` for a Bash call, so the command is unknown until the host answers; S7). Such a request shows in the
/// Queue as "input not available until approved" and no pack rule that reads the input may auto-allow it.
/// `reason` says why it was asked, when the agent says. `options` are the agent's own.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PermissionRequest {
    pub id: String,
    pub tool_call_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    pub kind: ToolKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default)]
    pub options: Vec<PermissionOption>,
}

/// The answer to a [`PermissionRequest`] (0.3, Permissions). Claude maps `allow` and `deny` to `behavior`, with the
/// message. ACP maps them to the options of kind `allow_once` and `reject_once`, drops the message and, when no
/// `allow_once` exists, answers `cancel` and reports [`AgentEvent::PermissionUnanswerable`].
///
/// The event log's `permission.answered` carries the shorter [`crate::events::PermissionAnswer`], which has no
/// message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum PermissionReply {
    Allow {},
    Deny {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        message: Option<String>,
    },
    Cancel {},
}

/// What a [`Usage`] event counts: one turn, one model step, the session so far, a compaction call, or another model call
/// outside the turn's own (0.3, Usage).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum UsageScope {
    Turn,
    Step,
    RunningTotal,
    /// The model call of a compaction. A turn's own usage leaves it out, so it is its own event, never added into a
    /// `Turn` (0.3, Usage; S9).
    Compaction,
    /// A model call that is neither the turn's own nor a compaction: a helper-model call, read as what the agent
    /// reports for the turn beyond the turn's own usage. Never added into a `Turn` either.
    OtherCalls,
}

/// How far the figures of a [`Usage`] event can be trusted. A total built from `partial` figures is shown as "at
/// least" and never summed with `exact` ones (0.13).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum UsageQuality {
    Exact,
    Partial,
    Unreported,
}

/// An amount in a currency. Claude reports USD.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Money {
    pub amount: f64,
    pub currency: String,
}

/// What an agent spent (0.3, Usage). Every number is an `Option`: a figure the agent did not report is `None`, never
/// 0, and is left out of the JSON. `cache_write_1h` and `cache_write_5m` split `cache_write` by tier, where the tool
/// reports the tiers (capability `usage_report.cache_write_tiers`). `model_time_ms` is the time the model call or calls
/// took, when the agent reports it (0.13: a turn's is a difference of a running total).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Usage {
    pub scope: UsageScope,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_read: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_write: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_write_1h: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_write_5m: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thought: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_used: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_size: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost: Option<Money>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_time_ms: Option<u64>,
    pub quality: UsageQuality,
}

/// Why a turn ended (0.3, A turn ends with). `error` carries the HTTP status of an API failure when there is one
/// (S1 measured a 429). `blocked` is a prompt a `UserPromptSubmit` hook refused: the turn never ran, and `reason` is
/// the hook's own, when it gave one (S9). The result that follows it is the hook's message, never the phase's report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum TurnEndReason {
    EndTurn {},
    MaxTokens {},
    MaxTurns {},
    Refusal {},
    Cancelled {},
    Error {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        status: Option<u16>,
    },
    Blocked {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },
}

/// How a hook call ended, as the agent reports it (S9: `success`, `error` for an exit other than 0, `cancelled` for a
/// timeout).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HookOutcome {
    Success,
    Error,
    Cancelled,
}

/// What a hook's own output decided, read from the `decision` it printed (0.3, The stream protocol). Only this word is
/// kept: the output itself may hold secrets. `block` is a `PostToolUse` hook's, which leaves the tool's result as it was.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HookDecision {
    Allow,
    Deny,
    Ask,
    Block,
}

/// How a hook that was meant to guard a tool call failed without stopping it (0.13, the failed-open detector).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HookFailure {
    /// The hook exited with a code that is neither 0 nor the deliberate block, 2 (S9: exit 1).
    ExitError,
    /// The hook timed out, so it gave no verdict.
    NoVerdict,
}

/// Who refused a tool call, or why it did not run (0.3, The stream protocol). The adapter reads it from the tool's own
/// metadata and never from the result's text. `unknown` is any other shape of refusal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RefusalCause {
    /// A hook denied it.
    Hook,
    /// Collide's own `deny` answer.
    Host,
    /// A permission rule, or an automatic deny.
    Rule,
    /// The session does not have the tool.
    MissingTool,
    Unknown,
}

/// Where a [`AgentEvent::FinalReport`] came from: Claude's `result` field, or, for ACP, the message chunks after the
/// last tool call.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FinalReportSource {
    ResultField,
    Reconstructed,
}

/// The `type` of every event but one that [`AgentEvent`] knows, as written in JSON. A test pins this list to the
/// schema. A `type` outside it reads as [`AgentEvent::Other`].
const KNOWN_EVENT_TYPES: [&str; 14] = [
    "message",
    "tool_call",
    "permission_request",
    "permission_unanswerable",
    "session_bound",
    "usage",
    "turn_end",
    "final_report",
    "limited",
    "hook_call",
    "hook_failed_open",
    "tool_refused",
    "tool_result_size",
    "other",
];

/// One normalized event of an agent session (0.3, Normalized events).
///
/// Every known event is a closed object: an unknown key is an error. An event whose `type` this version does not
/// know (a tool's newer stream, an event of a later Collide) reads as [`AgentEvent::Other`] with that name in
/// `raw_type` and the rest of the object in `raw`, because a log line must stay readable. `Other` is also what an
/// adapter emits for a stream event it does not map. It is written back as `{"type":"other","raw_type":...}`, so a
/// read-write-read is stable. The schema lists the known events only.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(
    remote = "Self",
    tag = "type",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum AgentEvent {
    /// A message of the agent, or one chunk of it.
    Message {
        text: String,
    },
    /// A tool call. `input` is `None` while the agent has not sent it.
    ToolCall {
        id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tool: Option<String>,
        kind: ToolKind,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        input: Option<Value>,
    },
    PermissionRequest {
        request: PermissionRequest,
    },
    /// The request had no option Collide may pick, so the session answered `cancel` (0.3, Permissions).
    PermissionUnanswerable {
        request_id: String,
    },
    /// The session id is known: at once for Claude, on the `session/new` response for ACP.
    SessionBound {
        id: String,
    },
    Usage {
        usage: Usage,
    },
    /// `empty` is true when the turn gave no message, no tool call and zero or unreported usage.
    TurnEnd {
        reason: TurnEndReason,
        cancel_requested: bool,
        empty: bool,
    },
    FinalReport {
        text: String,
        source: FinalReportSource,
    },
    /// The account's usage limit is reached (0.3, Usage and rate limits). `reset_at_ms` is the wall-clock time it
    /// resets, in milliseconds since the Unix epoch, and is absent when the tool does not say. Only an adapter with
    /// `limit_signal` reports it.
    Limited {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reset_at_ms: Option<u64>,
    },
    /// One hook call: its name (`PreToolUse:Bash`), the event it hooked (`PreToolUse`), how it ended, its exit code and
    /// the `decision` its output gave, when it gave one. The hook's `stdout` and `stderr` are untrusted text that may
    /// hold secrets, so the event has no place for them (0.3, The stream protocol).
    HookCall {
        name: String,
        event: String,
        outcome: HookOutcome,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        exit_code: Option<i32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        decision: Option<HookDecision>,
    },
    /// A tool hook failed (`cause`) and the call it hooked, `tool_call_id`, ran anyway: a guard that did not guard
    /// (0.13). It is what a session can know about the detector; the finding, with its cost, is the metrics item's.
    HookFailedOpen {
        name: String,
        event: String,
        tool_call_id: String,
        cause: HookFailure,
    },
    /// A tool call that did not run, by the id of the [`AgentEvent::ToolCall`], and who refused it.
    ToolRefused {
        id: String,
        cause: RefusalCause,
    },
    /// The size in bytes of a tool call's result, by the id of the [`AgentEvent::ToolCall`] (0.13, the token and context
    /// metrics). It is given for every result, a refused call's included. The result's content is untrusted and may be
    /// large, so the event holds the size and nothing else.
    ToolResultSize {
        tool_call_id: String,
        bytes: u64,
    },
    /// An event this version has no type for. `raw_type` is its own type name.
    Other {
        raw_type: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        raw: Option<Value>,
    },
}

impl Serialize for AgentEvent {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        Self::serialize(self, serializer)
    }
}

impl<'de> Deserialize<'de> for AgentEvent {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::Error;
        let value = Value::deserialize(deserializer)?;
        let tag = match value.get("type") {
            Some(Value::String(tag)) => tag.clone(),
            Some(_) => return Err(D::Error::custom("event `type` must be a string")),
            None if value.is_object() => return Err(D::Error::custom("event has no `type`")),
            None => return Err(D::Error::custom("event must be an object")),
        };
        if KNOWN_EVENT_TYPES.contains(&tag.as_str()) {
            return Self::deserialize(value).map_err(D::Error::custom);
        }
        let mut rest: Map<String, Value> = match value {
            Value::Object(map) => map,
            _ => unreachable!("a value with a `type` key is an object"),
        };
        rest.remove("type");
        Ok(AgentEvent::Other {
            raw_type: tag,
            raw: Some(Value::Object(rest)),
        })
    }
}

/// Whether an adapter can do something (0.3, Capabilities). A capability nobody has set is `unknown` (a profile for
/// an ACP agent that has not been probed). Collide refuses to start a phase that requires a capability on a tool that
/// says `no` or `unknown`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Support {
    Yes,
    No,
    #[default]
    Unknown,
}

/// The host answers permission requests, and what such an answer can carry.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct PermissionHost {
    pub support: Support,
    /// A `deny` can carry a message.
    pub deny_message: Support,
    /// The request carries the tool's input, not only a call id.
    pub input_at_request: Support,
    /// The request says why it was asked.
    pub request_reason: Support,
}

/// How a session resumes: `none`, `replay` (the agent replays its history as updates) or `silent`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SessionResume {
    #[default]
    Unknown,
    None,
    Replay,
    Silent,
}

/// How a role's system prompt reaches the tool: `none`, `append` or `override`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SystemPrompt {
    #[default]
    Unknown,
    None,
    Append,
    Override,
}

/// What the tool's usage reports give. `granularity` lists the scopes its [`Usage`] events come in (empty when not
/// declared). `quality` is the quality its figures usually have (`None` when not declared).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct UsageReport {
    pub support: Support,
    pub granularity: Vec<UsageScope>,
    /// A cost with its currency. A host-enforced budget needs it (0.3).
    pub cost: Support,
    pub cache_write_tiers: Support,
    /// `context_used` and `context_size`.
    pub context: Support,
    pub quality: Option<UsageQuality>,
}

/// Which context options the tool supports (0.3, Context and cache).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct ContextOptions {
    pub support: Support,
    pub compact_at: Support,
    pub bash_output_max: Support,
    pub mcp_output_max: Support,
}

/// Where the model is chosen: `launch` (on the command line), `session` (after the session is created) or `none`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ModelSelect {
    #[default]
    Unknown,
    None,
    Launch,
    Session,
}

/// Which MCP transports the tool takes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct McpServers {
    pub support: Support,
    pub stdio: Support,
    pub http: Support,
    pub sse: Support,
}

/// The names of the 18 capabilities of 0.3, as they are keys of [`Capabilities`] and as a role's `require` names
/// them.
pub const CAPABILITY_NAMES: [&str; 18] = [
    "headless_stream",
    "permission_host",
    "session_resume",
    "background",
    "budget_native",
    "system_prompt",
    "tool_allowlist",
    "named_agent",
    "usage_report",
    "limit_signal",
    "compaction_events",
    "hook_events",
    "refusal_source",
    "strict_config",
    "context_options",
    "model_select",
    "take_over",
    "mcp_servers",
];

/// The capability table of an adapter (0.3, Capabilities). `Capabilities::default()` is all `unknown`, and a key
/// that is left out reads as `unknown`. Every key is written.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Capabilities {
    /// The host reads a stream the tool writes without a terminal.
    pub headless_stream: Support,
    pub permission_host: PermissionHost,
    pub session_resume: SessionResume,
    /// The tool can run detached. Claude has it; ACP does not.
    pub background: Support,
    /// The tool enforces a budget itself (Claude). Any adapter with `usage_report.cost` can have the core enforce
    /// one instead.
    pub budget_native: Support,
    pub system_prompt: SystemPrompt,
    /// The tool takes a list of the tools an agent may use.
    pub tool_allowlist: Support,
    /// The tool takes a named agent definition (Claude's `--agents`).
    pub named_agent: Support,
    pub usage_report: UsageReport,
    /// A structured usage limit with its reset time.
    pub limit_signal: Support,
    /// The tool reports compactions.
    pub compaction_events: Support,
    /// Hook outcomes appear in the stream.
    pub hook_events: Support,
    /// A refused call says who refused it.
    pub refusal_source: Support,
    /// The tool can run with nothing from the worktree or the user's agent config.
    pub strict_config: Support,
    pub context_options: ContextOptions,
    pub model_select: ModelSelect,
    /// The tool can name an interactive command for a session.
    pub take_over: Support,
    pub mcp_servers: McpServers,
}

impl Capabilities {
    /// What the adapter says about one capability, by the name a role's `require` writes: a name of
    /// [`CAPABILITY_NAMES`], or a name with a sub-flag (`permission_host.deny_message`). A capability that has a
    /// sub-flag or a mode reads as `Yes` when the tool has any (`session_resume` is `Yes` for `replay` and `silent`,
    /// and `usage_report` and `context_options` read their `support`). `None` is a name the table does not have.
    pub fn support_of(&self, name: &str) -> Option<Support> {
        fn mode(has_it: bool, says_none: bool) -> Support {
            match (has_it, says_none) {
                (true, _) => Support::Yes,
                (false, true) => Support::No,
                (false, false) => Support::Unknown,
            }
        }
        Some(match name {
            "headless_stream" => self.headless_stream,
            "permission_host" | "permission_host.support" => self.permission_host.support,
            "permission_host.deny_message" => self.permission_host.deny_message,
            "permission_host.input_at_request" => self.permission_host.input_at_request,
            "permission_host.request_reason" => self.permission_host.request_reason,
            "session_resume" => mode(
                matches!(
                    self.session_resume,
                    SessionResume::Replay | SessionResume::Silent
                ),
                self.session_resume == SessionResume::None,
            ),
            "background" => self.background,
            "budget_native" => self.budget_native,
            "system_prompt" => mode(
                matches!(
                    self.system_prompt,
                    SystemPrompt::Append | SystemPrompt::Override
                ),
                self.system_prompt == SystemPrompt::None,
            ),
            "tool_allowlist" => self.tool_allowlist,
            "named_agent" => self.named_agent,
            "usage_report" | "usage_report.support" => self.usage_report.support,
            "usage_report.cost" => self.usage_report.cost,
            "usage_report.cache_write_tiers" => self.usage_report.cache_write_tiers,
            "usage_report.context" => self.usage_report.context,
            "limit_signal" => self.limit_signal,
            "compaction_events" => self.compaction_events,
            "hook_events" => self.hook_events,
            "refusal_source" => self.refusal_source,
            "strict_config" => self.strict_config,
            "context_options" | "context_options.support" => self.context_options.support,
            "context_options.compact_at" => self.context_options.compact_at,
            "context_options.bash_output_max" => self.context_options.bash_output_max,
            "context_options.mcp_output_max" => self.context_options.mcp_output_max,
            "model_select" => mode(
                matches!(
                    self.model_select,
                    ModelSelect::Launch | ModelSelect::Session
                ),
                self.model_select == ModelSelect::None,
            ),
            "take_over" => self.take_over,
            "mcp_servers" | "mcp_servers.support" => self.mcp_servers.support,
            "mcp_servers.stdio" => self.mcp_servers.stdio,
            "mcp_servers.http" => self.mcp_servers.http,
            "mcp_servers.sse" => self.mcp_servers.sse,
            _ => return None,
        })
    }
}
