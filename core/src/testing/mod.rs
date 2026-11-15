//! The test doubles (0.15, Dependency injection; Stage 2, Test doubles as outlines). Core logic takes the agent
//! traits and the two ports as traits, so a test hands it one of these and the engine runs with no process, no file
//! and no clock. This module is behind the `testing` feature of `collide-core`, off by default; the crate's own tests
//! turn it on through a dev-dependency on itself.
//!
//! | double | implements | stands for |
//! |---|---|---|
//! | [`FakeAdapter`] with [`NormalizedSession`] | `AgentAdapter` and `AgentSession` | a tool whose child writes normalized events |
//! | [`AcpSession`] | `AgentSession` | an ACP agent: handshake, request ids, permission requests |
//! | [`FakeProvider`] | `TaskProvider` | a task source held in memory |
//! | [`FakeHookRunner`] | `HookRunner` | the hook executor, scripted by hook name |
//!
//! [`contract`] is the one suite that every `AgentSession` must pass, run against both session doubles. Both doubles
//! replay a journal through [`crate::replay::replay_records`], the one walk, which is `core` code and not a double.
//!
//! Every double is deterministic and sans-IO: it reads no clock, opens no file, starts no process and draws no
//! random number, so the same script and inputs give the same output. A test in `core/tests` scans this directory to
//! keep that true.

pub mod acp;
pub mod adapter;
pub mod contract;
pub mod hooks;
pub mod provider;

pub use acp::{AcpDialect, AcpSession};
pub use adapter::{FakeAdapter, NormalizedDialect, NormalizedSession, sample_launch_request};
pub use hooks::{FakeHookRunner, hook_request, hook_result};
pub use provider::{FakeProvider, default_initialize, fake_task};

use crate::adapter::StampedEvent;
use crate::agent::AgentEvent;
use serde_json::Value;

/// One JSON value as the line a child or a session writes: compact, ended by a newline.
pub(crate) fn json_line(value: &Value) -> Vec<u8> {
    let mut bytes = serde_json::to_vec(value).expect("a JSON value serializes");
    bytes.push(b'\n');
    bytes
}

/// An event with the stored time of the line that made it.
pub(crate) fn stamped(wall_ns: Option<u64>, event: AgentEvent) -> StampedEvent {
    StampedEvent { wall_ns, event }
}

/// What a line that is not a message of the protocol becomes: an `Other` event that keeps the text, never a dropped
/// line.
pub(crate) fn unparsed_line(line: &[u8], wall_ns: u64) -> StampedEvent {
    let text = String::from_utf8_lossy(line).trim_end().to_string();
    stamped(
        Some(wall_ns),
        AgentEvent::Other {
            raw_type: "unparsed_line".into(),
            raw: Some(Value::String(text)),
        },
    )
}

/// What a phase came to, from how its last turn ended. A cancelled turn is `Stopped`, whatever the agent's own
/// reason said (the session rewrote it). An `end_turn` with no output and no usage is a failed call (S7, O4), never a
/// success.
pub(crate) fn outcome_of_turn(
    reason: &crate::agent::TurnEndReason,
    empty: bool,
) -> crate::adapter::PhaseOutcome {
    use crate::adapter::{PhaseFailure, PhaseOutcome};
    use crate::agent::TurnEndReason;
    let failed = |cause| PhaseOutcome::Failed { cause };
    match reason {
        TurnEndReason::Cancelled {} => PhaseOutcome::Stopped,
        TurnEndReason::EndTurn {} if empty => failed(PhaseFailure::EmptyTurn),
        TurnEndReason::EndTurn {} => PhaseOutcome::Ended,
        TurnEndReason::MaxTokens {} => failed(PhaseFailure::MaxTokens),
        TurnEndReason::MaxTurns {} => failed(PhaseFailure::MaxTurns),
        TurnEndReason::Refusal {} => failed(PhaseFailure::Refusal),
        TurnEndReason::Error { status } => failed(PhaseFailure::Api { status: *status }),
    }
}
