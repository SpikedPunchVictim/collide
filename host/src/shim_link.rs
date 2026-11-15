//! The link between the daemon and a shim, the parts both sides share and the daemon's half of the handshake
//! (0.2, The owner is `collide-daemon`, with a per-child shim; Stage 3a; `ts-0057-ck`). The messages are `core`'s
//! ([`collide_core::shim`]) and the framing is [`collide_core::frame`]; this module reads and writes them on a stream
//! socket.
//!
//! ## Reading frames from a socket
//!
//! A stream socket gives a reader what has arrived, which may be half a message, and a peer can die in the middle of
//! one. [`FrameReader`] keeps the bytes it has between reads and hands out a message only when every byte of it is
//! there ([`collide_core::frame::decode_message`]), so a message cut in half is never read as whole: it stays
//! incomplete until the rest arrives, or it is dropped with the connection ([`Next::Closed`] says how many bytes were
//! dropped). A length over the frame limit is an error as soon as the prefix is read, so a damaged prefix never makes a
//! reader wait for gigabytes.
//!
//! The reader uses a short read timeout and answers [`Next::Idle`] when the time passes with no whole message, so the
//! thread that reads can look at its own flags between reads. A partial message survives an idle turn.
//!
//! ## The handshake, from the daemon's side
//!
//! The shim owns the socket, so the daemon connects and the shim speaks first: [`ShimMessage::Hello`] with the
//! protocol the shim speaks. The daemon answers with [`collide_core::shim::answer_hello`]: `Welcome`, naming the
//! version both sides speak from then on, or `Refused`, which closes the connection. [`connect_shim`] does that, after
//! checking that the shim's uid is the daemon's own (0.2, The socket: a shim's socket follows the daemon's rules).

use crate::auth::{self, UnixPeer};
use crate::daemon::SocketError;
use crate::sysproc;
use collide_core::frame::{FrameError, decode_message, encode_message};
use collide_core::shim::{DaemonMessage, SHIM_PROTOCOL, ShimMessage, answer_hello};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::fmt;
use std::io::{self, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::{Duration, Instant};

/// How many bytes a reader asks the socket for at once.
const READ_CHUNK_BYTES: usize = 64 * 1024;

/// What one turn of [`FrameReader::read_next`] found.
#[derive(Debug)]
pub enum Next<T> {
    /// A whole message.
    Message(T),
    /// The read timeout passed with no whole message. Any bytes of a partial one are kept for the next turn.
    Idle,
    /// The peer closed the connection. `dropped` is the number of bytes of a message it had not finished: they were
    /// never read as a message.
    Closed { dropped: usize },
}

/// Why a frame could not be read.
#[derive(Debug)]
pub enum ReadError {
    Io(io::Error),
    /// A length over the limit, or a whole frame that is not the message it should be. The stream cannot be taken up
    /// again after this: the caller closes the connection.
    Frame(FrameError),
}

impl fmt::Display for ReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReadError::Io(error) => write!(f, "reading from the socket: {error}"),
            ReadError::Frame(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for ReadError {}

/// Reads length-framed messages from a stream socket, keeping the bytes of a message that has not all arrived.
#[derive(Debug)]
pub struct FrameReader {
    stream: UnixStream,
    buffer: Vec<u8>,
}

impl FrameReader {
    /// A reader on `stream` that gives up each read after `poll` ([`Next::Idle`]).
    pub fn new(stream: UnixStream, poll: Duration) -> FrameReader {
        // A stream that cannot take a timeout would block a reader for ever, so the failure is not ignored: the next
        // read says so with an error of its own, and a socket that is already closed is closed for the reader too.
        let _ = stream.set_read_timeout(Some(poll));
        FrameReader {
            stream,
            buffer: Vec::new(),
        }
    }

    /// The next message: a whole one, or the reason there is none yet.
    pub fn read_next<T: DeserializeOwned>(&mut self) -> Result<Next<T>, ReadError> {
        loop {
            if let Some((message, used)) =
                decode_message::<T>(&self.buffer).map_err(ReadError::Frame)?
            {
                self.buffer.drain(..used);
                return Ok(Next::Message(message));
            }
            let mut chunk = vec![0u8; READ_CHUNK_BYTES];
            match self.stream.read(&mut chunk) {
                Ok(0) => {
                    return Ok(Next::Closed {
                        dropped: self.buffer.len(),
                    });
                }
                Ok(read) => self.buffer.extend_from_slice(&chunk[..read]),
                Err(e)
                    if matches!(
                        e.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                    ) =>
                {
                    return Ok(Next::Idle);
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                // A peer that went away without a goodbye is a close, not a failure of this side.
                Err(e)
                    if matches!(
                        e.kind(),
                        io::ErrorKind::ConnectionReset | io::ErrorKind::BrokenPipe
                    ) =>
                {
                    return Ok(Next::Closed {
                        dropped: self.buffer.len(),
                    });
                }
                Err(e) => return Err(ReadError::Io(e)),
            }
        }
    }

    /// The bytes held of a message that has not all arrived.
    pub fn pending_bytes(&self) -> usize {
        self.buffer.len()
    }
}

/// Writes `message` as one frame. The whole frame is written with one `write_all`, so the stream gets the prefix and
/// the payload together; a write timeout on the stream bounds how long a peer that does not read can hold the caller.
pub fn send<T: Serialize>(stream: &mut UnixStream, message: &T) -> io::Result<()> {
    let frame =
        encode_message(message).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    stream.write_all(&frame)?;
    stream.flush()
}

/// Why the daemon could not take up a connection to a shim.
#[derive(Debug)]
pub enum ConnectError {
    /// Nothing is listening at the socket, or it could not be reached.
    Io { context: String, source: io::Error },
    /// The process at the socket is another user's. The daemon does not talk to it.
    ForeignUid { uid: u32, daemon_uid: u32 },
    /// The shim did not say `Hello` while the caller was willing to wait. It may be stopped, or hung.
    NoHello { waited: Duration },
    /// The shim said something that is not a `Hello`, or closed the connection first.
    Protocol { message: String },
    /// The shim speaks a protocol version this daemon does not accept. The message names it, the current one and the
    /// previous one.
    Refused { message: String },
}

impl fmt::Display for ConnectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConnectError::Io { context, source } => write!(f, "{context}: {source}"),
            ConnectError::ForeignUid { uid, daemon_uid } => write!(
                f,
                "the process at the shim's socket is uid {uid}, and the daemon's user is uid {daemon_uid}"
            ),
            ConnectError::NoHello { waited } => write!(
                f,
                "the shim did not say hello within {} ms",
                waited.as_millis()
            ),
            ConnectError::Protocol { message } => write!(f, "the shim's handshake: {message}"),
            ConnectError::Refused { message } => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for ConnectError {}

/// A connection to a shim that has passed the handshake.
#[derive(Debug)]
pub struct ShimConnection {
    /// For writing. Its write timeout is the one given to [`connect_shim`].
    pub writer: UnixStream,
    pub reader: FrameReader,
    /// The shim's process id, as the kernel reports it for the socket (`LOCAL_PEERPID`).
    pub shim_pid: Option<u32>,
    /// The protocol both sides speak on this connection.
    pub protocol: u32,
}

/// What [`connect_shim`] reports when it fails after it has found who is at the socket: the pid, so that a shim that
/// is stopped (and so says nothing) can still be named.
pub struct ConnectFailure {
    pub error: ConnectError,
    pub shim_pid: Option<u32>,
}

/// Connects to the shim's socket and does the daemon's half of the handshake: checks the peer's uid, reads `Hello`, and
/// answers it. `poll` is the reader's turn and `write_wait` bounds a write.
///
/// While the `Hello` has not come, `keep_waiting` is called after each turn with how long the wait has been, and the
/// wait goes on while it answers `true`. The caller decides when a shim that says nothing is unresponsive, marks it,
/// and decides when to give up, so the one connection is kept and a stopped shim is not given a queue of them.
pub fn connect_shim(
    socket: &Path,
    poll: Duration,
    write_wait: Duration,
    mut keep_waiting: impl FnMut(Duration) -> bool,
) -> Result<ShimConnection, ConnectFailure> {
    let fail = |error, shim_pid| ConnectFailure { error, shim_pid };
    let stream = UnixStream::connect(socket).map_err(|source| {
        fail(
            ConnectError::Io {
                context: format!("connecting to the shim's socket {}", socket.display()),
                source,
            },
            None,
        )
    })?;
    let shim_pid = sysproc::peer_pid(&stream).ok();
    let daemon_uid = sysproc::daemon_uid();
    // The same check a shim makes of the daemon: the uid at the other end must be this process's own.
    match auth::admit_shim(daemon_uid, &UnixPeer { stream: &stream }) {
        Ok(()) => {}
        Err(SocketError::ForeignUid { uid, daemon_uid }) => {
            return Err(fail(ConnectError::ForeignUid { uid, daemon_uid }, shim_pid));
        }
        Err(other) => {
            let source = io::Error::other(other.to_string());
            let context = "reading the uid of the shim's process".to_string();
            return Err(fail(ConnectError::Io { context, source }, shim_pid));
        }
    }
    let io_error = |context: &str| {
        let context = context.to_string();
        move |source| {
            fail(
                ConnectError::Io {
                    context: context.clone(),
                    source,
                },
                shim_pid,
            )
        }
    };
    stream
        .set_write_timeout(Some(write_wait))
        .map_err(io_error("setting the write timeout"))?;
    let mut writer = stream
        .try_clone()
        .map_err(io_error("cloning the shim's socket"))?;
    let mut reader = FrameReader::new(stream, poll);
    let started = Instant::now();
    let announced = loop {
        match reader.read_next::<ShimMessage>() {
            Ok(Next::Message(ShimMessage::Hello { protocol })) => break protocol,
            Ok(Next::Message(other)) => {
                return Err(fail(
                    ConnectError::Protocol {
                        message: format!("the first message was {other:?}, not a hello"),
                    },
                    shim_pid,
                ));
            }
            Ok(Next::Idle) => {
                let waited = started.elapsed();
                if !keep_waiting(waited) {
                    return Err(fail(ConnectError::NoHello { waited }, shim_pid));
                }
            }
            Ok(Next::Closed { .. }) => {
                return Err(fail(
                    ConnectError::Protocol {
                        message: "the shim closed the connection before it said hello".to_string(),
                    },
                    shim_pid,
                ));
            }
            Err(error) => {
                return Err(fail(
                    ConnectError::Protocol {
                        message: error.to_string(),
                    },
                    shim_pid,
                ));
            }
        }
    };
    let answer = answer_hello(SHIM_PROTOCOL, announced);
    send(&mut writer, &answer).map_err(io_error("answering the shim's hello"))?;
    match answer {
        DaemonMessage::Welcome { protocol } => Ok(ShimConnection {
            writer,
            reader,
            shim_pid,
            protocol,
        }),
        DaemonMessage::Refused { message } => {
            Err(fail(ConnectError::Refused { message }, shim_pid))
        }
        _ => unreachable!("`answer_hello` answers with a welcome or a refusal"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use collide_core::shim::DaemonMessage;

    fn pair() -> (UnixStream, FrameReader) {
        let (a, b) = UnixStream::pair().unwrap();
        (a, FrameReader::new(b, Duration::from_millis(20)))
    }

    #[test]
    fn half_a_frame_is_idle_and_the_rest_makes_it_whole() {
        let (mut peer, mut reader) = pair();
        let frame = encode_message(&DaemonMessage::Write {
            data: "hello".into(),
        })
        .unwrap();
        let (head, tail) = frame.split_at(frame.len() / 2);
        peer.write_all(head).unwrap();
        assert!(matches!(
            reader.read_next::<DaemonMessage>().unwrap(),
            Next::Idle
        ));
        assert_eq!(reader.pending_bytes(), head.len());
        peer.write_all(tail).unwrap();
        match reader.read_next::<DaemonMessage>().unwrap() {
            Next::Message(DaemonMessage::Write { data }) => assert_eq!(data, "hello"),
            other => panic!("expected the whole message, got {other:?}"),
        }
    }

    #[test]
    fn a_frame_cut_by_a_close_is_dropped_and_counted() {
        let (mut peer, mut reader) = pair();
        let frame = encode_message(&DaemonMessage::CloseStdin {}).unwrap();
        peer.write_all(&frame[..frame.len() - 1]).unwrap();
        drop(peer);
        match reader.read_next::<DaemonMessage>().unwrap() {
            Next::Closed { dropped } => assert_eq!(dropped, frame.len() - 1),
            other => panic!("expected a close, got {other:?}"),
        }
    }

    #[test]
    fn two_frames_in_one_read_come_out_one_at_a_time() {
        let (mut peer, mut reader) = pair();
        let mut both = encode_message(&DaemonMessage::CloseStdin {}).unwrap();
        both.extend(encode_message(&DaemonMessage::AckExit {}).unwrap());
        peer.write_all(&both).unwrap();
        assert!(matches!(
            reader.read_next::<DaemonMessage>().unwrap(),
            Next::Message(DaemonMessage::CloseStdin {})
        ));
        assert!(matches!(
            reader.read_next::<DaemonMessage>().unwrap(),
            Next::Message(DaemonMessage::AckExit {})
        ));
    }

    #[test]
    fn a_length_over_the_limit_is_an_error_at_once() {
        let (mut peer, mut reader) = pair();
        peer.write_all(&u32::MAX.to_be_bytes()).unwrap();
        assert!(matches!(
            reader.read_next::<DaemonMessage>(),
            Err(ReadError::Frame(_))
        ));
    }
}
