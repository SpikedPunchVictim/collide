//! The Claude session (0.3, Two core traits): one per child, a sans-IO state machine. It is nearly empty: the launch
//! line carries the first turn, so `start` writes nothing, and `feed` is the stream parser measured by S1. It reads no
//! clock and opens no file; the same input gives the same output, so a daemon restart rebuilds it from the shim's
//! journal with `replay`.
//!
//! `replay` is real here: it is [`replay_records`] over this session's [`Replayable`]. What a replay needs from the
//! session is the two stubs below, which a live write goes through as well, so a rebuilt session and a live one reach
//! the same state.
//!
//! | tag | method | what it does |
//! |---|---|---|
//! | `claude: session start` | `start` | the `SessionBound` event for `Assign` and `Resume`, which know their id at once |
//! | `claude: stream parser` | `feed`, `finish`, `read_line` | events from one line of stream-json; the phase's outcome |
//! | `claude: turn input` | `send_turn`, `answer`, `cancel` | a user turn, a `control_response`, an `interrupt` on stdin |
//! | `claude: replay` | `note_written` | learns what the session wrote (turns, answers) from an `in` record |

use collide_core::adapter::{AgentSession, ExitInfo, Finished, ReplayError, Step};
use collide_core::agent::{PermissionReply, SessionStart};
use collide_core::journal::JournalRecord;
use collide_core::replay::{Replayable, replay_records};

/// One Claude child's conversation.
#[derive(Debug, Clone, PartialEq)]
pub struct ClaudeSession {
    start: SessionStart,
}

impl ClaudeSession {
    /// A session that starts as `start` says: Claude supports `Assign` (a UUID Collide picked) and `Resume`.
    pub fn new(start: SessionStart) -> Self {
        Self { start }
    }

    /// How the session started.
    pub fn session_start(&self) -> &SessionStart {
        &self.start
    }
}

impl AgentSession for ClaudeSession {
    fn start(&mut self) -> Step {
        todo!("claude: session start")
    }

    fn feed(&mut self, line: &[u8], wall_ns: u64) -> Step {
        self.read_line(line, wall_ns)
    }

    fn send_turn(&mut self, text: &str) -> Step {
        let _ = text;
        todo!("claude: turn input")
    }

    fn answer(&mut self, request_id: &str, reply: PermissionReply) -> Step {
        let _ = (request_id, reply);
        todo!("claude: turn input")
    }

    fn cancel(&mut self) -> Step {
        todo!("claude: turn input")
    }

    fn finish(&mut self, exit: &ExitInfo) -> Finished {
        let _ = exit;
        todo!("claude: stream parser")
    }

    fn replay(&mut self, records: &[JournalRecord]) -> Result<Step, ReplayError> {
        replay_records(self, records)
    }
}

impl Replayable for ClaudeSession {
    fn note_written(&mut self, bytes: &[u8]) -> Result<(), String> {
        let _ = bytes;
        todo!("claude: replay")
    }

    fn read_line(&mut self, line: &[u8], wall_ns: u64) -> Step {
        let _ = (line, wall_ns);
        todo!("claude: stream parser")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_session_remembers_how_it_started() {
        let session = ClaudeSession::new(SessionStart::Resume { id: "abc".into() });
        assert_eq!(
            session.session_start(),
            &SessionStart::Resume { id: "abc".into() }
        );
    }

    #[test]
    #[should_panic(expected = "claude: stream parser")]
    fn feed_is_a_stub_tagged_for_stage_3b() {
        let mut session = ClaudeSession::new(SessionStart::Assign { uuid: "u".into() });
        let _ = session.feed(b"{}", 1);
    }

    #[test]
    #[should_panic(expected = "claude: replay")]
    fn replaying_a_written_record_reaches_the_replay_stub() {
        use collide_core::journal::Direction;
        let mut session = ClaudeSession::new(SessionStart::Assign { uuid: "u".into() });
        let record = JournalRecord {
            seq: 0,
            wall_ns: 1,
            direction: Direction::In,
            bytes: b"{}".to_vec(),
        };
        let _ = session.replay(&[record]);
    }

    #[test]
    fn replaying_an_empty_journal_gives_an_empty_step() {
        let mut session = ClaudeSession::new(SessionStart::Assign { uuid: "u".into() });
        assert_eq!(session.replay(&[]), Ok(Step::default()));
    }
}
