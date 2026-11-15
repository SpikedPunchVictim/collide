//! The owner's JSON-RPC (0.2, 0.11; ts-0032-pp): the contract between a client (the app, the CLI) and
//! `collide-daemon`. It is types and pure checks only. The socket server that carries it is Stage 3a's.
//!
//! ## The wire
//!
//! JSON-RPC 2.0, one message per line (newline-delimited JSON), at most [`MAX_MESSAGE_BYTES`] bytes. A line that is
//! not JSON answers [`code::PARSE_ERROR`] with a null id; JSON that is not a request answers
//! [`code::INVALID_REQUEST`]. A request is [`Request`]; its answer is [`Response`]; a message with no `id` that
//! comes from the daemon is a [`Notification`], and its body is one of the [`Notice`]s.
//!
//! ## The handshake
//!
//! The first request of a connection is `initialize` ([`OwnerInitializeParams`]). The daemon checks the per-client
//! token first and the protocol version second ([`check_initialize`]), so a peer that is not authenticated learns no
//! version. A version that is not [`OWNER_PROTOCOL`] is refused with a message that names both versions. A method
//! sent before `initialize` answers [`code::NOT_INITIALIZED`]. The result names the actor the daemon recorded for the
//! connection ([`OwnerInitializeResult::actor`]).
//!
//! ## The actor is never a parameter
//!
//! The daemon decides the actor from the connection (0.2, Who is asking) and records it on every event. Every
//! request type has `deny_unknown_fields`, so a request with an `actor` key is refused, and [`route`] says why.
//!
//! ## Events
//!
//! The daemon's events come as `daemon.event` notifications whose params are an [`crate::events::Event`]: they carry
//! `seq` and the `actor` the daemon recorded. `daemon.subscribe` starts them, from a sequence number the client
//! names. Three other notices carry what is not an event of the store: `run.output`, `run.terminal_output` and
//! `queue.changed`.
//!
//! ## Error codes
//!
//! [`CODES`] is the one table of them. The standard ones are JSON-RPC's; the owner's are in the server range.
//!
//! ## Methods
//!
//! [`Method`] is the one list of methods, with each method's family, what it serves (a CLI command or a section of
//! the plan), and the names of its request and result schemas, which are registered in [`crate::schema`]. The method
//! list is Milestones A and B's. `ask.*` is ts-0034-hh's and `ui_tools.*` is ts-0035-re's; both answer
//! [`code::METHOD_NOT_FOUND`] here.
//!
//! | family | module | methods |
//! |---|---|---|
//! | daemon | [`daemon`] | `daemon.status`, `daemon.recapture_environment`, `daemon.subscribe` |
//! | board | [`board`] | `board.list`, `board.open`, `board.init`, `board.trust` |
//! | task | [`task`] | `task.list`, `task.get`, `task.apply`, `task.preview`, `task.views`, `task.save_view`, `task.sections` |
//! | run | [`run`] | `run.start`, `run.preview`, `run.list`, `run.get`, `run.stop`, `run.diff`, `run.output`, `run.intervene`, and the take-over's six |
//! | queue | [`queue`] | `queue.list`, `queue.context`, `queue.answer` |
//! | workspace | [`workspace`] | `workspace.list`, `workspace.remove`, `workspace.resume` |
//! | config | [`config`] | `config.get`, `config.set`, `config.pack_list`, `config.pack_add`, `config.pack_link`, `config.pack_enable` |

pub mod board;
pub mod config;
pub mod daemon;
pub mod queue;
pub mod run;
pub mod task;
pub mod workspace;

pub use board::*;
pub use config::*;
pub use daemon::*;
pub use queue::*;
pub use run::*;
pub use task::*;
pub use workspace::*;

use crate::change::Actor;
use crate::context::ContextRecord;
use crate::events::Event;
use crate::provider::PreviewResult;
use crate::section::Sections;
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;
use std::fmt;

/// The version of this protocol. A client that sends another is refused ([`check_initialize`]). It moves when a
/// method, a field or a code changes in a way an older peer would misread.
pub const OWNER_PROTOCOL: u32 = 1;

/// The longest line the daemon reads or writes, in bytes. The plan sets no bound; a line is read whole, so one is
/// needed, and 8 MiB leaves room for a page of output and a diff's patch.
pub const MAX_MESSAGE_BYTES: usize = 8 * 1024 * 1024;

/// The `"jsonrpc": "2.0"` member. It writes that text and reads nothing else.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct JsonRpc2;

impl Serialize for JsonRpc2 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str("2.0")
    }
}

impl<'de> Deserialize<'de> for JsonRpc2 {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        if text == "2.0" {
            Ok(JsonRpc2)
        } else {
            Err(serde::de::Error::custom(format!(
                "`jsonrpc` is `{text}`, and must be `2.0`"
            )))
        }
    }
}

impl JsonSchema for JsonRpc2 {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "JsonRpc2".into()
    }

    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({ "type": "string", "const": "2.0" })
    }
}

/// A request's id: a number or a text, as JSON-RPC allows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum RequestId {
    Number(i64),
    Text(String),
}

/// A request. `params` is left out when null; a method that takes no params reads that as `{}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "OwnerRequest")]
pub struct Request {
    pub jsonrpc: JsonRpc2,
    pub id: RequestId,
    pub method: String,
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub params: Value,
}

/// A message from the daemon that expects no answer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "OwnerNotification")]
pub struct Notification {
    pub jsonrpc: JsonRpc2,
    pub method: String,
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub params: Value,
}

/// A failure: a code from [`CODES`], a message in Collide's words, and optional data for a program.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl RpcError {
    pub fn new(code: i64, message: impl Into<String>) -> Self {
        RpcError {
            code,
            message: message.into(),
            data: None,
        }
    }

    pub fn with_data(mut self, data: Value) -> Self {
        self.data = Some(data);
        self
    }

    pub fn method_not_found(method: &str) -> Self {
        RpcError::new(
            code::METHOD_NOT_FOUND,
            format!("method not found: `{method}`"),
        )
    }
}

impl fmt::Display for RpcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.message, self.code)
    }
}

impl std::error::Error for RpcError {}

/// An answer: the result, or the error. A failure's id is null when the request's id could not be read.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged, deny_unknown_fields)]
#[schemars(rename = "OwnerResponse")]
pub enum Response {
    Success {
        jsonrpc: JsonRpc2,
        id: RequestId,
        result: Value,
    },
    Failure {
        jsonrpc: JsonRpc2,
        id: Option<RequestId>,
        error: RpcError,
    },
}

impl Response {
    pub fn success(id: RequestId, result: Value) -> Self {
        Response::Success {
            jsonrpc: JsonRpc2,
            id,
            result,
        }
    }

    pub fn failure(id: Option<RequestId>, error: RpcError) -> Self {
        Response::Failure {
            jsonrpc: JsonRpc2,
            id,
            error,
        }
    }
}

/// The error codes, in one place: the standard five of JSON-RPC, then the owner's in the server range
/// (-32099 to -32000). A client matches on the number and shows the message.
pub mod code {
    /// The line was not JSON.
    pub const PARSE_ERROR: i64 = -32700;
    /// The JSON was not a request, or the line was over [`super::MAX_MESSAGE_BYTES`].
    pub const INVALID_REQUEST: i64 = -32600;
    /// No such method.
    pub const METHOD_NOT_FOUND: i64 = -32601;
    /// The params did not read as the method's request type, or carried an `actor`.
    pub const INVALID_PARAMS: i64 = -32602;
    /// A fault of the daemon's own.
    pub const INTERNAL_ERROR: i64 = -32603;
    /// A method came before `initialize`.
    pub const NOT_INITIALIZED: i64 = -32001;
    /// `initialize` came a second time on the connection.
    pub const ALREADY_INITIALIZED: i64 = -32002;
    /// The client's protocol version is not the daemon's.
    pub const VERSION_REFUSED: i64 = -32003;
    /// The per-client token is missing or wrong.
    pub const TOKEN_REFUSED: i64 = -32004;
    /// The connection's actor may not do this (a person-only operation asked by an agent).
    pub const NOT_PERMITTED: i64 = -32005;
    /// A run, a task, a board, a workspace, a Queue item or a terminal named in the request does not exist.
    pub const NOT_FOUND: i64 = -32010;
    /// The request is well formed and the state refuses it: a live run on the task, a workspace in use.
    pub const CONFLICT: i64 = -32011;
    /// The daemon cannot do it now: its store is unavailable, or it is shutting down.
    pub const UNAVAILABLE: i64 = -32012;
}

/// Every code with its name and what it means. A test keeps the values distinct and in range.
pub const CODES: &[(i64, &str, &str)] = &[
    (code::PARSE_ERROR, "parse_error", "the line was not JSON"),
    (
        code::INVALID_REQUEST,
        "invalid_request",
        "the JSON was not a request, or the line was over the size limit",
    ),
    (
        code::METHOD_NOT_FOUND,
        "method_not_found",
        "no method has that name",
    ),
    (
        code::INVALID_PARAMS,
        "invalid_params",
        "the params did not read as the method's request type, or carried an actor",
    ),
    (
        code::INTERNAL_ERROR,
        "internal_error",
        "a fault of the daemon's own",
    ),
    (
        code::NOT_INITIALIZED,
        "not_initialized",
        "a method came before initialize",
    ),
    (
        code::ALREADY_INITIALIZED,
        "already_initialized",
        "initialize came a second time on the connection",
    ),
    (
        code::VERSION_REFUSED,
        "version_refused",
        "the client's protocol version is not the daemon's",
    ),
    (
        code::TOKEN_REFUSED,
        "token_refused",
        "the per-client token is missing or wrong",
    ),
    (
        code::NOT_PERMITTED,
        "not_permitted",
        "the connection's actor may not do this",
    ),
    (
        code::NOT_FOUND,
        "not_found",
        "a run, task, board, workspace, Queue item or terminal in the request does not exist",
    ),
    (
        code::CONFLICT,
        "conflict",
        "the request is well formed and the current state refuses it",
    ),
    (
        code::UNAVAILABLE,
        "unavailable",
        "the daemon cannot do it now: its store is unavailable, or it is shutting down",
    ),
];

/// The answer of a method that returns nothing: an empty object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Ack {}

/// `initialize` params. `token` is the per-client token (0.2) and is optional on the wire only so that a missing one
/// gets [`code::TOKEN_REFUSED`] and not a params error. `client` names the program, for the daemon's log.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OwnerInitializeParams {
    pub protocol: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client: Option<String>,
}

/// `initialize` result: the daemon's version and protocol, and the actor it recorded for this connection.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OwnerInitializeResult {
    pub protocol: u32,
    pub daemon_version: String,
    pub actor: Actor,
}

/// A protocol version that is not the daemon's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VersionRefused {
    pub client: u32,
    pub daemon: u32,
}

impl fmt::Display for VersionRefused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let side = if self.client > self.daemon {
            "newer"
        } else {
            "older"
        };
        write!(
            f,
            "the client speaks owner protocol {}, and the daemon speaks {}: the client is {side} than the daemon",
            self.client, self.daemon
        )
    }
}

impl From<VersionRefused> for RpcError {
    fn from(refused: VersionRefused) -> Self {
        RpcError::new(code::VERSION_REFUSED, refused.to_string()).with_data(serde_json::json!({
            "client": refused.client,
            "daemon": refused.daemon,
        }))
    }
}

/// Whether two tokens are equal, comparing every byte of the longer one so the time does not say where they differ.
pub fn tokens_match(given: &str, expected: &str) -> bool {
    let (given, expected) = (given.as_bytes(), expected.as_bytes());
    let mut diff = given.len() ^ expected.len();
    for i in 0..given.len().max(expected.len()) {
        let a = given.get(i).copied().unwrap_or(0);
        let b = expected.get(i).copied().unwrap_or(0);
        diff |= usize::from(a ^ b);
    }
    diff == 0
}

/// Checks the `initialize` of a connection against the daemon's token: the token first, then the version. A message
/// never carries a token.
pub fn check_initialize(params: &OwnerInitializeParams, token: &str) -> Result<(), RpcError> {
    match params.token.as_deref() {
        None => {
            return Err(RpcError::new(
                code::TOKEN_REFUSED,
                "the per-client token is missing from initialize",
            ));
        }
        Some(given) if !tokens_match(given, token) => {
            return Err(RpcError::new(
                code::TOKEN_REFUSED,
                "the per-client token is wrong",
            ));
        }
        Some(_) => {}
    }
    if params.protocol != OWNER_PROTOCOL {
        return Err(VersionRefused {
            client: params.protocol,
            daemon: OWNER_PROTOCOL,
        }
        .into());
    }
    Ok(())
}

/// The family a method belongs to: the text before its dot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Family {
    Daemon,
    Board,
    Task,
    Run,
    Queue,
    Workspace,
    Config,
}

impl Family {
    pub fn name(self) -> &'static str {
        match self {
            Family::Daemon => "daemon",
            Family::Board => "board",
            Family::Task => "task",
            Family::Run => "run",
            Family::Queue => "queue",
            Family::Workspace => "workspace",
            Family::Config => "config",
        }
    }
}

/// Reads `params` as `T` (null is `{}`) and writes it back, so a caller sees what the daemon would act on.
fn normalize<T: Serialize + DeserializeOwned>(params: &Value) -> Result<Value, serde_json::Error> {
    let value = if params.is_null() {
        Value::Object(Default::default())
    } else {
        params.clone()
    };
    serde_json::to_value(serde_json::from_value::<T>(value)?)
}

/// Writes the `Method` list: for each method its name, family, what it serves, and its request and result types.
macro_rules! methods {
    ($($variant:ident, $name:literal, $family:ident, $serves:literal, $params:ty, $result:ty;)*) => {
        /// A method of the owner's JSON-RPC.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum Method {
            $($variant),*
        }

        impl Method {
            /// Every method, in the order of the plan's families.
            pub const ALL: &'static [Method] = &[$(Method::$variant),*];

            /// The name on the wire.
            pub fn name(self) -> &'static str {
                match self {
                    $(Method::$variant => $name),*
                }
            }

            pub fn from_name(name: &str) -> Option<Method> {
                match name {
                    $($name => Some(Method::$variant),)*
                    _ => None,
                }
            }

            pub fn family(self) -> Family {
                match self {
                    $(Method::$variant => Family::$family),*
                }
            }

            /// What the method serves: a CLI command (`collide ...`) or a section of the plan (`0.11, ...`).
            pub fn serves(self) -> &'static str {
                match self {
                    $(Method::$variant => $serves),*
                }
            }

            /// The name of the request type's schema (`<name>.input.json` in `schemas/`).
            pub fn params_schema(self) -> String {
                match self {
                    $(Method::$variant => <$params as JsonSchema>::schema_name().into_owned()),*
                }
            }

            /// The name of the result type's schema.
            pub fn result_schema(self) -> String {
                match self {
                    $(Method::$variant => <$result as JsonSchema>::schema_name().into_owned()),*
                }
            }

            /// Reads `params` as the request type and writes it back; an unknown key, an `actor` among them, is an
            /// error.
            pub fn normalize_params(self, params: &Value) -> Result<Value, serde_json::Error> {
                match self {
                    $(Method::$variant => normalize::<$params>(params)),*
                }
            }

            /// Reads `result` as the result type and writes it back.
            pub fn normalize_result(self, result: &Value) -> Result<Value, serde_json::Error> {
                match self {
                    $(Method::$variant => normalize::<$result>(result)),*
                }
            }
        }
    };
}

methods! {
    Initialize, "initialize", Daemon,
        "0.2, the handshake every connection opens with: protocol version and per-client token",
        OwnerInitializeParams, OwnerInitializeResult;
    DaemonStatus, "daemon.status", Daemon,
        "0.2, the daemon's health: version, uptime, live runs, the store and its warnings",
        DaemonStatusParams, DaemonStatusResult;
    DaemonRecaptureEnvironment, "daemon.recapture_environment", Daemon,
        "0.2, Recapture environment: the login-shell capture again, on request",
        DaemonRecaptureEnvironmentParams, DaemonRecaptureEnvironmentResult;
    DaemonSubscribe, "daemon.subscribe", Daemon,
        "0.2, the event channel: replay from a sequence number, then live `daemon.event` notices",
        DaemonSubscribeParams, DaemonSubscribeResult;
    BoardList, "board.list", Board,
        "0.11, the board switcher",
        BoardListParams, BoardListResult;
    BoardOpen, "board.open", Board,
        "0.11, opening a repo with no setup, and its first-run checks",
        BoardOpenParams, BoardOpenResult;
    BoardInit, "board.init", Board,
        "0.11, writing a board's starter files, or listing them with a dry run",
        BoardInitParams, BoardInitResult;
    BoardTrust, "board.trust", Board,
        "0.9, Trust: trusting a line's packs and gate code by their hash",
        BoardTrustParams, BoardTrustResult;
    TaskList, "task.list", Task,
        "collide tasks",
        TaskListParams, TaskListResult;
    TaskGet, "task.get", Task,
        "collide task, showing one item",
        TaskGetParams, TaskGetResult;
    TaskApply, "task.apply", Task,
        "collide task, changing an item with an op",
        TaskApplyParams, TaskApplyResult;
    TaskPreview, "task.preview", Task,
        "collide task, the files an op would change before it is applied",
        TaskPreviewParams, PreviewResult;
    TaskViews, "task.views", Task,
        "0.11, the views of a board with their sources and counts",
        TaskViewsParams, TaskViewsResult;
    TaskSaveView, "task.save_view", Task,
        "0.11, Save view: into the repo or the user's own files",
        TaskSaveViewParams, TaskSaveViewResult;
    TaskSections, "task.sections", Task,
        "collide sections",
        TaskSectionsParams, Sections;
    RunStart, "run.start", Run,
        "collide run, and Launch",
        RunStartParams, RunStartResult;
    RunPreview, "run.preview", Run,
        "0.11, the run preview that Launch shows before anything starts",
        RunPreviewParams, RunPreviewResult;
    RunList, "run.list", Run,
        "0.11, Tracks, and an app that reattaches after a restart",
        RunListParams, RunListResult;
    RunGet, "run.get", Run,
        "0.11, one run with what it pinned at launch",
        RunGetParams, RunGetResult;
    RunStop, "run.stop", Run,
        "collide stop",
        RunStopParams, RunStopResult;
    RunDiff, "run.diff", Run,
        "0.11, Review: a run's diff, and one file's patch",
        RunDiffParams, RunDiffResult;
    RunOutput, "run.output", Run,
        "0.11, Agent output: a page of a phase's normalized stream",
        RunOutputParams, RunOutputResult;
    RunIntervene, "run.intervene", Run,
        "0.11, sending a note to a running phase",
        RunIntervenParams, Ack;
    RunTakeOver, "run.take_over", Run,
        "0.11, Talking to an agent: the take-over's terminal, opened or attached again",
        RunTakeOverParams, RunTakeOverResult;
    RunTerminalInput, "run.terminal_input", Run,
        "0.11, Talking to an agent: keys for the take-over's terminal",
        RunTerminalInputParams, Ack;
    RunTerminalResize, "run.terminal_resize", Run,
        "0.11, Talking to an agent: the terminal's size",
        RunTerminalResizeParams, Ack;
    RunDetach, "run.detach", Run,
        "0.11, Talking to an agent: closing the view leaves the session running",
        RunDetachParams, Ack;
    RunEndTakeOver, "run.end_take_over", Run,
        "0.11, End take-over: the phase's gates then run as usual",
        RunEndTakeOverParams, Ack;
    QueueList, "queue.list", Queue,
        "0.11, Queue: every decision waiting on the user",
        QueueListParams, QueueListResult;
    QueueContext, "queue.context", Queue,
        "0.11, Queue: Why is this asked?",
        QueueContextParams, ContextRecord;
    QueueAnswer, "queue.answer", Queue,
        "0.11, Queue: allow, deny, proceed, stop, retry, accept, discard and the rest",
        QueueAnswerParams, QueueAnswerResult;
    WorkspaceList, "workspace.list", Workspace,
        "0.8, Workspace: the checkouts, with what holds each",
        WorkspaceListParams, WorkspaceListResult;
    WorkspaceRemove, "workspace.remove", Workspace,
        "0.8, Workspace: removing a checkout that nothing holds",
        WorkspaceRemoveParams, WorkspaceRemoveResult;
    WorkspaceResume, "workspace.resume", Workspace,
        "0.8, Workspace: reopening a kept checkout",
        WorkspaceResumeParams, WorkspaceResumeResult;
    ConfigGet, "config.get", Config,
        "collide config, reading a setting with the layer it came from",
        ConfigGetParams, ConfigGetResult;
    ConfigSet, "config.set", Config,
        "collide config, writing a setting to a layer",
        ConfigSetParams, ConfigSetResult;
    ConfigPackList, "config.pack_list", Config,
        "collide pack, listing the packs of a board",
        ConfigPackListParams, ConfigPackListResult;
    ConfigPackAdd, "config.pack_add", Config,
        "collide pack, adding a pack",
        ConfigPackAddParams, ConfigPackResult;
    ConfigPackLink, "config.pack_link", Config,
        "collide pack, linking a working-copy folder",
        ConfigPackLinkParams, ConfigPackResult;
    ConfigPackEnable, "config.pack_enable", Config,
        "collide pack, enabling or disabling a pack",
        ConfigPackEnableParams, ConfigPackResult;
}

/// What [`route`] found.
#[derive(Debug, Clone, PartialEq)]
pub enum Routed {
    /// `initialize`, with its params. The caller runs [`check_initialize`].
    Initialize(OwnerInitializeParams),
    /// A method whose params read as its request type. The caller reads them again as that type to act on them.
    Call(Method),
}

/// Decides what a request is, before any method runs: an unknown method is [`code::METHOD_NOT_FOUND`], a second
/// `initialize` is [`code::ALREADY_INITIALIZED`], any other method before `initialize` is
/// [`code::NOT_INITIALIZED`], and params that do not read, an `actor` among them, are [`code::INVALID_PARAMS`].
pub fn route(initialized: bool, request: &Request) -> Result<Routed, RpcError> {
    let method = Method::from_name(&request.method)
        .ok_or_else(|| RpcError::method_not_found(&request.method))?;
    if method == Method::Initialize {
        if initialized {
            return Err(RpcError::new(
                code::ALREADY_INITIALIZED,
                "initialize was already sent on this connection",
            ));
        }
    } else if !initialized {
        return Err(RpcError::new(
            code::NOT_INITIALIZED,
            format!(
                "`{}` came before initialize: the first request of a connection is `initialize`",
                request.method
            ),
        ));
    }
    if request.params.get("actor").is_some() {
        return Err(RpcError::new(
            code::INVALID_PARAMS,
            format!(
                "`{}` takes no `actor`: the daemon records the actor from the connection",
                request.method
            ),
        ));
    }
    let normalized = method.normalize_params(&request.params).map_err(|error| {
        RpcError::new(
            code::INVALID_PARAMS,
            format!("invalid params for `{}`: {error}", request.method),
        )
    })?;
    if method == Method::Initialize {
        let params = serde_json::from_value(normalized).map_err(|error| {
            RpcError::new(
                code::INVALID_PARAMS,
                format!("invalid params for `initialize`: {error}"),
            )
        })?;
        return Ok(Routed::Initialize(params));
    }
    Ok(Routed::Call(method))
}

/// Reads one line as a request. The failure is the response to send: [`code::PARSE_ERROR`] for text that is not
/// JSON, [`code::INVALID_REQUEST`] for JSON that is not a request or a line over [`MAX_MESSAGE_BYTES`], with the id
/// when one can be read.
pub fn parse_request(line: &str) -> Result<Request, Response> {
    if line.len() > MAX_MESSAGE_BYTES {
        return Err(Response::failure(
            None,
            RpcError::new(
                code::INVALID_REQUEST,
                format!(
                    "the message is {} bytes, over the {MAX_MESSAGE_BYTES}-byte limit",
                    line.len()
                ),
            ),
        ));
    }
    let value: Value = serde_json::from_str(line).map_err(|error| {
        Response::failure(
            None,
            RpcError::new(code::PARSE_ERROR, format!("not JSON: {error}")),
        )
    })?;
    let id = value
        .get("id")
        .and_then(|id| serde_json::from_value::<RequestId>(id.clone()).ok());
    serde_json::from_value::<Request>(value).map_err(|error| {
        Response::failure(
            id,
            RpcError::new(code::INVALID_REQUEST, format!("not a request: {error}")),
        )
    })
}

/// One message as a line: compact JSON and a newline. Newlines inside strings are escaped, so it is one line.
pub fn encode_line<T: Serialize>(message: &T) -> String {
    let mut line = serde_json::to_string(message).expect("a message of this protocol is JSON");
    line.push('\n');
    line
}

/// What the daemon sends without being asked, after `daemon.subscribe`. On the wire a notice is a [`Notification`]
/// whose `method` is the name below and whose `params` is the payload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "method", content = "params", deny_unknown_fields)]
#[schemars(rename = "OwnerNotice")]
pub enum Notice {
    /// An event of the store, with its `seq` and the actor the daemon recorded.
    #[serde(rename = "daemon.event")]
    DaemonEvent(Box<Event>),
    /// One new entry of a phase's normalized stream.
    #[serde(rename = "run.output")]
    RunOutput(Box<RunOutputNotice>),
    /// Bytes a take-over's terminal wrote.
    #[serde(rename = "run.terminal_output")]
    RunTerminalOutput(RunTerminalOutput),
    /// The Queue changed.
    #[serde(rename = "queue.changed")]
    QueueChanged(QueueChanged),
}

impl Notice {
    pub fn method(&self) -> &'static str {
        match self {
            Notice::DaemonEvent(_) => "daemon.event",
            Notice::RunOutput(_) => "run.output",
            Notice::RunTerminalOutput(_) => "run.terminal_output",
            Notice::QueueChanged(_) => "queue.changed",
        }
    }

    pub fn to_notification(&self) -> Notification {
        let mut value = serde_json::to_value(self).expect("a notice is JSON");
        let params = value["params"].take();
        Notification {
            jsonrpc: JsonRpc2,
            method: self.method().to_string(),
            params,
        }
    }

    /// Reads a notification as a notice. An unknown method, or params the notice's type does not take, is an error.
    pub fn from_notification(notification: &Notification) -> Result<Notice, serde_json::Error> {
        serde_json::from_value(serde_json::json!({
            "method": notification.method,
            "params": notification.params,
        }))
    }
}
