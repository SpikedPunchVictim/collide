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
use collide_core::adapter::{
    AgentSession, ExitInfo, PhaseFailure, PhaseOutcome, StampedEvent, Step,
};
use collide_core::agent::{AgentEvent, PermissionRequest, Usage, UsageScope};
use collide_core::journal::{Direction, JournalEnd, read_journal_from};
use collide_core::replay::replay_journal_bytes;
use collide_core::shim::ExitRecord;
use collide_core::state::PauseCause;
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
    /// Permission requests the session was asked in this read, in order. The daemon puts each to the person.
    pub requests: Vec<PermissionRequest>,
}

/// Why a settled phase pauses and does not end or fail, and what the pause needs to be resumed (0.3, Usage limits).
#[derive(Debug, Clone, PartialEq)]
pub struct Pause {
    /// `UsageLimit` for a limit, `AskHuman` for a budget that ran out.
    pub cause: PauseCause,
    /// A limit's reset time, in milliseconds since the epoch, when the stream gave one.
    pub reset_at_ms: Option<u64>,
    /// What the session had cost when the phase paused, in US dollars: a running total, which is the session's whole
    /// cost and not this process's alone (S1: the total carries across `--resume`).
    pub spent_usd: Option<f64>,
}

/// The permission requests a step reports, in order.
fn requests_of(step: &Step) -> Vec<PermissionRequest> {
    step.events
        .iter()
        .filter_map(|stamped| match &stamped.event {
            AgentEvent::PermissionRequest { request } => Some(request.clone()),
            _ => None,
        })
        .collect()
}

/// The failure text of a phase that ran without Collide's settings ([`PhaseFailure::SettingsIgnored`]): the daemon reads
/// it back from the phase's record to raise the Queue item.
pub const SETTINGS_IGNORED_FAILURE: &str = "settings_ignored";

/// The text of a failure as the store keeps it. An API error keeps its status (`api_500`), when the result had one.
pub fn failure_text(cause: &PhaseFailure) -> String {
    if let PhaseFailure::Api {
        status: Some(status),
    } = cause
    {
        return format!("api_{status}");
    }
    let text = match cause {
        PhaseFailure::EmptyTurn => "empty_turn",
        PhaseFailure::Refusal => "refusal",
        PhaseFailure::MaxTokens => "max_tokens",
        PhaseFailure::MaxTurns => "max_turns",
        PhaseFailure::Budget => "budget",
        PhaseFailure::Blocked { .. } => "blocked",
        PhaseFailure::Api { .. } => "api",
        PhaseFailure::NoResult => "no_result",
        PhaseFailure::SettingsIgnored => SETTINGS_IGNORED_FAILURE,
        PhaseFailure::ModelUnavailable => "model_unavailable",
    };
    text.to_string()
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
    /// A turn has ended (in the stream read, or in a journal replayed).
    turn_ended: bool,
    /// The turn hit a usage limit, with the reset time when the stream gave one. A new turn clears it.
    limited: Option<Option<u64>>,
    /// The running total's cost, the last time the session gave one.
    spent_usd: Option<f64>,
    /// The events taken since the caller last asked ([`AgentFeed::take_events`]), for the metrics.
    taken: Vec<StampedEvent>,
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
            turn_ended: false,
            limited: None,
            spent_usd: None,
            taken: Vec::new(),
        }
    }

    /// A feed for a phase whose shim outlived the daemon: the journal is replayed into `session`, so a request it was
    /// asked and nobody answered is still pending ([`AgentFeed::pending_permissions`]), and reading goes on after the
    /// last whole record. An error says why the journal could not be replayed.
    pub fn rebuild(
        mut session: Box<dyn AgentSession + Send>,
        phase_dir: &Path,
    ) -> Result<AgentFeed, String> {
        let journal = phase_dir.join(files::JOURNAL);
        let bytes = match fs::read(&journal) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(error) => return Err(format!("reading {}: {error}", journal.display())),
        };
        let replayed = replay_journal_bytes(session.as_mut(), &bytes)
            .map_err(|error| format!("replaying {}: {error}", journal.display()))?;
        let mut feed = AgentFeed::new(session, phase_dir);
        feed.offset = replayed.valid_len as u64;
        feed.next_seq = replayed.records as u64;
        feed.damaged = matches!(replayed.end, JournalEnd::Damaged { .. });
        feed.take(&replayed.step);
        // The replayed events were recorded before the restart.
        feed.taken.clear();
        Ok(feed)
    }

    /// The permission requests the session holds that nobody has answered.
    pub fn pending_permissions(&self) -> Vec<PermissionRequest> {
        self.session.pending_permissions()
    }

    /// The answer to a permission request: the bytes to write to the child, which the caller sends once it has let go
    /// of the feed. Empty for a request the session does not hold.
    pub fn answer(
        &mut self,
        request_id: &str,
        reply: collide_core::agent::PermissionReply,
    ) -> Step {
        let step = self.session.answer(request_id, reply);
        self.take(&step);
        step
    }

    /// An interrupt for the child's turn: the bytes to write.
    pub fn interrupt(&mut self) -> Step {
        let step = self.session.cancel();
        self.take(&step);
        step
    }

    /// Why the phase pauses, once it has settled with `outcome`: a turn that hit a usage limit and failed, or one that
    /// ran out of budget. `None` for any other end.
    pub fn pause(&self, outcome: &PhaseOutcome) -> Option<Pause> {
        let cause = match (outcome, self.limited) {
            (
                PhaseOutcome::Failed {
                    cause: PhaseFailure::Budget,
                },
                _,
            ) => PauseCause::AskHuman {},
            (PhaseOutcome::Failed { .. }, Some(_)) => PauseCause::UsageLimit {},
            _ => return None,
        };
        Some(Pause {
            cause,
            reset_at_ms: self.limited.flatten(),
            spent_usd: self.spent_usd,
        })
    }

    /// The events the feed took since the last call, in order: what the metrics record. The caller drains them after
    /// each call that can take some, so the list stays short.
    pub fn take_events(&mut self) -> Vec<StampedEvent> {
        std::mem::take(&mut self.taken)
    }

    /// What the session had cost, as far as it has said.
    pub fn spent_usd(&self) -> Option<f64> {
        self.spent_usd
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
        self.limited = None;
        let step = self.session.send_turn(text);
        self.take(&step);
        step
    }

    /// The record of the phase as far as the session has reported.
    pub fn record(&self, failure: Option<&PhaseFailure>) -> PhaseRecord {
        PhaseRecord {
            usage: self.usage.clone(),
            failure: failure.map(failure_text),
            failure_evidence: None,
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
            self.ask_to_close(&mut pumped);
            return pumped;
        }
        let read = read_journal_from(&bytes, self.next_seq);
        let usage_before = self.usage.clone();
        let report_before = self.report.clone();
        for record in &read.records {
            if record.direction == Direction::Out {
                let step = self.session.feed(&record.bytes, record.wall_ns);
                self.take(&step);
                pumped.requests.extend(requests_of(&step));
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
        self.ask_to_close(&mut pumped);
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
        let mut record = self.record(failure.as_ref());
        record.failure_evidence = finished.evidence;
        (finished.outcome, record, problem)
    }

    /// Asks for stdin to be closed, once, when a turn has ended and the feed was made to close after one.
    fn ask_to_close(&mut self, pumped: &mut Pumped) {
        if self.close_after_turn && self.turn_ended && !self.stdin_closed {
            self.stdin_closed = true;
            pumped.close_stdin = true;
        }
    }

    /// Takes what a step reported.
    fn take(&mut self, step: &Step) {
        self.taken.extend(step.events.iter().cloned());
        for stamped in &step.events {
            match &stamped.event {
                AgentEvent::Usage { usage } => {
                    if usage.scope == UsageScope::RunningTotal
                        && let Some(cost) = &usage.cost
                    {
                        self.spent_usd = Some(cost.amount);
                    }
                    self.usage = Some(usage.clone());
                }
                AgentEvent::FinalReport { text, .. } => self.report = Some(text.clone()),
                AgentEvent::TurnEnd { .. } => self.turn_ended = true,
                AgentEvent::Limited { reset_at_ms } => self.limited = Some(*reset_at_ms),
                _ => {}
            }
        }
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
    fn the_events_a_feed_took_are_given_once_and_in_order() {
        let dir = Dir::new();
        let mut feed = feed_in(&dir);
        feed.send_turn("go");
        for (seq, line) in report_lines().iter().enumerate() {
            dir.append(&out(seq as u64, line));
        }
        feed.pump();
        let taken = feed.take_events();
        assert!(
            matches!(
                taken.last().map(|e| &e.event),
                Some(AgentEvent::TurnEnd { .. })
            ),
            "{taken:?}"
        );
        assert!(feed.take_events().is_empty(), "given once");
    }

    #[test]
    fn a_rebuilt_feed_gives_no_events_for_the_journal_it_replayed() {
        let dir = Dir::new();
        for (seq, line) in report_lines().iter().enumerate() {
            dir.append(&out(seq as u64, line));
        }
        let session = ClaudeSession::new(SessionStart::Assign {
            uuid: "7c1b0e52-9f3a-4d2e-8b61-0a5d3c9e4f17".to_string(),
        });
        let mut feed = AgentFeed::rebuild(Box::new(session), &dir.0).unwrap();
        assert!(
            feed.take_events().is_empty(),
            "the events of the replay were recorded before the restart"
        );
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
            failure_text(&PhaseFailure::SettingsIgnored),
            "settings_ignored"
        );
        assert_eq!(
            failure_text(&PhaseFailure::Api { status: Some(529) }),
            "api_529"
        );
        assert_eq!(failure_text(&PhaseFailure::Api { status: None }), "api");
        assert_eq!(
            failure_text(&PhaseFailure::ModelUnavailable),
            "model_unavailable"
        );
        assert_eq!(
            failure_text(&PhaseFailure::Blocked { reason: None }),
            "blocked"
        );
    }
}
