//! The ACP session: an [`AgentSession`] that is the client of an ACP agent (0.3, Two core traits; S7, the `opencode
//! acp` run). It is sans-IO, like every session: it reads a line and a stored time and returns events and the bytes to
//! write. It began as the scripted double of `core` (`collide_core::testing::acp`), which stays `core`'s own test
//! double; the shared contract (`collide_core::testing::contract`) keeps the two in step.
//!
//! The session holds what an ACP conversation needs held:
//!
//! - **the handshake:** `start` writes `initialize`, with `clientInfo` (`collide` and this crate's version). Its
//!   response makes the session write `session/new`; that response gives `SessionBound` and makes it write the first
//!   `session/prompt`. The first prompt is the session's own data ([`AcpOptions`]), not a turn queued by `send_turn`,
//!   so a session rebuilt from the journal still has it;
//! - **the model:** when the request names one, the first prompt waits for it (see below);
//! - **request ids:** the requests it writes count from 0. A response is matched to its request by id, and one that
//!   matches nothing changes nothing;
//! - **the turn:** the `session/prompt` response ends it. A turn the host cancelled is `Cancelled` whatever
//!   `stopReason` says (S7, O1). The final report is rebuilt from the message chunks after the last tool call;
//! - **permission requests:** a `session/request_permission` is held, with the id the agent wrote it with, until
//!   `answer` or `cancel` answers it. The held requests are what [`AgentSession::pending_permissions`] gives, oldest
//!   first. They are rebuilt by a replay (a request in the journal with no answer after it), because the agent is a
//!   child of the shim, outlives the daemon, and keeps its request open across a restart.
//!
//! | agent writes (stdout) | the session reads | and writes |
//! |---|---|---|
//! | a response to `initialize` | nothing | `session/new` |
//! | a response to `session/new`, no model asked for, or it is current | `SessionBound` | the first `session/prompt` |
//! | a response to `session/new`, the model is offered | `SessionBound` | `session/set_config_option`, or `set_model` |
//! | a response to `session/new`, the model is not offered | `SessionBound`, `Other` `model_unavailable` | nothing |
//! | a response to the model request that shows the model set | nothing | the first `session/prompt` |
//! | a model response that is an error, or shows no model set | `Other`, and `finish` says `Failed` | nothing |
//! | a response to `initialize` or `session/new` that is an error | `Other`, and `finish` says `Failed` | nothing |
//! | `session/update` `agent_message_chunk` | `Message` | nothing |
//! | `session/update` `tool_call` | `ToolCall`, by its `kind` | nothing |
//! | any other `session/update` | `Other` named by its `sessionUpdate` | nothing |
//! | `session/request_permission` | `PermissionRequest`, held until `answer`, `cancel` or the turn's end | nothing |
//! | `session/request_permission` after `cancel`, in the open turn | `Other` | the outcome `cancelled` |
//! | (the host) `answer`, `cancel` | `PermissionUnanswerable` for an `Allow` with no one-time allow | the outcome |
//! | any other request (`fs/*`, `terminal/*`, `elicitation/*`, unknown) | `Other` named by its method | `-32601` |
//! | a response to `session/prompt` | `FinalReport` (if any words), `TurnEnd` | `cancelled` for each request held |
//! | a line that is not a JSON object | `Other` named `unparsed_line` | nothing |
//!
//! **The model** (0.3, Who picks what; S7, O8, O9, O11; [`super::model`]). The id is `ModelRef::adapter_id` in
//! [`AcpOptions::model`]. It is data for a JSON-RPC body and nowhere else. The models the agent offers are read from
//! the `session/new` response, from `configOptions` when it has a model option, else from the draft `models`. The id is
//! set only if the agent lists it, and only a request that names a model is acted on: the session never picks one by
//! default, so a hosted model the agent lists is never reached by accident. With `configOptions` the request is
//! `session/set_config_option`, believed only when its answer holds the id as the option's `currentValue`; with the
//! draft form it is `session/set_model`, and an answer that is not an error is enough. The first prompt is written when
//! that answer is read, never before. An id that is the current one writes nothing. An id the agent does not list, an
//! agent that lists none, and an answer that is an error or does not show the model set, write no prompt and make
//! `finish` say `PhaseFailure::ModelUnavailable`. The events are `Other`s named `model_unavailable` (it carries the
//! count of models offered, never the id), `model_set_error` and `model_not_set`. A `current_model_update` or a
//! `config_option_update` read later is an `Other`: the session does not follow an agent that changes its model.
//! `send_turn` writes nothing while the model request is out, and after it was refused.
//!
//! **No content from the agent is kept in an event of the `Other` kind.** A stream's content is untrusted and may be
//! large (0.3), and the journal already holds each line whole. An `Other` carries a name only, cut to 64 characters.
//!
//! The client advertises no `fs`, `terminal` or `elicitation` capability (0.3, Later adapters), so the agent keeps
//! its own file and shell tools and a daemon restart cannot stall a write. A request for one of them is answered
//! "method not found", so the agent never waits on it.
//!
//! **Answering** (0.3, Permissions). `Allow` picks the first option of kind `allow_once`, and never `allow_always`,
//! which would change the agent's own standing rules. `Deny` picks `reject_once`; its message is dropped, because ACP
//! has no place for it. `Cancel` writes the outcome `cancelled`. An `Allow` with no `allow_once` option writes
//! `cancelled` and makes `PermissionUnanswerable`, so the host can tell the person why; a `Deny` with no `reject_once`
//! writes `cancelled` and says nothing more. An id the session does not hold, or has answered, writes nothing.
//!
//! **Cancel.** `cancel` writes the outcome `cancelled` for each pending request, then the notification
//! `session/cancel` (it has no id). It writes nothing before the session has an id, after the turn ended, or when the
//! turn is cancelled already. The turn ends when the `session/prompt` response comes, and is `Cancelled` whatever the
//! agent's stop reason says. A request the agent asks after the cancel, while the turn is open, is answered
//! `cancelled` at once and never held, with an `Other` named `permission_request_after_cancel` in place of the
//! `PermissionRequest`, so no one is asked: the agent may wait on the answer, and the turn could not end.
//!
//! **A turn end clears what is held.** When the `session/prompt` response is read, each request still held is answered
//! `cancelled` and dropped, before the `TurnEnd`, so `pending_permissions` is empty after it and a later `answer` is
//! for an unknown id. The agent is never left waiting on Collide, as for the `-32601` answer. A replay drops the same
//! requests when it reads the response, and adds no write: the journal holds the answer or does not.
//!
//! **The kind of a request** is its `toolCall.kind`; when that has none, the kind of the `tool_call` announced earlier
//! in the turn with the same id; else `other`. The agent may ask before it announces the call (S7, O6), and a call
//! announced after the request changes nothing already given.
//!
//! Left to the item after this one, which hardens what the double had: empty turns, lost children and usage
//! (`ts-0091-pz`). Until then `TurnEnd.empty` is `false`, and no `Usage` event is made.
//!
//! `PermissionRequest` carries no paths (`core/src/agent.rs`); ACP's `toolCall.locations` has them and is left out.

use super::model::{self, ModelForm, ModelPlan};
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

/// What the client calls itself in `initialize`. The same for every agent, so it is not in a profile.
const CLIENT_NAME: &str = "collide";

/// The longest name an `Other` event carries, in characters.
const NAME_LIMIT: usize = 64;

/// What a request the session wrote is waiting to be answered as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Awaiting {
    Initialize,
    SessionNew,
    /// `session/set_config_option` or `session/set_model`, by the form the model came from.
    Model(ModelForm),
    Prompt,
}

/// A request of the agent that no one has answered. `raw_id` is the JSON-RPC id as the agent wrote it, which the
/// answer must echo (a number stays a number); `request.id` is its text.
#[derive(Debug, Clone, PartialEq)]
struct HeldRequest {
    raw_id: Value,
    request: PermissionRequest,
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
    writes: Vec<Outgoing>,
}

impl Reading {
    fn events(events: Vec<StampedEvent>) -> Self {
        Self {
            events,
            writes: Vec::new(),
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
    /// The kind of each tool call announced in the turn, to give a request that names a call but no kind.
    tool_kinds: Vec<(String, ToolKind)>,
    cancel_requested: bool,
    turn_report: String,
    handshake_failed: bool,
    /// The model the request named cannot be had, so no prompt was or will be written.
    model_refused: bool,
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
            tool_kinds: Vec::new(),
            cancel_requested: false,
            turn_report: String::new(),
            handshake_failed: false,
            model_refused: false,
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
            Some("session/request_permission") if has_id => self.read_request(&message, wall_ns),
            // A request the engine does not serve: tell the agent at once, so it never waits on the answer.
            Some(method) if has_id => Reading {
                events: vec![at(named(method))],
                writes: vec![Outgoing::Reply(json!({
                    "jsonrpc": "2.0", "id": message["id"],
                    "error": { "code": METHOD_NOT_FOUND, "message": "Method not found" }
                }))],
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
                let id = text_of(update, "toolCallId");
                let kind = tool_kind(update.get("kind"));
                self.tool_kinds.retain(|(known, _)| *known != id);
                self.tool_kinds.push((id.clone(), kind));
                Some(AgentEvent::ToolCall {
                    id,
                    tool: update
                        .get("title")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    kind,
                    input: known_input(update.get("rawInput")),
                })
            }
            _ => None,
        };
        let event = event.unwrap_or_else(|| named(kind.unwrap_or("session/update")));
        vec![stamped(Some(wall_ns), event)]
    }

    /// A request for permission. While the turn is cancelled and open the agent may be waiting on it, and cancel has
    /// already answered every request it knew, so this one is answered `cancelled` at once and never held: the turn
    /// could not end otherwise. The event is an `Other`, not a `PermissionRequest`, so no one is asked.
    fn read_request(&mut self, message: &Value, wall_ns: u64) -> Reading {
        let at = |event| stamped(Some(wall_ns), event);
        if self.cancel_requested && self.turn_open() {
            return Reading {
                events: vec![at(named("permission_request_after_cancel"))],
                writes: vec![Outgoing::Reply(cancelled_reply(&message["id"]))],
            };
        }
        Reading::events(vec![at(self.hold_request(message))])
    }

    /// Whether the request that sets the model is waiting for its answer.
    fn model_pending(&self) -> bool {
        self.awaiting
            .iter()
            .any(|(_, awaiting)| matches!(awaiting, Awaiting::Model(_)))
    }

    /// Whether a `session/prompt` is waiting for its response.
    fn turn_open(&self) -> bool {
        self.awaiting
            .iter()
            .any(|(_, awaiting)| *awaiting == Awaiting::Prompt)
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
        let tool_call_id = text_of(&call, "toolCallId");
        let request = PermissionRequest {
            id: id.clone(),
            kind: self.request_kind(&call, &tool_call_id),
            tool_call_id,
            tool: call
                .get("title")
                .and_then(Value::as_str)
                .map(str::to_string),
            input: known_input(call.get("rawInput")),
            reason: None,
            options,
        };
        self.held.retain(|held| held.request.id != id);
        self.held.push(HeldRequest {
            raw_id,
            request: request.clone(),
        });
        AgentEvent::PermissionRequest { request }
    }

    /// The kind of the tool a request is for: its own `kind`, else the kind of the `tool_call` the agent announced
    /// earlier with the same id, else `other`. The agent may ask before it announces the call (S7, O6), and a call
    /// announced after the request changes nothing already given.
    fn request_kind(&self, call: &Value, tool_call_id: &str) -> ToolKind {
        if call.get("kind").is_some_and(Value::is_string) {
            return tool_kind(call.get("kind"));
        }
        self.tool_kinds
            .iter()
            .find(|(known, _)| known == tool_call_id)
            .map_or(ToolKind::Other, |(_, kind)| *kind)
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
            return self.end_turn(message, wall_ns);
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
                    writes: vec![Outgoing::Request {
                        method: "session/new",
                        params: json!({
                            "cwd": self.options.cwd.to_string_lossy(),
                            "mcpServers": servers
                        }),
                    }],
                }
            }
            (Awaiting::Model(form), result) => self.model_answered(form, result, wall_ns),
            (_, Some(result)) => match result.get("sessionId").and_then(Value::as_str) {
                Some(session_id) => {
                    self.session_id = Some(session_id.to_string());
                    let mut reading = Reading::events(vec![at(AgentEvent::SessionBound {
                        id: session_id.into(),
                    })]);
                    match model::plan(result, session_id, self.options.model.as_deref()) {
                        ModelPlan::Keep => reading.writes.push(self.first_prompt(session_id)),
                        ModelPlan::Set(request) => reading.writes.push(Outgoing::Request {
                            method: request.method,
                            params: request.params,
                        }),
                        ModelPlan::Unavailable { offered } => {
                            self.model_refused = true;
                            reading.events.push(at(AgentEvent::Other {
                                raw_type: "model_unavailable".into(),
                                raw: Some(json!({ "offered": offered })),
                            }));
                        }
                    }
                    reading
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

    /// The first prompt, as the session's own data has it.
    fn first_prompt(&self, session_id: &str) -> Outgoing {
        Outgoing::Request {
            method: "session/prompt",
            params: json!({
                "sessionId": session_id,
                "prompt": [{ "type": "text", "text": self.options.first_prompt }]
            }),
        }
    }

    /// The answer to the request that sets the model. Only an answer that shows the model set lets the first prompt
    /// go (S7, O8): an agent may answer and keep its own. Otherwise nothing more is written and `finish` says so.
    fn model_answered(&mut self, form: ModelForm, result: Option<&Value>, wall_ns: u64) -> Reading {
        let wanted = self.options.model.as_deref().unwrap_or_default();
        let refusal = match (form, result) {
            (_, None) => Some("model_set_error"),
            (ModelForm::ConfigOption, Some(result))
                if !model::config_option_holds(result, wanted) =>
            {
                Some("model_not_set")
            }
            _ => None,
        };
        if let Some(name) = refusal {
            self.model_refused = true;
            return Reading::events(vec![stamped(Some(wall_ns), named(name))]);
        }
        let Some(session_id) = self.session_id.clone() else {
            return Reading::nothing();
        };
        Reading {
            events: Vec::new(),
            writes: vec![self.first_prompt(&session_id)],
        }
    }

    /// The handshake got an answer that no turn can follow. Nothing more is written, and `finish` says so.
    fn handshake_ended(&mut self, name: &str, wall_ns: u64) -> Reading {
        self.handshake_failed = true;
        Reading::events(vec![stamped(Some(wall_ns), named(name))])
    }

    fn end_turn(&mut self, message: &Value, wall_ns: u64) -> Reading {
        let at = |event| stamped(Some(wall_ns), event);
        let mut events = Vec::new();
        // The agent no longer waits on a request still held, so each is answered `cancelled` and dropped, before the
        // turn is reported. A request that came back after a restart would be a stale ask.
        let writes = std::mem::take(&mut self.held)
            .into_iter()
            .map(|held| Outgoing::Reply(cancelled_reply(&held.raw_id)))
            .collect();
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
        Reading { events, writes }
    }
}

impl Replayable for AcpSession {
    fn note_written(&mut self, bytes: &[u8]) -> Result<(), String> {
        let message: Value = serde_json::from_slice(bytes)
            .map_err(|reason| format!("not a JSON-RPC message: {reason}"))?;
        let id = message.get("id");
        match message.get("method").and_then(Value::as_str) {
            Some(
                method @ ("initialize"
                | "session/new"
                | "session/set_config_option"
                | "session/set_model"
                | "session/prompt"),
            ) => {
                let id = id
                    .and_then(Value::as_u64)
                    .ok_or_else(|| format!("a `{method}` request with no numeric id"))?;
                let awaiting = match method {
                    "initialize" => Awaiting::Initialize,
                    "session/new" => Awaiting::SessionNew,
                    "session/set_config_option" => Awaiting::Model(ModelForm::ConfigOption),
                    "session/set_model" => Awaiting::Model(ModelForm::Draft),
                    _ => {
                        // A new turn: it has no words and no tool calls yet, and is not cancelled.
                        self.turn_report.clear();
                        self.tool_kinds.clear();
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
                "clientInfo": { "name": CLIENT_NAME, "version": env!("CARGO_PKG_VERSION") },
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
            .writes
            .into_iter()
            .map(|outgoing| match outgoing {
                Outgoing::Request { method, params } => self.request(method, params),
                Outgoing::Reply(message) => self.write(message),
            })
            .collect();
        Step {
            events: reading.events,
            writes,
        }
    }

    fn send_turn(&mut self, text: &str) -> Step {
        // No prompt goes out before the model is set, and none after it was refused.
        let session_id = self
            .session_id
            .clone()
            .filter(|_| !self.model_refused && !self.model_pending());
        let Some(session_id) = session_id else {
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
        let Some(held) = self
            .held
            .iter()
            .find(|held| held.request.id == request_id)
            .cloned()
        else {
            return Step::default();
        };
        let pick = |kind| {
            held.request
                .options
                .iter()
                .find(|option| option.kind == kind)
        };
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

    fn pending_permissions(&self) -> Vec<PermissionRequest> {
        self.held.iter().map(|held| held.request.clone()).collect()
    }

    fn cancel(&mut self) -> Step {
        // Nothing to cancel before the session has an id, after the turn ended, or once the turn is cancelled.
        let Some(session_id) = self.session_id.clone().filter(|_| self.turn_open()) else {
            return Step::default();
        };
        if self.cancel_requested {
            return Step::default();
        }
        // ACP: every request still pending is answered `cancelled`, before the turn is cancelled.
        let pending: Vec<Value> = self.held.iter().map(|held| held.raw_id.clone()).collect();
        let mut writes: Vec<Vec<u8>> = pending
            .iter()
            .map(|raw_id| self.respond(raw_id, json!({ "outcome": "cancelled" })))
            .collect();
        writes.push(self.write(json!({
            "jsonrpc": "2.0", "method": "session/cancel", "params": { "sessionId": session_id }
        })));
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
            (Some(_), _) if self.model_refused => PhaseOutcome::Failed {
                cause: PhaseFailure::ModelUnavailable,
            },
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

/// The answer to a request for permission that picks nothing: the outcome `cancelled`, with the agent's own id.
fn cancelled_reply(raw_id: &Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": raw_id, "result": { "outcome": { "outcome": "cancelled" } } })
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
