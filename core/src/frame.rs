//! Length framing for the messages between the daemon and a shim (0.2, the shim). A frame is written to a stream
//! socket, where a write can arrive in pieces and a peer can die half way through one. The length prefix is how a
//! half-written message is never taken for a whole one: a decoder returns a message only when every byte of it is
//! there.
//!
//! ## Byte layout
//!
//! ```text
//! +----------------------+---------------------------+
//! | length: u32, big-endian | payload: `length` bytes |
//! +----------------------+---------------------------+
//! ```
//!
//! - The prefix is 4 bytes ([`FRAME_PREFIX_BYTES`]), most significant byte first. It counts the payload only, not
//!   itself.
//! - A payload may be empty. It is at most [`MAX_FRAME_PAYLOAD`] bytes: a longer length is refused when the prefix
//!   is read, so a damaged prefix never makes a reader wait for 4 GiB.
//! - For a message, the payload is the message as compact JSON (UTF-8). [`encode_message`] and [`decode_message`]
//!   do that; the types are in [`crate::shim`].
//!
//! The codec is pure: bytes in, a value or a typed error out. It reads no socket and no clock, and it does not
//! panic on any input. A caller that reads a socket keeps the bytes it has, calls [`decode_frame`] after each read,
//! and drops `consumed` bytes after each whole frame.

use serde::Serialize;
use serde::de::DeserializeOwned;
use std::fmt;

/// The width of the length prefix, in bytes.
pub const FRAME_PREFIX_BYTES: usize = 4;

/// The most bytes in one frame's payload (16 MiB).
pub const MAX_FRAME_PAYLOAD: usize = 16 * 1024 * 1024;

/// What [`decode_frame`] found at the start of a buffer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decoded<'a> {
    /// A whole frame: its payload, and how many bytes of the buffer it used (prefix included).
    Whole { payload: &'a [u8], consumed: usize },
    /// Not all of a frame is here. `needed` is how many more bytes complete it, once the prefix is known; with
    /// fewer than [`FRAME_PREFIX_BYTES`] bytes it is how many more complete the prefix, and is a lower bound.
    Incomplete { needed: usize },
}

/// Why a frame was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameError {
    /// The length is over [`MAX_FRAME_PAYLOAD`]: a damaged prefix, or a message that is too big to send.
    TooLarge { len: usize, max: usize },
    /// A whole frame whose payload is not the message it should be, or a message that cannot be written.
    Payload { reason: String },
}

impl fmt::Display for FrameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FrameError::TooLarge { len, max } => write!(
                f,
                "frame length {len} is over the limit of {max} bytes (a damaged length prefix, or a message too big to send)"
            ),
            FrameError::Payload { reason } => write!(f, "frame payload is not a message: {reason}"),
        }
    }
}

impl std::error::Error for FrameError {}

/// Frames `payload`: the prefix, then the payload. Refuses a payload over [`MAX_FRAME_PAYLOAD`].
pub fn encode_frame(payload: &[u8]) -> Result<Vec<u8>, FrameError> {
    if payload.len() > MAX_FRAME_PAYLOAD {
        return Err(FrameError::TooLarge {
            len: payload.len(),
            max: MAX_FRAME_PAYLOAD,
        });
    }
    let mut frame = Vec::with_capacity(FRAME_PREFIX_BYTES + payload.len());
    frame.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    frame.extend_from_slice(payload);
    Ok(frame)
}

/// Reads one frame from the start of `buf`. A buffer that holds less than a whole frame, down to no bytes at all,
/// is `Incomplete`, never `Whole`: more bytes may still arrive. Bytes after the frame are left alone.
pub fn decode_frame(buf: &[u8]) -> Result<Decoded<'_>, FrameError> {
    let Some(prefix) = buf.first_chunk::<FRAME_PREFIX_BYTES>() else {
        return Ok(Decoded::Incomplete {
            needed: FRAME_PREFIX_BYTES - buf.len(),
        });
    };
    let len = u32::from_be_bytes(*prefix) as usize;
    if len > MAX_FRAME_PAYLOAD {
        return Err(FrameError::TooLarge {
            len,
            max: MAX_FRAME_PAYLOAD,
        });
    }
    let consumed = FRAME_PREFIX_BYTES + len;
    match buf.get(FRAME_PREFIX_BYTES..consumed) {
        Some(payload) => Ok(Decoded::Whole { payload, consumed }),
        None => Ok(Decoded::Incomplete {
            needed: consumed - buf.len(),
        }),
    }
}

/// Writes `message` as compact JSON in one frame.
pub fn encode_message<T: Serialize>(message: &T) -> Result<Vec<u8>, FrameError> {
    let payload = serde_json::to_vec(message).map_err(|error| FrameError::Payload {
        reason: format!("cannot write the message: {error}"),
    })?;
    encode_frame(&payload)
}

/// Reads one message from the start of `buf`: `None` while the frame is not all here, and `Some((message,
/// consumed))` once it is. A whole frame whose payload is not a `T` is an error that names the cause.
pub fn decode_message<T: DeserializeOwned>(buf: &[u8]) -> Result<Option<(T, usize)>, FrameError> {
    match decode_frame(buf)? {
        Decoded::Incomplete { .. } => Ok(None),
        Decoded::Whole { payload, consumed } => {
            let message = serde_json::from_slice(payload).map_err(|error| FrameError::Payload {
                reason: error.to_string(),
            })?;
            Ok(Some((message, consumed)))
        }
    }
}
