//! The fake adapter and its session. The adapter is a tool whose child writes normalized events: one
//! [`AgentEvent`] as JSON on each line. An engine test writes its run as a list of events, takes the lines with
//! [`FakeAdapter::script_lines`] and feeds them to the session the adapter built, so the engine sees the events it
//! would see from a real tool, with no process (0.15, the fake adapter).
//!
//! | the child writes (stdout) | the session reads |
//! |---|---|
//! | an event as JSON, such as `{"type":"message","text":"hi"}` | that event, stamped with the line's `wall_ns` |
//! | anything else | an `Other` event named `unparsed_line` |
//!
//! | the session writes (stdin) | when |
//! |---|---|
//! | `{"type":"user_turn","text":...}` | `send_turn` |
//! | `{"type":"permission_reply","request_id":...,"reply":...}` | `answer` |
//! | `{"type":"interrupt"}` | `cancel` |
//!
//! Like a Claude session, it writes nothing at `start`: the launch line carries the first turn.

use super::contract::ChildDialect;
use super::{json_line, outcome_of_turn, stamped, unparsed_line};
use crate::adapter::{
    AgentAdapter, AgentSession, CommandSpec, ConfigPath, ConfigRoot, EnvStrip, ExitInfo, Finished,
    LaunchError, LaunchRequest, LaunchSpec, PhaseFailure, PhaseOutcome, ReplayError, SessionRef,
    Step,
};
use crate::agent::{
    AgentEvent, Capabilities, McpServers, ModelRef, PermissionHost, PermissionReply, SessionStart,
    Support, ToolKind, TurnEndReason, UsageReport,
};
use crate::journal::JournalRecord;
use crate::pack::Role;
use crate::replay::{Replayable, replay_records};
use crate::workflow::{AgentPhase, OnCold, Prompt};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// The capability table of the fake: what a test most often needs a tool to promise. A capability the fake does not
/// speak for stays `Unknown`, as an unprobed adapter's does.
fn default_capabilities() -> Capabilities {
    Capabilities {
        headless_stream: Support::Yes,
        permission_host: PermissionHost {
            support: Support::Yes,
            deny_message: Support::Yes,
            input_at_request: Support::Yes,
            ..Default::default()
        },
        usage_report: UsageReport {
            support: Support::Yes,
            ..Default::default()
        },
        take_over: Support::Yes,
        mcp_servers: McpServers {
            support: Support::Yes,
            stdio: Support::Yes,
            ..Default::default()
        },
        ..Default::default()
    }
}

/// A tool that runs from a script of normalized events. It has no state, like every adapter: `script` is the
/// events the child will write, and each session it builds starts clean.
#[derive(Debug, Clone)]
pub struct FakeAdapter {
    capabilities: Capabilities,
    script: Vec<AgentEvent>,
}

impl FakeAdapter {
    /// An adapter whose child writes `script`, with the fake's default capabilities.
    pub fn new(script: Vec<AgentEvent>) -> Self {
        Self {
            capabilities: default_capabilities(),
            script,
        }
    }

    /// The same adapter with this capability table, so a test can make the tool promise less or more.
    pub fn with_capabilities(mut self, capabilities: Capabilities) -> Self {
        self.capabilities = capabilities;
        self
    }

    /// The script as the child writes it: one line for each event, in order.
    pub fn script_lines(&self) -> Vec<Vec<u8>> {
        self.script
            .iter()
            .map(|event| json_line(&serde_json::to_value(event).expect("an event serializes")))
            .collect()
    }
}

impl AgentAdapter for FakeAdapter {
    fn capabilities(&self) -> Capabilities {
        self.capabilities.clone()
    }

    fn launch(&self, request: &LaunchRequest) -> Result<LaunchSpec, LaunchError> {
        Ok(LaunchSpec {
            argv: vec![
                "fake-agent".into(),
                "--role".into(),
                request.role_name.clone(),
            ],
            env: BTreeMap::new(),
            env_strip: EnvStrip::default(),
            cwd: request.cwd.clone(),
        })
    }

    fn session(&self, _request: &LaunchRequest) -> Box<dyn AgentSession + Send> {
        Box::new(NormalizedSession::new())
    }

    fn config_paths(&self) -> Vec<ConfigPath> {
        vec![ConfigPath {
            root: ConfigRoot::Repo,
            pattern: ".fake-agent/**".into(),
        }]
    }

    fn interactive_command(&self, session: &SessionRef) -> Option<CommandSpec> {
        (self.capabilities.take_over == Support::Yes).then(|| CommandSpec {
            argv: vec!["fake-agent".into(), "--resume".into(), session.id.clone()],
            cwd: session.cwd.clone(),
        })
    }
}

/// A launch request that needs nothing from the test: the role `demo/builder` on a tool called `fake-agent`, in
/// `/work`, with the prompt `do the task`.
pub fn sample_launch_request() -> LaunchRequest {
    LaunchRequest {
        role_name: "demo/builder".into(),
        role: serde_json::from_value::<Role>(json!({ "agent": "fake-agent" }))
            .expect("a role with an agent"),
        phase_name: "build".into(),
        phase: AgentPhase {
            role: "demo/builder".into(),
            prompt: Prompt::Name("build".into()),
            label: None,
            gates: Vec::new(),
            outputs: Vec::new(),
            record: false,
            sandbox: None,
            on_cold: OnCold::default(),
            fanout: None,
            then: None,
        },
        model: None::<ModelRef>,
        session: SessionStart::AgentAssigned {},
        cwd: PathBuf::from("/work"),
        snapshot_dir: PathBuf::from("/snap"),
        prompt: "do the task".into(),
        budget_left_usd: None,
    }
}

/// The session of a [`FakeAdapter`]: it passes the child's events through, and holds what a session must hold, which
/// is the permission requests still waiting for an answer, whether a cancel was sent, and how the last turn ended.
#[derive(Debug, Clone, Default)]
pub struct NormalizedSession {
    session_id: Option<String>,
    pending: Vec<String>,
    cancel_requested: bool,
    last_turn: Option<(TurnEndReason, bool)>,
}

impl NormalizedSession {
    pub fn new() -> Self {
        Self::default()
    }

    /// Writes one message: learns it as a replay would, then returns it as the bytes for the child.
    fn write(&mut self, message: Value) -> Vec<u8> {
        let bytes = json_line(&message);
        self.note_written(&bytes)
            .expect("a session reads its own writes");
        bytes
    }
}

impl Replayable for NormalizedSession {
    fn note_written(&mut self, bytes: &[u8]) -> Result<(), String> {
        let message: Value = serde_json::from_slice(bytes)
            .map_err(|reason| format!("not a JSON message: {reason}"))?;
        match message.get("type").and_then(Value::as_str) {
            // A new turn is a new thing to cancel.
            Some("user_turn") => self.cancel_requested = false,
            Some("interrupt") => self.cancel_requested = true,
            Some("permission_reply") => {
                let id = message
                    .get("request_id")
                    .and_then(Value::as_str)
                    .ok_or("a permission reply with no `request_id`")?;
                self.pending.retain(|pending| pending != id);
            }
            other => return Err(format!("not a message this session writes: type {other:?}")),
        }
        Ok(())
    }

    fn read_line(&mut self, line: &[u8], wall_ns: u64) -> Step {
        let mut step = Step::default();
        let Ok(mut event) = serde_json::from_slice::<AgentEvent>(line) else {
            step.events.push(unparsed_line(line, wall_ns));
            return step;
        };
        match &mut event {
            AgentEvent::SessionBound { id } => self.session_id = Some(id.clone()),
            AgentEvent::PermissionRequest { request } => {
                if !self.pending.contains(&request.id) {
                    self.pending.push(request.id.clone());
                }
            }
            AgentEvent::PermissionUnanswerable { request_id } => {
                self.pending.retain(|pending| pending != request_id);
            }
            AgentEvent::TurnEnd {
                reason,
                cancel_requested,
                empty,
            } => {
                // The session reports a cancelled turn itself, whatever the agent's own reason says (0.3).
                if self.cancel_requested {
                    *reason = TurnEndReason::Cancelled {};
                    *cancel_requested = true;
                }
                self.last_turn = Some((reason.clone(), *empty));
            }
            _ => {}
        }
        step.events.push(stamped(Some(wall_ns), event));
        step
    }
}

impl AgentSession for NormalizedSession {
    fn start(&mut self) -> Step {
        Step::default()
    }

    fn feed(&mut self, line: &[u8], wall_ns: u64) -> Step {
        self.read_line(line, wall_ns)
    }

    fn send_turn(&mut self, text: &str) -> Step {
        let write = self.write(json!({ "type": "user_turn", "text": text }));
        Step {
            events: Vec::new(),
            writes: vec![write],
        }
    }

    fn answer(&mut self, request_id: &str, reply: PermissionReply) -> Step {
        if !self.pending.iter().any(|id| id == request_id) {
            return Step::default();
        }
        let reply = serde_json::to_value(&reply).expect("a reply serializes");
        let write = self.write(json!({
            "type": "permission_reply", "request_id": request_id, "reply": reply
        }));
        Step {
            events: Vec::new(),
            writes: vec![write],
        }
    }

    fn cancel(&mut self) -> Step {
        let write = self.write(json!({ "type": "interrupt" }));
        Step {
            events: Vec::new(),
            writes: vec![write],
        }
    }

    fn finish(&mut self, exit: &ExitInfo) -> Finished {
        let outcome = match (&exit.record, &self.last_turn) {
            (None, _) => PhaseOutcome::Lost,
            (Some(_), None) => PhaseOutcome::Failed {
                cause: PhaseFailure::NoResult,
            },
            (Some(_), Some((reason, empty))) => outcome_of_turn(reason, *empty),
        };
        Finished {
            step: Step::default(),
            outcome,
            evidence: None,
        }
    }

    fn replay(&mut self, records: &[JournalRecord]) -> Result<Step, ReplayError> {
        replay_records(self, records)
    }
}

/// How the fake adapter's child speaks, for the shared contract: its lines are events.
#[derive(Debug, Clone, Copy, Default)]
pub struct NormalizedDialect;

impl NormalizedDialect {
    fn event_line(event: AgentEvent) -> Vec<u8> {
        json_line(&serde_json::to_value(event).expect("an event serializes"))
    }
}

impl ChildDialect for NormalizedDialect {
    type Session = NormalizedSession;

    fn session(&self) -> NormalizedSession {
        NormalizedSession::new()
    }

    fn handshake(&self) -> Vec<Vec<u8>> {
        vec![Self::event_line(AgentEvent::SessionBound {
            id: "s-1".into(),
        })]
    }

    fn message(&self, text: &str) -> Vec<u8> {
        Self::event_line(AgentEvent::Message { text: text.into() })
    }

    fn permission_request(&self, id: u32) -> Vec<u8> {
        Self::event_line(AgentEvent::PermissionRequest {
            request: crate::agent::PermissionRequest {
                id: id.to_string(),
                tool_call_id: format!("call-{id}"),
                tool: Some("bash".into()),
                kind: ToolKind::Execute,
                input: Some(json!({ "command": "ls" })),
                reason: None,
                options: Vec::new(),
            },
        })
    }

    fn turn_end(&self, _turn: u32) -> Vec<u8> {
        Self::event_line(AgentEvent::TurnEnd {
            reason: TurnEndReason::EndTurn {},
            cancel_requested: false,
            empty: false,
        })
    }
}
