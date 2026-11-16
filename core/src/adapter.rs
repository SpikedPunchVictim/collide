//! The two agent traits (0.3, Two core traits; S7, P1 to P9). A one-shot command line whose stdout describes itself
//! is Claude's shape; ACP is a stateful JSON-RPC conversation. A parser alone cannot answer an agent's request with
//! its id, so there are two traits, and neither names a process, Tauri or SQLite type:
//!
//! | trait | one per | state | holds |
//! |---|---|---|---|
//! | [`AgentAdapter`] | tool | none | capabilities, the launch spec, building a session, config paths, the interactive command |
//! | [`AgentSession`] | child | a state machine | the handshake, request ids, pending permission requests |
//!
//! [`AgentSession`] is **sans-IO**. Every method takes bytes, a line or a stored time and returns events and the
//! bytes to write; it reads no clock, opens no file and spawns nothing. The same input gives the same output, so a
//! daemon restart rebuilds a session with [`AgentSession::replay`] from the shim's journal ([`crate::journal`]),
//! which holds the time each line arrived (`wall_ns`).
//!
//! The data the traits move is in [`crate::agent`] (events, permissions, usage, capabilities). The ACP engine and the
//! ACP profile are `host`'s (Stage 3d) and are not here: `core` holds only these traits and the capability names.
//!
//! The one function here, [`refuse_unmet_capabilities`], is the rule that a role is refused for a capability the
//! adapter cannot promise.

use crate::agent::{
    AgentEvent, Capabilities, ModelRef, PermissionReply, PermissionRequest, SessionStart, Support,
};
use crate::journal::JournalRecord;
use crate::pack::Role;
use crate::shim::ExitRecord;
use crate::workflow::AgentPhase;
use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;

/// An event of a session with the stored time it is known to have happened at. `wall_ns` is the time the shim saw
/// the line that made the event (0.2, the journal), in nanoseconds since the Unix epoch. It is `None` for an event
/// that a call without a stored time made (`start`, `send_turn`, `answer`, `cancel`, `finish`): the daemon stamps
/// those with its own clock when it stores them, and a replay does not meet them, because their cause was a write.
#[derive(Debug, Clone, PartialEq)]
pub struct StampedEvent {
    pub wall_ns: Option<u64>,
    pub event: AgentEvent,
}

/// What one call of a session returns: the events, in order, and the bytes the host must write to the child's stdin,
/// in order. One entry of `writes` is one message; the shim journals it before the child sees it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Step {
    pub events: Vec<StampedEvent>,
    pub writes: Vec<Vec<u8>>,
}

/// How the child ended, as the shim learned it. `record` is `None` when the shim died without writing `exit.json`:
/// the phase is then `lost`, whatever the agent's stream said (0.2). `stderr_tail` is the end of the child's stderr,
/// kept as the evidence of an empty turn (0.3: an ACP turn that ends with no output and no usage).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExitInfo {
    pub record: Option<ExitRecord>,
    pub stderr_tail: String,
}

/// Why a phase failed when the session can tell (0.3, The stream protocol).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhaseFailure {
    /// A turn ended `end_turn` with no output and no usage. Only an adapter whose protocol has no failure flag
    /// reports it (ACP).
    EmptyTurn,
    /// A turn ended `refusal`.
    Refusal,
    /// A turn ended `max_tokens`.
    MaxTokens,
    /// A turn ended `max_turns`.
    MaxTurns,
    /// A turn ended `budget`: the role's budget ran out. The daemon pauses the phase and asks (0.3).
    Budget,
    /// A hook blocked the prompt, so the turn never ran. Carries the hook's reason when it gave one. The phase's Queue
    /// item shows it (0.3, The user's own tooling).
    Blocked { reason: Option<String> },
    /// An API failure other than a usage limit. Carries the HTTP status when there is one.
    Api { status: Option<u16> },
    /// The child exited and its stdout ended with no final result.
    NoResult,
    /// A turn ended but the settings canary never appeared: Claude ignored Collide's settings, so its hooks and
    /// permissions did not apply (0.3, Settings that fail validation are ignored silently under `-p`).
    SettingsIgnored,
    /// The role asked for a model that the agent does not offer, or could not be given (ACP: the id is not in the list
    /// the agent advertised, or the agent has no way to set a model). The phase never ran a turn (0.3, Who picks what).
    ModelUnavailable,
}

/// What a phase came to, which [`AgentSession::finish`] returns. A phase settles once the child has exited and its
/// stdout has reached its end, in either order; that rule decides when reading stops, not which outcome this is
/// (0.3). A pause (a usage limit, a budget) is not an outcome: the session reports it as an event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhaseOutcome {
    /// The agent finished its work. The phase's report is the [`AgentEvent::FinalReport`] it sent.
    Ended,
    Failed {
        cause: PhaseFailure,
    },
    /// The host stopped the phase: a `cancel` was sent and the turn ended `Cancelled`.
    Stopped,
    /// The child went away with no prompt response (ACP) or no exit record, so nothing is known about its work.
    Lost,
}

/// What [`AgentSession::finish`] returns: the last events and the outcome. `step.writes` is empty, because the child
/// has gone.
///
/// `evidence` is a short text that backs a failed outcome: the end of the child's stderr for an empty turn, the error
/// code for an error response (0.3, The stream protocol). It is `None` for every other outcome, and for a failure
/// that has none. It is at most [`EVIDENCE_BYTES`] long ([`evidence_tail`]), so the store never holds more.
#[derive(Debug, Clone, PartialEq)]
pub struct Finished {
    pub step: Step,
    pub outcome: PhaseOutcome,
    pub evidence: Option<String>,
}

/// The most evidence a failure keeps, in bytes (0.3: the record keeps the agent's stderr tail, bounded).
pub const EVIDENCE_BYTES: usize = 2048;

/// The end of `text`, as evidence: surrounding white space is dropped, then at most [`EVIDENCE_BYTES`] are kept from
/// the end, starting on a character boundary. `None` when nothing is left.
pub fn evidence_tail(text: &str) -> Option<String> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    let mut start = text.len().saturating_sub(EVIDENCE_BYTES);
    while !text.is_char_boundary(start) {
        start += 1;
    }
    Some(text[start..].to_string())
}

/// Why a [`AgentSession::replay`] stopped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplayError {
    /// A record's `seq` is not the one after the previous record's (0 for the first). Records are never reordered.
    OutOfOrder { expected: u64, found: u64 },
    /// The session could not read the record: the journal does not belong to this session.
    Rejected { seq: u64, message: String },
}

impl fmt::Display for ReplayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReplayError::OutOfOrder { expected, found } => write!(
                f,
                "journal record out of order: expected seq {expected}, found {found}"
            ),
            ReplayError::Rejected { seq, message } => {
                write!(
                    f,
                    "journal record {seq} was rejected by the session: {message}"
                )
            }
        }
    }
}

impl std::error::Error for ReplayError {}

/// One agent child's conversation, as a state machine (0.3). A Claude session is nearly empty: `start` writes nothing,
/// the first turn is a `user` message from `send_turn`, and `feed` is the stream parser. An ACP session holds its handshake, its
/// request ids and its pending permission requests.
///
/// Every method returns what happened and what to write, and changes only the session. The host writes `writes` to the
/// child's stdin after journalling them, and calls `feed` once for each line the child writes to stdout.
pub trait AgentSession {
    /// Begins the session. ACP: `initialize`, then `session/new` or a resume, then the first prompt. Claude: nothing
    /// to write.
    fn start(&mut self) -> Step;

    /// One line of the child's stdout, and `wall_ns`, the time the shim saw it (stored in the journal). The session
    /// never reads a clock, so the same line and time give the same step.
    fn feed(&mut self, line: &[u8], wall_ns: u64) -> Step;

    /// A user turn for the child.
    fn send_turn(&mut self, text: &str) -> Step;

    /// The answer to a permission request, by the request's id. Answering an id the session does not hold writes
    /// nothing.
    fn answer(&mut self, request_id: &str, reply: PermissionReply) -> Step;

    /// The permission requests the session has been asked and has not answered, oldest first. The daemon reads them
    /// after it rebuilt a session from the journal, so a request asked before a restart is put to the person again.
    /// Both engines hold them: an ACP agent is a child of the shim, outlives the daemon and keeps its request open, so
    /// the ACP session rebuilds its requests from the journal as Claude's does. A session that holds none has the
    /// default.
    fn pending_permissions(&self) -> Vec<PermissionRequest> {
        Vec::new()
    }

    /// Asks the child to stop its turn. The session reports `Cancelled` itself when the turn ends, whatever the
    /// agent's own reason says (0.3).
    fn cancel(&mut self) -> Step;

    /// The child has gone and its stdout has ended: the exit info in, the phase's outcome out.
    fn finish(&mut self, exit: &ExitInfo) -> Finished;

    /// Rebuilds the session from the shim's journal after a daemon restart: the records in `seq` order. An `in`
    /// record is bytes the session wrote, which rebuilds its request ids and pending requests; an `out` record goes
    /// through [`AgentSession::feed`] with its stored `wall_ns`. A replay writes nothing to the child, so the
    /// returned step's `writes` is empty. Records out of `seq` order are [`ReplayError::OutOfOrder`]; a cut-short last
    /// record was dropped by [`crate::journal::read_journal`] before it got here. The walk is [`crate::replay::replay_records`],
    /// which a session calls over its [`crate::replay::Replayable`]; [`crate::replay::replay_journal_bytes`] is the path from
    /// the journal's bytes.
    fn replay(&mut self, records: &[JournalRecord]) -> Result<Step, ReplayError>;
}

/// What a launch needs besides the role (0.3, the launch line). The adapter reads these and the host never has to
/// know which of them a tool uses.
#[derive(Debug, Clone, PartialEq)]
pub struct LaunchRequest {
    /// The role as a workflow names it, `pack/role`.
    pub role_name: String,
    pub role: Role,
    pub phase_name: String,
    pub phase: AgentPhase,
    /// The model the role's alias resolved to for this tool, if the role has one.
    pub model: Option<ModelRef>,
    pub session: SessionStart,
    /// The workspace the child runs in.
    pub cwd: PathBuf,
    /// The line's snapshot of settings, agent definitions and MCP config for this phase.
    pub snapshot_dir: PathBuf,
    /// The phase's first turn. Claude is sent it by `send_turn` as a `user` message; an ACP session sends it in `start`.
    pub prompt: String,
    /// What is left of the phase's budget, for a tool that enforces one itself (`budget_native`).
    pub budget_left_usd: Option<f64>,
}

/// The environment variables to remove from everything Collide starts (0.3, What the adapter declares): exact
/// names and name prefixes.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EnvStrip {
    pub names: Vec<String>,
    pub prefixes: Vec<String>,
}

/// How to start one child: the argument vector, the environment to add, the variables to strip first, and the
/// working directory (0.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchSpec {
    pub argv: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub env_strip: EnvStrip,
    pub cwd: PathBuf,
}

/// Why an adapter cannot make a launch spec.
#[derive(Debug, Clone, PartialEq)]
pub enum LaunchError {
    /// The role requires a capability that the adapter says `No` or `Unknown` for.
    Capability(CapabilityRefusal),
    /// The request is not valid for this tool, such as a `compact_at` under the floor. The message says why.
    Invalid { message: String },
}

impl fmt::Display for LaunchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LaunchError::Capability(refusal) => refusal.fmt(f),
            LaunchError::Invalid { message } => f.write_str(message),
        }
    }
}

impl std::error::Error for LaunchError {}

/// Where a tool's config path is rooted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigRoot {
    /// Below a checkout of the repo (Claude: `.claude/**`, `CLAUDE.md`, `.mcp.json`).
    Repo,
    /// Below the user's home (Claude: the auto memory directory).
    Home,
}

/// A path pattern of the tool's own config. An edit to one cannot reach a later automatic phase, which loads from the
/// snapshot, but it matters once it is about to reach the line's ref, and in a take-over (0.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigPath {
    pub root: ConfigRoot,
    pub pattern: String,
}

/// A session that exists, for an interactive take-over: its id and the directory it ran in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionRef {
    pub id: String,
    pub cwd: PathBuf,
}

/// A command a person runs in a terminal (Claude: `claude --resume <id>`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSpec {
    pub argv: Vec<String>,
    pub cwd: PathBuf,
}

/// One tool's declarations and launch (0.3). It has no state, so one value serves every phase, and two calls with the
/// same request give the same answer.
pub trait AgentAdapter {
    /// The capability table (0.3, Capabilities). A capability the adapter has not probed is `Unknown`.
    fn capabilities(&self) -> Capabilities;

    /// The launch spec for a role and a phase: argv, env, working directory and the variables to strip.
    fn launch(&self, request: &LaunchRequest) -> Result<LaunchSpec, LaunchError>;

    /// A new session for one child, which starts as `request.session` says. It is `Send`, because the daemon's
    /// watcher thread for the phase owns it.
    fn session(&self, request: &LaunchRequest) -> Box<dyn AgentSession + Send>;

    /// The tool's config paths in a repo, and the user's own state it keeps.
    fn config_paths(&self) -> Vec<ConfigPath>;

    /// The command that opens a session interactively, for a take-over. `None` when the adapter's `take_over` is not
    /// `Yes`.
    fn interactive_command(&self, session: &SessionRef) -> Option<CommandSpec>;
}

/// A role that requires a capability the adapter could not promise (0.3). `capability` is the name as the role wrote
/// it (`mcp_servers.http`), and `support` is what the adapter said.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityRefusal {
    pub agent: String,
    pub capability: String,
    pub support: Support,
}

impl fmt::Display for CapabilityRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let said = match self.support {
            Support::Yes => "yes",
            Support::No => "no",
            Support::Unknown => "unknown",
        };
        write!(
            f,
            "`{}` does not support `{}` (the adapter says {said}); a role that requires it cannot run on this tool",
            self.agent, self.capability
        )
    }
}

impl std::error::Error for CapabilityRefusal {}

/// Refuses a role that requires a capability the adapter reports `No` or `Unknown` for (0.3, Capabilities), rather
/// than losing it silently. `required` holds the names the role requires: a name of
/// [`crate::agent::CAPABILITY_NAMES`], or a name with a sub-flag as `permission_host.deny_message`. Only `Yes` meets
/// a requirement. An unprobed ACP profile says `Unknown`, so a role that requires anything of it is refused until a
/// probe says `Yes`. A name the table does not have cannot be promised either, and reads as `Unknown`. Every unmet
/// capability is returned once, in the order the role wrote them, so one message names them all; an empty `required`
/// is met.
///
/// A role with a budget needs `budget_native` or `usage_report.cost`, either one. That choice is the caller's: it
/// puts the one the adapter says `Yes` for in `required`, or `budget_native` when neither is, so the refusal names it.
///
/// The role key that carries `required` is a decision for the owner (`pack::Role` has none yet).
pub fn refuse_unmet_capabilities(
    agent: &str,
    capabilities: &Capabilities,
    required: &[String],
) -> Result<(), Vec<CapabilityRefusal>> {
    let mut refusals: Vec<CapabilityRefusal> = Vec::new();
    for name in required {
        let support = capabilities.support_of(name).unwrap_or(Support::Unknown);
        let already = refusals.iter().any(|refusal| &refusal.capability == name);
        if support != Support::Yes && !already {
            refusals.push(CapabilityRefusal {
                agent: agent.to_string(),
                capability: name.clone(),
                support,
            });
        }
    }
    if refusals.is_empty() {
        Ok(())
    } else {
        Err(refusals)
    }
}
