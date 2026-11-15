//! Replaying a session over the shim's journal (0.2, the shim and its journal; 0.3, `AgentSession`, `replay`). After
//! a daemon restart a session is rebuilt from the journal of its child ([`crate::journal`]): the records in `seq`
//! order, an `in` record as bytes the session wrote and an `out` record as a line the child wrote, fed again with the
//! `wall_ns` the shim stored. This module is that walk, written once, and the bytes-to-session path around it:
//!
//! ```text
//! journal bytes --read_journal--> whole records --replay_records--> a rebuilt session and its events
//! ```
//!
//! - [`replay_records`] is the walk. A session's [`AgentSession::replay`](crate::adapter::AgentSession::replay) is
//!   this call over the two things a session must say about itself, [`Replayable`]. The two doubles of
//!   [`crate::testing`] do this, and so will the ACP session of Stage 3d.
//! - [`replay_journal_bytes`] is what the daemon calls: the journal file's bytes in, a rebuilt session out, with how
//!   the bytes ended.
//!
//! The walk is pure. It reads no clock, opens no file and writes nothing to the child: the returned [`Step`] has
//! events and no `writes`, because what the session wrote is already in the journal as `in` records.
//!
//! ## An error leaves the session half rebuilt
//!
//! A replay that returns `Err` has already applied the records before the one it stopped at. The caller discards the
//! session and builds a new one; it does not carry on with it.
//!
//! ## A journal that ends badly
//!
//! [`replay_journal_bytes`] replays every whole record that [`read_journal`] returns, then says how the bytes ended
//! in [`JournalReplay::end`]. A [`JournalEnd::Cut`] is what a crash leaves and is expected. A [`JournalEnd::Damaged`]
//! is not a crash: something other than the shim wrote or changed the file. It is replayed the same way, up to
//! [`JournalReplay::valid_len`], and reported; the caller looks at `end` and decides whether a session rebuilt from a
//! prefix is good enough, which the walk cannot know. (Refusing outright would leave the daemon with no session and
//! no way to show the run's last known state.)

use crate::adapter::{AgentSession, ReplayError, Step};
use crate::journal::{Direction, JournalEnd, JournalRecord, RECORD_HEADER_BYTES, read_journal};

/// What a session says about itself so that [`replay_records`] can rebuild it: how it reads what it wrote, and how it
/// reads what the child wrote.
pub trait Replayable {
    /// Learns what the session wrote: the ids it used, the turns it sent, the requests it answered. A live write
    /// should go through here too, so a replayed session and a live one reach the same state. `Err` is why the bytes
    /// are not something this session writes.
    fn note_written(&mut self, bytes: &[u8]) -> Result<(), String>;

    /// The child's line, as `AgentSession::feed` reads it, with the time the shim stored for it. The `writes` of the
    /// returned step are not used: a replay writes nothing.
    fn read_line(&mut self, line: &[u8], wall_ns: u64) -> Step;
}

/// The walk of `AgentSession::replay`: the records in `seq` order from 0, `in` records through
/// [`Replayable::note_written`], `out` records through [`Replayable::read_line`] with the stored time. The events
/// come back in order and nothing is written. A record out of order is [`ReplayError::OutOfOrder`], never moved; one
/// the session cannot read is [`ReplayError::Rejected`].
pub fn replay_records<S: Replayable + ?Sized>(
    session: &mut S,
    records: &[JournalRecord],
) -> Result<Step, ReplayError> {
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
            Direction::In => {
                session
                    .note_written(&record.bytes)
                    .map_err(|message| ReplayError::Rejected {
                        seq: record.seq,
                        message,
                    })?;
            }
            Direction::Out => {
                events.extend(session.read_line(&record.bytes, record.wall_ns).events)
            }
        }
    }
    Ok(Step {
        events,
        writes: Vec::new(),
    })
}

/// What [`replay_journal_bytes`] did.
#[derive(Debug, Clone, PartialEq)]
pub struct JournalReplay {
    /// The events of the replay, in order, and no writes.
    pub step: Step,
    /// How many whole records were replayed.
    pub records: usize,
    /// The offset just after the last whole record: a cut or damaged journal can be truncated here.
    pub valid_len: usize,
    /// How the bytes ended. [`JournalEnd::Damaged`] is for the caller to act on; see the module docs.
    pub end: JournalEnd,
}

/// Rebuilds `session` from the bytes of a journal: [`read_journal`], then [`AgentSession::replay`] over the whole
/// records. A record cut short by a crash is dropped (its offset is in `end`); a damaged journal is replayed up to the
/// damage and reported (see the module docs). An error is [`AgentSession::replay`]'s; a [`ReplayError::Rejected`]
/// names the offset of its record in the journal as well as its `seq`.
pub fn replay_journal_bytes<S: AgentSession + ?Sized>(
    session: &mut S,
    bytes: &[u8],
) -> Result<JournalReplay, ReplayError> {
    let read = read_journal(bytes);
    let step = session
        .replay(&read.records)
        .map_err(|error| with_offset(error, &read.records))?;
    Ok(JournalReplay {
        step,
        records: read.records.len(),
        valid_len: read.valid_len,
        end: read.end,
    })
}

/// Adds the offset of a rejected record to its message. A journal read from bytes has `seq` equal to the record's
/// index, so the offset is the length of the records before it. A `seq` that is not an index (a session that rejects
/// with its own number) keeps its message.
fn with_offset(error: ReplayError, records: &[JournalRecord]) -> ReplayError {
    match error {
        ReplayError::Rejected { seq, message } => {
            let message = match usize::try_from(seq)
                .ok()
                .filter(|&index| index < records.len())
            {
                Some(index) => {
                    let offset: usize = records[..index]
                        .iter()
                        .map(|record| RECORD_HEADER_BYTES + record.bytes.len())
                        .sum();
                    format!("{message} (the record at offset {offset})")
                }
                None => message,
            };
            ReplayError::Rejected { seq, message }
        }
        other => other,
    }
}
