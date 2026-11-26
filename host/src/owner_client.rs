//! A client of the daemon's owner socket (0.2, The wire): what the CLI uses to ask the daemon for something. It
//! connects, reads the token from the token file (which [`crate::auth::read_token_file`] checks for mode `0600`), sends
//! `initialize`, and then makes calls one at a time, each answered before the next is sent. The messages are `core`'s
//! ([`collide_core::owner`]); the server is [`crate::daemon`].

use crate::auth;
use crate::daemon::DaemonPaths;
use collide_core::owner::{
    JsonRpc2, MAX_MESSAGE_BYTES, Notification, OWNER_PROTOCOL, OwnerInitializeParams,
    OwnerInitializeResult, Request, RequestId, Response, RpcError, encode_line,
};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::fmt;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;

/// Why a call to the daemon did not give a result.
#[derive(Debug)]
pub enum ClientError {
    /// The socket, the token file or a read or write failed; `context` says which.
    Io { context: String, source: io::Error },
    /// The daemon answered with an error.
    Rpc(RpcError),
    /// The daemon's answer was not what the protocol says.
    Protocol(String),
}

impl fmt::Display for ClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClientError::Io { context, source } => write!(f, "{context}: {source}"),
            ClientError::Rpc(error) => write!(f, "the daemon said: {}", error.message),
            ClientError::Protocol(message) => {
                write!(f, "the daemon's answer was not readable: {message}")
            }
        }
    }
}

impl std::error::Error for ClientError {}

/// What reading the next notice found.
#[derive(Debug)]
pub enum Incoming {
    Notice(Notification),
    /// Nothing came within the wait ([`OwnerClient::set_read_wait`]).
    Quiet,
    /// The daemon ended the stream: it closed the connection, after the notices it had already sent.
    Closed,
}

/// One connection that has done `initialize`.
pub struct OwnerClient {
    writer: UnixStream,
    reader: BufReader<UnixStream>,
    next_id: i64,
    /// The start of a notice that a wait that ran out left ([`OwnerClient::next_notice`]).
    partial: String,
}

impl OwnerClient {
    /// Connects to the daemon at `paths.socket` and initializes as `client` (a name for the daemon's log).
    pub fn connect(paths: &DaemonPaths, client: &str) -> Result<OwnerClient, ClientError> {
        OwnerClient::connect_as(paths, client, false)
    }

    /// Like [`OwnerClient::connect`], with `defaults` asking for Start with defaults: the daemon reads only the Default
    /// layer, the built-in packs and the built-in views for this connection, and refuses its writes.
    pub fn connect_as(
        paths: &DaemonPaths,
        client: &str,
        defaults: bool,
    ) -> Result<OwnerClient, ClientError> {
        let io_error = |context: String| move |source| ClientError::Io { context, source };
        let token = auth::read_token_file(&paths.token_file).map_err(|error| ClientError::Io {
            context: "reading the token file".to_string(),
            source: io::Error::other(error.to_string()),
        })?;
        let stream = UnixStream::connect(&paths.socket).map_err(io_error(format!(
            "connecting to {}",
            paths.socket.display()
        )))?;
        let reader = stream
            .try_clone()
            .map_err(io_error("setting up the connection".to_string()))?;
        let mut connected = OwnerClient {
            writer: stream,
            reader: BufReader::new(reader),
            next_id: 1,
            partial: String::new(),
        };
        let _: OwnerInitializeResult = connected.call(
            "initialize",
            &OwnerInitializeParams {
                protocol: OWNER_PROTOCOL,
                token: Some(token.trim().to_string()),
                client: Some(client.to_string()),
                defaults,
            },
            Some(Duration::from_secs(10)),
        )?;
        Ok(connected)
    }

    /// Whether the daemon has closed the connection, asked of the socket without waiting. A socket that cannot be asked
    /// is taken to be closed. Bytes waiting on it are not read or lost.
    pub fn is_closed(&mut self) -> bool {
        if self.reader.get_ref().set_nonblocking(true).is_err() {
            return true;
        }
        let ended = match self.reader.fill_buf() {
            // The end of the stream is nothing to read. Bytes that are waiting mean it is open, and stay unread.
            Ok(bytes) => bytes.is_empty(),
            Err(e) => !would_wait(&e),
        };
        ended || self.reader.get_ref().set_nonblocking(false).is_err()
    }

    /// How long [`OwnerClient::next_notice`] waits for a notice (`None` waits for as long as it takes). Set it once,
    /// right after the answer to `daemon.subscribe`. On macOS a socket option cannot be set after the daemon has shut
    /// the stream down, so the error is the client's to ignore: it then reads the notices that were sent and the end of
    /// the stream, which it needs to do anyway.
    pub fn set_read_wait(&self, wait: Option<Duration>) -> io::Result<()> {
        self.reader.get_ref().set_read_timeout(wait)
    }

    /// The next notice the daemon sends unasked, for a client that subscribed. A notice whose first half arrived before
    /// the wait ran out is kept and finished by the next read.
    pub fn next_notice(&mut self) -> Result<Incoming, ClientError> {
        let io_error = |source| ClientError::Io {
            context: "reading a notice from the daemon".to_string(),
            source,
        };
        match (&mut self.reader)
            .take(MAX_MESSAGE_BYTES as u64 + 1)
            .read_line(&mut self.partial)
        {
            Ok(0) => Ok(Incoming::Closed),
            Ok(_) if !self.partial.ends_with('\n') => {
                if self.partial.len() > MAX_MESSAGE_BYTES {
                    Err(ClientError::Protocol(
                        "a notice is over the size limit".to_string(),
                    ))
                } else {
                    // The stream ended in the middle of a line.
                    Ok(Incoming::Closed)
                }
            }
            Ok(_) => {
                let line = std::mem::take(&mut self.partial);
                serde_json::from_str::<Notification>(&line)
                    .map(Incoming::Notice)
                    .map_err(|e| ClientError::Protocol(format!("a notice: {e}")))
            }
            Err(e) if would_wait(&e) => Ok(Incoming::Quiet),
            Err(e) if e.kind() == io::ErrorKind::ConnectionReset => Ok(Incoming::Closed),
            Err(e) => Err(io_error(e)),
        }
    }

    /// One call: sends the request and reads its answer, waiting up to `wait` for it (`None` waits for as long as the
    /// daemon takes, which a stop with a long grace period needs).
    pub fn call<P: Serialize, R: DeserializeOwned>(
        &mut self,
        method: &str,
        params: &P,
        wait: Option<Duration>,
    ) -> Result<R, ClientError> {
        let io_error = |context: &'static str| {
            move |source| ClientError::Io {
                context: context.to_string(),
                source,
            }
        };
        let id = self.next_id;
        self.next_id += 1;
        let request = Request {
            jsonrpc: JsonRpc2,
            id: RequestId::Number(id),
            method: method.to_string(),
            params: serde_json::to_value(params)
                .map_err(|e| ClientError::Protocol(format!("the params: {e}")))?,
        };
        self.writer
            .set_read_timeout(wait)
            .map_err(io_error("setting the wait"))?;
        self.writer
            .write_all(encode_line(&request).as_bytes())
            .map_err(io_error("sending the request"))?;
        let mut line = String::new();
        // One more byte than the limit, so a line over it is seen.
        let read = (&mut self.reader)
            .take(MAX_MESSAGE_BYTES as u64 + 1)
            .read_line(&mut line)
            .map_err(io_error("waiting for the daemon's answer"))?;
        if read == 0 {
            return Err(ClientError::Protocol(
                "the daemon closed the connection without answering".to_string(),
            ));
        }
        match serde_json::from_str::<Response>(&line)
            .map_err(|e| ClientError::Protocol(e.to_string()))?
        {
            Response::Success { result, .. } => serde_json::from_value(result)
                .map_err(|e| ClientError::Protocol(format!("the result: {e}"))),
            Response::Failure { error, .. } => Err(ClientError::Rpc(error)),
        }
    }
}

/// A read that found nothing yet, because its wait ran out.
fn would_wait(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
    )
}
