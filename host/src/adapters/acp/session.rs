//! The ACP session: an [`AgentSession`] that is the client of an ACP agent (0.3, Two core traits; S7, the `opencode
//! acp` run). It is sans-IO, like every session: it reads a line and a stored time and returns events and the bytes to
//! write. It began as the scripted double of `core` (`collide_core::testing::acp`), which stays `core`'s own test
//! double; the shared contract (`collide_core::testing::contract`) keeps the two in step.
//!
//! The session holds what an ACP conversation needs held:
//!
//! - **the handshake:** `start` writes `initialize`; its response makes the session write `session/new`; that
//!   response gives `SessionBound` and makes it write the first `session/prompt`. The first prompt is the session's
//!   own data ([`AcpOptions`]), not a turn queued by `send_turn`, so a session rebuilt from the journal still has it;
//! - **request ids:** the requests it writes count from 0. A response is matched to its request by id, and one that
//!   matches nothing changes nothing;
//! - **the turn:** the `session/prompt` response ends it. A turn the host cancelled is `Cancelled` whatever
//!   `stopReason` says (S7, O1). The final report is rebuilt from the message chunks after the last tool call.
//!
//! | agent writes (stdout) | the session reads | and writes |
//! |---|---|---|
//! | a response to `initialize` | nothing | `session/new` |
//! | a response to `session/new` | `SessionBound` | the first `session/prompt` |
//! | a response to `initialize` or `session/new` that is an error | `Other`, and `finish` says `Failed` | nothing |
//! | `session/update` `agent_message_chunk` | `Message` | nothing |
//! | `session/update` `tool_call` | `ToolCall`, by its `kind` | nothing |
//! | any other `session/update` | `Other` named by its `sessionUpdate` | nothing |
//! | `session/request_permission` | `PermissionRequest`, held until `answer` or `cancel` | nothing |
//! | any other request (`fs/*`, `terminal/*`, `elicitation/*`, unknown) | `Other` named by its method | `-32601` |
//! | a response to `session/prompt` | `FinalReport` (if any words), `TurnEnd` | nothing |
//! | a line that is not a JSON object | `Other` named `unparsed_line` | nothing |
//!
//! **No content from the agent is kept in an event of the `Other` kind.** A stream's content is untrusted and may be
//! large (0.3), and the journal already holds each line whole. An `Other` carries a name only, cut to 64 characters.
//!
//! The client advertises no `fs`, `terminal` or `elicitation` capability (0.3, Later adapters), so the agent keeps
//! its own file and shell tools and a daemon restart cannot stall a write. A request for one of them is answered
//! "method not found", so the agent never waits on it.
//!
//! Left to the items after this one, which harden what the double had: permission and cancel (`ts-0089-wv`), the
//! model (`ts-0090-jd`), empty turns, lost children and usage (`ts-0091-pz`). Until then `pending_permissions` is the
//! trait's default, `TurnEnd.empty` is `false`, and no `Usage` event is made.
//!
//! `PermissionRequest` carries no paths (`core/src/agent.rs`); ACP's `toolCall.locations` has them and is left out.

use super::options::{AcpOptions, McpServer};
use collide_core::adapter::{
    AgentSession, ExitInfo, Finished, PhaseFailure, PhaseOutcome, ReplayError, StampedEvent, Step,
};
use collide_core::agent::{
    AgentEvent, FinalReportSource, PermissionOption, PermissionOptionKind, PermissionReply,
    PermissionRequest, ToolKind, TurnEndReason,
};
use collide_core::journal::JournalRecord;
use collide_core::replay::{Replayable, replay_records};
use serde_json::{Value, json};

/// The JSON-RPC error for a method the client does not have.
const METHOD_NOT_FOUND: i64 = -32601;

/// The longest name an `Other` event carries, in characters.
const NAME_LIMIT: usize = 64;

/// What a request the session wrote is waiting to be answered as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Awaiting {
    Initialize,
    SessionNew,
    Prompt,
}

/// A request of the agent that no one has answered. `raw_id` is the JSON-RPC id as the agent wrote it, which the
/// answer must echo; the normalized id of the request is its text.
#[derive(Debug, Clone, PartialEq)]
struct HeldRequest {
    id: String,
    raw_id: Value,
    options: Vec<PermissionOption>,
}

/// What reading a line makes the session write next, if anything.
enum Outgoing {
    /// A request of the session's own, which takes the next id.
    Request { method: &'static str, params: Value },
    /// A reply to a request of the agent, with the agent's id.
    Reply(Value),
}

/// What reading one line gave.
struct Reading {
    events: Vec<StampedEvent>,
    write: Option<Outgoing>,
}

impl Reading {
    fn events(events: Vec<StampedEvent>) -> Self {
        Self {
            events,
            write: None,
        }
    }

    fn nothing() -> Self {
        Self::events(Vec::new())
    }
}

/// The ACP session. It compares equal to another that holds the same state, so a test can check that a replay
/// reached the state of the live session.
#[derive(Debug, Clone, PartialEq)]
pub struct AcpSession {
    options: AcpOptions,
    next_id: u64,
    awaiting: Vec<(u64, Awaiting)>,
    session_id: Option<String>,
    held: Vec<HeldRequest>,
    cancel_requested: bool,
    turn_report: String,
    handshake_failed: bool,
    last_turn: Option<TurnEndReason>,
}

impl AcpSession {
    /// A session for an agent that works as `options` say.
    pub fn new(options: AcpOptions) -> Self {
        Self {
            options,
            next_id: 0,
            awaiting: Vec::new(),
            session_id: None,
            held: Vec::new(),
            cancel_requested: false,
            turn_report: String::new(),
            handshake_failed: false,
            last_turn: None,
        }
    }

    /// Writes one message: learns it as a replay would, then returns it as the bytes for the child.
    fn write(&mut self, message: Value) -> Vec<u8> {
        let mut bytes = serde_json::to_vec(&message).expect("a JSON value serializes");
        bytes.push(b'\n');
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

    /// Reads one line of the agent. Reading changes what the agent said; a replay reads and drops the write, because
    /// the journal holds it.
    fn read(&mut self, line: &[u8], wall_ns: u64) -> Reading {
        let at = |event| stamped(Some(wall_ns), event);
        let Ok(message @ Value::Object(_)) = serde_json::from_slice::<Value>(line) else {
            return Reading::events(vec![at(named("unparsed_line"))]);
        };
        let has_id = message.get("id").is_some();
        match message.get("method").and_then(Value::as_str) {
            Some("session/update") => Reading::events(self.read_update(&message, wall_ns)),
            Some("session/request_permission") if has_id => {
                Reading::events(vec![at(self.hold_request(&message))])
            }
            // A request the engine does not serve: tell the agent at once, so it never waits on the answer.
            Some(method) if has_id => Reading {
                events: vec![at(named(method))],
                write: Some(Outgoing::Reply(json!({
                    "jsonrpc": "2.0", "id": message["id"],
                    "error": { "code": METHOD_NOT_FOUND, "message": "Method not found" }
                }))),
            },
            Some(method) => Reading::events(vec![at(named(method))]),
            None => self.read_response(&message, wall_ns),
        }
    }

    fn read_update(&mut self, message: &Value, wall_ns: u64) -> Vec<StampedEvent> {
        let update = message.pointer("/params/update").unwrap_or(&Value::Null);
        let kind = update.get("sessionUpdate").and_then(Value::as_str);
        let event = match kind {
            Some("agent_message_chunk") => {
                let text = update.pointer("/content/text").and_then(Value::as_str);
                text.map(|text| {
                    self.turn_report.push_str(text);
                    AgentEvent::Message { text: text.into() }
                })
            }
            Some("tool_call") => {
                // The report is what the agent says after its last tool call.
                self.turn_report.clear();
                Some(AgentEvent::ToolCall {
                    id: text_of(update, "toolCallId"),
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
        let event = event.unwrap_or_else(|| named(kind.unwrap_or("session/update")));
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

    fn read_response(&mut self, message: &Value, wall_ns: u64) -> Reading {
        let at = |event| stamped(Some(wall_ns), event);
        let Some(id) = message.get("id").and_then(Value::as_u64) else {
            return Reading::nothing();
        };
        let Some(position) = self.awaiting.iter().position(|(awaited, _)| *awaited == id) else {
            return Reading::nothing();
        };
        let (_, awaiting) = self.awaiting.remove(position);
        if awaiting == Awaiting::Prompt {
            return Reading::events(self.end_turn(message, wall_ns));
        }
        let failed = message.get("error").is_some();
        let result = message.get("result").filter(|_| !failed);
        match (awaiting, result) {
            (Awaiting::Initialize, Some(_)) => {
                let servers: Vec<Value> = self
                    .options
                    .mcp_servers
                    .iter()
                    .map(McpServer::to_json)
                    .collect();
                Reading {
                    events: Vec::new(),
                    write: Some(Outgoing::Request {
                        method: "session/new",
                        params: json!({
                            "cwd": self.options.cwd.to_string_lossy(),
                            "mcpServers": servers
                        }),
                    }),
                }
            }
            (_, Some(result)) => match result.get("sessionId").and_then(Value::as_str) {
                Some(session_id) => {
                    self.session_id = Some(session_id.to_string());
                    Reading {
                        events: vec![at(AgentEvent::SessionBound {
                            id: session_id.into(),
                        })],
                        write: Some(Outgoing::Request {
                            method: "session/prompt",
                            params: json!({
                                "sessionId": session_id,
                                "prompt": [{ "type": "text", "text": self.options.first_prompt }]
                            }),
                        }),
                    }
                }
                None => self.handshake_ended("malformed_response", wall_ns),
            },
            // An error, or an answer with no result: nothing more can be written, and the agent cannot be used.
            (Awaiting::Initialize, None) => self.handshake_ended(
                if failed {
                    "initialize_error"
                } else {
                    "malformed_response"
                },
                wall_ns,
            ),
            (_, None) => self.handshake_ended(
                if failed {
                    "session_new_error"
                } else {
                    "malformed_response"
                },
                wall_ns,
            ),
        }
    }

    /// The handshake got an answer that no turn can follow. Nothing more is written, and `finish` says so.
    fn handshake_ended(&mut self, name: &str, wall_ns: u64) -> Reading {
        self.handshake_failed = true;
        Reading::events(vec![stamped(Some(wall_ns), named(name))])
    }

    fn end_turn(&mut self, message: &Value, wall_ns: u64) -> Vec<StampedEvent> {
        let at = |event| stamped(Some(wall_ns), event);
        let mut events = Vec::new();
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
        self.last_turn = Some(reason.clone());
        events.push(at(AgentEvent::TurnEnd {
            reason,
            cancel_requested: self.cancel_requested,
            empty: false,
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
                // An answer to a request of the agent: it is no longer waiting.
                let id = id.ok_or("a message with neither a method nor an id")?;
                self.held.retain(|held| &held.raw_id != id);
            }
        }
        Ok(())
    }

    fn read_line(&mut self, line: &[u8], wall_ns: u64) -> Step {
        Step {
            events: self.read(line, wall_ns).events,
            writes: Vec::new(),
        }
    }
}

impl AgentSession for AcpSession {
    fn start(&mut self) -> Step {
        // Only the `session` flags: no `fs`, `terminal` or `elicitation`, so the agent keeps its own tools.
        let write = self.request(
            "initialize",
            json!({
                "protocolVersion": 1,
                "clientCapabilities": {
                    "session": { "notices": {}, "compaction": {}, "configOptions": {} }
                }
            }),
        );
        Step {
            events: Vec::new(),
            writes: vec![write],
        }
    }

    fn feed(&mut self, line: &[u8], wall_ns: u64) -> Step {
        let reading = self.read(line, wall_ns);
        let writes = reading
            .write
            .map(|outgoing| match outgoing {
                Outgoing::Request { method, params } => self.request(method, params),
                Outgoing::Reply(message) => self.write(message),
            })
            .into_iter()
            .collect();
        Step {
            events: reading.events,
            writes,
        }
    }

    fn send_turn(&mut self, text: &str) -> Step {
        let Some(session_id) = self.session_id.clone() else {
            // There is no session to prompt yet. Say so, rather than lose the turn without a trace.
            return Step {
                events: vec![stamped(None, named("turn_not_sent"))],
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
            (None, _) => PhaseOutcome::Lost,
            (Some(_), _) if self.handshake_failed => PhaseOutcome::Failed {
                cause: PhaseFailure::Api { status: None },
            },
            (Some(_), Some(reason)) => outcome_of(reason),
            (Some(_), None) => PhaseOutcome::Lost,
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

/// What a phase came to, from how its last turn ended.
fn outcome_of(reason: &TurnEndReason) -> PhaseOutcome {
    let failed = |cause| PhaseOutcome::Failed { cause };
    match reason {
        TurnEndReason::Cancelled {} => PhaseOutcome::Stopped,
        TurnEndReason::EndTurn {} => PhaseOutcome::Ended,
        TurnEndReason::MaxTokens {} => failed(PhaseFailure::MaxTokens),
        TurnEndReason::MaxTurns {} => failed(PhaseFailure::MaxTurns),
        TurnEndReason::Budget {} => failed(PhaseFailure::Budget),
        TurnEndReason::Refusal {} => failed(PhaseFailure::Refusal),
        TurnEndReason::Error { status } => failed(PhaseFailure::Api { status: *status }),
        TurnEndReason::Blocked { reason } => failed(PhaseFailure::Blocked {
            reason: reason.clone(),
        }),
    }
}

fn stamped(wall_ns: Option<u64>, event: AgentEvent) -> StampedEvent {
    StampedEvent { wall_ns, event }
}

/// An event that has a name and nothing else: what the session did not map. The name comes from the agent, so it is
/// cut short.
fn named(name: &str) -> AgentEvent {
    AgentEvent::Other {
        raw_type: name.chars().take(NAME_LIMIT).collect(),
        raw: None,
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

/// The kind of a tool call is the agent's `kind`, never its title: a title changes from the tool's name to its path
/// while the call runs (S7, O12).
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

/// The input of a tool call, or `None` while the agent has not sent it: the agent sends `rawInput: {}` for a Bash
/// call before the host answers (S7, O5).
fn known_input(raw: Option<&Value>) -> Option<Value> {
    match raw {
        None | Some(Value::Null) => None,
        Some(Value::Object(map)) if map.is_empty() => None,
        Some(value) => Some(value.clone()),
    }
}
