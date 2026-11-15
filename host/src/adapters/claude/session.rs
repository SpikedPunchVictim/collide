//! The Claude session (0.3, Two core traits; The stream protocol): one per child, a sans-IO state machine. It holds what
//! a Claude child needs remembered: the permission requests waiting for an answer, whether a cancel was sent, and how
//! the last turn ended. `feed` is the stream parser measured by S1 and S9. The session reads no clock and opens no file;
//! the same input gives the same output, so a daemon restart rebuilds it from the shim's journal with `replay`.
//!
//! Every turn, the first included, is a `user` message on stdin, so `start` writes nothing: it reports the session id
//! at once, because the launch line passed it (`--session-id`, or `--resume`).
//!
//! What a line of the child's stdout becomes (`feed`; every event is stamped with the line's `wall_ns`):
//!
//! | the child writes | the session reads |
//! |---|---|
//! | `system/init` | nothing: the id was bound at `start`, and `init` comes again on every turn |
//! | `assistant` | a `Message` for each non-empty text block, a `ToolCall` for each `tool_use`; thinking is not an event |
//! | `control_request` `can_use_tool` | a `PermissionRequest`, kept until `answer` |
//! | `result` | a `FinalReport` from `result.result`, then a `TurnEnd` |
//! | anything else | an `Other` named by the line's `type`, and its `subtype` when it has one; no content is kept |
//!
//! What the session writes (stdin), for the calls that write:
//!
//! | call | the message |
//! |---|---|
//! | `send_turn` | `{"type":"user","message":{"role":"user","content":[{"type":"text","text":...}]},...}` |
//! | `answer` | `{"type":"control_response","response":{"subtype":"success","request_id":...,"response":{"behavior":...}}}` |
//! | `cancel` | `{"type":"control_request","request_id":"collide-interrupt-N","request":{"subtype":"interrupt"}}` |
//!
//! A live write goes through [`Replayable::note_written`], the same code a replay runs over an `in` record, so a rebuilt
//! session and a live one reach the same state. A replay is [`replay_records`] over this session's [`Replayable`].

use collide_core::adapter::{
    AgentSession, ExitInfo, Finished, PhaseFailure, PhaseOutcome, ReplayError, StampedEvent, Step,
};
use collide_core::agent::{
    AgentEvent, FinalReportSource, PermissionReply, PermissionRequest, SessionStart, ToolKind,
    TurnEndReason,
};
use collide_core::journal::JournalRecord;
use collide_core::replay::{Replayable, replay_records};
use serde_json::{Value, json};

/// The message of a `deny` that carries none: Claude requires one.
const DENY_WITHOUT_MESSAGE: &str = "The host denied this tool call.";

/// The message of a `Cancel` answer. Claude has no cancel answer; this is a deny that says so, and the turn goes on.
/// `cancel()` is what interrupts it.
const CANCELLED_REQUEST: &str = "The host cancelled this request.";

/// A permission request that has been asked and not answered. `input` is what an `allow` echoes back as
/// `updatedInput`, and is `None` when the request carried none.
#[derive(Debug, Clone, PartialEq)]
struct Pending {
    id: String,
    input: Option<Value>,
}

/// One Claude child's conversation.
#[derive(Debug, Clone, PartialEq)]
pub struct ClaudeSession {
    start: SessionStart,
    pending: Vec<Pending>,
    cancel_requested: bool,
    interrupts_written: u64,
    /// Whether the turn in flight has given a message or a tool call.
    gave_output: bool,
    /// How the last turn ended; `None` until one does, and again once a new turn is sent.
    last_turn: Option<TurnEndReason>,
}

impl ClaudeSession {
    /// A session that starts as `start` says: Claude supports `Assign` (a UUID Collide picked) and `Resume`.
    pub fn new(start: SessionStart) -> Self {
        Self {
            start,
            pending: Vec::new(),
            cancel_requested: false,
            interrupts_written: 0,
            gave_output: false,
            last_turn: None,
        }
    }

    /// How the session started.
    pub fn session_start(&self) -> &SessionStart {
        &self.start
    }

    /// Learns a message as a replay would, then returns it as the bytes for the child.
    fn write(&mut self, message: Value) -> Vec<u8> {
        let bytes = json_line(&message);
        self.note_written(&bytes)
            .expect("a session reads its own writes");
        bytes
    }

    fn read_assistant(&mut self, value: &Value, wall_ns: u64) -> Vec<StampedEvent> {
        let Some(blocks) = value.pointer("/message/content").and_then(Value::as_array) else {
            return vec![other("assistant", wall_ns)];
        };
        let mut events = Vec::new();
        for block in blocks {
            let event = match block.get("type").and_then(Value::as_str) {
                Some("text") => match block.get("text").and_then(Value::as_str) {
                    Some(text) if !text.is_empty() => {
                        self.gave_output = true;
                        AgentEvent::Message { text: text.into() }
                    }
                    _ => continue,
                },
                Some("tool_use") => match tool_call(block) {
                    Some(call) => {
                        self.gave_output = true;
                        call
                    }
                    None => other_event("assistant/tool_use"),
                },
                Some("thinking" | "redacted_thinking") => continue,
                Some(kind) => other_event(&format!("assistant/{kind}")),
                None => other_event("assistant"),
            };
            events.push(stamped(Some(wall_ns), event));
        }
        events
    }

    fn read_control_request(&mut self, value: &Value, wall_ns: u64) -> Vec<StampedEvent> {
        let request = value.get("request");
        let subtype = request
            .and_then(|r| r.get("subtype"))
            .and_then(Value::as_str);
        let id = value.get("request_id").and_then(Value::as_str);
        let (Some("can_use_tool"), Some(id), Some(request)) = (subtype, id, request) else {
            let name = match subtype {
                Some(subtype) => format!("control_request/{subtype}"),
                None => "control_request".to_string(),
            };
            return vec![other(&name, wall_ns)];
        };
        let request = permission_request(id, request);
        self.pending.retain(|pending| pending.id != request.id);
        self.pending.push(Pending {
            id: request.id.clone(),
            input: request.input.clone(),
        });
        vec![stamped(
            Some(wall_ns),
            AgentEvent::PermissionRequest { request },
        )]
    }

    fn read_result(&mut self, value: &Value, wall_ns: u64) -> Vec<StampedEvent> {
        let failed = is_failed_result(value);
        let reason = if self.cancel_requested {
            TurnEndReason::Cancelled {}
        } else {
            turn_end_reason(value, failed)
        };
        let mut events = Vec::new();
        let report = value
            .get("result")
            .and_then(Value::as_str)
            .filter(|text| !failed && !text.is_empty());
        if let Some(text) = report {
            events.push(AgentEvent::FinalReport {
                text: text.into(),
                source: FinalReportSource::ResultField,
            });
        }
        events.push(AgentEvent::TurnEnd {
            reason: reason.clone(),
            cancel_requested: self.cancel_requested,
            empty: !self.gave_output && usage_tokens(value) == 0,
        });
        self.gave_output = false;
        self.last_turn = Some(reason);
        events
            .into_iter()
            .map(|event| stamped(Some(wall_ns), event))
            .collect()
    }
}

impl AgentSession for ClaudeSession {
    fn start(&mut self) -> Step {
        let id = match &self.start {
            SessionStart::Assign { uuid } => Some(uuid.clone()),
            SessionStart::Resume { id } => Some(id.clone()),
            // Claude cannot name its own session; launch refuses this start, so there is nothing to bind.
            SessionStart::AgentAssigned {} => None,
        };
        Step {
            events: id
                .map(|id| stamped(None, AgentEvent::SessionBound { id }))
                .into_iter()
                .collect(),
            writes: Vec::new(),
        }
    }

    fn feed(&mut self, line: &[u8], wall_ns: u64) -> Step {
        self.read_line(line, wall_ns)
    }

    fn send_turn(&mut self, text: &str) -> Step {
        let write = self.write(json!({
            "type": "user",
            "message": { "role": "user", "content": [{ "type": "text", "text": text }] },
            "parent_tool_use_id": null,
            "session_id": "",
        }));
        written(write)
    }

    fn answer(&mut self, request_id: &str, reply: PermissionReply) -> Step {
        let Some(pending) = self.pending.iter().find(|pending| pending.id == request_id) else {
            return Step::default();
        };
        let response = match reply {
            PermissionReply::Allow {} => {
                let mut allow = json!({ "behavior": "allow" });
                // An allow with an input replaces the call's input, so one that was never known is left out,
                // never sent as an empty object.
                if let Some(input) = &pending.input {
                    allow["updatedInput"] = input.clone();
                }
                allow
            }
            PermissionReply::Deny { message } => json!({
                "behavior": "deny",
                "message": message.as_deref().unwrap_or(DENY_WITHOUT_MESSAGE),
            }),
            PermissionReply::Cancel {} => {
                json!({ "behavior": "deny", "message": CANCELLED_REQUEST })
            }
        };
        let write = self.write(json!({
            "type": "control_response",
            "response": { "subtype": "success", "request_id": request_id, "response": response },
        }));
        written(write)
    }

    fn cancel(&mut self) -> Step {
        let write = self.write(json!({
            "type": "control_request",
            "request_id": format!("collide-interrupt-{}", self.interrupts_written + 1),
            "request": { "subtype": "interrupt" },
        }));
        written(write)
    }

    fn finish(&mut self, exit: &ExitInfo) -> Finished {
        let outcome = match (&exit.record, &self.last_turn) {
            (None, _) => PhaseOutcome::Lost,
            (Some(_), None) => PhaseOutcome::Failed {
                cause: PhaseFailure::NoResult,
            },
            (Some(_), Some(reason)) => outcome_of_turn(reason),
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

impl Replayable for ClaudeSession {
    fn note_written(&mut self, bytes: &[u8]) -> Result<(), String> {
        let message: Value = serde_json::from_slice(bytes)
            .map_err(|reason| format!("not a JSON message: {reason}"))?;
        match message.get("type").and_then(Value::as_str) {
            // A new turn is a new thing to cancel, and has not ended.
            Some("user") => {
                self.cancel_requested = false;
                self.gave_output = false;
                self.last_turn = None;
            }
            Some("control_response") => {
                let id = message
                    .pointer("/response/request_id")
                    .and_then(Value::as_str)
                    .ok_or("a control response with no `request_id`")?;
                self.pending.retain(|pending| pending.id != id);
            }
            Some("control_request")
                if message.pointer("/request/subtype").and_then(Value::as_str)
                    == Some("interrupt") =>
            {
                self.cancel_requested = true;
                self.interrupts_written += 1;
            }
            other => return Err(format!("not a message this session writes: type {other:?}")),
        }
        Ok(())
    }

    fn read_line(&mut self, line: &[u8], wall_ns: u64) -> Step {
        let events = match serde_json::from_slice::<Value>(line) {
            Ok(value) => match value.get("type").and_then(Value::as_str) {
                Some("assistant") => self.read_assistant(&value, wall_ns),
                Some("control_request") => self.read_control_request(&value, wall_ns),
                Some("result") => self.read_result(&value, wall_ns),
                Some("system") if subtype(&value) == Some("init") => Vec::new(),
                Some(kind) => vec![other(&type_name(kind, &value), wall_ns)],
                None => vec![other("unparsed_line", wall_ns)],
            },
            Err(_) => vec![other("unparsed_line", wall_ns)],
        };
        Step {
            events,
            writes: Vec::new(),
        }
    }
}

fn json_line(value: &Value) -> Vec<u8> {
    let mut bytes = serde_json::to_vec(value).expect("a JSON value serializes");
    bytes.push(b'\n');
    bytes
}

fn written(bytes: Vec<u8>) -> Step {
    Step {
        events: Vec::new(),
        writes: vec![bytes],
    }
}

fn stamped(wall_ns: Option<u64>, event: AgentEvent) -> StampedEvent {
    StampedEvent { wall_ns, event }
}

/// An `Other` that keeps no content: the log must not hold a hook's output or a prompt that the stream carried, and the
/// journal has the line whole.
fn other_event(raw_type: &str) -> AgentEvent {
    AgentEvent::Other {
        raw_type: raw_type.into(),
        raw: None,
    }
}

fn other(raw_type: &str, wall_ns: u64) -> StampedEvent {
    stamped(Some(wall_ns), other_event(raw_type))
}

fn subtype(value: &Value) -> Option<&str> {
    value.get("subtype").and_then(Value::as_str)
}

/// `type`, and `type/subtype` for a line that has a subtype (`system/status`).
fn type_name(kind: &str, value: &Value) -> String {
    match subtype(value) {
        Some(subtype) => format!("{kind}/{subtype}"),
        None => kind.to_string(),
    }
}

/// What a tool call does, by the tool's name: Claude's own tools are named for what they do, and an MCP tool or one
/// this version does not know is `Other`.
fn tool_kind(name: &str) -> ToolKind {
    match name {
        "Read" => ToolKind::Read,
        "Grep" | "Glob" => ToolKind::Search,
        "Edit" | "Write" | "MultiEdit" | "NotebookEdit" => ToolKind::Edit,
        "Bash" => ToolKind::Execute,
        "WebFetch" | "WebSearch" => ToolKind::Fetch,
        _ => ToolKind::Other,
    }
}

fn tool_call(block: &Value) -> Option<AgentEvent> {
    let id = block.get("id").and_then(Value::as_str)?;
    let tool = block.get("name").and_then(Value::as_str);
    Some(AgentEvent::ToolCall {
        id: id.into(),
        tool: tool.map(String::from),
        kind: tool.map_or(ToolKind::Other, tool_kind),
        input: block.get("input").filter(|input| !input.is_null()).cloned(),
    })
}

/// The request as the host sees it. `input` is `None` when the request has none. `reason` is the hook's own text for
/// a hook that asked (S9); the agent's own options are none, so no answer offers an "always" (0.3, Permissions).
fn permission_request(id: &str, request: &Value) -> PermissionRequest {
    let tool = request.get("tool_name").and_then(Value::as_str);
    PermissionRequest {
        id: id.into(),
        tool_call_id: request
            .get("tool_use_id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .into(),
        tool: tool.map(String::from),
        kind: tool.map_or(ToolKind::Other, tool_kind),
        input: request
            .get("input")
            .filter(|input| !input.is_null())
            .cloned(),
        reason: request
            .get("decision_reason")
            .and_then(Value::as_str)
            .map(String::from),
        options: Vec::new(),
    }
}

/// A result that is an error: `is_error`, or a subtype other than `success`. Its `result` is never the phase's report
/// (S1: a `result/success` with `is_error` is an API failure).
fn is_failed_result(result: &Value) -> bool {
    let error = result
        .get("is_error")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    error || subtype(result).is_some_and(|subtype| subtype != "success")
}

/// The agent's own reason for the turn's end. A finer reading of an error result (a usage limit, a blocked prompt, a
/// spent budget) is the parser's later work.
fn turn_end_reason(result: &Value, failed: bool) -> TurnEndReason {
    if failed {
        return match subtype(result) {
            Some("error_max_turns") => TurnEndReason::MaxTurns {},
            _ => TurnEndReason::Error {
                status: result
                    .get("api_error_status")
                    .and_then(Value::as_u64)
                    .and_then(|status| u16::try_from(status).ok()),
            },
        };
    }
    match result.get("stop_reason").and_then(Value::as_str) {
        Some("max_tokens") => TurnEndReason::MaxTokens {},
        Some("refusal") => TurnEndReason::Refusal {},
        _ => TurnEndReason::EndTurn {},
    }
}

/// Every token `result.usage` counts. A result with no `usage`, or with all of it 0, has none.
fn usage_tokens(result: &Value) -> u64 {
    [
        "input_tokens",
        "output_tokens",
        "cache_read_input_tokens",
        "cache_creation_input_tokens",
    ]
    .iter()
    .filter_map(|key| {
        result
            .pointer(&format!("/usage/{key}"))
            .and_then(Value::as_u64)
    })
    .sum()
}

/// What a phase came to, from how its last turn ended. An empty `end_turn` is not a failure here: Claude's protocol
/// has a failure flag, so only an adapter without one fails an empty turn (0.3, The stream protocol).
fn outcome_of_turn(reason: &TurnEndReason) -> PhaseOutcome {
    let failed = |cause| PhaseOutcome::Failed { cause };
    match reason {
        TurnEndReason::Cancelled {} => PhaseOutcome::Stopped,
        TurnEndReason::EndTurn {} => PhaseOutcome::Ended,
        TurnEndReason::MaxTokens {} => failed(PhaseFailure::MaxTokens),
        TurnEndReason::MaxTurns {} => failed(PhaseFailure::MaxTurns),
        TurnEndReason::Refusal {} => failed(PhaseFailure::Refusal),
        TurnEndReason::Error { status } => failed(PhaseFailure::Api { status: *status }),
        TurnEndReason::Blocked { reason } => failed(PhaseFailure::Blocked {
            reason: reason.clone(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_session_remembers_how_it_started() {
        let session = ClaudeSession::new(SessionStart::Resume { id: "abc".into() });
        assert_eq!(
            session.session_start(),
            &SessionStart::Resume { id: "abc".into() }
        );
    }

    #[test]
    fn replaying_an_empty_journal_gives_an_empty_step() {
        let mut session = ClaudeSession::new(SessionStart::Assign { uuid: "u".into() });
        assert_eq!(session.replay(&[]), Ok(Step::default()));
    }

    #[test]
    fn a_message_the_session_never_writes_is_not_readable_as_one_it_wrote() {
        let mut session = ClaudeSession::new(SessionStart::Assign { uuid: "u".into() });
        assert!(
            session
                .note_written(b"{\"type\":\"permission_reply\"}\n")
                .is_err()
        );
        assert!(session.note_written(b"not json\n").is_err());
        assert!(
            session
                .note_written(b"{\"type\":\"control_response\",\"response\":{}}\n")
                .is_err()
        );
        assert!(
            session
                .note_written(
                    b"{\"type\":\"control_request\",\"request\":{\"subtype\":\"initialize\"}}\n"
                )
                .is_err()
        );
    }

    #[test]
    fn usage_tokens_adds_the_four_counts_and_ignores_the_rest() {
        let result = json!({"usage": {"input_tokens": 1, "output_tokens": 2, "cache_read_input_tokens": 3,
                                      "cache_creation_input_tokens": 4, "service_tier": "standard"}});
        assert_eq!(usage_tokens(&result), 10);
        assert_eq!(usage_tokens(&json!({})), 0);
    }
}
