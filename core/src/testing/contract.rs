//! The `AgentSession` contract (0.3, Two core traits), written once. Each function below is one scenario that every
//! session must pass, over any protocol: it drives the session through a [`ChildDialect`], which says how that
//! protocol's child writes the few lines the scenarios need. [`session_contract_tests!`] turns the scenarios into
//! `#[test]` functions for one dialect, so a session double and the real session of Stage 3d run the same suite.
//!
//! What a session promises, as the scenarios check it:
//!
//! - `feed` reads one line and stamps its events with the `wall_ns` it was given; a line never makes a write that
//!   answers a permission request, so a request stays pending across any number of feeds until `answer`;
//! - `send_turn`, `answer` and `cancel` write, and their events carry no stored time;
//! - an answer to an id the session does not hold writes nothing, and a request is answered once;
//! - a turn that ends after a `cancel` is reported `Cancelled`, whatever the child said;
//! - `finish` is `Ended` after a turn that ended, `Stopped` after a cancelled one, and `Lost` when the shim left no
//!   exit record;
//! - `replay` rebuilds the state, so the rebuilt session answers, and numbers its next write, as the live one does;
//!   it writes nothing, stamps events from the stored time and refuses records out of `seq` order.
//!
//! The same input gives the same output.

use crate::adapter::{AgentSession, ExitInfo, PhaseOutcome, ReplayError, StampedEvent, Step};
use crate::agent::{AgentEvent, PermissionReply, TurnEndReason};
use crate::journal::{Direction, JournalRecord};
use crate::shim::{ExitEnd, ExitRecord, SHIM_PROTOCOL};

/// How one protocol's child writes the lines the scenarios feed. A request is named by a number, which the session
/// reads as the id `n.to_string()`. Turns are numbered from 0, the turn the session starts with.
pub trait ChildDialect {
    type Session: AgentSession + 'static;

    /// A new session, not started.
    fn session(&self) -> Self::Session;

    /// The lines that make the session ready for a turn, in order: whatever binds the session id.
    fn handshake(&self) -> Vec<Vec<u8>>;

    /// One chunk of the agent's words.
    fn message(&self, text: &str) -> Vec<u8>;

    /// A request to run a tool, which waits for an answer.
    fn permission_request(&self, id: u32) -> Vec<u8>;

    /// The end of turn number `turn`, as the agent reports a normal end (`end_turn`).
    fn turn_end(&self, turn: u32) -> Vec<u8>;
}

fn exit_record() -> ExitRecord {
    ExitRecord {
        protocol: SHIM_PROTOCOL,
        end: ExitEnd::Exited { code: 0 },
        cpu_user_us: 10,
        cpu_system_us: 5,
        max_rss_bytes: 4096,
    }
}

fn exited() -> ExitInfo {
    ExitInfo {
        record: Some(exit_record()),
        stderr_tail: String::new(),
    }
}

fn no_exit_record() -> ExitInfo {
    ExitInfo {
        record: None,
        stderr_tail: "shim died".into(),
    }
}

/// A session that is started and has read the handshake.
fn ready<D: ChildDialect>(dialect: &D) -> D::Session {
    let mut session = dialect.session();
    session.start();
    for (index, line) in dialect.handshake().iter().enumerate() {
        session.feed(line, 100 + index as u64);
    }
    session
}

fn events_of(step: &Step) -> Vec<&AgentEvent> {
    step.events.iter().map(|stamped| &stamped.event).collect()
}

/// The journal of a live conversation, built the way the shim builds it: what the session writes is an `in` record,
/// what the child writes an `out` record, and `seq` counts from 0.
#[derive(Default)]
struct Recorder {
    journal: Vec<JournalRecord>,
}

impl Recorder {
    fn log(&mut self, direction: Direction, bytes: Vec<u8>) -> u64 {
        let seq = self.journal.len() as u64;
        let wall_ns = 100 + seq * 10;
        self.journal.push(JournalRecord {
            seq,
            wall_ns,
            direction,
            bytes,
        });
        wall_ns
    }

    fn writes(&mut self, step: Step) {
        for write in step.writes {
            self.log(Direction::In, write);
        }
    }

    fn feed<S: AgentSession>(&mut self, session: &mut S, line: Vec<u8>) {
        let wall_ns = self.log(Direction::Out, line.clone());
        let step = session.feed(&line, wall_ns);
        self.writes(step);
    }
}

/// A live conversation with request 1 answered and request 2 still pending, and its journal.
fn live_conversation<D: ChildDialect>(dialect: &D) -> (D::Session, Vec<JournalRecord>) {
    let mut session = dialect.session();
    let mut recorder = Recorder::default();
    recorder.writes(session.start());
    for line in dialect.handshake() {
        recorder.feed(&mut session, line);
    }
    recorder.feed(&mut session, dialect.message("working"));
    recorder.writes(session.send_turn("fix the bug"));
    recorder.feed(&mut session, dialect.permission_request(1));
    recorder.feed(&mut session, dialect.permission_request(2));
    recorder.writes(session.answer("1", PermissionReply::Allow {}));
    (session, recorder.journal)
}

// --- feed ---

pub fn the_handshake_binds_the_session_with_the_time_its_line_arrived<D: ChildDialect>(
    dialect: &D,
) {
    let mut session = dialect.session();
    session.start();
    let lines = dialect.handshake();
    let times: Vec<u64> = (0..lines.len() as u64)
        .map(|i| 1_700_000_000_000_000_000 + i)
        .collect();
    let mut events: Vec<StampedEvent> = Vec::new();
    for (line, time) in lines.iter().zip(&times) {
        events.extend(session.feed(line, *time).events);
    }
    let bound: Vec<&StampedEvent> = events
        .iter()
        .filter(|stamped| matches!(stamped.event, AgentEvent::SessionBound { .. }))
        .collect();
    assert_eq!(bound.len(), 1, "one SessionBound, got {events:?}");
    let time = bound[0].wall_ns.expect("a fed event has the stored time");
    assert!(
        times.contains(&time),
        "{time} is not the time of a handshake line"
    );
}

pub fn feeding_a_message_writes_nothing_and_stamps_the_event<D: ChildDialect>(dialect: &D) {
    let mut session = ready(dialect);
    let step = session.feed(&dialect.message("hello"), 500);
    assert!(step.writes.is_empty());
    assert_eq!(
        step.events,
        vec![StampedEvent {
            wall_ns: Some(500),
            event: AgentEvent::Message {
                text: "hello".into()
            }
        }]
    );
}

pub fn a_permission_request_stays_pending_across_feeds_until_it_is_answered<D: ChildDialect>(
    dialect: &D,
) {
    let mut session = ready(dialect);
    let asked = session.feed(&dialect.permission_request(1), 200);
    assert_eq!(asked.events.len(), 1);
    let AgentEvent::PermissionRequest { request } = &asked.events[0].event else {
        panic!("expected a permission request, got {:?}", asked.events);
    };
    assert_eq!(request.id, "1");
    assert_eq!(asked.events[0].wall_ns, Some(200));
    assert!(asked.writes.is_empty());

    for (index, text) in ["still", "working", "on it"].iter().enumerate() {
        let step = session.feed(&dialect.message(text), 210 + index as u64);
        assert!(step.writes.is_empty(), "a feed must not answer the request");
    }
    session.feed(&dialect.permission_request(2), 230);

    // Both are still there, however many lines came in between, and each is answered once.
    assert_eq!(
        session.answer("2", PermissionReply::Allow {}).writes.len(),
        1
    );
    assert!(
        session
            .answer("2", PermissionReply::Allow {})
            .writes
            .is_empty()
    );
    assert_eq!(
        session.answer("1", PermissionReply::Allow {}).writes.len(),
        1
    );
}

// --- send_turn, answer, cancel ---

pub fn send_turn_writes_the_turn_and_its_events_have_no_stored_time<D: ChildDialect>(dialect: &D) {
    let mut session = ready(dialect);
    let step = session.send_turn("hello");
    assert!(!step.writes.is_empty());
    assert!(step.events.iter().all(|stamped| stamped.wall_ns.is_none()));
}

pub fn each_kind_of_reply_writes_its_own_answer<D: ChildDialect>(dialect: &D) {
    let answer = |reply: PermissionReply| {
        let mut session = ready(dialect);
        session.feed(&dialect.permission_request(1), 200);
        session.answer("1", reply).writes
    };
    let allow = answer(PermissionReply::Allow {});
    let deny = answer(PermissionReply::Deny {
        message: Some("not in this folder".into()),
    });
    let cancel = answer(PermissionReply::Cancel {});
    assert!(!allow.is_empty() && !deny.is_empty() && !cancel.is_empty());
    assert_ne!(allow, deny);
    assert_ne!(allow, cancel);
    assert_ne!(deny, cancel);
}

pub fn answering_a_request_nobody_asked_writes_nothing<D: ChildDialect>(dialect: &D) {
    let mut session = ready(dialect);
    let step = session.answer("9", PermissionReply::Allow {});
    assert!(step.writes.is_empty() && step.events.is_empty());
}

pub fn cancel_writes_and_a_turn_that_ends_after_it_is_reported_cancelled<D: ChildDialect>(
    dialect: &D,
) {
    let mut session = ready(dialect);
    assert!(!session.cancel().writes.is_empty());
    let step = session.feed(&dialect.turn_end(0), 300);
    let events = events_of(&step);
    let Some(AgentEvent::TurnEnd {
        reason,
        cancel_requested,
        ..
    }) = events.last().copied()
    else {
        panic!("expected a turn end last, got {events:?}");
    };
    assert_eq!(reason, &TurnEndReason::Cancelled {});
    assert!(cancel_requested);
}

// --- finish ---

pub fn finish_after_a_turn_that_ended_is_ended<D: ChildDialect>(dialect: &D) {
    let mut session = ready(dialect);
    session.feed(&dialect.message("done"), 200);
    session.feed(&dialect.turn_end(0), 300);
    assert_eq!(session.finish(&exited()).outcome, PhaseOutcome::Ended);
}

pub fn finish_with_no_exit_record_is_lost_whatever_the_stream_said<D: ChildDialect>(dialect: &D) {
    let mut session = ready(dialect);
    session.feed(&dialect.message("done"), 200);
    session.feed(&dialect.turn_end(0), 300);
    assert_eq!(
        session.finish(&no_exit_record()).outcome,
        PhaseOutcome::Lost
    );
}

pub fn finish_after_a_cancelled_turn_is_stopped<D: ChildDialect>(dialect: &D) {
    let mut session = ready(dialect);
    session.feed(&dialect.message("working"), 200);
    session.cancel();
    session.feed(&dialect.turn_end(0), 300);
    assert_eq!(session.finish(&exited()).outcome, PhaseOutcome::Stopped);
}

pub fn finish_writes_nothing_because_the_child_has_gone<D: ChildDialect>(dialect: &D) {
    let mut session = ready(dialect);
    session.feed(&dialect.message("done"), 200);
    session.feed(&dialect.turn_end(0), 300);
    assert!(session.finish(&exited()).step.writes.is_empty());
}

// --- determinism ---

pub fn the_same_input_gives_the_same_output<D: ChildDialect>(dialect: &D) {
    let run = || {
        let mut session = dialect.session();
        let mut steps = vec![session.start()];
        for (index, line) in dialect.handshake().iter().enumerate() {
            steps.push(session.feed(line, 110 + index as u64));
        }
        steps.push(session.send_turn("go"));
        steps.push(session.feed(&dialect.permission_request(1), 130));
        steps.push(session.feed(&dialect.message("words"), 140));
        steps.push(session.answer("1", PermissionReply::Allow {}));
        steps.push(session.cancel());
        steps.push(session.feed(&dialect.turn_end(0), 150));
        let finished = session.finish(&exited());
        (steps, finished)
    };
    assert_eq!(run(), run());
}

// --- replay ---

pub fn replay_rebuilds_a_pending_request_and_the_numbering_of_the_next_write<D: ChildDialect>(
    dialect: &D,
) {
    let (mut live, journal) = live_conversation(dialect);
    let mut rebuilt = dialect.session();
    rebuilt.replay(&journal).expect("a live journal replays");

    // Request 1 was answered before the journal ended, so it is gone; request 2 is still waiting.
    assert!(
        rebuilt
            .answer("1", PermissionReply::Allow {})
            .writes
            .is_empty()
    );
    let rebuilt_answer = rebuilt.answer("2", PermissionReply::Allow {});
    assert!(!rebuilt_answer.writes.is_empty());
    assert_eq!(rebuilt_answer, live.answer("2", PermissionReply::Allow {}));

    // The next turn is written exactly as the live session writes it, so no request id is used twice.
    assert_eq!(rebuilt.send_turn("next"), live.send_turn("next"));
}

pub fn replay_stamps_events_with_the_stored_time_of_their_line<D: ChildDialect>(dialect: &D) {
    let (_, journal) = live_conversation(dialect);
    let stored: Vec<u64> = journal
        .iter()
        .filter(|record| record.direction == Direction::Out)
        .map(|record| record.wall_ns)
        .collect();
    let step = dialect
        .session()
        .replay(&journal)
        .expect("a live journal replays");
    assert!(!step.events.is_empty());
    for event in &step.events {
        let time = event.wall_ns.expect("a replayed event has the stored time");
        assert!(
            stored.contains(&time),
            "{time} is not the time of an `out` record"
        );
    }
}

pub fn replay_writes_nothing_to_the_child<D: ChildDialect>(dialect: &D) {
    let (_, journal) = live_conversation(dialect);
    let step = dialect
        .session()
        .replay(&journal)
        .expect("a live journal replays");
    assert!(step.writes.is_empty());
}

pub fn replay_of_an_empty_journal_changes_nothing<D: ChildDialect>(dialect: &D) {
    let mut replayed = dialect.session();
    let step = replayed.replay(&[]).expect("an empty journal replays");
    assert!(step.events.is_empty() && step.writes.is_empty());
    let mut fresh = dialect.session();
    assert_eq!(replayed.start(), fresh.start());
}

pub fn replay_over_records_out_of_seq_order_is_an_error<D: ChildDialect>(dialect: &D) {
    let (_, mut journal) = live_conversation(dialect);
    journal.swap(1, 2);
    assert_eq!(
        dialect.session().replay(&journal),
        Err(ReplayError::OutOfOrder {
            expected: 1,
            found: 2
        })
    );
}

pub fn replay_of_a_written_record_the_session_cannot_read_is_rejected<D: ChildDialect>(
    dialect: &D,
) {
    let journal = vec![JournalRecord {
        seq: 0,
        wall_ns: 100,
        direction: Direction::In,
        bytes: b"this is not a message\n".to_vec(),
    }];
    let result = dialect.session().replay(&journal);
    assert!(
        matches!(result, Err(ReplayError::Rejected { seq: 0, .. })),
        "{result:?}"
    );
}

pub fn a_session_is_usable_through_a_trait_object<D: ChildDialect>(dialect: &D) {
    let mut session: Box<dyn AgentSession> = Box::new(dialect.session());
    session.start();
    session.feed(&dialect.message("hi"), 1);
    assert!(
        session
            .answer("9", PermissionReply::Allow {})
            .writes
            .is_empty()
    );
}

/// The contract as `#[test]` functions, for one dialect, inside a module named `$module`:
/// `session_contract_tests!(my_session, MyDialect)`. The dialect is an expression that is evaluated for each test.
#[macro_export]
macro_rules! session_contract_tests {
    ($module:ident, $dialect:expr) => {
        mod $module {
            use super::*;

            #[test]
            fn the_handshake_binds_the_session_with_the_time_its_line_arrived() {
                $crate::testing::contract::the_handshake_binds_the_session_with_the_time_its_line_arrived(&$dialect);
            }

            #[test]
            fn feeding_a_message_writes_nothing_and_stamps_the_event() {
                $crate::testing::contract::feeding_a_message_writes_nothing_and_stamps_the_event(&$dialect);
            }

            #[test]
            fn a_permission_request_stays_pending_across_feeds_until_it_is_answered() {
                $crate::testing::contract::a_permission_request_stays_pending_across_feeds_until_it_is_answered(&$dialect);
            }

            #[test]
            fn send_turn_writes_the_turn_and_its_events_have_no_stored_time() {
                $crate::testing::contract::send_turn_writes_the_turn_and_its_events_have_no_stored_time(&$dialect);
            }

            #[test]
            fn each_kind_of_reply_writes_its_own_answer() {
                $crate::testing::contract::each_kind_of_reply_writes_its_own_answer(&$dialect);
            }

            #[test]
            fn answering_a_request_nobody_asked_writes_nothing() {
                $crate::testing::contract::answering_a_request_nobody_asked_writes_nothing(&$dialect);
            }

            #[test]
            fn cancel_writes_and_a_turn_that_ends_after_it_is_reported_cancelled() {
                $crate::testing::contract::cancel_writes_and_a_turn_that_ends_after_it_is_reported_cancelled(&$dialect);
            }

            #[test]
            fn finish_after_a_turn_that_ended_is_ended() {
                $crate::testing::contract::finish_after_a_turn_that_ended_is_ended(&$dialect);
            }

            #[test]
            fn finish_with_no_exit_record_is_lost_whatever_the_stream_said() {
                $crate::testing::contract::finish_with_no_exit_record_is_lost_whatever_the_stream_said(&$dialect);
            }

            #[test]
            fn finish_after_a_cancelled_turn_is_stopped() {
                $crate::testing::contract::finish_after_a_cancelled_turn_is_stopped(&$dialect);
            }

            #[test]
            fn finish_writes_nothing_because_the_child_has_gone() {
                $crate::testing::contract::finish_writes_nothing_because_the_child_has_gone(&$dialect);
            }

            #[test]
            fn the_same_input_gives_the_same_output() {
                $crate::testing::contract::the_same_input_gives_the_same_output(&$dialect);
            }

            #[test]
            fn replay_rebuilds_a_pending_request_and_the_numbering_of_the_next_write() {
                $crate::testing::contract::replay_rebuilds_a_pending_request_and_the_numbering_of_the_next_write(&$dialect);
            }

            #[test]
            fn replay_stamps_events_with_the_stored_time_of_their_line() {
                $crate::testing::contract::replay_stamps_events_with_the_stored_time_of_their_line(&$dialect);
            }

            #[test]
            fn replay_writes_nothing_to_the_child() {
                $crate::testing::contract::replay_writes_nothing_to_the_child(&$dialect);
            }

            #[test]
            fn replay_of_an_empty_journal_changes_nothing() {
                $crate::testing::contract::replay_of_an_empty_journal_changes_nothing(&$dialect);
            }

            #[test]
            fn replay_over_records_out_of_seq_order_is_an_error() {
                $crate::testing::contract::replay_over_records_out_of_seq_order_is_an_error(&$dialect);
            }

            #[test]
            fn replay_of_a_written_record_the_session_cannot_read_is_rejected() {
                $crate::testing::contract::replay_of_a_written_record_the_session_cannot_read_is_rejected(&$dialect);
            }

            #[test]
            fn a_session_is_usable_through_a_trait_object() {
                $crate::testing::contract::a_session_is_usable_through_a_trait_object(&$dialect);
            }
        }
    };
}
