//! How an ACP phase ends (ts-0091-pz; 0.3, The stream protocol): the `session/prompt` response is the outcome, a
//! protocol with no failure flag must not let a silent failure look like success, and a child that goes away with no
//! response leaves nothing known. The turns are fed by hand, as bytes; the fake child under the real shim comes last.

use super::fake::{play, play_until_exit, recording};
use super::session::{SESSION, events_of, exited, ready, session};
use collide_core::adapter::{
    AgentSession, EVIDENCE_BYTES, ExitInfo, PhaseFailure, PhaseOutcome, Step,
};
use collide_core::agent::{AgentEvent, PermissionReply, TurnEndReason};
use collide_core::shim::{ExitEnd, ExitRecord, SHIM_PROTOCOL};
use collide_core::testing::acp::{
    initialize_response, message_chunk, new_session_response, permission_request, prompt_error,
    prompt_response, standard_options, tool_call,
};
use collide_host::adapters::acp::options::AcpOptions;
use collide_host::adapters::acp::session::AcpSession;
use serde_json::json;

fn exit(end: ExitEnd, stderr: &str) -> ExitInfo {
    ExitInfo {
        record: Some(ExitRecord {
            protocol: SHIM_PROTOCOL,
            end,
            cpu_user_us: 10,
            cpu_system_us: 5,
            max_rss_bytes: 4096,
        }),
        stderr_tail: stderr.to_string(),
    }
}

fn exit_0_with(stderr: &str) -> ExitInfo {
    exit(ExitEnd::Exited { code: 0 }, stderr)
}

fn failed(cause: PhaseFailure) -> PhaseOutcome {
    PhaseOutcome::Failed { cause }
}

fn turn_end_empty(step: &Step) -> bool {
    match events_of(step).last() {
        Some(AgentEvent::TurnEnd { empty, .. }) => *empty,
        other => panic!("expected a turn end last, got {other:?}"),
    }
}

/// A ready session that has read `lines` and then the response to its first prompt.
fn ended_with(lines: &[Vec<u8>], response: &[u8]) -> (AcpSession, Step) {
    let mut session = ready();
    for line in lines {
        session.feed(line, 500);
    }
    let step = session.feed(response, 600);
    (session, step)
}

fn end_turn() -> Vec<u8> {
    prompt_response(2, "end_turn", None)
}

fn a_call(id: &str) -> Vec<u8> {
    tool_call(SESSION, id, "Read notes.txt", "read")
}

// ---- an empty turn --------------------------------------------------------------------------------------------------

#[test]
fn an_end_turn_with_no_words_no_tool_call_and_no_usage_is_an_empty_turn() {
    let (mut session, step) = ended_with(&[], &end_turn());
    assert!(turn_end_empty(&step));
    let finished = session.finish(&exit_0_with(""));
    assert_eq!(finished.outcome, failed(PhaseFailure::EmptyTurn));
}

#[test]
fn an_empty_turn_keeps_the_end_of_the_childs_stderr_as_its_evidence() {
    let (mut session, _) = ended_with(&[], &end_turn());
    let finished = session.finish(&exit_0_with(
        "AI_APICallError: model 'stand-in:1b' not found\n",
    ));
    assert_eq!(
        finished.evidence.as_deref(),
        Some("AI_APICallError: model 'stand-in:1b' not found"),
        "the text, with the newline that ended it dropped"
    );
}

#[test]
fn an_empty_turn_with_no_stderr_has_no_evidence() {
    for stderr in ["", "\n  \n"] {
        let (mut session, _) = ended_with(&[], &end_turn());
        let finished = session.finish(&exit_0_with(stderr));
        assert_eq!(finished.outcome, failed(PhaseFailure::EmptyTurn));
        assert_eq!(finished.evidence, None, "{stderr:?}");
    }
}

#[test]
fn a_turn_with_usage_and_no_words_is_not_empty() {
    let usage = json!({ "inputTokens": 34, "outputTokens": 0 });
    let response = prompt_response(2, "end_turn", Some(usage));
    let (mut session, step) = ended_with(&[], &response);
    assert!(!turn_end_empty(&step));
    let finished = session.finish(&exit_0_with("noise"));
    assert_eq!(finished.outcome, PhaseOutcome::Ended);
    assert_eq!(finished.evidence, None, "an ended phase keeps no evidence");
}

#[test]
fn a_turn_with_a_tool_call_and_no_words_is_not_empty() {
    let (mut session, step) = ended_with(&[a_call("c1")], &end_turn());
    assert!(!turn_end_empty(&step));
    assert_eq!(
        session.finish(&exit_0_with("")).outcome,
        PhaseOutcome::Ended
    );
}

#[test]
fn a_turn_with_words_and_no_usage_is_not_empty() {
    let (mut session, step) = ended_with(&[message_chunk(SESSION, "done")], &end_turn());
    assert!(!turn_end_empty(&step));
    assert_eq!(
        session.finish(&exit_0_with("")).outcome,
        PhaseOutcome::Ended
    );
}

#[test]
fn the_whole_turn_is_read_not_the_last_chunk() {
    // Words, then a tool call that clears the report, then nothing: the turn had output.
    let lines = [message_chunk(SESSION, "Let me look. "), a_call("c1")];
    let (mut session, step) = ended_with(&lines, &end_turn());
    assert!(!turn_end_empty(&step));
    assert_eq!(
        session.finish(&exit_0_with("")).outcome,
        PhaseOutcome::Ended
    );
    // A tool call, then an empty-text chunk, then nothing: still a turn with output.
    let lines = [a_call("c1"), message_chunk(SESSION, "")];
    let (mut session, step) = ended_with(&lines, &end_turn());
    assert!(!turn_end_empty(&step));
    assert_eq!(
        session.finish(&exit_0_with("")).outcome,
        PhaseOutcome::Ended
    );
}

#[test]
fn a_chunk_with_no_text_is_not_output() {
    let (mut session, step) = ended_with(&[message_chunk(SESSION, "")], &end_turn());
    assert!(turn_end_empty(&step));
    assert_eq!(
        session.finish(&exit_0_with("")).outcome,
        failed(PhaseFailure::EmptyTurn)
    );
}

#[test]
fn a_usage_of_zeros_with_no_output_is_still_an_empty_turn() {
    // S7, O4: the agent answered a missing model with `end_turn` and zeros.
    let usage = json!({ "inputTokens": 0, "outputTokens": 0, "totalTokens": 0 });
    let response = prompt_response(2, "end_turn", Some(usage));
    let (mut session, step) = ended_with(&[], &response);
    assert!(turn_end_empty(&step));
    assert_eq!(
        session.finish(&exit_0_with("")).outcome,
        failed(PhaseFailure::EmptyTurn)
    );
}

#[test]
fn a_permission_request_counts_as_a_tool_call() {
    // The agent may ask before it announces the call (S7, O6): the ask is the call.
    let call = json!({ "toolCallId": "c1", "title": "bash", "kind": "execute", "rawInput": {} });
    let ask = permission_request(7, SESSION, call, standard_options());
    let (mut session, step) = ended_with(&[ask], &end_turn());
    assert!(!turn_end_empty(&step));
    assert_eq!(
        session.finish(&exit_0_with("")).outcome,
        PhaseOutcome::Ended
    );
}

#[test]
fn the_next_turn_starts_with_no_output() {
    let (mut session, _) = ended_with(&[message_chunk(SESSION, "first")], &end_turn());
    session.send_turn("again");
    let step = session.feed(&prompt_response(3, "end_turn", None), 700);
    assert!(
        turn_end_empty(&step),
        "the first turn's words are not this turn's"
    );
    assert_eq!(
        session.finish(&exit_0_with("")).outcome,
        failed(PhaseFailure::EmptyTurn)
    );
}

// ---- the other stop reasons -----------------------------------------------------------------------------------------

#[test]
fn refusal_max_tokens_and_max_turn_requests_give_their_causes_even_when_the_turn_is_empty() {
    for (word, cause) in [
        ("refusal", PhaseFailure::Refusal),
        ("max_tokens", PhaseFailure::MaxTokens),
        ("max_turn_requests", PhaseFailure::MaxTurns),
    ] {
        let (mut session, _) = ended_with(&[], &prompt_response(2, word, None));
        let finished = session.finish(&exit_0_with("stderr text"));
        assert_eq!(finished.outcome, failed(cause), "{word}");
        assert_eq!(finished.evidence, None, "{word}");
    }
}

#[test]
fn an_error_response_is_an_api_failure_that_keeps_the_errors_code_and_nothing_else() {
    let response = prompt_error(2, -32603, "Internal error SECRET-STAND-IN");
    let (mut session, _) = ended_with(&[], &response);
    let finished = session.finish(&exit_0_with("stderr SECRET-STAND-IN"));
    assert_eq!(finished.outcome, failed(PhaseFailure::Api { status: None }));
    let evidence = finished.evidence.expect("the code is the evidence");
    assert!(evidence.contains("-32603"), "{evidence}");
    assert!(!evidence.contains("SECRET-STAND-IN"), "{evidence}");
}

#[test]
fn an_error_response_with_words_before_it_is_still_an_api_failure() {
    let (mut session, _) = ended_with(&[message_chunk(SESSION, "half")], &prompt_error(2, -1, "x"));
    assert_eq!(
        session.finish(&exit_0_with("")).outcome,
        failed(PhaseFailure::Api { status: None })
    );
}

#[test]
fn an_unknown_stop_reason_is_an_api_failure_with_no_evidence() {
    let (mut session, _) = ended_with(
        &[],
        &prompt_response(2, "a_reason_of_a_later_version", None),
    );
    let finished = session.finish(&exit_0_with("noise"));
    assert_eq!(finished.outcome, failed(PhaseFailure::Api { status: None }));
    assert_eq!(finished.evidence, None);
}

// ---- cancel ---------------------------------------------------------------------------------------------------------

#[test]
fn a_cancelled_turn_is_stopped_even_when_the_agent_said_end_turn_with_nothing() {
    // S7, O1: opencode answers a cancel with `end_turn`; with no output and no usage it would read as empty.
    let mut session = ready();
    session.cancel();
    let step = session.feed(&end_turn(), 600);
    assert!(matches!(
        events_of(&step).last(),
        Some(AgentEvent::TurnEnd {
            reason: TurnEndReason::Cancelled {},
            cancel_requested: true,
            ..
        })
    ));
    let finished = session.finish(&exit_0_with("noise"));
    assert_eq!(finished.outcome, PhaseOutcome::Stopped);
    assert_eq!(finished.evidence, None);
}

#[test]
fn a_cancelled_turn_with_a_request_held_is_stopped() {
    let call = json!({ "toolCallId": "c1", "title": "bash", "kind": "execute", "rawInput": {} });
    let mut session = ready();
    session.feed(
        &permission_request(7, SESSION, call, standard_options()),
        500,
    );
    session.cancel();
    session.feed(&end_turn(), 600);
    assert_eq!(
        session.finish(&exit_0_with("")).outcome,
        PhaseOutcome::Stopped
    );
}

// ---- a child that went away -----------------------------------------------------------------------------------------

#[test]
fn a_child_that_exits_with_no_prompt_response_is_lost_whatever_its_exit() {
    for end in [
        ExitEnd::Exited { code: 0 },
        ExitEnd::Exited { code: 1 },
        ExitEnd::Signaled { signal: 9 },
    ] {
        let mut session = ready();
        session.feed(&message_chunk(SESSION, "some words"), 500);
        let finished = session.finish(&exit(end, "it said why"));
        assert_eq!(finished.outcome, PhaseOutcome::Lost, "{end:?}");
        assert_eq!(finished.evidence, None, "{end:?}");
    }
}

#[test]
fn a_child_that_exits_0_with_a_request_pending_is_lost_and_not_ended() {
    // S7, O7.
    let call = json!({ "toolCallId": "c1", "title": "bash", "kind": "execute", "rawInput": {} });
    let mut session = ready();
    session.feed(
        &permission_request(7, SESSION, call, standard_options()),
        500,
    );
    assert_eq!(session.pending_permissions().len(), 1);
    assert_eq!(session.finish(&exit_0_with("")).outcome, PhaseOutcome::Lost);
}

#[test]
fn a_child_that_exits_in_the_handshake_is_lost() {
    let mut fresh = session();
    fresh.start();
    assert_eq!(
        fresh
            .finish(&exit(ExitEnd::Exited { code: 1 }, "boom"))
            .outcome,
        PhaseOutcome::Lost,
        "before `initialize` was answered"
    );
    let mut asked = session();
    asked.start();
    asked.feed(&initialize_response(0), 100);
    assert_eq!(asked.finish(&exited()).outcome, PhaseOutcome::Lost);
    let mut bound = session();
    bound.start();
    bound.feed(&initialize_response(0), 100);
    bound.feed(&new_session_response(1, SESSION), 110);
    assert_eq!(
        bound.finish(&exited()).outcome,
        PhaseOutcome::Lost,
        "after `session/new`, with the first prompt out and unanswered"
    );
}

#[test]
fn a_shim_that_left_no_exit_record_is_lost_even_after_a_good_turn() {
    let (mut session, _) = ended_with(&[message_chunk(SESSION, "done")], &end_turn());
    let info = ExitInfo {
        record: None,
        stderr_tail: String::new(),
    };
    assert_eq!(session.finish(&info).outcome, PhaseOutcome::Lost);
}

#[test]
fn a_second_turn_with_no_response_is_lost_and_not_the_first_turns_outcome() {
    let (mut session, _) = ended_with(&[message_chunk(SESSION, "done")], &end_turn());
    session.send_turn("again");
    assert_eq!(session.finish(&exit_0_with("")).outcome, PhaseOutcome::Lost);
}

#[test]
fn a_permission_answer_does_not_settle_a_turn() {
    let call = json!({ "toolCallId": "c1", "title": "bash", "kind": "execute", "rawInput": {} });
    let mut session = ready();
    session.feed(
        &permission_request(7, SESSION, call, standard_options()),
        500,
    );
    session.answer("7", PermissionReply::Allow {});
    assert_eq!(session.finish(&exit_0_with("")).outcome, PhaseOutcome::Lost);
}

// ---- the evidence's bound -------------------------------------------------------------------------------------------

fn evidence_of(stderr: &str) -> Option<String> {
    let (mut session, _) = ended_with(&[], &end_turn());
    session.finish(&exit_0_with(stderr)).evidence
}

#[test]
fn evidence_shorter_than_the_bound_is_kept_whole() {
    let text = "x".repeat(EVIDENCE_BYTES);
    assert_eq!(evidence_of(&text), Some(text));
}

#[test]
fn evidence_longer_than_the_bound_is_cut_from_the_front_and_keeps_the_end() {
    let text = format!("{}END", "a".repeat(EVIDENCE_BYTES * 3));
    let kept = evidence_of(&text).expect("evidence");
    assert_eq!(kept.len(), EVIDENCE_BYTES);
    assert!(kept.ends_with("aaEND"), "from the end of stderr");
}

#[test]
fn evidence_cut_inside_a_multi_byte_character_starts_on_the_next_boundary() {
    // Two-byte characters: the bound falls inside one when the text is odd in bytes after a one-byte lead.
    let text = format!("a{}", "\u{e9}".repeat(EVIDENCE_BYTES));
    assert!(text.len() > EVIDENCE_BYTES);
    let kept = evidence_of(&text).expect("evidence");
    assert!(kept.len() <= EVIDENCE_BYTES, "{}", kept.len());
    assert!(kept.chars().all(|c| c == '\u{e9}'), "no half character");
    assert!(text.ends_with(&kept), "it is the end of the text");
    assert_eq!(
        kept.len(),
        EVIDENCE_BYTES,
        "two-byte characters fill 2048 exactly"
    );
    // Three-byte characters: 2048 is not a multiple of three, so the cut must move forward.
    let text = "\u{20ac}".repeat(EVIDENCE_BYTES);
    let kept = evidence_of(&text).expect("evidence");
    assert!(kept.chars().all(|c| c == '\u{20ac}'), "no half character");
    assert_eq!(kept.len(), 2046);
}

// ---- replay ---------------------------------------------------------------------------------------------------------

/// The journal of a live session: what it wrote (`in`) and what it was fed (`out`), in the order they happened.
mod journal {
    use super::*;
    use collide_core::journal::{Direction, JournalRecord, encode_record};

    pub struct Live {
        pub session: AcpSession,
        records: Vec<JournalRecord>,
    }

    impl Live {
        pub fn new() -> Live {
            let mut live = Live {
                session: session(),
                records: Vec::new(),
            };
            let step = live.session.start();
            live.keep_writes(&step);
            live
        }

        fn keep_writes(&mut self, step: &Step) {
            for bytes in &step.writes {
                self.push(Direction::In, bytes);
            }
        }

        fn push(&mut self, direction: Direction, bytes: &[u8]) {
            self.records.push(JournalRecord {
                seq: self.records.len() as u64,
                wall_ns: 1_000 + self.records.len() as u64,
                direction,
                bytes: bytes.to_vec(),
            });
        }

        pub fn feed(&mut self, line: &[u8]) {
            let wall_ns = 1_000 + self.records.len() as u64;
            self.push(Direction::Out, line);
            let step = self.session.feed(line, wall_ns);
            self.keep_writes(&step);
        }

        pub fn cancel(&mut self) {
            let step = self.session.cancel();
            self.keep_writes(&step);
        }

        pub fn bytes(&self) -> Vec<u8> {
            self.records
                .iter()
                .flat_map(|record| encode_record(record).unwrap())
                .collect()
        }
    }
}

use journal::Live;

fn handshake(live: &mut Live) {
    live.feed(&initialize_response(0));
    live.feed(&new_session_response(1, SESSION));
}

/// Replays the live session's journal into a new session and checks both finish the same way.
fn assert_replay_finishes_as_live(mut live: Live, info: &ExitInfo, outcome: PhaseOutcome) {
    let mut rebuilt = session();
    collide_core::replay::replay_journal_bytes(&mut rebuilt, &live.bytes())
        .expect("the journal replays");
    assert_eq!(rebuilt, live.session, "the same state");
    let live_finished = live.session.finish(info);
    assert_eq!(live_finished.outcome, outcome, "the live outcome");
    assert_eq!(
        rebuilt.finish(info),
        live_finished,
        "the rebuilt one agrees"
    );
}

#[test]
fn a_replayed_journal_finishes_as_the_live_session_did_for_each_outcome() {
    let ok = exit_0_with("the model is missing");
    let call = json!({ "toolCallId": "c1", "title": "bash", "kind": "execute", "rawInput": {} });
    type Script = fn(&mut Live);
    let turns: [(&str, Script, PhaseOutcome); 9] = [
        (
            "ended",
            |live| {
                live.feed(&message_chunk(SESSION, "done"));
                live.feed(&end_turn());
            },
            PhaseOutcome::Ended,
        ),
        (
            "empty",
            |live| live.feed(&end_turn()),
            failed(PhaseFailure::EmptyTurn),
        ),
        (
            "usage only",
            |live| {
                let usage = json!({ "inputTokens": 5 });
                live.feed(&prompt_response(2, "end_turn", Some(usage)));
            },
            PhaseOutcome::Ended,
        ),
        (
            "refusal",
            |live| live.feed(&prompt_response(2, "refusal", None)),
            failed(PhaseFailure::Refusal),
        ),
        (
            "max tokens",
            |live| live.feed(&prompt_response(2, "max_tokens", None)),
            failed(PhaseFailure::MaxTokens),
        ),
        (
            "max turn requests",
            |live| live.feed(&prompt_response(2, "max_turn_requests", None)),
            failed(PhaseFailure::MaxTurns),
        ),
        (
            "error response",
            |live| live.feed(&prompt_error(2, -32603, "x")),
            failed(PhaseFailure::Api { status: None }),
        ),
        (
            "cancelled",
            |live| {
                live.cancel();
                live.feed(&end_turn());
            },
            PhaseOutcome::Stopped,
        ),
        (
            "no response",
            |live| live.feed(&tool_call(SESSION, "c", "t", "read")),
            PhaseOutcome::Lost,
        ),
    ];
    for (name, script, outcome) in turns {
        let mut live = Live::new();
        handshake(&mut live);
        script(&mut live);
        println!("replaying: {name}");
        assert_replay_finishes_as_live(live, &ok, outcome);
    }
    // A request pending when the child goes: lost.
    let mut live = Live::new();
    handshake(&mut live);
    live.feed(&permission_request(7, SESSION, call, standard_options()));
    assert_replay_finishes_as_live(live, &ok, PhaseOutcome::Lost);
    // The handshake cut short.
    let mut live = Live::new();
    live.feed(&initialize_response(0));
    assert_replay_finishes_as_live(live, &ok, PhaseOutcome::Lost);
}

#[test]
fn a_replayed_empty_turn_has_the_same_evidence_as_the_live_one() {
    let mut live = Live::new();
    handshake(&mut live);
    live.feed(&end_turn());
    let info = exit_0_with("AI_APICallError: model not found\n");
    let mut rebuilt = session();
    collide_core::replay::replay_journal_bytes(&mut rebuilt, &live.bytes()).unwrap();
    let finished = rebuilt.finish(&info);
    assert_eq!(finished.outcome, failed(PhaseFailure::EmptyTurn));
    assert_eq!(
        finished.evidence.as_deref(),
        Some("AI_APICallError: model not found")
    );
}

// ---- the fake child under the real shim -----------------------------------------------------------------------------

fn options() -> AcpOptions {
    AcpOptions::new("/work/repo", "read the notes")
}

#[test]
fn a_child_that_answers_end_turn_and_writes_its_error_on_stderr_is_an_empty_turn_with_that_text() {
    let mut played = play(&recording("empty-turn"), options(), &[], |_, _| {
        Step::default()
    });
    assert!(played.exit.stderr_tail.contains("not found"));
    let finished = played.session.finish(&played.exit);
    assert_eq!(finished.outcome, failed(PhaseFailure::EmptyTurn));
    let evidence = finished.evidence.expect("the stderr text");
    assert!(
        evidence.contains("model 'stand-in-model:1b' not found"),
        "{evidence}"
    );
}

#[test]
fn a_child_that_exits_after_the_permission_request_with_no_answer_given_is_lost() {
    // Handshake (2 lines) and the request.
    let mut played = play_until_exit(&recording("lost-request-pending"), options(), 3, |_, _| {
        Step::default()
    });
    assert_eq!(played.session.pending_permissions().len(), 1);
    assert_eq!(
        played.session.finish(&played.exit).outcome,
        PhaseOutcome::Lost
    );
}

#[test]
fn a_child_that_exits_1_or_is_killed_before_its_response_is_lost() {
    for name in ["lost-exit-1", "lost-signaled"] {
        let mut played = play_until_exit(&recording(name), options(), 3, |_, _| Step::default());
        assert_eq!(
            played.session.finish(&played.exit).outcome,
            PhaseOutcome::Lost,
            "{name}"
        );
    }
}

#[test]
fn the_exit_a_lost_child_leaves_is_what_the_recording_did() {
    let played = play_until_exit(&recording("lost-exit-1"), options(), 3, |_, _| {
        Step::default()
    });
    let record = played.exit.record.expect("a record");
    assert_eq!(record.end, ExitEnd::Exited { code: 1 });
    let played = play_until_exit(&recording("lost-signaled"), options(), 3, |_, _| {
        Step::default()
    });
    let record = played.exit.record.expect("a record");
    assert_eq!(record.end, ExitEnd::Signaled { signal: 9 });
}

#[test]
fn a_child_that_exits_in_the_handshake_is_lost_under_the_shim() {
    let mut played = play_until_exit(&recording("lost-in-handshake"), options(), 1, |_, _| {
        Step::default()
    });
    assert_eq!(
        played.session.finish(&played.exit).outcome,
        PhaseOutcome::Lost
    );
}

#[test]
fn the_journal_of_a_lost_child_replays_to_the_same_outcome() {
    let mut played = play_until_exit(&recording("lost-request-pending"), options(), 3, |_, _| {
        Step::default()
    });
    let mut rebuilt = AcpSession::new(options());
    collide_core::replay::replay_journal_bytes(&mut rebuilt, &played.journal).unwrap();
    assert_eq!(
        rebuilt.finish(&played.exit),
        played.session.finish(&played.exit)
    );
    assert_eq!(rebuilt.finish(&played.exit).outcome, PhaseOutcome::Lost);
}

#[test]
fn the_journal_of_an_empty_turn_replays_to_the_same_outcome_and_evidence() {
    let mut played = play(&recording("empty-turn"), options(), &[], |_, _| {
        Step::default()
    });
    let mut rebuilt = AcpSession::new(options());
    collide_core::replay::replay_journal_bytes(&mut rebuilt, &played.journal).unwrap();
    let live = played.session.finish(&played.exit);
    assert_eq!(rebuilt.finish(&played.exit), live);
    assert!(live.evidence.is_some());
}

// ---- the daemon's feed: what the record keeps -----------------------------------------------------------------------

mod record {
    use super::*;
    use crate::common::TempRoot;
    use collide_core::agent::UsageQuality;
    use collide_host::agent_feed::AgentFeed;
    use collide_host::shim::files;
    use collide_host::store::PhaseRecord;
    use std::fs;

    /// A phase folder holding the journal the shim wrote and a stderr log with `stderr`, settled as the daemon does it:
    /// the session is rebuilt from the journal, then told the child has exited.
    fn settled(
        played: &super::super::fake::Played,
        stderr: &str,
        options: AcpOptions,
    ) -> (PhaseOutcome, PhaseRecord) {
        let root = TempRoot::new();
        fs::write(root.join(files::JOURNAL), &played.journal).unwrap();
        fs::write(root.join(files::STDERR_LOG), stderr).unwrap();
        let mut feed = AgentFeed::rebuild(Box::new(AcpSession::new(options)), root.path())
            .expect("the journal replays");
        let (outcome, record, problem) = feed.settle(played.exit.record.clone());
        assert_eq!(problem, None);
        (outcome, record)
    }

    #[test]
    fn an_empty_turn_settles_as_failed_with_the_stderr_text_in_the_record() {
        let played = play(&recording("empty-turn"), options(), &[], |_, _| {
            Step::default()
        });
        let (outcome, record) = settled(&played, &played.exit.stderr_tail, options());
        assert_eq!(outcome, failed(PhaseFailure::EmptyTurn));
        assert_eq!(record.failure.as_deref(), Some("empty_turn"));
        let evidence = record.failure_evidence.expect("the evidence");
        assert!(
            evidence.contains("model 'stand-in-model:1b' not found"),
            "{evidence}"
        );
    }

    #[test]
    fn the_evidence_in_the_record_is_the_end_of_a_long_stderr_within_the_bound() {
        let played = play(&recording("empty-turn"), options(), &[], |_, _| {
            Step::default()
        });
        let stderr = format!("{}LAST LINE\n", "noise\n".repeat(5000));
        let (_, record) = settled(&played, &stderr, options());
        let evidence = record.failure_evidence.expect("the evidence");
        assert!(evidence.len() <= EVIDENCE_BYTES, "{}", evidence.len());
        assert!(evidence.ends_with("LAST LINE"), "{evidence}");
    }

    #[test]
    fn a_lost_child_settles_as_lost_with_no_failure_and_no_evidence() {
        let played = play_until_exit(&recording("lost-exit-1"), options(), 3, |_, _| {
            Step::default()
        });
        let (outcome, record) = settled(&played, "fatal: stand-in failure", options());
        assert_eq!(outcome, PhaseOutcome::Lost);
        assert_eq!(record.failure, None);
        assert_eq!(record.failure_evidence, None);
    }

    #[test]
    fn an_ended_turn_keeps_no_evidence_whatever_the_child_wrote_on_stderr() {
        let played = play(&recording("prompt-turn"), options(), &[], |_, _| {
            Step::default()
        });
        let (outcome, record) = settled(&played, "a warning", options());
        assert_eq!(outcome, PhaseOutcome::Ended);
        assert_eq!(record.failure, None);
        assert_eq!(record.failure_evidence, None);
    }

    #[test]
    fn the_record_keeps_the_usage_the_session_reported_and_never_a_zero_for_a_missing_one() {
        let claim = || {
            AcpOptions::new("/work/repo", "read the notes").with_usage(UsageQuality::Partial, false)
        };
        let with = play(&recording("prompt-turn"), claim(), &[], |_, _| {
            Step::default()
        });
        let (_, record) = settled(&with, "", claim());
        let usage = record.usage.expect("usage");
        assert_eq!(usage.quality, UsageQuality::Partial);
        assert_eq!(usage.cache_write, None);
        let without = play(&recording("prompt-turn-no-usage"), claim(), &[], |_, _| {
            Step::default()
        });
        let (_, record) = settled(&without, "", claim());
        let usage = record.usage.expect("an unreported usage, not none");
        assert_eq!(usage.quality, UsageQuality::Unreported);
        assert_eq!(usage.input, None);
        assert_eq!(usage.output, None);
    }
}
