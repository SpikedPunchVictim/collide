//! The shim protocol (0.2, the shim): the messages between `collide-daemon` and a `collide shim`, the version
//! handshake, and `exit.json`. There is one shim per child. The byte-level pieces are the framing of
//! [`crate::frame`] (every message below travels as one frame) and the journal of [`crate::journal`]. This module
//! is the types and pure codecs. The shim and the daemon that speak it are Stage 3a.
//!
//! ## Versioning
//!
//! [`SHIM_PROTOCOL`] is the version of this protocol, and it moves on its own: it is not the daemon's build, the
//! hook protocol's or any other protocol's. A shim outlives the daemon that started it, so after an upgrade a new
//! daemon meets shims an older build started. **The daemon accepts the previous shim version**, the current one,
//! and refuses any other (see [`check_shim_version`]). A change that an older shim could not read raises the
//! version, and the build keeps reading the one before it.
//!
//! ## The handshake
//!
//! 1. The shim connects and sends [`ShimMessage::Hello`] with the version it speaks.
//! 2. The daemon answers with [`answer_hello`]: [`DaemonMessage::Welcome`] when the version is the current or the
//!    previous one, naming the version both sides speak from then on (the shim's, so a shim of the previous version
//!    is spoken to as one), or [`DaemonMessage::Refused`] with a message that names the shim's version, the current
//!    version and the previous one.
//!
//! ## What the shim promises
//!
//! - It writes a journal record for the bytes of a [`DaemonMessage::Write`] before the child sees them, and says
//!   so with [`ShimMessage::Wrote`] carrying that record's `seq`.
//! - It writes `exit.json` atomically (temp file, fsync, rename, fsync of the directory) when the child is reaped
//!   with `wait4`, and sends [`ShimMessage::Exited`] with the same [`ExitRecord`]. On a full disk it keeps the record
//!   in memory and serves it until the daemon sends [`DaemonMessage::AckExit`].
//! - A shim that dies without writing `exit.json` makes its phase `lost`, whatever the agent's stream said.
//!
//! | what | type |
//! |---|---|
//! | shim to daemon | [`ShimMessage`] |
//! | daemon to shim | [`DaemonMessage`] |
//! | `exit.json` | [`ExitRecord`] |

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt;

/// The version of the shim protocol. It starts at 1 and is raised when a shim of the previous version could no
/// longer read what a daemon writes or the reverse.
pub const SHIM_PROTOCOL: u32 = 1;

/// A signal the daemon asks the shim to send to its child's process group (0.5, Stopping). The shim sends it; the
/// daemon never signals a child it does not own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum StopSignal {
    Hup,
    Term,
    Kill,
}

/// How the child ended. Exactly one of the two is true of a reaped child, so the type holds one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ExitEnd {
    /// The child called `exit`. `code` is its exit status.
    Exited { code: i32 },
    /// A signal ended the child. `signal` is its number.
    Signaled { signal: i32 },
}

/// `exit.json`: what the shim learned when it reaped its child with `wait4` (0.2; measured S3). The file holds this
/// one object as JSON. `protocol` is the shim's [`SHIM_PROTOCOL`], so a daemon reads a record a shim of the
/// previous version wrote.
///
/// The times are the child's own and its waited-for descendants' (`ru_utime` and `ru_stime`), in microseconds.
/// `max_rss_bytes` is the largest resident set of any one process counted, in bytes: the shim converts `ru_maxrss`,
/// which macOS gives in bytes and Linux in KiB. A descendant that was double-forked and orphaned is in neither
/// number (measured S3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExitRecord {
    pub protocol: u32,
    pub end: ExitEnd,
    pub cpu_user_us: u64,
    pub cpu_system_us: u64,
    pub max_rss_bytes: u64,
}

/// A message from the shim to the daemon.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ShimMessage {
    /// The first message on a connection: the protocol version the shim speaks.
    Hello { protocol: u32 },
    /// A [`DaemonMessage::Write`] is in the journal, as the record with this `seq`. The journal record is
    /// written before the child sees the bytes, so the child may not have read them yet.
    Wrote { seq: u64 },
    /// The child was reaped. The shim serves this until it gets [`DaemonMessage::AckExit`].
    Exited { exit: ExitRecord },
    /// The shim could not do what it was asked, for example a journal write that failed. `message` says what and
    /// why, with context.
    Failed { message: String },
}

/// A message from the daemon to the shim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum DaemonMessage {
    /// The answer to a `Hello` that is accepted: the version spoken on this connection.
    Welcome { protocol: u32 },
    /// The answer to a `Hello` that is not: why, naming the shim's version, the current and the previous.
    Refused { message: String },
    /// Write `data`, as UTF-8, to the child's stdin: journal first, then the child.
    Write { data: String },
    /// Close the child's stdin.
    CloseStdin {},
    /// Send the signal to the child's process group.
    Signal { signal: StopSignal },
    /// The daemon has the exit record. The shim may stop serving it.
    AckExit {},
}

/// Which accepted version a shim speaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionMatch {
    Current,
    Previous,
}

/// A shim version the daemon does not accept. [`fmt::Display`] names the shim's version, the current one and the
/// previous one, and says whether the shim is older or newer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolRefused {
    pub announced: u32,
    pub current: u32,
    pub previous: Option<u32>,
}

impl fmt::Display for ProtocolRefused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let age = if self.announced < self.current {
            "older"
        } else {
            "newer"
        };
        write!(
            f,
            "shim protocol {} is {age} than this daemon accepts: it speaks {} (current)",
            self.announced, self.current
        )?;
        match self.previous {
            Some(previous) => write!(f, " and accepts {previous} (previous)"),
            None => write!(f, " and has no previous version"),
        }
    }
}

impl std::error::Error for ProtocolRefused {}

/// The version before `current`, which a daemon also accepts. Versions start at 1, so version 1 has none.
pub fn previous_version(current: u32) -> Option<u32> {
    current.checked_sub(1).filter(|previous| *previous >= 1)
}

/// Decides whether a daemon speaking `current` accepts a shim that announced `announced`: the current version and
/// the previous one, nothing else. An older version and a newer one (a daemon rolled back under a running shim)
/// are both refused.
pub fn check_shim_version(current: u32, announced: u32) -> Result<VersionMatch, ProtocolRefused> {
    if announced == current {
        Ok(VersionMatch::Current)
    } else if Some(announced) == previous_version(current) {
        Ok(VersionMatch::Previous)
    } else {
        Err(ProtocolRefused {
            announced,
            current,
            previous: previous_version(current),
        })
    }
}

/// [`check_shim_version`] against this build's [`SHIM_PROTOCOL`].
pub fn accept_shim(announced: u32) -> Result<VersionMatch, ProtocolRefused> {
    check_shim_version(SHIM_PROTOCOL, announced)
}

/// The daemon's answer to a shim's `Hello`: `Welcome` with the shim's version when it is accepted, `Refused`
/// with the reason when it is not.
pub fn answer_hello(current: u32, announced: u32) -> DaemonMessage {
    match check_shim_version(current, announced) {
        Ok(_) => DaemonMessage::Welcome {
            protocol: announced,
        },
        Err(refused) => DaemonMessage::Refused {
            message: refused.to_string(),
        },
    }
}

/// Why `exit.json` could not be read. Each variant is a reason for the daemon to move the file aside and report,
/// never to overwrite it (0.2, Durability).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExitRecordError {
    /// The file is empty, or holds only white space.
    Empty,
    /// The file stops inside the JSON value, at this position.
    Cut { line: usize, column: usize },
    /// The file is not an exit record: not JSON, not UTF-8, or a field is missing, unknown or out of range.
    Invalid {
        line: usize,
        column: usize,
        reason: String,
    },
    /// A record of a protocol version the daemon does not accept.
    Protocol(ProtocolRefused),
}

impl fmt::Display for ExitRecordError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExitRecordError::Empty => write!(f, "exit.json is empty"),
            ExitRecordError::Cut { line, column } => write!(
                f,
                "exit.json ends inside its JSON value, at line {line} column {column}"
            ),
            ExitRecordError::Invalid {
                line,
                column,
                reason,
            } => write!(
                f,
                "exit.json is not an exit record (line {line} column {column}): {reason}"
            ),
            ExitRecordError::Protocol(refused) => write!(f, "exit.json: {refused}"),
        }
    }
}

impl std::error::Error for ExitRecordError {}

/// The bytes of `exit.json` for `record`: compact JSON and a final newline.
pub fn exit_record_bytes(record: &ExitRecord) -> Vec<u8> {
    let mut bytes = serde_json::to_vec(record).expect("an exit record is JSON");
    bytes.push(b'\n');
    bytes
}

/// Reads `exit.json` for a daemon speaking `current`: the record, or the typed reason the file is damaged.
pub fn parse_exit_record_for(current: u32, bytes: &[u8]) -> Result<ExitRecord, ExitRecordError> {
    if bytes.iter().all(u8::is_ascii_whitespace) {
        return Err(ExitRecordError::Empty);
    }
    let record: ExitRecord = serde_json::from_slice(bytes).map_err(|error| {
        if error.is_eof() {
            ExitRecordError::Cut {
                line: error.line(),
                column: error.column(),
            }
        } else {
            ExitRecordError::Invalid {
                line: error.line(),
                column: error.column(),
                reason: error.to_string(),
            }
        }
    })?;
    check_shim_version(current, record.protocol).map_err(ExitRecordError::Protocol)?;
    Ok(record)
}

/// [`parse_exit_record_for`] against this build's [`SHIM_PROTOCOL`].
pub fn parse_exit_record(bytes: &[u8]) -> Result<ExitRecord, ExitRecordError> {
    parse_exit_record_for(SHIM_PROTOCOL, bytes)
}
