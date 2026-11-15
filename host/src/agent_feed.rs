//! What the daemon does with an agent phase's stdout (0.3, The stream protocol; ts-0065-kx). An [`AgentFeed`] holds the
//! phase's [`AgentSession`] and reads the shim's journal as it grows: every `out` record is one stdout line with the
//! time the shim saw it, and goes to [`AgentSession::feed`] with that time. The journal is the source and not the
//! pipe, because the shim writes a line there before anything else sees it, so what the session was fed is what a
//! restart can replay.
//!
//! ## Settling
//!
//! A phase settles once the child has exited **and** its stdout has ended, in either order (0.3). The shim reports
//! `Exited` only after it has drained both pipes, bounded by its `drain_after_exit`, and writes `exit.json` after
//! that. So when the daemon hears `Exited`, the journal holds every line that will ever be in it: [`AgentFeed::settle`]
//! reads what is left, then calls [`AgentSession::finish`]. A child that exits first leaves its last lines in the
//! pipe, and the shim reads them before it reports; a child whose stdout ends first is waited for, and the exit
//! comes when it does. Neither order loses the `result` line.
//!
//! ## What it keeps
//!
//! The last [`Usage`] the session reported and the final report's text, for the phase's record
//! ([`crate::store::PhaseRecord`]). The events themselves are not stored here: that is the metrics and spans item's
//! (ts-0067).
//!
//! ## What it does not do
//!
//! It writes nothing to the child: the first turn is the caller's ([`AgentFeed::send_turn`]), and the feed asks for
//! stdin to be closed once a turn ends ([`Pumped::close_stdin`]) when it was made to ([`AgentFeed::close_after_turn`]).
//! A line the session answers with bytes of its own (the Claude session never does) is returned in [`Pumped::writes`]
//! for the caller to send without waiting.

use crate::shim::files;
use crate::store::PhaseRecord;
use collide_core::adapter::{AgentSession, ExitInfo, PhaseFailure, PhaseOutcome, Step};
use collide_core::agent::{AgentEvent, Usage};
use collide_core::journal::{Direction, JournalEnd, read_journal_from};
use collide_core::shim::ExitRecord;
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

/// How much of the end of the child's stderr the session is given as evidence (`ExitInfo::stderr_tail`).
pub const STDERR_TAIL_BYTES: u64 = 4096;

/// What one read of the journal found.
#[derive(Debug, Default, PartialEq)]
pub struct Pumped {
    /// Bytes the session wants written to the child, in order.
    pub writes: Vec<Vec<u8>>,
    /// A turn ended and the feed was made to close stdin after one: the caller closes it.
    pub close_stdin: bool,
    /// The last usage changed, so the phase's record is worth writing again.
    pub record_changed: bool,
    /// A problem to log: the journal could not be read or holds a damaged record.
    pub problem: Option<String>,
}

/// The text of a failure as the store keeps it.
pub fn failure_text(cause: &PhaseFailure) -> &'static str {
    match cause {
        PhaseFailure::EmptyTurn => "empty_turn",
        PhaseFailure::Refusal => "refusal",
        PhaseFailure::MaxTokens => "max_tokens",
        PhaseFailure::MaxTurns => "max_turns",
        PhaseFailure::Blocked { .. } => "blocked",
        PhaseFailure::Api { .. } => "api",
        PhaseFailure::NoResult => "no_result",
    }
}

pub struct AgentFeed {
    session: Box<dyn AgentSession + Send>,
    journal: PathBuf,
    stderr_log: PathBuf,
    /// The bytes of the journal already read: a record boundary.
    offset: u64,
    next_seq: u64,
    close_after_turn: bool,
    stdin_closed: bool,
    damaged: bool,
    usage: Option<Usage>,
    report: Option<String>,
}

impl AgentFeed {
    /// A feed for the phase whose shim directory is `phase_dir`.
    pub fn new(session: Box<dyn AgentSession + Send>, phase_dir: &Path) -> AgentFeed {
        AgentFeed {
            session,
            journal: phase_dir.join(files::JOURNAL),
            stderr_log: phase_dir.join(files::STDERR_LOG),
            offset: 0,
            next_seq: 0,
            close_after_turn: false,
            stdin_closed: false,
            damaged: false,
            usage: None,
            report: None,
        }
    }

    /// Asks for stdin to be closed after the first turn that ends: a run of one turn, whose Claude child waits for
    /// stdin to close before it exits.
    pub fn close_after_turn(mut self) -> AgentFeed {
        self.close_after_turn = true;
        self
    }

    /// The session's own start: what it would write before any turn (nothing, for Claude).
    pub fn start(&mut self) -> Step {
        let step = self.session.start();
        self.take(&step);
        step
    }

    /// A user turn: the bytes to write, which the caller sends once it has let go of the feed.
    pub fn send_turn(&mut self, text: &str) -> Step {
        let step = self.session.send_turn(text);
        self.take(&step);
        step
    }

    /// The record of the phase as far as the session has reported.
    pub fn record(&self, failure: Option<&PhaseFailure>) -> PhaseRecord {
        PhaseRecord {
            usage: self.usage.clone(),
            failure: failure.map(|cause| failure_text(cause).to_string()),
            report: self.report.clone(),
        }
    }

    /// Reads the records the shim has added to the journal since the last read and feeds the `out` ones to the session.
    pub fn pump(&mut self) -> Pumped {
        let mut pumped = Pumped::default();
        if self.damaged {
            return pumped;
        }
        let bytes = match self.read_new() {
            Ok(bytes) => bytes,
            Err(problem) => {
                pumped.problem = Some(problem);
                return pumped;
            }
        };
        if bytes.is_empty() {
            return pumped;
        }
        let read = read_journal_from(&bytes, self.next_seq);
        let usage_before = self.usage.clone();
        let report_before = self.report.clone();
        for record in &read.records {
            if record.direction == Direction::Out {
                let step = self.session.feed(&record.bytes, record.wall_ns);
                if self.take(&step) {
                    pumped.close_stdin = true;
                }
                pumped.writes.extend(step.writes);
            }
        }
        self.next_seq += read.records.len() as u64;
        self.offset += read.valid_len as u64;
        if let JournalEnd::Damaged { error } = &read.end {
            self.damaged = true;
            pumped.problem = Some(format!(
                "the journal holds a record that cannot be read ({error}); the rest is not fed"
            ));
        }
        if pumped.close_stdin {
            if self.stdin_closed {
                pumped.close_stdin = false;
            } else {
                self.stdin_closed = true;
            }
        }
        pumped.record_changed = self.usage != usage_before || self.report != report_before;
        pumped
    }

    /// The child has exited and its stdout has ended: reads what is left of the journal and asks the session how the
    /// phase ended. Returns the outcome and the record to store.
    pub fn settle(
        &mut self,
        exit: Option<ExitRecord>,
    ) -> (PhaseOutcome, PhaseRecord, Option<String>) {
        let last = self.pump();
        let problem = last.problem;
        let info = ExitInfo {
            record: exit,
            stderr_tail: self.stderr_tail(),
        };
        let finished = self.session.finish(&info);
        self.take(&finished.step);
        let failure = match &finished.outcome {
            PhaseOutcome::Failed { cause } => Some(cause.clone()),
            _ => None,
        };
        let record = self.record(failure.as_ref());
        (finished.outcome, record, problem)
    }

    /// Takes what a step reported. Returns whether a turn ended and the feed is to close stdin.
    fn take(&mut self, step: &Step) -> bool {
        let mut turn_ended = false;
        for stamped in &step.events {
            match &stamped.event {
                AgentEvent::Usage { usage } => self.usage = Some(usage.clone()),
                AgentEvent::FinalReport { text, .. } => self.report = Some(text.clone()),
                AgentEvent::TurnEnd { .. } => turn_ended = true,
                _ => {}
            }
        }
        turn_ended && self.close_after_turn
    }

    /// The journal's bytes from `offset` to its end. A journal that is not there yet reads as empty.
    fn read_new(&self) -> Result<Vec<u8>, String> {
        let mut file = match fs::File::open(&self.journal) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(format!("reading {}: {error}", self.journal.display())),
        };
        file.seek(SeekFrom::Start(self.offset))
            .map_err(|error| format!("reading {}: {error}", self.journal.display()))?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)
            .map_err(|error| format!("reading {}: {error}", self.journal.display()))?;
        Ok(bytes)
    }

    fn stderr_tail(&self) -> String {
        let read = || -> std::io::Result<Vec<u8>> {
            let mut file = fs::File::open(&self.stderr_log)?;
            let len = file.metadata()?.len();
            file.seek(SeekFrom::Start(len.saturating_sub(STDERR_TAIL_BYTES)))?;
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes)?;
            Ok(bytes)
        };
        read()
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::claude::session::ClaudeSession;
    use collide_core::agent::SessionStart;
    use collide_core::journal::{JournalRecord, encode_record};
    use std::io::Write;
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    /// A phase folder for one test; `Drop` removes the files in it by their own names, then the folder.
    struct Dir(PathBuf);

    impl Dir {
        fn new() -> Dir {
            let n = COUNTER.fetch_add(1, Ordering::SeqCst);
            let dir = std::env::temp_dir().join(format!("collide-feed-{}-{n}", std::process::id()));
            fs::create_dir_all(&dir).unwrap();
            Dir(dir)
        }

        fn journal(&self) -> PathBuf {
            self.0.join(files::JOURNAL)
        }

        fn append(&self, bytes: &[u8]) {
            let mut file = fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(self.journal())
                .unwrap();
            file.write_all(bytes).unwrap();
        }
    }

    impl Drop for Dir {
        fn drop(&mut self) {
            for name in [files::JOURNAL, files::STDERR_LOG] {
                let _ = fs::remove_file(self.0.join(name));
            }
            let _ = fs::remove_dir(&self.0);
        }
    }

    fn feed_in(dir: &Dir) -> AgentFeed {
        let session = ClaudeSession::new(SessionStart::Assign {
            uuid: "7c1b0e52-9f3a-4d2e-8b61-0a5d3c9e4f17".to_string(),
        });
        AgentFeed::new(Box::new(session), &dir.0).close_after_turn()
    }

    /// The lines of the `report` recording, which ends in a `result`.
    fn report_lines() -> Vec<Vec<u8>> {
        include_str!("../tests/adapters/claude/fixtures/recordings/report.jsonl")
            .lines()
            .filter(|line| line.starts_with("{\"say\""))
            .map(|line| {
                let step: serde_json::Value = serde_json::from_str(line).unwrap();
                let mut bytes = serde_json::to_vec(&step["say"]).unwrap();
                bytes.push(b'\n');
                bytes
            })
            .collect()
    }

    fn out(seq: u64, bytes: &[u8]) -> Vec<u8> {
        encode_record(&JournalRecord {
            seq,
            wall_ns: 1_000 + seq,
            direction: Direction::Out,
            bytes: bytes.to_vec(),
        })
        .unwrap()
    }

    #[test]
    fn a_pump_reads_only_what_was_added_since_the_last_one() {
        let dir = Dir::new();
        let lines = report_lines();
        let mut feed = feed_in(&dir);
        feed.send_turn("go");
        assert_eq!(feed.pump(), Pumped::default(), "no journal yet");
        dir.append(&out(0, &lines[0]));
        feed.pump();
        dir.append(&out(1, &lines[1]));
        let second = feed.pump();
        assert!(second.problem.is_none(), "{second:?}");
        assert_eq!(feed.next_seq, 2);
    }

    #[test]
    fn a_record_that_is_cut_short_is_read_when_the_rest_of_it_arrives() {
        let dir = Dir::new();
        let lines = report_lines();
        let mut feed = feed_in(&dir);
        feed.send_turn("go");
        let record = out(0, &lines[0]);
        let (head, tail) = record.split_at(record.len() / 2);
        dir.append(head);
        assert!(feed.pump().problem.is_none());
        assert_eq!(feed.next_seq, 0, "half a record is not read");
        dir.append(tail);
        feed.pump();
        assert_eq!(feed.next_seq, 1);
    }

    #[test]
    fn stdin_is_asked_to_close_once_after_the_first_turn_ends() {
        let dir = Dir::new();
        let mut feed = feed_in(&dir);
        feed.send_turn("go");
        for (seq, line) in report_lines().iter().enumerate() {
            dir.append(&out(seq as u64, line));
        }
        assert!(feed.pump().close_stdin);
        assert!(!feed.pump().close_stdin, "asked once");
    }

    #[test]
    fn settling_with_a_result_gives_the_report_and_the_last_usage() {
        let dir = Dir::new();
        let mut feed = feed_in(&dir);
        feed.send_turn("go");
        for (seq, line) in report_lines().iter().enumerate() {
            dir.append(&out(seq as u64, line));
        }
        let exit = ExitRecord {
            protocol: 1,
            end: collide_core::shim::ExitEnd::Exited { code: 0 },
            cpu_user_us: 0,
            cpu_system_us: 0,
            max_rss_bytes: 0,
        };
        let (outcome, record, problem) = feed.settle(Some(exit));
        assert_eq!(outcome, PhaseOutcome::Ended);
        assert_eq!(problem, None);
        assert_eq!(
            record.report.as_deref(),
            Some("The notes start with the first line.")
        );
        assert!(record.usage.is_some());
        assert_eq!(record.failure, None);
    }

    #[test]
    fn a_journal_with_a_damaged_record_stops_feeding_and_says_why() {
        let dir = Dir::new();
        let mut feed = feed_in(&dir);
        let mut bad = out(0, b"x\n");
        bad[16] = 9; // the direction byte
        dir.append(&bad);
        let pumped = feed.pump();
        assert!(pumped.problem.unwrap().contains("cannot be read"));
        dir.append(&out(1, b"y\n"));
        assert_eq!(feed.pump(), Pumped::default());
    }

    #[test]
    fn a_failure_is_worded_for_the_store() {
        assert_eq!(failure_text(&PhaseFailure::NoResult), "no_result");
        assert_eq!(
            failure_text(&PhaseFailure::Api { status: Some(529) }),
            "api"
        );
        assert_eq!(
            failure_text(&PhaseFailure::Blocked { reason: None }),
            "blocked"
        );
    }
}
