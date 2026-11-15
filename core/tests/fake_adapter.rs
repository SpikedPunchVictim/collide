//! The fake adapter (ts-0022-mc): an `AgentAdapter` whose session reads one normalized event as JSON on each line,
//! so an engine test scripts a run as a list of events and never starts a process. The `AgentSession` contract that
//! its session shares with the ACP double is in `agent_session_contract.rs`; these tests are what only the adapter
//! and its passthrough session have.

use collide_core::adapter::{
    AgentAdapter, AgentSession, ConfigRoot, ExitInfo, PhaseFailure, PhaseOutcome, SessionRef,
};
use collide_core::agent::{
    AgentEvent, FinalReportSource, PermissionReply, PermissionRequest, Support, ToolKind,
    TurnEndReason,
};
use collide_core::shim::{ExitEnd, ExitRecord, SHIM_PROTOCOL};
use collide_core::testing::{FakeAdapter, NormalizedSession, sample_launch_request};
use serde_json::{Value, json};
use std::path::PathBuf;

fn exited() -> ExitInfo {
    ExitInfo {
        record: Some(ExitRecord {
            protocol: SHIM_PROTOCOL,
            end: ExitEnd::Exited { code: 0 },
            cpu_user_us: 10,
            cpu_system_us: 5,
            max_rss_bytes: 4096,
        }),
        stderr_tail: String::new(),
    }
}

fn turn_end(reason: TurnEndReason) -> AgentEvent {
    AgentEvent::TurnEnd {
        reason,
        cancel_requested: false,
        empty: false,
    }
}

fn ask(id: &str) -> AgentEvent {
    AgentEvent::PermissionRequest {
        request: PermissionRequest {
            id: id.into(),
            tool_call_id: format!("call-{id}"),
            tool: Some("bash".into()),
            kind: ToolKind::Execute,
            input: Some(json!({ "command": "ls" })),
            reason: None,
            options: Vec::new(),
        },
    }
}

/// Feeds `event` as the child's line, the way a script does, and returns the step.
fn feed(
    session: &mut NormalizedSession,
    event: &AgentEvent,
    wall_ns: u64,
) -> collide_core::adapter::Step {
    let mut line = serde_json::to_vec(event).unwrap();
    line.push(b'\n');
    session.feed(&line, wall_ns)
}

// --- The adapter ---

#[test]
fn the_launch_spec_is_the_same_for_the_same_request_and_names_the_role_and_cwd() {
    let adapter = FakeAdapter::new(Vec::new());
    let first = adapter.launch(&sample_launch_request()).unwrap();
    let second = adapter.launch(&sample_launch_request()).unwrap();
    assert_eq!(first, second);
    assert_eq!(first.argv, vec!["fake-agent", "--role", "demo/builder"]);
    assert_eq!(first.cwd, PathBuf::from("/work"));
}

#[test]
fn the_capabilities_are_the_ones_given_and_otherwise_a_small_honest_table() {
    let default = FakeAdapter::new(Vec::new()).capabilities();
    assert_eq!(default.headless_stream, Support::Yes);
    assert_eq!(default.budget_native, Support::Unknown);
    let none = collide_core::agent::Capabilities {
        take_over: Support::No,
        ..Default::default()
    };
    let given = FakeAdapter::new(Vec::new()).with_capabilities(none.clone());
    assert_eq!(given.capabilities(), none);
}

#[test]
fn the_config_paths_are_below_the_repo() {
    let paths = FakeAdapter::new(Vec::new()).config_paths();
    assert_eq!(paths[0].root, ConfigRoot::Repo);
    assert_eq!(paths[0].pattern, ".fake-agent/**");
}

#[test]
fn an_interactive_command_is_offered_only_when_take_over_is_yes() {
    let session = SessionRef {
        id: "s-1".into(),
        cwd: PathBuf::from("/work"),
    };
    let yes = FakeAdapter::new(Vec::new());
    assert_eq!(
        yes.interactive_command(&session).unwrap().argv,
        vec!["fake-agent", "--resume", "s-1"]
    );
    let mut caps = yes.capabilities();
    caps.take_over = Support::No;
    let no = FakeAdapter::new(Vec::new()).with_capabilities(caps);
    assert_eq!(no.interactive_command(&session), None);
}

#[test]
fn the_adapter_builds_a_session_that_reads_its_own_script() {
    let script = vec![
        AgentEvent::SessionBound { id: "s-1".into() },
        AgentEvent::Message {
            text: "hello".into(),
        },
        turn_end(TurnEndReason::EndTurn {}),
    ];
    let adapter = FakeAdapter::new(script.clone());
    let mut session = adapter.session(&sample_launch_request());
    let heard: Vec<AgentEvent> = adapter
        .script_lines()
        .iter()
        .enumerate()
        .flat_map(|(index, line)| session.feed(line, 100 + index as u64).events)
        .map(|stamped| stamped.event)
        .collect();
    assert_eq!(heard, script);
}

#[test]
fn each_scripted_event_is_one_line_of_json_ended_by_a_newline() {
    let adapter = FakeAdapter::new(vec![
        AgentEvent::Message { text: "a".into() },
        AgentEvent::Message { text: "b".into() },
    ]);
    let lines = adapter.script_lines();
    assert_eq!(lines.len(), 2);
    for line in &lines {
        assert_eq!(line.iter().filter(|b| **b == b'\n').count(), 1);
        assert_eq!(line.last(), Some(&b'\n'));
        assert!(serde_json::from_slice::<Value>(line).is_ok());
    }
}

// --- The session ---

#[test]
fn a_fed_event_comes_out_with_the_time_it_was_fed() {
    let mut session = NormalizedSession::new();
    let step = feed(&mut session, &AgentEvent::Message { text: "hi".into() }, 42);
    assert_eq!(step.events[0].wall_ns, Some(42));
    assert!(step.writes.is_empty());
}

#[test]
fn a_permission_request_waits_and_the_reply_writes_the_answer_for_that_request() {
    let mut session = NormalizedSession::new();
    feed(&mut session, &ask("r-1"), 10);
    feed(&mut session, &AgentEvent::Message { text: "x".into() }, 11);
    let step = session.answer("r-1", PermissionReply::Allow {});
    let write: Value = serde_json::from_slice(&step.writes[0]).unwrap();
    assert_eq!(write["type"], "permission_reply");
    assert_eq!(write["request_id"], "r-1");
    assert_eq!(write["reply"], json!({ "type": "allow" }));
}

#[test]
fn a_turn_end_after_a_cancel_is_reported_cancelled_whatever_the_event_said() {
    let mut session = NormalizedSession::new();
    session.cancel();
    let step = feed(&mut session, &turn_end(TurnEndReason::EndTurn {}), 50);
    assert_eq!(
        step.events[0].event,
        AgentEvent::TurnEnd {
            reason: TurnEndReason::Cancelled {},
            cancel_requested: true,
            empty: false
        }
    );
}

#[test]
fn a_line_that_is_not_an_event_is_an_other_event_and_not_dropped() {
    let mut session = NormalizedSession::new();
    let step = session.feed(b"garbage\n", 5);
    assert_eq!(
        step.events[0].event,
        AgentEvent::Other {
            raw_type: "unparsed_line".into(),
            raw: Some(json!("garbage"))
        }
    );
}

#[test]
fn finish_reads_the_outcome_from_how_the_turn_ended() {
    let cases = [
        (TurnEndReason::EndTurn {}, PhaseOutcome::Ended),
        (
            TurnEndReason::Refusal {},
            PhaseOutcome::Failed {
                cause: PhaseFailure::Refusal,
            },
        ),
        (
            TurnEndReason::MaxTokens {},
            PhaseOutcome::Failed {
                cause: PhaseFailure::MaxTokens,
            },
        ),
        (
            TurnEndReason::MaxTurns {},
            PhaseOutcome::Failed {
                cause: PhaseFailure::MaxTurns,
            },
        ),
        (
            TurnEndReason::Error { status: Some(429) },
            PhaseOutcome::Failed {
                cause: PhaseFailure::Api { status: Some(429) },
            },
        ),
        (
            TurnEndReason::Blocked {
                reason: Some("not now".into()),
            },
            PhaseOutcome::Failed {
                cause: PhaseFailure::Blocked {
                    reason: Some("not now".into()),
                },
            },
        ),
    ];
    for (reason, outcome) in cases {
        let mut session = NormalizedSession::new();
        feed(&mut session, &turn_end(reason.clone()), 50);
        assert_eq!(session.finish(&exited()).outcome, outcome, "{reason:?}");
    }
}

#[test]
fn an_empty_end_turn_fails_the_phase_as_an_empty_turn() {
    let mut session = NormalizedSession::new();
    feed(
        &mut session,
        &AgentEvent::TurnEnd {
            reason: TurnEndReason::EndTurn {},
            cancel_requested: false,
            empty: true,
        },
        50,
    );
    assert_eq!(
        session.finish(&exited()).outcome,
        PhaseOutcome::Failed {
            cause: PhaseFailure::EmptyTurn
        }
    );
}

#[test]
fn a_child_that_exits_with_no_turn_end_has_no_result() {
    let mut session = NormalizedSession::new();
    feed(
        &mut session,
        &AgentEvent::FinalReport {
            text: "partial".into(),
            source: FinalReportSource::ResultField,
        },
        50,
    );
    assert_eq!(
        session.finish(&exited()).outcome,
        PhaseOutcome::Failed {
            cause: PhaseFailure::NoResult
        }
    );
}
