//! The shim's journal (0.2, the shim; 0.3, `AgentSession`). For one child the shim keeps one ordered file of the
//! bytes that went to the child's stdin (`in`) and the bytes read from its stdout (`out`). An ACP session is rebuilt
//! by replaying both directions, and one file with one counter gives replay a single order that two logs cannot.
//! This module is the record type and the pure codec. The shim's file writer is Stage 3a; replaying a session over
//! the journal is [`crate::replay`].
//!
//! ## The contract
//!
//! - **The shim writes a journal record before the child sees the bytes.** An `in` record is on disk (handed to
//!   the file) before the write to the child's stdin, so a crash between the two leaves a record for bytes the
//!   child may not have read, and never the reverse: an `in` record does not go missing. Replay treats a journaled
//!   `in` as written.
//! - `seq` is one counter for both directions. It starts at 0 and goes up by one for each record.
//! - `wall_ns` is the wall clock when the shim saw the bytes, in nanoseconds since the Unix epoch (0.2, Clocks).
//!   It is stored so that replay hands the same time to `AgentSession::feed` that the live session had.
//! - A record cut short by a crash is dropped at replay: [`read_journal`] returns every whole record before it.
//!
//! ## Byte layout
//!
//! The file is the records, back to back, with no header and no footer. Each record is:
//!
//! ```text
//! offset  size  field
//!      0     8  seq      u64, big-endian
//!      8     8  wall_ns  u64, big-endian
//!     16     1  dir      0 = in (written to the child's stdin), 1 = out (read from its stdout)
//!     17     4  len      u32, big-endian: the number of bytes that follow
//!     21   len  bytes    the traffic, as it was
//! ```
//!
//! A record is [`RECORD_HEADER_BYTES`] (21) plus `len` bytes long, and `len` is at most [`MAX_RECORD_BYTES`].
//!
//! The codec is pure and never panics: truncated and garbage input give a typed result with the offset.

use std::fmt;

/// The bytes of a record before its `bytes` field: `seq`, `wall_ns`, `dir` and `len`.
pub const RECORD_HEADER_BYTES: usize = 21;

/// The most bytes one record may hold (16 MiB, the same as a frame's payload). A longer `len` while reading is
/// damage, not a record to wait for.
pub const MAX_RECORD_BYTES: usize = 16 * 1024 * 1024;

const DIRECTION_IN: u8 = 0;
const DIRECTION_OUT: u8 = 1;

/// Which way the bytes of a record went.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// Written to the child's stdin.
    In,
    /// Read from the child's stdout.
    Out,
}

impl Direction {
    fn to_byte(self) -> u8 {
        match self {
            Direction::In => DIRECTION_IN,
            Direction::Out => DIRECTION_OUT,
        }
    }

    fn from_byte(byte: u8) -> Option<Direction> {
        match byte {
            DIRECTION_IN => Some(Direction::In),
            DIRECTION_OUT => Some(Direction::Out),
            _ => None,
        }
    }
}

/// One journal record: `{seq, wall_ns, dir, bytes}`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JournalRecord {
    pub seq: u64,
    pub wall_ns: u64,
    pub direction: Direction,
    pub bytes: Vec<u8>,
}

/// A record that cannot be written because its `bytes` are over [`MAX_RECORD_BYTES`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordTooLarge {
    pub len: usize,
    pub max: usize,
}

impl fmt::Display for RecordTooLarge {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "journal record of {} bytes is over the limit of {} bytes; split it into several records",
            self.len, self.max
        )
    }
}

impl std::error::Error for RecordTooLarge {}

/// Damage found while reading a journal. Every variant gives the offset of the record it was found in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JournalError {
    /// The `dir` byte is neither 0 (in) nor 1 (out).
    BadDirection { offset: usize, found: u8 },
    /// The `len` is over [`MAX_RECORD_BYTES`].
    RecordTooLarge {
        offset: usize,
        len: usize,
        max: usize,
    },
    /// The `seq` is not the one after the last record's (0 for the first record).
    OutOfOrder {
        offset: usize,
        expected: u64,
        found: u64,
    },
}

impl fmt::Display for JournalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JournalError::BadDirection { offset, found } => write!(
                f,
                "journal record at offset {offset}: direction byte is {found}, expected 0 (in) or 1 (out)"
            ),
            JournalError::RecordTooLarge { offset, len, max } => write!(
                f,
                "journal record at offset {offset}: length {len} is over the limit of {max} bytes"
            ),
            JournalError::OutOfOrder {
                offset,
                expected,
                found,
            } => write!(
                f,
                "journal record at offset {offset}: expected seq {expected}, found {found}"
            ),
        }
    }
}

impl std::error::Error for JournalError {}

/// How a journal's bytes ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JournalEnd {
    /// The last record is whole, or there are no records.
    Clean,
    /// The bytes stop inside the record that starts at `offset`: a crash cut it short. It is not in `records`.
    Cut { offset: usize },
    /// The bytes at [`JournalRead::valid_len`] are not a record.
    Damaged { error: JournalError },
}

/// What [`read_journal`] found: the whole records, in order, and how the bytes ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JournalRead {
    pub records: Vec<JournalRecord>,
    /// The offset just after the last whole record. A cut journal can be truncated here and appended to.
    pub valid_len: usize,
    pub end: JournalEnd,
}

/// Writes one record in the layout above. Refuses a record whose `bytes` are over [`MAX_RECORD_BYTES`].
pub fn encode_record(record: &JournalRecord) -> Result<Vec<u8>, RecordTooLarge> {
    if record.bytes.len() > MAX_RECORD_BYTES {
        return Err(RecordTooLarge {
            len: record.bytes.len(),
            max: MAX_RECORD_BYTES,
        });
    }
    let mut out = Vec::with_capacity(RECORD_HEADER_BYTES + record.bytes.len());
    out.extend_from_slice(&record.seq.to_be_bytes());
    out.extend_from_slice(&record.wall_ns.to_be_bytes());
    out.push(record.direction.to_byte());
    out.extend_from_slice(&(record.bytes.len() as u32).to_be_bytes());
    out.extend_from_slice(&record.bytes);
    Ok(out)
}

/// Reads a journal from its first byte. It returns every whole record up to the first one that is cut short or
/// damaged, and says which of the two ended the read. A cut at any offset gives the records before it.
pub fn read_journal(bytes: &[u8]) -> JournalRead {
    let mut records = Vec::new();
    let mut offset = 0;
    let end = loop {
        if offset == bytes.len() {
            break JournalEnd::Clean;
        }
        let rest = &bytes[offset..];
        let Some(header) = rest.first_chunk::<RECORD_HEADER_BYTES>() else {
            break JournalEnd::Cut { offset };
        };
        let seq = u64::from_be_bytes(header[0..8].try_into().expect("8 bytes"));
        let wall_ns = u64::from_be_bytes(header[8..16].try_into().expect("8 bytes"));
        let len = u32::from_be_bytes(header[17..21].try_into().expect("4 bytes")) as usize;
        let Some(direction) = Direction::from_byte(header[16]) else {
            break JournalEnd::Damaged {
                error: JournalError::BadDirection {
                    offset,
                    found: header[16],
                },
            };
        };
        if len > MAX_RECORD_BYTES {
            break JournalEnd::Damaged {
                error: JournalError::RecordTooLarge {
                    offset,
                    len,
                    max: MAX_RECORD_BYTES,
                },
            };
        }
        let expected = records.len() as u64;
        if seq != expected {
            break JournalEnd::Damaged {
                error: JournalError::OutOfOrder {
                    offset,
                    expected,
                    found: seq,
                },
            };
        }
        let Some(body) = rest.get(RECORD_HEADER_BYTES..RECORD_HEADER_BYTES + len) else {
            break JournalEnd::Cut { offset };
        };
        records.push(JournalRecord {
            seq,
            wall_ns,
            direction,
            bytes: body.to_vec(),
        });
        offset += RECORD_HEADER_BYTES + len;
    };
    JournalRead {
        records,
        valid_len: offset,
        end,
    }
}
