//! The ACP session's permission requests and its cancel (ts-0089-wv): what the answer to each kind of reply writes,
//! what `pending_permissions` holds live and after a replay, how `cancel` orders its writes, and the same paths under
//! the real shim with the fake child. The agent's lines are written out here when a test needs a request id that is
//! not a number or a shape the double of `core` has no function for.

use super::fake::{play, recording};
use super::session::{SESSION, events_of, exited, line, ready, session, written};
use collide_core::adapter::{AgentSession, PhaseFailure, PhaseOutcome, StampedEvent, Step};
use collide_core::agent::{
    AgentEvent, PermissionOption, PermissionOptionKind, PermissionReply, PermissionRequest,
    ToolKind, TurnEndReason,
};
use collide_core::journal::{Direction, JournalRecord, RECORD_HEADER_BYTES, read_journal};
use collide_core::replay::replay_journal_bytes;
use collide_core::testing::acp::{
    initialize_response, message_chunk, new_session_response, prompt_error, prompt_response,
    tool_call,
};
use collide_host::adapters::acp::options::AcpOptions;
use collide_host::adapters::acp::session::AcpSession;
use serde_json::{Value, json};

/// The options an agent offers, as `(id, kind)` pairs. The name is made from the id.
fn offered(options: &[(&str, &str)]) -> Value {
    Value::Array(
        options
            .iter()
            .map(|(id, kind)| json!({ "optionId": id, "name": format!("name of {id}"), "kind": kind }))
            .collect(),
    )
}

/// A `session/request_permission` with the id the agent chose, which is a number or a string.
fn request_line(id: Value, call: Value, options: Value) -> Vec<u8> {
    line(json!({
        "jsonrpc": "2.0", "id": id, "method": "session/request_permission",
        "params": { "sessionId": SESSION, "toolCall": call, "options": options }
    }))
}

fn bash_call() -> Value {
    json!({ "toolCallId": "call-stand-in-1", "title": "bash", "kind": "execute", "rawInput": {} })
}

/// A session that read a request with id `id` and these options.
fn asked(id: Value, options: &[(&str, &str)]) -> AcpSession {
    let mut session = ready();
    session.feed(&request_line(id, bash_call(), offered(options)), 500);
    session
}

/// What the agent reads when the host chose the option `option_id`.
fn selected(id: Value, option_id: &str) -> Value {
    json!({
        "jsonrpc": "2.0", "id": id,
        "result": { "outcome": { "outcome": "selected", "optionId": option_id } }
    })
}

fn cancelled(id: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": { "outcome": { "outcome": "cancelled" } } })
}

fn session_cancel() -> Value {
    json!({ "jsonrpc": "2.0", "method": "session/cancel", "params": { "sessionId": SESSION } })
}

const STANDARD: [(&str, &str); 3] = [
    ("once", "allow_once"),
    ("always", "allow_always"),
    ("reject", "reject_once"),
];

fn request_of(event: &AgentEvent) -> &PermissionRequest {
    match event {
        AgentEvent::PermissionRequest { request } => request,
        other => panic!("expected a permission request, got {other:?}"),
    }
}

fn turn_end_of(step: &Step) -> (TurnEndReason, bool) {
    match events_of(step).last() {
        Some(AgentEvent::TurnEnd {
            reason,
            cancel_requested,
            ..
        }) => (reason.clone(), *cancel_requested),
        other => panic!("expected a turn end last, got {other:?}"),
    }
}

// ---- Allow ---------------------------------------------------------------------------------------------------------

#[test]
fn allow_picks_the_allow_once_option_when_it_is_the_only_allow() {
    let mut session = asked(
        json!(7),
        &[("once", "allow_once"), ("reject", "reject_once")],
    );
    let step = session.answer("7", PermissionReply::Allow {});
    assert_eq!(written(&step), vec![selected(json!(7), "once")]);
    assert!(step.events.is_empty());
}

#[test]
fn allow_picks_allow_once_and_never_allow_always_when_both_are_offered() {
    // The always option comes first and a second once option comes after the first: the first once is the one.
    let options = [
        ("always", "allow_always"),
        ("once-a", "allow_once"),
        ("once-b", "allow_once"),
        ("reject", "reject_once"),
    ];
    let mut session = asked(json!(7), &options);
    let step = session.answer("7", PermissionReply::Allow {});
    assert_eq!(written(&step), vec![selected(json!(7), "once-a")]);
    let text = String::from_utf8(step.writes[0].clone()).unwrap();
    assert!(!text.contains("always"), "{text}");
}

#[test]
fn allow_with_only_allow_always_is_cancelled_and_unanswerable() {
    let mut session = asked(
        json!(7),
        &[("always", "allow_always"), ("never", "reject_always")],
    );
    let step = session.answer("7", PermissionReply::Allow {});
    assert_eq!(written(&step), vec![cancelled(json!(7))]);
    assert_eq!(
        events_of(&step),
        vec![AgentEvent::PermissionUnanswerable {
            request_id: "7".into()
        }]
    );
    assert!(step.events.iter().all(|stamped| stamped.wall_ns.is_none()));
}

#[test]
fn allow_with_no_allow_option_at_all_is_cancelled_and_unanswerable() {
    for options in [
        vec![("reject", "reject_once")],
        vec![("never", "reject_always")],
        vec![],
    ] {
        let mut session = asked(json!(7), &options);
        let step = session.answer("7", PermissionReply::Allow {});
        assert_eq!(written(&step), vec![cancelled(json!(7))], "{options:?}");
        assert_eq!(
            events_of(&step),
            vec![AgentEvent::PermissionUnanswerable {
                request_id: "7".into()
            }],
            "{options:?}"
        );
    }
}

#[test]
fn an_option_of_a_kind_this_version_does_not_know_is_not_offered_and_never_picked() {
    let options = [("later", "allow_for_session"), ("reject", "reject_once")];
    let mut session = ready();
    let asked = session.feed(&request_line(json!(7), bash_call(), offered(&options)), 500);
    let events = events_of(&asked);
    let request = request_of(&events[0]);
    assert_eq!(request.options.len(), 1);
    let step = session.answer("7", PermissionReply::Allow {});
    assert_eq!(written(&step), vec![cancelled(json!(7))]);
}

// ---- Deny and Cancel ----------------------------------------------------------------------------------------------

#[test]
fn deny_picks_reject_once() {
    let mut session = asked(json!(7), &STANDARD);
    let step = session.answer("7", PermissionReply::Deny { message: None });
    assert_eq!(written(&step), vec![selected(json!(7), "reject")]);
    assert!(step.events.is_empty());
}

#[test]
fn deny_with_only_reject_always_is_cancelled_and_not_unanswerable() {
    let mut session = asked(
        json!(7),
        &[("always", "allow_always"), ("never", "reject_always")],
    );
    let step = session.answer("7", PermissionReply::Deny { message: None });
    assert_eq!(written(&step), vec![cancelled(json!(7))]);
    assert!(step.events.is_empty());
}

#[test]
fn a_deny_message_appears_nowhere_in_the_bytes_written() {
    for options in [&STANDARD[..], &[("never", "reject_always")][..]] {
        let mut session = asked(json!(7), options);
        let step = session.answer(
            "7",
            PermissionReply::Deny {
                message: Some("SECRET-STAND-IN not in this folder".into()),
            },
        );
        let text = String::from_utf8(step.writes.concat()).unwrap();
        assert!(!text.contains("SECRET-STAND-IN"), "{text}");
        assert!(!text.contains("folder"), "{text}");
    }
}

#[test]
fn a_cancel_reply_writes_the_outcome_cancelled() {
    let mut session = asked(json!(7), &STANDARD);
    let step = session.answer("7", PermissionReply::Cancel {});
    assert_eq!(written(&step), vec![cancelled(json!(7))]);
    assert!(step.events.is_empty());
}

// ---- the id --------------------------------------------------------------------------------------------------------

#[test]
fn a_number_id_stays_a_number_and_gives_the_event_its_text() {
    let mut session = ready();
    let step = session.feed(
        &request_line(json!(41), bash_call(), offered(&STANDARD)),
        500,
    );
    assert_eq!(request_of(&events_of(&step)[0]).id, "41");
    let step = session.answer("41", PermissionReply::Allow {});
    assert_eq!(written(&step)[0]["id"], json!(41));
    assert!(written(&step)[0]["id"].is_number());
}

#[test]
fn a_string_id_stays_a_string() {
    let mut session = ready();
    let step = session.feed(
        &request_line(json!("perm-stand-in-1"), bash_call(), offered(&STANDARD)),
        500,
    );
    assert_eq!(request_of(&events_of(&step)[0]).id, "perm-stand-in-1");
    let step = session.answer("perm-stand-in-1", PermissionReply::Allow {});
    assert_eq!(written(&step)[0]["id"], json!("perm-stand-in-1"));
}

#[test]
fn a_string_id_that_looks_like_a_number_is_answered_as_the_string_the_agent_wrote() {
    let mut session = ready();
    session.feed(
        &request_line(json!("7"), bash_call(), offered(&STANDARD)),
        500,
    );
    let step = session.answer("7", PermissionReply::Allow {});
    assert_eq!(written(&step)[0]["id"], json!("7"));
}

#[test]
fn an_id_the_session_does_not_hold_writes_nothing() {
    let mut session = asked(json!(7), &STANDARD);
    assert_eq!(
        session.answer("8", PermissionReply::Allow {}),
        Step::default()
    );
    assert_eq!(
        session.answer("", PermissionReply::Allow {}),
        Step::default()
    );
    assert_eq!(session.pending_permissions().len(), 1, "7 is still held");
}

#[test]
fn an_id_answered_twice_writes_nothing_the_second_time() {
    let mut session = asked(json!(7), &STANDARD);
    assert_eq!(
        session.answer("7", PermissionReply::Allow {}).writes.len(),
        1
    );
    for reply in [
        PermissionReply::Allow {},
        PermissionReply::Deny { message: None },
        PermissionReply::Cancel {},
    ] {
        assert_eq!(session.answer("7", reply), Step::default());
    }
}

// ---- the request's kind --------------------------------------------------------------------------------------------

fn kind_of_request(session: &mut AcpSession, call: Value) -> ToolKind {
    let step = session.feed(&request_line(json!(7), call, offered(&STANDARD)), 500);
    request_of(&events_of(&step)[0]).kind
}

#[test]
fn the_kind_comes_from_the_request_when_it_has_one() {
    let mut session = ready();
    let call = json!({ "toolCallId": "c1", "title": "Read notes.txt", "kind": "delete" });
    assert_eq!(kind_of_request(&mut session, call), ToolKind::Delete);
}

#[test]
fn the_kind_of_the_request_wins_over_the_earlier_tool_call() {
    let mut session = ready();
    session.feed(&tool_call(SESSION, "c1", "Read a", "read"), 400);
    let call = json!({ "toolCallId": "c1", "kind": "edit" });
    assert_eq!(kind_of_request(&mut session, call), ToolKind::Edit);
}

#[test]
fn a_request_with_no_kind_takes_it_from_the_earlier_tool_call_with_the_same_id() {
    let mut session = ready();
    session.feed(&tool_call(SESSION, "c1", "Read a", "fetch"), 400);
    session.feed(&tool_call(SESSION, "c2", "Read b", "search"), 410);
    let call = json!({ "toolCallId": "c1", "title": "bash" });
    assert_eq!(kind_of_request(&mut session, call), ToolKind::Fetch);
}

#[test]
fn a_request_with_no_kind_and_no_earlier_tool_call_is_of_kind_other() {
    let mut session = ready();
    session.feed(&tool_call(SESSION, "c2", "Read b", "search"), 410);
    let call = json!({ "toolCallId": "c1", "title": "Read notes.txt" });
    assert_eq!(kind_of_request(&mut session, call), ToolKind::Other);
    let mut bare = ready();
    assert_eq!(kind_of_request(&mut bare, json!({})), ToolKind::Other);
}

#[test]
fn a_request_with_a_kind_this_version_does_not_know_is_of_kind_other() {
    let mut session = ready();
    session.feed(&tool_call(SESSION, "c1", "Read a", "read"), 400);
    let call = json!({ "toolCallId": "c1", "kind": "a_kind_of_a_later_version" });
    assert_eq!(kind_of_request(&mut session, call), ToolKind::Other);
}

#[test]
fn a_tool_call_after_the_request_changes_nothing_already_given() {
    let mut session = ready();
    let call = json!({ "toolCallId": "c1", "title": "bash" });
    session.feed(&request_line(json!(7), call, offered(&STANDARD)), 500);
    let before = session.pending_permissions();
    session.feed(&tool_call(SESSION, "c1", "bash", "execute"), 510);
    assert_eq!(session.pending_permissions(), before);
    assert_eq!(before[0].kind, ToolKind::Other);
}

#[test]
fn the_tool_calls_of_an_earlier_turn_are_not_looked_in() {
    let mut session = ready();
    session.feed(&tool_call(SESSION, "c1", "Read a", "read"), 400);
    session.feed(&prompt_response(2, "end_turn", None), 410);
    session.send_turn("again");
    let call = json!({ "toolCallId": "c1" });
    assert_eq!(kind_of_request(&mut session, call), ToolKind::Other);
}

#[test]
fn a_request_with_no_input_sent_has_none() {
    let mut session = ready();
    let step = session.feed(
        &request_line(json!(7), bash_call(), offered(&STANDARD)),
        500,
    );
    assert_eq!(request_of(&events_of(&step)[0]).input, None);
}

// ---- pending_permissions -------------------------------------------------------------------------------------------

fn standard_options() -> Vec<PermissionOption> {
    let option = |id: &str, kind| PermissionOption {
        id: id.into(),
        name: format!("name of {id}"),
        kind,
    };
    vec![
        option("once", PermissionOptionKind::AllowOnce),
        option("always", PermissionOptionKind::AllowAlways),
        option("reject", PermissionOptionKind::RejectOnce),
    ]
}

#[test]
fn a_session_that_was_asked_nothing_holds_no_request() {
    assert!(ready().pending_permissions().is_empty());
    assert!(session().pending_permissions().is_empty());
}

#[test]
fn a_request_is_pending_with_its_options_until_it_is_answered() {
    let mut session = ready();
    let step = session.feed(
        &request_line(json!(7), bash_call(), offered(&STANDARD)),
        500,
    );
    let given = request_of(&events_of(&step)[0]).clone();
    assert_eq!(session.pending_permissions(), vec![given.clone()]);
    assert_eq!(given.options, standard_options());
    assert_eq!(given.tool_call_id, "call-stand-in-1");
    session.feed(&message_chunk(SESSION, "still working"), 510);
    assert_eq!(session.pending_permissions(), vec![given]);
    session.answer("7", PermissionReply::Allow {});
    assert!(session.pending_permissions().is_empty());
}

#[test]
fn pending_requests_are_returned_oldest_first_and_an_answer_takes_one_out() {
    let mut session = ready();
    for id in [json!(9), json!("b"), json!(3)] {
        session.feed(&request_line(id, bash_call(), offered(&STANDARD)), 500);
    }
    let ids = |session: &AcpSession| -> Vec<String> {
        session
            .pending_permissions()
            .into_iter()
            .map(|request| request.id)
            .collect()
    };
    assert_eq!(ids(&session), ["9", "b", "3"]);
    session.answer("b", PermissionReply::Deny { message: None });
    assert_eq!(ids(&session), ["9", "3"]);
}

#[test]
fn an_unanswerable_request_is_no_longer_pending() {
    let mut session = asked(json!(7), &[("always", "allow_always")]);
    session.answer("7", PermissionReply::Allow {});
    assert!(session.pending_permissions().is_empty());
}

/// The journal of a live conversation, as the shim writes it: what the session writes is an `in` record, what the
/// child writes an `out` record, and `seq` counts from 0.
#[derive(Default)]
struct Journal {
    records: Vec<JournalRecord>,
}

impl Journal {
    fn push(&mut self, direction: Direction, bytes: Vec<u8>) -> u64 {
        let seq = self.records.len() as u64;
        let wall_ns = 100 + seq * 10;
        self.records.push(JournalRecord {
            seq,
            wall_ns,
            direction,
            bytes,
        });
        wall_ns
    }

    fn writes(&mut self, step: Step) {
        for write in step.writes {
            self.push(Direction::In, write);
        }
    }

    fn feed(&mut self, session: &mut AcpSession, bytes: Vec<u8>) {
        let wall_ns = self.push(Direction::Out, bytes.clone());
        let step = session.feed(&bytes, wall_ns);
        self.writes(step);
    }
}

/// A live session that was asked for 1 and 2, with the journal of it.
fn live_with_two_requests() -> (AcpSession, Journal) {
    let mut session = session();
    let mut journal = Journal::default();
    journal.writes(session.start());
    journal.feed(&mut session, initialize_response(0));
    journal.feed(&mut session, new_session_response(1, SESSION));
    for id in [1, 2] {
        let call = json!({ "toolCallId": format!("c{id}"), "title": "bash", "kind": "execute" });
        journal.feed(
            &mut session,
            request_line(json!(id), call, offered(&STANDARD)),
        );
    }
    (session, journal)
}

fn rebuilt_from(journal: &Journal) -> AcpSession {
    let mut rebuilt = session();
    let step = rebuilt
        .replay(&journal.records)
        .expect("a live journal replays");
    assert!(step.writes.is_empty());
    rebuilt
}

#[test]
fn after_a_replay_that_has_no_answer_in_the_journal_both_requests_are_pending() {
    let (live, journal) = live_with_two_requests();
    let rebuilt = rebuilt_from(&journal);
    assert_eq!(rebuilt.pending_permissions().len(), 2);
    assert_eq!(rebuilt.pending_permissions(), live.pending_permissions());
    assert_eq!(rebuilt, live);
}

#[test]
fn after_a_replay_a_request_the_journal_shows_answered_is_not_pending() {
    let (mut live, mut journal) = live_with_two_requests();
    journal.writes(live.answer("1", PermissionReply::Allow {}));
    let rebuilt = rebuilt_from(&journal);
    let pending = rebuilt.pending_permissions();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].id, "2");
    assert_eq!(pending[0].options, standard_options());
    assert_eq!(pending, live.pending_permissions());
}

#[test]
fn a_request_the_journal_shows_cancelled_is_not_pending_after_a_replay() {
    let (mut live, mut journal) = live_with_two_requests();
    journal.writes(live.cancel());
    assert!(rebuilt_from(&journal).pending_permissions().is_empty());
}

#[test]
fn a_rebuilt_session_answers_a_request_as_the_live_one_does_with_the_agents_own_id() {
    let (mut live, journal) = live_with_two_requests();
    let mut rebuilt = rebuilt_from(&journal);
    assert_eq!(
        rebuilt.answer("2", PermissionReply::Allow {}),
        live.answer("2", PermissionReply::Allow {})
    );
}

#[test]
fn a_rebuilt_session_cancels_as_the_live_one_does() {
    let (mut live, journal) = live_with_two_requests();
    let mut rebuilt = rebuilt_from(&journal);
    assert_eq!(rebuilt.cancel(), live.cancel());
}

/// How many records of `journal` are the answer to the agent's request `id`, an `in` record with a result.
fn answers_to(journal: &Journal, id: Value) -> usize {
    journal
        .records
        .iter()
        .filter(|record| record.direction == Direction::In)
        .filter_map(|record| serde_json::from_slice::<Value>(&record.bytes).ok())
        .filter(|message| message["id"] == id && message.get("result").is_some())
        .count()
}

/// A live session that was asked for 1 and 2 and then got the prompt response. The two answers it wrote are the last
/// two records of the journal.
fn live_with_a_turn_end() -> (AcpSession, Journal) {
    let (mut live, mut journal) = live_with_two_requests();
    journal.feed(&mut live, prompt_response(2, "end_turn", None));
    (live, journal)
}

#[test]
fn a_journal_cut_after_the_prompt_response_has_nothing_pending_and_the_session_is_the_live_one() {
    let (live, journal) = live_with_a_turn_end();
    // The cut is just after the response: the two answers the live session wrote are not in it, so they are owed, and
    // owed they are the bytes the live session wrote (ts-0092-tw).
    let cut = Journal {
        records: journal.records[..journal.records.len() - 2].to_vec(),
    };
    let mut rebuilt = rebuilt_from(&cut);
    assert!(rebuilt.pending_permissions().is_empty());
    let wrote: Vec<Vec<u8>> = journal.records[journal.records.len() - 2..]
        .iter()
        .map(|record| record.bytes.clone())
        .collect();
    assert_eq!(rebuilt.take_owed_writes(), wrote);
    assert_eq!(rebuilt, live);
}

#[test]
fn a_journal_that_holds_the_answers_of_a_turn_end_replays_to_the_same_state_and_answers_nothing_more()
 {
    let (live, journal) = live_with_a_turn_end();
    assert_eq!(answers_to(&journal, json!(1)), 1);
    assert_eq!(answers_to(&journal, json!(2)), 1);
    let rebuilt = rebuilt_from(&journal);
    assert!(rebuilt.pending_permissions().is_empty());
    assert_eq!(rebuilt, live);
}

/// A live session that has cancelled its turn and then read a request, and the journal of it. The answer the read
/// wrote is the last record.
fn live_with_a_request_after_a_cancel() -> (AcpSession, Journal) {
    let mut live = session();
    let mut journal = Journal::default();
    journal.writes(live.start());
    journal.feed(&mut live, initialize_response(0));
    journal.feed(&mut live, new_session_response(1, SESSION));
    journal.writes(live.cancel());
    journal.feed(
        &mut live,
        request_line(json!(7), bash_call(), offered(&STANDARD)),
    );
    (live, journal)
}

#[test]
fn a_request_read_after_a_cancel_is_answered_once_in_the_journal_and_a_replay_adds_no_second_answer()
 {
    let (live, journal) = live_with_a_request_after_a_cancel();
    assert_eq!(answers_to(&journal, json!(7)), 1);
    let rebuilt = rebuilt_from(&journal);
    assert!(rebuilt.pending_permissions().is_empty());
    assert_eq!(rebuilt, live);
}

#[test]
fn a_journal_cut_before_the_answer_to_a_request_after_a_cancel_leaves_nothing_pending() {
    // A replay writes nothing, so the answer that was not journaled is owed, and owed it is the one the live session
    // wrote (ts-0092-tw).
    let (live, journal) = live_with_a_request_after_a_cancel();
    let cut = Journal {
        records: journal.records[..journal.records.len() - 1].to_vec(),
    };
    let mut rebuilt = rebuilt_from(&cut);
    assert!(rebuilt.pending_permissions().is_empty());
    assert_eq!(answers_to(&cut, json!(7)), 0);
    let wrote = journal.records[journal.records.len() - 1].bytes.clone();
    assert_eq!(rebuilt.take_owed_writes(), vec![wrote]);
    assert_eq!(rebuilt, live);
}

// ---- cancel --------------------------------------------------------------------------------------------------------

#[test]
fn cancel_answers_each_pending_request_cancelled_and_then_writes_session_cancel() {
    let mut session = ready();
    session.feed(
        &request_line(json!(7), bash_call(), offered(&STANDARD)),
        500,
    );
    session.feed(
        &request_line(json!("b"), bash_call(), offered(&STANDARD)),
        510,
    );
    let step = session.cancel();
    assert_eq!(
        written(&step),
        vec![cancelled(json!(7)), cancelled(json!("b")), session_cancel()]
    );
    assert!(step.events.is_empty());
    assert!(session.pending_permissions().is_empty());
    assert_eq!(
        session.answer("7", PermissionReply::Allow {}),
        Step::default(),
        "a request cancel answered is answered"
    );
}

#[test]
fn session_cancel_is_a_notification_with_no_id_for_the_session_id() {
    let mut session = ready();
    let step = session.cancel();
    let messages = written(&step);
    assert_eq!(messages, vec![session_cancel()]);
    assert!(!messages[0].as_object().unwrap().contains_key("id"));
}

#[test]
fn cancel_with_no_pending_request_writes_session_cancel_alone() {
    let mut session = ready();
    session.feed(&message_chunk(SESSION, "working"), 500);
    assert_eq!(written(&session.cancel()), vec![session_cancel()]);
}

#[test]
fn a_second_cancel_writes_nothing() {
    let mut session = ready();
    session.feed(
        &request_line(json!(7), bash_call(), offered(&STANDARD)),
        500,
    );
    assert_eq!(session.cancel().writes.len(), 2);
    assert_eq!(session.cancel(), Step::default());
}

#[test]
fn a_request_asked_after_a_cancel_is_answered_cancelled_at_once_and_never_held() {
    // The agent may wait on the answer, and the turn cannot end while it does. So the read writes, as `-32601` does.
    let mut session = ready();
    session.cancel();
    let step = session.feed(
        &request_line(json!(7), bash_call(), offered(&STANDARD)),
        500,
    );
    assert_eq!(written(&step), vec![cancelled(json!(7))]);
    assert_eq!(
        events_of(&step),
        vec![AgentEvent::Other {
            raw_type: "permission_request_after_cancel".into(),
            raw: None
        }]
    );
    assert!(session.pending_permissions().is_empty());
    assert_eq!(session.cancel(), Step::default(), "a second cancel");
    assert_eq!(
        session.answer("7", PermissionReply::Cancel {}),
        Step::default(),
        "nothing is held to answer"
    );
}

#[test]
fn a_request_after_a_cancel_keeps_the_agents_id_as_a_string() {
    let mut session = ready();
    session.cancel();
    let step = session.feed(
        &request_line(json!("perm-stand-in-1"), bash_call(), offered(&STANDARD)),
        500,
    );
    assert_eq!(written(&step), vec![cancelled(json!("perm-stand-in-1"))]);
}

#[test]
fn a_request_asked_before_a_cancel_and_one_after_it_are_each_answered_once() {
    let mut session = ready();
    session.feed(
        &request_line(json!(1), bash_call(), offered(&STANDARD)),
        500,
    );
    assert_eq!(written(&session.cancel()).len(), 2);
    let step = session.feed(
        &request_line(json!(2), bash_call(), offered(&STANDARD)),
        510,
    );
    assert_eq!(written(&step), vec![cancelled(json!(2))]);
}

#[test]
fn a_request_asked_in_the_turn_after_a_cancelled_one_is_held() {
    let mut session = ready();
    session.cancel();
    session.feed(&prompt_response(2, "cancelled", None), 600);
    session.send_turn("again");
    let step = session.feed(
        &request_line(json!(7), bash_call(), offered(&STANDARD)),
        700,
    );
    assert!(step.writes.is_empty());
    assert_eq!(session.pending_permissions().len(), 1);
}

// ---- the end of a turn clears what is held -------------------------------------------------------------------------

#[test]
fn a_turn_end_answers_the_request_still_held_cancelled_and_drops_it() {
    let mut session = asked(json!(7), &STANDARD);
    let step = session.feed(&prompt_response(2, "end_turn", None), 600);
    assert_eq!(written(&step), vec![cancelled(json!(7))]);
    assert!(session.pending_permissions().is_empty());
    assert_eq!(
        session.answer("7", PermissionReply::Allow {}),
        Step::default(),
        "an unknown id, as before"
    );
}

#[test]
fn a_turn_end_answers_two_held_requests_oldest_first_with_their_own_ids() {
    let mut session = ready();
    session.feed(
        &request_line(json!(9), bash_call(), offered(&STANDARD)),
        500,
    );
    session.feed(
        &request_line(json!("b"), bash_call(), offered(&STANDARD)),
        510,
    );
    let step = session.feed(&prompt_response(2, "end_turn", None), 600);
    assert_eq!(
        written(&step),
        vec![cancelled(json!(9)), cancelled(json!("b"))]
    );
    assert!(session.pending_permissions().is_empty());
}

#[test]
fn a_turn_end_by_any_response_clears_what_is_held() {
    for response in [
        prompt_response(2, "refusal", None),
        prompt_response(2, "a_reason_of_a_later_version", None),
        prompt_error(2, -32603, "Internal error"),
    ] {
        let mut session = asked(json!(7), &STANDARD);
        let step = session.feed(&response, 600);
        assert_eq!(written(&step), vec![cancelled(json!(7))]);
        assert!(session.pending_permissions().is_empty());
    }
}

#[test]
fn the_request_is_dropped_before_the_turn_end_event_and_the_events_are_the_same() {
    let mut with_request = asked(json!(7), &STANDARD);
    let mut without = ready();
    let held = with_request.feed(&prompt_response(2, "end_turn", None), 600);
    let none = without.feed(&prompt_response(2, "end_turn", None), 600);
    // The ask counts as output, so only `empty` differs: nothing else is made for the answer.
    let as_if_empty = |mut events: Vec<StampedEvent>| {
        for stamped in &mut events {
            if let AgentEvent::TurnEnd { empty, .. } = &mut stamped.event {
                *empty = true;
            }
        }
        events
    };
    assert_eq!(
        as_if_empty(held.events.clone()),
        none.events,
        "no event for the answer"
    );
    assert!(matches!(
        events_of(&held).last(),
        Some(AgentEvent::TurnEnd { .. })
    ));
    assert!(
        none.writes.is_empty(),
        "a turn end with nothing held writes nothing"
    );
}

#[test]
fn a_turn_end_after_a_cancel_writes_nothing_because_cancel_answered_the_requests() {
    let mut session = asked(json!(7), &STANDARD);
    session.cancel();
    let step = session.feed(&prompt_response(2, "end_turn", None), 600);
    assert!(step.writes.is_empty());
}

#[test]
fn a_request_cleared_by_a_turn_end_is_not_asked_again_by_the_next_turn() {
    let mut session = asked(json!(7), &STANDARD);
    session.feed(&prompt_response(2, "end_turn", None), 600);
    session.send_turn("again");
    assert!(session.pending_permissions().is_empty());
    let step = session.feed(&prompt_response(3, "end_turn", None), 700);
    assert!(step.writes.is_empty());
}

#[test]
fn cancel_before_the_session_has_an_id_writes_nothing() {
    let mut fresh = session();
    assert_eq!(fresh.cancel(), Step::default());
    fresh.start();
    assert_eq!(fresh.cancel(), Step::default());
    fresh.feed(&initialize_response(0), 100);
    assert_eq!(fresh.cancel(), Step::default());
}

#[test]
fn cancel_after_the_turn_ended_writes_nothing() {
    let mut session = ready();
    session.feed(&prompt_response(2, "end_turn", None), 600);
    assert_eq!(session.cancel(), Step::default());
}

#[test]
fn cancel_cancels_the_turn_a_later_prompt_opened_and_a_turn_after_it_is_not_cancelled() {
    let mut session = ready();
    session.feed(&prompt_response(2, "end_turn", None), 600);
    session.send_turn("again");
    assert_eq!(written(&session.cancel()), vec![session_cancel()]);
    let step = session.feed(&prompt_response(3, "end_turn", None), 700);
    assert_eq!(turn_end_of(&step), (TurnEndReason::Cancelled {}, true));
    session.send_turn("and again");
    let step = session.feed(&prompt_response(4, "end_turn", None), 800);
    assert_eq!(turn_end_of(&step), (TurnEndReason::EndTurn {}, false));
}

#[test]
fn a_turn_the_host_cancelled_ends_cancelled_whatever_the_agent_says() {
    for word in ["end_turn", "cancelled", "refusal"] {
        let mut session = ready();
        session.cancel();
        let step = session.feed(&prompt_response(2, word, None), 600);
        assert_eq!(
            turn_end_of(&step),
            (TurnEndReason::Cancelled {}, true),
            "{word}"
        );
        assert_eq!(
            session.finish(&exited()).outcome,
            PhaseOutcome::Stopped,
            "{word}"
        );
    }
}

#[test]
fn a_turn_the_host_did_not_cancel_ends_as_the_agent_says() {
    for (word, reason) in [
        ("end_turn", TurnEndReason::EndTurn {}),
        ("cancelled", TurnEndReason::Cancelled {}),
        ("refusal", TurnEndReason::Refusal {}),
    ] {
        let mut session = ready();
        let step = session.feed(&prompt_response(2, word, None), 600);
        assert_eq!(turn_end_of(&step), (reason, false), "{word}");
    }
}

#[test]
fn a_cancel_that_arrives_when_the_agent_has_already_answered_the_prompt_is_not_a_cancel() {
    let mut session = ready();
    session.feed(&prompt_response(2, "refusal", None), 600);
    session.cancel();
    assert_eq!(
        session.finish(&exited()).outcome,
        PhaseOutcome::Failed {
            cause: PhaseFailure::Refusal
        }
    );
}

// ---- under the real shim -------------------------------------------------------------------------------------------

fn options() -> AcpOptions {
    AcpOptions::new("/work/repo", "read the notes")
}

fn allow(session: &mut AcpSession, event: &AgentEvent) -> Step {
    match event {
        AgentEvent::PermissionRequest { request } => {
            session.answer(&request.id, PermissionReply::Allow {})
        }
        _ => Step::default(),
    }
}

fn kinds(events: &[AgentEvent]) -> Vec<&'static str> {
    events
        .iter()
        .map(|event| match event {
            AgentEvent::SessionBound { .. } => "bound",
            AgentEvent::Message { .. } => "message",
            AgentEvent::ToolCall { .. } => "tool_call",
            AgentEvent::PermissionRequest { .. } => "permission_request",
            AgentEvent::PermissionUnanswerable { .. } => "unanswerable",
            AgentEvent::Usage { .. } => "usage",
            AgentEvent::FinalReport { .. } => "final_report",
            AgentEvent::TurnEnd { .. } => "turn_end",
            other => panic!("an event the turn does not make: {other:?}"),
        })
        .collect()
}

/// The messages the session wrote, from the journal the shim kept.
fn written_in(journal: &[u8]) -> Vec<Value> {
    read_journal(journal)
        .records
        .iter()
        .filter(|record| record.direction == Direction::In)
        .map(|record| serde_json::from_slice(&record.bytes).unwrap())
        .collect()
}

#[test]
fn a_request_answered_allow_lets_the_child_go_on_to_the_end_of_its_turn() {
    let played = play(&recording("permission-request"), options(), &[], allow);
    let events = played.agent_events();
    assert_eq!(
        kinds(&events),
        [
            "bound",
            "permission_request",
            "tool_call",
            "message",
            "usage",
            "final_report",
            "turn_end"
        ]
    );
    assert_eq!(request_of(&events[1]).id, "7");
    assert!(played.session.pending_permissions().is_empty());
    assert_eq!(
        played.session.clone().finish(&played.exit).outcome,
        PhaseOutcome::Ended
    );
}

#[test]
fn a_request_answered_deny_writes_the_reject_option_and_no_message() {
    let played = play(
        &recording("permission-deny"),
        options(),
        &[],
        |session, event| match event {
            AgentEvent::PermissionRequest { request } => session.answer(
                &request.id,
                PermissionReply::Deny {
                    message: Some("SECRET-STAND-IN not here".into()),
                },
            ),
            _ => Step::default(),
        },
    );
    assert_eq!(kinds(&played.agent_events()).last(), Some(&"turn_end"));
    let journal = String::from_utf8_lossy(&played.journal);
    assert!(!journal.contains("SECRET-STAND-IN"));
    let answers: Vec<Value> = written_in(&played.journal)
        .into_iter()
        .filter(|message| message.get("result").is_some() && message["id"] == json!(7))
        .collect();
    assert_eq!(answers, vec![selected(json!(7), "reject")]);
}

#[test]
fn a_request_with_no_one_time_allow_is_answered_cancelled_and_reported_unanswerable() {
    let played = play(&recording("permission-unanswerable"), options(), &[], allow);
    let events = played.agent_events();
    assert!(
        events.contains(&AgentEvent::PermissionUnanswerable {
            request_id: "7".into()
        }),
        "{events:?}"
    );
    assert_eq!(kinds(&events).last(), Some(&"turn_end"), "{events:?}");
}

#[test]
fn a_cancel_mid_turn_writes_session_cancel_and_the_turn_ends_cancelled() {
    let played = play(
        &recording("cancel-mid-turn"),
        options(),
        &[],
        |session, event| match event {
            AgentEvent::Message { .. } => session.cancel(),
            _ => Step::default(),
        },
    );
    let events = played.agent_events();
    let Some(AgentEvent::TurnEnd {
        reason,
        cancel_requested,
        ..
    }) = events.last()
    else {
        panic!("{events:?}");
    };
    assert_eq!(
        (reason, cancel_requested),
        (&TurnEndReason::Cancelled {}, &true),
        "the child said end_turn"
    );
    let cancels: Vec<Value> = written_in(&played.journal)
        .into_iter()
        .filter(|message| message["method"] == json!("session/cancel"))
        .collect();
    assert_eq!(cancels, vec![session_cancel()]);
    assert_eq!(
        played.session.clone().finish(&played.exit).outcome,
        PhaseOutcome::Stopped
    );
}

#[test]
fn a_cancel_with_a_request_pending_answers_it_first_and_the_turn_ends_cancelled() {
    let played = play(
        &recording("permission-cancel"),
        options(),
        &[],
        |session, event| match event {
            AgentEvent::PermissionRequest { .. } => session.cancel(),
            _ => Step::default(),
        },
    );
    let order: Vec<Value> = written_in(&played.journal).into_iter().skip(3).collect();
    assert_eq!(
        order,
        vec![cancelled(json!("perm-stand-in-1")), session_cancel()]
    );
    assert!(played.session.pending_permissions().is_empty());
    assert_eq!(
        played.session.clone().finish(&played.exit).outcome,
        PhaseOutcome::Stopped
    );
}

#[test]
fn a_journal_cut_while_the_request_waits_has_it_pending_and_the_whole_one_has_not() {
    let played = play(&recording("permission-request"), options(), &[], allow);
    let read = read_journal(&played.journal);
    let asked = read
        .records
        .iter()
        .position(|record| {
            record.direction == Direction::Out
                && String::from_utf8_lossy(&record.bytes).contains("request_permission")
        })
        .expect("the request is in the journal");
    let end = |count: usize| -> usize {
        read.records[..count]
            .iter()
            .map(|record| RECORD_HEADER_BYTES + record.bytes.len())
            .sum()
    };

    let mut waiting = AcpSession::new(options());
    replay_journal_bytes(&mut waiting, &played.journal[..end(asked + 1)]).unwrap();
    let pending = waiting.pending_permissions();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].id, "7");
    assert_eq!(pending[0].kind, ToolKind::Execute);
    assert_eq!(pending[0].input, None);
    assert_eq!(pending[0].options.len(), 3);

    let mut whole = AcpSession::new(options());
    replay_journal_bytes(&mut whole, &played.journal).unwrap();
    assert!(whole.pending_permissions().is_empty());
}

#[test]
fn a_turn_that_ends_with_a_request_open_answers_it_cancelled_and_leaves_nothing_pending() {
    // The recording expects the answer after it ended its turn; the host writes it when it reads the response.
    let played = play(
        &recording("permission-open-at-turn-end"),
        options(),
        &[],
        |_, _| Step::default(),
    );
    let events = played.agent_events();
    assert_eq!(request_of(&events[1]).id, "7");
    assert_eq!(kinds(&events).last(), Some(&"turn_end"));
    assert!(played.session.pending_permissions().is_empty());
    let messages = written_in(&played.journal);
    assert_eq!(messages.last(), Some(&cancelled(json!(7))));
    assert_eq!(
        messages
            .iter()
            .filter(|message| message["id"] == json!(7))
            .count(),
        1
    );
    let mut rebuilt = AcpSession::new(options());
    replay_journal_bytes(&mut rebuilt, &played.journal).unwrap();
    assert!(rebuilt.pending_permissions().is_empty());
    assert_eq!(rebuilt, played.session);
}

#[test]
fn a_request_read_after_a_cancel_is_answered_at_once_and_the_turn_ends_cancelled() {
    let played = play(
        &recording("permission-after-cancel"),
        options(),
        &[],
        |session, event| match event {
            AgentEvent::Message { .. } => session.cancel(),
            _ => Step::default(),
        },
    );
    let events = played.agent_events();
    assert!(
        events.contains(&AgentEvent::Other {
            raw_type: "permission_request_after_cancel".into(),
            raw: None
        }),
        "{events:?}"
    );
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, AgentEvent::PermissionRequest { .. }))
    );
    assert!(played.session.pending_permissions().is_empty());
    let messages = written_in(&played.journal);
    assert_eq!(&messages[3..], [session_cancel(), cancelled(json!(7))]);
    assert_eq!(
        played.session.clone().finish(&played.exit).outcome,
        PhaseOutcome::Stopped
    );
}
