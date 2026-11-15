//! A client of the daemon's owner socket (0.2, The wire): what the CLI uses to ask the daemon for something. It
//! connects, reads the token from the token file (which [`crate::auth::read_token_file`] checks for mode `0600`), sends
//! `initialize`, and then makes calls one at a time, each answered before the next is sent. The messages are `core`'s
//! ([`collide_core::owner`]); the server is [`crate::daemon`].

use crate::auth;
use crate::daemon::DaemonPaths;
use collide_core::owner::{
    JsonRpc2, MAX_MESSAGE_BYTES, OWNER_PROTOCOL, OwnerInitializeParams, OwnerInitializeResult,
    Request, RequestId, Response, RpcError, encode_line,
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

/// One connection that has done `initialize`.
pub struct OwnerClient {
    writer: UnixStream,
    reader: BufReader<UnixStream>,
    next_id: i64,
}

impl OwnerClient {
    /// Connects to the daemon at `paths.socket` and initializes as `client` (a name for the daemon's log).
    pub fn connect(paths: &DaemonPaths, client: &str) -> Result<OwnerClient, ClientError> {
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
        };
        let _: OwnerInitializeResult = connected.call(
            "initialize",
            &OwnerInitializeParams {
                protocol: OWNER_PROTOCOL,
                token: Some(token.trim().to_string()),
                client: Some(client.to_string()),
            },
            Some(Duration::from_secs(10)),
        )?;
        Ok(connected)
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
