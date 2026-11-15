//! Tests for the `AgentSession` contract (ts-0019-vm): a fake session in this file implements the trait with the
//! standard library alone, and the tests drive `start`, `feed`, `send_turn`, `answer`, `cancel`, `finish` and
//! `replay` with fixed `wall_ns` values. Nothing here reads a clock or does I/O, which is what "sans-IO" means.
//!
//! The fake speaks a one-word-per-line protocol so that the tests stay readable:
//!
//! | child's stdout line | what the session reads |
//! |---|---|
//! | `session <id>` | the child named the session id |
//! | `message <text>` | one message chunk |
//! | `ask <id>` | a permission request that waits for an answer |
//! | `done` | the turn ended normally |
//!
//! | the session writes | when |
//! |---|---|
//! | `new` | `start` |
//! | `turn <text>` | `send_turn` |
//! | `reply <id> <allow\|deny\|cancel>` | `answer` |
//! | `cancel` | `cancel` |

use collide_core::adapter::{
    AgentSession, ExitInfo, Finished, PhaseOutcome, ReplayError, StampedEvent, Step,
};
use collide_core::agent::{AgentEvent, PermissionReply, ToolKind, TurnEndReason};
use collide_core::journal::{Direction, JournalRecord};
use collide_core::shim::{ExitEnd, ExitRecord, SHIM_PROTOCOL};

/// The state a session reaches, so a live session and a replayed one can be compared.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct Snapshot {
    session_id: Option<String>,
    pending: Vec<String>,
    turns_sent: u32,
    cancel_requested: bool,
    turn_ended: bool,
}

#[derive(Default)]
struct FakeSession {
    state: Snapshot,
}

fn line_of(text: &str) -> Vec<u8> {
    format!("{text}\n").into_bytes()
}

fn stamped(wall_ns: Option<u64>, event: AgentEvent) -> StampedEvent {
    StampedEvent { wall_ns, event }
}

impl FakeSession {
    fn snapshot(&self) -> Snapshot {
        self.state.clone()
    }

    /// The bytes of one line that the session wrote, as a replay learns them from an `in` record.
    fn note_written(&mut self, bytes: &[u8]) {
        let text = String::from_utf8_lossy(bytes);
        let text = text.trim_end_matches('\n');
        if text.starts_with("turn ") {
            self.state.turns_sent += 1;
        } else if text == "cancel" {
            self.state.cancel_requested = true;
        } else if let Some(rest) = text.strip_prefix("reply ")
            && let Some(id) = rest.split(' ').next()
        {
            self.state.pending.retain(|pending| pending != id);
        }
    }
}

impl AgentSession for FakeSession {
    fn start(&mut self) -> Step {
        Step {
            events: Vec::new(),
            writes: vec![line_of("new")],
        }
    }

    fn feed(&mut self, line: &[u8], wall_ns: u64) -> Step {
        let text = String::from_utf8_lossy(line).trim_end().to_string();
        let mut step = Step::default();
        if let Some(id) = text.strip_prefix("session ") {
            self.state.session_id = Some(id.to_string());
            step.events.push(stamped(
                Some(wall_ns),
                AgentEvent::SessionBound { id: id.to_string() },
            ));
        } else if let Some(message) = text.strip_prefix("message ") {
            step.events.push(stamped(
                Some(wall_ns),
                AgentEvent::Message {
                    text: message.to_string(),
                },
            ));
        } else if let Some(id) = text.strip_prefix("ask ") {
            self.state.pending.push(id.to_string());
            step.events.push(stamped(
                Some(wall_ns),
                AgentEvent::PermissionRequest {
                    request: collide_core::agent::PermissionRequest {
                        id: id.to_string(),
                        tool_call_id: format!("call-{id}"),
                        tool: None,
                        kind: ToolKind::Execute,
                        input: None,
                        reason: None,
                        options: Vec::new(),
                    },
                },
            ));
        } else if text == "done" {
            self.state.turn_ended = true;
            step.events.push(stamped(
                Some(wall_ns),
                AgentEvent::TurnEnd {
                    reason: if self.state.cancel_requested {
                        TurnEndReason::Cancelled {}
                    } else {
                        TurnEndReason::EndTurn {}
                    },
                    cancel_requested: self.state.cancel_requested,
                    empty: false,
                },
            ));
        }
        step
    }

    fn send_turn(&mut self, text: &str) -> Step {
        self.state.turns_sent += 1;
        Step {
            events: Vec::new(),
            writes: vec![line_of(&format!("turn {text}"))],
        }
    }

    fn answer(&mut self, request_id: &str, reply: PermissionReply) -> Step {
        if !self.state.pending.iter().any(|id| id == request_id) {
            return Step::default();
        }
        self.state.pending.retain(|id| id != request_id);
        let word = match reply {
            PermissionReply::Allow {} => "allow",
            PermissionReply::Deny { .. } => "deny",
            PermissionReply::Cancel {} => "cancel",
        };
        Step {
            events: Vec::new(),
            writes: vec![line_of(&format!("reply {request_id} {word}"))],
        }
    }

    fn cancel(&mut self) -> Step {
        self.state.cancel_requested = true;
        Step {
            events: Vec::new(),
            writes: vec![line_of("cancel")],
        }
    }

    fn finish(&mut self, exit: &ExitInfo) -> Finished {
        let outcome = match (&exit.record, self.state.turn_ended) {
            (None, _) => PhaseOutcome::Lost,
            (Some(_), true) if self.state.cancel_requested => PhaseOutcome::Stopped,
            (Some(_), true) => PhaseOutcome::Ended,
            (Some(_), false) => PhaseOutcome::Lost,
        };
        Finished {
            step: Step::default(),
            outcome,
        }
    }

    fn replay(&mut self, records: &[JournalRecord]) -> Result<Step, ReplayError> {
        let mut events = Vec::new();
        for (index, record) in records.iter().enumerate() {
            let expected = index as u64;
            if record.seq != expected {
                return Err(ReplayError::OutOfOrder {
                    expected,
                    found: record.seq,
                });
            }
            match record.direction {
                Direction::In => self.note_written(&record.bytes),
                Direction::Out => events.extend(self.feed(&record.bytes, record.wall_ns).events),
            }
        }
        // A replay writes nothing to the child.
        Ok(Step {
            events,
            writes: Vec::new(),
        })
    }
}

// --- Scenarios ---

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

/// The journal of a live conversation: the writes as `in` records, the child's lines as `out` records, in order.
fn live_conversation() -> (FakeSession, Vec<JournalRecord>) {
    let mut session = FakeSession::default();
    let mut journal: Vec<JournalRecord> = Vec::new();
    let mut log = |direction: Direction, bytes: Vec<u8>, wall_ns: u64| {
        let seq = journal.len() as u64;
        journal.push(JournalRecord {
            seq,
            wall_ns,
            direction,
            bytes,
        });
    };
    for write in session.start().writes {
        log(Direction::In, write, 100);
    }
    log(Direction::Out, line_of("session s-1"), 110);
    session.feed(&line_of("session s-1"), 110);
    for write in session.send_turn("fix the bug").writes {
        log(Direction::In, write, 120);
    }
    log(Direction::Out, line_of("ask r-1"), 130);
    session.feed(&line_of("ask r-1"), 130);
    log(Direction::Out, line_of("ask r-2"), 140);
    session.feed(&line_of("ask r-2"), 140);
    for write in session.answer("r-1", PermissionReply::Allow {}).writes {
        log(Direction::In, write, 150);
    }
    (session, journal)
}

#[test]
fn start_writes_the_handshake_and_reports_no_event_yet() {
    let mut session = FakeSession::default();
    let step = session.start();
    assert_eq!(step.writes, vec![line_of("new")]);
    assert!(step.events.is_empty());
}

#[test]
fn feeding_the_session_line_binds_the_id_with_the_time_it_arrived() {
    let mut session = FakeSession::default();
    let step = session.feed(b"session s-1\n", 1_700_000_000_000_000_000);
    assert_eq!(
        step.events,
        vec![stamped(
            Some(1_700_000_000_000_000_000),
            AgentEvent::SessionBound { id: "s-1".into() }
        )]
    );
}

#[test]
fn feeding_a_line_writes_nothing_to_the_child() {
    let mut session = FakeSession::default();
    assert!(session.feed(b"message hello\n", 5).writes.is_empty());
}

#[test]
fn send_turn_writes_the_turn_and_has_no_stored_time_to_stamp() {
    let mut session = FakeSession::default();
    let step = session.send_turn("hello");
    assert_eq!(step.writes, vec![line_of("turn hello")]);
    assert!(step.events.iter().all(|event| event.wall_ns.is_none()));
}

#[test]
fn a_permission_request_stays_pending_until_it_is_answered() {
    let mut session = FakeSession::default();
    session.feed(b"ask r-1\n", 10);
    session.feed(b"message still working\n", 20);
    assert_eq!(session.snapshot().pending, vec!["r-1".to_string()]);
}

#[test]
fn answering_allow_writes_the_reply_and_clears_the_request() {
    let mut session = FakeSession::default();
    session.feed(b"ask r-1\n", 10);
    let step = session.answer("r-1", PermissionReply::Allow {});
    assert_eq!(step.writes, vec![line_of("reply r-1 allow")]);
    assert!(session.snapshot().pending.is_empty());
}

#[test]
fn answering_deny_with_a_message_and_cancel_write_their_own_words() {
    let mut session = FakeSession::default();
    session.feed(b"ask r-1\n", 10);
    session.feed(b"ask r-2\n", 11);
    let deny = session.answer(
        "r-1",
        PermissionReply::Deny {
            message: Some("not in this folder".into()),
        },
    );
    let cancel = session.answer("r-2", PermissionReply::Cancel {});
    assert_eq!(deny.writes, vec![line_of("reply r-1 deny")]);
    assert_eq!(cancel.writes, vec![line_of("reply r-2 cancel")]);
}

#[test]
fn answering_a_request_nobody_asked_writes_nothing() {
    let mut session = FakeSession::default();
    let step = session.answer("r-9", PermissionReply::Allow {});
    assert!(step.writes.is_empty());
    assert!(step.events.is_empty());
}

#[test]
fn cancel_writes_cancel_and_the_turn_then_ends_cancelled() {
    let mut session = FakeSession::default();
    assert_eq!(session.cancel().writes, vec![line_of("cancel")]);
    let step = session.feed(b"done\n", 50);
    assert_eq!(
        step.events,
        vec![stamped(
            Some(50),
            AgentEvent::TurnEnd {
                reason: TurnEndReason::Cancelled {},
                cancel_requested: true,
                empty: false,
            }
        )]
    );
}

#[test]
fn finish_after_a_turn_end_is_ended() {
    let mut session = FakeSession::default();
    session.feed(b"done\n", 50);
    assert_eq!(session.finish(&exited()).outcome, PhaseOutcome::Ended);
}

#[test]
fn finish_with_no_exit_record_is_lost_whatever_the_stream_said() {
    let mut session = FakeSession::default();
    session.feed(b"done\n", 50);
    let gone = ExitInfo {
        record: None,
        stderr_tail: "shim died".into(),
    };
    assert_eq!(session.finish(&gone).outcome, PhaseOutcome::Lost);
}

#[test]
fn finish_after_a_cancelled_turn_is_stopped() {
    let mut session = FakeSession::default();
    session.cancel();
    session.feed(b"done\n", 50);
    assert_eq!(session.finish(&exited()).outcome, PhaseOutcome::Stopped);
}

#[test]
fn the_same_input_gives_the_same_output() {
    let run = || {
        let mut session = FakeSession::default();
        let mut steps = vec![session.start()];
        steps.push(session.feed(b"session s-1\n", 110));
        steps.push(session.send_turn("go"));
        steps.push(session.feed(b"ask r-1\n", 130));
        steps.push(session.answer("r-1", PermissionReply::Allow {}));
        steps
    };
    assert_eq!(run(), run());
}

#[test]
fn replay_rebuilds_the_state_of_the_live_session_including_a_pending_request() {
    let (live, journal) = live_conversation();
    let mut rebuilt = FakeSession::default();
    rebuilt.replay(&journal).unwrap();
    assert_eq!(rebuilt.snapshot(), live.snapshot());
    assert_eq!(rebuilt.snapshot().pending, vec!["r-2".to_string()]);
}

#[test]
fn replay_stamps_events_with_the_stored_wall_ns() {
    let (_, journal) = live_conversation();
    let mut rebuilt = FakeSession::default();
    let step = rebuilt.replay(&journal).unwrap();
    let times: Vec<Option<u64>> = step.events.iter().map(|event| event.wall_ns).collect();
    assert_eq!(times, vec![Some(110), Some(130), Some(140)]);
}

#[test]
fn replay_writes_nothing_to_the_child() {
    let (_, journal) = live_conversation();
    let mut rebuilt = FakeSession::default();
    assert!(rebuilt.replay(&journal).unwrap().writes.is_empty());
}

#[test]
fn replay_of_an_empty_journal_changes_nothing() {
    let mut session = FakeSession::default();
    let step = session.replay(&[]).unwrap();
    assert_eq!(session.snapshot(), Snapshot::default());
    assert!(step.events.is_empty() && step.writes.is_empty());
}

#[test]
fn replay_over_records_out_of_seq_order_is_an_error() {
    let (_, mut journal) = live_conversation();
    journal.swap(1, 2);
    let mut rebuilt = FakeSession::default();
    assert_eq!(
        rebuilt.replay(&journal),
        Err(ReplayError::OutOfOrder {
            expected: 1,
            found: 2
        })
    );
}

#[test]
fn a_session_is_usable_through_a_trait_object() {
    let mut session: Box<dyn AgentSession> = Box::new(FakeSession::default());
    assert_eq!(session.start().writes, vec![line_of("new")]);
}

#[test]
fn core_reads_no_clock() {
    // The names are joined at run time so that this test does not match itself.
    let forbidden = [
        ["System", "Time"].concat(),
        ["Instant", "::now"].concat(),
        ["UNIX", "_EPOCH"].concat(),
    ];
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut found = Vec::new();
    for entry in std::fs::read_dir(&src).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|ext| ext != "rs") {
            continue;
        }
        for (number, line) in std::fs::read_to_string(&path).unwrap().lines().enumerate() {
            let code = line.trim_start();
            if code.starts_with("//") {
                continue;
            }
            if forbidden.iter().any(|name| line.contains(name.as_str())) {
                found.push(format!("{}:{}", path.display(), number + 1));
            }
        }
    }
    assert!(found.is_empty(), "core reads a clock at {found:?}");
}
