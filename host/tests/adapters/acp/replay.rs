//! Replaying the ACP session from the journal the real shim wrote (ts-0087-mf; Stage 3d: the second `AgentSession`
//! keeps the trait honest). The fake ACP child runs under the shim ([`super::fake::play`]) with a live session; the
//! shim's journal bytes are then replayed into a new session, whole and cut short, as `replay` of the Claude session is
//! tested.

use super::fake::{Played, play, recording};
use collide_core::adapter::{AgentSession, ReplayError, Step};
use collide_core::agent::{AgentEvent, PermissionReply};
use collide_core::journal::{
    Direction, JournalEnd, JournalRecord, RECORD_HEADER_BYTES, encode_record, read_journal,
};
use collide_core::replay::replay_journal_bytes;
use collide_host::adapters::acp::options::AcpOptions;
use collide_host::adapters::acp::session::AcpSession;

fn options() -> AcpOptions {
    AcpOptions::new("/work/repo", "read the notes")
}

fn allow_all(session: &mut AcpSession, event: &AgentEvent) -> Step {
    match event {
        AgentEvent::PermissionRequest { request } => {
            session.answer(&request.id, PermissionReply::Allow {})
        }
        _ => Step::default(),
    }
}

fn prompt_turn() -> Played {
    play(&recording("prompt-turn"), options(), &[], |_, _| {
        Step::default()
    })
}

fn permission() -> Played {
    play(&recording("permission-request"), options(), &[], allow_all)
}

/// Replays the journal into a new session, and checks it is the live session: the same events, with the stored times,
/// no write, and the same state (the sessions compare equal).
fn assert_replay_reaches_the_live_state(played: &Played) -> AcpSession {
    let mut rebuilt = AcpSession::new(options());
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
fn replayed_from_the_journal_the_prompt_turn_reaches_the_state_of_the_live_session() {
    assert_replay_reaches_the_live_state(&prompt_turn());
}

#[test]
fn replayed_from_the_journal_the_permission_recording_reaches_the_state_of_the_live_session() {
    assert_replay_reaches_the_live_state(&permission());
}

#[test]
fn a_replayed_session_finishes_as_the_live_one_does() {
    let mut live = prompt_turn();
    let mut rebuilt = assert_replay_reaches_the_live_state(&live);
    assert_eq!(rebuilt.finish(&live.exit), live.session.finish(&live.exit));
}

#[test]
fn a_replayed_session_numbers_its_next_write_as_the_live_one_does() {
    let mut live = prompt_turn();
    let mut rebuilt = assert_replay_reaches_the_live_state(&live);
    let next = rebuilt.send_turn("third");
    assert_eq!(next, live.session.send_turn("third"));
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&next.writes[0]).unwrap()["id"],
        3,
        "after 0, 1 and 2"
    );
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
        let mut session = AcpSession::new(options());
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
fn the_prompt_turn_journal_cut_at_any_byte_replays_up_to_the_last_whole_record() {
    cut_at_every_byte(&prompt_turn());
}

#[test]
fn the_permission_journal_cut_at_any_byte_replays_up_to_the_last_whole_record() {
    cut_at_every_byte(&permission());
}

#[test]
fn a_journal_cut_after_the_handshake_reply_has_the_session_id_and_numbers_the_next_write_after_it()
{
    // Cut after the `session/new` response and before the first prompt: the session is bound, and the id it would
    // write next is 2, as in the live session.
    let played = prompt_turn();
    let read = read_journal(&played.journal);
    let cut = record_ends(&played.journal)[read
        .records
        .iter()
        .position(|record| String::from_utf8_lossy(&record.bytes).contains("sessionId"))
        .expect("the `session/new` response is in the journal")];
    let mut rebuilt = AcpSession::new(options());
    let replay = replay_journal_bytes(&mut rebuilt, &played.journal[..cut]).unwrap();
    assert!(matches!(
        replay.step.events.last().map(|stamped| &stamped.event),
        Some(AgentEvent::SessionBound { .. })
    ));
    let next = rebuilt.send_turn("next");
    let sent: serde_json::Value = serde_json::from_slice(&next.writes[0]).unwrap();
    assert_eq!(sent["id"], 2);
    assert_eq!(sent["params"]["sessionId"], "ses-stand-in-1");
}

#[test]
fn a_journal_cut_after_the_agents_request_and_before_the_answer_answers_as_the_live_session_did() {
    let played = permission();
    let read = read_journal(&played.journal);
    let asked = read
        .records
        .iter()
        .position(|record| {
            record.direction == Direction::Out
                && String::from_utf8_lossy(&record.bytes).contains("request_permission")
        })
        .expect("the request is in the journal");
    let answer = read
        .records
        .iter()
        .find(|record| {
            record.direction == Direction::In
                && String::from_utf8_lossy(&record.bytes).contains("\"result\"")
        })
        .expect("the answer is in the journal");
    let cut = record_ends(&played.journal)[asked];

    let mut rebuilt = AcpSession::new(options());
    replay_journal_bytes(&mut rebuilt, &played.journal[..cut]).unwrap();
    let step = rebuilt.answer("7", PermissionReply::Allow {});
    assert_eq!(
        step.writes,
        std::slice::from_ref(&answer.bytes),
        "the rebuilt session writes the answer the live one did"
    );
    assert_eq!(
        rebuilt.answer("7", PermissionReply::Allow {}),
        Step::default(),
        "and once"
    );
}

#[test]
fn a_journal_that_is_not_this_sessions_is_rejected_with_the_record() {
    let bytes = encode_record(&JournalRecord {
        seq: 0,
        wall_ns: 1,
        direction: Direction::In,
        bytes: b"{\"type\":\"permission_reply\"}\n".to_vec(),
    })
    .unwrap();
    let mut session = AcpSession::new(options());
    let error = replay_journal_bytes(&mut session, &bytes).unwrap_err();
    assert!(
        matches!(error, ReplayError::Rejected { seq: 0, .. }),
        "{error:?}"
    );
}

#[test]
fn a_journal_with_a_method_this_session_never_writes_is_rejected_at_that_record() {
    let mut bytes = Vec::new();
    for (seq, text) in [
        (
            0,
            &b"{\"jsonrpc\":\"2.0\",\"id\":0,\"method\":\"initialize\",\"params\":{}}\n"[..],
        ),
        (
            1,
            &b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"fs/read_text_file\",\"params\":{}}\n"[..],
        ),
    ] {
        bytes.extend(
            encode_record(&JournalRecord {
                seq,
                wall_ns: 1,
                direction: Direction::In,
                bytes: text.to_vec(),
            })
            .unwrap(),
        );
    }
    let mut session = AcpSession::new(options());
    let error = replay_journal_bytes(&mut session, &bytes).unwrap_err();
    assert!(
        matches!(error, ReplayError::Rejected { seq: 1, .. }),
        "{error:?}"
    );
}

// ---- writes the journal cut left owed (ts-0092-tw) ------------------------------------------------------------------
//
// A cut can fall after a line the session answers and before its answer: the shim journals the line the child wrote
// first, and the daemon's write comes after. A replay writes nothing, so the agent would wait. The rebuilt session
// reports the writes it still owes ([`AgentSession::take_owed_writes`]); the daemon sends them once.

const MODEL: &str = "stand-in-model-b";

/// A session rebuilt from the first `records` records of the journal.
fn rebuilt_from(played: &Played, options: &AcpOptions, records: usize) -> AcpSession {
    let ends = record_ends(&played.journal);
    let cut = if records == 0 { 0 } else { ends[records - 1] };
    let mut session = AcpSession::new(options.clone());
    replay_journal_bytes(&mut session, &played.journal[..cut]).expect("a cut journal replays");
    session
}

/// At every record boundary: what a rebuilt session owes is the `in` records that follow the cut, which the live
/// session wrote for the lines before it. Owed once, the session is as one rebuilt from a journal that holds the
/// writes, and that journal owes nothing.
fn assert_owes_what_the_live_session_wrote_next(played: &Played, options: &AcpOptions) {
    let records = read_journal(&played.journal).records;
    for cut in 0..=records.len() {
        let expected: Vec<Vec<u8>> = records[cut..]
            .iter()
            .take_while(|record| record.direction == Direction::In)
            .map(|record| record.bytes.clone())
            .collect();
        let mut session = rebuilt_from(played, options, cut);
        assert_eq!(
            session.take_owed_writes(),
            expected,
            "cut after {cut} records"
        );
        assert!(
            session.take_owed_writes().is_empty(),
            "cut after {cut} records: owed once"
        );
        let mut holding = rebuilt_from(played, options, cut + expected.len());
        assert!(
            holding.take_owed_writes().is_empty(),
            "cut after {cut} records: a journal that holds the writes owes none"
        );
        assert_eq!(session, holding, "cut after {cut} records: the same state");
    }
}

#[test]
fn a_prompt_turn_journal_owes_at_every_cut_what_the_live_session_wrote_next() {
    assert_owes_what_the_live_session_wrote_next(&prompt_turn(), &options());
}

#[test]
fn a_journal_with_a_request_nobody_serves_owes_the_not_found_answer_at_every_cut() {
    let played = play(&recording("unserved-request"), options(), &[], |_, _| {
        Step::default()
    });
    assert_owes_what_the_live_session_wrote_next(&played, &options());
}

#[test]
fn a_journal_with_a_request_open_at_the_turns_end_owes_the_cancelled_answer_at_every_cut() {
    let played = play(
        &recording("permission-open-at-turn-end"),
        options(),
        &[],
        |_, _| Step::default(),
    );
    assert_owes_what_the_live_session_wrote_next(&played, &options());
}

#[test]
fn a_journal_with_a_model_request_owes_the_request_and_then_the_prompt_at_every_cut() {
    let options = options().with_model(MODEL);
    let played = play(
        &recording("model-config-options"),
        options.clone(),
        &[],
        |_, _| Step::default(),
    );
    assert_owes_what_the_live_session_wrote_next(&played, &options);
}

#[test]
fn an_empty_journal_owes_the_initialize_the_session_starts_with() {
    let mut started = AcpSession::new(options());
    let initialize = started.start().writes;
    let mut rebuilt = AcpSession::new(options());
    replay_journal_bytes(&mut rebuilt, &[]).unwrap();
    assert_eq!(rebuilt.take_owed_writes(), initialize);
    assert!(rebuilt.take_owed_writes().is_empty());
    assert_eq!(rebuilt, started, "it is the session that started");
}

#[test]
fn a_journal_cut_before_the_first_prompt_owes_it_and_a_journal_that_has_it_owes_nothing() {
    let played = prompt_turn();
    let read = read_journal(&played.journal);
    let prompt = read
        .records
        .iter()
        .position(|record| String::from_utf8_lossy(&record.bytes).contains("session/prompt"))
        .expect("the prompt is in the journal");
    let mut cut = rebuilt_from(&played, &options(), prompt);
    assert_eq!(
        cut.take_owed_writes(),
        vec![read.records[prompt].bytes.clone()],
        "the prompt the live session wrote"
    );
    let mut holding = rebuilt_from(&played, &options(), prompt + 1);
    assert!(holding.take_owed_writes().is_empty());
}

#[test]
fn a_request_asked_after_the_cancel_owes_its_cancelled_answer_when_the_journal_stops_after_it() {
    let played = play(
        &recording("permission-after-cancel"),
        options(),
        &[],
        |session, event| match event {
            AgentEvent::Message { .. } => session.cancel(),
            _ => Step::default(),
        },
    );
    let records = read_journal(&played.journal).records;
    let asked = records
        .iter()
        .position(|record| String::from_utf8_lossy(&record.bytes).contains("request_permission"))
        .expect("the request is in the journal");
    let answer = &records[asked + 1];
    assert_eq!(answer.direction, Direction::In);
    let mut session = rebuilt_from(&played, &options(), asked + 1);
    assert_eq!(session.take_owed_writes(), vec![answer.bytes.clone()]);
    assert!(session.take_owed_writes().is_empty());
    let mut holding = rebuilt_from(&played, &options(), asked + 2);
    assert!(holding.take_owed_writes().is_empty());
}

#[test]
fn a_live_session_owes_nothing() {
    let mut live = AcpSession::new(options());
    assert!(live.take_owed_writes().is_empty());
    live.start();
    assert!(live.take_owed_writes().is_empty());
}
