//! Session replay over the journal (ts-0039-ps; 0.2, the shim and its journal; 0.3, `AgentSession`). A live run of the
//! scripted ACP child is recorded the way the shim records it: every write of the session is an `in` record, every
//! line the child wrote is an `out` record with the time it arrived, and each record goes through `encode_record`.
//! The journal's bytes are then read and replayed into a fresh session, which must be the live one: the same state, a
//! pending permission request that is answered with the same request id, and events stamped with the stored time.
//!
//! The recorder here stands for the shim, so it holds the only clock of these tests: a counter that moves by ten
//! nanoseconds a record. The sessions read none.

use collide_core::adapter::{AgentSession, ReplayError, StampedEvent, Step};
use collide_core::agent::PermissionReply;
use collide_core::journal::{
    Direction, JournalEnd, JournalError, JournalRecord, RECORD_HEADER_BYTES, encode_record,
};
use collide_core::replay::{replay_journal_bytes, replay_records};
use collide_core::testing::{AcpSession, acp};
use serde_json::{Value, json};

const SESSION: &str = "ses_1";

/// A live ACP session and the journal the shim would keep for it.
struct Recorded {
    session: AcpSession,
    records: Vec<JournalRecord>,
    now: u64,
    /// Every event the live session returned from `feed`, in order: what a replay must return.
    fed_events: Vec<StampedEvent>,
}

impl Recorded {
    fn new() -> Self {
        Recorded {
            session: AcpSession::new("/work", "do the task"),
            records: Vec::new(),
            now: 1_000,
            fed_events: Vec::new(),
        }
    }

    fn push(&mut self, direction: Direction, bytes: Vec<u8>) -> u64 {
        self.now += 10;
        self.records.push(JournalRecord {
            seq: self.records.len() as u64,
            wall_ns: self.now,
            direction,
            bytes,
        });
        self.now
    }

    /// The shim journals what the session wrote, then the child reads it.
    fn journal_writes(&mut self, step: Step) -> Step {
        for write in &step.writes {
            self.push(Direction::In, write.clone());
        }
        step
    }

    fn start(&mut self) {
        let step = self.session.start();
        self.journal_writes(step);
    }

    /// The child wrote `line`: the shim journals it with the time it saw it, then the session reads it.
    fn child_writes(&mut self, line: Vec<u8>) {
        let wall_ns = self.push(Direction::Out, line.clone());
        let step = self.session.feed(&line, wall_ns);
        self.fed_events.extend(step.events.clone());
        self.journal_writes(step);
    }

    fn answer(&mut self, request_id: &str, reply: PermissionReply) {
        let step = self.session.answer(request_id, reply);
        self.journal_writes(step);
    }

    fn bytes(&self) -> Vec<u8> {
        self.records
            .iter()
            .flat_map(|record| encode_record(record).expect("a small record encodes"))
            .collect()
    }

    /// The offset where record `index` starts in `bytes()`.
    fn offset_of(&self, index: usize) -> usize {
        self.records[..index]
            .iter()
            .map(|record| RECORD_HEADER_BYTES + record.bytes.len())
            .sum()
    }
}

fn bash_call() -> Value {
    json!({ "toolCallId": "call-1", "title": "bash", "kind": "execute", "status": "pending", "rawInput": {} })
}

fn ask(recorded: &mut Recorded, id: u64) {
    recorded.child_writes(acp::permission_request(
        id,
        SESSION,
        bash_call(),
        acp::standard_options(),
    ));
}

/// The handshake, some words, request 7 asked and answered, request 8 asked and left waiting, more words. The last
/// record is an `out` record.
fn live_run_ending_with_words() -> Recorded {
    let mut live = live_run_ending_with_an_answer();
    live.child_writes(acp::message_chunk(SESSION, "still waiting for you"));
    live
}

/// The same run, whose last record is an `in` record: request 7's answer is written after request 8 is asked.
fn live_run_ending_with_an_answer() -> Recorded {
    let mut live = Recorded::new();
    live.start();
    live.child_writes(acp::initialize_response(0));
    live.child_writes(acp::new_session_response(1, SESSION));
    live.child_writes(acp::message_chunk(SESSION, "I will run a command"));
    ask(&mut live, 7);
    ask(&mut live, 8);
    live.answer("7", PermissionReply::Allow {});
    live
}

fn fresh() -> AcpSession {
    AcpSession::new("/work", "do the task")
}

/// A session's whole state, as text. The doubles derive `Debug` over every field, so two sessions with the same
/// text are in the same state.
fn state(session: &AcpSession) -> String {
    format!("{session:?}")
}

fn parse(bytes: &[u8]) -> Value {
    serde_json::from_slice(bytes).expect("the session writes JSON")
}

// --- The live run and its replay ---

#[test]
fn a_replayed_journal_rebuilds_the_state_of_the_live_session() {
    let live = live_run_ending_with_words();
    let mut replayed = fresh();
    let outcome =
        replay_journal_bytes(&mut replayed, &live.bytes()).expect("a live journal replays");

    assert_eq!(outcome.end, JournalEnd::Clean);
    assert_eq!(outcome.records, live.records.len());
    assert_eq!(state(&replayed), state(&live.session));
}

#[test]
fn the_pending_permission_request_is_answered_after_the_replay_with_the_same_request_id() {
    let mut live = live_run_ending_with_words();
    let mut replayed = fresh();
    replay_journal_bytes(&mut replayed, &live.bytes()).expect("a live journal replays");

    // Request 7 was answered before the journal ended, so it is gone; request 8 still waits.
    assert!(
        replayed
            .answer("7", PermissionReply::Allow {})
            .writes
            .is_empty()
    );
    let live_answer = live.session.answer("8", PermissionReply::Allow {});
    let replayed_answer = replayed.answer("8", PermissionReply::Allow {});
    assert_eq!(replayed_answer, live_answer);
    assert_eq!(parse(&replayed_answer.writes[0])["id"], json!(8));
}

#[test]
fn the_next_turn_after_a_replay_takes_the_request_id_the_live_session_would_take() {
    let mut live = live_run_ending_with_words();
    let mut replayed = fresh();
    replay_journal_bytes(&mut replayed, &live.bytes()).expect("a live journal replays");

    let next = replayed.send_turn("and then");
    assert_eq!(next, live.session.send_turn("and then"));
    assert_eq!(parse(&next.writes[0])["id"], json!(3));
}

#[test]
fn replayed_events_are_the_live_events_with_the_stored_time_of_their_lines() {
    let live = live_run_ending_with_words();
    let mut replayed = fresh();
    let outcome =
        replay_journal_bytes(&mut replayed, &live.bytes()).expect("a live journal replays");

    assert!(!outcome.step.events.is_empty());
    assert_eq!(outcome.step.events, live.fed_events);
    let out_times: Vec<u64> = live
        .records
        .iter()
        .filter(|record| record.direction == Direction::Out)
        .map(|record| record.wall_ns)
        .collect();
    for event in &outcome.step.events {
        let time = event.wall_ns.expect("a replayed event has a stored time");
        assert!(out_times.contains(&time), "{time} is not an `out` time");
    }
}

#[test]
fn a_replay_writes_nothing_to_the_child() {
    let live = live_run_ending_with_words();
    let outcome =
        replay_journal_bytes(&mut fresh(), &live.bytes()).expect("a live journal replays");
    assert!(outcome.step.writes.is_empty());
}

#[test]
fn replaying_the_records_of_a_live_run_gives_the_same_result_as_replaying_their_bytes() {
    let live = live_run_ending_with_words();
    let mut from_records = fresh();
    let step = replay_records(&mut from_records, &live.records).expect("a live journal replays");
    let mut from_bytes = fresh();
    let outcome = replay_journal_bytes(&mut from_bytes, &live.bytes()).expect("it replays");

    assert_eq!(step, outcome.step);
    assert_eq!(state(&from_records), state(&from_bytes));
}

#[test]
fn a_session_behind_a_trait_object_replays_from_bytes() {
    let live = live_run_ending_with_words();
    let mut boxed: Box<dyn AgentSession> = Box::new(fresh());
    let outcome =
        replay_journal_bytes(boxed.as_mut(), &live.bytes()).expect("a live journal replays");
    assert_eq!(outcome.records, live.records.len());
    assert_eq!(outcome.step.events, live.fed_events);
}

// --- The empty sink ---

#[test]
fn an_empty_journal_replays_to_a_session_that_is_still_new() {
    let mut replayed = fresh();
    let outcome = replay_journal_bytes(&mut replayed, &[]).expect("an empty journal replays");

    assert_eq!(outcome.step, Step::default());
    assert_eq!(outcome.records, 0);
    assert_eq!(outcome.valid_len, 0);
    assert_eq!(outcome.end, JournalEnd::Clean);
    assert_eq!(state(&replayed), state(&fresh()));
}

// --- A journal cut short ---

/// Cuts the journal at every byte inside its last record, from the record's first byte to one byte short of its
/// end, and checks each cut replays to the state of the whole records before it.
fn assert_every_cut_of_the_last_record_replays_to_the_records_before(live: &Recorded) {
    let bytes = live.bytes();
    let last = live.records.len() - 1;
    let last_start = live.offset_of(last);
    assert!(
        bytes.len() - last_start > RECORD_HEADER_BYTES,
        "a record with a body"
    );

    let mut expected = fresh();
    let before = replay_records(&mut expected, &live.records[..last]).expect("replays");

    for cut in last_start..bytes.len() {
        let mut replayed = fresh();
        let outcome = replay_journal_bytes(&mut replayed, &bytes[..cut])
            .unwrap_or_else(|error| panic!("cut at {cut}: {error}"));
        let end = if cut == last_start {
            JournalEnd::Clean
        } else {
            JournalEnd::Cut { offset: last_start }
        };
        assert_eq!(outcome.end, end, "cut at {cut}");
        assert_eq!(outcome.records, last, "cut at {cut}");
        assert_eq!(outcome.valid_len, last_start, "cut at {cut}");
        assert_eq!(outcome.step, before, "cut at {cut}");
        assert_eq!(state(&replayed), state(&expected), "cut at {cut}");
    }
}

#[test]
fn a_journal_cut_inside_its_last_out_record_replays_up_to_the_last_whole_record_at_every_byte() {
    let live = live_run_ending_with_words();
    assert_eq!(
        live.records.last().expect("records").direction,
        Direction::Out
    );
    assert_every_cut_of_the_last_record_replays_to_the_records_before(&live);
}

#[test]
fn a_journal_cut_inside_its_last_in_record_replays_up_to_the_last_whole_record_at_every_byte() {
    let live = live_run_ending_with_an_answer();
    assert_eq!(
        live.records.last().expect("records").direction,
        Direction::In
    );
    assert_every_cut_of_the_last_record_replays_to_the_records_before(&live);
}

#[test]
fn a_cut_journal_keeps_the_request_that_the_dropped_answer_would_have_cleared() {
    // The answer to request 7 is the last record and is cut short, so the replayed session still holds request 7.
    let live = live_run_ending_with_an_answer();
    let bytes = live.bytes();
    let mut replayed = fresh();
    replay_journal_bytes(&mut replayed, &bytes[..bytes.len() - 1]).expect("it replays");

    let answer = replayed.answer("7", PermissionReply::Allow {});
    assert_eq!(answer.writes.len(), 1);
}

// --- Damage ---

#[test]
fn a_damaged_journal_replays_up_to_the_damage_and_reports_where_and_why() {
    let live = live_run_ending_with_words();
    let mut bytes = live.bytes();
    let damaged = 4;
    let at = live.offset_of(damaged);
    bytes[at + 16] = 9; // the record's direction byte

    let mut replayed = fresh();
    let outcome = replay_journal_bytes(&mut replayed, &bytes).expect("the whole records replay");

    assert_eq!(outcome.records, damaged);
    assert_eq!(outcome.valid_len, at);
    assert_eq!(
        outcome.end,
        JournalEnd::Damaged {
            error: JournalError::BadDirection {
                offset: at,
                found: 9
            }
        }
    );
    let mut expected = fresh();
    replay_records(&mut expected, &live.records[..damaged]).expect("replays");
    assert_eq!(state(&replayed), state(&expected));
}

#[test]
fn a_journal_with_a_record_out_of_order_in_its_bytes_is_damage_and_is_never_reordered() {
    let live = live_run_ending_with_words();
    let mut swapped = live.records.clone();
    swapped.swap(5, 6);
    let bytes: Vec<u8> = swapped
        .iter()
        .flat_map(|record| encode_record(record).expect("encodes"))
        .collect();

    let mut replayed = fresh();
    let outcome = replay_journal_bytes(&mut replayed, &bytes).expect("the records before replay");

    assert_eq!(outcome.records, 5);
    assert_eq!(
        outcome.end,
        JournalEnd::Damaged {
            error: JournalError::OutOfOrder {
                offset: live.offset_of(5),
                expected: 5,
                found: 6
            }
        }
    );
}

// --- Records out of seq order ---

#[test]
fn records_out_of_seq_order_are_an_error_that_says_which_seq_was_expected() {
    let mut records = live_run_ending_with_words().records;
    records.swap(1, 2);
    assert_eq!(
        replay_records(&mut fresh(), &records),
        Err(ReplayError::OutOfOrder {
            expected: 1,
            found: 2
        })
    );
}

#[test]
fn a_gap_in_seq_is_an_error() {
    let mut records = live_run_ending_with_words().records;
    records.remove(3);
    assert_eq!(
        replay_records(&mut fresh(), &records),
        Err(ReplayError::OutOfOrder {
            expected: 3,
            found: 4
        })
    );
}

#[test]
fn a_repeated_seq_is_an_error() {
    let mut records = live_run_ending_with_words().records;
    records.insert(2, records[1].clone());
    assert_eq!(
        replay_records(&mut fresh(), &records),
        Err(ReplayError::OutOfOrder {
            expected: 2,
            found: 1
        })
    );
}

#[test]
fn a_journal_that_does_not_start_at_seq_0_is_an_error() {
    let mut records = live_run_ending_with_words().records;
    records.remove(0);
    assert_eq!(
        replay_records(&mut fresh(), &records),
        Err(ReplayError::OutOfOrder {
            expected: 0,
            found: 1
        })
    );
}

#[test]
fn the_out_of_order_error_names_both_seq_numbers() {
    let message = ReplayError::OutOfOrder {
        expected: 1,
        found: 2,
    }
    .to_string();
    assert!(
        message.contains("expected seq 1") && message.contains("found 2"),
        "{message}"
    );
}

// --- A record the session cannot read ---

#[test]
fn a_written_record_the_session_cannot_read_is_rejected_with_its_seq_and_offset() {
    let live = live_run_ending_with_words();
    let mut records = live.records.clone();
    let bad = 3;
    records[bad].direction = Direction::In;
    records[bad].bytes = b"this is not a message\n".to_vec();
    let bytes: Vec<u8> = records
        .iter()
        .flat_map(|record| encode_record(record).expect("encodes"))
        .collect();
    let offset: usize = records[..bad]
        .iter()
        .map(|record| RECORD_HEADER_BYTES + record.bytes.len())
        .sum();

    let error = replay_journal_bytes(&mut fresh(), &bytes).expect_err("the record is rejected");

    let ReplayError::Rejected { seq, message } = &error else {
        panic!("expected a rejection, got {error:?}");
    };
    assert_eq!(*seq, bad as u64);
    assert!(
        message.contains(&format!("offset {offset}")),
        "the message gives the offset: {message}"
    );
}

// --- One walk ---

#[test]
fn the_order_check_of_a_replay_is_written_once_in_core() {
    // The doubles and Stage 3d's session use `collide_core::replay`; none of them builds the error by itself. The
    // error's own definition (`adapter.rs`) and the contract suite that asserts it (`contract.rs`) name it.
    let mut found = Vec::new();
    let needle = ["ReplayError", "::OutOfOrder {"].concat();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut dirs = vec![root];
    while let Some(dir) = dirs.pop() {
        for entry in std::fs::read_dir(&dir).expect("core/src is readable") {
            let path = entry.expect("an entry").path();
            if path.is_dir() {
                dirs.push(path);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                let text = std::fs::read_to_string(&path).expect("a source file is text");
                let names_it = path.ends_with("adapter.rs")
                    && path.parent().is_some_and(|dir| dir.ends_with("src"));
                if text.contains(&needle) && !names_it && !path.ends_with("contract.rs") {
                    found.push(path.file_name().map(|name| name.to_owned()));
                }
            }
        }
    }
    assert_eq!(found, vec![Some("replay.rs".into())], "{found:?}");
}
