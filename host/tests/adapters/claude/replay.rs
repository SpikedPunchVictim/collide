//! Replaying the Claude session from the journal the real shim wrote (ts-0062-rj; Stage 3b criterion: over the same
//! recorded session, the session replayed from the journal reaches the same state as the live one). Fake Claude runs
//! under the shim ([`super::fake::play`]) with a live session; the shim's journal bytes are then replayed into a new
//! session, whole and cut short.

use super::fake::{Played, assigned, play};
use collide_core::adapter::{AgentSession, Step};
use collide_core::agent::{AgentEvent, PermissionReply};
use collide_core::journal::{Direction, JournalEnd, RECORD_HEADER_BYTES, read_journal};
use collide_core::replay::replay_journal_bytes;
use collide_host::adapters::claude::session::ClaudeSession;

fn allow_all(session: &mut ClaudeSession, event: &AgentEvent) -> Step {
    match event {
        AgentEvent::PermissionRequest { request } => {
            session.answer(&request.id, PermissionReply::Allow {})
        }
        _ => Step::default(),
    }
}

fn deny_all(session: &mut ClaudeSession, event: &AgentEvent) -> Step {
    match event {
        AgentEvent::PermissionRequest { request } => session.answer(
            &request.id,
            PermissionReply::Deny {
                message: Some("Not in this phase.".into()),
            },
        ),
        _ => Step::default(),
    }
}

fn cancel_at_the_tool_call(session: &mut ClaudeSession, event: &AgentEvent) -> Step {
    match event {
        AgentEvent::ToolCall { .. } => session.cancel(),
        _ => Step::default(),
    }
}

fn permission() -> Played {
    play(
        "permission-request",
        assigned(),
        &["touch the file"],
        allow_all,
    )
}

fn multi_turn() -> Played {
    play("multi-turn", assigned(), &["first", "second"], |_, _| {
        Step::default()
    })
}

/// Replays the journal into a new session, and checks it is the live session: the same events, with the stored times,
/// no write, and the same state (the sessions compare equal).
fn assert_replay_reaches_the_live_state(played: &Played) -> ClaudeSession {
    let mut rebuilt = ClaudeSession::new(assigned());
    let replay =
        replay_journal_bytes(&mut rebuilt, &played.journal).expect("the real journal replays");
    assert_eq!(replay.end, JournalEnd::Clean);
    assert_eq!(
        replay.step.events,
        played.fed_events(),
        "the same events, with the stored times"
    );
    assert!(
        replay.step.writes.is_empty(),
        "a replay writes nothing to the child"
    );
    assert_eq!(rebuilt, played.session, "the same state");
    rebuilt
}

#[test]
fn replayed_from_the_journal_the_permission_recording_reaches_the_state_of_the_live_session() {
    assert_replay_reaches_the_live_state(&permission());
}

#[test]
fn replayed_from_the_journal_the_multi_turn_recording_reaches_the_state_of_the_live_session() {
    assert_replay_reaches_the_live_state(&multi_turn());
}

#[test]
fn replayed_from_the_journal_a_host_deny_and_a_cancel_reach_the_state_of_the_live_session() {
    assert_replay_reaches_the_live_state(&play(
        "host-deny",
        assigned(),
        &["touch the file"],
        deny_all,
    ));
    assert_replay_reaches_the_live_state(&play(
        "cancelled",
        assigned(),
        &["wait a while"],
        cancel_at_the_tool_call,
    ));
}

#[test]
fn the_journal_holds_what_the_session_wrote_as_in_records_in_order_with_what_the_child_said() {
    let played = permission();
    let read = read_journal(&played.journal);
    let ins: Vec<serde_json::Value> = read
        .records
        .iter()
        .filter(|record| record.direction == Direction::In)
        .map(|record| serde_json::from_slice(&record.bytes).unwrap())
        .collect();
    assert_eq!(ins.len(), 2, "a turn and an answer");
    assert_eq!(ins[0]["type"], "user");
    assert_eq!(ins[1]["type"], "control_response");
    assert_eq!(ins[1]["response"]["request_id"], "req-01");
}

#[test]
fn a_replayed_session_answers_and_numbers_its_next_write_as_the_live_one_does() {
    let mut live = multi_turn();
    let mut rebuilt = assert_replay_reaches_the_live_state(&live);
    assert_eq!(rebuilt.send_turn("third"), live.session.send_turn("third"));
    assert_eq!(rebuilt.cancel(), live.session.cancel());
    assert_eq!(
        rebuilt.cancel(),
        live.session.cancel(),
        "the second interrupt has its own id"
    );
}

#[test]
fn a_replayed_session_has_the_cancel_the_journal_holds_so_the_turn_end_is_cancelled_again() {
    let played = play(
        "cancelled",
        assigned(),
        &["wait a while"],
        cancel_at_the_tool_call,
    );
    let mut rebuilt = ClaudeSession::new(assigned());
    replay_journal_bytes(&mut rebuilt, &played.journal).unwrap();
    // Both sessions have cancelled the turn that ended, and a result that follows is read as a cancelled turn.
    let result = br#"{"type":"result","subtype":"success","is_error":false,"result":"x","stop_reason":"end_turn"}"#;
    let mut line = result.to_vec();
    line.push(b'\n');
    assert_eq!(rebuilt.feed(&line, 9), {
        let mut live = played.session;
        live.feed(&line, 9)
    });
}

// ---- a journal cut short -------------------------------------------------------------------------------------------

fn record_ends(journal: &[u8]) -> Vec<usize> {
    let mut offset = 0;
    read_journal(journal)
        .records
        .iter()
        .map(|record| {
            offset += RECORD_HEADER_BYTES + record.bytes.len();
            offset
        })
        .collect()
}

fn cut_at_every_byte(played: &Played) {
    let ends = record_ends(&played.journal);
    let live = played.fed_events();
    for cut in 0..=played.journal.len() {
        let expected = ends.iter().filter(|&&end| end <= cut).count();
        let mut session = ClaudeSession::new(assigned());
        let replay = replay_journal_bytes(&mut session, &played.journal[..cut])
            .unwrap_or_else(|e| panic!("cut at {cut}: {e}"));
        assert_eq!(replay.records, expected, "cut at {cut}");
        assert!(
            live.starts_with(&replay.step.events),
            "cut at {cut}: a cut journal replays a prefix of the live events"
        );
        if ends.contains(&cut) || cut == 0 {
            assert_eq!(replay.end, JournalEnd::Clean, "cut at {cut}");
        } else {
            assert!(matches!(replay.end, JournalEnd::Cut { .. }), "cut at {cut}");
        }
    }
}

#[test]
fn the_permission_journal_cut_at_any_byte_replays_up_to_the_last_whole_record() {
    cut_at_every_byte(&permission());
}

#[test]
fn the_multi_turn_journal_cut_at_any_byte_replays_up_to_the_last_whole_record() {
    cut_at_every_byte(&multi_turn());
}

#[test]
fn a_journal_cut_after_the_request_and_before_the_answer_rebuilds_the_pending_request() {
    let played = permission();
    let read = read_journal(&played.journal);
    let asked = read
        .records
        .iter()
        .position(|record| {
            record.direction == Direction::Out
                && String::from_utf8_lossy(&record.bytes).contains("can_use_tool")
        })
        .expect("the request is in the journal");
    let answer = read
        .records
        .iter()
        .find(|record| {
            record.direction == Direction::In
                && String::from_utf8_lossy(&record.bytes).contains("control_response")
        })
        .expect("the answer is in the journal");
    let cut = record_ends(&played.journal)[asked];

    let mut rebuilt = ClaudeSession::new(assigned());
    replay_journal_bytes(&mut rebuilt, &played.journal[..cut]).unwrap();
    let step = rebuilt.answer("req-01", PermissionReply::Allow {});
    assert_eq!(
        step.writes,
        std::slice::from_ref(&answer.bytes),
        "the rebuilt session writes the answer the live one did"
    );
    assert_eq!(
        rebuilt.answer("req-01", PermissionReply::Allow {}),
        Step::default(),
        "and once"
    );
}

#[test]
fn a_journal_cut_after_the_answer_has_no_request_left_to_answer() {
    let played = permission();
    let read = read_journal(&played.journal);
    let answered = read
        .records
        .iter()
        .position(|record| {
            record.direction == Direction::In
                && String::from_utf8_lossy(&record.bytes).contains("control_response")
        })
        .unwrap();
    let cut = record_ends(&played.journal)[answered];
    let mut rebuilt = ClaudeSession::new(assigned());
    replay_journal_bytes(&mut rebuilt, &played.journal[..cut]).unwrap();
    assert_eq!(
        rebuilt.answer("req-01", PermissionReply::Allow {}),
        Step::default()
    );
}

#[test]
fn a_journal_that_is_not_this_sessions_is_rejected_with_the_record() {
    use collide_core::adapter::ReplayError;
    use collide_core::journal::{JournalRecord, encode_record};
    let bytes = encode_record(&JournalRecord {
        seq: 0,
        wall_ns: 1,
        direction: Direction::In,
        bytes: b"{\"type\":\"permission_reply\"}\n".to_vec(),
    })
    .unwrap();
    let mut session = ClaudeSession::new(assigned());
    let error = replay_journal_bytes(&mut session, &bytes).unwrap_err();
    assert!(
        matches!(error, ReplayError::Rejected { seq: 0, .. }),
        "{error:?}"
    );
}
