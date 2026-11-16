//! The scripted ACP double: an [`AgentSession`] that is fed the lines of an ACP agent and writes the client's side
//! of the conversation (S7, the `opencode acp` run). It is `core`'s own test double. The ACP engine is
//! `host::adapters::acp::session::AcpSession`, which began as a copy of this logic and has hardened it since; the two
//! are kept in step by the shared contract (`contract`), which both run. The engine's tests reuse the script here
//! ([`initialize_response`] and the functions after it).
//!
//! The session is the client. It holds what an ACP conversation needs held (0.3, Two core traits):
//!
//! - **the handshake:** `start` writes `initialize`; its response makes the session write `session/new`; that
//!   response binds the session id and makes it write the first `session/prompt`;
//! - **request ids:** the ids of the requests it writes count from 0, so `initialize` is 0, `session/new` is 1, the
//!   first prompt is 2 and each `send_turn` takes the next. A response is matched to its request by id; one that
//!   matches nothing changes nothing;
//! - **permission requests:** a `session/request_permission` is a request of the agent with its own id. The session
//!   holds it, however many lines come in, until `answer`, and `cancel` answers every one still held `cancelled`, as
//!   ACP requires;
//! - **the turn end:** the `session/prompt` response ends the turn, and a turn the host cancelled is reported
//!   `Cancelled` whatever `stopReason` says (opencode answers `end_turn`; S7, O1). An `end_turn` with no words, no
//!   tool call and no usage is `empty` (S7, O4).
//!
//! | agent writes (stdout) | the session reads |
//! |---|---|
//! | a response to `initialize` | writes `session/new` |
//! | a response to `session/new` | `SessionBound`; writes the first prompt |
//! | `session/update` `agent_message_chunk` | `Message` |
//! | `session/update` `tool_call` | `ToolCall` |
//! | any other `session/update` | `Other`, with the update |
//! | `session/request_permission` | `PermissionRequest`, held |
//! | a response to `session/prompt` | `Usage` (if sent), `FinalReport` (if any words), `TurnEnd` |
//!
//! The double advertises no `fs`, `terminal` or `elicitation` capability, so the daemon stays out of the agent's
//! data path (S7, proposal 9). It does not resume a session (`session/load`) or write `session/set_model`.

use super::contract::ChildDialect;
use super::{json_line, outcome_of_turn, stamped, unparsed_line};
use crate::adapter::{
    AgentSession, ExitInfo, Finished, PhaseOutcome, ReplayError, StampedEvent, Step,
};
use crate::agent::{
    AgentEvent, FinalReportSource, PermissionOption, PermissionOptionKind, PermissionReply,
    PermissionRequest, ToolKind, TurnEndReason, Usage, UsageQuality, UsageScope,
};
use crate::journal::JournalRecord;
use crate::replay::{Replayable, replay_records};
use serde_json::{Value, json};

/// What a request the session wrote is waiting to be answered as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Awaiting {
    Initialize,
    SessionNew,
    Prompt,
}

/// A request of the agent that no one has answered. `raw_id` is the JSON-RPC id as the agent wrote it, which the
/// answer must echo; the normalized id of the request is its text.
#[derive(Debug, Clone)]
struct HeldRequest {
    id: String,
    raw_id: Value,
    options: Vec<PermissionOption>,
}

/// The scripted ACP session.
#[derive(Debug, Clone)]
pub struct AcpSession {
    cwd: String,
    first_prompt: String,
    next_id: u64,
    awaiting: Vec<(u64, Awaiting)>,
    session_id: Option<String>,
    held: Vec<HeldRequest>,
    cancel_requested: bool,
    turn_had_output: bool,
    turn_report: String,
    last_turn: Option<(TurnEndReason, bool)>,
}

impl AcpSession {
    /// A session for an agent that runs in `cwd` and starts with `first_prompt` as its first turn.
    pub fn new(cwd: &str, first_prompt: &str) -> Self {
        Self {
            cwd: cwd.into(),
            first_prompt: first_prompt.into(),
            next_id: 0,
            awaiting: Vec::new(),
            session_id: None,
            held: Vec::new(),
            cancel_requested: false,
            turn_had_output: false,
            turn_report: String::new(),
            last_turn: None,
        }
    }

    /// Writes one message: learns it as a replay would, then returns it as the bytes for the child.
    fn write(&mut self, message: Value) -> Vec<u8> {
        let bytes = json_line(&message);
        self.note_written(&bytes)
            .expect("a session reads its own writes");
        bytes
    }

    /// Writes a request with the next id.
    fn request(&mut self, method: &str, params: Value) -> Vec<u8> {
        let id = self.next_id;
        self.write(json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }))
    }

    fn prompt(&mut self, session_id: &str, text: &str) -> Vec<u8> {
        self.request(
            "session/prompt",
            json!({ "sessionId": session_id, "prompt": [{ "type": "text", "text": text }] }),
        )
    }

    /// The answer to a request of the agent.
    fn respond(&mut self, raw_id: &Value, outcome: Value) -> Vec<u8> {
        self.write(json!({ "jsonrpc": "2.0", "id": raw_id, "result": { "outcome": outcome } }))
    }

    /// Reads one line of the agent: the events, and the request the session must write next, if any. Reading only
    /// changes what the agent said; a replay reads and drops the request, because the journal holds the write.
    fn read(
        &mut self,
        line: &[u8],
        wall_ns: u64,
    ) -> (Vec<StampedEvent>, Option<(&'static str, Value)>) {
        let Ok(message) = serde_json::from_slice::<Value>(line) else {
            return (vec![unparsed_line(line, wall_ns)], None);
        };
        let at = |event| stamped(Some(wall_ns), event);
        match message.get("method").and_then(Value::as_str) {
            Some("session/update") => (self.read_update(&message, wall_ns), None),
            Some("session/request_permission") if message.get("id").is_some() => {
                (vec![at(self.hold_request(&message))], None)
            }
            Some(method) => (
                vec![at(AgentEvent::Other {
                    raw_type: method.into(),
                    raw: message.get("params").cloned(),
                })],
                None,
            ),
            None => self.read_response(&message, wall_ns),
        }
    }

    fn read_update(&mut self, message: &Value, wall_ns: u64) -> Vec<StampedEvent> {
        let update = message
            .pointer("/params/update")
            .cloned()
            .unwrap_or(Value::Null);
        let kind = update
            .get("sessionUpdate")
            .and_then(Value::as_str)
            .map(str::to_string);
        let event = match kind.as_deref() {
            Some("agent_message_chunk") => {
                let text = update.pointer("/content/text").and_then(Value::as_str);
                text.map(|text| {
                    self.turn_had_output = true;
                    self.turn_report.push_str(text);
                    AgentEvent::Message { text: text.into() }
                })
            }
            Some("tool_call") => {
                self.turn_had_output = true;
                self.turn_report.clear();
                Some(AgentEvent::ToolCall {
                    id: text_of(&update, "toolCallId"),
                    tool: update
                        .get("title")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    kind: tool_kind(update.get("kind")),
                    input: known_input(update.get("rawInput")),
                })
            }
            _ => None,
        };
        let event = event.unwrap_or_else(|| AgentEvent::Other {
            raw_type: kind.unwrap_or_else(|| "session/update".into()),
            raw: Some(update),
        });
        vec![stamped(Some(wall_ns), event)]
    }

    fn hold_request(&mut self, message: &Value) -> AgentEvent {
        let raw_id = message["id"].clone();
        let id = id_text(&raw_id);
        let call = message
            .pointer("/params/toolCall")
            .cloned()
            .unwrap_or(Value::Null);
        // An option of a kind Collide does not know cannot be picked, so it is not offered.
        let options: Vec<PermissionOption> = message
            .pointer("/params/options")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|option| {
                Some(PermissionOption {
                    id: option.get("optionId")?.as_str()?.to_string(),
                    name: text_of(option, "name"),
                    kind: option_kind(option.get("kind")?.as_str()?)?,
                })
            })
            .collect();
        self.held.retain(|held| held.id != id);
        self.held.push(HeldRequest {
            id: id.clone(),
            raw_id,
            options: options.clone(),
        });
        AgentEvent::PermissionRequest {
            request: PermissionRequest {
                id,
                tool_call_id: text_of(&call, "toolCallId"),
                tool: call
                    .get("title")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                kind: tool_kind(call.get("kind")),
                input: known_input(call.get("rawInput")),
                reason: None,
                options,
            },
        }
    }

    fn read_response(
        &mut self,
        message: &Value,
        wall_ns: u64,
    ) -> (Vec<StampedEvent>, Option<(&'static str, Value)>) {
        let at = |event| stamped(Some(wall_ns), event);
        let Some(id) = message.get("id").and_then(Value::as_u64) else {
            return (Vec::new(), None);
        };
        let Some(position) = self.awaiting.iter().position(|(awaited, _)| *awaited == id) else {
            return (Vec::new(), None);
        };
        let (_, awaiting) = self.awaiting.remove(position);
        if awaiting == Awaiting::Prompt {
            return (self.end_turn(message, wall_ns), None);
        }
        let result = message.get("result");
        match (awaiting, result) {
            (Awaiting::Initialize, Some(_)) => (
                Vec::new(),
                Some(("session/new", json!({ "cwd": self.cwd, "mcpServers": [] }))),
            ),
            (Awaiting::SessionNew, Some(result)) => {
                let Some(session_id) = result.get("sessionId").and_then(Value::as_str) else {
                    return (vec![at(malformed(message))], None);
                };
                self.session_id = Some(session_id.to_string());
                let prompt = json!({
                    "sessionId": session_id,
                    "prompt": [{ "type": "text", "text": self.first_prompt }]
                });
                (
                    vec![at(AgentEvent::SessionBound {
                        id: session_id.into(),
                    })],
                    Some(("session/prompt", prompt)),
                )
            }
            _ => (vec![at(malformed(message))], None),
        }
    }

    fn end_turn(&mut self, message: &Value, wall_ns: u64) -> Vec<StampedEvent> {
        let at = |event| stamped(Some(wall_ns), event);
        let mut events = Vec::new();
        let usage = message.pointer("/result/usage").and_then(read_usage);
        let spent = usage.as_ref().is_some_and(|usage| {
            [
                usage.input,
                usage.output,
                usage.thought,
                usage.cache_read,
                usage.cache_write,
            ]
            .into_iter()
            .flatten()
            .any(|count| count > 0)
        });
        if let Some(usage) = usage {
            events.push(at(AgentEvent::Usage { usage }));
        }
        let failed = message.get("error").is_some();
        if !failed && !self.turn_report.is_empty() {
            events.push(at(AgentEvent::FinalReport {
                text: std::mem::take(&mut self.turn_report),
                source: FinalReportSource::Reconstructed,
            }));
        }
        // A stop reason this version does not know is an error rather than a success.
        let reason = match message
            .pointer("/result/stopReason")
            .and_then(Value::as_str)
        {
            _ if failed => TurnEndReason::Error { status: None },
            Some("end_turn") => TurnEndReason::EndTurn {},
            Some("max_tokens") => TurnEndReason::MaxTokens {},
            Some("max_turn_requests") => TurnEndReason::MaxTurns {},
            Some("refusal") => TurnEndReason::Refusal {},
            Some("cancelled") => TurnEndReason::Cancelled {},
            _ => TurnEndReason::Error { status: None },
        };
        // The host cancelled this turn, so it is cancelled, whatever the agent said (S7, O1).
        let reason = if self.cancel_requested {
            TurnEndReason::Cancelled {}
        } else {
            reason
        };
        let empty = !self.turn_had_output && !spent;
        self.last_turn = Some((reason.clone(), empty));
        events.push(at(AgentEvent::TurnEnd {
            reason,
            cancel_requested: self.cancel_requested,
            empty,
        }));
        events
    }
}

impl Replayable for AcpSession {
    fn note_written(&mut self, bytes: &[u8]) -> Result<(), String> {
        let message: Value = serde_json::from_slice(bytes)
            .map_err(|reason| format!("not a JSON-RPC message: {reason}"))?;
        let id = message.get("id");
        match message.get("method").and_then(Value::as_str) {
            Some(method @ ("initialize" | "session/new" | "session/prompt")) => {
                let id = id
                    .and_then(Value::as_u64)
                    .ok_or_else(|| format!("a `{method}` request with no numeric id"))?;
                let awaiting = match method {
                    "initialize" => Awaiting::Initialize,
                    "session/new" => Awaiting::SessionNew,
                    _ => {
                        // A new turn: it has no words yet and is not cancelled.
                        self.turn_had_output = false;
                        self.turn_report.clear();
                        self.cancel_requested = false;
                        Awaiting::Prompt
                    }
                };
                self.awaiting.push((id, awaiting));
                self.next_id = self.next_id.max(id + 1);
            }
            Some("session/cancel") => self.cancel_requested = true,
            Some(other) => return Err(format!("not a request this session writes: `{other}`")),
            None => {
                let id = id.ok_or("a message with neither a method nor an id")?;
                self.held.retain(|held| &held.raw_id != id);
            }
        }
        Ok(())
    }

    fn read_line(&mut self, line: &[u8], wall_ns: u64) -> Step {
        Step {
            events: self.read(line, wall_ns).0,
            writes: Vec::new(),
        }
    }
}

impl AgentSession for AcpSession {
    fn start(&mut self) -> Step {
        let write = self.request(
            "initialize",
            json!({ "protocolVersion": 1, "clientCapabilities": {} }),
        );
        Step {
            events: Vec::new(),
            writes: vec![write],
        }
    }

    fn feed(&mut self, line: &[u8], wall_ns: u64) -> Step {
        let (events, next) = self.read(line, wall_ns);
        let writes = next
            .map(|(method, params)| self.request(method, params))
            .into_iter()
            .collect();
        Step { events, writes }
    }

    fn send_turn(&mut self, text: &str) -> Step {
        let Some(session_id) = self.session_id.clone() else {
            // There is no session to prompt yet. Say so, rather than lose the turn without a trace.
            return Step {
                events: vec![stamped(
                    None,
                    AgentEvent::Other {
                        raw_type: "turn_not_sent".into(),
                        raw: Some(json!({ "reason": "the session is not bound yet" })),
                    },
                )],
                writes: Vec::new(),
            };
        };
        Step {
            events: Vec::new(),
            writes: vec![self.prompt(&session_id, text)],
        }
    }

    fn answer(&mut self, request_id: &str, reply: PermissionReply) -> Step {
        let Some(held) = self.held.iter().find(|held| held.id == request_id).cloned() else {
            return Step::default();
        };
        let pick = |kind| held.options.iter().find(|option| option.kind == kind);
        let selected =
            |option: &PermissionOption| json!({ "outcome": "selected", "optionId": option.id });
        let cancelled = json!({ "outcome": "cancelled" });
        let mut events = Vec::new();
        let outcome = match reply {
            PermissionReply::Allow {} => match pick(PermissionOptionKind::AllowOnce) {
                Some(option) => selected(option),
                None => {
                    // Collide picks only `*_once` options, so there is nothing it may select (0.3).
                    events.push(stamped(
                        None,
                        AgentEvent::PermissionUnanswerable {
                            request_id: request_id.into(),
                        },
                    ));
                    cancelled
                }
            },
            // ACP carries no deny message, so it is dropped.
            PermissionReply::Deny { .. } => match pick(PermissionOptionKind::RejectOnce) {
                Some(option) => selected(option),
                None => cancelled,
            },
            PermissionReply::Cancel {} => cancelled,
        };
        Step {
            events,
            writes: vec![self.respond(&held.raw_id, outcome)],
        }
    }

    fn cancel(&mut self) -> Step {
        let Some(session_id) = self.session_id.clone() else {
            // Nothing has been prompted yet, so there is no turn to cancel.
            return Step::default();
        };
        let mut writes = vec![self.write(json!({
            "jsonrpc": "2.0", "method": "session/cancel", "params": { "sessionId": session_id }
        }))];
        // After `session/cancel` the client must answer every request still pending as cancelled.
        let held: Vec<Value> = self.held.iter().map(|held| held.raw_id.clone()).collect();
        for raw_id in held {
            writes.push(self.respond(&raw_id, json!({ "outcome": "cancelled" })));
        }
        Step {
            events: Vec::new(),
            writes,
        }
    }

    fn finish(&mut self, exit: &ExitInfo) -> Finished {
        // Exit alone is not an outcome in ACP: the prompt response is (S7). A child that went away with no
        // response, a pending request or not, leaves nothing known about its work.
        let outcome = match (&exit.record, &self.last_turn) {
            (Some(_), Some((reason, empty))) => outcome_of_turn(reason, *empty),
            _ => PhaseOutcome::Lost,
        };
        Finished {
            step: Step::default(),
            outcome,
        }
    }

    fn replay(&mut self, records: &[JournalRecord]) -> Result<Step, ReplayError> {
        replay_records(self, records)
    }
}

fn malformed(message: &Value) -> AgentEvent {
    AgentEvent::Other {
        raw_type: "malformed_response".into(),
        raw: Some(message.clone()),
    }
}

fn text_of(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

/// A JSON-RPC id as the text the normalized request carries.
fn id_text(id: &Value) -> String {
    match id {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

fn tool_kind(kind: Option<&Value>) -> ToolKind {
    match kind.and_then(Value::as_str) {
        Some("read") => ToolKind::Read,
        Some("edit") => ToolKind::Edit,
        Some("delete") => ToolKind::Delete,
        Some("move") => ToolKind::Move,
        Some("search") => ToolKind::Search,
        Some("execute") => ToolKind::Execute,
        Some("think") => ToolKind::Think,
        Some("fetch") => ToolKind::Fetch,
        _ => ToolKind::Other,
    }
}

fn option_kind(kind: &str) -> Option<PermissionOptionKind> {
    match kind {
        "allow_once" => Some(PermissionOptionKind::AllowOnce),
        "allow_always" => Some(PermissionOptionKind::AllowAlways),
        "reject_once" => Some(PermissionOptionKind::RejectOnce),
        "reject_always" => Some(PermissionOptionKind::RejectAlways),
        _ => None,
    }
}

/// The input of a tool call, or `None` while the agent has not sent it: opencode sends `rawInput: {}` for a Bash
/// call before the host answers (S7, O5).
fn known_input(raw: Option<&Value>) -> Option<Value> {
    match raw {
        None | Some(Value::Null) => None,
        Some(Value::Object(map)) if map.is_empty() => None,
        Some(value) => Some(value.clone()),
    }
}

/// `PromptResponse.usage` (unstable in ACP). It is `partial`: opencode reports the last model step only (S7, O2).
fn read_usage(usage: &Value) -> Option<Usage> {
    let object = usage.as_object()?;
    let count = |key: &str| object.get(key).and_then(Value::as_u64);
    Some(Usage {
        scope: UsageScope::Turn,
        input: count("inputTokens"),
        output: count("outputTokens"),
        cache_read: count("cachedReadTokens"),
        cache_write: count("cachedWriteTokens"),
        cache_write_1h: None,
        cache_write_5m: None,
        thought: count("thoughtTokens"),
        context_used: None,
        context_size: None,
        cost: None,
        model_time_ms: None,
        quality: UsageQuality::Partial,
    })
}

// --- The script: what an ACP agent writes, for a test or the engine's tests to feed ---

/// The agent's response to `initialize` request `id`.
pub fn initialize_response(id: u64) -> Vec<u8> {
    json_line(&json!({
        "jsonrpc": "2.0", "id": id,
        "result": {
            "protocolVersion": 1,
            "agentCapabilities": { "loadSession": true },
            "agentInfo": { "name": "fake-acp-agent", "version": "0" },
            "authMethods": []
        }
    }))
}

/// The agent's response to `session/new` request `id`, naming the session it made.
pub fn new_session_response(id: u64, session_id: &str) -> Vec<u8> {
    json_line(&json!({ "jsonrpc": "2.0", "id": id, "result": { "sessionId": session_id } }))
}

/// A `session/update` notification with `update`.
pub fn update(session_id: &str, update: Value) -> Vec<u8> {
    json_line(&json!({
        "jsonrpc": "2.0", "method": "session/update",
        "params": { "sessionId": session_id, "update": update }
    }))
}

/// One chunk of the agent's words.
pub fn message_chunk(session_id: &str, text: &str) -> Vec<u8> {
    update(
        session_id,
        json!({ "sessionUpdate": "agent_message_chunk", "content": { "type": "text", "text": text } }),
    )
}

/// The start of a tool call. `kind` is ACP's word (`read`, `edit`, `execute` and the rest).
pub fn tool_call(session_id: &str, tool_call_id: &str, title: &str, kind: &str) -> Vec<u8> {
    update(
        session_id,
        json!({
            "sessionUpdate": "tool_call", "toolCallId": tool_call_id, "title": title,
            "kind": kind, "status": "pending"
        }),
    )
}

/// A `session/request_permission` request with the agent's own `id`, for `tool_call` (an ACP `ToolCallUpdate`) and
/// `options` (an array of `{optionId, name, kind}`).
pub fn permission_request(id: u64, session_id: &str, tool_call: Value, options: Value) -> Vec<u8> {
    json_line(&json!({
        "jsonrpc": "2.0", "id": id, "method": "session/request_permission",
        "params": { "sessionId": session_id, "toolCall": tool_call, "options": options }
    }))
}

/// The three options opencode offers: allow once, allow always, reject.
pub fn standard_options() -> Value {
    json!([
        { "optionId": "once", "name": "Allow once", "kind": "allow_once" },
        { "optionId": "always", "name": "Allow always", "kind": "allow_always" },
        { "optionId": "reject", "name": "Reject", "kind": "reject_once" }
    ])
}

/// The response to `session/prompt` request `id`: its `stopReason` (`end_turn`, `max_tokens`, `max_turn_requests`,
/// `refusal` or `cancelled`) and the usage, when the agent sends one.
pub fn prompt_response(id: u64, stop_reason: &str, usage: Option<Value>) -> Vec<u8> {
    let mut result = json!({ "stopReason": stop_reason });
    if let Some(usage) = usage {
        result["usage"] = usage;
    }
    json_line(&json!({ "jsonrpc": "2.0", "id": id, "result": result }))
}

/// A JSON-RPC error as the response to `session/prompt` request `id`.
pub fn prompt_error(id: u64, code: i64, message: &str) -> Vec<u8> {
    json_line(&json!({
        "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message }
    }))
}

/// How the ACP double's agent speaks, for the shared contract.
#[derive(Debug, Clone, Copy, Default)]
pub struct AcpDialect;

const DIALECT_SESSION: &str = "ses_1";

impl ChildDialect for AcpDialect {
    type Session = AcpSession;

    fn session(&self) -> AcpSession {
        AcpSession::new("/work", "do the task")
    }

    fn handshake(&self) -> Vec<Vec<u8>> {
        vec![
            initialize_response(0),
            new_session_response(1, DIALECT_SESSION),
        ]
    }

    fn message(&self, text: &str) -> Vec<u8> {
        message_chunk(DIALECT_SESSION, text)
    }

    fn permission_request(&self, id: u32) -> Vec<u8> {
        let call = json!({
            "toolCallId": format!("call-{id}"), "title": "bash", "kind": "execute",
            "rawInput": { "command": "ls" }
        });
        permission_request(u64::from(id), DIALECT_SESSION, call, standard_options())
    }

    fn turn_end(&self, turn: u32) -> Vec<u8> {
        // The first prompt is request 2, and each later turn takes the next number.
        prompt_response(2 + u64::from(turn), "end_turn", None)
    }
}
